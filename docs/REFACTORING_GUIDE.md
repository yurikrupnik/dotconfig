# Refactoring Guide: Decoupling and Abstraction

This document explains the refactoring work done to address tight coupling and missing abstractions in PR #2.

## Overview

We've introduced two main abstractions to improve code quality:

1. **CommandContext trait** - Decouples commands from the concrete `App` struct
2. **KubeClient trait** - Abstracts Kubernetes operations for better testability

## Changes Made

### 1. CommandContext Trait (`src/traits/command_context.rs`)

**Problem:** Commands were tightly coupled to the `App` struct, making testing difficult and reducing flexibility.

**Solution:** Created a trait that defines the interface commands need:

```rust
pub trait CommandContext: Send + Sync {
    fn dry_run(&self) -> bool;
    fn output_format(&self) -> OutputFormat;
    fn no_color(&self) -> bool;
    fn debug_level(&self) -> u8;
    fn postgres_url(&self) -> Option<&str>;
    fn redis_url(&self) -> Option<&str>;
    fn mongo_url(&self) -> Option<&str>;
    fn neo4j_uri(&self) -> &str;
    fn neo4j_username(&self) -> &str;
    fn neo4j_password(&self) -> &str;

    // Convenience methods with default implementations
    fn tracing_level(&self) -> Level { /* ... */ }
    fn should_output(&self) -> bool { /* ... */ }
    fn is_json_output(&self) -> bool { /* ... */ }
}
```

**Benefits:**
- ✅ Commands can be tested with mock contexts
- ✅ Multiple implementations possible (testing, production, different backends)
- ✅ Clearer API surface - only exposes what commands actually need
- ✅ Easier to extend without breaking existing code

### 2. KubeClient Trait (`src/traits/kube_client.rs`)

**Problem:** The operator directly used `kube-rs`, making it impossible to test without a real Kubernetes cluster.

**Solution:** Created a trait abstracting Kubernetes operations:

```rust
#[async_trait]
pub trait KubeClient: Send + Sync {
    async fn list_nodes(&self) -> Result<Vec<Node>, KubeError>;
    async fn get_node_metrics(&self, node_name: &str) -> Result<NodeMetrics, KubeError>;
    async fn get_nodes_summary(&self) -> Result<Vec<NodeSummary>, KubeError>;
}
```

**Implementations:**
- `RealKubeClient` - Production implementation using kube-rs
- `MockKubeClient` - Test implementation for unit tests

**Benefits:**
- ✅ Unit tests don't need a real cluster
- ✅ Easy to test error conditions
- ✅ Future flexibility (different cloud providers, mock servers)
- ✅ Consistent error handling with `KubeError` enum

### 3. Updated RunCommand Trait

**Before:**
```rust
#[async_trait::async_trait]
pub trait RunCommand {
    async fn run(&self, app: &App) -> Result<()>;
}
```

**After:**
```rust
#[async_trait::async_trait]
pub trait RunCommand {
    async fn run(&self, ctx: &dyn CommandContext) -> Result<()>;
}
```

Now commands work with any `CommandContext` implementation, not just `App`.

## Migration Guide

### For Command Implementations

**Before:**
```rust
#[async_trait::async_trait]
impl RunCommand for MyCommand {
    async fn run(&self, app: &App) -> Result<()> {
        if app.ctx.dry_run {
            println!("Dry run!");
        }
        let uri = &app.state.neo4j_uri;
        // ...
    }
}
```

**After:**
```rust
#[async_trait::async_trait]
impl RunCommand for MyCommand {
    async fn run(&self, ctx: &dyn CommandContext) -> Result<()> {
        if ctx.dry_run() {
            println!("Dry run!");
        }
        let uri = ctx.neo4j_uri();
        // ...
    }
}
```

**Changes:**
1. Parameter type: `app: &App` → `ctx: &dyn CommandContext`
2. Field access: `app.ctx.dry_run` → `ctx.dry_run()`
3. Field access: `app.state.neo4j_uri` → `ctx.neo4j_uri()`

### For Testing Commands

**Before:**
```rust
#[tokio::test]
async fn test_my_command() {
    let ctx = AppContext::new(/* ... */);
    let state = AppState::new();
    let app = App::new(ctx, state);

    let cmd = MyCommand { /* ... */ };
    cmd.run(&app).await.unwrap();
}
```

**After:**
```rust
use crate::traits::command_context::MockContext;

#[tokio::test]
async fn test_my_command() {
    // Create a minimal mock context with only what you need
    let ctx = MockContext {
        dry_run: true,
        output_format: OutputFormat::Human,
        // ... only set fields relevant to your test
    };

    let cmd = MyCommand { /* ... */ };
    cmd.run(&ctx).await.unwrap();
}
```

### For Kubernetes Operations

**Before (operator.rs):**
```rust
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = kube::Client::try_default().await?;
    let api: Api<Node> = Api::all(client.clone());
    let nodes = api.list(&ListParams::default()).await?;
    // ... direct kube-rs usage
}
```

