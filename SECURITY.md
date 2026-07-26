# Security Policy

QBitCoin is unaudited experimental testnet software. Do not use it to hold real
financial value. A standards-conforming primitive or passing test suite is not
evidence that this protocol, implementation, wallet, or deployment is secure.

## Supported Versions

Only the latest commit on the default branch receives security fixes during the
research phase. There are no production-supported releases.

## Private Vulnerability Reporting

Do not open a public issue for a suspected vulnerability.

1. Use GitHub Private Vulnerability Reporting at
   <https://github.com/JamesHotch/qbitcoin-contingency-network/security/advisories/new>.
2. Include affected commit, network, prerequisites, impact, reproduction steps,
   and a minimal proof of concept where safe.
3. Remove private keys, wallet files, tokens, personal data, and third-party
   secrets from the report.
4. If GitHub private reporting is unavailable, open a public issue containing
   only the words "private security contact requested." Do not include technical
   details. A maintainer will establish a private channel.

The project will acknowledge a private report when a maintainer is available,
triage it, coordinate a fix and regression test, and publish an advisory after
affected users have a reasonable opportunity to update. Research-project
staffing means no response-time or remediation-time guarantee can be offered.

## Priority Areas

- Consensus divergence, inflation, double spend, or checkpoint bypass.
- Signature verification, domain separation, key handling, or wallet
  encryption failures.
- Mainnet gate bypass or committee authority after retirement.
- Remote code execution, memory exhaustion, parser differentials, or durable
  denial of service.
- Secret exposure in logs, builds, containers, APIs, or repository history.
- Reproducibility and software supply-chain compromise.

## Coordinated Disclosure

Please allow maintainers to investigate before publication. The project will
credit reporters who want attribution. Never access systems, funds, keys, or
data that you do not own or have explicit permission to test.

## Out of Scope

- Claims that quantum computers may someday threaten classical signatures,
  without an implementation-specific defect.
- Market price, liquidity, exchange listing, profitability, or investment loss.
- Mainnet availability; mainnet is intentionally disabled and unconfigured.
- Attacks requiring modification of a local development key that is explicitly
  public and deterministic.

## Release Security

No release should be described as audited or production-ready unless independent
reports are public and the mainnet readiness gates are separately satisfied.
Release artifacts require multiple-maintainer review and detached signatures as
described in [docs/RELEASES.md](docs/RELEASES.md).
