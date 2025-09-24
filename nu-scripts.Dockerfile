# Dockerfile for Nu scripts
FROM ghcr.io/nushell/nushell:latest-alpine

# Install additional tools that your scripts might need
RUN apk add --no-cache \
    docker-cli \
    docker-compose \
    curl \
    git \
    jq

# Create app directory
WORKDIR /app

# Copy Nu scripts
COPY scripts/ ./scripts/

# Make scripts executable (if needed)
RUN find ./scripts -name "*.nu" -type f -exec chmod +x {} \;

# Create a non-root user
RUN addgroup -g 1001 -S nuuser && \
    adduser -S nuuser -G nuuser

# Change ownership of app directory
RUN chown -R nuuser:nuuser /app

# Switch to non-root user
USER nuuser

# Set the default working directory for scripts
WORKDIR /app/scripts

# Default command - you can override this in docker-compose
CMD ["nu", "--help"]