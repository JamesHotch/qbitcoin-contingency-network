#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

docker compose down --volumes --remove-orphans
rm -f -- "$ROOT_DIR/.env.devnet"
printf 'Removed development containers, volumes, and generated local credentials.\n'

