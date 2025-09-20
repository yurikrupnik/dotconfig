use std::env;
use tracing::info;
use tracing_subscriber::EnvFilter;

pub fn init_tracing() {
    env::set_var("RUST_BACKTRACE", "0");
    // Determine the environment: "production" or "development"
    let rust_env = env::var("RUST_ENV").unwrap_or_else(|_| "development".to_string());
    let is_production = rust_env.eq_ignore_ascii_case("production");
    // Configure the EnvFilter:
    // - Try to create it from RUST_LOG
    // - If RUST_LOG is not set or invalid, default to "info"
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    // Initialize the tracing subscriber based on the environment
    if is_production {
        tracing_subscriber::fmt()
            .json() // JSON format for production logs
            .with_env_filter(filter)
            .with_target(false)
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_target(false)
            .pretty() // Pretty-printed logs for development
            .init();
    }

    // Test log to verify that logging is set up correctly
    info!("Logging initialized. Environment: {}", rust_env);
}