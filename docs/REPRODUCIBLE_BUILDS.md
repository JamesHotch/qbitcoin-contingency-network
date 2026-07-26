# Reproducible Builds

## Current Scope

The Rust toolchain is pinned to 1.97.1, dependencies are locked, and release
profiles fix LTO, codegen units, overflow checks, and panic behavior. Docker
uses a pinned Rust and Debian release family. These controls improve
repeatability but do not yet prove bit-for-bit reproducibility.

## Local Procedure

```bash
./scripts/reproducible-build.sh
```

The script builds the locked release twice in clean target directories and
compares SHA-256 hashes for every shipped executable. On supported Unix hosts,
it also records compiler, Cargo, source commit, and binary hashes.

Differences must block a release and be investigated for timestamps, absolute
paths, host toolchain input, linker variation, build scripts, or dependencies.

## Independent Verification

At least two maintainers on independently administered machines should:

1. Verify the source tag and signed commit.
2. Use the pinned toolchain and locked dependencies.
3. Build without network after dependencies are vendored or cached.
4. Publish toolchain, operating system, source hash, commands, and artifact
   hashes.
5. Compare every artifact before signing a release manifest.

Current builds are not claimed reproducible until independent results match.

## Containers

Container tags alone are mutable identifiers unless pinned by digest. A release
process must pin base-image digests, generate an SBOM, scan the final image, and
sign both image and provenance. Version 0.1 CI checks construction and scanning
but is not a complete SLSA process.
