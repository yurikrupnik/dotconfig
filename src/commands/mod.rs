use anyhow::Result;
use crate::app::App;

#[async_trait::async_trait]
pub trait RunCommand {
    async fn run(&self, app: &App) -> Result<()>;
}
