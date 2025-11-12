use crate::context::OutputFormat;
use tracing::Level;

/// Trait for command execution context
///
/// This trait decouples commands from the concrete `App` struct,
/// making testing easier and allowing different implementations.
#[allow(dead_code)]
pub trait CommandContext: Send + Sync {
    /// Returns whether this is a dry-run execution
    fn dry_run(&self) -> bool;

    /// Returns the output format preference
    fn output_format(&self) -> OutputFormat;

    /// Returns whether color output is disabled
    fn no_color(&self) -> bool;

    /// Returns the debug/logging level
    fn debug_level(&self) -> u8;

    /// Returns the tracing level based on debug_level
    fn tracing_level(&self) -> Level {
        match self.debug_level() {
            0 => Level::INFO,
            1 => Level::DEBUG,
            _ => Level::TRACE,
        }
    }

    /// Returns whether output should be shown (not quiet mode)
    fn should_output(&self) -> bool {
        self.output_format() != OutputFormat::Quiet
    }

    /// Returns whether output should be in JSON format
    fn is_json_output(&self) -> bool {
        self.output_format() == OutputFormat::Json
    }

    /// Returns the PostgreSQL connection URL if configured
    fn postgres_url(&self) -> Option<&str>;

    /// Returns the Redis connection URL if configured
    fn redis_url(&self) -> Option<&str>;

    /// Returns the MongoDB connection URL if configured
    fn mongo_url(&self) -> Option<&str>;

    /// Returns the Neo4j connection URI
    fn neo4j_uri(&self) -> &str;

    /// Returns the Neo4j username
    fn neo4j_username(&self) -> &str;

    /// Returns the Neo4j password
    fn neo4j_password(&self) -> &str;
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockContext {
        pub dry_run: bool,
        pub output_format: OutputFormat,
        pub no_color: bool,
        pub debug_level: u8,
        pub neo4j_uri: String,
    }

    impl CommandContext for MockContext {
        fn dry_run(&self) -> bool {
            self.dry_run
        }

        fn output_format(&self) -> OutputFormat {
            self.output_format
        }

        fn no_color(&self) -> bool {
            self.no_color
        }

        fn debug_level(&self) -> u8 {
            self.debug_level
        }

        fn postgres_url(&self) -> Option<&str> {
            None
        }

        fn redis_url(&self) -> Option<&str> {
            None
        }

        fn mongo_url(&self) -> Option<&str> {
            None
        }

        fn neo4j_uri(&self) -> &str {
            &self.neo4j_uri
        }

        fn neo4j_username(&self) -> &str {
            "test"
        }

        fn neo4j_password(&self) -> &str {
            "test"
        }
    }

    #[test]
    fn test_tracing_level_info() {
        let ctx = MockContext {
            dry_run: false,
            output_format: OutputFormat::Human,
            no_color: false,
            debug_level: 0,
            neo4j_uri: "bolt://localhost:7687".into(),
        };
        assert_eq!(ctx.tracing_level(), Level::INFO);
    }

    #[test]
    fn test_tracing_level_debug() {
        let ctx = MockContext {
            dry_run: false,
            output_format: OutputFormat::Human,
            no_color: false,
            debug_level: 1,
            neo4j_uri: "bolt://localhost:7687".into(),
        };
        assert_eq!(ctx.tracing_level(), Level::DEBUG);
    }

    #[test]
    fn test_tracing_level_trace() {
        let ctx = MockContext {
            dry_run: false,
            output_format: OutputFormat::Human,
            no_color: false,
            debug_level: 2,
            neo4j_uri: "bolt://localhost:7687".into(),
        };
        assert_eq!(ctx.tracing_level(), Level::TRACE);
    }

    #[test]
    fn test_should_output_human() {
        let ctx = MockContext {
            dry_run: false,
            output_format: OutputFormat::Human,
            no_color: false,
            debug_level: 0,
            neo4j_uri: "bolt://localhost:7687".into(),
        };
        assert!(ctx.should_output());
    }

    #[test]
    fn test_should_output_quiet() {
        let ctx = MockContext {
            dry_run: false,
            output_format: OutputFormat::Quiet,
            no_color: false,
            debug_level: 0,
            neo4j_uri: "bolt://localhost:7687".into(),
        };
        assert!(!ctx.should_output());
    }

    #[test]
    fn test_is_json_output() {
        let ctx = MockContext {
            dry_run: false,
            output_format: OutputFormat::Json,
            no_color: false,
            debug_level: 0,
            neo4j_uri: "bolt://localhost:7687".into(),
        };
        assert!(ctx.is_json_output());
    }
}
