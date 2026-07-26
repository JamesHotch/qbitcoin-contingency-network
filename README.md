EXPERIMENTAL POST-QUANTUM BITCOIN-STYLE TESTNET

QBitCoin is unaudited research software. It is not production-ready,
not investment-ready, and must not be described as quantum-proof.
Do not use it to hold real financial value.

# QBitCoin Contingency Network

[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Mainnet: disabled](https://img.shields.io/badge/mainnet-disabled-red.svg)](MAINNET_READINESS.md)

QBitCoin (QBTC) is a Rust research implementation of a Bitcoin-inspired UTXO
blockchain. It asks whether Bitcoin-style digital scarcity and proof of work
can be combined with hybrid post-quantum transaction authorization and a
temporary, progressively retiring checkpoint layer during a low-hash-rate
bootstrap period.

This repository uses the permitted Rust fallback, not a Bitcoin Core fork. It
does not claim Bitcoin Core compatibility or equivalent maturity. The ticker is
an experimental network label, not a representation of value.

## Security Boundary

- Wallet owners sign their own transactions.
- Full nodes independently validate every transaction and block.
- Miners construct valid candidates and perform SHA-256d proof of work.
- The temporary committee can checkpoint only an already-valid, sufficiently
  deep block from the active chain.
- Committee signatures cannot mint coins, move user funds, freeze addresses,
  reverse one payment, change balances, alter monetary rules, or make invalid
  data valid.
- Before retirement, an accepted checkpoint prevents a reorganization that
  would replace that historical block. Shallow forks still use chainwork.
- In `OBSERVER_ONLY`, committee messages are informational.
- In `RETIRED`, committee messages have permanently zero consensus authority.
- No premine, ICO, fiat integration, BTC backing, redemption promise, arbitrary
  mint RPC, or permanent founder transaction tax exists.

Checkpoint finality deliberately introduces trust. Eleven containers on one
computer are still one operational failure domain and provide no organizational
independence.

## Implemented

- Fixed 21,000,000 QBTC cap, 100,000,000 atomic units per QBTC, 50 QBTC initial
  subsidy, 210,000-block halvings, coinbase maturity, and overflow-safe integer
  accounting.
- Deterministic UTXO transactions, canonical decoding, witness-separated
  `txid`/`wtxid`, fees, change, lock time, relative timelocks, and replay
  separation through unique chain IDs.
- Versioned BIP340 Schnorr plus FIPS 204 ML-DSA-65 normal spends using strict
  `Schnorr AND ML-DSA`, with optional delayed FIPS 205 SLH-DSA recovery.
- Versioned SHA-256d proof of work, compact targets, bounded difficulty
  adjustment, cumulative-chainwork fork selection, and Merkle commitments.
- Four forward-only stages: `BOOTSTRAP_FINALITY`, `REDUCED_FINALITY`,
  `OBSERVER_ONLY`, and `RETIRED`.
- Individual ML-DSA checkpoint signatures, 7-of-11 development quorum,
  historical-depth and spacing rules, equivocation evidence, deterministic
  transition qualification, activation delays, and irreversible retirement.
- Encrypted CLI wallet, REST node API, CPU development miner, authenticated
  local signer service, bounded P2P framing, mempool, persistent chainstate,
  crash recovery, explorer/dashboard page, and Prometheus metrics.
- Unit, integration, property, adversarial, multi-chainstate, wallet restore,
  crash recovery, reorganization, supply, encoding, replay, and cryptographic
  policy tests.
- Deterministic 17-step progression demo and 13-scenario attack demo.

## Deliberately Incomplete

- Mainnet committee keys, finality thresholds, chain parameters, treasury
  decision, and all review attestations are unset. Mainnet cannot start.
- The Docker nodes expose inbound P2P handlers but do not yet dial, discover, or
  synchronize with one another. The multi-node demo revalidates one chain using
  three independent in-process chainstates; it is not a WAN network test.
- Static seeds are parameterized, but DNS seed resolution, persisted address
  gossip, outbound `addnode`, and automatic header-first synchronization are not
  wired into the node process.
- The signer service supports authenticated local-development endpoints only.
  Distributed operator enrollment, hardware-backed keys, and committee key
  rotation are specified but not implemented.
- Treasury arithmetic is implemented and defaults off, but consensus activation
  intentionally rejects a nonzero treasury until a reviewed quorum output and
  complete spending rules are implemented.
- The web wallet is a read-only testnet wallet-status service. Faucet requests,
  GUI transaction creation, and a public faucet are not implemented.
- Mining-pool and Stratum behavior is a specification, not a running server.
- Mining concentration and stale-block metrics are placeholders pending
  coinbase-tag indexing and connected peers.
- The eclipse scenario is a bounded peer-diversity simulation. Other demo and
  attack steps execute real protocol code deterministically.
- No independent audit, formal verification, interoperability test, public
  long-running testnet, performance campaign, or side-channel review exists.

See [ROADMAP.md](ROADMAP.md), [THREAT_MODEL.md](THREAT_MODEL.md), and
[MAINNET_READINESS.md](MAINNET_READINESS.md) for the complete gap register.

## Quick Start

Requirements:

- Rust 1.97.1, installed automatically through `rust-toolchain.toml`.
- Docker Engine 26+ and Docker Compose v2 for the container topology.
- OpenSSL for automatic local development credential generation.

Build and test:

```bash
cargo build --locked --release
cargo test --locked --all-targets
cargo clippy --locked --all-targets --all-features -- -D warnings
```

Run a native regtest node:

```bash
cargo run --release --bin qbtc-node -- \
  --chain regtest \
  --data-dir data/node \
  --rpc-bind 127.0.0.1:58332 \
  --p2p-bind 127.0.0.1:58333
```

Create an encrypted development wallet and mine one block:

```bash
export QBTC_WALLET_PASSWORD='use-a-random-local-password'
cargo run --release --bin qbtc-wallet -- create \
  --wallet wallets/dev.qbtc \
  --recovery-delay 144
cargo run --release --bin qbtc-miner -- \
  --chain regtest \
  --wallet wallets/dev.qbtc \
  --node-url http://127.0.0.1:58332 \
  --blocks 1
```

Wallet files contain valuable test secrets. They are ignored by Git, but users
must still protect backups from malware and loss. ML-DSA and SLH-DSA material
makes backups substantially larger than conventional wallets.

## Docker Development Network

```bash
./scripts/start-devnet.sh
./scripts/run-demo.sh
./scripts/attack-demo.sh
./scripts/reset-devnet.sh
```

`start-devnet.sh` creates ignored local credentials with restrictive file
permissions, then starts three nodes, two miners, eleven signer containers, a
wallet service, an explorer, a dashboard, and Prometheus.

```text
DEVELOPMENT COMMITTEE ONLY:
All checkpoint signers share one operational failure domain.
This configuration provides no organisational decentralisation.
```

- Dashboard: <http://localhost:8080>
- Explorer: <http://localhost:8081>
- Wallet service: <http://localhost:8090>
- Node API: <http://localhost:58332/v1/status>
- Prometheus: <http://localhost:9090>

The demo scripts first start the container topology and then run deterministic
protocol scenarios through `qbtc-demo`. A healthy Docker daemon is required.
The simulations can also run without Docker:

```bash
cargo run --release --bin qbtc-demo -- --mode run
cargo run --release --bin qbtc-demo -- --mode attack
```

## Cryptographic Position

The most immediate quantum concern for a Bitcoin-style system is transaction
authorization: a sufficiently capable quantum adversary could target exposed
elliptic-curve public keys. Merely replacing a mining hash does not protect
signatures. Grover-style search also does not mean SHA-256 is currently broken.
QBitCoin therefore retains SHA-256d proof of work in version 0.1 and focuses its
experiment on authorization.

Normal spends require:

```text
valid_spend =
    valid_schnorr_signature
    AND
    valid_ml_dsa_signature
```

Hybrid authorization avoids depending exclusively on a newer primitive while
retaining a post-quantum component. NIST standardization does not guarantee that
a particular library, integration, wallet, random-number generator, build, or
deployment is secure. Explicit algorithm and policy identifiers are used
because cryptographic agility is essential if assumptions change.

Bitcoin may adopt its own post-quantum protections. QBitCoin is not objectively
more secure than Bitcoin overall and does not claim to be. See
[CRYPTOGRAPHY.md](CRYPTOGRAPHY.md).

## Consensus and Economics

Proof of work remains the block-production and post-sunset fork-choice
mechanism. Crossing a historical chainwork or difficulty threshold does not
guarantee future miner diversity, honest hash rate, hardware availability, or
security. Transition dates shown by any dashboard are projections, never
guarantees.

Scarcity rules do not create demand, liquidity, utility, price stability, or
market value. An open-source repository likewise does not make this project
investment-ready. A permanent founder transfer tax was rejected because it
would burden all transfers indefinitely, centralize benefit, and complicate
neutrality. The optional research treasury is subsidy-only, time-limited,
quorum-controlled by design, transparent, and disabled.

Non-protocol funding options include ordinary mining, hosted-node services,
optional APIs, pool operation, enterprise wallet software, donations, and
research grants. None is promised or built into ownership of QBTC. See
[ECONOMIC_MODEL.md](ECONOMIC_MODEL.md).

## API and Tools

- OpenAPI: <http://localhost:58332/openapi.json> or
  [docs/openapi.json](docs/openapi.json)
- Protocol diagrams: [docs/architecture.md](docs/architecture.md)
- P2P protocol: [docs/P2P.md](docs/P2P.md)
- Mining and pool integration: [docs/MINING.md](docs/MINING.md)
- Signer operations: [docs/SIGNER_OPERATIONS.md](docs/SIGNER_OPERATIONS.md)
- Testing and demos: [docs/TESTING.md](docs/TESTING.md)
- Reproducible builds: [docs/REPRODUCIBLE_BUILDS.md](docs/REPRODUCIBLE_BUILDS.md)

Run the single-host cryptographic cost probe:

```bash
cargo run --release --bin qbtc-bench -- --iterations 100
```

These results are engineering observations, not production parameter studies.

## Mainnet Refusal

`ENABLE_UNAUDITED_MAINNET` defaults to false. Setting it to `1` at compile time
emits a prominent warning. A mainnet start also requires
`--acknowledge-unaudited-mainnet`, rejects local committee mode, and still fails
because reviewed committee keys and transition thresholds are intentionally
unset. There is no supported mainnet launch path in version 0.1.

```bash
ENABLE_UNAUDITED_MAINNET=1 cargo build --release
./target/release/qbtc-node --chain main \
  --acknowledge-unaudited-mainnet
```

The command above demonstrates layered refusal; it is not a launch instruction.
No real mainnet committee private keys are generated or hard-coded.

## Why No Investment Claims

QBTC has no guaranteed exchange rate, BTC redemption, fiat link, return, or
market. Public promotion, sale, custody, brokerage, exchange, mining-pool, or
hosted-wallet activity may create separate legal, tax, consumer-protection,
sanctions, privacy, securities, commodities, money-transmission, or licensing
obligations. Obtain jurisdiction-specific professional advice before any such
activity. See [REGULATORY_BOUNDARIES.md](REGULATORY_BOUNDARIES.md).

## Contributing and Security

Read [CONTRIBUTING.md](CONTRIBUTING.md) before proposing consensus changes.
Report vulnerabilities privately using the process in [SECURITY.md](SECURITY.md).
Automated tests do not replace independent review.

## Licence

MIT. See [LICENSE](LICENSE). Third-party dependencies retain their own licences.
