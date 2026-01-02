//! CPU-intensive File Processor
//!
//! Demonstrates CPU-intensive workloads that benefit from Rust's performance:
//! - Parallel file scanning with rayon
//! - Memory-mapped file reading
//! - SHA-256 hashing
//! - Regex-based code analysis
//! - Duplicate file detection

use anyhow::{Context, Result};
use async_trait::async_trait;
use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use clap::Parser;
use rayon::prelude::*;
use regex::Regex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use shared::{Skill, SkillError, SkillInput, SkillOutput};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing::{error, info};
use walkdir::WalkDir;

#[derive(Parser, Debug)]
#[command(name = "file-processor")]
#[command(about = "CPU-intensive file processing skill")]
struct Args {
    /// Service port
    #[arg(long, env = "PORT", default_value = "3002")]
    port: u16,

    /// Number of parallel workers (0 = auto)
    #[arg(long, env = "WORKERS", default_value = "0")]
    workers: usize,

    /// Maximum file size to process (MB)
    #[arg(long, env = "MAX_FILE_SIZE_MB", default_value = "100")]
    max_file_size_mb: u64,
}

/// File information
#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileInfo {
    path: String,
    size: u64,
    hash: Option<String>,
    mime_type: Option<String>,
    modified: Option<String>,
}

/// Code analysis result
#[derive(Debug, Clone, Serialize, Deserialize)]
struct CodeAnalysis {
    path: String,
    language: String,
    lines_total: usize,
    lines_code: usize,
    lines_comment: usize,
    lines_blank: usize,
    complexity: u32,
    issues: Vec<CodeIssue>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CodeIssue {
    line: usize,
    severity: String,
    message: String,
    rule: String,
}

/// Duplicate file group
#[derive(Debug, Clone, Serialize, Deserialize)]
struct DuplicateGroup {
    hash: String,
    size: u64,
    files: Vec<String>,
}

/// File Processor Skill
struct FileProcessorSkill {
    max_file_size: u64,
}

impl FileProcessorSkill {
    fn new(max_file_size_mb: u64) -> Self {
        Self {
            max_file_size: max_file_size_mb * 1024 * 1024,
        }
    }

    /// Scan directory and collect file information
    fn scan_directory(&self, path: &Path, pattern: Option<&str>) -> Vec<FileInfo> {
        let regex = pattern.and_then(|p| Regex::new(p).ok());

        WalkDir::new(path)
            .into_iter()
            .par_bridge()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| {
                regex
                    .as_ref()
                    .map(|r| r.is_match(&e.path().to_string_lossy()))
                    .unwrap_or(true)
            })
            .map(|entry| {
                let metadata = entry.metadata().ok();
                FileInfo {
                    path: entry.path().to_string_lossy().to_string(),
                    size: metadata.as_ref().map(|m| m.len()).unwrap_or(0),
                    hash: None,
                    mime_type: self.detect_mime_type(entry.path()),
                    modified: metadata.and_then(|m| {
                        m.modified().ok().map(|t| {
                            chrono::DateTime::<chrono::Utc>::from(t)
                                .format("%Y-%m-%d %H:%M:%S")
                                .to_string()
                        })
                    }),
                }
            })
            .collect()
    }

    /// Calculate SHA-256 hash using memory-mapped files for large files
    fn hash_file(&self, path: &Path) -> Result<String> {
        let file = std::fs::File::open(path)?;
        let metadata = file.metadata()?;

        if metadata.len() > self.max_file_size {
            anyhow::bail!("File too large");
        }

        let hash = if metadata.len() > 10 * 1024 * 1024 {
            // Use memory mapping for files > 10MB
            let mmap = unsafe { memmap2::Mmap::map(&file)? };
            let mut hasher = Sha256::new();
            hasher.update(&mmap[..]);
            hasher.finalize()
        } else {
            // Read small files into memory
            let content = std::fs::read(path)?;
            let mut hasher = Sha256::new();
            hasher.update(&content);
            hasher.finalize()
        };

        Ok(format!("{:x}", hash))
    }

    /// Find duplicate files by hash
    fn find_duplicates(&self, path: &Path) -> Vec<DuplicateGroup> {
        let files: Vec<_> = WalkDir::new(path)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .collect();

        // Calculate hashes in parallel
        let hashes: Vec<_> = files
            .par_iter()
            .filter_map(|entry| {
                let size = entry.metadata().ok()?.len();
                if size > self.max_file_size || size == 0 {
                    return None;
                }
                let hash = self.hash_file(entry.path()).ok()?;
                Some((hash, entry.path().to_path_buf(), size))
            })
            .collect();

        // Group by hash
        let mut groups: HashMap<String, (u64, Vec<PathBuf>)> = HashMap::new();
        for (hash, path, size) in hashes {
            groups
                .entry(hash)
                .or_insert((size, Vec::new()))
                .1
                .push(path);
        }

        // Filter to only duplicates
        groups
            .into_iter()
            .filter(|(_, (_, files))| files.len() > 1)
            .map(|(hash, (size, files))| DuplicateGroup {
                hash,
                size,
                files: files.iter().map(|p| p.to_string_lossy().to_string()).collect(),
            })
            .collect()
    }

