FROM rust:1.99.1-bookworm AS builder
WORKDIR /src
COPY Cargo.toml Cargo.lock rust-toolchain.toml build.rs ./
COPY src ./src
COPY docs ./docs
COPY contrib ./contrib
RUN cargo build --locked --release --bins

FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/* \
    && useradd --create-home --uid 10001 qbtc
COPY --from=builder /src/target/release/qbtc-node /usr/local/bin/qbtc-node
COPY --from=builder /src/target/release/qbtc-wallet /usr/local/bin/qbtc-wallet
COPY --from=builder /src/target/release/qbtc-wallet-service /usr/local/bin/qbtc-wallet-service
COPY --from=builder /src/target/release/qbtc-miner /usr/local/bin/qbtc-miner
COPY --from=builder /src/target/release/qbtc-signer /usr/local/bin/qbtc-signer
COPY --from=builder /src/target/release/qbtc-demo /usr/local/bin/qbtc-demo
COPY --from=builder /src/target/release/qbtc-bench /usr/local/bin/qbtc-bench
RUN mkdir -p /data && chown qbtc:qbtc /data
USER qbtc
WORKDIR /data
