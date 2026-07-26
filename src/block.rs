use crate::{
    amount::Amount,
    encoding::put_compact,
    hash::Hash32,
    params::ChainParams,
    pow::{PowError, PowProvider, verify_pow},
    transaction::{OutputPolicy, Transaction, TxOutput},
    utxo::{UtxoSet, ValidationError},
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const CONSENSUS_VERSION: u16 = 1;
pub const HEADER_ENCODED_SIZE: usize = 116;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum BlockError {
    #[error(transparent)]
    Pow(#[from] PowError),
    #[error(transparent)]
    Validation(#[from] ValidationError),
    #[error("block belongs to another network")]
    WrongNetwork,
    #[error("unsupported consensus or proof-of-work version")]
    UnsupportedVersion,
    #[error("previous block commitment is incorrect")]
    WrongPreviousBlock,
    #[error("block difficulty target is incorrect")]
    WrongDifficulty,
    #[error("block timestamp is not greater than its parent")]
    InvalidTimestamp,
    #[error("transaction Merkle commitment is invalid")]
    InvalidMerkleRoot,
    #[error("block must contain exactly one first-position coinbase")]
    InvalidCoinbasePlacement,
    #[error("block exceeds its configured weight limit")]
    BlockWeight,
    #[error("block signature verification cost exceeds its configured limit")]
    BlockSignatureCost,
    #[error("nonce search exhausted without finding valid proof of work")]
    MiningExhausted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BlockHeader {
    pub consensus_version: u16,
    pub pow_version: u16,
    pub chain_id: Hash32,
    pub previous_block: Hash32,
    pub transaction_merkle_root: Hash32,
    pub timestamp: u64,
    pub bits: u32,
    pub nonce: u32,
}

impl BlockHeader {
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(HEADER_ENCODED_SIZE);
        bytes.extend_from_slice(&self.consensus_version.to_le_bytes());
        bytes.extend_from_slice(&self.pow_version.to_le_bytes());
        bytes.extend_from_slice(&self.chain_id.0);
        bytes.extend_from_slice(&self.previous_block.0);
        bytes.extend_from_slice(&self.transaction_merkle_root.0);
        bytes.extend_from_slice(&self.timestamp.to_le_bytes());
        bytes.extend_from_slice(&self.bits.to_le_bytes());
        bytes.extend_from_slice(&self.nonce.to_le_bytes());
        bytes
    }

    #[must_use]
    pub fn hash(&self, provider: &dyn PowProvider) -> Hash32 {
        provider.hash(&self.encode())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Block {
    pub header: BlockHeader,
    pub transactions: Vec<Transaction>,
}

impl Block {
    #[must_use]
    pub fn candidate(
        params: &ChainParams,
        provider: &dyn PowProvider,
        previous_block: Hash32,
        timestamp: u64,
        bits: u32,
        transactions: Vec<Transaction>,
    ) -> Self {
        let merkle_root = transaction_merkle_root(&transactions);
        Self {
            header: BlockHeader {
                consensus_version: CONSENSUS_VERSION,
                pow_version: provider.version(),
                chain_id: params.chain_id,
                previous_block,
                transaction_merkle_root: merkle_root,
                timestamp,
                bits,
                nonce: 0,
            },
            transactions,
        }
    }

    #[must_use]
    pub fn hash(&self, provider: &dyn PowProvider) -> Hash32 {
        self.header.hash(provider)
    }

    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut bytes = self.header.encode();
        put_compact(self.transactions.len() as u64, &mut bytes);
        for transaction in &self.transactions {
            let encoded = transaction.encode(true);
            put_compact(encoded.len() as u64, &mut bytes);
            bytes.extend_from_slice(&encoded);
        }
        bytes
    }

    #[must_use]
    pub fn weight(&self) -> u64 {
        (HEADER_ENCODED_SIZE as u64)
            .saturating_mul(4)
            .saturating_add(
                self.transactions
                    .iter()
                    .map(Transaction::weight)
                    .sum::<u64>(),
            )
    }

    pub fn mine(
        &mut self,
        provider: &dyn PowProvider,
        maximum_nonce: u32,
    ) -> Result<Hash32, BlockError> {
        loop {
            let hash = self.hash(provider);
            if verify_pow(hash, self.header.bits).is_ok() {
                return Ok(hash);
            }
            if self.header.nonce == maximum_nonce {
                return Err(BlockError::MiningExhausted);
            }
            self.header.nonce = self.header.nonce.wrapping_add(1);
        }
    }

    pub fn validate_and_apply(
        &self,
        height: u64,
        expected_previous: Hash32,
        parent_timestamp: Option<u64>,
        expected_bits: u32,
        params: &ChainParams,
        pow_provider: &dyn PowProvider,
        signature_provider: &dyn crate::crypto::SignatureProvider,
        parent_utxos: &UtxoSet,
    ) -> Result<UtxoSet, BlockError> {
        if self.header.chain_id != params.chain_id {
            return Err(BlockError::WrongNetwork);
        }
        if self.header.consensus_version != CONSENSUS_VERSION
            || self.header.pow_version != pow_provider.version()
        {
            return Err(BlockError::UnsupportedVersion);
        }
        if self.header.previous_block != expected_previous {
            return Err(BlockError::WrongPreviousBlock);
        }
        if self.header.bits != expected_bits {
            return Err(BlockError::WrongDifficulty);
        }
        if parent_timestamp.is_some_and(|timestamp| self.header.timestamp <= timestamp) {
            return Err(BlockError::InvalidTimestamp);
        }
        if self.header.transaction_merkle_root != transaction_merkle_root(&self.transactions) {
            return Err(BlockError::InvalidMerkleRoot);
        }
        verify_pow(self.hash(pow_provider), self.header.bits)?;
        if self.weight() > params.max_block_weight {
            return Err(BlockError::BlockWeight);
        }
        let signature_cost: u64 = self
            .transactions
            .iter()
            .map(Transaction::signature_verification_cost)
            .sum();
        if signature_cost > params.max_signature_verification_cost {
            return Err(BlockError::BlockSignatureCost);
        }
        if self.transactions.first().is_none_or(|tx| !tx.is_coinbase())
            || self
                .transactions
                .iter()
                .skip(1)
                .any(Transaction::is_coinbase)
        {
            return Err(BlockError::InvalidCoinbasePlacement);
        }

        let mut next_utxos = parent_utxos.clone();
        let mut fees = Amount::ZERO;
        for transaction in self.transactions.iter().skip(1) {
            let fee = next_utxos.apply_regular_transaction(
                transaction,
                height,
                params,
                signature_provider,
            )?;
            fees = fees.checked_add(fee).map_err(ValidationError::from)?;
        }
        next_utxos.apply_coinbase(&self.transactions[0], height, fees, params)?;
        Ok(next_utxos)
    }
}

#[must_use]
pub fn transaction_merkle_root(transactions: &[Transaction]) -> Hash32 {
    if transactions.is_empty() {
        return Hash32::ZERO;
    }
    let mut layer: Vec<Hash32> = transactions.iter().map(Transaction::txid).collect();
    while layer.len() > 1 {
        if layer.len() % 2 == 1 {
            layer.push(*layer.last().expect("non-empty Merkle layer"));
        }
        layer = layer
            .chunks_exact(2)
            .map(|pair| {
                let mut bytes = [0_u8; 64];
                bytes[..32].copy_from_slice(&pair[0].0);
                bytes[32..].copy_from_slice(&pair[1].0);
                Hash32::sha256d(&bytes)
            })
            .collect();
    }
    layer[0]
}

pub fn genesis_block(
    params: &ChainParams,
    provider: &dyn PowProvider,
) -> Result<Block, BlockError> {
    let commitment = Hash32::tagged_sha3(
        "QBTC/GENESIS/v1",
        format!(
            "{}:{}:{}",
            params.network.name(),
            params.genesis_timestamp,
            params.genesis_message
        )
        .as_bytes(),
    );
    let coinbase = Transaction::coinbase(
        params.chain_id,
        0,
        vec![TxOutput {
            value: params.genesis_allocation,
            policy: OutputPolicy::Burn { commitment },
        }],
        params.genesis_message.as_bytes(),
    );
    let mut block = Block::candidate(
        params,
        provider,
        Hash32::ZERO,
        params.genesis_timestamp,
        params.initial_pow_bits,
        vec![coinbase],
    );
    block.mine(provider, u32::MAX)?;
    Ok(block)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        crypto::{HybridKeyPair, RustCryptoProvider},
        params::{ChainParams, Network},
        pow::Sha256dPowV1,
    };

    #[test]
    fn genesis_is_valid_and_has_no_spendable_allocation() {
        let params = ChainParams::for_network(Network::Regtest);
        let pow = Sha256dPowV1;
        let genesis = genesis_block(&params, &pow).unwrap();
        let utxos = genesis
            .validate_and_apply(
                0,
                Hash32::ZERO,
                None,
                params.initial_pow_bits,
                &params,
                &pow,
                &RustCryptoProvider,
                &UtxoSet::default(),
            )
            .unwrap();
        assert!(utxos.is_empty());
        assert_eq!(utxos.total_issued(), Amount::ZERO);
    }

    #[test]
    fn merkle_root_changes_with_transaction_order() {
        let params = ChainParams::for_network(Network::Regtest);
        let keys = HybridKeyPair::generate(None).public_keys();
        let first = Transaction::coinbase(
            params.chain_id,
            1,
            vec![TxOutput {
                value: Amount::ZERO,
                policy: OutputPolicy::Hybrid(keys.clone()),
            }],
            b"a",
        );
        let second = Transaction::coinbase(
            params.chain_id,
            2,
            vec![TxOutput {
                value: Amount::ZERO,
                policy: OutputPolicy::Hybrid(keys),
            }],
            b"b",
        );
        assert_ne!(
            transaction_merkle_root(&[first.clone(), second.clone()]),
            transaction_merkle_root(&[second, first])
        );
    }
}
