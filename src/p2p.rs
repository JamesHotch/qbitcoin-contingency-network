use crate::{
    block::{Block, BlockHeader},
    crypto::{DomainTag, signature_digest},
    hash::Hash32,
    params::{ChainParams, Network},
    transaction::Transaction,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    net::{IpAddr, SocketAddr},
    time::Duration,
};
use thiserror::Error;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub const MAX_P2P_MESSAGE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_HEADERS_PER_MESSAGE: usize = 2_000;
pub const P2P_PROTOCOL_VERSION: u16 = 1;

#[derive(Debug, Error)]
pub enum P2pError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error("peer message exceeds the size limit")]
    MessageTooLarge,
    #[error("peer message uses another network magic")]
    WrongMagic,
    #[error("peer handshake belongs to another chain")]
    WrongNetwork,
    #[error("peer protocol version is unsupported")]
    UnsupportedVersion,
    #[error("peer operation timed out")]
    Timeout,
    #[error("header batch exceeds its consensus-independent relay limit")]
    TooManyHeaders,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Handshake {
    pub protocol_version: u16,
    pub chain_id: Hash32,
    pub network: Network,
    pub services: u64,
    pub nonce: u64,
    pub user_agent: String,
    pub start_height: u64,
    pub timestamp: u64,
    pub commitment: Hash32,
}

impl Handshake {
    #[must_use]
    pub fn new(
        params: &ChainParams,
        services: u64,
        nonce: u64,
        user_agent: String,
        start_height: u64,
        timestamp: u64,
    ) -> Self {
        let mut payload = Vec::new();
        payload.extend_from_slice(&services.to_le_bytes());
        payload.extend_from_slice(&nonce.to_le_bytes());
        payload.extend_from_slice(user_agent.as_bytes());
        payload.extend_from_slice(&start_height.to_le_bytes());
        payload.extend_from_slice(&timestamp.to_le_bytes());
        let commitment = signature_digest(&params.chain_id, DomainTag::NetworkHandshake, &payload);
        Self {
            protocol_version: P2P_PROTOCOL_VERSION,
            chain_id: params.chain_id,
            network: params.network,
            services,
            nonce,
            user_agent,
            start_height,
            timestamp,
            commitment,
        }
    }

