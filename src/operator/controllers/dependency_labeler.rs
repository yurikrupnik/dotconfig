//! Dependency Labeler Controller
//!
//! Watches Deployments and automatically adds labels based on environment variables
//! indicating database and service dependencies.
//!
//! Labels added:
//! - `platform.yurikrupnik.com/postgres`: "true" if POSTGRES_URL or DATABASE_URL with postgres
//! - `platform.yurikrupnik.com/mongo`: "true" if MONGO_URL or MONGODB_URI
//! - `platform.yurikrupnik.com/redis`: "true" if REDIS_URL
//! - `platform.yurikrupnik.com/internal`: "true" if using in-cluster services
//! - `platform.yurikrupnik.com/provider`: cloud provider name (neon, atlas, aiven, etc.)

use crate::operator::{Context, OperatorError};
use k8s_openapi::api::apps::v1::Deployment;
use kube::api::{Patch, PatchParams};
use kube::runtime::controller::Action;
use kube::{Api, ResourceExt};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Label prefix for dependency labels
const LABEL_PREFIX: &str = "platform.yurikrupnik.com";

/// Annotation to opt-in to dependency labeling
const OPT_IN_ANNOTATION: &str = "platform.yurikrupnik.com/auto-label";

/// Environment variable patterns to detect
#[derive(Debug, Clone)]
struct EnvPattern {
    /// Environment variable names to match
    env_names: Vec<&'static str>,
    /// Label key (without prefix)
    label_key: &'static str,
    /// Provider detection patterns (substring -> provider name)
    providers: Vec<(&'static str, &'static str)>,
}

/// Detected dependency information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DependencyInfo {
    /// Database/service dependencies detected
    pub dependencies: HashSet<String>,
    /// Whether services are internal (in-cluster)
    pub internal: bool,
    /// Cloud providers detected
    pub providers: HashSet<String>,
}

impl EnvPattern {
    fn patterns() -> Vec<Self> {
        vec![
            EnvPattern {
                env_names: vec!["POSTGRES_URL", "POSTGRES_URI", "DATABASE_URL", "PG_URL", "PGURL"],
                label_key: "postgres",
                providers: vec![
                    ("neon.tech", "neon"),
                    ("supabase", "supabase"),
                    ("aiven", "aiven"),
                    ("elephantsql", "elephantsql"),
                    ("render.com", "render"),
                    ("railway.app", "railway"),
                    ("cockroachlabs", "cockroachdb"),
                    ("timescale", "timescale"),
                    ("amazonaws.com", "aws-rds"),
                    ("azure.com", "azure"),
                    ("cloud.google.com", "gcp-cloudsql"),
                ],
            },
            EnvPattern {
                env_names: vec!["MONGO_URL", "MONGO_URI", "MONGODB_URL", "MONGODB_URI"],
                label_key: "mongo",
                providers: vec![
                    ("mongodb.net", "atlas"),
                    ("aiven", "aiven"),
                    ("cosmosdb", "azure-cosmosdb"),
                    ("docdb", "aws-documentdb"),
                ],
            },
            EnvPattern {
                env_names: vec!["REDIS_URL", "REDIS_URI", "REDIS_HOST"],
                label_key: "redis",
                providers: vec![
                    ("upstash", "upstash"),
                    ("redis.cloud", "redis-cloud"),
                    ("aiven", "aiven"),
                    ("elasticache", "aws-elasticache"),
                    ("azure", "azure-cache"),
                ],
            },
            EnvPattern {
                env_names: vec!["KAFKA_BROKERS", "KAFKA_URL", "KAFKA_BOOTSTRAP_SERVERS"],
                label_key: "kafka",
                providers: vec![
                    ("confluent", "confluent"),
                    ("aiven", "aiven"),
                    ("upstash", "upstash"),
                    ("msk", "aws-msk"),
                ],
            },
            EnvPattern {
                env_names: vec!["RABBITMQ_URL", "AMQP_URL", "CLOUDAMQP_URL"],
                label_key: "rabbitmq",
                providers: vec![
                    ("cloudamqp", "cloudamqp"),
                    ("aiven", "aiven"),
                    ("amazonaws", "aws-mq"),
                ],
            },
            EnvPattern {
                env_names: vec!["ELASTICSEARCH_URL", "ELASTIC_URL", "ES_URL"],
                label_key: "elasticsearch",
                providers: vec![
                    ("elastic.co", "elastic-cloud"),
                    ("aiven", "aiven"),
                    ("bonsai", "bonsai"),
                    ("aws", "aws-opensearch"),
                ],
            },
            EnvPattern {
                env_names: vec!["NEO4J_URI", "NEO4J_URL"],
                label_key: "neo4j",
                providers: vec![
                    ("neo4j.io", "aura"),
                    ("graphenedb", "graphenedb"),
                ],
            },
            EnvPattern {
                env_names: vec!["MYSQL_URL", "MYSQL_URI"],
                label_key: "mysql",
                providers: vec![
                    ("planetscale", "planetscale"),
                    ("aiven", "aiven"),
                    ("amazonaws.com", "aws-rds"),
                    ("azure.com", "azure"),
                ],
            },
            // Auth providers
            EnvPattern {
                env_names: vec!["AUTH0_DOMAIN", "AUTH0_CLIENT_ID"],
                label_key: "auth0",
                providers: vec![("auth0", "auth0")],
            },
            EnvPattern {
                env_names: vec!["CLERK_SECRET_KEY", "CLERK_PUBLISHABLE_KEY"],
                label_key: "clerk",
                providers: vec![("clerk", "clerk")],
            },
            EnvPattern {
                env_names: vec!["SUPABASE_URL", "SUPABASE_KEY"],
                label_key: "supabase",
                providers: vec![("supabase", "supabase")],
            },
            EnvPattern {
                env_names: vec!["FIREBASE_PROJECT_ID", "FIREBASE_API_KEY"],
                label_key: "firebase",
                providers: vec![("firebase", "firebase")],
            },
            // Monitoring
            EnvPattern {
                env_names: vec!["DATADOG_API_KEY", "DD_API_KEY"],
                label_key: "datadog",
                providers: vec![("datadoghq", "datadog")],
            },
            EnvPattern {
                env_names: vec!["NEW_RELIC_LICENSE_KEY", "NEWRELIC_LICENSE_KEY"],
                label_key: "newrelic",
                providers: vec![("newrelic", "newrelic")],
            },
            EnvPattern {
                env_names: vec!["SENTRY_DSN"],
                label_key: "sentry",
                providers: vec![("sentry.io", "sentry")],
            },
            // Object storage
            EnvPattern {
                env_names: vec!["S3_BUCKET", "AWS_S3_BUCKET", "S3_ENDPOINT"],
                label_key: "s3",
                providers: vec![
                    ("amazonaws.com", "aws-s3"),
                    ("r2.cloudflarestorage", "cloudflare-r2"),
                    ("digitaloceanspaces", "digitalocean-spaces"),
                    ("backblazeb2", "backblaze-b2"),
                ],
            },
        ]
    }
}

