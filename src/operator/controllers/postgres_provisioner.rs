//! PostgreSQL Provisioner Controller
//!
//! Watches Deployments with the `platform.yurikrupnik.com/postgres` label
//! and automatically provisions/deletes CNPG (CloudNativePG) clusters.
//!
//! Labels:
//! - `platform.yurikrupnik.com/postgres: "true"` - Provision a postgres cluster
//! - `platform.yurikrupnik.com/postgres-provider: "cnpg"` - Use CNPG (default)
//! - `platform.yurikrupnik.com/postgres-provider: "neon"` - Use Neon (Crossplane)
//! - `platform.yurikrupnik.com/postgres-internal: "true"` - Use in-cluster CNPG
//!
//! Annotations (optional configuration):
//! - `platform.yurikrupnik.com/postgres-database: "mydb"` - Database name
//! - `platform.yurikrupnik.com/postgres-storage: "10Gi"` - Storage size
//! - `platform.yurikrupnik.com/postgres-instances: "2"` - Number of replicas

use crate::operator::{Context, OperatorError};
use k8s_openapi::api::apps::v1::Deployment;
use kube::api::{DeleteParams, Patch, PatchParams};
use kube::runtime::controller::Action;
use kube::{Api, ResourceExt};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Label constants
const LABEL_POSTGRES: &str = "platform.yurikrupnik.com/postgres";
const LABEL_PROVIDER: &str = "platform.yurikrupnik.com/postgres-provider";
const LABEL_INTERNAL: &str = "platform.yurikrupnik.com/postgres-internal";

/// Annotation constants for configuration
const ANNO_DATABASE: &str = "platform.yurikrupnik.com/postgres-database";
const ANNO_STORAGE: &str = "platform.yurikrupnik.com/postgres-storage";
const ANNO_INSTANCES: &str = "platform.yurikrupnik.com/postgres-instances";
const ANNO_POOLER: &str = "platform.yurikrupnik.com/postgres-pooler";

/// Field manager for server-side apply
const FIELD_MANAGER: &str = "postgres-provisioner.platform.yurikrupnik.com";

