use anyhow::Result;
use crate::traits::CommandContext;

/// Trait for executable commands
///
/// This trait uses `CommandContext` instead of a concrete `App` type,
/// allowing commands to be tested with mock contexts and improving flexibility.
#[async_trait::async_trait]
pub trait RunCommand {
    /// Execute the command with the given context
    ///
    /// # Arguments
    /// * `ctx` - Any type implementing `CommandContext`
    ///
    /// # Returns
    /// `Ok(())` on success, or an error describing what went wrong
    async fn run(&self, ctx: &dyn CommandContext) -> Result<()>;
}
