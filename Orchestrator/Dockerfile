FROM rust:1.92-slim AS builder

WORKDIR /build
COPY ReviewAgent /build/ReviewAgent
COPY FixAgent /build/FixAgent
COPY Orchestrator /build/Orchestrator

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

RUN cargo build --release --manifest-path /build/Orchestrator/Cargo.toml

FROM debian:12.12-slim

RUN apt-get update && apt-get install -y \
    ca-certificates \
    git \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /build/Orchestrator/target/release/orchestrator /usr/local/bin/orchestrator

ENV RUST_LOG=info

CMD ["/usr/local/bin/orchestrator", "serve-http"]
