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

