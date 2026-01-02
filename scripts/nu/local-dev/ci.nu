#!/usr/bin/env nu

# CI commands - Dagger pipeline management

use ../shared/shared.nu [log]

const CI_DIR = "ci"

# Run full CI pipeline
export def "main ci all" [] {
    log info "Running full CI pipeline..."
    cd $CI_DIR
    npm run all
}

# Build binaries
export def "main ci build" [] {
    log info "Building binaries..."
    cd $CI_DIR
    npm run build
}

# Run tests
export def "main ci test" [] {
    log info "Running tests..."
    cd $CI_DIR
    npm run test
}

# Run linter
export def "main ci lint" [] {
    log info "Running clippy..."
    cd $CI_DIR
    npm run lint
}

# Build container images
export def "main ci container" [] {
    log info "Building containers..."
    cd $CI_DIR
    npm run container
}

# Publish containers to registry
export def "main ci publish" [
    --registry(-r): string = "ghcr.io/yurikrupnik"  # Container registry
    --tag(-t): string = "latest"                     # Image tag
] {
    log info $"Publishing to ($registry)..."
    cd $CI_DIR
    $env.CONTAINER_REGISTRY = $registry
    $env.TAG = $tag
    npm run publish
}

# Install CI dependencies
export def "main ci setup" [] {
    log info "Installing CI dependencies..."
    cd $CI_DIR
    npm install
}

# Build single binary container with Docker
export def "main ci docker" [
    binary: string = "operator"  # Binary to build: operator or dotconfig
    --tag(-t): string = ""       # Image tag
] {
    let tag = if $tag == "" { $binary } else { $tag }
    log info $"Building Docker image for ($binary)..."
    docker build --build-arg BINARY=($binary) -t $tag .
}

# Run container locally
export def "main ci run" [
    binary: string = "operator"  # Binary to run
] {
    log info $"Running ($binary) container..."
    docker run --rm -it --name $binary $binary
}

# Main help
def main [] {
    print "CI Pipeline Commands"
    print ""
    print "Commands:"
    print "  ci all       - Run full pipeline (lint, test, build)"
    print "  ci build     - Build binaries"
    print "  ci test      - Run tests"
    print "  ci lint      - Run clippy"
    print "  ci container - Build container images"
    print "  ci publish   - Publish to registry"
    print "  ci setup     - Install dependencies"
    print "  ci docker    - Build with Docker directly"
    print "  ci run       - Run container locally"
}