    pub fn validate(&self, params: &ChainParams) -> Result<(), P2pError> {
        if self.protocol_version != P2P_PROTOCOL_VERSION {
            return Err(P2pError::UnsupportedVersion);
        }
        if self.chain_id != params.chain_id || self.network != params.network {
            return Err(P2pError::WrongNetwork);
        }
        let expected = Self::new(
            params,
            self.services,
            self.nonce,
            self.user_agent.clone(),
            self.start_height,
            self.timestamp,
        );
        if self.commitment != expected.commitment {
            return Err(P2pError::WrongNetwork);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum PeerMessage {
    Handshake(Handshake),
    GetHeaders {
        locator: Vec<Hash32>,
        stop: Option<Hash32>,
    },
    Headers(Vec<BlockHeader>),
    Transaction(Transaction),
    Block(Block),
    Ping(u64),
    Pong(u64),
}

pub async fn write_message<W: AsyncWrite + Unpin>(
    writer: &mut W,
    magic: [u8; 4],
    message: &PeerMessage,
    timeout: Duration,
) -> Result<(), P2pError> {
    if matches!(message, PeerMessage::Headers(headers) if headers.len() > MAX_HEADERS_PER_MESSAGE) {
        return Err(P2pError::TooManyHeaders);
    }
    let payload = serde_json::to_vec(message)?;
    if payload.len() > MAX_P2P_MESSAGE_BYTES {
        return Err(P2pError::MessageTooLarge);
    }
    let length = u32::try_from(payload.len()).map_err(|_| P2pError::MessageTooLarge)?;
    tokio::time::timeout(timeout, async {
        writer.write_all(&magic).await?;
        writer.write_all(&length.to_le_bytes()).await?;
        writer.write_all(&payload).await?;
        writer.flush().await
    })
    .await
    .map_err(|_| P2pError::Timeout)??;
    Ok(())
}

pub async fn read_message<R: AsyncRead + Unpin>(
    reader: &mut R,
    expected_magic: [u8; 4],
    timeout: Duration,
) -> Result<PeerMessage, P2pError> {
    let mut header = [0_u8; 8];
    tokio::time::timeout(timeout, reader.read_exact(&mut header))
        .await
        .map_err(|_| P2pError::Timeout)??;
    if header[..4] != expected_magic {
        return Err(P2pError::WrongMagic);
    }
    let length = u32::from_le_bytes(
        header[4..]
            .try_into()
            .map_err(|_| P2pError::MessageTooLarge)?,
    ) as usize;
    if length > MAX_P2P_MESSAGE_BYTES {
        return Err(P2pError::MessageTooLarge);
    }
    let mut payload = vec![0_u8; length];
    tokio::time::timeout(timeout, reader.read_exact(&mut payload))
        .await
        .map_err(|_| P2pError::Timeout)??;
    let message: PeerMessage = serde_json::from_slice(&payload)?;
    if matches!(&message, PeerMessage::Headers(headers) if headers.len() > MAX_HEADERS_PER_MESSAGE)
    {
        return Err(P2pError::TooManyHeaders);
    }
    Ok(message)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PeerRecord {
    pub address: SocketAddr,
    pub score: i32,
    pub cooldown_until: u64,
    pub last_seen: u64,
}

#[derive(Debug, Clone, Default)]
pub struct PeerBook {
    peers: BTreeMap<SocketAddr, PeerRecord>,
}

impl PeerBook {
    pub fn observe(&mut self, address: SocketAddr, now: u64) {
        self.peers
            .entry(address)
            .and_modify(|peer| peer.last_seen = now)
            .or_insert(PeerRecord {
                address,
                score: 0,
                cooldown_until: 0,
                last_seen: now,
            });
    }

    pub fn penalize(&mut self, address: SocketAddr, points: i32, now: u64) {
        self.observe(address, now);
        let peer = self.peers.get_mut(&address).expect("peer was inserted");
        peer.score = peer.score.saturating_sub(points.max(0));
        if peer.score <= -100 {
            peer.cooldown_until = now.saturating_add(3_600);
        }
    }

    #[must_use]
    pub fn is_available(&self, address: SocketAddr, now: u64) -> bool {
        self.peers
            .get(&address)
            .is_none_or(|peer| peer.cooldown_until <= now)
    }

    #[must_use]
    pub fn diversity_groups(&self) -> usize {
        self.peers
            .keys()
            .map(|address| ip_group(address.ip()))
            .collect::<std::collections::BTreeSet<_>>()
            .len()
    }

    #[must_use]
    pub fn records(&self) -> impl Iterator<Item = &PeerRecord> {
        self.peers.values()
    }
}

fn ip_group(address: IpAddr) -> Vec<u8> {
    match address {
        IpAddr::V4(ip) => ip.octets()[..2].to_vec(),
        IpAddr::V6(ip) => ip.octets()[..4].to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handshake_rejects_cross_network_replay() {
        let regtest = ChainParams::for_network(Network::Regtest);
        let testnet = ChainParams::for_network(Network::Testnet);
        let handshake = Handshake::new(&regtest, 1, 42, "qbtc-test".to_owned(), 1, 10);
        handshake.validate(&regtest).unwrap();
        assert!(matches!(
            handshake.validate(&testnet),
            Err(P2pError::WrongNetwork)
        ));
    }

    #[test]
    fn repeated_misbehaviour_causes_a_cooldown() {
        let mut peers = PeerBook::default();
        let address: SocketAddr = "192.0.2.10:38333".parse().unwrap();
        peers.penalize(address, 50, 100);
        peers.penalize(address, 50, 101);
        assert!(!peers.is_available(address, 102));
        assert!(peers.is_available(address, 3_702));
    }

    #[tokio::test]
    async fn framed_message_round_trips_with_limits() {
        let (mut writer, mut reader) = tokio::io::duplex(4_096);
        let message = PeerMessage::Ping(7);
        let write = write_message(&mut writer, [1, 2, 3, 4], &message, Duration::from_secs(1));
        let read = read_message(&mut reader, [1, 2, 3, 4], Duration::from_secs(1));
        let (write_result, read_result) = tokio::join!(write, read);
        write_result.unwrap();
        assert_eq!(read_result.unwrap(), message);
    }
}
