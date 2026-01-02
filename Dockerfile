# Multi-stage Dockerfile for dotconfig binaries
# Builds: dotconfig (CLI), platform-operator, node-metrics

ARG BINARY=platform-operator
ARG RUST_VERSION=1.83
ARG HELM_VERSION=3.16.3

# Build stage
FROM rust:${RUST_VERSION}-slim AS builder

RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    musl-tools \
    && rm -rf /var/lib/apt/lists/*

RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /app

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir -p src/bin src/operator && \
    echo "fn main() {}" > src/main.rs && \
    echo "fn main() {}" > src/bin/platform_operator.rs && \
    echo "fn main() {}" > src/node_metrics.rs && \
    echo "pub mod operator;" > src/lib.rs && \
    echo "" > src/operator/mod.rs
RUN cargo build --release --target x86_64-unknown-linux-musl 2>/dev/null || true
RUN rm -rf src

# Build actual binaries
COPY src ./src
ARG BINARY
RUN cargo build --bin ${BINARY} --release --target x86_64-unknown-linux-musl

# Download Helm
FROM alpine:3.20 AS helm-downloader
ARG HELM_VERSION
RUN apk add --no-cache curl tar && \
    curl -fsSL https://get.helm.sh/helm-v${HELM_VERSION}-linux-amd64.tar.gz | tar xz && \
    mv linux-amd64/helm /usr/local/bin/helm && \
    chmod +x /usr/local/bin/helm

# Runtime stage - Alpine for helm compatibility
FROM alpine:3.20

RUN apk add --no-cache ca-certificates

ARG BINARY
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/${BINARY} /app/binary
COPY --from=helm-downloader /usr/local/bin/helm /usr/local/bin/helm

# Create non-root user
RUN adduser -D -u 65532 operator
USER operator

ENTRYPOINT ["/app/binary"]
