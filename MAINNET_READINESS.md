# Mainnet Readiness

## Current Decision: Blocked

There is no supported QBitCoin mainnet. Version 0.1 is unaudited testnet
research and must not hold real financial value. This file is a blocker
register, not a launch plan.

## Layered Refusal

1. `ENABLE_UNAUDITED_MAINNET` is absent by default, so normal builds reject
   `--chain main`.
2. Setting `ENABLE_UNAUDITED_MAINNET=1` prints a build warning.
3. The node separately requires `--acknowledge-unaudited-mainnet`.
4. Local committee mode is rejected for mainnet regardless of other flags.
5. Mainnet committee keys, quorum, checkpoint depths, transition thresholds,
   and review marker are unset, so startup still fails.

No real mainnet committee private keys exist in this repository.

## Mandatory Gates

Every gate below is unresolved:

- [ ] Final consensus specification.
- [ ] Independent cryptographic review.
- [ ] Independent consensus review.
- [ ] Independent wallet review.
- [ ] Long-running public testnet.
- [ ] Multiple independent node operators.
- [ ] Multiple independent miners.
- [ ] Independently held committee keys.
- [ ] Reproducible builds.
- [ ] Incident-response process.
- [ ] Final chain parameters.
- [ ] Final treasury decision.
- [ ] Legal review.
- [ ] Public risk disclosures.

Additional implementation blockers:

- [ ] Persist finality stage, checkpoint guard, retirement latch, pending
  transition, and equivocation evidence crash-consistently.
- [ ] Derive transition snapshots from validated on-chain history in the node.
- [ ] Implement outbound P2P, peer discovery, header sync, and live relay.
- [ ] Implement and exercise independent signer operation.
- [ ] Specify, implement, and test bounded committee rotation or remove it.
- [ ] Complete performance studies for PQ weight and verification limits.
- [ ] Harden API authentication, TLS deployment, rate limits, and permissions.
- [ ] Complete wallet indexing, recovery operations, and hardware isolation.
- [ ] Conduct sanitizer, fuzz, fault-injection, and long-duration tests.
- [ ] Run incident, partition, eclipse, quorum-loss, and recovery exercises.

## Parameters Requiring Independent Review

- Genesis timestamp, message, hash, proof-of-work target, and fair-launch process.
- Network magic, ports, address HRP, and unique chain ID.
- Target interval, adjustment period, timestamp policy, and adjustment clamp.
- Block, transaction, witness, and signature-verification limits.
- Coinbase maturity, relay fees, dust rules, and mempool eviction.
- Checkpoint quorum, initial public keys, depths, spacing, and timestamp window.
- Every transition age, height, work, rolling window, sustained period, interval
  bound, and activation delay.
- Rotation count, quorum, delay, audit format, and failure behavior.
- Treasury enablement, fraction, expiry, recipients, quorum output, custody,
  reporting, and legal implications.
- Seed list, peer diversity policy, protocol version, and upgrade rules.

The repository does not invent placeholder production-safe values. See
`mainnet-readiness.toml`; blank values and false attestations are deliberate.

## Required Independent Reports

Reports must identify the reviewed commit, methodology, scope exclusions,
findings, fixes, and retest status. At minimum:

- Cryptographic construction, library integration, domain separation,
  randomness, side channels, and wallet key lifecycle.
- Consensus encoding, UTXO state, issuance, work, difficulty, reorganization,
  finality, retirement, persistence, and upgrade behavior.
- Wallet encryption, backup, restore, recovery, transaction display, malware
  model, and hardware boundaries.
- Network parser, peer management, eclipse resistance, denial of service, API,
  containers, dependencies, and release pipeline.
- Economic and legal analysis of launch, mining, treasury, services,
  communications, custody, and distribution.

Automated tools and maintainer self-review cannot satisfy these gates.

## Operational Independence Evidence

Mainnet consideration requires multiple unaffiliated operators and miners,
independently administered signer keys, separate infrastructure providers and
jurisdictions where practical, monitored failover, public runbooks, and a
successful disappearance exercise in which the original developer and bootstrap
seed are removed without stopping validation or peer discovery.

## Approval Rules

No one maintainer may mark readiness complete. A future process must define
named reviewers, conflict disclosures, public evidence, a cooling-off period,
and user-adopted activation software. Committee signatures cannot approve
mainnet, alter this checklist, or bypass startup refusal.
