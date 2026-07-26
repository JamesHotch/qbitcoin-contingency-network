# Threat Model

## Status and Assumptions

This model covers the version 0.1 research prototype. It assumes the Rust
compiler and dependencies behave as intended, users obtain authentic binaries,
and local operating systems can protect process memory and encrypted files.
Those assumptions can fail.

Protected assets include authorization keys, UTXO ownership, monetary supply,
valid history, checkpoint retirement, network availability, wallet metadata,
and release integrity.

## Adversaries

- Remote peers sending malformed, oversized, replayed, or resource-intensive
  messages.
- Miners with minority or majority hash rate attempting reorganizations,
  censorship, timestamp distortion, or difficulty manipulation.
- Up to six unavailable or malicious development checkpoint signers.
- A quorum of malicious signers attempting to constrain valid history.
- Malware or an operator with access to wallet, signer, node, or build hosts.
- Supply-chain attackers compromising dependencies, CI, release keys, or
  maintainers.
- Future cryptanalytic or quantum advances affecting one or more primitives.
- Market promoters misrepresenting research software as an investment.

## Security Properties

| Property | Mechanism | Residual risk |
| --- | --- | --- |
| No arbitrary issuance | Subsidy schedule, checked amounts, UTXO validation | Consensus bug or implementation differential |
| User-only spending | Schnorr AND ML-DSA witness | Key theft, side channels, both algorithms failing, wallet compromise |
| Validity before work | Full block validation before fork choice | Parser or state-machine bug |
| Deep reorg protection | Historical 7-of-11 checkpoint guard | Quorum compromise, liveness loss, state persistence gap |
| Committee sunset | Forward-only stage and retirement latch | Finality is not yet persisted by node storage |
| Replay resistance | Chain ID, network magic, address HRP, domain tags | Incorrect integration or future version collision |
| Resource limits | Frame, weight, block, transaction, and signature-cost caps | Limits are not empirically production-tuned |
| Wallet confidentiality | Argon2id and XChaCha20-Poly1305 | Weak password, malware, backups, metadata, memory capture |

## Consensus Attacks

### Invalid Inflation or Signatures

Work and committee signatures do not bypass transaction or block validation.
Coinbase value, total issued value, duplicate inputs, input existence, maturity,
fees, encoding, and hybrid signatures are checked on each candidate branch.
Regression tests exercise higher-work inflation and committee-signed invalid
history.

Residual risk: the implementation has not been independently reviewed or
formally verified. A shared bug across nodes remains consensus.

### Majority Hash Rate

Before observer mode, a majority-hash branch cannot replace a finalized block.
It can still reorganize uncheckpointed history, censor recent transactions, and
delay progress. After retirement, a sufficiently higher-work valid branch can
replace any history according to normal fork choice.

A recorded historical work threshold says nothing certain about future miner
ownership or hash rate. Rapid entry and departure can still harm liveness and
confirmation reliability.

### Checkpoint Quorum

Six of eleven cannot finalize. Seven valid signers can protect only an eligible
active-chain block. A malicious quorum can choose between eligible valid
histories or withhold checkpoints, creating trust and coordination risk. It
cannot create validity or prevent deterministic retirement in the protocol
model.

The local development keys are deterministically derivable and all services
share a host, credentials, network, and administrator. They provide no security
against a real operator compromise.

### State Restart

Block chainstate is persisted through alternating manifests. Finality stage,
pending transition, latest checkpoint, equivocation journal, and the retirement
latch are currently in memory only. Restart can therefore discard checkpoint
protection or stage history. This is a critical known vulnerability and a
mainnet blocker.

## Cryptographic Attacks

Normal authorization requires both BIP340 Schnorr and ML-DSA-65. This helps if
only one assumption fails, but composition can introduce new encoding,
domain-separation, key-generation, side-channel, or implementation errors.
FIPS 204 and FIPS 205 standardization does not certify this integration.

SLH-DSA recovery is optional and delayed. Compromise of a recovery key becomes
effective after its relative timelock; loss makes that path unavailable.
Recovery and normal keys stored in one encrypted wallet are not operationally
independent.

The provider uses RustCrypto-family crates and forbids application unsafe code,
but dependencies may contain unsafe code. There is no constant-time or
side-channel audit.

## Network Attacks

Framing checks magic, payload length, checksum, canonical JSON, header count,
and timeouts. Handshakes commit to chain and domain-separated network identity.
Peer scores cause cooldown after repeated misconduct. Diversity metrics group
IPv4 peers by /16 and IPv6 peers by the first four bytes.

Critical gaps:

- The node does not dial peers or run automatic header synchronization.
- No DNS seed resolver, address gossip, NAT traversal, feeler connection,
  anchor-peer, or eviction strategy exists.
- Peer cooldowns and address records are in memory.
- CORS is permissive and the REST API has no authentication or TLS.
- Eclipse mitigation is only a diversity metric and bounded simulation.
- P2P messages use JSON, not a hardened compact binary protocol.

Bind the REST API to loopback or a trusted private network only.

## Denial of Service

Large post-quantum witnesses create bandwidth, storage, CPU, and propagation
pressure. Admission checks weight before signature verification and caps HTTP
bodies at 9 MiB. P2P frames and header batches are bounded.

Residual risks include expensive valid-signature floods, connection churn,
JSON amplification, disk growth, UTXO growth, and missing global/per-peer rate
limits. Benchmark values are single-host observations.

## Wallet and Signer Threats

Wallet encryption does not protect an unlocked process from malware. Passwords
passed on a command line may be observable to local users; prefer the hidden
environment variable only on a trusted host. Backups must preserve large key
material and be tested. The wallet currently lacks hardware isolation,
watch-only descriptors, multi-device recovery, and secure UI confirmation.

Signer bearer authentication is compared without early exit, but transport is
not TLS-protected by the service itself. Docker-network isolation is not a
replacement for mutually authenticated encrypted transport.

## Build and Governance Threats

Dependency pinning, audit tools, CodeQL, secret scanning, container scanning, and
reproducibility checks reduce risk but cannot prove source or binary safety.
One maintainer or one compromised GitHub account remains a governance risk.
Protected branches, hardware-backed maintainer accounts, independent builders,
and threshold release approval are required before meaningful distribution.

## Non-Technical Abuse

Scarcity language can be used to solicit investment or imply guaranteed value.
The project rejects token sales, fiat integration, BTC redemption, guaranteed
returns, and claims of quantum-proof or universally superior security.
Regulatory obligations depend on activity and jurisdiction, not repository
labels.
