# Node and Wallet APIs

The node serves a versioned REST surface. The canonical OpenAPI document is
`docs/openapi.json` and is served at `/openapi.json`.

## Node Endpoints

| Method | Path | Purpose |
| --- | --- | --- |
| GET | `/v1/status` | Tip, work, stage, checkpoint, peers, and transition status |
| GET | `/v1/blocks/{height}` | Active-chain block |
| GET | `/v1/mempool` | Accepted transaction IDs |
| POST | `/v1/transactions` | Signed transaction admission |
| POST | `/v1/mining/template` | Validated block candidate |
| POST | `/v1/blocks` | Mined block validation |
| POST | `/v1/checkpoints` | Certificate validation |
| GET | `/metrics` | Prometheus text |
| GET | `/` and `/explorer` | Read-only dashboard |

Error responses use `{"error":"message"}` with an appropriate HTTP status.
Request bodies are limited to 9 MiB.

## Security

The node API is unauthenticated, CORS-permissive, and unencrypted. Bind it to
loopback or a protected development network only. Never expose it directly to
the Internet.

The signer API is separate. It requires `Authorization: Bearer TOKEN`, but still
needs external TLS and network access controls.

## Projection Data

The status schema includes a projected transition date and warning. Version 0.1
returns no date because the live node does not yet calculate transition
snapshots. Any future estimate must remain explicitly non-guaranteed.

## Compatibility

Only `/v1` exists. No backward-compatibility guarantee is made during the
research phase. Consensus encodings are distinct from REST JSON and must not be
inferred from API field order.