**After (operator_refactored.rs):**
```rust
use crate::traits::kube_client::{RealKubeClient, KubeClient};
use crate::operator_refactored::run_operator;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let client = RealKubeClient::try_default().await?;
    run_operator(&client).await?;
    Ok(())
}
```

**Testing with Mock:**
```rust
use crate::traits::kube_client::MockKubeClient;
use crate::operator_refactored::run_operator;

#[tokio::test]
async fn test_operator() {
    let client = MockKubeClient::new()
        .with_node("test-node")
        .with_metrics("test-node", 1_000_000, 2_000_000);

    let result = run_operator(&client).await;
    assert!(result.is_ok());
}
```

## Example: Testing With Mock Context

```rust
use crate::traits::CommandContext;
use crate::context::OutputFormat;
use tracing::Level;

struct TestContext {
    pub dry_run: bool,
    pub output: OutputFormat,
}

impl CommandContext for TestContext {
    fn dry_run(&self) -> bool {
        self.dry_run
    }

    fn output_format(&self) -> OutputFormat {
        self.output
    }

    // ... implement other required methods
}

#[tokio::test]
async fn test_command_dry_run() {
    let ctx = TestContext {
        dry_run: true,
        output: OutputFormat::Human,
    };

    let cmd = MyCommand::new();
    let result = cmd.run(&ctx).await;

    assert!(result.is_ok());
    // Command should have done nothing due to dry_run
}

#[tokio::test]
async fn test_command_json_output() {
    let ctx = TestContext {
        dry_run: false,
        output: OutputFormat::Json,
    };

    let cmd = MyCommand::new();
    let result = cmd.run(&ctx).await;

    assert!(result.is_ok());
    // Verify JSON output format was used
}
```

## Example: Testing Kubernetes Operations

```rust
use crate::traits::kube_client::{MockKubeClient, KubeError};

#[tokio::test]
async fn test_node_metrics_not_found() {
    let client = MockKubeClient::new()
        .with_node("existing-node");

    let result = client.get_node_metrics("non-existent-node").await;

    assert!(result.is_err());
    assert!(matches!(result.unwrap_err(), KubeError::NotFound(_)));
}

#[tokio::test]
async fn test_get_nodes_summary() {
    let client = MockKubeClient::new()
        .with_node("node-1")
        .with_metrics("node-1", 1_000_000_000, 2_000_000_000)
        .with_node("node-2")
        .with_metrics("node-2", 500_000_000, 1_000_000_000);

    let summaries = client.get_nodes_summary().await.unwrap();

    assert_eq!(summaries.len(), 2);
    assert_eq!(summaries[0].name, "node-1");
    assert_eq!(summaries[1].name, "node-2");
}
```

## Benefits Summary

### Before Refactoring

| Issue | Impact |
|-------|--------|
| Commands depend on concrete `App` struct | Hard to test, inflexible |
| Direct kube-rs usage in operator | Requires real cluster for tests |
| No error abstraction for K8s ops | Inconsistent error handling |
| Tight coupling throughout | Changes ripple across codebase |

### After Refactoring

| Improvement | Benefit |
|-------------|---------|
| Commands use `CommandContext` trait | Easy mocking, flexible implementations |
| K8s ops behind `KubeClient` trait | Unit tests work without clusters |
| Dedicated `KubeError` type | Consistent, explicit error handling |
| Loose coupling via traits | Changes isolated to implementations |

## Future Extensions

These abstractions enable future improvements:

1. **Multiple Cloud Providers**: Implement `KubeClient` for different providers
2. **Caching Layer**: Wrap `RealKubeClient` with caching
3. **Rate Limiting**: Add rate limiting to Kubernetes calls
4. **Telemetry**: Inject metrics collection via traits
5. **Configuration Sources**: Different `CommandContext` implementations for various config sources

## Testing Strategy

### Unit Tests
- Use `MockContext` and `MockKubeClient`
- Test business logic in isolation
- Fast, no external dependencies

### Integration Tests
- Use `RealKubeClient` with test cluster
- Verify real Kubernetes integration
- Slower, requires infrastructure

### Example Test Structure

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Unit test - fast, no dependencies
    #[tokio::test]
    async fn test_command_logic() {
        let ctx = MockContext::new();
        let cmd = MyCommand::new();
        assert!(cmd.run(&ctx).await.is_ok());
    }

    // Integration test - requires real K8s
    #[tokio::test]
    #[ignore] // Run explicitly with: cargo test --ignored
    async fn test_with_real_cluster() {
        let client = RealKubeClient::try_default().await.unwrap();
        let result = run_operator(&client).await;
        assert!(result.is_ok());
    }
}
```

## Conclusion

These abstractions significantly improve:
- **Testability**: Mock implementations for fast unit tests
- **Flexibility**: Easy to swap implementations
- **Maintainability**: Clear interfaces and separation of concerns
- **Error Handling**: Explicit error types and consistent handling

All existing code continues to work through the `CommandContext` implementation for `App`.
