# Building QBitCoin

## Supported Research Toolchain

- Rust 1.97.1 with `rustfmt` and `clippy`.
- Cargo in locked mode.
- Docker Engine 26+ and Docker Compose v2 for containers.
- Linux is the CI and container target; Windows development is supported for
  native Rust commands.

No CMake or C++ compiler is required because this repository uses the Rust
fallback rather than forking Bitcoin Core.

## Native Build

```bash
rustup show
cargo build --locked --release
```

Binaries appear under `target/release`:

```text
qbtc-node
qbtc-wallet
qbtc-wallet-service
qbtc-miner
qbtc-signer
qbtc-demo
qbtc-bench
```

## Quality Gates

```bash
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets
cargo doc --locked --no-deps
```

## Container Build

```bash
docker build --tag qbitcoin-devnet:local .
docker compose config --quiet
```

The final image runs as unprivileged user ID 10001.

## Windows

Use PowerShell for Cargo commands. The `.sh` development scripts require Git
Bash or WSL. Docker Desktop must have a healthy Linux engine; Compose syntax
validation alone does not prove containers can start.

## Mainnet

Normal builds omit `ENABLE_UNAUDITED_MAINNET` and reject mainnet. Do not set the
flag for ordinary development. Even a manually flagged build cannot start with
the intentionally unset mainnet finality parameters.
