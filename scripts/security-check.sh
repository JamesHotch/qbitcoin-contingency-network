#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

for command in cargo-audit cargo-deny gitleaks; do
  if ! command -v "$command" >/dev/null 2>&1; then
    printf 'Missing %s; see docs/TESTING.md for pinned installation commands.\n' \
      "$command" >&2
    exit 1
  fi
done

cargo audit --deny warnings
cargo deny --locked check
gitleaks git --redact --no-banner --log-opts="--all"
