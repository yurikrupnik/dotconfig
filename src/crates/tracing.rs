use std::env;
use tracing::{info, Level};
use tracing_subscriber::EnvFilter;

pub fn init_tracing_with_level(level: Level, no_color: bool) {
    env::set_var("RUST_BACKTRACE", "0");
    let rust_env = env::var("RUST_ENV").unwrap_or_else(|_| "development".into());
    let is_production = rust_env.eq_ignore_ascii_case("production");

    let level_str = match level {
        Level::TRACE => "trace",
        Level::DEBUG => "debug",
        Level::INFO => "info",
        Level::WARN => "warn",
        Level::ERROR => "error",
    };

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(level_str));

    if is_production {
        tracing_subscriber::fmt()
            .json()
            .with_env_filter(filter)
            .with_target(false)
            .init();
    } else {
        let builder = tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(false);

        if no_color {
            builder.with_ansi(false).init();
        } else {
            builder.pretty().init();
        }
    }

    info!("Logging initialized. Environment: {}, Level: {}", rust_env, level_str);
}
