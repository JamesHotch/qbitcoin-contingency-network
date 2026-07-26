# QBitCoin benchmark tooling

Run:

```bash
cargo run --release --bin qbtc-bench -- --iterations 100
```

The tool measures hybrid signing and verification, reports encoded key and
signature sizes, and prints the configured testnet weight and signature-cost
limits. Results are environment-specific and are not production parameters.

Post-quantum signatures materially increase bandwidth, storage, verification
CPU, mempool denial-of-service exposure, and block propagation time. Defaults
in this repository are conservative research values pending repeatable results
from multiple architectures and independent review.
