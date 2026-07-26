# Contributing

Contributions are welcome to this research codebase. The highest priority is
making security assumptions and limitations easier to verify.

## Before Starting

1. Read `README.md`, `THREAT_MODEL.md`, `CRYPTOGRAPHY.md`, and
   `CHECKPOINT_FINALITY.md`.
2. Search existing issues and proposals.
3. Discuss consensus, monetary, cryptographic, network-format, or wallet-format
   changes before implementation.
4. Never include private keys, generated wallets, credentials, personal data,
   or mainnet launch material.

## Development

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets
cargo build --locked --release
```

Changes should be small, reviewable, deterministic, and covered by a regression
test. Avoid floating-point consensus logic, platform-dependent serialization,
unbounded allocations, unauthenticated secret operations, and new unsafe code.

## Consensus Change Checklist

- Specify activation and compatibility behavior.
- State exact byte encoding and domain-separation rules.
- Add positive, negative, replay, boundary, and adversarial tests.
- Prove that supply accounting remains bounded.
- Explain reorganization and checkpoint effects.
- Explain denial-of-service costs for larger messages or signatures.
- Update the threat model, whitepaper, readiness template, and changelog.
- Obtain independent review before treating parameters as final.

The checkpoint committee must never gain authority over transaction validity,
balances, supply, address policy, or stage-transition thresholds.

## Pull Requests

Describe the problem, security impact, test evidence, compatibility impact, and
known limitations. Keep generated artifacts out of Git. Passing automation does
not replace human or independent review.

## Conduct

Participation is governed by [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md).
Security findings must follow [SECURITY.md](SECURITY.md).
