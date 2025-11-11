use crate::context::AppContext;
use crate::state::AppState;

#[derive(Clone)]
pub struct App {
    pub ctx: AppContext,
    pub state: AppState,
}

impl App {
    pub fn new(ctx: AppContext, state: AppState) -> Self {
        Self { ctx, state }
    }
}
