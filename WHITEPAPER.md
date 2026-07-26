# QBitCoin Contingency Network

## Research Whitepaper, Version 0.1

### Abstract

QBitCoin studies a narrow question: can a Bitcoin-style fixed-supply,
proof-of-work UTXO system use hybrid classical and post-quantum transaction
authorization while temporarily limiting deep reorganizations during a
low-hash-rate bootstrap period? The prototype combines BIP340 Schnorr and
FIPS 204 ML-DSA signatures with an optional delayed FIPS 205 SLH-DSA recovery
path. A four-stage checkpoint committee can initially protect only valid,
historical blocks, then automatically loses authority and is permanently
retired.

The project is unaudited testnet research. It is not Bitcoin Core, is not
quantum-proof, and makes no claim of market value or production safety.

## 1. Scope

QBitCoin reproduces selected Bitcoin architectural properties:

- Unspent transaction outputs rather than account balances.
- Fixed-point integer amounts and a 21 million unit cap.
- Coinbase issuance, halvings, maturity, and transaction fees.
- Block headers committing to previous history and a transaction Merkle root.
- SHA-256d proof of work and cumulative-chainwork fork selection.
- Independent full-node validation that work cannot override invalid rules.

It does not reproduce Bitcoin Core's decades of compatibility behavior,
networking, script system, wallet descriptors, pruning, compact blocks, peer
discovery, production database, operational hardening, or review history.

## 2. Roles

Wallet owners create authorization for specific UTXO spends. Full nodes verify
the transaction, UTXO, amount, encoding, signature, block, supply, and work
rules. Miners choose transactions and search for a valid nonce. Checkpoint
signers independently verify a historical active-chain block before signing a
proposal. These roles are intentionally separate.

A checkpoint is a fork-choice constraint, not a validity override. Validation
is performed before checkpoint eligibility. A certificate cannot create an
output, repair a bad signature, raise a subsidy, select another network, or
target an unknown block.

## 3. Transaction Authorization

Normal version 1 outputs bind a Schnorr public key, an ML-DSA-65 public key, and
optional recovery metadata into a SHA3-256 policy commitment. Spending reveals
the keys in segregated witness-style data and requires both signatures over a
chain-specific, domain-separated digest:

```text
normal_valid = schnorr_valid AND ml_dsa_valid
```

This is defense in depth, not a proof that the combined system is secure. The
Schnorr component preserves a mature classical assumption; ML-DSA provides the
post-quantum component. A missing or invalid component fails the spend.

An optional SLH-DSA-SHAKE-128s key can authorize recovery only after the output's
relative block delay. It must be independently derived and protected because
loss or compromise changes the long-term recovery risk.

## 4. Blocks and Work

The version 1 header commits to:

- Consensus and proof-of-work versions.
- Unique chain ID.
- Previous block hash.
- Transaction Merkle root.
- Timestamp.
- Compact difficulty target.
- Nonce.

The block hash and proof of work use SHA-256d. New domain-separated commitments
use SHA3-256. Replacing the mining hash alone would not protect transaction
authorization, and this design does not claim that SHA-256 is broken by quantum
computing.

Each valid branch accumulates target-derived work. The valid branch with the
greatest cumulative work becomes active unless doing so would replace an active
checkpoint while checkpoint authority remains enabled. Invalid branches never
participate in fork choice.

## 5. Monetary System

One QBTC is 100,000,000 atomic units. The initial subsidy is 50 QBTC and halves
every 210,000 blocks. Genesis allocates zero spendable units. Consensus checks
all arithmetic, transaction conservation, coinbase reward, maturity, and total
issuance without floating point.

The optional research treasury is not active. Its parameter model can split a
declared fraction of an early subsidy and expires at a fixed height. It cannot
tax transactions or be extended by checkpoint signers. Version 0.1 deliberately
fails closed if a nonzero treasury is requested because the quorum output and
spending lifecycle have not received public review.

Scarcity is a protocol constraint, not evidence of utility, liquidity, demand,
price, or investment merit.

## 6. Progressive Checkpoint Finality

The state machine permits only:

```text
BOOTSTRAP_FINALITY -> REDUCED_FINALITY -> OBSERVER_ONLY -> RETIRED
```

In bootstrap, a 7-of-11 development quorum can protect sufficiently deep,
already-valid history. In reduced mode, greater depth and spacing make that
protection less frequent. Observer mode accepts signed observations without
changing fork choice. Retirement ignores committee messages and sets a separate
chainstate latch that rejects any attempt to reinstall a checkpoint guard.

Qualification requires every deterministic condition: age, height, cumulative
work, rolling work, sustained adjustment periods, bounded average intervals, no
unresolved checkpoint conflict, no prohibited deep reorganization, and then an
activation delay. Once scheduled, later short-term threshold decline does not
cancel the transition. Committee approval is not part of transition
qualification.

These mechanics are implemented and tested in memory. Version 0.1 does not yet
derive transition snapshots in the running node or persist finality state across
restart. That is a critical blocker to any durable network deployment.

## 7. Committee Trust

Every signer uses an individual ML-DSA signature. The prototype does not invent
threshold ML-DSA. Signed data includes chain, network, height, block hash,
chainwork, epoch, stage, prior checkpoint, timestamp window, signer identity,
and algorithm version. Conflicting statements by one signer at the same epoch
and height produce equivocation evidence.

Committee compromise can constrain which valid history nodes accept before
observer mode. It cannot bypass ordinary validity rules. Committee outage does
not stop mining or transaction validation, but it can prevent new checkpoint
formation. Running all signers under one operator provides quorum mechanics
without organizational decentralization.

## 8. Retirement

Reduced authority is a consensus transition, not a committee promise. Observer
activation clears the checkpoint fork-choice guard. Retirement sets a permanent
latch and causes certificate processing to return an ignored result before
signature authority is considered. Restoring authority would require an
explicit future hard fork adopted by node users; no emergency switch exists.

After retirement, proof of work and cumulative chainwork alone choose among
valid branches. This does not eliminate majority-hash attacks or guarantee
future mining decentralization.

## 9. Engineering Limits

Post-quantum keys and signatures are large. ML-DSA-65 uses 1,952-byte public keys
and 3,309-byte signatures in this implementation; SLH-DSA-SHAKE-128s recovery
signatures are 7,856 bytes. QBitCoin therefore applies transaction weight,
block weight, and signature-verification cost limits before expensive
verification. The values are conservative testnet defaults, not production
parameters.

The current process has inbound P2P framing but no outbound peer lifecycle,
live chain synchronization, or distributed signer operation. Performance,
propagation, storage growth, side channels, and interoperability require
independent measurement.

## 10. Conclusion

The prototype demonstrates that these rules can be expressed and tested
together. It does not establish that they are economically desirable,
cryptographically safe in deployment, or superior to Bitcoin. Bitcoin may add
its own post-quantum protections. QBitCoin should be evaluated only as an
explicitly limited contingency-network experiment.
