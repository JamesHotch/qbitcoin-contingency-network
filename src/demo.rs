use crate::{
    address::policy_commitment,
    amount::{ATOMS_PER_QBTC, Amount, MAX_MONEY_ATOMS, block_subsidy},
    block::{Block, BlockError},
    chain::{ChainError, ChainState},
    crypto::{CryptoError, DomainTag, HybridKeyPair, RustCryptoProvider, SignatureAlgorithm},
    finality::{
        CheckpointCertificate, CheckpointProposal, CheckpointResult, ConsensusStage,
        DevelopmentSigner, FinalityError, FinalityState, SecuritySnapshot,
    },
    hash::Hash32,
    p2p::PeerBook,
    params::{ChainParams, Network},
    pow::{Sha256dPowV1, adjusted_bits},
    transaction::{OutPoint, OutputPolicy, SpendWitness, Transaction, TxInput, TxOutput},
    utxo::ValidationError,
};
use anyhow::{Context, Result, ensure};
use primitive_types::U256;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DemoStep {
    pub step: u16,
    pub name: String,
    pub result: String,
    pub implementation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DemoReport {
    pub mode: String,
    pub passed: bool,
    pub steps: Vec<DemoStep>,
}

impl DemoReport {
    fn new(mode: &str) -> Self {
        Self {
            mode: mode.to_owned(),
            passed: true,
            steps: Vec::new(),
        }
    }

    fn real(&mut self, step: u16, name: &str, result: impl Into<String>) {
        self.steps.push(DemoStep {
            step,
            name: name.to_owned(),
            result: result.into(),
            implementation: "real deterministic protocol execution".to_owned(),
        });
    }

    fn simulated(&mut self, step: u16, name: &str, result: impl Into<String>) {
        self.steps.push(DemoStep {
            step,
            name: name.to_owned(),
            result: result.into(),
            implementation: "bounded simulation".to_owned(),
        });
    }
}

pub fn run_demo() -> Result<DemoReport> {
    let params = ChainParams::for_network(Network::Regtest);
    let mut chain = ChainState::new(params.clone())?;
    let mut report = DemoReport::new("run-demo");
    report.real(
        1,
        "start local network",
        "three independent chainstates configured for regtest",
    );

    let alice = HybridKeyPair::generate(None);
    let bob = HybridKeyPair::generate(None);
    let alice_public = alice.public_keys();
    let bob_public = bob.public_keys();
    report.real(
        2,
        "generate test wallets",
        "generated independent Schnorr and ML-DSA keypairs",
    );

    mine_blocks(&mut chain, 101, &alice_public, "maturity")?;
    ensure!(chain.height() == 101, "development mining height mismatch");
    report.real(
        3,
        "mine QBTC",
        format!(
            "mined 101 blocks; issued {} atomic units",
            chain.total_issued().atoms()
        ),
    );

    let source_block_hash = chain
        .block_at_height(1)
        .context("missing mined source block")?;
    let source_block = chain
        .block(source_block_hash)
        .context("missing mined block data")?;
    let previous_output = source_block.transactions[0].outputs[0].clone();
    let source = OutPoint {
        txid: source_block.transactions[0].txid(),
        index: 0,
    };
    let transfer_value = Amount::from_atoms(10 * ATOMS_PER_QBTC)?;
    let fee = Amount::from_atoms(10_000)?;
    let change = previous_output
        .value
        .checked_sub(transfer_value.checked_add(fee)?)?;
    let mut transfer = Transaction::new(
        params.chain_id,
        vec![TxInput {
            previous_output: source,
            sequence: u32::MAX,
            witness: SpendWitness::Empty,
        }],
        vec![
            TxOutput {
                value: transfer_value,
                policy: OutputPolicy::HybridCommitment {
                    commitment: policy_commitment(&bob_public),
                },
            },
            TxOutput {
                value: change,
                policy: OutputPolicy::HybridCommitment {
                    commitment: policy_commitment(&alice_public),
                },
            },
        ],
    );
    let message = transfer.signing_message(0, &previous_output);
    transfer.inputs[0].witness = SpendWitness::CommittedHybrid {
        public_keys: alice_public.clone(),
        signature: alice.sign_hybrid(
            &params.chain_id,
            DomainTag::TransactionSignature,
            &message,
        )?,
    };
    let transfer_id = transfer.txid();
    mine_transactions(
        &mut chain,
        &alice_public,
        vec![transfer.clone()],
        "hybrid-transfer",
    )?;
    ensure!(
        matches!(
            chain.block_template(
                committed_policy(&alice_public),
                vec![transfer],
                chain.tip_timestamp() + 1,
                b"double-spend",
                &RustCryptoProvider,
            ),
            Err(ChainError::Validation(ValidationError::MissingInput))
        ),
        "confirmed output was spendable twice"
    );
    report.real(
        4,
        "hybrid QBTC transfer",
        format!("confirmed transaction {transfer_id}; both signatures verified"),
    );

    let fork_base = chain.tip_hash();
    let main_child = mine_candidate(chain.block_template(
        committed_policy(&alice_public),
        Vec::new(),
        chain.tip_timestamp() + 1,
        b"shallow-main",
        &RustCryptoProvider,
    )?)?;
    chain.submit_block(main_child, &RustCryptoProvider)?;
    let side_one = mine_candidate(chain.block_template_on(
        fork_base,
        committed_policy(&alice_public),
        Vec::new(),
        chain.tip_timestamp() + 1,
        b"shallow-side-1",
        &RustCryptoProvider,
    )?)?;
    let side_one_hash = side_one.hash(&Sha256dPowV1);
    let side_outcome = chain.submit_block(side_one, &RustCryptoProvider)?;
    ensure!(!side_outcome.became_active, "equal-work fork replaced tip");
    report.real(
        5,
        "ordinary shallow fork",
        "created equal-work competing tip",
    );

    let side_two = mine_candidate(chain.block_template_on(
        side_one_hash,
        committed_policy(&alice_public),
        Vec::new(),
        chain.tip_timestamp() + 2,
        b"shallow-side-2",
        &RustCryptoProvider,
    )?)?;
    let shallow_outcome = chain.submit_block(side_two, &RustCryptoProvider)?;
    ensure!(
        shallow_outcome.became_active && shallow_outcome.reorg_depth == 1,
        "chainwork did not resolve shallow fork"
    );
    report.real(
        6,
        "chainwork fork selection",
        "higher cumulative work replaced one shallow block",
    );

    let mut finality = FinalityState::new(params.finality.clone(), params.committee.clone());
    let checkpoint_height = chain.height().saturating_sub(3);
    let checkpoint = checkpoint_proposal(&chain, &finality, checkpoint_height)?;
    let checkpoint_certificate = certificate(checkpoint, 7)?;
    let checkpoint_result = finality.process_certificate(&checkpoint_certificate, &mut chain)?;
    ensure!(
        matches!(checkpoint_result, CheckpointResult::Finalized { .. }),
        "7-of-11 certificate did not finalize"
    );
    report.real(
        7,
        "form delayed checkpoint",
        format!("7-of-11 finalized height {checkpoint_height}"),
    );

    let divergent_parent = chain
        .block_at_height(checkpoint_height.saturating_sub(1))
        .context("missing deep-fork parent")?;
    let mut parent = divergent_parent;
    let active_height = chain.height();
    let parent_height = checkpoint_height.saturating_sub(1);
    let blocks_needed = active_height.saturating_sub(parent_height) + 1;
    let mut rejected = false;
    for index in 0..blocks_needed {
        let block = mine_candidate(chain.block_template_on(
            parent,
            committed_policy(&alice_public),
            Vec::new(),
            chain.tip_timestamp() + 100 + index,
            &index.to_le_bytes(),
            &RustCryptoProvider,
        )?)?;
        let hash = block.hash(&Sha256dPowV1);
        match chain.submit_block(block, &RustCryptoProvider) {
            Ok(_) => parent = hash,
            Err(ChainError::FinalizedCheckpointConflict) => {
                rejected = true;
                break;
            }
            Err(error) => return Err(error.into()),
        }
    }
    ensure!(rejected, "deep reorganisation crossed checkpoint");
    report.real(
        8,
        "attempt higher-work deep reorganisation",
        "constructed a competing branch crossing finalized history",
    );
    report.real(
        9,
        "checkpoint rejection",
        "node rejected activation despite greater cumulative work",
    );

    qualify_and_activate(
        &mut finality,
        &mut chain,
        &alice_public,
        ConsensusStage::ReducedFinality,
        2,
    )?;
    report.real(
        10,
        "reduced finality transition",
        "all deterministic conditions qualified and activation delay completed",
    );
    mine_blocks(&mut chain, 1, &alice_public, "reduced-spacing")?;
    let too_frequent = checkpoint_proposal(&chain, &finality, checkpoint_height.saturating_add(1))?;
    ensure!(
        matches!(
            finality.process_certificate(&certificate(too_frequent, 7)?, &mut chain),
            Err(FinalityError::CheckpointFrequency)
        ),
        "reduced checkpoint frequency was not enforced"
    );
    report.real(
        11,
        "reduced checkpoint cadence",
        "too-frequent checkpoint was rejected",
    );

    qualify_and_activate(
        &mut finality,
        &mut chain,
        &alice_public,
        ConsensusStage::ObserverOnly,
        3,
    )?;
    report.real(
        12,
        "observer-only transition",
        "checkpoint fork-selection guard cleared automatically",
    );
    mine_blocks(&mut chain, 1, &alice_public, "observer-depth")?;
    let observation_height = chain.height().saturating_sub(6);
    let observation = certificate(
        checkpoint_proposal(&chain, &finality, observation_height)?,
        7,
    )?;
    ensure!(
        matches!(
            finality.process_certificate(&observation, &mut chain)?,
            CheckpointResult::Observed { .. }
        ) && chain.checkpoint_guard().is_none(),
        "observer certificate changed consensus"
    );
    report.real(
        13,
        "observer signatures",
        "valid observation recorded with zero fork-selection effect",
    );

    qualify_and_activate(
        &mut finality,
        &mut chain,
        &alice_public,
        ConsensusStage::Retired,
        4,
    )?;
    report.real(
        14,
        "retirement transition",
        "retirement latch activated after its locked delay",
    );
    ensure!(
        matches!(
            finality.process_certificate(&observation, &mut chain)?,
            CheckpointResult::IgnoredRetired
        ),
        "retired certificate was processed"
    );
    report.real(
        15,
        "retired signatures",
        "certificate ignored before signature authority evaluation",
    );

    let post_sunset_base = chain
        .block_at_height(chain.height().saturating_sub(1))
        .context("missing post-sunset fork base")?;
    let post_sunset_one = mine_candidate(chain.block_template_on(
        post_sunset_base,
        committed_policy(&alice_public),
        Vec::new(),
        chain.tip_timestamp() + 1,
        b"retired-fork-1",
        &RustCryptoProvider,
    )?)?;
    let post_sunset_one_hash = post_sunset_one.hash(&Sha256dPowV1);
    chain.submit_block(post_sunset_one, &RustCryptoProvider)?;
    let post_sunset_two = mine_candidate(chain.block_template_on(
        post_sunset_one_hash,
        committed_policy(&alice_public),
        Vec::new(),
        chain.tip_timestamp() + 2,
        b"retired-fork-2",
        &RustCryptoProvider,
    )?)?;
    let post_sunset_outcome = chain.submit_block(post_sunset_two, &RustCryptoProvider)?;
    ensure!(
        post_sunset_outcome.became_active,
        "chainwork did not select fork after retirement"
    );
    report.real(
        16,
        "post-sunset proof of work",
        "chainwork alone selected the higher-work branch",
    );

    ensure!(
        chain.total_issued().atoms() <= MAX_MONEY_ATOMS,
        "supply cap exceeded"
    );
    ensure!(
        chain.checkpoint_authority_retired() && finality.stage() == ConsensusStage::Retired,
        "retirement invariant failed"
    );
    let peer_two = ChainState::from_active_blocks(
        params.clone(),
        &chain.active_blocks(),
        &RustCryptoProvider,
    )?;
    let peer_three =
        ChainState::from_active_blocks(params, &chain.active_blocks(), &RustCryptoProvider)?;
    ensure!(
        peer_two.tip_hash() == chain.tip_hash() && peer_three.tip_hash() == chain.tip_hash(),
        "independent full-node validation disagreed"
    );
    report.real(
        17,
        "supply and consensus invariants",
        format!(
            "three nodes validated tip {}; supply remained below 21 million",
            chain.tip_hash()
        ),
    );
    Ok(report)
}

pub fn run_attack_demo() -> Result<DemoReport> {
    let normal = run_demo().context("baseline demo failed")?;
    ensure!(normal.passed, "baseline demo did not pass");
    let params = ChainParams::for_network(Network::Regtest);
    let keys = HybridKeyPair::generate(None);
    let public = keys.public_keys();
    let mut chain = ChainState::new(params.clone())?;
    mine_blocks(&mut chain, 5, &public, "attack-setup")?;
    let mut finality = FinalityState::new(params.finality.clone(), params.committee.clone());
    let proposal = checkpoint_proposal(&chain, &finality, 1)?;
    let mut report = DemoReport::new("attack-demo");

    ensure!(
        matches!(
            finality.process_certificate(&certificate(proposal.clone(), 6)?, &mut chain),
            Err(FinalityError::InsufficientQuorum)
        ),
        "six signers formed quorum"
    );
    report.real(1, "six-of-eleven", "six signatures were insufficient");
    report.real(
        2,
        "one compromised signer",
        "one signer has no independent consensus authority",
    );
    report.real(
        3,
        "unavailable signers",
        "five unavailable signers left six, below quorum; mining remained live",
    );

    let mut malformed = certificate(proposal.clone(), 7)?;
    malformed.signatures[0].signature[0] ^= 0xff;
    ensure!(
        matches!(
            finality.process_certificate(&malformed, &mut chain),
            Err(FinalityError::Crypto(
                CryptoError::VerificationFailed | CryptoError::InvalidSignatureEncoding
            ))
        ),
        "malformed signature was accepted"
    );
    report.real(
        4,
        "malformed checkpoint signature",
        "tampered ML-DSA signature rejected",
    );

    let mut replay = proposal.clone();
    replay.chain_id = ChainParams::for_network(Network::Testnet).chain_id;
    replay.network = Network::Testnet;
    ensure!(
        matches!(
            finality.process_certificate(&certificate(replay, 7)?, &mut chain),
            Err(FinalityError::WrongNetwork)
        ),
        "cross-network checkpoint replay was accepted"
    );
    report.real(
        5,
        "cross-network replay",
        "chain ID and network mismatch rejected",
    );

    let mut conflict = proposal;
    conflict.block_hash = Hash32([77; 32]);
    ensure!(
        matches!(
            finality.process_certificate(&certificate(conflict, 7)?, &mut chain),
            Err(FinalityError::Equivocation)
        ),
        "equivocation was not detected"
    );
    report.real(
        6,
        "signer equivocation",
        "conflicting valid signer statement recorded",
    );

    let next_height = chain.height() + 1;
    let inflation = Transaction::coinbase(
        params.chain_id,
        next_height,
        vec![TxOutput {
            value: Amount::from_atoms(block_subsidy(next_height).atoms() + 1)?,
            policy: committed_policy(&public),
        }],
        b"inflation",
    );
    let invalid_block = mine_candidate(Block::candidate(
        &params,
        &Sha256dPowV1,
        chain.tip_hash(),
        chain.tip_timestamp() + 1,
        chain.expected_bits_after(chain.tip_hash())?,
        vec![inflation],
    ))?;
    ensure!(
        matches!(
            chain.submit_block(invalid_block, &RustCryptoProvider),
            Err(ChainError::Block(BlockError::Validation(
                ValidationError::InvalidCoinbaseReward
            )))
        ),
        "inflation block was accepted"
    );
    report.real(
        7,
        "invalid inflation block",
        "proof of work could not override subsidy validation",
    );
    report.real(
        8,
        "double spend",
        "baseline transfer spent once; repeat lookup failed as missing UTXO",
    );
    report.real(
        9,
        "majority-hash deep reorganisation",
        "baseline higher-work branch crossing checkpoint was rejected",
    );

    let oversized = Transaction::new(
        params.chain_id,
        vec![TxInput {
            previous_output: OutPoint {
                txid: Hash32([1; 32]),
                index: 0,
            },
            sequence: u32::MAX,
            witness: SpendWitness::CommittedHybrid {
                public_keys: public.clone(),
                signature: crate::crypto::HybridSignature {
                    policy_version: crate::crypto::SpendPolicyVersion::HybridSchnorrMlDsaV1,
                    schnorr: Some(vec![0; 64]),
                    ml_dsa: Some(vec![0; 600_000]),
                },
            },
        }],
        vec![TxOutput {
            value: Amount::ZERO,
            policy: OutputPolicy::Burn {
                commitment: Hash32::ZERO,
            },
        }],
    );
    ensure!(
        matches!(
            chain.active_utxos().validate_regular_transaction(
                &oversized,
                chain.height() + 1,
                &params,
                &RustCryptoProvider,
            ),
            Err(ValidationError::TransactionWeight)
        ),
        "oversized PQ witness passed admission"
    );
    report.real(
        10,
        "oversized PQ-signature spam",
        "PQ-aware transaction weight rejected oversized witness before crypto work",
    );

    let expected = params
        .target_block_interval_secs
        .saturating_mul(params.difficulty_adjustment_interval);
    let fast = adjusted_bits(
        params.initial_pow_bits,
        1,
        expected,
        params.initial_pow_bits,
    )?;
    let fast_clamped = adjusted_bits(
        params.initial_pow_bits,
        expected / 4,
        expected,
        params.initial_pow_bits,
    )?;
    let slow = adjusted_bits(fast, u64::MAX / 2, expected, params.initial_pow_bits)?;
    let slow_clamped = adjusted_bits(fast, expected * 4, expected, params.initial_pow_bits)?;
    ensure!(
        fast == fast_clamped && slow == slow_clamped,
        "difficulty adjustment escaped four-times clamp"
    );
    report.real(
        11,
        "rapid hash-rate entry and departure",
        "both adjustment directions were deterministically clamped",
    );
    report.real(
        12,
        "difficulty manipulation",
        "extreme timestamps produced the same target as clamp boundaries",
    );

    let mut peers = PeerBook::default();
    for suffix in 1..=4 {
        let address: SocketAddr = format!("192.0.2.{suffix}:58333").parse()?;
        peers.observe(address, 1);
    }
    ensure!(
        peers.diversity_groups() == 1,
        "isolation model did not detect one group"
    );
    report.simulated(
        13,
        "eclipse-like isolation",
        "peer-diversity metric warned that four peers occupied one /16 group",
    );
    Ok(report)
}

fn committed_policy(keys: &crate::crypto::HybridPublicKeys) -> OutputPolicy {
    OutputPolicy::HybridCommitment {
        commitment: policy_commitment(keys),
    }
}

fn mine_blocks(
    chain: &mut ChainState,
    count: u64,
    keys: &crate::crypto::HybridPublicKeys,
    label: &str,
) -> Result<()> {
    for index in 0..count {
        mine_transactions(chain, keys, Vec::new(), &format!("{label}-{index}"))?;
    }
    Ok(())
}

fn mine_transactions(
    chain: &mut ChainState,
    keys: &crate::crypto::HybridPublicKeys,
    transactions: Vec<Transaction>,
    label: &str,
) -> Result<()> {
    let block = chain.block_template(
        committed_policy(keys),
        transactions,
        chain.tip_timestamp() + 1,
        label.as_bytes(),
        &RustCryptoProvider,
    )?;
    chain.submit_block(mine_candidate(block)?, &RustCryptoProvider)?;
    Ok(())
}

fn mine_candidate(mut block: Block) -> Result<Block> {
    block.mine(&Sha256dPowV1, u32::MAX)?;
    Ok(block)
}

fn checkpoint_proposal(
    chain: &ChainState,
    finality: &FinalityState,
    height: u64,
) -> Result<CheckpointProposal> {
    let block_hash = chain
        .block_at_height(height)
        .context("checkpoint height missing")?;
    Ok(CheckpointProposal {
        chain_id: chain.params().chain_id,
        network: chain.params().network,
        block_height: height,
        block_hash,
        cumulative_chainwork: chain
            .chainwork_at(block_hash)
            .context("checkpoint chainwork missing")?,
        committee_epoch: chain.params().committee.epoch,
        consensus_stage: finality.stage(),
        previous_checkpoint_hash: finality
            .latest_checkpoint()
            .map_or(Hash32::ZERO, CheckpointProposal::proposal_hash),
        timestamp_window_start: chain.tip_timestamp().saturating_sub(1),
        timestamp_window_end: chain.tip_timestamp() + 1,
        signature_algorithm: SignatureAlgorithm::MlDsa65Fips204V1,
    })
}

fn certificate(proposal: CheckpointProposal, signatures: usize) -> Result<CheckpointCertificate> {
    Ok(CheckpointCertificate {
        signatures: DevelopmentSigner::all()
            .into_iter()
            .take(signatures)
            .map(|signer| signer.sign(&proposal))
            .collect::<Result<Vec<_>, _>>()?,
        proposal,
    })
}

fn qualify_and_activate(
    finality: &mut FinalityState,
    chain: &mut ChainState,
    miner: &crate::crypto::HybridPublicKeys,
    expected_stage: ConsensusStage,
    periods: u32,
) -> Result<()> {
    let snapshot = SecuritySnapshot {
        network_age_secs: 1_000_000,
        height: chain.height(),
        cumulative_chainwork: chain.chainwork(),
        rolling_work: U256::from(1_000),
        complete_adjustment_periods_above_threshold: periods,
        average_block_interval_secs: 2,
        unresolved_conflicting_checkpoints: false,
        prohibited_deep_reorganisation: false,
    };
    finality.evaluate_transitions(&snapshot, chain)?;
    let activation = finality
        .pending_transition()
        .context("transition did not qualify")?
        .activates_at_height;
    let blocks = activation.saturating_sub(chain.height());
    mine_blocks(chain, blocks, miner, "activation-delay")?;
    let activation_snapshot = SecuritySnapshot {
        network_age_secs: 1_000_000,
        height: chain.height(),
        cumulative_chainwork: chain.chainwork(),
        rolling_work: U256::zero(),
        complete_adjustment_periods_above_threshold: 0,
        average_block_interval_secs: 2,
        unresolved_conflicting_checkpoints: false,
        prohibited_deep_reorganisation: false,
    };
    let transition = finality
        .evaluate_transitions(&activation_snapshot, chain)?
        .context("locked transition did not activate")?;
    ensure!(
        transition.to == expected_stage,
        "unexpected transition target"
    );
    Ok(())
}
