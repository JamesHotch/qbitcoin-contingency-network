use crate::{
    amount::{Amount, AmountError, block_subsidy, valid_subsidy_total_at_height},
    crypto::{CryptoError, DomainTag, SignatureProvider, verify_hybrid, verify_recovery},
    params::ChainParams,
    transaction::{OutPoint, OutputPolicy, SpendWitness, Transaction, TransactionError, TxOutput},
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ValidationError {
    #[error(transparent)]
    Transaction(#[from] TransactionError),
    #[error(transparent)]
    Amount(#[from] AmountError),
    #[error(transparent)]
    Crypto(#[from] CryptoError),
    #[error("transaction belongs to another network")]
    WrongNetwork,
    #[error("coinbase transaction appeared where a regular transaction was required")]
    UnexpectedCoinbase,
    #[error("referenced UTXO does not exist or was already spent")]
    MissingInput,
    #[error("coinbase output has not reached maturity")]
    ImmatureCoinbase,
    #[error("absolute or relative timelock has not matured")]
    Timelock,
    #[error("output policy is provably unspendable")]
    BurnOutput,
    #[error("witness does not satisfy the referenced output policy")]
    WrongWitness,
    #[error("transaction outputs exceed its inputs")]
    InputsBelowOutputs,
    #[error("transaction exceeds the configured weight limit")]
    TransactionWeight,
    #[error("signature verification cost exceeds the configured limit")]
    SignatureCost,
    #[error("coinbase height does not match its block")]
    CoinbaseHeight,
    #[error("coinbase reward exceeds subsidy plus fees")]
    InvalidCoinbaseReward,
    #[error("total issued supply exceeds the valid subsidy schedule")]
    SupplyExceeded,
    #[error("transaction ID collides with an existing unspent output")]
    TxidCollision,
    #[error("enabled treasury parameters require a reviewed quorum script implementation")]
    TreasuryNotReady,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UtxoEntry {
    pub output: TxOutput,
    pub created_height: u64,
    pub coinbase: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UtxoSet {
    entries: BTreeMap<OutPoint, UtxoEntry>,
    total_issued: Amount,
}

impl UtxoSet {
    #[must_use]
    pub fn get(&self, outpoint: &OutPoint) -> Option<&UtxoEntry> {
        self.entries.get(outpoint)
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
    pub const fn total_issued(&self) -> Amount {
        self.total_issued
    }

    pub fn validate_regular_transaction(
        &self,
        transaction: &Transaction,
        height: u64,
        params: &ChainParams,
        provider: &dyn SignatureProvider,
    ) -> Result<Amount, ValidationError> {
        transaction.validate_structure()?;
        if transaction.is_coinbase() {
            return Err(ValidationError::UnexpectedCoinbase);
        }
        if transaction.chain_id != params.chain_id {
            return Err(ValidationError::WrongNetwork);
        }
        if transaction.lock_time > height {
            return Err(ValidationError::Timelock);
        }
        if transaction.weight() > params.max_transaction_weight {
            return Err(ValidationError::TransactionWeight);
        }
        if transaction.signature_verification_cost() > params.max_signature_verification_cost {
            return Err(ValidationError::SignatureCost);
        }

        let mut input_total = Amount::ZERO;
        for (input_index, input) in transaction.inputs.iter().enumerate() {
            let entry = self
                .entries
                .get(&input.previous_output)
                .ok_or(ValidationError::MissingInput)?;
            let age = height
                .checked_sub(entry.created_height)
                .ok_or(ValidationError::Timelock)?;
            if entry.coinbase && age < params.coinbase_maturity {
                return Err(ValidationError::ImmatureCoinbase);
            }
            if input.sequence != u32::MAX && age < u64::from(input.sequence) {
                return Err(ValidationError::Timelock);
            }

            let message = transaction.signing_message(input_index, &entry.output);
            match (&entry.output.policy, &input.witness) {
                (OutputPolicy::Hybrid(keys), SpendWitness::Hybrid(signature)) => {
                    verify_hybrid(
                        provider,
                        &params.chain_id,
                        DomainTag::TransactionSignature,
                        &message,
                        keys,
                        signature,
                    )?;
                }
                (OutputPolicy::Hybrid(keys), SpendWitness::Recovery(signature)) => {
                    let recovery = keys
                        .recovery
                        .as_ref()
                        .ok_or(ValidationError::WrongWitness)?;
                    verify_recovery(
                        provider,
                        &params.chain_id,
                        &message,
                        recovery,
                        signature,
                        age,
                    )?;
                }
                (OutputPolicy::Burn { .. }, _) => return Err(ValidationError::BurnOutput),
                _ => return Err(ValidationError::WrongWitness),
            }
            input_total = input_total.checked_add(entry.output.value)?;
        }

        let output_total = sum_outputs(&transaction.outputs)?;
        input_total
            .checked_sub(output_total)
            .map_err(|_| ValidationError::InputsBelowOutputs)
    }

    pub fn apply_regular_transaction(
        &mut self,
        transaction: &Transaction,
        height: u64,
        params: &ChainParams,
        provider: &dyn SignatureProvider,
    ) -> Result<Amount, ValidationError> {
        let fee = self.validate_regular_transaction(transaction, height, params, provider)?;
        for input in &transaction.inputs {
            self.entries
                .remove(&input.previous_output)
                .ok_or(ValidationError::MissingInput)?;
        }
        self.insert_outputs(transaction, height, false)?;
        Ok(fee)
    }

    pub fn apply_coinbase(
        &mut self,
        transaction: &Transaction,
        height: u64,
        fees: Amount,
        params: &ChainParams,
    ) -> Result<(), ValidationError> {
        transaction.validate_structure()?;
        if !transaction.is_coinbase() {
            return Err(ValidationError::InvalidCoinbaseReward);
        }
        if transaction.chain_id != params.chain_id {
            return Err(ValidationError::WrongNetwork);
        }
        if transaction.coinbase_height()? != height {
            return Err(ValidationError::CoinbaseHeight);
        }
        if params.treasury.enabled {
            // No default profile can reach this path. Enabling it is refused
            // until the quorum-spend script receives independent review.
            return Err(ValidationError::TreasuryNotReady);
        }

        let reward = sum_outputs(&transaction.outputs)?;
        let maximum_reward = block_subsidy(height).checked_add(fees)?;
        if reward > maximum_reward {
            return Err(ValidationError::InvalidCoinbaseReward);
        }
        let newly_issued = reward.checked_sub(fees).unwrap_or(Amount::ZERO);
        let total_issued = self.total_issued.checked_add(newly_issued)?;
        if total_issued > valid_subsidy_total_at_height(height) {
            return Err(ValidationError::SupplyExceeded);
        }

        self.insert_outputs(transaction, height, true)?;
        self.total_issued = total_issued;
        Ok(())
    }

    fn insert_outputs(
        &mut self,
        transaction: &Transaction,
        height: u64,
        coinbase: bool,
    ) -> Result<(), ValidationError> {
        let txid = transaction.txid();
        for (index, output) in transaction.outputs.iter().enumerate() {
            if matches!(output.policy, OutputPolicy::Burn { .. }) {
                continue;
            }
            let outpoint = OutPoint {
                txid,
                index: u32::try_from(index).map_err(|_| ValidationError::TxidCollision)?,
            };
            let replaced = self.entries.insert(
                outpoint,
                UtxoEntry {
                    output: output.clone(),
                    created_height: height,
                    coinbase,
                },
            );
            if replaced.is_some() {
                return Err(ValidationError::TxidCollision);
            }
        }
        Ok(())
    }
}

fn sum_outputs(outputs: &[TxOutput]) -> Result<Amount, ValidationError> {
    outputs.iter().try_fold(Amount::ZERO, |sum, output| {
        sum.checked_add(output.value).map_err(Into::into)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        crypto::{DomainTag, HybridKeyPair, RustCryptoProvider},
        hash::Hash32,
        params::{ChainParams, Network},
        transaction::{OutPoint, SpendWitness, Transaction, TxInput},
    };

    fn funded_utxo(
        params: &ChainParams,
        keys: &HybridKeyPair,
        height: u64,
    ) -> (UtxoSet, OutPoint, TxOutput) {
        let output = TxOutput {
            value: Amount::from_atoms(10_000).unwrap(),
            policy: OutputPolicy::Hybrid(keys.public_keys()),
        };
        let coinbase =
            Transaction::coinbase(params.chain_id, height, vec![output.clone()], b"fund");
        let outpoint = OutPoint {
            txid: coinbase.txid(),
            index: 0,
        };
        let mut set = UtxoSet::default();
        set.apply_coinbase(&coinbase, height, Amount::ZERO, params)
            .unwrap();
        (set, outpoint, output)
    }

    fn signed_spend(
        params: &ChainParams,
        keys: &HybridKeyPair,
        outpoint: OutPoint,
        previous_output: &TxOutput,
    ) -> Transaction {
        let mut transaction = Transaction::new(
            params.chain_id,
            vec![TxInput {
                previous_output: outpoint,
                sequence: u32::MAX,
                witness: SpendWitness::Empty,
            }],
            vec![TxOutput {
                value: Amount::from_atoms(9_000).unwrap(),
                policy: OutputPolicy::Burn {
                    commitment: Hash32([9; 32]),
                },
            }],
        );
        let message = transaction.signing_message(0, previous_output);
        transaction.inputs[0].witness = SpendWitness::Hybrid(
            keys.sign_hybrid(&params.chain_id, DomainTag::TransactionSignature, &message)
                .unwrap(),
        );
        transaction
    }

    #[test]
    fn one_utxo_cannot_be_spent_twice() {
        let params = ChainParams::for_network(Network::Regtest);
        let keys = HybridKeyPair::generate(None);
        let (mut set, outpoint, previous_output) = funded_utxo(&params, &keys, 1);
        let spend = signed_spend(&params, &keys, outpoint, &previous_output);
        let mature_height = 1 + params.coinbase_maturity;
        let fee = set
            .apply_regular_transaction(&spend, mature_height, &params, &RustCryptoProvider)
            .unwrap();
        assert_eq!(fee.atoms(), 1_000);
        assert_eq!(
            set.apply_regular_transaction(&spend, mature_height, &params, &RustCryptoProvider,),
            Err(ValidationError::MissingInput)
        );
    }

    #[test]
    fn inflationary_transaction_is_rejected() {
        let params = ChainParams::for_network(Network::Regtest);
        let keys = HybridKeyPair::generate(None);
        let (set, outpoint, previous_output) = funded_utxo(&params, &keys, 1);
        let mut spend = signed_spend(&params, &keys, outpoint, &previous_output);
        spend.outputs[0].value = Amount::from_atoms(10_001).unwrap();
        let message = spend.signing_message(0, &previous_output);
        spend.inputs[0].witness = SpendWitness::Hybrid(
            keys.sign_hybrid(&params.chain_id, DomainTag::TransactionSignature, &message)
                .unwrap(),
        );
        assert_eq!(
            set.validate_regular_transaction(
                &spend,
                1 + params.coinbase_maturity,
                &params,
                &RustCryptoProvider,
            ),
            Err(ValidationError::InputsBelowOutputs)
        );
    }
}