/// Check if a URL indicates an internal (in-cluster) service
fn is_internal_url(url: &str) -> bool {
    let internal_patterns = [
        "localhost",
        "127.0.0.1",
        ".svc.cluster.local",
        ".svc.cluster",
        ".svc",
        ":5432",  // Default postgres port without host usually means local
        ":27017", // Default mongo port
        ":6379",  // Default redis port
    ];

    // Check for kubernetes service DNS patterns
    if url.contains(".svc") || url.contains("localhost") || url.contains("127.0.0.1") {
        return true;
    }

    // If no protocol and just a service name, likely internal
    if !url.contains("://") && !url.contains('.') {
        return true;
    }

    internal_patterns.iter().any(|p| url.contains(p))
}

/// Extract provider from URL
fn detect_provider(url: &str, providers: &[(&str, &str)]) -> Option<String> {
    let url_lower = url.to_lowercase();
    for (pattern, provider) in providers {
        if url_lower.contains(pattern) {
            return Some(provider.to_string());
        }
    }

    // Check for internal providers
    if is_internal_url(url) {
        return Some("internal".to_string());
    }

    None
}

/// Analyze deployment environment variables and extract dependency info
fn analyze_deployment(deployment: &Deployment) -> DependencyInfo {
    let mut info = DependencyInfo::default();
    let patterns = EnvPattern::patterns();

    // Get all containers
    let containers = deployment
        .spec
        .as_ref()
        .map(|s| &s.template.spec)
        .and_then(|s| s.as_ref())
        .map(|s| &s.containers)
        .cloned()
        .unwrap_or_default();

    for container in containers {
        let env_vars = container.env.unwrap_or_default();

        for env_var in &env_vars {
            let env_name = &env_var.name;

            // Check each pattern
            for pattern in &patterns {
                if pattern.env_names.iter().any(|n| n.eq_ignore_ascii_case(env_name)) {
                    // Found a matching env var
                    info.dependencies.insert(pattern.label_key.to_string());

                    // Try to extract the provider from value
                    if let Some(value) = &env_var.value {
                        if is_internal_url(value) {
                            info.internal = true;
                        }
                        if let Some(provider) = detect_provider(value, &pattern.providers) {
                            if provider != "internal" {
                                info.providers.insert(provider);
                            }
                        }
                    }

                    // Check valueFrom (secrets/configmaps) - can't determine provider
                    // but we know the dependency exists
                    if env_var.value_from.is_some() {
                        debug!(
                            "Env var {} uses valueFrom, cannot determine provider",
                            env_name
                        );
                    }
                }
            }
        }
    }

    info
}

