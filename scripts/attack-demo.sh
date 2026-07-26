#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
"$ROOT_DIR/scripts/start-devnet.sh"
cd "$ROOT_DIR"
docker compose --profile tools run --rm demo --mode attack
