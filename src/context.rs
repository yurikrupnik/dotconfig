use tracing::Level;

#[derive(Debug, Clone)]
pub struct AppContext {
    pub debug_level: u8,
    pub dry_run: bool,
    #[allow(dead_code)]
    pub output_format: OutputFormat,
    pub no_color: bool,
    #[allow(dead_code)]
    pub postgres_url: Option<String>,
    #[allow(dead_code)]
    pub redis_url: Option<String>,
    #[allow(dead_code)]
    pub mongo_url: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum OutputFormat {
    Human,
    Json,
    Quiet,
}

impl AppContext {
    pub fn new(
        debug_level: u8,
        dry_run: bool,
        output_format: OutputFormat,
        no_color: bool,
        postgres_url: Option<String>,
        redis_url: Option<String>,
        mongo_url: Option<String>,
    ) -> Self {
        Self {
            debug_level,
            dry_run,
            output_format,
            no_color,
            postgres_url,
            redis_url,
            mongo_url,
        }
    }

    pub fn tracing_level(&self) -> Level {
        match self.debug_level {
            0 => Level::INFO,
            1 => Level::DEBUG,
            _ => Level::TRACE,
        }
    }

    #[allow(dead_code)]
    pub fn should_output(&self) -> bool {
        self.output_format != OutputFormat::Quiet
    }

    #[allow(dead_code)]
    pub fn is_json_output(&self) -> bool {
        self.output_format == OutputFormat::Json
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_context_new() {
        let ctx = AppContext::new(
            0,
            false,
            OutputFormat::Human,
            false,
            Some("postgres://localhost".into()),
            None,
            None,
        );

        assert_eq!(ctx.debug_level, 0);
        assert!(!ctx.dry_run);
        assert_eq!(ctx.output_format, OutputFormat::Human);
        assert!(!ctx.no_color);
        assert_eq!(ctx.postgres_url, Some("postgres://localhost".into()));
        assert_eq!(ctx.redis_url, None);
        assert_eq!(ctx.mongo_url, None);
    }

    #[test]
    fn test_tracing_level_info() {
        let ctx = AppContext::new(0, false, OutputFormat::Human, false, None, None, None);
        assert_eq!(ctx.tracing_level(), Level::INFO);
    }

    #[test]
    fn test_tracing_level_debug() {
        let ctx = AppContext::new(1, false, OutputFormat::Human, false, None, None, None);
        assert_eq!(ctx.tracing_level(), Level::DEBUG);
    }

    #[test]
    fn test_tracing_level_trace() {
        let ctx = AppContext::new(2, false, OutputFormat::Human, false, None, None, None);
        assert_eq!(ctx.tracing_level(), Level::TRACE);

        let ctx = AppContext::new(5, false, OutputFormat::Human, false, None, None, None);
        assert_eq!(ctx.tracing_level(), Level::TRACE);
    }

    #[test]
    fn test_should_output_human() {
        let ctx = AppContext::new(0, false, OutputFormat::Human, false, None, None, None);
        assert!(ctx.should_output());
    }

    #[test]
    fn test_should_output_json() {
        let ctx = AppContext::new(0, false, OutputFormat::Json, false, None, None, None);
        assert!(ctx.should_output());
    }

    #[test]
    fn test_should_output_quiet() {
        let ctx = AppContext::new(0, false, OutputFormat::Quiet, false, None, None, None);
        assert!(!ctx.should_output());
    }

    #[test]
    fn test_is_json_output() {
        let ctx = AppContext::new(0, false, OutputFormat::Json, false, None, None, None);
        assert!(ctx.is_json_output());

        let ctx = AppContext::new(0, false, OutputFormat::Human, false, None, None, None);
        assert!(!ctx.is_json_output());

        let ctx = AppContext::new(0, false, OutputFormat::Quiet, false, None, None, None);
        assert!(!ctx.is_json_output());
    }

    #[test]
    fn test_dry_run_flag() {
        let ctx = AppContext::new(0, true, OutputFormat::Human, false, None, None, None);
        assert!(ctx.dry_run);

        let ctx = AppContext::new(0, false, OutputFormat::Human, false, None, None, None);
        assert!(!ctx.dry_run);
    }

    #[test]
    fn test_no_color_flag() {
        let ctx = AppContext::new(0, false, OutputFormat::Human, true, None, None, None);
        assert!(ctx.no_color);

        let ctx = AppContext::new(0, false, OutputFormat::Human, false, None, None, None);
        assert!(!ctx.no_color);
    }

    #[test]
    fn test_output_format_values() {
        assert_ne!(OutputFormat::Human, OutputFormat::Json);
        assert_ne!(OutputFormat::Human, OutputFormat::Quiet);
        assert_ne!(OutputFormat::Json, OutputFormat::Quiet);
    }
}

