use crate::{
    block::{Block, BlockHeader},
    chain::{ChainError, ChainState, SubmitOutcome},
    crypto::{HybridPublicKeys, RustCryptoProvider},
    finality::{CheckpointCertificate, CheckpointResult, ConsensusStage, FinalityState},
    mempool::Mempool,
    p2p::{Handshake, P2pError, PeerBook, PeerMessage, read_message, write_message},
    params::{ChainParams, ChainParamsError},
    storage::{ChainStore, StorageError},
    transaction::{OutputPolicy, Transaction},
};
use axum::{
    Json, Router,
    extract::{Path, State},
    http::{HeaderValue, StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
};
use primitive_types::U256;
use serde::{Deserialize, Serialize};
use std::{
    net::SocketAddr,
    path::Path as FsPath,
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use tokio::{
    net::{TcpListener, TcpStream},
    sync::RwLock,
};
use tower_http::{cors::CorsLayer, limit::RequestBodyLimitLayer, trace::TraceLayer};

pub type SharedNode = Arc<RwLock<NodeRuntime>>;

#[derive(Debug, Error)]
pub enum NodeError {
    #[error(transparent)]
    ChainParams(#[from] ChainParamsError),
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error(transparent)]
    Chain(#[from] ChainError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    P2p(#[from] P2pError),
}

pub struct NodeRuntime {
    pub chain: ChainState,
    pub finality: FinalityState,
    pub mempool: Mempool,
    pub peers: PeerBook,
    store: ChainStore,
    started_at: u64,
    recent_reorg_depth: u64,
}

impl NodeRuntime {
    pub fn open(params: ChainParams, data_dir: impl AsRef<FsPath>) -> Result<Self, NodeError> {
        params.enforce_startup(false, params.committee.local_development)?;
        let mut store = ChainStore::open(data_dir)?;
        let chain = store.load(params.clone(), &RustCryptoProvider)?;
        store.persist_active_chain(&chain)?;
        Ok(Self {
            chain,
            finality: FinalityState::new(params.finality.clone(), params.committee.clone()),
            mempool: Mempool::default(),
            peers: PeerBook::default(),
            store,
            started_at: unix_time(),
            recent_reorg_depth: 0,
        })
    }

    fn status(&self) -> StatusResponse {
        let latest_checkpoint = self.finality.latest_checkpoint();
        StatusResponse {
            network: self.chain.params().network.name().to_owned(),
            chain_id: self.chain.params().chain_id.to_string(),
            height: self.chain.height(),
            tip_hash: self.chain.tip_hash().to_string(),
            cumulative_chainwork: u256_hex(self.chain.chainwork()),
            difficulty_bits: self
                .chain
                .block(self.chain.tip_hash())
                .map_or(0, |block| block.header.bits),
            recent_reorg_depth: self.recent_reorg_depth,
            consensus_stage: self.finality.stage(),
            latest_checkpoint_height: latest_checkpoint.map(|checkpoint| checkpoint.block_height),
            latest_checkpoint_hash: latest_checkpoint
                .map(|checkpoint| checkpoint.block_hash.to_string()),
            quorum_required: self.chain.params().committee.quorum,
            configured_signers: self.chain.params().committee.members.len(),
            signer_availability: 0,
            pending_transition_height: self
                .finality
                .pending_transition()
                .map(|transition| transition.activates_at_height),
            mempool_transactions: self.mempool.len(),
            peer_count: self.peers.records().count(),
            peer_diversity_groups: self.peers.diversity_groups(),
            uptime_seconds: unix_time().saturating_sub(self.started_at),
            projected_transition_date: None,
            projection_warning: "Projected dates are estimates, not guarantees.".to_owned(),
        }
    }

    fn submit_block(&mut self, block: Block) -> Result<SubmitOutcome, NodeError> {
        let transactions = block.transactions.clone();
        let outcome = self.chain.submit_block(block, &RustCryptoProvider)?;
        if outcome.became_active {
            self.recent_reorg_depth = outcome.reorg_depth;
            self.mempool.remove_confirmed(&transactions);
            self.store.persist_active_chain(&self.chain)?;
        }
        Ok(outcome)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct StatusResponse {
    pub network: String,
    pub chain_id: String,
    pub height: u64,
    pub tip_hash: String,
    pub cumulative_chainwork: String,
    pub difficulty_bits: u32,
    pub recent_reorg_depth: u64,
    pub consensus_stage: ConsensusStage,
    pub latest_checkpoint_height: Option<u64>,
    pub latest_checkpoint_hash: Option<String>,
    pub quorum_required: u16,
    pub configured_signers: usize,
    pub signer_availability: usize,
    pub pending_transition_height: Option<u64>,
    pub mempool_transactions: usize,
    pub peer_count: usize,
    pub peer_diversity_groups: usize,
    pub uptime_seconds: u64,
    pub projected_transition_date: Option<String>,
    pub projection_warning: String,
}

#[derive(Debug, Deserialize)]
struct TemplateRequest {
    miner_public_keys: HybridPublicKeys,
    timestamp: Option<u64>,
    extra_nonce: Option<String>,
}

#[derive(Debug, Serialize)]
struct SubmitBlockResponse {
    hash: String,
    height: u64,
    chainwork: String,
    became_active: bool,
    reorg_depth: u64,
}

#[derive(Debug, Serialize)]
struct MempoolResponse {
    count: usize,
    txids: Vec<String>,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(error: impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: error.to_string(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(serde_json::json!({ "error": self.message })),
        )
            .into_response()
    }
}

#[must_use]
pub fn router(state: SharedNode) -> Router {
    Router::new()
        .route("/", get(dashboard))
        .route("/explorer", get(dashboard))
        .route("/openapi.json", get(openapi))
        .route("/metrics", get(metrics))
        .route("/v1/status", get(status))
        .route("/v1/blocks/{height}", get(block_at_height))
        .route("/v1/mempool", get(mempool))
        .route("/v1/transactions", post(submit_transaction))
        .route("/v1/checkpoints", post(submit_checkpoint))
        .route("/v1/mining/template", post(block_template))
        .route("/v1/blocks", post(submit_block))
        .layer(RequestBodyLimitLayer::new(9 * 1024 * 1024))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub async fn serve_http(state: SharedNode, address: SocketAddr) -> Result<(), NodeError> {
    let listener = TcpListener::bind(address).await?;
    axum::serve(listener, router(state)).await?;
    Ok(())
}

pub async fn serve_p2p(state: SharedNode, address: SocketAddr) -> Result<(), NodeError> {
    let listener = TcpListener::bind(address).await?;
    loop {
        let (stream, peer) = listener.accept().await?;
        let state = Arc::clone(&state);
        tokio::spawn(async move {
            if let Err(error) = handle_peer(state.clone(), stream, peer).await {
                let mut node = state.write().await;
                node.peers.penalize(peer, 25, unix_time());
                tracing::warn!(%peer, %error, "peer session ended");
            }
        });
    }
}

async fn handle_peer(
    state: SharedNode,
    mut stream: TcpStream,
    peer: SocketAddr,
) -> Result<(), NodeError> {
    let (magic, params, height) = {
        let node = state.read().await;
        (
            node.chain.params().message_magic,
            node.chain.params().clone(),
            node.chain.height(),
        )
    };
    let first = read_message(&mut stream, magic, Duration::from_secs(10)).await?;
    let PeerMessage::Handshake(handshake) = first else {
        return Err(P2pError::UnsupportedVersion.into());
    };
    handshake.validate(&params)?;
    let response = PeerMessage::Handshake(Handshake::new(
        &params,
        1,
        rand::random(),
        format!("qbitcoin/{}", crate::SOFTWARE_VERSION),
        height,
        unix_time(),
    ));
    write_message(&mut stream, magic, &response, Duration::from_secs(10)).await?;
    state.write().await.peers.observe(peer, unix_time());

    loop {
        let message = read_message(&mut stream, magic, Duration::from_secs(60)).await?;
        match message {
            PeerMessage::Transaction(transaction) => {
                let mut node = state.write().await;
                let NodeRuntime { chain, mempool, .. } = &mut *node;
                mempool
                    .admit(transaction, chain, &RustCryptoProvider)
                    .map_err(|_| P2pError::UnsupportedVersion)?;
            }
            PeerMessage::Block(block) => {
                state.write().await.submit_block(block)?;
            }
            PeerMessage::GetHeaders { .. } => {
                let headers: Vec<BlockHeader> = state
                    .read()
                    .await
                    .chain
                    .active_blocks()
                    .into_iter()
                    .rev()
                    .take(crate::p2p::MAX_HEADERS_PER_MESSAGE)
                    .map(|block| block.header)
                    .collect();
                write_message(
                    &mut stream,
                    magic,
                    &PeerMessage::Headers(headers),
                    Duration::from_secs(10),
                )
                .await?;
            }
            PeerMessage::Ping(nonce) => {
                write_message(
                    &mut stream,
                    magic,
                    &PeerMessage::Pong(nonce),
                    Duration::from_secs(10),
                )
                .await?;
            }
            PeerMessage::Handshake(_) | PeerMessage::Headers(_) | PeerMessage::Pong(_) => {}
        }
    }
}

async fn status(State(state): State<SharedNode>) -> Json<StatusResponse> {
    Json(state.read().await.status())
}

async fn block_at_height(
    State(state): State<SharedNode>,
    Path(height): Path<u64>,
) -> Result<Json<Block>, ApiError> {
    let node = state.read().await;
    let hash = node.chain.block_at_height(height).ok_or_else(|| ApiError {
        status: StatusCode::NOT_FOUND,
        message: "block height is not on the active chain".to_owned(),
    })?;
    Ok(Json(
        node.chain
            .block(hash)
            .expect("active hash has a block")
            .clone(),
    ))
}

async fn mempool(State(state): State<SharedNode>) -> Json<MempoolResponse> {
    let node = state.read().await;
    Json(MempoolResponse {
        count: node.mempool.len(),
        txids: node
            .mempool
            .entries()
            .map(|(txid, _)| txid.to_string())
            .collect(),
    })
}

async fn submit_transaction(
    State(state): State<SharedNode>,
    Json(transaction): Json<Transaction>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let mut node = state.write().await;
    let NodeRuntime { chain, mempool, .. } = &mut *node;
    let txid = mempool
        .admit(transaction, chain, &RustCryptoProvider)
        .map_err(ApiError::bad_request)?;
    Ok(Json(serde_json::json!({ "txid": txid.to_string() })))
}

async fn submit_checkpoint(
    State(state): State<SharedNode>,
    Json(certificate): Json<CheckpointCertificate>,
) -> Result<Json<CheckpointResult>, ApiError> {
    let mut node = state.write().await;
    let NodeRuntime {
        chain, finality, ..
    } = &mut *node;
    let result = finality
        .process_certificate(&certificate, chain)
        .map_err(ApiError::bad_request)?;
    Ok(Json(result))
}

async fn block_template(
    State(state): State<SharedNode>,
    Json(request): Json<TemplateRequest>,
) -> Result<Json<Block>, ApiError> {
    let node = state.read().await;
    let timestamp = request
        .timestamp
        .unwrap_or_else(unix_time)
        .max(node.chain.tip_timestamp().saturating_add(1));
    let transactions = node.mempool.transactions_by_fee_rate();
    let block = node
        .chain
        .block_template(
            OutputPolicy::HybridCommitment {
                commitment: crate::address::policy_commitment(&request.miner_public_keys),
            },
            transactions,
            timestamp,
            request.extra_nonce.as_deref().unwrap_or("rpc").as_bytes(),
            &RustCryptoProvider,
        )
        .map_err(ApiError::bad_request)?;
    Ok(Json(block))
}

async fn submit_block(
    State(state): State<SharedNode>,
    Json(block): Json<Block>,
) -> Result<Json<SubmitBlockResponse>, ApiError> {
    let outcome = state
        .write()
        .await
        .submit_block(block)
        .map_err(ApiError::bad_request)?;
    Ok(Json(SubmitBlockResponse {
        hash: outcome.hash.to_string(),
        height: outcome.height,
        chainwork: u256_hex(outcome.chainwork),
        became_active: outcome.became_active,
        reorg_depth: outcome.reorg_depth,
    }))
}

async fn metrics(State(state): State<SharedNode>) -> Response {
    let node = state.read().await;
    let stage = node.finality.stage() as u8;
    let checkpoint = node
        .finality
        .latest_checkpoint()
        .map_or(0, |checkpoint| checkpoint.block_height);
    let body = format!(
        "# HELP qbtc_chain_height Active-chain block height.\n\
# TYPE qbtc_chain_height gauge\n\
qbtc_chain_height {}\n\
# HELP qbtc_mempool_transactions Transactions awaiting mining.\n\
# TYPE qbtc_mempool_transactions gauge\n\
qbtc_mempool_transactions {}\n\
# HELP qbtc_consensus_stage 0 bootstrap, 1 reduced, 2 observer, 3 retired.\n\
# TYPE qbtc_consensus_stage gauge\n\
qbtc_consensus_stage {stage}\n\
# HELP qbtc_latest_checkpoint_height Latest accepted checkpoint or observation.\n\
# TYPE qbtc_latest_checkpoint_height gauge\n\
qbtc_latest_checkpoint_height {checkpoint}\n\
# HELP qbtc_peer_diversity_groups Approximate connected peer network groups.\n\
# TYPE qbtc_peer_diversity_groups gauge\n\
qbtc_peer_diversity_groups {}\n",
        node.chain.height(),
        node.mempool.len(),
        node.peers.diversity_groups(),
    );
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; version=0.0.4"),
        )],
        body,
    )
        .into_response()
}

async fn openapi() -> Response {
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        )],
        include_str!("../docs/openapi.json"),
    )
        .into_response()
}

async fn dashboard() -> Html<&'static str> {
    Html(include_str!("../contrib/dashboard/index.html"))
}

fn unix_time() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

fn u256_hex(value: U256) -> String {
    format!("0x{}", hex::encode(value.to_big_endian()))
}
