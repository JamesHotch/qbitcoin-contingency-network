# Roadmap

This roadmap is a research sequence, not a launch promise. Completion of one
phase does not imply security, value, or readiness for the next.

## 0.1 Research Prototype

- [x] Fixed-supply UTXO consensus and chainwork fork choice.
- [x] Hybrid Schnorr and ML-DSA authorization.
- [x] Optional delayed SLH-DSA recovery.
- [x] Progressive checkpoint authority and irreversible retirement.
- [x] Wallet, node, CPU miner, local signers, API, metrics, and dashboards.
- [x] Deterministic progression and adversarial demonstrations.
- [ ] Healthy-Docker-host validation of the complete Compose topology.

## 0.2 Network Interoperability

- [ ] Outbound peer manager with manual `addnode`.
- [ ] DNS and multiple static seed sources.
- [ ] Persisted address gossip and diversified connection buckets.
- [ ] Header-first initial block download and transaction/block relay inventory.
- [ ] Live three-node partition, recovery, stale-block, and reorganization tests.
- [ ] Protocol conformance corpus and wire-format version negotiation.

## 0.3 Signer Independence

- [ ] Encrypted nondeterministic signer keystores.
- [ ] Mutually authenticated operator transport.
- [ ] Independent-node validation requirements and anti-equivocation journal.
- [ ] Auditable M-of-N key rotation with delayed activation and hard rotation cap.
- [ ] Hardware-backed key integration where ML-DSA support exists.
- [ ] Geographic and organizational independence exercise.

## 0.4 Wallet and Mining

- [ ] Descriptor-like watch-only and offline signing flow.
- [ ] Coin discovery, rescan, history indexing, and explicit receive rotation.
- [ ] Testnet faucet with abuse controls.
- [ ] Accessible web transaction workflow without exposing secrets to the node.
- [ ] Stratum-compatible development server and pool integration tests.
- [ ] Propagation, stale-rate, signature-cost, and storage-growth benchmarks.

## 0.5 Assurance

- [ ] Independent cryptographic implementation review.
- [ ] Independent consensus and supply review.
- [ ] Independent wallet and storage review.
- [ ] Side-channel, fault-injection, sanitizer, and long-duration fuzz campaigns.
- [ ] Reproducible release artifacts verified by multiple maintainers.
- [ ] Public incident-response exercise and long-running valueless testnet.

## Mainnet

No mainnet milestone is scheduled. Every item in `MAINNET_READINESS.md` must be
resolved through public, independent review before even proposing one.
