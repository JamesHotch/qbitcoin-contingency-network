#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUILD_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/qbtc-repro.XXXXXX")"
trap 'rm -rf -- "$BUILD_ROOT"' EXIT

cd "$ROOT_DIR"
export CARGO_INCREMENTAL=0
export SOURCE_DATE_EPOCH="${SOURCE_DATE_EPOCH:-$(git log -1 --format=%ct)}"

BINARIES=(
  qbtc-node
  qbtc-wallet
  qbtc-wallet-service
  qbtc-miner
  qbtc-signer
  qbtc-demo
  qbtc-bench
)

for pass in 1 2; do
  target="$BUILD_ROOT/target-$pass"
  export RUSTFLAGS="--remap-path-prefix=$ROOT_DIR=/qbtc-source --remap-path-prefix=$target=/qbtc-target"
  CARGO_TARGET_DIR="$BUILD_ROOT/target-$pass" \
    cargo build --locked --release --bins
done

for binary in "${BINARIES[@]}"; do
  first="$BUILD_ROOT/target-1/release/$binary"
  second="$BUILD_ROOT/target-2/release/$binary"
  first_hash="$(sha256sum "$first" | cut -d ' ' -f 1)"
  second_hash="$(sha256sum "$second" | cut -d ' ' -f 1)"
  if [[ "$first_hash" != "$second_hash" ]]; then
    printf 'NON-REPRODUCIBLE %s\nfirst:  %s\nsecond: %s\n' \
      "$binary" "$first_hash" "$second_hash" >&2
    exit 1
  fi
  printf '%s  %s\n' "$first_hash" "$binary"
done

printf 'Reproducibility smoke check passed for commit %s with %s.\n' \
  "$(git rev-parse HEAD)" "$(rustc --version)"
