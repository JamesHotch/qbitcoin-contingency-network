# Mining and Pool Integration

## Responsibility Boundary

Wallet owners sign spends. Miners select already-signed transactions, construct
coinbase transactions and block headers, and perform proof of work. Full nodes
independently reject invalid signatures, inflation, malformed encoding, or
incorrect work regardless of miner hash rate.

## Solo CPU Development Mining

Start a regtest node and create a wallet as shown in `README.md`, then:

```bash
cargo run --release --bin qbtc-miner -- \
  --chain regtest \
  --wallet wallets/dev.qbtc \
  --node-url http://127.0.0.1:58332 \
  --blocks 10
```

The miner requests `POST /v1/mining/template`, iterates the 32-bit nonce using
the version 1 SHA-256d provider, and submits to `POST /v1/blocks`.

The CPU miner is for easy test targets. It is neither optimized nor suitable
for valuable mining.

## Difficulty

Difficulty adjusts at the network interval using elapsed timestamps and is
clamped to a four-times change in either direction for one period. Work is
derived from the decoded target, and fork choice compares cumulative work.
Timestamp policy and adjustment behavior require much broader manipulation
analysis before production use.

## Pool Protocol Status

A Stratum-compatible server is not implemented. A future prototype should map:

- `mining.subscribe` to protocol/version negotiation.
- `mining.authorize` to worker accounting only, never consensus authority.
- `mining.notify` to a QBitCoin template, chain ID, previous hash, Merkle data,
  target, consensus version, and mutable extra nonce.
- `mining.submit` to share validation and full block submission.

See [STRATUM.md](STRATUM.md) for the non-normative message sketch.

## Monitoring

Node status exposes chain height, target bits, chainwork, and recent reorg depth.
Prometheus exports basic chain metrics. Stale-block rate, hash rate, and mining
concentration are not yet computed.

A future concentration estimate may group declared coinbase tags. Such an
estimate is imperfect because miners can omit, copy, split, or change tags and
pools do not necessarily represent hardware ownership.

Hash-rate concentration can alter or censor recent valid history. It cannot
make full nodes accept invalid supply or signature rules.
