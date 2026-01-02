use serde::{Deserialize, Serialize};
use std::io::{self, Read, Write};

/// Input structure - customize for your skill
#[derive(Debug, Deserialize)]
struct Input {
    /// The data to process
    data: String,
    /// Optional configuration
    #[serde(default)]
    config: Config,
}

#[derive(Debug, Deserialize, Default)]
struct Config {
    /// Enable verbose output
    #[serde(default)]
    verbose: bool,
    /// Processing mode
    #[serde(default = "default_mode")]
    mode: String,
}

fn default_mode() -> String {
    "default".to_string()
}

/// Output structure - customize for your skill
#[derive(Debug, Serialize)]
struct Output {
    /// Processing result
    result: String,
    /// Whether processing succeeded
    success: bool,
    /// Optional metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<Metadata>,
}

#[derive(Debug, Serialize)]
struct Metadata {
    processed_bytes: usize,
    duration_ms: u64,
}

/// Error response
#[derive(Debug, Serialize)]
struct ErrorOutput {
    success: bool,
    error: ErrorDetails,
}

#[derive(Debug, Serialize)]
struct ErrorDetails {
    code: String,
    message: String,
}

fn main() {
    if let Err(e) = run() {
        let error = ErrorOutput {
            success: false,
            error: ErrorDetails {
                code: "EXECUTION_ERROR".to_string(),
                message: e.to_string(),
            },
        };
        eprintln!("{}", serde_json::to_string(&error).unwrap());
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    // Read JSON input from stdin
    let mut input_str = String::new();
    io::stdin().read_to_string(&mut input_str)?;

    // Parse input
    let input: Input = serde_json::from_str(&input_str)?;

    // Process (replace with your logic)
    let start = std::time::Instant::now();
    let result = process(&input)?;
    let duration = start.elapsed();

    // Build output
    let output = Output {
        result,
        success: true,
        metadata: Some(Metadata {
            processed_bytes: input.data.len(),
            duration_ms: duration.as_millis() as u64,
        }),
    };

    // Write JSON output to stdout
    let output_json = serde_json::to_string(&output)?;
    io::stdout().write_all(output_json.as_bytes())?;
    io::stdout().write_all(b"\n")?;

    Ok(())
}

/// Main processing logic - customize this function
fn process(input: &Input) -> anyhow::Result<String> {
    // Example: Process the input data
    let processed = match input.config.mode.as_str() {
        "uppercase" => input.data.to_uppercase(),
        "lowercase" => input.data.to_lowercase(),
        "reverse" => input.data.chars().rev().collect(),
        _ => format!("Processed: {}", input.data),
    };

    if input.config.verbose {
        eprintln!("Processing mode: {}", input.config.mode);
        eprintln!("Input length: {} bytes", input.data.len());
    }

    Ok(processed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_process_default() {
        let input = Input {
            data: "hello".to_string(),
            config: Config::default(),
        };
        let result = process(&input).unwrap();
        assert_eq!(result, "Processed: hello");
    }

    #[test]
    fn test_process_uppercase() {
        let input = Input {
            data: "hello".to_string(),
            config: Config {
                mode: "uppercase".to_string(),
                ..Default::default()
            },
        };
        let result = process(&input).unwrap();
        assert_eq!(result, "HELLO");
    }
}
