use pulldown_cmark::{html, Options, Parser};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum DocError {
    #[error("Document not found: {0}")]
    NotFound(String),
    #[error("Failed to read document: {0}")]
    ReadError(String),
    #[error("Failed to parse frontmatter: {0}")]
    ParseError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocTree {
    pub sections: Vec<DocSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocSection {
    pub title: String,
    pub slug: String,
    pub pages: Vec<DocPage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocPage {
    pub title: String,
    pub slug: String,
    pub order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Document {
    pub title: String,
    pub description: Option<String>,
    pub html: String,
    pub last_modified: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub title: String,
    pub slug: String,
    pub excerpt: Option<String>,
    pub score: f32,
}

#[derive(Debug, Deserialize)]
struct Frontmatter {
    title: Option<String>,
    description: Option<String>,
    order: Option<i32>,
}

impl DocTree {
    pub async fn load() -> Result<Self, DocError> {
        // In a real implementation, this would scan the docs directory
        // For now, return a static structure based on the workspace
        Ok(DocTree {
            sections: vec![
                DocSection {
                    title: "Getting Started".to_string(),
                    slug: "getting-started".to_string(),
                    pages: vec![
                        DocPage {
                            title: "Installation".to_string(),
                            slug: "getting-started/installation".to_string(),
                            order: 1,
                        },
                        DocPage {
                            title: "Quick Start".to_string(),
                            slug: "getting-started/quick-start".to_string(),
                            order: 2,
                        },
                        DocPage {
                            title: "Configuration".to_string(),
                            slug: "getting-started/configuration".to_string(),
                            order: 3,
                        },
                    ],
                },
                DocSection {
                    title: "Custom Resources".to_string(),
                    slug: "crds".to_string(),
                    pages: vec![
                        DocPage {
                            title: "PlatformApp".to_string(),
                            slug: "crds/platform-app".to_string(),
                            order: 1,
                        },
                        DocPage {
                            title: "GitOpsApp".to_string(),
                            slug: "crds/gitops-app".to_string(),
                            order: 2,
                        },
                        DocPage {
                            title: "CrossplaneResource".to_string(),
                            slug: "crds/crossplane-resource".to_string(),
                            order: 3,
                        },
                        DocPage {
                            title: "ExternalSecretConfig".to_string(),
                            slug: "crds/external-secret".to_string(),
                            order: 4,
                        },
                    ],
                },
                DocSection {
                    title: "Guides".to_string(),
                    slug: "guides".to_string(),
                    pages: vec![
                        DocPage {
                            title: "Helm Integration".to_string(),
                            slug: "guides/helm".to_string(),
                            order: 1,
                        },
                        DocPage {
                            title: "KCL Manifests".to_string(),
                            slug: "guides/kcl".to_string(),
                            order: 2,
                        },
                        DocPage {
                            title: "GitOps with FluxCD".to_string(),
                            slug: "guides/gitops".to_string(),
                            order: 3,
                        },
                        DocPage {
                            title: "Secret Management".to_string(),
                            slug: "guides/secrets".to_string(),
                            order: 4,
                        },
                    ],
                },
                DocSection {
                    title: "Reference".to_string(),
                    slug: "reference".to_string(),
                    pages: vec![
                        DocPage {
                            title: "API Reference".to_string(),
                            slug: "reference/api".to_string(),
                            order: 1,
                        },
                        DocPage {
                            title: "kube-rs Limitations".to_string(),
                            slug: "reference/kube-rs-limitations".to_string(),
                            order: 2,
                        },
                        DocPage {
                            title: "Troubleshooting".to_string(),
                            slug: "reference/troubleshooting".to_string(),
                            order: 3,
                        },
                    ],
                },
            ],
        })
    }
}

pub async fn load_doc(path: &str) -> Result<Document, DocError> {
    // Construct the file path
    let docs_dir = get_docs_dir();
    let file_path = docs_dir.join(format!("{}.md", path));

    // Read the file
    let content = tokio::fs::read_to_string(&file_path)
        .await
        .map_err(|_| DocError::NotFound(path.to_string()))?;

    // Parse frontmatter and content
    let (frontmatter, markdown) = parse_frontmatter(&content)?;

    // Convert markdown to HTML
    let html = markdown_to_html(&markdown);

    // Get file metadata
    let last_modified = tokio::fs::metadata(&file_path)
        .await
        .ok()
        .and_then(|m| m.modified().ok())
        .map(|t| {
            chrono::DateTime::<chrono::Utc>::from(t)
                .format("%Y-%m-%d")
                .to_string()
        });

    Ok(Document {
        title: frontmatter.title.unwrap_or_else(|| title_from_path(path)),
        description: frontmatter.description,
        html,
        last_modified,
    })
}

pub async fn search_docs(query: &str) -> Result<Vec<SearchResult>, DocError> {
    let docs_dir = get_docs_dir();
    let mut results = Vec::new();

    // Simple search implementation - in production would use a proper search index
    let query_lower = query.to_lowercase();

    if let Ok(entries) = std::fs::read_dir(&docs_dir) {
        for entry in entries.flatten() {
            search_dir_recursive(&entry.path(), &query_lower, &mut results).await;
        }
    }

    // Sort by score
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));

    // Limit results
    results.truncate(10);

    Ok(results)
}

async fn search_dir_recursive(path: &PathBuf, query: &str, results: &mut Vec<SearchResult>) {
    if path.is_dir() {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                Box::pin(search_dir_recursive(&entry.path(), query, results)).await;
            }
        }
    } else if path.extension().map(|e| e == "md").unwrap_or(false) {
        if let Ok(content) = tokio::fs::read_to_string(path).await {
            let content_lower = content.to_lowercase();

            if content_lower.contains(query) {
                let (frontmatter, markdown) = match parse_frontmatter(&content) {
                    Ok((fm, md)) => (fm, md),
                    Err(_) => return,
                };

                let title = frontmatter.title.unwrap_or_else(|| {
                    path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("Untitled")
                        .to_string()
                });

                // Calculate score based on matches
                let title_matches = title.to_lowercase().matches(query).count();
                let content_matches = content_lower.matches(query).count();
                let score = (title_matches * 10 + content_matches) as f32;

                // Extract excerpt around match
                let excerpt = extract_excerpt(&markdown, query);

                let slug = path_to_slug(path);

                results.push(SearchResult {
                    title,
                    slug,
                    excerpt,
                    score,
                });
            }
        }
    }
}

