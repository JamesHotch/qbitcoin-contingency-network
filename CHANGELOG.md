# Changelog

All notable research milestones are recorded here. This project does not yet
make compatibility or production-support promises.

## [Unreleased]

### Added

- Security automation, reproducible-build checks, and broader fuzzing remain
  planned for the next development milestone.

## [0.1.0] - 2026-07-26

### Added

- Bitcoin-style fixed-supply UTXO accounting and SHA-256d proof of work.
- Versioned hybrid Schnorr and ML-DSA transaction authorization.
- Optional relative-timelocked SLH-DSA recovery.
- Progressive four-stage checkpoint finality with an irreversible sunset.
- Encrypted CLI wallet, CPU miner, node REST/P2P services, and persistent
  chainstate.
- Eleven-container local signer topology, dashboards, explorer, and metrics.
- Deterministic progression and adversarial demonstrations.
- Unit, integration, property, fuzz-harness, and crash-recovery coverage.

### Security

- Mainnet defaults off and remains unconfigured.
- Treasury defaults off and nonzero activation is fail-closed.
- Local committee mode is prohibited on mainnet.

### Known Limitations

- No independent audit or production deployment.
- Outbound P2P discovery and live multi-node synchronization are incomplete.
- Distributed signer operations, rotation, faucet, and Stratum server are not
  implemented.