/// Build labels from dependency info
fn build_labels(info: &DependencyInfo) -> BTreeMap<String, String> {
    let mut labels = BTreeMap::new();

    // Add dependency labels
    for dep in &info.dependencies {
        labels.insert(format!("{}/{}", LABEL_PREFIX, dep), "true".to_string());
    }

    // Add internal label
    if info.internal {
        labels.insert(format!("{}/internal", LABEL_PREFIX), "true".to_string());
    }

    // Add provider labels (comma-separated if multiple)
    if !info.providers.is_empty() {
        let providers: Vec<_> = info.providers.iter().cloned().collect();
        labels.insert(
            format!("{}/providers", LABEL_PREFIX),
            providers.join(","),
        );
    }

    labels
}

/// Reconciles Deployments and adds dependency labels
pub async fn reconcile(
    deployment: Arc<Deployment>,
    ctx: Arc<Context>,
) -> std::result::Result<Action, OperatorError> {
    let name = deployment.name_any();
    let namespace = deployment.namespace().unwrap_or_else(|| "default".to_string());

    // Check if deployment opts-in to auto-labeling
    let annotations = deployment
        .metadata
        .annotations
        .as_ref()
        .cloned()
        .unwrap_or_default();

    let opt_in = annotations
        .get(OPT_IN_ANNOTATION)
        .map(|v| v == "true" || v == "enabled")
        .unwrap_or(false);

    if !opt_in {
        // Skip deployments without an opt-in annotation
        debug!(
            "Skipping Deployment {}/{} - no opt-in annotation",
            namespace, name
        );
        return Ok(Action::requeue(Duration::from_secs(300)));
    }

    info!("Reconciling Deployment {}/{} for dependency labels", namespace, name);

    // Analyze deployment
    let dep_info = analyze_deployment(&deployment);

    if dep_info.dependencies.is_empty() {
        debug!(
            "No dependencies detected for Deployment {}/{}",
            namespace, name
        );
        return Ok(Action::requeue(Duration::from_secs(300)));
    }

    info!(
        "Detected dependencies for {}/{}: {:?}",
        namespace, name, dep_info
    );

    // Build labels
    let new_labels = build_labels(&dep_info);

    // Get existing labels
    let existing_labels = deployment
        .metadata
        .labels
        .as_ref()
        .cloned()
        .unwrap_or_default();

    // Check if an update is needed
    let needs_update = new_labels.iter().any(|(k, v)| {
        existing_labels.get(k).map(|ev| ev != v).unwrap_or(true)
    });

    if !needs_update {
        debug!(
            "Labels already up-to-date for Deployment {}/{}",
            namespace, name
        );
        return Ok(Action::requeue(Duration::from_secs(300)));
    }

    // Apply labels using a strategic merge patch
    let api: Api<Deployment> = Api::namespaced(ctx.client.clone(), &namespace);

    let patch = serde_json::json!({
        "metadata": {
            "labels": new_labels
        }
    });

    api.patch(
        &name,
        &PatchParams::apply("dependency-labeler.platform.yurikrupnik.com"),
        &Patch::Apply(&patch),
    )
    .await?;

    info!(
        "Updated labels for Deployment {}/{}: {:?}",
        namespace, name, new_labels
    );

    // Requeue for periodic re-evaluation
    Ok(Action::requeue(Duration::from_secs(300)))
}

/// Error policy for the controller
pub fn error_policy(
    deployment: Arc<Deployment>,
    error: &OperatorError,
    _ctx: Arc<Context>,
) -> Action {
    warn!(
        "Reconcile error for Deployment {}: {}",
        deployment.name_any(),
        error
    );

    // Retry with backoff
    Action::requeue(Duration::from_secs(60))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_internal_url() {
        assert!(is_internal_url("localhost:5432"));
        assert!(is_internal_url("postgres.default.svc.cluster.local:5432"));
        assert!(is_internal_url("redis.svc:6379"));
        assert!(is_internal_url("127.0.0.1:5432"));
        assert!(is_internal_url("postgres")); // Just service name

        assert!(!is_internal_url("postgres://user:pass@db.neon.tech/mydb"));
        assert!(!is_internal_url("mongodb+srv://cluster.mongodb.net"));
    }

    #[test]
    fn test_detect_provider() {
        let postgres_providers = &[
            ("neon.tech", "neon"),
            ("supabase", "supabase"),
        ];

        assert_eq!(
            detect_provider("postgres://user:pass@db.neon.tech/mydb", postgres_providers),
            Some("neon".to_string())
        );
        assert_eq!(
            detect_provider("postgres://localhost:5432/mydb", postgres_providers),
            Some("internal".to_string())
        );
    }

    #[test]
    fn test_build_labels() {
        let mut info = DependencyInfo::default();
        info.dependencies.insert("postgres".to_string());
        info.dependencies.insert("redis".to_string());
        info.internal = true;
        info.providers.insert("neon".to_string());

        let labels = build_labels(&info);

        assert_eq!(labels.get("platform.yurikrupnik.com/postgres"), Some(&"true".to_string()));
        assert_eq!(labels.get("platform.yurikrupnik.com/redis"), Some(&"true".to_string()));
        assert_eq!(labels.get("platform.yurikrupnik.com/internal"), Some(&"true".to_string()));
        assert!(labels.get("platform.yurikrupnik.com/providers").is_some());
    }
}
