use crate::{
    amount::Amount,
    chain::ChainState,
    crypto::SignatureProvider,
    hash::Hash32,
    transaction::{OutPoint, Transaction},
    utxo::ValidationError,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MempoolError {
    #[error(transparent)]
    Validation(#[from] ValidationError),
    #[error("transaction is already in the mempool")]
    Duplicate,
    #[error("mempool conflict; replace-by-fee is disabled")]
    Conflict,
    #[error("mempool capacity has been reached")]
    Capacity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MempoolEntry {
    pub transaction: Transaction,
    pub fee: Amount,
    pub admitted_at_height: u64,
}

#[derive(Debug, Clone)]
pub struct Mempool {
    entries: BTreeMap<Hash32, MempoolEntry>,
    spent_outpoints: BTreeMap<OutPoint, Hash32>,
    maximum_transactions: usize,
}

impl Default for Mempool {
    fn default() -> Self {
        Self::new(10_000)
    }
}

impl Mempool {
    #[must_use]
    pub fn new(maximum_transactions: usize) -> Self {
        Self {
            entries: BTreeMap::new(),
            spent_outpoints: BTreeMap::new(),
            maximum_transactions,
        }
    }

    pub fn admit(
        &mut self,
        transaction: Transaction,
        chain: &ChainState,
        provider: &dyn SignatureProvider,
    ) -> Result<Hash32, MempoolError> {
        if self.entries.len() >= self.maximum_transactions {
            return Err(MempoolError::Capacity);
        }
        let txid = transaction.txid();
        if self.entries.contains_key(&txid) {
            return Err(MempoolError::Duplicate);
        }
        if transaction
            .inputs
            .iter()
            .any(|input| self.spent_outpoints.contains_key(&input.previous_output))
        {
            return Err(MempoolError::Conflict);
        }
        let fee = chain.active_utxos().validate_regular_transaction(
            &transaction,
            chain.height().saturating_add(1),
            chain.params(),
            provider,
        )?;
        for input in &transaction.inputs {
            self.spent_outpoints.insert(input.previous_output, txid);
        }
        self.entries.insert(
            txid,
            MempoolEntry {
                transaction,
                fee,
                admitted_at_height: chain.height(),
            },
        );
        Ok(txid)
    }

    #[must_use]
    pub fn transactions_by_fee_rate(&self) -> Vec<Transaction> {
        let mut entries: Vec<&MempoolEntry> = self.entries.values().collect();
        entries.sort_by(|left, right| {
            let left_rate = u128::from(left.fee.atoms()) * u128::from(right.transaction.weight());
            let right_rate = u128::from(right.fee.atoms()) * u128::from(left.transaction.weight());
            right_rate.cmp(&left_rate)
        });
        entries
            .into_iter()
            .map(|entry| entry.transaction.clone())
            .collect()
    }

    pub fn remove_confirmed(&mut self, transactions: &[Transaction]) {
        for transaction in transactions {
            if let Some(entry) = self.entries.remove(&transaction.txid()) {
                for input in entry.transaction.inputs {
                    self.spent_outpoints.remove(&input.previous_output);
                }
            }
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    #[must_use]
    pub fn entries(&self) -> impl Iterator<Item = (&Hash32, &MempoolEntry)> {
        self.entries.iter()
    }
}
