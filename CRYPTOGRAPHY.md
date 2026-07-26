# Cryptographic Architecture

## Warning

QBitCoin is unaudited and must not be called quantum-proof. NIST standards
specify algorithms; they do not certify this implementation, composition,
wallet, random-number generation, build, or operational environment.

## Provider Boundary

`SignatureProvider` is the narrow verification interface between consensus and
third-party cryptographic crates. Every algorithm has an explicit numeric
identifier:

| Identifier | Algorithm | Use |
| --- | --- | --- |
| 1 | BIP340 secp256k1 Schnorr | Classical normal-spend component |
| 2 | FIPS 204 ML-DSA-65 | Post-quantum normal-spend and checkpoint component |
| 3 | FIPS 205 SLH-DSA-SHAKE-128s | Optional delayed recovery |

Policy identifiers distinguish the hybrid normal path from the recovery path.
Unknown identifiers fail closed rather than falling back.

The implementation uses maintained Rust crates `k256`, `ml-dsa`, and `slh-dsa`.
Application code forbids unsafe Rust. This does not mean dependencies are free
from unsafe code or side channels.

## Hybrid Normal Spend

Version `0x0100` requires both:

```text
verify(BIP340, schnorr_public_key, schnorr_signature, digest)
AND
verify(ML-DSA-65, ml_dsa_public_key, ml_dsa_signature, digest, context)
```

A valid signature from only one component fails. Public keys may be committed
in the output and revealed in witness data. The policy commitment prevents
substitution.

Hybrid composition limits reliance on either one algorithm but adds code,
bandwidth, parsing, and operational complexity. It does not protect against
simultaneous key theft, common random-number failure, wallet compromise,
incorrect message construction, or vulnerabilities that affect both library
integrations.

## Recovery Spend

Version `0x0101` permits SLH-DSA-SHAKE-128s only if:

- The output committed to that recovery public key and delay.
- The input age is at least the relative timelock.
- The recovery signature verifies under the wallet-recovery domain.

Recovery keys should be independently generated and stored. The prototype
stores them in the same encrypted wallet envelope, so it does not yet provide
true operational independence.

## Domain Separation

Every signed or authenticated digest begins with the unique chain ID and uses a
tagged SHA3-256 commitment. Defined tags are:

```text
QBTC/TX-SIGNATURE/v1
QBTC/BLOCK-SIGNATURE/v1
QBTC/CHECKPOINT-SIGNATURE/v1
QBTC/WALLET-RECOVERY/v1
QBTC/NETWORK-HANDSHAKE/v1
QBTC/GOVERNANCE-MESSAGE/v1
```

ML-DSA and SLH-DSA also receive the tag as their context. Network magic, address
HRP, and chain ID provide additional network separation. Cross-domain and
cross-network tests intentionally fail.

## Sizes and Cost

| Material | Encoded bytes |
| --- | ---: |
| Schnorr public key | 32 |
| Schnorr signature | 64 |
| ML-DSA-65 public key | 1,952 |
| ML-DSA-65 signature | 3,309 |
| SLH-DSA-SHAKE-128s public key | 32 |
| SLH-DSA-SHAKE-128s signature | 7,856 |

Large signatures increase transaction bandwidth, disk growth, initial sync
time, mempool pressure, verification CPU, and block propagation delay. The
prototype charges witness-aware weight and signature cost and rejects oversized
transactions before expensive verification. Limits remain research values.

## Key Storage

Wallet secrets are encrypted with Argon2id-derived keys and
XChaCha20-Poly1305. Plaintext and derived-key buffers are zeroized where the
types permit. Secret key types do not implement `Debug`; raw secret export is
limited to encrypted-wallet serialization.

Limitations:

- An unlocked process remains exposed to malware and memory capture.
- Command-line passwords may leak through process inspection.
- No hardware wallet or hardware-backed ML-DSA integration exists.
- No side-channel, fault-injection, entropy-source, or zeroization audit exists.
- Backups are larger and loss of every authorization path is irreversible.

## Mining Hashes

Version 1 retains SHA-256d proof of work because it is mature and the immediate
experiment concerns transaction signatures. Quantum search does not mean
SHA-256 is currently broken. A versioned proof-of-work interface permits future
research into standard alternatives, but changing the mining hash alone would
not protect exposed signing keys.

## Agility and Upgrades

Algorithm identifiers make parsers explicit; they do not by themselves provide
a safe activation mechanism. A future algorithm change requires a public
consensus specification, test vectors, activation rules, downgrade analysis,
wallet migration, replay analysis, and independent review. Silent negotiation
or an administrator-selected algorithm is prohibited.