    /// Analyze code files
    fn analyze_code(&self, path: &Path) -> Vec<CodeAnalysis> {
        let code_extensions = ["rs", "ts", "js", "py", "go", "java", "cpp", "c", "h"];

        WalkDir::new(path)
            .into_iter()
            .par_bridge()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| code_extensions.contains(&ext))
                    .unwrap_or(false)
            })
            .filter_map(|entry| self.analyze_file(entry.path()).ok())
            .collect()
    }

    fn analyze_file(&self, path: &Path) -> Result<CodeAnalysis> {
        let content = std::fs::read_to_string(path)?;
        let extension = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("unknown");

        let language = match extension {
            "rs" => "Rust",
            "ts" | "tsx" => "TypeScript",
            "js" | "jsx" => "JavaScript",
            "py" => "Python",
            "go" => "Go",
            "java" => "Java",
            "cpp" | "cc" | "cxx" => "C++",
            "c" => "C",
            "h" | "hpp" => "Header",
            _ => "Unknown",
        };

        let lines: Vec<&str> = content.lines().collect();
        let lines_total = lines.len();

        let (lines_blank, lines_comment, lines_code) = self.count_lines(&lines, language);
        let complexity = self.calculate_complexity(&content, language);
        let issues = self.find_issues(&lines, language, path);

        Ok(CodeAnalysis {
            path: path.to_string_lossy().to_string(),
            language: language.to_string(),
            lines_total,
            lines_code,
            lines_comment,
            lines_blank,
            complexity,
            issues,
        })
    }

    fn count_lines(&self, lines: &[&str], language: &str) -> (usize, usize, usize) {
        let mut blank = 0;
        let mut comment = 0;
        let mut code = 0;
        let mut in_block_comment = false;

        let (line_comment, block_start, block_end) = match language {
            "Python" => ("#", "\"\"\"", "\"\"\""),
            "Rust" | "Go" | "JavaScript" | "TypeScript" | "Java" | "C" | "C++" => {
                ("//", "/*", "*/")
            }
            _ => ("//", "/*", "*/"),
        };

        for line in lines {
            let trimmed = line.trim();

            if trimmed.is_empty() {
                blank += 1;
            } else if in_block_comment {
                comment += 1;
                if trimmed.contains(block_end) {
                    in_block_comment = false;
                }
            } else if trimmed.starts_with(block_start) {
                comment += 1;
                if !trimmed.contains(block_end) || trimmed.ends_with(block_start) {
                    in_block_comment = true;
                }
            } else if trimmed.starts_with(line_comment) {
                comment += 1;
            } else {
                code += 1;
            }
        }

        (blank, comment, code)
    }

    fn calculate_complexity(&self, content: &str, language: &str) -> u32 {
        let keywords = match language {
            "Rust" => vec!["if", "else", "match", "for", "while", "loop", "?", "&&", "||"],
            "Python" => vec!["if", "elif", "else", "for", "while", "try", "except", "and", "or"],
            "JavaScript" | "TypeScript" => {
                vec!["if", "else", "switch", "case", "for", "while", "do", "try", "catch", "&&", "||", "?"]
            }
            "Go" => vec!["if", "else", "switch", "case", "for", "select", "&&", "||"],
            _ => vec!["if", "else", "for", "while", "switch", "case"],
        };

        let mut complexity: u32 = 1;
        for keyword in keywords {
            complexity += content.matches(keyword).count() as u32;
        }
        complexity
    }

    fn find_issues(&self, lines: &[&str], language: &str, path: &Path) -> Vec<CodeIssue> {
        let mut issues = Vec::new();

        for (i, line) in lines.iter().enumerate() {
            let line_num = i + 1;

            // Long lines
            if line.len() > 120 {
                issues.push(CodeIssue {
                    line: line_num,
                    severity: "warning".to_string(),
                    message: format!("Line exceeds 120 characters ({} chars)", line.len()),
                    rule: "line-length".to_string(),
                });
            }

            // TODO/FIXME comments
            if line.contains("TODO") || line.contains("FIXME") {
                issues.push(CodeIssue {
                    line: line_num,
                    severity: "info".to_string(),
                    message: "Contains TODO/FIXME comment".to_string(),
                    rule: "todo-comment".to_string(),
                });
            }

            // Language-specific checks
            match language {
                "Rust" => {
                    if line.contains("unwrap()") {
                        issues.push(CodeIssue {
                            line: line_num,
                            severity: "warning".to_string(),
                            message: "Use of unwrap() can panic".to_string(),
                            rule: "rust-unwrap".to_string(),
                        });
                    }
                    if line.contains("unsafe") {
                        issues.push(CodeIssue {
                            line: line_num,
                            severity: "info".to_string(),
                            message: "Contains unsafe block".to_string(),
                            rule: "rust-unsafe".to_string(),
                        });
                    }
                }
                "JavaScript" | "TypeScript" => {
                    if line.contains("console.log") {
                        issues.push(CodeIssue {
                            line: line_num,
                            severity: "warning".to_string(),
                            message: "Remove console.log in production".to_string(),
                            rule: "no-console".to_string(),
                        });
                    }
                    if line.contains("any") && language == "TypeScript" {
                        issues.push(CodeIssue {
                            line: line_num,
                            severity: "warning".to_string(),
                            message: "Avoid using 'any' type".to_string(),
                            rule: "no-any".to_string(),
                        });
                    }
                }
                _ => {}
            }
        }

        issues
    }

    fn detect_mime_type(&self, path: &Path) -> Option<String> {
        let extension = path.extension()?.to_str()?;
        let mime = match extension {
            "rs" => "text/x-rust",
            "ts" | "tsx" => "text/typescript",
            "js" | "jsx" => "text/javascript",
            "py" => "text/x-python",
            "go" => "text/x-go",
            "java" => "text/x-java",
            "json" => "application/json",
            "yaml" | "yml" => "application/x-yaml",
            "toml" => "application/toml",
            "md" => "text/markdown",
            "html" => "text/html",
            "css" => "text/css",
            "txt" => "text/plain",
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "svg" => "image/svg+xml",
            "pdf" => "application/pdf",
            _ => "application/octet-stream",
        };
        Some(mime.to_string())
    }
}

