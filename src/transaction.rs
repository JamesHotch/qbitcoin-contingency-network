use crate::{
    amount::Amount,
    crypto::{
        HybridPublicKeys, HybridSignature, RecoveryPublicKey, RecoverySignature, SpendPolicyVersion,
    },
    encoding::{Decoder, EncodingError, put_bytes, put_compact},
    hash::Hash32,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const TRANSACTION_VERSION: u16 = 1;
pub const MAX_INPUTS: usize = 10_000;
pub const MAX_OUTPUTS: usize = 10_000;
pub const MAX_COINBASE_DATA: usize = 100;
pub const MAX_PUBLIC_KEY_BYTES: usize = 64 * 1024;
pub const MAX_SIGNATURE_BYTES: usize = 128 * 1024;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum TransactionError {
    #[error(transparent)]
    Encoding(#[from] EncodingError),
    #[error("transaction version is not supported")]
    UnsupportedVersion,
    #[error("transaction has no inputs")]
    NoInputs,
    #[error("transaction has no outputs")]
    NoOutputs,
    #[error("transaction contains a duplicate input")]
    DuplicateInput,
    #[error("coinbase encoding is invalid")]
    InvalidCoinbase,
}

#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct OutPoint {
    pub txid: Hash32,
    pub index: u32,
}

impl OutPoint {
    pub const COINBASE: Self = Self {
        txid: Hash32::ZERO,
        index: u32::MAX,
    };
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxInput {
    pub previous_output: OutPoint,
    /// Minimum relative age in blocks. `u32::MAX` disables the general lock.
    pub sequence: u32,
    pub witness: SpendWitness,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SpendWitness {
    Empty,
    Coinbase {
        #[serde(with = "hex_vec")]
        data: Vec<u8>,
    },
    Hybrid(HybridSignature),
    Recovery(RecoverySignature),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OutputPolicy {
    Hybrid(HybridPublicKeys),
    Burn { commitment: Hash32 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TxOutput {
    pub value: Amount,
    pub policy: OutputPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transaction {
    pub version: u16,
    pub chain_id: Hash32,
    pub inputs: Vec<TxInput>,
    pub outputs: Vec<TxOutput>,
    /// Absolute block-height lock for this research testnet.
    pub lock_time: u64,
}

impl Transaction {
    #[must_use]
    pub fn new(chain_id: Hash32, inputs: Vec<TxInput>, outputs: Vec<TxOutput>) -> Self {
        Self {
            version: TRANSACTION_VERSION,
            chain_id,
            inputs,
            outputs,
            lock_time: 0,
        }
    }

    #[must_use]
    pub fn coinbase(
        chain_id: Hash32,
        height: u64,
        outputs: Vec<TxOutput>,
        extra_nonce: &[u8],
    ) -> Self {
        let mut data = height.to_le_bytes().to_vec();
        data.extend_from_slice(extra_nonce);
        data.truncate(MAX_COINBASE_DATA);
        Self::new(
            chain_id,
            vec![TxInput {
                previous_output: OutPoint::COINBASE,
                sequence: u32::MAX,
                witness: SpendWitness::Coinbase { data },
            }],
            outputs,
        )
    }

    #[must_use]
    pub fn is_coinbase(&self) -> bool {
        self.inputs.len() == 1
            && self.inputs[0].previous_output == OutPoint::COINBASE
            && matches!(self.inputs[0].witness, SpendWitness::Coinbase { .. })
    }

    pub fn coinbase_height(&self) -> Result<u64, TransactionError> {
        if !self.is_coinbase() {
            return Err(TransactionError::InvalidCoinbase);
        }
        let SpendWitness::Coinbase { data } = &self.inputs[0].witness else {
            return Err(TransactionError::InvalidCoinbase);
        };
        let encoded_height = data
            .get(..8)
            .ok_or(TransactionError::InvalidCoinbase)?
            .try_into()
            .map_err(|_| TransactionError::InvalidCoinbase)?;
        Ok(u64::from_le_bytes(encoded_height))
    }

    pub fn validate_structure(&self) -> Result<(), TransactionError> {
        if self.version != TRANSACTION_VERSION {
            return Err(TransactionError::UnsupportedVersion);
        }
        if self.inputs.is_empty() {
            return Err(TransactionError::NoInputs);
        }
        if self.outputs.is_empty() {
            return Err(TransactionError::NoOutputs);
        }
        if self.inputs.len() > MAX_INPUTS || self.outputs.len() > MAX_OUTPUTS {
            return Err(EncodingError::LengthLimit.into());
        }
        if self.is_coinbase() {
            let SpendWitness::Coinbase { data } = &self.inputs[0].witness else {
                return Err(TransactionError::InvalidCoinbase);
            };
            if !(8..=MAX_COINBASE_DATA).contains(&data.len()) {
                return Err(TransactionError::InvalidCoinbase);
            }
        } else {
            let mut seen = BTreeSet::new();
            for input in &self.inputs {
                if input.previous_output == OutPoint::COINBASE {
                    return Err(TransactionError::InvalidCoinbase);
                }
                if !seen.insert(input.previous_output) {
                    return Err(TransactionError::DuplicateInput);
                }
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn txid(&self) -> Hash32 {
        Hash32::sha256d(&self.encode(false))
    }

    #[must_use]
    pub fn wtxid(&self) -> Hash32 {
        Hash32::sha256d(&self.encode(true))
    }

    #[must_use]
    pub fn encode(&self, include_witness: bool) -> Vec<u8> {
        let mut output = Vec::new();
        output.extend_from_slice(&self.version.to_le_bytes());
        output.extend_from_slice(&self.chain_id.0);
        put_compact(self.inputs.len() as u64, &mut output);
        for input in &self.inputs {
            output.extend_from_slice(&input.previous_output.txid.0);
            output.extend_from_slice(&input.previous_output.index.to_le_bytes());
            output.extend_from_slice(&input.sequence.to_le_bytes());
            // Coinbase height and entropy are part of its immutable identity,
            // even though ordinary spend witnesses remain segregated.
            if include_witness || matches!(input.witness, SpendWitness::Coinbase { .. }) {
                encode_witness(&input.witness, &mut output);
            }
        }
        put_compact(self.outputs.len() as u64, &mut output);
        for transaction_output in &self.outputs {
            encode_output(transaction_output, &mut output);
        }
        output.extend_from_slice(&self.lock_time.to_le_bytes());
        output
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, TransactionError> {
        let mut decoder = Decoder::new(bytes);
        let version = decoder.read_u16()?;
        let chain_id = Hash32(decoder.read_array()?);
        let input_count = read_bounded_count(&mut decoder, MAX_INPUTS)?;
        let mut inputs = Vec::with_capacity(input_count);
        for _ in 0..input_count {
            inputs.push(TxInput {
                previous_output: OutPoint {
                    txid: Hash32(decoder.read_array()?),
                    index: decoder.read_u32()?,
                },
                sequence: decoder.read_u32()?,
                witness: decode_witness(&mut decoder)?,
            });
        }
        let output_count = read_bounded_count(&mut decoder, MAX_OUTPUTS)?;
        let mut outputs = Vec::with_capacity(output_count);
        for _ in 0..output_count {
            outputs.push(decode_output(&mut decoder)?);
        }
        let transaction = Self {
            version,
            chain_id,
            inputs,
            outputs,
            lock_time: decoder.read_u64()?,
        };
        decoder.finish()?;
        transaction.validate_structure()?;
        if transaction.encode(true) != bytes {
            return Err(EncodingError::InvalidValue.into());
        }
        Ok(transaction)
    }

    #[must_use]
    pub fn signing_message(&self, input_index: usize, previous_output: &TxOutput) -> Vec<u8> {
        let mut bytes = self.encode(false);
        bytes.extend_from_slice(&(input_index as u64).to_le_bytes());
        encode_output(previous_output, &mut bytes);
        bytes
    }

    #[must_use]
    pub fn base_size(&self) -> usize {
        self.encode(false).len()
    }

    #[must_use]
    pub fn witness_size(&self) -> usize {
        self.encode(true).len().saturating_sub(self.base_size())
    }

    #[must_use]
    pub fn weight(&self) -> u64 {
        (self.base_size() as u64)
            .saturating_mul(4)
            .saturating_add((self.witness_size() as u64).saturating_mul(2))
    }

    #[must_use]
    pub fn signature_verification_cost(&self) -> u64 {
        self.inputs
            .iter()
            .map(|input| match input.witness {
                SpendWitness::Hybrid(_) => 51,
                SpendWitness::Recovery(_) => 200,
                SpendWitness::Empty | SpendWitness::Coinbase { .. } => 0,
            })
            .sum()
    }
}

fn read_bounded_count(decoder: &mut Decoder<'_>, maximum: usize) -> Result<usize, EncodingError> {
    let count = usize::try_from(decoder.read_compact()?).map_err(|_| EncodingError::LengthLimit)?;
    if count > maximum {
        Err(EncodingError::LengthLimit)
    } else {
        Ok(count)
    }
}

fn encode_witness(witness: &SpendWitness, output: &mut Vec<u8>) {
    match witness {
        SpendWitness::Empty => output.push(0),
        SpendWitness::Coinbase { data } => {
            output.push(1);
            put_bytes(data, output);
        }
        SpendWitness::Hybrid(signature) => {
            output.push(2);
            output.extend_from_slice(&(signature.policy_version as u16).to_le_bytes());
            encode_optional_bytes(signature.schnorr.as_deref(), output);
            encode_optional_bytes(signature.ml_dsa.as_deref(), output);
        }
        SpendWitness::Recovery(signature) => {
            output.push(3);
            output.extend_from_slice(&(signature.policy_version as u16).to_le_bytes());
            put_bytes(&signature.slh_dsa, output);
        }
    }
}

fn decode_witness(decoder: &mut Decoder<'_>) -> Result<SpendWitness, EncodingError> {
    match decoder.read_u8()? {
        0 => Ok(SpendWitness::Empty),
        1 => Ok(SpendWitness::Coinbase {
            data: decoder.read_bytes(MAX_COINBASE_DATA)?,
        }),
        2 => Ok(SpendWitness::Hybrid(HybridSignature {
            policy_version: decode_policy_version(decoder.read_u16()?)?,
            schnorr: decode_optional_bytes(decoder, MAX_SIGNATURE_BYTES)?,
            ml_dsa: decode_optional_bytes(decoder, MAX_SIGNATURE_BYTES)?,
        })),
        3 => Ok(SpendWitness::Recovery(RecoverySignature {
            policy_version: decode_policy_version(decoder.read_u16()?)?,
            slh_dsa: decoder.read_bytes(MAX_SIGNATURE_BYTES)?,
        })),
        _ => Err(EncodingError::UnknownTag),
    }
}

fn encode_optional_bytes(bytes: Option<&[u8]>, output: &mut Vec<u8>) {
    match bytes {
        Some(bytes) => {
            output.push(1);
            put_bytes(bytes, output);
        }
        None => output.push(0),
    }
}

fn decode_optional_bytes(
    decoder: &mut Decoder<'_>,
    maximum: usize,
) -> Result<Option<Vec<u8>>, EncodingError> {
    match decoder.read_u8()? {
        0 => Ok(None),
        1 => Ok(Some(decoder.read_bytes(maximum)?)),
        _ => Err(EncodingError::UnknownTag),
    }
}

pub(crate) fn encode_output(transaction_output: &TxOutput, output: &mut Vec<u8>) {
    output.extend_from_slice(&transaction_output.value.atoms().to_le_bytes());
    match &transaction_output.policy {
        OutputPolicy::Hybrid(keys) => {
            output.push(1);
            output.extend_from_slice(&(keys.policy_version as u16).to_le_bytes());
            put_bytes(&keys.schnorr, output);
            put_bytes(&keys.ml_dsa, output);
            match &keys.recovery {
                Some(recovery) => {
                    output.push(1);
                    encode_recovery_public_key(recovery, output);
                }
                None => output.push(0),
            }
        }
        OutputPolicy::Burn { commitment } => {
            output.push(2);
            output.extend_from_slice(&commitment.0);
        }
    }
}

fn decode_output(decoder: &mut Decoder<'_>) -> Result<TxOutput, EncodingError> {
    let value = Amount::from_atoms(decoder.read_u64()?).map_err(|_| EncodingError::InvalidValue)?;
    let policy = match decoder.read_u8()? {
        1 => {
            let policy_version = decode_policy_version(decoder.read_u16()?)?;
            let schnorr = decoder.read_bytes(MAX_PUBLIC_KEY_BYTES)?;
            let ml_dsa = decoder.read_bytes(MAX_PUBLIC_KEY_BYTES)?;
            let recovery = match decoder.read_u8()? {
                0 => None,
                1 => Some(decode_recovery_public_key(decoder)?),
                _ => return Err(EncodingError::UnknownTag),
            };
            OutputPolicy::Hybrid(HybridPublicKeys {
                policy_version,
                schnorr,
                ml_dsa,
                recovery,
            })
        }
        2 => OutputPolicy::Burn {
            commitment: Hash32(decoder.read_array()?),
        },
        _ => return Err(EncodingError::UnknownTag),
    };
    Ok(TxOutput { value, policy })
}

fn encode_recovery_public_key(recovery: &RecoveryPublicKey, output: &mut Vec<u8>) {
    output.extend_from_slice(&(recovery.policy_version as u16).to_le_bytes());
    output.extend_from_slice(&recovery.relative_timelock.to_le_bytes());
    put_bytes(&recovery.slh_dsa, output);
}

fn decode_recovery_public_key(
    decoder: &mut Decoder<'_>,
) -> Result<RecoveryPublicKey, EncodingError> {
    Ok(RecoveryPublicKey {
        policy_version: decode_policy_version(decoder.read_u16()?)?,
        relative_timelock: decoder.read_u32()?,
        slh_dsa: decoder.read_bytes(MAX_PUBLIC_KEY_BYTES)?,
    })
}

fn decode_policy_version(value: u16) -> Result<SpendPolicyVersion, EncodingError> {
    match value {
        0x0100 => Ok(SpendPolicyVersion::HybridSchnorrMlDsaV1),
        0x0101 => Ok(SpendPolicyVersion::SlhDsaRecoveryV1),
        _ => Err(EncodingError::UnknownTag),
    }
}

mod hex_vec {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&hex::encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
        let encoded = String::deserialize(deserializer)?;
        hex::decode(encoded).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        crypto::HybridKeyPair,
        params::{ChainParams, Network},
    };

    fn sample_transaction() -> Transaction {
        let params = ChainParams::for_network(Network::Regtest);
        let keys = HybridKeyPair::generate(None).public_keys();
        Transaction::new(
            params.chain_id,
            vec![TxInput {
                previous_output: OutPoint {
                    txid: Hash32([3; 32]),
                    index: 1,
                },
                sequence: u32::MAX,
                witness: SpendWitness::Empty,
            }],
            vec![TxOutput {
                value: Amount::from_atoms(42).unwrap(),
                policy: OutputPolicy::Hybrid(keys),
            }],
        )
    }

    #[test]
    fn canonical_transaction_round_trip() {
        let transaction = sample_transaction();
        let bytes = transaction.encode(true);
        assert_eq!(Transaction::decode(&bytes).unwrap(), transaction);
    }

    #[test]
    fn txid_excludes_witness_but_wtxid_commits_to_it() {
        let transaction = sample_transaction();
        let mut changed = transaction.clone();
        changed.inputs[0].witness = SpendWitness::Hybrid(HybridSignature {
            policy_version: SpendPolicyVersion::HybridSchnorrMlDsaV1,
            schnorr: Some(vec![1; 64]),
            ml_dsa: Some(vec![2; 3_309]),
        });
        assert_eq!(transaction.txid(), changed.txid());
        assert_ne!(transaction.wtxid(), changed.wtxid());
    }

    #[test]
    fn duplicate_inputs_are_rejected() {
        let mut transaction = sample_transaction();
        transaction.inputs.push(transaction.inputs[0].clone());
        assert_eq!(
            transaction.validate_structure(),
            Err(TransactionError::DuplicateInput)
        );
    }
}