fn get_docs_dir() -> PathBuf {
    // In production, this would be configurable
    std::env::var("DOCS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("docs"))
}

fn parse_frontmatter(content: &str) -> Result<(Frontmatter, String), DocError> {
    if content.starts_with("---") {
        let parts: Vec<&str> = content.splitn(3, "---").collect();
        if parts.len() >= 3 {
            let frontmatter: Frontmatter = serde_yaml::from_str(parts[1])
                .map_err(|e| DocError::ParseError(e.to_string()))?;
            return Ok((frontmatter, parts[2].trim().to_string()));
        }
    }

    Ok((
        Frontmatter {
            title: None,
            description: None,
            order: None,
        },
        content.to_string(),
    ))
}

fn markdown_to_html(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_HEADING_ATTRIBUTES);

    let parser = Parser::new_ext(markdown, options);

    let mut html_output = String::new();
    html::push_html(&mut html_output, parser);

    html_output
}

fn title_from_path(path: &str) -> String {
    path.split('/')
        .last()
        .unwrap_or(path)
        .replace('-', " ")
        .split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => first.to_uppercase().chain(chars).collect(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn path_to_slug(path: &PathBuf) -> String {
    let docs_dir = get_docs_dir();
    path.strip_prefix(&docs_dir)
        .unwrap_or(path)
        .with_extension("")
        .to_string_lossy()
        .to_string()
}

fn extract_excerpt(content: &str, query: &str) -> Option<String> {
    let content_lower = content.to_lowercase();
    let pos = content_lower.find(query)?;

    let start = pos.saturating_sub(50);
    let end = (pos + query.len() + 100).min(content.len());

    let mut excerpt = content[start..end].to_string();

    if start > 0 {
        excerpt = format!("...{}", excerpt.trim_start());
    }
    if end < content.len() {
        excerpt = format!("{}...", excerpt.trim_end());
    }

    Some(excerpt)
}
