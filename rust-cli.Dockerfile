# Multi-stage build for Rust CLI
FROM rust:1.90-alpine AS builder

# Install dependencies for building
RUN apk add --no-cache musl-dev openssl-dev pkgconfig

# Set working directory
WORKDIR /app

# Copy Cargo files
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src/ ./src/

# Build the release binary
RUN cargo build --release --bin dotconfig

# Runtime stage
#FROM alpine:latest
FROM scratch
#scratch

# Install runtime dependencies
#RUN apk add --no-cache ca-certificates

# Create app user
#RUN addgroup -g 1001 -S appuser && \
#    adduser -S appuser -G appuser

# Copy the binary from builder stage
COPY --from=builder /app/target/release/dotconfig /usr/local/bin/dotconfig

# Make it executable
#RUN chmod +x /usr/local/bin/dotconfig

# Switch to non-root user
#USER appuser

# Set entrypoint
ENTRYPOINT ["dotconfig"]

# Default command
CMD ["--help"]