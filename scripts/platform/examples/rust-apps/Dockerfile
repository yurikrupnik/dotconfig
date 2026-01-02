# Multi-stage Dockerfile for Rust example apps
# Supports building any app: docker build --target <app-name> .

# ============================================
# Stage 1: Build environment
# ============================================
FROM rust:1.83-slim-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy workspace manifests
COPY Cargo.toml Cargo.lock ./
COPY shared/Cargo.toml ./shared/
COPY queue-worker/Cargo.toml ./queue-worker/
COPY dapr-service/Cargo.toml ./dapr-service/
COPY llm-agent/Cargo.toml ./llm-agent/
COPY file-processor/Cargo.toml ./file-processor/

# Create dummy source files to cache dependencies
RUN mkdir -p shared/src queue-worker/src dapr-service/src llm-agent/src file-processor/src \
    && echo "pub fn dummy() {}" > shared/src/lib.rs \
    && echo "fn main() {}" > queue-worker/src/main.rs \
    && echo "fn main() {}" > dapr-service/src/main.rs \
    && echo "fn main() {}" > llm-agent/src/main.rs \
    && echo "fn main() {}" > file-processor/src/main.rs

# Build dependencies only (cached layer)
RUN cargo build --release --workspace && rm -rf target/release/.fingerprint/shared-* target/release/.fingerprint/queue-worker-* target/release/.fingerprint/dapr-service-* target/release/.fingerprint/llm-agent-* target/release/.fingerprint/file-processor-*

# Copy actual source code
COPY shared/src ./shared/src
COPY queue-worker/src ./queue-worker/src
COPY dapr-service/src ./dapr-service/src
COPY llm-agent/src ./llm-agent/src
COPY file-processor/src ./file-processor/src

# Build all apps
RUN cargo build --release --workspace

# ============================================
# Stage 2: Runtime base
# ============================================
FROM gcr.io/distroless/cc-debian12 AS runtime-base

# ============================================
# Queue Worker
# ============================================
FROM runtime-base AS queue-worker

COPY --from=builder /app/target/release/queue-worker /usr/local/bin/

ENV RUST_LOG=info
ENV REDIS_URL=redis://localhost:6379
ENV QUEUE_NAME=tasks
ENV CONCURRENCY=10
ENV HEALTH_PORT=8080

EXPOSE 8080

ENTRYPOINT ["/usr/local/bin/queue-worker"]

# ============================================
# Dapr Service
# ============================================
FROM runtime-base AS dapr-service

COPY --from=builder /app/target/release/dapr-service /usr/local/bin/

ENV RUST_LOG=info
ENV PORT=3000
ENV DAPR_HTTP_PORT=3500
ENV STATE_STORE=statestore
ENV PUBSUB_NAME=pubsub

EXPOSE 3000

ENTRYPOINT ["/usr/local/bin/dapr-service"]

# ============================================
# LLM Agent
# ============================================
FROM runtime-base AS llm-agent

COPY --from=builder /app/target/release/llm-agent /usr/local/bin/

ENV RUST_LOG=info
ENV PORT=3001
ENV DEFAULT_MODEL=anthropic/claude-3.5-sonnet
ENV FALLBACK_MODEL=openai/gpt-4o
ENV MAX_TOKENS=4096
# OPENROUTER_API_KEY must be provided

EXPOSE 3001

ENTRYPOINT ["/usr/local/bin/llm-agent"]

# ============================================
# File Processor
# ============================================
FROM runtime-base AS file-processor

COPY --from=builder /app/target/release/file-processor /usr/local/bin/

ENV RUST_LOG=info
ENV PORT=3002
ENV WORKERS=0
ENV MAX_FILE_SIZE_MB=100

EXPOSE 3002

ENTRYPOINT ["/usr/local/bin/file-processor"]

# ============================================
# Default: All apps in one image (for development)
# ============================================
FROM debian:bookworm-slim AS all

RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/queue-worker /usr/local/bin/
COPY --from=builder /app/target/release/dapr-service /usr/local/bin/
COPY --from=builder /app/target/release/llm-agent /usr/local/bin/
COPY --from=builder /app/target/release/file-processor /usr/local/bin/

ENV RUST_LOG=info

# Default to queue-worker, override with command
CMD ["/usr/local/bin/queue-worker"]
