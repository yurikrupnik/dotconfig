# Build stage
FROM rust:1.83-slim-bookworm AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy the API crate
COPY apps/api ./

# Build the release binary
RUN cargo build --release --bin api-server

# Runtime stage
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder /app/target/release/api-server /usr/local/bin/api-server

# Create non-root user
RUN useradd -r -s /bin/false apiuser
USER apiuser

# Expose port
EXPOSE 3000

# Set environment variables
ENV PORT=3000
ENV RUST_LOG=api_server=info,tower_http=info

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:3000/health || exit 1

# Run the server
CMD ["api-server"]
