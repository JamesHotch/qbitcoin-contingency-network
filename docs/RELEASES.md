# Release Procedure

There are no production releases. Research tags must remain clearly marked
experimental.

## Multi-Maintainer Process

1. Freeze a reviewed commit through a protected pull request.
2. Require CI, dependency, CodeQL, fuzz-smoke, container, and reproducibility
   checks.
3. Have at least two maintainers inspect the diff and changelog.
4. Build independently following `REPRODUCIBLE_BUILDS.md`.
5. Compare hashes and generate an SBOM.
6. Create an annotated tag signed with a hardware-backed maintainer key.
7. Have a second maintainer countersign the release manifest.
8. Publish source commit, artifact hashes, toolchains, signatures, SBOM, scan
   results, and all known limitations.

Never share one signing key or CI identity between maintainers. Rotate release
keys through a public, pre-announced process. A failed or unmatched independent
build blocks release.

## Branch Protection Recommendation

Protect `main` with pull requests, two approvals, stale-review dismissal, code
owner review for consensus and workflows, signed commits where practical,
conversation resolution, linear history, required status checks, blocked force
pushes/deletions, and administrator enforcement.

Automation does not authorize mainnet and release signatures do not mean an
independent security audit occurred.