#[async_trait]
impl Skill for FileProcessorSkill {
    fn name(&self) -> &str {
        "file-processor"
    }

    fn description(&self) -> &str {
        "CPU-intensive file processing: scanning, hashing, duplicate detection, code analysis"
    }

    async fn execute(&self, input: SkillInput) -> Result<SkillOutput, SkillError> {
        let start = std::time::Instant::now();

        let operation = input
            .data
            .get("operation")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SkillError::InvalidInput("Missing 'operation' field".to_string()))?;

        let path_str = input
            .data
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SkillError::InvalidInput("Missing 'path' field".to_string()))?;

        let path = PathBuf::from(path_str);
        if !path.exists() {
            return Err(SkillError::InvalidInput(format!(
                "Path does not exist: {}",
                path_str
            )));
        }

        let result = match operation {
            "scan" => {
                let pattern = input.data.get("pattern").and_then(|v| v.as_str());
                let files = tokio::task::spawn_blocking({
                    let this = self.clone();
                    let path = path.clone();
                    let pattern = pattern.map(String::from);
                    move || this.scan_directory(&path, pattern.as_deref())
                })
                .await
                .map_err(|e| SkillError::ExecutionFailed(e.to_string()))?;

                serde_json::json!({
                    "operation": "scan",
                    "path": path_str,
                    "file_count": files.len(),
                    "files": files,
                })
            }
            "hash" => {
                let hash = tokio::task::spawn_blocking({
                    let this = self.clone();
                    let path = path.clone();
                    move || this.hash_file(&path)
                })
                .await
                .map_err(|e| SkillError::ExecutionFailed(e.to_string()))?
                .map_err(|e| SkillError::ExecutionFailed(e.to_string()))?;

                serde_json::json!({
                    "operation": "hash",
                    "path": path_str,
                    "algorithm": "sha256",
                    "hash": hash,
                })
            }
            "duplicates" => {
                let duplicates = tokio::task::spawn_blocking({
                    let this = self.clone();
                    let path = path.clone();
                    move || this.find_duplicates(&path)
                })
                .await
                .map_err(|e| SkillError::ExecutionFailed(e.to_string()))?;

                let total_wasted: u64 = duplicates
                    .iter()
                    .map(|g| g.size * (g.files.len() as u64 - 1))
                    .sum();

                serde_json::json!({
                    "operation": "duplicates",
                    "path": path_str,
                    "groups": duplicates.len(),
                    "total_wasted_bytes": total_wasted,
                    "duplicates": duplicates,
                })
            }
            "analyze" => {
                let analysis = tokio::task::spawn_blocking({
                    let this = self.clone();
                    let path = path.clone();
                    move || this.analyze_code(&path)
                })
                .await
                .map_err(|e| SkillError::ExecutionFailed(e.to_string()))?;

                let total_lines: usize = analysis.iter().map(|a| a.lines_total).sum();
                let total_code: usize = analysis.iter().map(|a| a.lines_code).sum();
                let total_issues: usize = analysis.iter().map(|a| a.issues.len()).sum();

                serde_json::json!({
                    "operation": "analyze",
                    "path": path_str,
                    "files_analyzed": analysis.len(),
                    "total_lines": total_lines,
                    "total_code_lines": total_code,
                    "total_issues": total_issues,
                    "analysis": analysis,
                })
            }
            _ => {
                return Err(SkillError::InvalidInput(format!(
                    "Unknown operation: {}. Valid: scan, hash, duplicates, analyze",
                    operation
                )));
            }
        };

        let duration_ms = start.elapsed().as_millis() as u64;

        Ok(SkillOutput::new(input.id, result, duration_ms))
    }
}

