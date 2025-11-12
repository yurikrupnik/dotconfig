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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::OutputFormat;

    #[test]
    fn test_app_new() {
        let ctx = AppContext::new(0, false, OutputFormat::Human, false, None, None, None);
        let state = AppState::new();

        let app = App::new(ctx.clone(), state.clone());

        assert_eq!(app.ctx.debug_level, 0);
        assert!(!app.ctx.dry_run);
        assert_eq!(app.state.neo4j_uri, state.neo4j_uri);
    }

    #[test]
    fn test_app_cloneable() {
        let ctx = AppContext::new(1, true, OutputFormat::Json, true, None, None, None);
        let state = AppState::new();
        let app = App::new(ctx, state);

        let cloned = app.clone();

        assert_eq!(app.ctx.debug_level, cloned.ctx.debug_level);
        assert_eq!(app.ctx.dry_run, cloned.ctx.dry_run);
        assert_eq!(app.state.neo4j_uri, cloned.state.neo4j_uri);
    }

    #[test]
    fn test_app_with_different_contexts() {
        let state = AppState::new();

        let app1 = App::new(
            AppContext::new(0, false, OutputFormat::Human, false, None, None, None),
            state.clone(),
        );
        let app2 = App::new(
            AppContext::new(2, true, OutputFormat::Json, true, None, None, None),
            state,
        );

        assert_ne!(app1.ctx.debug_level, app2.ctx.debug_level);
        assert_ne!(app1.ctx.dry_run, app2.ctx.dry_run);
        assert_eq!(app1.state.neo4j_uri, app2.state.neo4j_uri);
    }
}
