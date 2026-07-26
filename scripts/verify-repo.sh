#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-targets
cargo build --locked --release --bins
cargo check --manifest-path fuzz/Cargo.toml

REMOVE_ENV=0
if [[ ! -f .env.devnet ]]; then
  cp .env.devnet.example .env.devnet
  REMOVE_ENV=1
fi
trap 'if [[ "$REMOVE_ENV" == "1" ]]; then rm -f -- .env.devnet; fi' EXIT
docker compose config --quiet

printf 'Repository verification passed. Docker runtime startup is a separate check.\n'
