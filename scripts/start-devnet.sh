#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

if [[ ! -f .env.devnet ]]; then
  umask 077
  WALLET_PASSWORD="$(openssl rand -hex 24)"
  SIGNER_TOKEN="$(openssl rand -hex 32)"
  {
    printf 'QBTC_WALLET_PASSWORD=%s\n' "$WALLET_PASSWORD"
    printf 'QBTC_SIGNER_AUTH_TOKEN=%s\n' "$SIGNER_TOKEN"
  } > .env.devnet
fi

cat <<'WARNING'
DEVELOPMENT COMMITTEE ONLY:
All checkpoint signers share one operational failure domain.
This configuration provides no organisational decentralisation.
WARNING

docker compose up --detach --build
printf 'Dashboard: http://localhost:8080\nExplorer: http://localhost:8081\nPrometheus: http://localhost:9090\n'

