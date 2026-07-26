use crate::{
    chain::{ChainError, ChainState},
    crypto::{
        CryptoError, DomainTag, MlDsaKeyPair, RustCryptoProvider, SignatureAlgorithm,
        SignatureProvider, signature_digest,
    },
    hash::Hash32,
    params::Network,
};
use primitive_types::U256;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const DEVELOPMENT_COMMITTEE_WARNING: &str = "DEVELOPMENT COMMITTEE ONLY:\n\
All checkpoint signers share one operational failure domain.\n\
This configuration provides no organisational decentralisation.";

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[repr(u8)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ConsensusStage {
    BootstrapFinality = 0,
    ReducedFinality = 1,
    ObserverOnly = 2,
    Retired = 3,
}

impl ConsensusStage {
    #[must_use]
    pub const fn next(self) -> Option<Self> {
        match self {
            Self::BootstrapFinality => Some(Self::ReducedFinality),
            Self::ReducedFinality => Some(Self::ObserverOnly),
            Self::ObserverOnly => Some(Self::Retired),
            Self::Retired => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitteeMember {
    pub signer_id: u16,
    #[serde(with = "hex_vec")]
    pub ml_dsa_public_key: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommitteeConfig {
    pub epoch: u32,
    pub quorum: u16,
    pub members: Vec<CommitteeMember>,
    pub local_development: bool,
    pub maximum_rotations: u16,
    pub rotation_activation_delay: u64,
}

impl CommitteeConfig {
    #[must_use]
    pub fn development() -> Self {
        let members = DevelopmentSigner::all()
            .into_iter()
            .map(|signer| CommitteeMember {
                signer_id: signer.signer_id,
                ml_dsa_public_key: signer.key.public_key(),
            })
            .collect();
        Self {
            epoch: 0,
            quorum: 7,
            members,
            local_development: true,
            maximum_rotations: 2,
            rotation_activation_delay: 20,
        }
    }

    #[must_use]
    pub const fn unconfigured_mainnet() -> Self {
        Self {
            epoch: 0,
            quorum: 0,
            members: Vec::new(),
            local_development: false,
            maximum_rotations: 0,
            rotation_activation_delay: 0,
        }
    }

    #[must_use]
    pub fn commitment(&self) -> Hash32 {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.epoch.to_le_bytes());
        bytes.extend_from_slice(&self.quorum.to_le_bytes());
        bytes.push(u8::from(self.local_development));
        bytes.extend_from_slice(&self.maximum_rotations.to_le_bytes());
        bytes.extend_from_slice(&self.rotation_activation_delay.to_le_bytes());
        for member in &self.members {
            bytes.extend_from_slice(&member.signer_id.to_le_bytes());
            bytes.extend_from_slice(&(member.ml_dsa_public_key.len() as u64).to_le_bytes());
            bytes.extend_from_slice(&member.ml_dsa_public_key);
        }
        Hash32::tagged_sha3("QBTC/COMMITTEE-CONFIG/v1", &bytes)
    }
}

pub struct DevelopmentSigner {
    pub signer_id: u16,
    key: MlDsaKeyPair,
}

impl DevelopmentSigner {
    #[must_use]
    pub fn all() -> Vec<Self> {
        (0..11).map(Self::from_id).collect()
    }

    #[must_use]
    pub fn from_id(signer_id: u16) -> Self {
        let seed = Hash32::tagged_sha3("QBTC/DEVELOPMENT-SIGNER-SEED/v1", &signer_id.to_le_bytes());
        Self {
            signer_id,
            key: MlDsaKeyPair::from_seed(seed.0),
        }
    }

    pub fn sign(&self, proposal: &CheckpointProposal) -> Result<CheckpointSignature, CryptoError> {
        let digest = signature_digest(
            &proposal.chain_id,
            DomainTag::CheckpointSignature,
            &proposal.signing_bytes(self.signer_id),
        );
        Ok(CheckpointSignature {
            signer_id: self.signer_id,
            algorithm: SignatureAlgorithm::MlDsa65Fips204V1,
            signature: self
                .key
                .sign_digest(&digest, DomainTag::CheckpointSignature)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointPolicy {
    pub minimum_depth: u64,
    pub minimum_spacing: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TransitionThreshold {
    pub minimum_network_age_secs: u64,
    pub minimum_height: u64,
    pub minimum_cumulative_chainwork: U256,
    pub minimum_rolling_work: U256,
    pub sustained_adjustment_periods: u32,
    pub minimum_average_block_interval_secs: u64,
    pub maximum_average_block_interval_secs: u64,
    pub activation_delay_blocks: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FinalityParams {
    pub bootstrap: CheckpointPolicy,
    pub reduced: CheckpointPolicy,
    pub bootstrap_to_reduced: Option<TransitionThreshold>,
    pub reduced_to_observer: Option<TransitionThreshold>,
    pub observer_to_retired: Option<TransitionThreshold>,
    pub mainnet_thresholds_reviewed: bool,
}

impl FinalityParams {
    #[must_use]
    pub fn for_network(network: Network) -> Self {
        match network {
            Network::Main => Self {
                bootstrap: CheckpointPolicy {
                    minimum_depth: 0,
                    minimum_spacing: 0,
                },
                reduced: CheckpointPolicy {
                    minimum_depth: 0,
                    minimum_spacing: 0,
                },
                bootstrap_to_reduced: None,
                reduced_to_observer: None,
                observer_to_retired: None,
                mainnet_thresholds_reviewed: false,
            },
            Network::Testnet => Self {
                bootstrap: CheckpointPolicy {
                    minimum_depth: 24,
                    minimum_spacing: 12,
                },
                reduced: CheckpointPolicy {
                    minimum_depth: 144,
                    minimum_spacing: 144,
                },
                bootstrap_to_reduced: Some(TransitionThreshold {
                    minimum_network_age_secs: 7 * 24 * 60 * 60,
                    minimum_height: 1_000,
                    minimum_cumulative_chainwork: U256::from(10_000_u64),
                    minimum_rolling_work: U256::from(2_u64),
                    sustained_adjustment_periods: 3,
                    minimum_average_block_interval_secs: 15,
                    maximum_average_block_interval_secs: 180,
                    activation_delay_blocks: 288,
                }),
                reduced_to_observer: Some(TransitionThreshold {
                    minimum_network_age_secs: 30 * 24 * 60 * 60,
                    minimum_height: 5_000,
                    minimum_cumulative_chainwork: U256::from(50_000_u64),
                    minimum_rolling_work: U256::from(3_u64),
                    sustained_adjustment_periods: 6,
                    minimum_average_block_interval_secs: 20,
                    maximum_average_block_interval_secs: 150,
                    activation_delay_blocks: 1_008,
                }),
                observer_to_retired: Some(TransitionThreshold {
                    minimum_network_age_secs: 90 * 24 * 60 * 60,
                    minimum_height: 15_000,
                    minimum_cumulative_chainwork: U256::from(150_000_u64),
                    minimum_rolling_work: U256::from(4_u64),
                    sustained_adjustment_periods: 9,
                    minimum_average_block_interval_secs: 20,
                    maximum_average_block_interval_secs: 150,
                    activation_delay_blocks: 2_016,
                }),
                mainnet_thresholds_reviewed: false,
            },
            Network::Regtest => Self {
                bootstrap: CheckpointPolicy {
                    minimum_depth: 3,
                    minimum_spacing: 2,
                },
                reduced: CheckpointPolicy {
                    minimum_depth: 6,
                    minimum_spacing: 5,
                },
                bootstrap_to_reduced: Some(TransitionThreshold {
                    minimum_network_age_secs: 10,
                    minimum_height: 10,
                    minimum_cumulative_chainwork: U256::one(),
                    minimum_rolling_work: U256::one(),
                    sustained_adjustment_periods: 2,
                    minimum_average_block_interval_secs: 1,
                    maximum_average_block_interval_secs: 10,
                    activation_delay_blocks: 3,
                }),
                reduced_to_observer: Some(TransitionThreshold {
                    minimum_network_age_secs: 20,
                    minimum_height: 20,
                    minimum_cumulative_chainwork: U256::one(),
                    minimum_rolling_work: U256::one(),
                    sustained_adjustment_periods: 3,
                    minimum_average_block_interval_secs: 1,
                    maximum_average_block_interval_secs: 10,
                    activation_delay_blocks: 3,
                }),
                observer_to_retired: Some(TransitionThreshold {
                    minimum_network_age_secs: 30,
                    minimum_height: 30,
                    minimum_cumulative_chainwork: U256::one(),
                    minimum_rolling_work: U256::one(),
                    sustained_adjustment_periods: 4,
                    minimum_average_block_interval_secs: 1,
                    maximum_average_block_interval_secs: 10,
                    activation_delay_blocks: 3,
                }),
                mainnet_thresholds_reviewed: false,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointProposal {
    pub chain_id: Hash32,
    pub network: Network,
    pub block_height: u64,
    pub block_hash: Hash32,
    pub cumulative_chainwork: U256,
    pub committee_epoch: u32,
    pub consensus_stage: ConsensusStage,
    pub previous_checkpoint_hash: Hash32,
    pub timestamp_window_start: u64,
    pub timestamp_window_end: u64,
    pub signature_algorithm: SignatureAlgorithm,
}

impl CheckpointProposal {
    #[must_use]
    pub fn signing_bytes(&self, signer_id: u16) -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&self.chain_id.0);
        bytes.push(match self.network {
            Network::Main => 0,
            Network::Testnet => 1,
            Network::Regtest => 2,
        });
        bytes.extend_from_slice(&self.block_height.to_le_bytes());
        bytes.extend_from_slice(&self.block_hash.0);
        bytes.extend_from_slice(&self.cumulative_chainwork.to_big_endian());
        bytes.extend_from_slice(&self.committee_epoch.to_le_bytes());
        bytes.push(self.consensus_stage as u8);
        bytes.extend_from_slice(&self.previous_checkpoint_hash.0);
        bytes.extend_from_slice(&self.timestamp_window_start.to_le_bytes());
        bytes.extend_from_slice(&self.timestamp_window_end.to_le_bytes());
        bytes.extend_from_slice(&(self.signature_algorithm as u16).to_le_bytes());
        bytes.extend_from_slice(&signer_id.to_le_bytes());
        bytes
    }

    #[must_use]
    pub fn proposal_hash(&self) -> Hash32 {
        Hash32::tagged_sha3("QBTC/CHECKPOINT-PROPOSAL/v1", &self.signing_bytes(u16::MAX))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointSignature {
    pub signer_id: u16,
    pub algorithm: SignatureAlgorithm,
    #[serde(with = "hex_vec")]
    pub signature: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CheckpointCertificate {
    pub proposal: CheckpointProposal,
    pub signatures: Vec<CheckpointSignature>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquivocationEvidence {
    pub committee_epoch: u32,
    pub block_height: u64,
    pub signer_id: u16,
    pub first_block_hash: Hash32,
    pub conflicting_block_hash: Hash32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckpointResult {
    Finalized { height: u64, hash: Hash32 },
    Observed { height: u64, hash: Hash32 },
    IgnoredRetired,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecuritySnapshot {
    pub network_age_secs: u64,
    pub height: u64,
    pub cumulative_chainwork: U256,
    pub rolling_work: U256,
    pub complete_adjustment_periods_above_threshold: u32,
    pub average_block_interval_secs: u64,
    pub unresolved_conflicting_checkpoints: bool,
    pub prohibited_deep_reorganisation: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingTransition {
    pub target: ConsensusStage,
    pub qualified_at_height: u64,
    pub activates_at_height: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageTransition {
    pub from: ConsensusStage,
    pub to: ConsensusStage,
    pub activation_height: u64,
}

#[derive(Debug, Error)]
pub enum FinalityError {
    #[error(transparent)]
    Crypto(#[from] CryptoError),
    #[error(transparent)]
    Chain(#[from] ChainError),
    #[error("checkpoint belongs to another chain or network")]
    WrongNetwork,
    #[error("checkpoint stage or committee epoch is incorrect")]
    WrongStageOrEpoch,
    #[error("checkpoint does not extend the previous checkpoint record")]
    WrongPreviousCheckpoint,
    #[error("checkpoint does not reference valid active-chain history")]
    UnknownHistoricalBlock,
    #[error("checkpoint cumulative chainwork does not match the referenced block")]
    WrongChainwork,
    #[error("checkpoint is not sufficiently deep")]
    InsufficientDepth,
    #[error("checkpoint is too close to the previous checkpoint")]
    CheckpointFrequency,
    #[error("checkpoint timestamp window is invalid")]
    TimestampWindow,
    #[error("checkpoint contains an unknown or duplicate signer")]
    SignerSet,
    #[error("checkpoint does not have the required quorum")]
    InsufficientQuorum,
    #[error("signer equivocation was detected")]
    Equivocation,
    #[error("mainnet transition thresholds have not received independent review")]
    MainnetThresholdsUnset,
    #[error("security snapshot does not match the active chain")]
    SnapshotMismatch,
}

pub struct FinalityState {
    params: FinalityParams,
    committee: CommitteeConfig,
    stage: ConsensusStage,
    latest_checkpoint: Option<CheckpointProposal>,
    seen_signatures: BTreeMap<(u32, u64, u16), Hash32>,
    equivocations: Vec<EquivocationEvidence>,
    pending_transition: Option<PendingTransition>,
}

impl FinalityState {
    #[must_use]
    pub fn new(params: FinalityParams, committee: CommitteeConfig) -> Self {
        Self {
            params,
            committee,
            stage: ConsensusStage::BootstrapFinality,
            latest_checkpoint: None,
            seen_signatures: BTreeMap::new(),
            equivocations: Vec::new(),
            pending_transition: None,
        }
    }

    #[must_use]
    pub const fn stage(&self) -> ConsensusStage {
        self.stage
    }

    #[must_use]
    pub fn latest_checkpoint(&self) -> Option<&CheckpointProposal> {
        self.latest_checkpoint.as_ref()
    }

    #[must_use]
    pub fn equivocations(&self) -> &[EquivocationEvidence] {
        &self.equivocations
    }

    #[must_use]
    pub fn pending_transition(&self) -> Option<&PendingTransition> {
        self.pending_transition.as_ref()
    }

    pub fn process_certificate(
        &mut self,
        certificate: &CheckpointCertificate,
        chain: &mut ChainState,
    ) -> Result<CheckpointResult, FinalityError> {
        if self.stage == ConsensusStage::Retired {
            return Ok(CheckpointResult::IgnoredRetired);
        }
        let proposal = &certificate.proposal;
        if proposal.chain_id != chain.params().chain_id
            || proposal.network != chain.params().network
        {
            return Err(FinalityError::WrongNetwork);
        }
        if proposal.consensus_stage != self.stage
            || proposal.committee_epoch != self.committee.epoch
            || proposal.signature_algorithm != SignatureAlgorithm::MlDsa65Fips204V1
        {
            return Err(FinalityError::WrongStageOrEpoch);
        }

        let expected_previous = self
            .latest_checkpoint
            .as_ref()
            .map_or(Hash32::ZERO, CheckpointProposal::proposal_hash);
        if proposal.previous_checkpoint_hash != expected_previous {
            return Err(FinalityError::WrongPreviousCheckpoint);
        }

        self.verify_signatures(certificate)?;
        if chain.block_at_height(proposal.block_height) != Some(proposal.block_hash) {
            return Err(FinalityError::UnknownHistoricalBlock);
        }
        if chain.chainwork_at(proposal.block_hash) != Some(proposal.cumulative_chainwork) {
            return Err(FinalityError::WrongChainwork);
        }
        let policy = match self.stage {
            ConsensusStage::BootstrapFinality => &self.params.bootstrap,
            ConsensusStage::ReducedFinality | ConsensusStage::ObserverOnly => &self.params.reduced,
            ConsensusStage::Retired => unreachable!("retired handled above"),
        };
        if chain.height().saturating_sub(proposal.block_height) < policy.minimum_depth {
            return Err(FinalityError::InsufficientDepth);
        }
        if self.latest_checkpoint.as_ref().is_some_and(|previous| {
            proposal.block_height.saturating_sub(previous.block_height) < policy.minimum_spacing
        }) {
            return Err(FinalityError::CheckpointFrequency);
        }
        if proposal.timestamp_window_start > proposal.timestamp_window_end
            || !(proposal.timestamp_window_start..=proposal.timestamp_window_end)
                .contains(&chain.tip_timestamp())
        {
            return Err(FinalityError::TimestampWindow);
        }

        self.latest_checkpoint = Some(proposal.clone());
        if self.stage == ConsensusStage::ObserverOnly {
            Ok(CheckpointResult::Observed {
                height: proposal.block_height,
                hash: proposal.block_hash,
            })
        } else {
            chain.install_checkpoint_guard(proposal.block_height, proposal.block_hash)?;
            Ok(CheckpointResult::Finalized {
                height: proposal.block_height,
                hash: proposal.block_hash,
            })
        }
    }

    pub fn evaluate_transitions(
        &mut self,
        snapshot: &SecuritySnapshot,
        chain: &mut ChainState,
    ) -> Result<Option<StageTransition>, FinalityError> {
        if snapshot.height != chain.height() || snapshot.cumulative_chainwork != chain.chainwork() {
            return Err(FinalityError::SnapshotMismatch);
        }
        if let Some(pending) = &self.pending_transition {
            if snapshot.height >= pending.activates_at_height {
                let transition = StageTransition {
                    from: self.stage,
                    to: pending.target,
                    activation_height: pending.activates_at_height,
                };
                self.stage = pending.target;
                self.pending_transition = None;
                if self.stage == ConsensusStage::ObserverOnly {
                    chain.clear_checkpoint_guard();
                } else if self.stage == ConsensusStage::Retired {
                    chain.retire_checkpoint_authority();
                }
                return Ok(Some(transition));
            }
            return Ok(None);
        }

        let (target, threshold) = match self.stage {
            ConsensusStage::BootstrapFinality => (
                ConsensusStage::ReducedFinality,
                self.params.bootstrap_to_reduced.as_ref(),
            ),
            ConsensusStage::ReducedFinality => (
                ConsensusStage::ObserverOnly,
                self.params.reduced_to_observer.as_ref(),
            ),
            ConsensusStage::ObserverOnly => (
                ConsensusStage::Retired,
                self.params.observer_to_retired.as_ref(),
            ),
            ConsensusStage::Retired => return Ok(None),
        };
        let threshold = threshold.ok_or(FinalityError::MainnetThresholdsUnset)?;
        if qualifies(snapshot, threshold) {
            self.pending_transition = Some(PendingTransition {
                target,
                qualified_at_height: snapshot.height,
                activates_at_height: snapshot
                    .height
                    .saturating_add(threshold.activation_delay_blocks),
            });
        }
        Ok(None)
    }

    fn verify_signatures(
        &mut self,
        certificate: &CheckpointCertificate,
    ) -> Result<(), FinalityError> {
        let members: BTreeMap<u16, &[u8]> = self
            .committee
            .members
            .iter()
            .map(|member| (member.signer_id, member.ml_dsa_public_key.as_slice()))
            .collect();
        let mut unique = BTreeSet::new();
        let mut valid = 0_u16;
        let provider = RustCryptoProvider;

        for signature in &certificate.signatures {
            if signature.algorithm != SignatureAlgorithm::MlDsa65Fips204V1
                || !unique.insert(signature.signer_id)
            {
                return Err(FinalityError::SignerSet);
            }
            let public_key = members
                .get(&signature.signer_id)
                .ok_or(FinalityError::SignerSet)?;
            let digest = signature_digest(
                &certificate.proposal.chain_id,
                DomainTag::CheckpointSignature,
                &certificate.proposal.signing_bytes(signature.signer_id),
            );
            provider.verify(
                signature.algorithm,
                public_key,
                &signature.signature,
                &digest,
                DomainTag::CheckpointSignature,
            )?;

            let key = (
                certificate.proposal.committee_epoch,
                certificate.proposal.block_height,
                signature.signer_id,
            );
            if let Some(first) = self
                .seen_signatures
                .insert(key, certificate.proposal.block_hash)
                && first != certificate.proposal.block_hash
            {
                self.equivocations.push(EquivocationEvidence {
                    committee_epoch: certificate.proposal.committee_epoch,
                    block_height: certificate.proposal.block_height,
                    signer_id: signature.signer_id,
                    first_block_hash: first,
                    conflicting_block_hash: certificate.proposal.block_hash,
                });
                return Err(FinalityError::Equivocation);
            }
            valid = valid.saturating_add(1);
        }
        if valid < self.committee.quorum {
            return Err(FinalityError::InsufficientQuorum);
        }
        Ok(())
    }
}

fn qualifies(snapshot: &SecuritySnapshot, threshold: &TransitionThreshold) -> bool {
    snapshot.network_age_secs >= threshold.minimum_network_age_secs
        && snapshot.height >= threshold.minimum_height
        && snapshot.cumulative_chainwork >= threshold.minimum_cumulative_chainwork
        && snapshot.rolling_work >= threshold.minimum_rolling_work
        && snapshot.complete_adjustment_periods_above_threshold
            >= threshold.sustained_adjustment_periods
        && snapshot.average_block_interval_secs >= threshold.minimum_average_block_interval_secs
        && snapshot.average_block_interval_secs <= threshold.maximum_average_block_interval_secs
        && !snapshot.unresolved_conflicting_checkpoints
        && !snapshot.prohibited_deep_reorganisation
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
        block::Block,
        crypto::{HybridKeyPair, RustCryptoProvider},
        params::ChainParams,
        pow::Sha256dPowV1,
        transaction::OutputPolicy,
    };

    fn mine_blocks(chain: &mut ChainState, count: u64) {
        let policy = OutputPolicy::Hybrid(HybridKeyPair::generate(None).public_keys());
        for index in 0..count {
            let mut block = chain
                .block_template(
                    policy.clone(),
                    Vec::new(),
                    chain.tip_timestamp() + 1,
                    &index.to_le_bytes(),
                    &RustCryptoProvider,
                )
                .unwrap();
            block.mine(&Sha256dPowV1, u32::MAX).unwrap();
            chain.submit_block(block, &RustCryptoProvider).unwrap();
        }
    }

    fn proposal(chain: &ChainState, finality: &FinalityState, height: u64) -> CheckpointProposal {
        let block_hash = chain.block_at_height(height).unwrap();
        CheckpointProposal {
            chain_id: chain.params().chain_id,
            network: chain.params().network,
            block_height: height,
            block_hash,
            cumulative_chainwork: chain.chainwork_at(block_hash).unwrap(),
            committee_epoch: 0,
            consensus_stage: finality.stage(),
            previous_checkpoint_hash: finality
                .latest_checkpoint()
                .map_or(Hash32::ZERO, CheckpointProposal::proposal_hash),
            timestamp_window_start: chain.tip_timestamp().saturating_sub(1),
            timestamp_window_end: chain.tip_timestamp() + 1,
            signature_algorithm: SignatureAlgorithm::MlDsa65Fips204V1,
        }
    }

    fn certificate(proposal: CheckpointProposal, signers: usize) -> CheckpointCertificate {
        let signatures = DevelopmentSigner::all()
            .into_iter()
            .take(signers)
            .map(|signer| signer.sign(&proposal).unwrap())
            .collect();
        CheckpointCertificate {
            proposal,
            signatures,
        }
    }

    fn security_snapshot(
        chain: &ChainState,
        network_age_secs: u64,
        periods: u32,
        rolling_work: U256,
    ) -> SecuritySnapshot {
        SecuritySnapshot {
            network_age_secs,
            height: chain.height(),
            cumulative_chainwork: chain.chainwork(),
            rolling_work,
            complete_adjustment_periods_above_threshold: periods,
            average_block_interval_secs: 2,
            unresolved_conflicting_checkpoints: false,
            prohibited_deep_reorganisation: false,
        }
    }

    #[test]
    fn six_signatures_fail_and_seven_finalize_deep_valid_history() {
        let params = ChainParams::for_network(Network::Regtest);
        let committee = params.committee.clone();
        let mut chain = ChainState::new(params.clone()).unwrap();
        mine_blocks(&mut chain, 5);
        let mut finality = FinalityState::new(params.finality.clone(), committee);
        let checkpoint = proposal(&chain, &finality, 1);
        assert!(matches!(
            finality.process_certificate(&certificate(checkpoint.clone(), 6), &mut chain),
            Err(FinalityError::InsufficientQuorum)
        ));
        assert!(matches!(
            finality.process_certificate(&certificate(checkpoint, 7), &mut chain),
            Ok(CheckpointResult::Finalized { height: 1, .. })
        ));
        assert_eq!(chain.checkpoint_guard().unwrap().height, 1);
    }

    #[test]
    fn quorum_cannot_finalize_an_unknown_or_shallow_block() {
        let params = ChainParams::for_network(Network::Regtest);
        let mut chain = ChainState::new(params.clone()).unwrap();
        mine_blocks(&mut chain, 5);
        let mut finality = FinalityState::new(params.finality.clone(), params.committee.clone());
        let mut unknown = proposal(&chain, &finality, 1);
        unknown.block_hash = Hash32([99; 32]);
        assert!(matches!(
            finality.process_certificate(&certificate(unknown, 7), &mut chain),
            Err(FinalityError::UnknownHistoricalBlock | FinalityError::WrongChainwork)
        ));

        let shallow = proposal(&chain, &finality, 4);
        assert!(matches!(
            finality.process_certificate(&certificate(shallow, 7), &mut chain),
            Err(FinalityError::InsufficientDepth)
        ));
    }

    #[test]
    fn checkpoint_does_not_participate_in_block_validation() {
        let params = ChainParams::for_network(Network::Regtest);
        let mut chain = ChainState::new(params.clone()).unwrap();
        mine_blocks(&mut chain, 5);
        let mut finality = FinalityState::new(params.finality.clone(), params.committee.clone());
        let cert = certificate(proposal(&chain, &finality, 1), 7);
        finality.process_certificate(&cert, &mut chain).unwrap();
        let before = chain.height();
        mine_blocks(&mut chain, 1);
        assert_eq!(chain.height(), before + 1);
    }

    #[test]
    fn higher_work_reorganisation_cannot_cross_a_final_checkpoint() {
        let params = ChainParams::for_network(Network::Regtest);
        let mut chain = ChainState::new(params.clone()).unwrap();
        mine_blocks(&mut chain, 5);
        let mut finality = FinalityState::new(params.finality.clone(), params.committee.clone());
        let cert = certificate(proposal(&chain, &finality, 1), 7);
        finality.process_certificate(&cert, &mut chain).unwrap();

        let genesis = chain.block_at_height(0).unwrap();
        let policy = OutputPolicy::Hybrid(HybridKeyPair::generate(None).public_keys());
        let mut parent = genesis;
        for index in 0_u64..6 {
            let mut block = chain
                .block_template_on(
                    parent,
                    policy.clone(),
                    Vec::new(),
                    params.genesis_timestamp + 100 + index,
                    &index.to_le_bytes(),
                    &RustCryptoProvider,
                )
                .unwrap();
            block.mine(&Sha256dPowV1, u32::MAX).unwrap();
            let hash = block.hash(&Sha256dPowV1);
            let result = chain.submit_block(block, &RustCryptoProvider);
            if index < 5 {
                assert!(result.is_ok());
                parent = hash;
            } else {
                assert!(matches!(
                    result,
                    Err(ChainError::FinalizedCheckpointConflict)
                ));
            }
        }
    }

    #[test]
    fn stage_transition_uses_all_conditions_and_a_locked_delay() {
        let params = ChainParams::for_network(Network::Regtest);
        let mut chain = ChainState::new(params.clone()).unwrap();
        mine_blocks(&mut chain, 10);
        let mut finality = FinalityState::new(params.finality.clone(), params.committee.clone());
        let qualifying = security_snapshot(&chain, 10, 2, U256::from(2));
        assert_eq!(
            finality
                .evaluate_transitions(&qualifying, &mut chain)
                .unwrap(),
            None
        );
        assert_eq!(
            finality.pending_transition().unwrap().activates_at_height,
            13
        );
        mine_blocks(&mut chain, 3);
        let after_delay = security_snapshot(&chain, 13, 0, U256::zero());
        let transition = finality
            .evaluate_transitions(&after_delay, &mut chain)
            .unwrap()
            .unwrap();
        assert_eq!(transition.to, ConsensusStage::ReducedFinality);
    }

    #[test]
    fn observer_and_retirement_remove_authority_irreversibly() {
        let params = ChainParams::for_network(Network::Regtest);
        let mut chain = ChainState::new(params.clone()).unwrap();
        let mut finality = FinalityState::new(params.finality.clone(), params.committee.clone());

        mine_blocks(&mut chain, 10);
        finality
            .evaluate_transitions(&security_snapshot(&chain, 10, 2, U256::one()), &mut chain)
            .unwrap();
        mine_blocks(&mut chain, 3);
        finality
            .evaluate_transitions(&security_snapshot(&chain, 13, 0, U256::zero()), &mut chain)
            .unwrap();
        assert_eq!(finality.stage(), ConsensusStage::ReducedFinality);

        mine_blocks(&mut chain, 7);
        finality
            .evaluate_transitions(&security_snapshot(&chain, 20, 3, U256::one()), &mut chain)
            .unwrap();
        mine_blocks(&mut chain, 3);
        finality
            .evaluate_transitions(&security_snapshot(&chain, 23, 0, U256::zero()), &mut chain)
            .unwrap();
        assert_eq!(finality.stage(), ConsensusStage::ObserverOnly);
        assert_eq!(chain.checkpoint_guard(), None);

        let observation = certificate(proposal(&chain, &finality, 10), 7);
        assert!(matches!(
            finality.process_certificate(&observation, &mut chain),
            Ok(CheckpointResult::Observed { height: 10, .. })
        ));
        assert_eq!(chain.checkpoint_guard(), None);

        mine_blocks(&mut chain, 7);
        finality
            .evaluate_transitions(&security_snapshot(&chain, 30, 4, U256::one()), &mut chain)
            .unwrap();
        mine_blocks(&mut chain, 3);
        finality
            .evaluate_transitions(&security_snapshot(&chain, 33, 0, U256::zero()), &mut chain)
            .unwrap();
        assert_eq!(finality.stage(), ConsensusStage::Retired);
        assert!(chain.checkpoint_authority_retired());
        assert!(matches!(
            finality.process_certificate(&observation, &mut chain),
            Ok(CheckpointResult::IgnoredRetired)
        ));
        assert!(matches!(
            chain.install_checkpoint_guard(1, chain.block_at_height(1).unwrap()),
            Err(ChainError::CheckpointAuthorityRetired)
        ));
    }

    #[test]
    fn conflicting_signatures_are_recorded_as_equivocation() {
        let params = ChainParams::for_network(Network::Regtest);
        let mut chain = ChainState::new(params.clone()).unwrap();
        mine_blocks(&mut chain, 5);
        let mut finality = FinalityState::new(params.finality.clone(), params.committee.clone());
        let first = proposal(&chain, &finality, 1);
        assert!(matches!(
            finality.process_certificate(&certificate(first.clone(), 6), &mut chain),
            Err(FinalityError::InsufficientQuorum)
        ));

        let mut conflict = first;
        conflict.block_hash = Hash32([42; 32]);
        assert!(matches!(
            finality.process_certificate(&certificate(conflict, 7), &mut chain),
            Err(FinalityError::Equivocation)
        ));
        assert_eq!(finality.equivocations().len(), 1);
    }

    #[test]
    fn proof_of_work_selects_forks_after_retirement() {
        let params = ChainParams::for_network(Network::Regtest);
        let mut chain = ChainState::new(params.clone()).unwrap();
        mine_blocks(&mut chain, 5);
        let checkpoint_hash = chain.block_at_height(1).unwrap();
        chain.install_checkpoint_guard(1, checkpoint_hash).unwrap();
        chain.retire_checkpoint_authority();

        let genesis = chain.block_at_height(0).unwrap();
        let policy = OutputPolicy::Hybrid(HybridKeyPair::generate(None).public_keys());
        let mut parent = genesis;
        for index in 0_u64..6 {
            let mut block = chain
                .block_template_on(
                    parent,
                    policy.clone(),
                    Vec::new(),
                    params.genesis_timestamp + 100 + index,
                    &index.to_le_bytes(),
                    &RustCryptoProvider,
                )
                .unwrap();
            block.mine(&Sha256dPowV1, u32::MAX).unwrap();
            let hash = block.hash(&Sha256dPowV1);
            let outcome = chain.submit_block(block, &RustCryptoProvider).unwrap();
            parent = hash;
            if index == 5 {
                assert!(outcome.became_active);
                assert_eq!(chain.tip_hash(), hash);
            }
        }
    }

    #[test]
    fn certificate_signatures_commit_to_signer_identity() {
        let params = ChainParams::for_network(Network::Regtest);
        let mut chain = ChainState::new(params.clone()).unwrap();
        mine_blocks(&mut chain, 5);
        let mut finality = FinalityState::new(params.finality.clone(), params.committee.clone());
        let checkpoint = proposal(&chain, &finality, 1);
        let mut cert = certificate(checkpoint, 7);
        cert.signatures[0].signer_id = 10;
        assert!(matches!(
            finality.process_certificate(&cert, &mut chain),
            Err(FinalityError::Crypto(CryptoError::VerificationFailed))
        ));
    }

    #[allow(dead_code)]
    fn _assert_block_is_send_sync(_: &Block) {}
}