impl Clone for FileProcessorSkill {
    fn clone(&self) -> Self {
        Self {
            max_file_size: self.max_file_size,
        }
    }
}

struct AppState {
    skill: FileProcessorSkill,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("file_processor=info".parse()?)
        )
        .json()
        .init();

    let args = Args::parse();
    info!("Starting file processor on port {}", args.port);

    // Configure rayon thread pool
    if args.workers > 0 {
        rayon::ThreadPoolBuilder::new()
            .num_threads(args.workers)
            .build_global()
            .context("Failed to configure thread pool")?;
    }

    let state = Arc::new(AppState {
        skill: FileProcessorSkill::new(args.max_file_size_mb),
    });

    let app = Router::new()
        .route("/health", get(health))
        .route("/execute", post(execute_skill))
        .route("/info", get(skill_info))
        // Convenience endpoints
        .route("/scan", post(scan))
        .route("/hash", post(hash))
        .route("/duplicates", post(duplicates))
        .route("/analyze", post(analyze))
        .with_state(state)
        .layer(TraceLayer::new_for_http());

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", args.port)).await?;
    info!("Listening on {}", listener.local_addr()?);

    axum::serve(listener, app).await?;

    Ok(())
}

async fn health() -> &'static str {
    "OK"
}

async fn execute_skill(
    State(state): State<Arc<AppState>>,
    Json(input): Json<SkillInput>,
) -> Result<Json<SkillOutput>, StatusCode> {
    state
        .skill
        .execute(input)
        .await
        .map(Json)
        .map_err(|e| {
            error!("Skill execution failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

#[derive(Serialize)]
struct SkillInfo {
    name: String,
    description: String,
    operations: Vec<String>,
}

async fn skill_info(State(state): State<Arc<AppState>>) -> Json<SkillInfo> {
    Json(SkillInfo {
        name: state.skill.name().to_string(),
        description: state.skill.description().to_string(),
        operations: vec![
            "scan".to_string(),
            "hash".to_string(),
            "duplicates".to_string(),
            "analyze".to_string(),
        ],
    })
}

// Convenience endpoint handlers
#[derive(Deserialize)]
struct PathRequest {
    path: String,
    #[serde(default)]
    pattern: Option<String>,
}

async fn scan(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PathRequest>,
) -> Result<Json<SkillOutput>, StatusCode> {
    let input = SkillInput::new(serde_json::json!({
        "operation": "scan",
        "path": req.path,
        "pattern": req.pattern,
    }));

    execute_skill(State(state), Json(input)).await
}

async fn hash(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PathRequest>,
) -> Result<Json<SkillOutput>, StatusCode> {
    let input = SkillInput::new(serde_json::json!({
        "operation": "hash",
        "path": req.path,
    }));

    execute_skill(State(state), Json(input)).await
}

async fn duplicates(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PathRequest>,
) -> Result<Json<SkillOutput>, StatusCode> {
    let input = SkillInput::new(serde_json::json!({
        "operation": "duplicates",
        "path": req.path,
    }));

    execute_skill(State(state), Json(input)).await
}

async fn analyze(
    State(state): State<Arc<AppState>>,
    Json(req): Json<PathRequest>,
) -> Result<Json<SkillOutput>, StatusCode> {
    let input = SkillInput::new(serde_json::json!({
        "operation": "analyze",
        "path": req.path,
    }));

    execute_skill(State(state), Json(input)).await
}
