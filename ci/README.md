# Dagger CI Pipeline

Modernized CI/CD pipeline using **Daggerverse modules** for reusable, cached builds.

## Prerequisites

Install Dagger CLI:

```bash
# macOS
brew install dagger/tap/dagger

# or via curl
curl -fsSL https://dl.dagger.io/dagger/install.sh | sh
```

## Daggerverse Modules Used

| Module | Purpose |
|--------|---------|
| [purpleclay/rust](https://daggerverse.dev/mod/github.com/purpleclay/daggerverse/rust) | Rust builds with cargo caching |
| [shykes/wolfi](https://daggerverse.dev/mod/github.com/shykes/daggerverse/wolfi) | Minimal secure container base |

## Usage

From the `ci/` directory:

```bash
# Initialize module (first time only)
dagger develop

# View available functions
dagger functions

# Run commands
dagger call build --source=..
dagger call test --source=..
dagger call lint --source=..
dagger call all --source=..              # Full pipeline

# Build containers
dagger call container --source=.. --binary=platform_operator
dagger call container --source=.. --binary=resource_stats_operator

# Publish to registry
dagger call publish --source=.. --registry=ghcr.io/yurikrupnik --tag=v1.0.0

# Interactive dev shell
dagger call dev --source=.. terminal
```

Or use npm scripts:

```bash
npm run build
npm run test
npm run all
npm run dev
```

## Functions

| Function | Description |
|----------|-------------|
| `build` | Compile Rust binaries |
| `test` | Run cargo test |
| `lint` | Run clippy linter |
| `check` | Run cargo check |
| `container` | Build container for a single binary |
| `containers` | Build containers for all binaries |
| `publish` | Publish containers to registry |
| `all` | Run full CI pipeline |
| `dev` | Interactive development container |

## Migration from Old TypeScript SDK

The old `src/index.ts` used the legacy Dagger TypeScript SDK with manual container setup. The new module-based approach:

- Uses pre-built Daggerverse modules (less code to maintain)
- Better caching via the Rust module
- Cleaner CLI interface with `dagger call`
- Type-safe Go implementation