/// CNPG Cluster CRD
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CnpgCluster {
    pub api_version: String,
    pub kind: String,
    pub metadata: CnpgMetadata,
    pub spec: CnpgClusterSpec,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CnpgMetadata {
    pub name: String,
    pub namespace: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub labels: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub owner_references: Vec<OwnerReference>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OwnerReference {
    pub api_version: String,
    pub kind: String,
    pub name: String,
    pub uid: String,
    #[serde(default)]
    pub controller: bool,
    #[serde(default)]
    pub block_owner_deletion: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CnpgClusterSpec {
    pub instances: i32,
    pub storage: CnpgStorage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bootstrap: Option<CnpgBootstrap>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub postgresql: Option<CnpgPostgresql>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub monitoring: Option<CnpgMonitoring>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pooler: Option<CnpgPooler>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CnpgStorage {
    pub size: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CnpgBootstrap {
    pub initdb: CnpgInitdb,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CnpgInitdb {
    pub database: String,
    pub owner: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CnpgPostgresql {
    pub parameters: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CnpgMonitoring {
    pub enable_pod_monitor: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CnpgPooler {
    pub instances: i32,
    #[serde(rename = "type")]
    pub pooler_type: String,
    pub pgbouncer: CnpgPgBouncer,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CnpgPgBouncer {
    pub pool_mode: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub parameters: BTreeMap<String, String>,
}

/// Configuration extracted from deployment labels/annotations
#[derive(Debug, Clone)]
struct PostgresConfig {
    enabled: bool,
    provider: String,
    internal: bool,
    database: String,
    storage: String,
    instances: i32,
    pooler_enabled: bool,
}

impl PostgresConfig {
    fn from_deployment(deployment: &Deployment) -> Self {
        let labels = deployment
            .metadata
            .labels
            .as_ref()
            .cloned()
            .unwrap_or_default();

        let annotations = deployment
            .metadata
            .annotations
            .as_ref()
            .cloned()
            .unwrap_or_default();

        let enabled = labels
            .get(LABEL_POSTGRES)
            .map(|v| v == "true")
            .unwrap_or(false);

        let provider = labels
            .get(LABEL_PROVIDER)
            .cloned()
            .unwrap_or_else(|| "cnpg".to_string());

        let internal = labels
            .get(LABEL_INTERNAL)
            .map(|v| v == "true")
            .unwrap_or(true); // Default to internal

        let database = annotations
            .get(ANNO_DATABASE)
            .cloned()
            .unwrap_or_else(|| "app".to_string());

        let storage = annotations
            .get(ANNO_STORAGE)
            .cloned()
            .unwrap_or_else(|| "10Gi".to_string());

        let instances: i32 = annotations
            .get(ANNO_INSTANCES)
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);

        let pooler_enabled = annotations
            .get(ANNO_POOLER)
            .map(|v| v == "true")
            .unwrap_or(false);

        Self {
            enabled,
            provider,
            internal,
            database,
            storage,
            instances,
            pooler_enabled,
        }
    }
}

/// Build CNPG cluster name from deployment
fn cluster_name(deployment_name: &str) -> String {
    format!("{}-postgres", deployment_name)
}

/// Create a CNPG Cluster resource
fn build_cnpg_cluster(
    deployment: &Deployment,
    config: &PostgresConfig,
) -> Result<CnpgCluster, OperatorError> {
    let name = deployment.name_any();
    let namespace = deployment
        .namespace()
        .unwrap_or_else(|| "default".to_string());
    let cluster_name = cluster_name(&name);

    // Build owner reference for garbage collection
    let owner_ref = OwnerReference {
        api_version: "apps/v1".to_string(),
        kind: "Deployment".to_string(),
        name: name.clone(),
        uid: deployment
            .metadata
            .uid
            .clone()
            .ok_or_else(|| OperatorError::Config("Deployment has no UID".into()))?,
        controller: true,
        block_owner_deletion: true,
    };

    let mut labels = BTreeMap::new();
    labels.insert("app.kubernetes.io/name".to_string(), cluster_name.clone());
    labels.insert(
        "app.kubernetes.io/component".to_string(),
        "database".to_string(),
    );
    labels.insert(
        "app.kubernetes.io/managed-by".to_string(),
        "platform-operator".to_string(),
    );
    labels.insert("platform.yurikrupnik.com/postgres".to_string(), "true".to_string());
    labels.insert(
        "platform.yurikrupnik.com/owner".to_string(),
        name.clone(),
    );

    let mut postgres_params = BTreeMap::new();
    postgres_params.insert("max_connections".to_string(), "200".to_string());
    postgres_params.insert("shared_buffers".to_string(), "256MB".to_string());

    // Build pooler config if enabled
    let pooler = if config.pooler_enabled {
        let mut pgbouncer_params = BTreeMap::new();
        pgbouncer_params.insert("max_client_conn".to_string(), "1000".to_string());
        pgbouncer_params.insert("default_pool_size".to_string(), "20".to_string());

        Some(CnpgPooler {
            instances: 1,
            pooler_type: "rw".to_string(),
            pgbouncer: CnpgPgBouncer {
                pool_mode: "transaction".to_string(),
                parameters: pgbouncer_params,
            },
        })
    } else {
        None
    };

    Ok(CnpgCluster {
        api_version: "postgresql.cnpg.io/v1".to_string(),
        kind: "Cluster".to_string(),
        metadata: CnpgMetadata {
            name: cluster_name,
            namespace,
            labels,
            annotations: BTreeMap::new(),
            owner_references: vec![owner_ref],
        },
        spec: CnpgClusterSpec {
            instances: config.instances,
            storage: CnpgStorage {
                size: config.storage.clone(),
            },
            bootstrap: Some(CnpgBootstrap {
                initdb: CnpgInitdb {
                    database: config.database.clone(),
                    owner: "app".to_string(),
                },
            }),
            postgresql: Some(CnpgPostgresql {
                parameters: postgres_params,
            }),
            monitoring: Some(CnpgMonitoring {
                enable_pod_monitor: true,
            }),
            pooler,
        },
    })
}

/// Apply CNPG cluster using dynamic API
async fn apply_cnpg_cluster(
    client: &kube::Client,
    cluster: &CnpgCluster,
) -> Result<(), OperatorError> {
    let gvk = kube::api::GroupVersionKind {
        group: "postgresql.cnpg.io".to_string(),
        version: "v1".to_string(),
        kind: "Cluster".to_string(),
    };

    let api_resource = kube::discovery::ApiResource::from_gvk(&gvk);
    let api: Api<kube::api::DynamicObject> =
        Api::namespaced_with(client.clone(), &cluster.metadata.namespace, &api_resource);

    let obj: kube::api::DynamicObject = serde_json::from_value(serde_json::to_value(cluster)?)?;

    let patch_params = PatchParams::apply(FIELD_MANAGER);
    api.patch(&cluster.metadata.name, &patch_params, &Patch::Apply(&obj))
        .await?;

    info!(
        "Applied CNPG Cluster {}/{}",
        cluster.metadata.namespace, cluster.metadata.name
    );

    Ok(())
}

/// Delete CNPG cluster
async fn delete_cnpg_cluster(
    client: &kube::Client,
    namespace: &str,
    name: &str,
) -> Result<(), OperatorError> {
    let gvk = kube::api::GroupVersionKind {
        group: "postgresql.cnpg.io".to_string(),
        version: "v1".to_string(),
        kind: "Cluster".to_string(),
    };

    let api_resource = kube::discovery::ApiResource::from_gvk(&gvk);
    let api: Api<kube::api::DynamicObject> =
        Api::namespaced_with(client.clone(), namespace, &api_resource);

    match api.delete(name, &DeleteParams::default()).await {
        Ok(_) => {
            info!("Deleted CNPG Cluster {}/{}", namespace, name);
            Ok(())
        }
        Err(kube::Error::Api(err)) if err.code == 404 => {
            debug!("CNPG Cluster {}/{} not found, nothing to delete", namespace, name);
            Ok(())
        }
        Err(e) => Err(e.into()),
    }
}

/// Check if CNPG cluster exists
async fn cnpg_cluster_exists(
    client: &kube::Client,
    namespace: &str,
    name: &str,
) -> Result<bool, OperatorError> {
    let gvk = kube::api::GroupVersionKind {
        group: "postgresql.cnpg.io".to_string(),
        version: "v1".to_string(),
        kind: "Cluster".to_string(),
    };

    let api_resource = kube::discovery::ApiResource::from_gvk(&gvk);
    let api: Api<kube::api::DynamicObject> =
        Api::namespaced_with(client.clone(), namespace, &api_resource);

    match api.get(name).await {
        Ok(_) => Ok(true),
        Err(kube::Error::Api(err)) if err.code == 404 => Ok(false),
        Err(e) => Err(e.into()),
    }
}

/// Reconciles Deployments and provisions/deletes CNPG clusters based on labels
pub async fn reconcile(
    deployment: Arc<Deployment>,
    ctx: Arc<Context>,
) -> Result<Action, OperatorError> {
    let name = deployment.name_any();
    let namespace = deployment
        .namespace()
        .unwrap_or_else(|| "default".to_string());

    // Extract postgres configuration from labels/annotations
    let config = PostgresConfig::from_deployment(&deployment);
    let cluster_name = cluster_name(&name);

    // Check if deployment is being deleted
    let is_deleting = deployment.metadata.deletion_timestamp.is_some();

    if is_deleting {
        // Deployment is being deleted - CNPG cluster will be cleaned up via owner reference
        debug!(
            "Deployment {}/{} is being deleted, CNPG cluster will be garbage collected",
            namespace, name
        );
        return Ok(Action::await_change());
    }

    // Check if postgres is requested
    if !config.enabled {
        // Postgres not requested - check if we need to clean up an existing cluster
        let exists = cnpg_cluster_exists(&ctx.client, &namespace, &cluster_name).await?;
        if exists {
            info!(
                "Postgres label removed from {}/{}, deleting CNPG cluster",
                namespace, name
            );
            delete_cnpg_cluster(&ctx.client, &namespace, &cluster_name).await?;
        }
        return Ok(Action::requeue(Duration::from_secs(300)));
    }

    // Only handle internal CNPG provider for now
    if config.provider != "cnpg" && !config.internal {
        debug!(
            "Deployment {}/{} uses external postgres provider '{}', skipping CNPG provisioning",
            namespace, name, config.provider
        );
        return Ok(Action::requeue(Duration::from_secs(300)));
    }

    info!(
        "Provisioning CNPG cluster for Deployment {}/{}: database={}, storage={}, instances={}",
        namespace, name, config.database, config.storage, config.instances
    );

    // Build and apply CNPG cluster
    let cluster = build_cnpg_cluster(&deployment, &config)?;
    apply_cnpg_cluster(&ctx.client, &cluster).await?;

    // Requeue to check cluster status
    Ok(Action::requeue(Duration::from_secs(60)))
}

/// Error policy for the controller
pub fn error_policy(
    deployment: Arc<Deployment>,
    error: &OperatorError,
    _ctx: Arc<Context>,
) -> Action {
    warn!(
        "PostgresProvisioner reconcile error for Deployment {}: {}",
        deployment.name_any(),
        error
    );

    // Retry with backoff
    Action::requeue(Duration::from_secs(60))
}

#[cfg(test)]
mod tests {
    use super::*;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

    fn create_test_deployment(labels: BTreeMap<String, String>) -> Deployment {
        Deployment {
            metadata: ObjectMeta {
                name: Some("test-app".to_string()),
                namespace: Some("default".to_string()),
                uid: Some("test-uid-123".to_string()),
                labels: Some(labels),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[test]
    fn test_postgres_config_disabled() {
        let deployment = create_test_deployment(BTreeMap::new());
        let config = PostgresConfig::from_deployment(&deployment);
        assert!(!config.enabled);
    }

    #[test]
    fn test_postgres_config_enabled() {
        let mut labels = BTreeMap::new();
        labels.insert(LABEL_POSTGRES.to_string(), "true".to_string());

        let deployment = create_test_deployment(labels);
        let config = PostgresConfig::from_deployment(&deployment);

        assert!(config.enabled);
        assert_eq!(config.provider, "cnpg");
        assert!(config.internal);
        assert_eq!(config.database, "app");
        assert_eq!(config.storage, "10Gi");
        assert_eq!(config.instances, 1);
    }

    #[test]
    fn test_cluster_name() {
        assert_eq!(cluster_name("my-app"), "my-app-postgres");
    }
}
