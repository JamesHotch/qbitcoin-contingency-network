# Economic Model

## Units and Issuance

- Ticker: `QBTC`.
- Atomic precision: 100,000,000 units per QBTC.
- Maximum valid supply: 21,000,000 QBTC.
- Genesis allocation: zero.
- Initial subsidy: 50 QBTC.
- Halving interval: 210,000 blocks.
- Coinbase maturity: 100 blocks.
- Transaction fees: input value minus output value, paid to the block miner.

Consensus uses checked unsigned integers and no floating point. For every
non-coinbase transaction:

```text
sum(inputs) >= sum(outputs)
fee = sum(inputs) - sum(outputs)
```

For each block:

```text
coinbase_outputs <= subsidy(height) + transaction_fees
total_issued <= valid_cumulative_subsidy(height)
```

Rounding occurs only through integer division in the halving schedule. Once the
shifted subsidy reaches zero, no further subsidy is issued. No RPC, committee
message, administrator key, or genesis output can create additional supply.

## Fair-Launch Position

The design has no premine, hidden allocation, ICO function, BTC backing,
redemption promise, fiat peg, or guaranteed exchange rate. CPU regtest mining is
for development and does not establish a fair public launch. A real fair-launch
analysis would require public notice, finalized software, accessible mining,
multiple independent miners, anti-instamine review, and legal analysis. None has
occurred.

## Development Treasury

The code models a transparent, optional split of early block subsidy:

```text
treasury = subsidy * basis_points / 10,000
miner = subsidy - treasury
```

The parameters include enablement, basis points, fixed expiry height, and a
commitment to a quorum-controlled output. Defaults are disabled for every
network. Expiry is automatic and checkpoint signers cannot alter it.

Version 0.1 intentionally rejects a nonzero treasury during coinbase
application. Activation remains blocked until all of the following are public:

- Exact fraction and expiry.
- Identified recipients and conflicts of interest.
- Reviewed quorum output and spending policy.
- Key custody, replacement, loss, and audit procedures.
- On-chain reporting commitments.
- Independent economic and legal review.

Activating a treasury changes the economic design and requires broad public
review. It must never become a permanent transfer tax or personal founder key.

## Rejected Permanent Founder Tax

A percentage of every transfer was rejected because it would permanently burden
users, create a privileged beneficiary, complicate fungibility and wallet
behavior, and couple protocol neutrality to one party. Research funding should
be explicit, bounded, reviewable, and separate from ordinary ownership
transfers.

## Non-Protocol Funding

Possible business or grant activity outside consensus includes:

- Ordinary competitive mining.
- Hosted full-node operations.
- Optional paid APIs.
- Mining-pool operation.
- Enterprise wallet and integration software.
- Donations and research grants.

These activities confer no protocol privilege and are not promised. Custody,
exchange, pool, hosted wallet, sale, or payment activity can create separate
legal and consumer obligations.

## What Scarcity Does Not Mean

A 21 million cap does not guarantee demand, utility, security, liquidity,
exchange access, stable price, appreciation, mining profitability, or legal
status. QBitCoin has no represented market value. Open source improves
inspectability, not investment readiness.
