# Stratum-Compatible Research Sketch

This is not a deployed server or a finalized standard. It exists to make future
pool integration requirements reviewable.

## Transport

Use newline-delimited JSON-RPC over authenticated TLS for development. Production
work should evaluate Stratum V2 rather than designing another security
protocol. Worker authentication is pool accounting, not blockchain consensus.

## Template Fields

A job must commit to:

- Protocol and consensus versions.
- Chain ID and network.
- Previous block hash.
- Coinbase prefix/suffix and extra-nonce ranges.
- Ordered transaction IDs and Merkle branches.
- Timestamp constraints.
- Compact target and share target.
- Maximum block weight and signature cost.

## Required Methods

```text
mining.subscribe
mining.authorize
mining.configure
mining.notify
mining.set_difficulty
mining.submit
client.reconnect
```

## Safety Requirements

- Full block validation at pool and node boundaries.
- Bounded messages, requests, workers, and share rates.
- Unique job IDs and stale-job rejection.
- No remote key access to participant wallets.
- Explicit fee/coinbase policy.
- TLS identity and credential rotation.
- Metrics for stale shares, invalid shares, block candidates, and concentration.

No code in version 0.1 implements this protocol.
