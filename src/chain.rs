use crate::{
    amount::{Amount, block_subsidy},
    block::{Block, BlockError, genesis_block},
    crypto::{RustCryptoProvider, SignatureProvider},
    hash::Hash32,
    params::ChainParams,
    pow::{PowError, Sha256dPowV1, adjusted_bits, work_for_bits},
    transaction::{OutputPolicy, Transaction, TxOutput},
    utxo::UtxoSet,
};
use primitive_types::U256;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ChainError {
    #[error(transparent)]
    Block(#[from] BlockError),
    #[error(transparent)]
    Pow(#[from] PowError),
    #[error(transparent)]
    Validation(#[from] crate::utxo::ValidationError),
    #[error("parent block is unknown")]
    UnknownParent,
    #[error("block is already known")]
    AlreadyKnown,
    #[error("chainwork arithmetic overflow")]
    ChainworkOverflow,
    #[error("block height arithmetic overflow")]
    HeightOverflow,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubmitOutcome {
    pub hash: Hash32,
    pub height: u64,
    pub chainwork: U256,
    pub became_active: bool,
    pub reorg_depth: u64,
}

#[derive(Debug, Clone)]
struct IndexedBlock {
    block: Block,
    height: u64,
    chainwork: U256,
    utxos: UtxoSet,
}

pub struct ChainState {
    params: ChainParams,
    blocks: BTreeMap<Hash32, IndexedBlock>,
    active_tip: Hash32,
}

impl ChainState {
    pub fn new(params: ChainParams) -> Result<Self, ChainError> {
        let pow = Sha256dPowV1;
        let genesis = genesis_block(&params, &pow)?;
        let hash = genesis.hash(&pow);
        let utxos = genesis.validate_and_apply(
            0,
            Hash32::ZERO,
            None,
            params.initial_pow_bits,
            &params,
            &pow,
            &RustCryptoProvider,
            &UtxoSet::default(),
        )?;
        let chainwork = work_for_bits(genesis.header.bits)?;
        let mut blocks = BTreeMap::new();
        blocks.insert(
            hash,
            IndexedBlock {
                block: genesis,
                height: 0,
                chainwork,
                utxos,
            },
        );
        Ok(Self {
            params,
            blocks,
            active_tip: hash,
        })
    }

    #[must_use]
    pub fn params(&self) -> &ChainParams {
        &self.params
    }

    #[must_use]
    pub fn tip_hash(&self) -> Hash32 {
        self.active_tip
    }

    #[must_use]
    pub fn height(&self) -> u64 {
        self.blocks[&self.active_tip].height
    }

    #[must_use]
    pub fn chainwork(&self) -> U256 {
        self.blocks[&self.active_tip].chainwork
    }

    #[must_use]
    pub fn total_issued(&self) -> Amount {
        self.blocks[&self.active_tip].utxos.total_issued()
    }

    #[must_use]
    pub fn active_utxos(&self) -> &UtxoSet {
        &self.blocks[&self.active_tip].utxos
    }

    pub fn expected_bits_after(&self, parent_hash: Hash32) -> Result<u32, ChainError> {
        let parent = self
            .blocks
            .get(&parent_hash)
            .ok_or(ChainError::UnknownParent)?;
        let next_height = parent
            .height
            .checked_add(1)
            .ok_or(ChainError::HeightOverflow)?;
        let interval = self.params.difficulty_adjustment_interval;
        if interval == 0 || next_height % interval != 0 {
            return Ok(parent.block.header.bits);
        }

        let start_height = next_height.saturating_sub(interval);
        let start_hash = self
            .ancestor_at_height(parent_hash, start_height)
            .ok_or(ChainError::UnknownParent)?;
        let start = &self.blocks[&start_hash];
        let actual_timespan = parent
            .block
            .header
            .timestamp
            .saturating_sub(start.block.header.timestamp)
            .max(1);
        let expected_timespan = self
            .params
            .target_block_interval_secs
            .saturating_mul(interval)
            .max(1);
        Ok(adjusted_bits(
            parent.block.header.bits,
            actual_timespan,
            expected_timespan,
            self.params.initial_pow_bits,
        )?)
    }

    pub fn block_template(
        &self,
        miner_policy: OutputPolicy,
        regular_transactions: Vec<Transaction>,
        timestamp: u64,
        extra_nonce: &[u8],
        signature_provider: &dyn SignatureProvider,
    ) -> Result<Block, ChainError> {
        self.block_template_on(
            self.active_tip,
            miner_policy,
            regular_transactions,
            timestamp,
            extra_nonce,
            signature_provider,
        )
    }

    pub fn submit_block(
        &mut self,
        block: Block,
        signature_provider: &dyn SignatureProvider,
    ) -> Result<SubmitOutcome, ChainError> {
        let pow = Sha256dPowV1;
        let hash = block.hash(&pow);
        if self.blocks.contains_key(&hash) {
            return Err(ChainError::AlreadyKnown);
        }
        let parent_hash = block.header.previous_block;
        let parent = self
            .blocks
            .get(&parent_hash)
            .ok_or(ChainError::UnknownParent)?;
        let height = parent
            .height
            .checked_add(1)
            .ok_or(ChainError::HeightOverflow)?;
        let expected_bits = self.expected_bits_after(parent_hash)?;
        let next_utxos = block.validate_and_apply(
            height,
            parent_hash,
            Some(parent.block.header.timestamp),
            expected_bits,
            &self.params,
            &pow,
            signature_provider,
            &parent.utxos,
        )?;
        let chainwork = parent
            .chainwork
            .checked_add(work_for_bits(block.header.bits)?)
            .ok_or(ChainError::ChainworkOverflow)?;

        let old_tip = self.active_tip;
        self.blocks.insert(
            hash,
            IndexedBlock {
                block,
                height,
                chainwork,
                utxos: next_utxos,
            },
        );
        let became_active = chainwork > self.blocks[&old_tip].chainwork;
        let reorg_depth = if became_active {
            let depth = self.reorg_depth(old_tip, hash);
            self.active_tip = hash;
            depth
        } else {
            0
        };
        Ok(SubmitOutcome {
            hash,
            height,
            chainwork,
            became_active,
            reorg_depth,
        })
    }

    #[must_use]
    pub fn active_chain_contains(&self, hash: Hash32) -> bool {
        let mut cursor = Some(self.active_tip);
        while let Some(current) = cursor {
            if current == hash {
                return true;
            }
            cursor = self.parent_hash(current);
        }
        false
    }

    #[must_use]
    pub fn block_at_height(&self, height: u64) -> Option<Hash32> {
        self.ancestor_at_height(self.active_tip, height)
    }

    fn block_template_on(
        &self,
        parent_hash: Hash32,
        miner_policy: OutputPolicy,
        regular_transactions: Vec<Transaction>,
        timestamp: u64,
        extra_nonce: &[u8],
        signature_provider: &dyn SignatureProvider,
    ) -> Result<Block, ChainError> {
        let parent = self
            .blocks
            .get(&parent_hash)
            .ok_or(ChainError::UnknownParent)?;
        let height = parent
            .height
            .checked_add(1)
            .ok_or(ChainError::HeightOverflow)?;
        let mut temporary = parent.utxos.clone();
        let mut fees = Amount::ZERO;
        for transaction in &regular_transactions {
            let fee = temporary.apply_regular_transaction(
                transaction,
                height,
                &self.params,
                signature_provider,
            )?;
            fees = fees
                .checked_add(fee)
                .map_err(crate::utxo::ValidationError::from)?;
        }
        let reward = block_subsidy(height)
            .checked_add(fees)
            .map_err(crate::utxo::ValidationError::from)?;
        let coinbase = Transaction::coinbase(
            self.params.chain_id,
            height,
            vec![TxOutput {
                value: reward,
                policy: miner_policy,
            }],
            extra_nonce,
        );
        let mut transactions = Vec::with_capacity(regular_transactions.len() + 1);
        transactions.push(coinbase);
        transactions.extend(regular_transactions);
        Ok(Block::candidate(
            &self.params,
            &Sha256dPowV1,
            parent_hash,
            timestamp,
            self.expected_bits_after(parent_hash)?,
            transactions,
        ))
    }

    fn parent_hash(&self, hash: Hash32) -> Option<Hash32> {
        let block = self.blocks.get(&hash)?;
        (block.height > 0).then_some(block.block.header.previous_block)
    }

    fn ancestor_at_height(&self, hash: Hash32, height: u64) -> Option<Hash32> {
        let mut cursor = hash;
        loop {
            let indexed = self.blocks.get(&cursor)?;
            if indexed.height == height {
                return Some(cursor);
            }
            if indexed.height < height || indexed.height == 0 {
                return None;
            }
            cursor = indexed.block.header.previous_block;
        }
    }

    fn reorg_depth(&self, old_tip: Hash32, new_tip: Hash32) -> u64 {
        let mut old_ancestors = BTreeSet::new();
        let mut cursor = Some(old_tip);
        while let Some(hash) = cursor {
            old_ancestors.insert(hash);
            cursor = self.parent_hash(hash);
        }
        let mut new_cursor = Some(new_tip);
        while let Some(hash) = new_cursor {
            if old_ancestors.contains(&hash) {
                return self.blocks[&old_tip]
                    .height
                    .saturating_sub(self.blocks[&hash].height);
            }
            new_cursor = self.parent_hash(hash);
        }
        self.blocks[&old_tip].height
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        crypto::{HybridKeyPair, RustCryptoProvider},
        params::Network,
    };

    fn mine(mut block: Block) -> Block {
        block.mine(&Sha256dPowV1, u32::MAX).unwrap();
        block
    }

    #[test]
    fn greater_cumulative_work_selects_a_competing_chain() {
        let params = ChainParams::for_network(Network::Regtest);
        let mut chain = ChainState::new(params.clone()).unwrap();
        let policy = OutputPolicy::Hybrid(HybridKeyPair::generate(None).public_keys());
        let genesis = chain.tip_hash();
        let first = mine(
            chain
                .block_template(
                    policy.clone(),
                    Vec::new(),
                    params.genesis_timestamp + 1,
                    b"main-1",
                    &RustCryptoProvider,
                )
                .unwrap(),
        );
        let first_hash = first.hash(&Sha256dPowV1);
        chain.submit_block(first, &RustCryptoProvider).unwrap();

        let alternative_one = mine(
            chain
                .block_template_on(
                    genesis,
                    policy.clone(),
                    Vec::new(),
                    params.genesis_timestamp + 2,
                    b"alt-1",
                    &RustCryptoProvider,
                )
                .unwrap(),
        );
        let alternative_one_hash = alternative_one.hash(&Sha256dPowV1);
        let outcome = chain
            .submit_block(alternative_one, &RustCryptoProvider)
            .unwrap();
        assert!(!outcome.became_active);
        assert_eq!(chain.tip_hash(), first_hash);

        let alternative_two = mine(
            chain
                .block_template_on(
                    alternative_one_hash,
                    policy,
                    Vec::new(),
                    params.genesis_timestamp + 3,
                    b"alt-2",
                    &RustCryptoProvider,
                )
                .unwrap(),
        );
        let alternative_two_hash = alternative_two.hash(&Sha256dPowV1);
        let outcome = chain
            .submit_block(alternative_two, &RustCryptoProvider)
            .unwrap();
        assert!(outcome.became_active);
        assert_eq!(outcome.reorg_depth, 1);
        assert_eq!(chain.tip_hash(), alternative_two_hash);
    }

    #[test]
    fn work_cannot_make_an_inflation_block_valid() {
        let params = ChainParams::for_network(Network::Regtest);
        let mut chain = ChainState::new(params.clone()).unwrap();
        let policy = OutputPolicy::Hybrid(HybridKeyPair::generate(None).public_keys());
        let height = 1;
        let invalid_reward = Amount::from_atoms(block_subsidy(height).atoms() + 1).unwrap();
        let coinbase = Transaction::coinbase(
            params.chain_id,
            height,
            vec![TxOutput {
                value: invalid_reward,
                policy,
            }],
            b"inflation",
        );
        let mut block = Block::candidate(
            &params,
            &Sha256dPowV1,
            chain.tip_hash(),
            params.genesis_timestamp + 1,
            params.initial_pow_bits,
            vec![coinbase],
        );
        block.mine(&Sha256dPowV1, u32::MAX).unwrap();
        assert!(matches!(
            chain.submit_block(block, &RustCryptoProvider),
            Err(ChainError::Block(BlockError::Validation(
                crate::utxo::ValidationError::InvalidCoinbaseReward
            )))
        ));
        assert_eq!(chain.height(), 0);
    }
}
