# Testing and Demonstrations

## Test Matrix

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets
cargo build --locked --release
cargo check --manifest-path fuzz/Cargo.toml
```

Coverage includes amounts and supply, canonical encoding, chain/network replay,
hybrid signature requirements, recovery delays, UTXO double spends, coinbase
rewards, proof of work, reorganization, checkpoint depth/quorum/equivocation,
stage progression, retirement, wallet encryption/restore, P2P bounds, and
chainstate manifest recovery.

Property tests vary canonical transaction amounts and checked arithmetic. The
fuzz harness asserts that any accepted transaction encoding round-trips
byte-for-byte.

## Progression Demo

```bash
cargo run --release --bin qbtc-demo -- --mode run
```

The 17 steps execute real deterministic protocol code in one process:

1. Configure three chainstates.
2. Generate hybrid wallets.
3. Mine through maturity.
4. Transfer with both signatures.
5. Create a shallow fork.
6. Select by chainwork.
7. Finalize a delayed 7-of-11 checkpoint.
8. Build a deeper higher-work fork.
9. Reject activation across the checkpoint.
10. Qualify and activate reduced mode.
11. Reject too-frequent reduced checkpoints.
12. Qualify and activate observer mode.
13. Record an observation with no fork-choice effect.
14. Qualify and activate retirement.
15. Ignore retired signatures.
16. Select post-sunset forks only by work.
17. Revalidate on three chainstates and check supply/retirement invariants.

The three chainstates are independent validators but not communicating network
processes.

## Attack Demo

```bash
cargo run --release --bin qbtc-demo -- --mode attack
```

Real protocol checks cover insufficient quorum, one compromised signer,
unavailable signers, malformed ML-DSA, cross-network replay, equivocation,
inflation, double spend, majority-hash deep reorganization, oversized PQ
witness, rapid hash-rate changes, and difficulty manipulation. Eclipse-like
isolation is the only bounded simulation; it verifies the peer-diversity
warning rather than creating a real network partition.

## Fuzzing

With nightly Rust and `cargo-fuzz`:

```bash
cargo install cargo-fuzz --locked
cargo +nightly fuzz run transaction_decode -- -max_total_time=60
```

Long-duration corpora, differential parsers, block/P2P/wallet targets, and
coverage reporting remain future work.

## Interpretation

Passing tests show that tested behavior matched expectations on one toolchain
and host. They do not establish absence of consensus, cryptographic, side
channel, persistence, networking, or operational vulnerabilities.
