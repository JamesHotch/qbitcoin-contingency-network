## Summary

Describe the problem and the smallest reviewable solution.

## Security and Consensus Impact

- [ ] No consensus or wire-format change.
- [ ] Supply and UTXO invariants remain enforced.
- [ ] Checkpoint authority is not expanded.
- [ ] Mainnet remains disabled and unconfigured.
- [ ] No secrets, private keys, wallets, or credentials are included.

Explain any checked item that does not apply.

## Verification

- [ ] `cargo fmt --all -- --check`
- [ ] `cargo clippy --locked --all-targets --all-features -- -D warnings`
- [ ] `cargo test --locked --all-targets`
- [ ] Adversarial or regression tests added where behavior changed.
- [ ] Documentation and threat model updated where assumptions changed.

## Limitations

State what remains unimplemented, simulated, unaudited, or unverified.
