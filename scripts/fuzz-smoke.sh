#!/usr/bin/env bash
set -euo pipefail

SECONDS_TO_RUN="${1:-60}"
cargo +nightly-2026-07-20 fuzz run transaction_decode -- \
  -max_total_time="$SECONDS_TO_RUN" \
  -timeout=10
