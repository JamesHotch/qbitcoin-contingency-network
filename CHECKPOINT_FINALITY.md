# Progressive Checkpoint Finality

## Purpose

The committee is a temporary defense against deep proof-of-work reorganizations
while testnet hash rate is low. It is not a block producer, transaction signer,
administrator, mint, censor, recovery service, or validity oracle.

## Certificate Validation Order

A certificate is accepted only after the node confirms:

1. Authority is not retired.
2. Chain ID and network match.
3. Stage, committee epoch, and ML-DSA algorithm version match.
4. The previous-checkpoint commitment forms the expected audit chain.
5. Signer IDs are known and unique.
6. Every individual signature commits to its signer ID and verifies.
7. At least M signatures are valid.
8. The block hash is already on valid active-chain history.
9. Claimed cumulative chainwork matches that block.
10. Required depth, spacing, and timestamp window hold.

Only then can bootstrap or reduced mode install a fork-choice guard. Observer
mode records an observation without a guard. Retired mode ignores the message
before evaluating signature authority.

## Signed Fields

Each signer commits to chain ID, network, block height, block hash, cumulative
chainwork, committee epoch, consensus stage, previous checkpoint hash,
timestamp-window start and end, signer identifier, and signature algorithm
version.

Signing two block hashes for the same epoch and height records equivocation
evidence. Evidence recording is currently in memory and must become durable.

## Stages

| Stage | Checkpoint effect | Fork choice |
| --- | --- | --- |
| `BOOTSTRAP_FINALITY` | Eligible historical quorum certificates install a guard | Chainwork above the guard; no branch may replace it |
| `REDUCED_FINALITY` | Same narrow power, with greater depth and spacing | Almost all ordinary forks use chainwork |
| `OBSERVER_ONLY` | Signed observations only | Chainwork only |
| `RETIRED` | Messages ignored; APIs should be historical/read-only | Chainwork only, permanently |

Transitions can call only `next()` and no reverse edge exists. Observer
activation clears the existing guard. Retirement also sets a chainstate latch;
attempts to reinstall a guard return `CheckpointAuthorityRetired`.

## Transition Qualification

Every transition requires all of:

- Minimum network age.
- Minimum block height.
- Minimum cumulative chainwork.
- Minimum rolling work.
- Difficulty sustained for complete adjustment periods.
- Average block interval within lower and upper bounds.
- No unresolved conflicting checkpoint.
- No prohibited deep reorganization during qualification.
- A block-based activation delay after qualification.

Once a transition is pending, its activation is deterministic at the recorded
height and is not canceled by later short-term threshold decline. The committee
does not vote on thresholds or activation.

## Research Parameters

| Network/stage | Minimum depth | Minimum spacing |
| --- | ---: | ---: |
| Regtest bootstrap | 3 | 2 |
| Regtest reduced | 6 | 5 |
| Testnet bootstrap | 24 | 12 |
| Testnet reduced | 144 | 144 |
| Mainnet | unset | unset |

Regtest transitions qualify at heights 10, 20, and 30 with short three-block
activation delays and deliberately accelerated age/work conditions. Testnet
uses larger but still unreviewed research values. Mainnet has no transition
thresholds and startup fails.

## Signer Model

Development mode commits eleven deterministic ML-DSA public keys and requires
seven individual signatures. It does not use a threshold-signature invention.
All development private keys are derivable test material and must never be used
for value.

The local signer HTTP endpoint checks a bearer token and independently queries a
node for chain ID, stage, height, and block hash before signing. The service
does not provide TLS, durable anti-equivocation storage, hardware keys, or
distributed operator enrollment.

The planned distributed mode requires separate operators, endpoints, encrypted
keystores, mutually authenticated transport, independent full-node validation,
and hardware-backed storage where implementations permit.

## Rotation

Chain parameters reserve an epoch, maximum rotation count, and activation
delay. Rotation processing is not implemented. A future implementation may
change only signer public keys and quorum metadata after:

- M-of-N authorization from the existing committee.
- A long deterministic activation delay.
- Enforcement of a predetermined total rotation cap.
- A complete public audit record.

Rotation must not add consensus powers, change monetary rules, change transition
thresholds, delay retirement, or restore retired authority.

## Safety and Liveness

Committee downtime does not stop blocks, transactions, mining, or chainwork
selection, but it can prevent new checkpoints. Six signers cannot finalize; a
quorum can constrain valid history but cannot validate an invalid block.

Checkpoint trust creates risks of quorum compromise, coercion, coordinated
withholding, equivocation, operational centralization, and social confusion.
Progressive retirement reduces this authority but does not prove future mining
security.

## Critical Persistence Limitation

The running node currently persists active blocks and UTXOs, not finality stage,
checkpoint guard, pending transition, signer evidence, or the retirement latch.
It also does not yet derive security snapshots from live chain history.
Therefore the state machine is implemented and tested but not deployment-safe.
Durable, crash-consistent finality storage and restart tests are mandatory
before any public persistent testnet.
