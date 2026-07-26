EXPERIMENTAL POST-QUANTUM BITCOIN-STYLE TESTNET

QBitCoin is unaudited research software. It is not production-ready,
not investment-ready, and must not be described as quantum-proof.
Do not use it to hold real financial value.

# QBitCoin Contingency Network

QBitCoin (QBTC) is a research implementation of a Bitcoin-inspired UTXO
blockchain. It studies hybrid classical and post-quantum transaction
authorisation together with a temporary, progressively retiring checkpoint
layer for a low-hash-rate bootstrap period.

This repository uses the prompt's permitted Rust fallback. It is not Bitcoin
Core and does not claim Bitcoin Core compatibility. The implementation will
state which behaviours are implemented, simulated, or still blocked.

## Safety boundary

- Mainnet is disabled by default and requires both the build-time
  `ENABLE_UNAUDITED_MAINNET=1` gate and a separate runtime acknowledgement.
- No premine, ICO, fiat integration, BTC backing, redemption promise, arbitrary
  minting interface, or permanent founder tax is included.
- Checkpoint signers cannot authorise transactions, alter balances, censor
  payments, or make an invalid block valid.
- The local eleven-signer topology is a development demonstration with one
  operational failure domain. It is not organisational decentralisation.
- Standards-conforming cryptography does not imply a secure implementation.
  Independent cryptographic, consensus, wallet, and operational review remain
  mandatory before any valuable deployment.

## Current status

Active research implementation. See `ROADMAP.md` and `MAINNET_READINESS.md` as
they are added. Until the validation report in this README says otherwise,
assume features are incomplete.

## Licence

MIT. See `LICENSE`. Third-party dependencies retain their own licences.

