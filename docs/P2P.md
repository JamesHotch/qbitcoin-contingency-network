# P2P Protocol

## Version 1 Frame

Each frame contains network magic, protocol version, message type, payload
length, checksum, and a canonical JSON payload. Readers apply a hard frame-size
limit before allocation, verify checksum and magic, reject unsupported
versions, bound header batches, and enforce read/write timeouts.

Messages:

- `Handshake`
- `Transaction`
- `Block`
- `GetHeaders`
- `Headers`
- `Ping`
- `Pong`

The handshake carries chain ID, network, version, nonce, user agent, start
height, timestamp, and a domain-separated authentication commitment. It
prevents accidental cross-network replay; it does not authenticate a human or
encrypt transport.

## Implemented Peer Controls

- Unique network magic and chain ID.
- Maximum message and header counts.
- Timeouts.
- Misbehavior scoring and one-hour cooldown at score `-100`.
- Approximate diversity groups: IPv4 `/16`, IPv6 first 32 bits.
- Prometheus diversity-group metric.
- Inbound transaction, block, header request, and ping handling.

## Missing Before Public Networking

- Outbound connection manager and `--addnode`.
- DNS seed lookup and multiple independently operated seed lists.
- Address gossip, persisted peer database, anchor peers, and feeler connections.
- Header locator validation, download scheduling, orphan handling, and initial
  block download.
- Inventory relay, compact blocks, transaction request tracking, and rebroadcast.
- Per-peer bandwidth/rate budgets, authenticated encryption, and ban persistence.
- Stronger eclipse controls, ASN diversity, proxy/Tor support, and NAT handling.

The current static seed is loopback only. It is not a bootstrap service. A real
network must use multiple optional discovery paths and prove that removal of the
original developer's infrastructure does not stop discovery.

## Manual Testing

Start an inbound listener:

```bash
cargo run --release --bin qbtc-node -- \
  --chain regtest \
  --rpc-bind 127.0.0.1:58332 \
  --p2p-bind 127.0.0.1:58333
```

The P2P API is a protocol-development surface, not an Internet-hardened service.
Keep it firewalled during version 0.1 testing.
