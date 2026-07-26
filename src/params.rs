use crate::amount::Amount;
use crate::hash::Hash32;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use thiserror::Error;

pub const GENESIS_MESSAGE: &str =
    "QBitCoin experimental testnet: no premine, no value, research only";

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ChainParamsError {
    #[error("mainnet is disabled; compile with ENABLE_UNAUDITED_MAINNET=1")]
    MainnetBuildDisabled,
    #[error("mainnet requires --acknowledge-unaudited-mainnet")]
    MainnetRuntimeAcknowledgementRequired,
    #[error("local committee mode is prohibited on mainnet")]
    LocalCommitteeOnMainnet,
    #[error("treasury allocation exceeds its declared subsidy fraction")]
    InvalidTreasuryAllocation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Network {
    Main,
    Testnet,
    Regtest,
}

impl Network {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Testnet => "testnet",
            Self::Regtest => "regtest",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreasuryParams {
    pub enabled: bool,
    pub subsidy_basis_points: u16,
    pub expires_at_height: u64,
    /// Commitment to a publicly reviewed quorum script, never a private key.
    pub quorum_script_hash: Hash32,
}

impl TreasuryParams {
    pub const DISABLED: Self = Self {
        enabled: false,
        subsidy_basis_points: 0,
        expires_at_height: 0,
        quorum_script_hash: Hash32::ZERO,
    };

    pub fn split_subsidy(
        &self,
        height: u64,
        subsidy: Amount,
    ) -> Result<(Amount, Amount), ChainParamsError> {
        if !self.enabled || height >= self.expires_at_height {
            return Ok((subsidy, Amount::ZERO));
        }
        if self.subsidy_basis_points > 10_000 || self.quorum_script_hash == Hash32::ZERO {
            return Err(ChainParamsError::InvalidTreasuryAllocation);
        }

        let treasury_atoms = subsidy
            .atoms()
            .checked_mul(u64::from(self.subsidy_basis_points))
            .ok_or(ChainParamsError::InvalidTreasuryAllocation)?
            / 10_000;
        let treasury = Amount::from_atoms(treasury_atoms)
            .map_err(|_| ChainParamsError::InvalidTreasuryAllocation)?;
        let miner = subsidy
            .checked_sub(treasury)
            .map_err(|_| ChainParamsError::InvalidTreasuryAllocation)?;
        Ok((miner, treasury))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChainParams {
    pub network: Network,
    pub chain_id: Hash32,
    pub message_magic: [u8; 4],
    pub p2p_port: u16,
    pub rpc_port: u16,
    pub bech32m_hrp: String,
    pub target_block_interval_secs: u64,
    pub difficulty_adjustment_interval: u64,
    pub initial_pow_bits: u32,
    pub coinbase_maturity: u64,
    pub max_block_weight: u64,
    pub max_transaction_weight: u64,
    pub max_signature_verification_cost: u64,
    pub genesis_message: String,
    pub genesis_timestamp: u64,
    pub genesis_allocation: Amount,
    pub treasury: TreasuryParams,
    pub static_seeds: Vec<SocketAddr>,
}

impl ChainParams {
    #[must_use]
    pub fn for_network(network: Network) -> Self {
        let (
            magic,
            p2p_port,
            rpc_port,
            hrp,
            block_secs,
            adjustment_interval,
            genesis_timestamp,
            initial_pow_bits,
        ) = match network {
            Network::Main => (
                [0xd1, 0x6f, 0x31, 0xa8],
                38_333,
                38_332,
                "qbtc",
                600,
                2_016,
                1_800_000_000,
                0x1f00_ffff,
            ),
            Network::Testnet => (
                [0x6c, 0xf2, 0x89, 0x51],
                48_333,
                48_332,
                "tqbtc",
                60,
                144,
                1_800_000_100,
                0x2000_ffff,
            ),
            Network::Regtest => (
                [0xfa, 0x71, 0xb4, 0xc2],
                58_333,
                58_332,
                "rqbtc",
                1,
                20,
                1_800_000_200,
                0x207f_ffff,
            ),
        };
        let chain_id = Hash32::tagged_sha3(
            "QBTC/CHAIN-ID/v1",
            format!("{}:{genesis_timestamp}:{GENESIS_MESSAGE}", network.name()).as_bytes(),
        );

        Self {
            network,
            chain_id,
            message_magic: magic,
            p2p_port,
            rpc_port,
            bech32m_hrp: hrp.to_owned(),
            target_block_interval_secs: block_secs,
            difficulty_adjustment_interval: adjustment_interval,
            initial_pow_bits,
            coinbase_maturity: 100,
            // Testnet research defaults. They are intentionally not copied from Bitcoin.
            max_block_weight: 8_000_000,
            max_transaction_weight: 1_000_000,
            max_signature_verification_cost: 2_000,
            genesis_message: GENESIS_MESSAGE.to_owned(),
            genesis_timestamp,
            genesis_allocation: Amount::ZERO,
            treasury: TreasuryParams::DISABLED,
            static_seeds: vec![SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), p2p_port)],
        }
    }

    pub fn enforce_startup(
        &self,
        acknowledge_unaudited_mainnet: bool,
        local_committee: bool,
    ) -> Result<(), ChainParamsError> {
        if self.network != Network::Main {
            return Ok(());
        }
        if !crate::unaudited_mainnet_build_enabled() {
            return Err(ChainParamsError::MainnetBuildDisabled);
        }
        if !acknowledge_unaudited_mainnet {
            return Err(ChainParamsError::MainnetRuntimeAcknowledgementRequired);
        }
        if local_committee {
            return Err(ChainParamsError::LocalCommitteeOnMainnet);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::amount::{ATOMS_PER_QBTC, block_subsidy};

    #[test]
    fn networks_are_replay_separated() {
        let main = ChainParams::for_network(Network::Main);
        let test = ChainParams::for_network(Network::Testnet);
        let regtest = ChainParams::for_network(Network::Regtest);
        assert_ne!(main.chain_id, test.chain_id);
        assert_ne!(test.chain_id, regtest.chain_id);
        assert_ne!(main.message_magic, test.message_magic);
        assert_ne!(main.bech32m_hrp, test.bech32m_hrp);
    }

    #[test]
    fn every_default_treasury_is_disabled() {
        for network in [Network::Main, Network::Testnet, Network::Regtest] {
            assert!(!ChainParams::for_network(network).treasury.enabled);
        }
    }

    #[test]
    fn treasury_is_subsidy_only_and_expires() {
        let treasury = TreasuryParams {
            enabled: true,
            subsidy_basis_points: 1_000,
            expires_at_height: 10,
            quorum_script_hash: Hash32([7; 32]),
        };
        let subsidy = block_subsidy(1);
        let (miner, development) = treasury.split_subsidy(1, subsidy).unwrap();
        assert_eq!(development.atoms(), 5 * ATOMS_PER_QBTC);
        assert_eq!(
            miner.checked_add(development).unwrap().atoms(),
            subsidy.atoms()
        );
        assert_eq!(
            treasury.split_subsidy(10, subsidy).unwrap(),
            (subsidy, Amount::ZERO)
        );
    }

    #[test]
    fn normal_build_refuses_mainnet() {
        let main = ChainParams::for_network(Network::Main);
        assert_eq!(
            main.enforce_startup(true, false),
            Err(ChainParamsError::MainnetBuildDisabled)
        );
    }

    #[test]
    fn genesis_has_no_allocation() {
        for network in [Network::Main, Network::Testnet, Network::Regtest] {
            assert_eq!(
                ChainParams::for_network(network).genesis_allocation,
                Amount::ZERO
            );
        }
    }
}
