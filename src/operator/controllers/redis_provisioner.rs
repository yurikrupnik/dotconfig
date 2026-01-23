//! Redis Provisioner Controller
//!
//! Watches Deployments with the `platform.yurikrupnik.com/redis` label
//! and automatically provisions/deletes Redis clusters.
//!
//! Supported Providers (fallback chain):
//! 1. Spotahome Redis Operator (CNCF, preferred)
//! 2. Dragonfly (Redis-compatible, high performance)
//! 3. KeyDB (Redis-compatible, multithreaded)
//! 4. Simple Deployment (dev/test)
//! 5. Bitnami Helm Chart
//! 6. AWS ElastiCache (via Crossplane)
//!
//! Labels:
//! - `platform.yurikrupnik.com/redis: "true"` - Provision a Redis cluster
//! - `platform.yurikrupnik.com/redis-provider: "auto"` - Auto-detect (default)
//! - `platform.yurikrupnik.com/redis-provider: "spotahome"` - Use Spotahome operator
//! - `platform.yurikrupnik.com/redis-provider: "dragonfly"` - Use Dragonfly
//! - `platform.yurikrupnik.com/redis-provider: "keydb"` - Use KeyDB
//! - `platform.yurikrupnik.com/redis-provider: "deployment"` - Simple deployment
//!
//! Annotations (optional configuration):
//! - `platform.yurikrupnik.com/redis-storage: "5Gi"` - Storage size
//! - `platform.yurikrupnik.com/redis-replicas: "3"` - Number of replicas
//! - `platform.yurikrupnik.com/redis-mode: "cluster"` - Mode: standalone, sentinel, cluster
//! - `platform.yurikrupnik.com/redis-password-secret: "my-secret"` - Existing password secret
//! - `platform.yurikrupnik.com/redis-memory: "256Mi"` - Memory limit
//! - `platform.yurikrupnik.com/redis-version: "7.2"` - Redis version

use crate::operator::{Context, OperatorError};
use k8s_openapi::api::apps::v1::{Deployment, StatefulSet, StatefulSetSpec};
use k8s_openapi::api::core::v1::{
    Container, ContainerPort, EnvVar, PersistentVolumeClaim, PersistentVolumeClaimSpec,
    PodSpec, PodTemplateSpec, ResourceRequirements, Service, ServicePort, ServiceSpec,
    VolumeResourceRequirements,
};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, ObjectMeta};
use k8s_openapi::apimachinery::pkg::util::intstr::IntOrString;
use kube::api::{DeleteParams, Patch, PatchParams, PostParams};
use kube::runtime::controller::Action;
use kube::{Api, ResourceExt};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Label constants
const LABEL_REDIS: &str = "platform.yurikrupnik.com/redis";
const LABEL_PROVIDER: &str = "platform.yurikrupnik.com/redis-provider";

/// Annotation constants for configuration
const ANNO_STORAGE: &str = "platform.yurikrupnik.com/redis-storage";
const ANNO_REPLICAS: &str = "platform.yurikrupnik.com/redis-replicas";
const ANNO_MODE: &str = "platform.yurikrupnik.com/redis-mode";
const ANNO_PASSWORD_SECRET: &str = "platform.yurikrupnik.com/redis-password-secret";
const ANNO_MEMORY: &str = "platform.yurikrupnik.com/redis-memory";
const ANNO_VERSION: &str = "platform.yurikrupnik.com/redis-version";

/// Field manager for server-side apply
const FIELD_MANAGER: &str = "redis-provisioner.platform.yurikrupnik.com";

/// Redis deployment mode
#[derive(Debug, Clone, PartialEq)]
pub enum RedisMode {
    Standalone,
    Sentinel,
    Cluster,
}

impl RedisMode {
    fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "sentinel" => RedisMode::Sentinel,
            "cluster" => RedisMode::Cluster,
            _ => RedisMode::Standalone,
        }
    }
}

/// Redis provider type
#[derive(Debug, Clone, PartialEq)]
pub enum RedisProvider {
    Auto,
    Spotahome,
    Dragonfly,
    KeyDB,
    Deployment,
    Helm,
    ElastiCache,
}

impl RedisProvider {
    fn from_str(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "spotahome" => RedisProvider::Spotahome,
            "dragonfly" => RedisProvider::Dragonfly,
            "keydb" => RedisProvider::KeyDB,
            "deployment" | "simple" => RedisProvider::Deployment,
            "helm" | "bitnami" => RedisProvider::Helm,
            "elasticache" | "aws" => RedisProvider::ElastiCache,
            _ => RedisProvider::Auto,
        }
    }
}

/// Spotahome RedisFailover CRD
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisFailover {
    pub api_version: String,
    pub kind: String,
    pub metadata: ResourceMetadata,
    pub spec: RedisFailoverSpec,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ResourceMetadata {
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
pub struct RedisFailoverSpec {
    pub sentinel: SentinelSpec,
    pub redis: RedisSpec,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth: Option<AuthSpec>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SentinelSpec {
    pub replicas: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourceSpec>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RedisSpec {
    pub replicas: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<ResourceSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage: Option<StorageSpec>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceSpec {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requests: Option<BTreeMap<String, String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub limits: Option<BTreeMap<String, String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageSpec {
    pub persistent_volume_claim: PvcSpec,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PvcSpec {
    pub spec: PvcSpecInner,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PvcSpecInner {
    pub access_modes: Vec<String>,
    pub resources: PvcResources,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PvcResources {
    pub requests: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthSpec {
    pub secret_path: String,
}

/// Configuration extracted from deployment labels/annotations
#[derive(Debug, Clone)]
pub struct RedisConfig {
    pub enabled: bool,
    pub provider: RedisProvider,
    pub mode: RedisMode,
    pub storage: String,
    pub replicas: i32,
    pub memory: String,
    pub version: String,
    pub password_secret: Option<String>,
}

impl RedisConfig {
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
            .get(LABEL_REDIS)
            .map(|v| v == "true")
            .unwrap_or(false);

        let provider = labels
            .get(LABEL_PROVIDER)
            .map(|v| RedisProvider::from_str(v))
            .unwrap_or(RedisProvider::Auto);

        let mode = annotations
            .get(ANNO_MODE)
            .map(|v| RedisMode::from_str(v))
            .unwrap_or(RedisMode::Standalone);

        let storage = annotations
            .get(ANNO_STORAGE)
            .cloned()
            .unwrap_or_else(|| "1Gi".to_string());

        let replicas: i32 = annotations
            .get(ANNO_REPLICAS)
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);

        let memory = annotations
            .get(ANNO_MEMORY)
            .cloned()
            .unwrap_or_else(|| "256Mi".to_string());

        let version = annotations
            .get(ANNO_VERSION)
            .cloned()
            .unwrap_or_else(|| "7.2".to_string());

        let password_secret = annotations.get(ANNO_PASSWORD_SECRET).cloned();

        Self {
            enabled,
            provider,
            mode,
            storage,
            replicas,
            memory,
            version,
            password_secret,
        }
    }
}

/// Build Redis resource name from deployment
fn redis_name(deployment_name: &str) -> String {
    format!("{}-redis", deployment_name)
}

/// Build owner reference for garbage collection
fn build_owner_reference(deployment: &Deployment) -> Result<OwnerReference, OperatorError> {
    Ok(OwnerReference {
        api_version: "apps/v1".to_string(),
        kind: "Deployment".to_string(),
        name: deployment.name_any(),
        uid: deployment
            .metadata
            .uid
            .clone()
            .ok_or_else(|| OperatorError::Config("Deployment has no UID".into()))?,
        controller: true,
        block_owner_deletion: true,
    })
}

/// Build common labels
fn build_labels(name: &str, deployment_name: &str) -> BTreeMap<String, String> {
    let mut labels = BTreeMap::new();
    labels.insert("app.kubernetes.io/name".to_string(), name.to_string());
    labels.insert(
        "app.kubernetes.io/component".to_string(),
        "cache".to_string(),
    );
    labels.insert(
        "app.kubernetes.io/managed-by".to_string(),
        "platform-operator".to_string(),
    );
    labels.insert(
        "platform.yurikrupnik.com/redis".to_string(),
        "true".to_string(),
    );
    labels.insert(
        "platform.yurikrupnik.com/owner".to_string(),
        deployment_name.to_string(),
    );
    labels
}

/// Build Spotahome RedisFailover resource
fn build_redis_failover(
    deployment: &Deployment,
    config: &RedisConfig,
) -> Result<RedisFailover, OperatorError> {
    let name = deployment.name_any();
    let namespace = deployment
        .namespace()
        .unwrap_or_else(|| "default".to_string());
    let redis_name = redis_name(&name);

    let owner_ref = build_owner_reference(deployment)?;
    let labels = build_labels(&redis_name, &name);

    let mut memory_limits = BTreeMap::new();
    memory_limits.insert("memory".to_string(), config.memory.clone());

    let mut storage_requests = BTreeMap::new();
    storage_requests.insert("storage".to_string(), config.storage.clone());

    let sentinel_replicas = if config.mode == RedisMode::Sentinel { 3 } else { 0 };
    let redis_replicas = config.replicas.max(1);

    let auth = config.password_secret.as_ref().map(|secret| AuthSpec {
        secret_path: format!("{}/password", secret),
    });

    Ok(RedisFailover {
        api_version: "databases.spotahome.com/v1".to_string(),
        kind: "RedisFailover".to_string(),
        metadata: ResourceMetadata {
            name: redis_name,
            namespace,
            labels,
            annotations: BTreeMap::new(),
            owner_references: vec![owner_ref],
        },
        spec: RedisFailoverSpec {
            sentinel: SentinelSpec {
                replicas: sentinel_replicas,
                resources: Some(ResourceSpec {
                    limits: Some(memory_limits.clone()),
                    requests: None,
                }),
            },
            redis: RedisSpec {
                replicas: redis_replicas,
                resources: Some(ResourceSpec {
                    limits: Some(memory_limits),
                    requests: None,
                }),
                storage: Some(StorageSpec {
                    persistent_volume_claim: PvcSpec {
                        spec: PvcSpecInner {
                            access_modes: vec!["ReadWriteOnce".to_string()],
                            resources: PvcResources {
                                requests: storage_requests,
                            },
                        },
                    },
                }),
                image: Some(format!("redis:{}", config.version)),
            },
            auth,
        },
    })
}

/// Build a simple Redis StatefulSet for dev/test
fn build_simple_redis(
    deployment: &Deployment,
    config: &RedisConfig,
) -> Result<(StatefulSet, Service), OperatorError> {
    let name = deployment.name_any();
    let namespace = deployment
        .namespace()
        .unwrap_or_else(|| "default".to_string());
    let redis_name = redis_name(&name);

    let owner_ref = k8s_openapi::apimachinery::pkg::apis::meta::v1::OwnerReference {
        api_version: "apps/v1".to_string(),
        kind: "Deployment".to_string(),
        name: name.clone(),
        uid: deployment
            .metadata
            .uid
            .clone()
            .ok_or_else(|| OperatorError::Config("Deployment has no UID".into()))?,
        controller: Some(true),
        block_owner_deletion: Some(true),
    };

    let mut labels = BTreeMap::new();
    labels.insert("app".to_string(), redis_name.clone());
    labels.insert(
        "app.kubernetes.io/name".to_string(),
        redis_name.clone(),
    );
    labels.insert(
        "app.kubernetes.io/component".to_string(),
        "cache".to_string(),
    );
    labels.insert(
        "platform.yurikrupnik.com/redis".to_string(),
        "true".to_string(),
    );
    labels.insert(
        "platform.yurikrupnik.com/owner".to_string(),
        name.clone(),
    );

    let mut env_vars = vec![];
    if let Some(secret) = &config.password_secret {
        env_vars.push(EnvVar {
            name: "REDIS_PASSWORD".to_string(),
            value_from: Some(k8s_openapi::api::core::v1::EnvVarSource {
                secret_key_ref: Some(k8s_openapi::api::core::v1::SecretKeySelector {
                    name: secret.clone(),
                    key: "password".to_string(),
                    optional: Some(false),
                }),
                ..Default::default()
            }),
            ..Default::default()
        });
    }

    let mut resources = BTreeMap::new();
    resources.insert("memory".to_string(), Quantity(config.memory.clone()));

    let container = Container {
        name: "redis".to_string(),
        image: Some(format!("redis:{}-alpine", config.version)),
        ports: Some(vec![ContainerPort {
            container_port: 6379,
            name: Some("redis".to_string()),
            ..Default::default()
        }]),
        env: if env_vars.is_empty() { None } else { Some(env_vars) },
        resources: Some(ResourceRequirements {
            limits: Some(resources.clone()),
            requests: Some(resources),
            ..Default::default()
        }),
        command: if config.password_secret.is_some() {
            Some(vec![
                "redis-server".to_string(),
                "--requirepass".to_string(),
                "$(REDIS_PASSWORD)".to_string(),
            ])
        } else {
            None
        },
        ..Default::default()
    };

    let statefulset = StatefulSet {
        metadata: ObjectMeta {
            name: Some(redis_name.clone()),
            namespace: Some(namespace.clone()),
            labels: Some(labels.clone()),
            owner_references: Some(vec![owner_ref.clone()]),
            ..Default::default()
        },
        spec: Some(StatefulSetSpec {
            replicas: Some(config.replicas),
            selector: LabelSelector {
                match_labels: Some({
                    let mut m = BTreeMap::new();
                    m.insert("app".to_string(), redis_name.clone());
                    m
                }),
                ..Default::default()
            },
            service_name: Some(redis_name.clone()),
            template: PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    labels: Some({
                        let mut m = BTreeMap::new();
                        m.insert("app".to_string(), redis_name.clone());
                        m
                    }),
                    ..Default::default()
                }),
                spec: Some(PodSpec {
                    containers: vec![container],
                    ..Default::default()
                }),
            },
            volume_claim_templates: Some(vec![PersistentVolumeClaim {
                metadata: ObjectMeta {
                    name: Some("data".to_string()),
                    ..Default::default()
                },
                spec: Some(PersistentVolumeClaimSpec {
                    access_modes: Some(vec!["ReadWriteOnce".to_string()]),
                    resources: Some(VolumeResourceRequirements {
                        requests: Some({
                            let mut m = BTreeMap::new();
                            m.insert("storage".to_string(), Quantity(config.storage.clone()));
                            m
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }]),
            ..Default::default()
        }),
        ..Default::default()
    };

    let service = Service {
        metadata: ObjectMeta {
            name: Some(redis_name.clone()),
            namespace: Some(namespace),
            labels: Some(labels),
            owner_references: Some(vec![owner_ref]),
            ..Default::default()
        },
        spec: Some(ServiceSpec {
            selector: Some({
                let mut m = BTreeMap::new();
                m.insert("app".to_string(), redis_name);
                m
            }),
            ports: Some(vec![ServicePort {
                port: 6379,
                target_port: Some(IntOrString::Int(6379)),
                name: Some("redis".to_string()),
                ..Default::default()
            }]),
            ..Default::default()
        }),
        ..Default::default()
    };

    Ok((statefulset, service))
}

/// Check if Spotahome Redis Operator is available
async fn spotahome_available(client: &kube::Client) -> bool {
    let gvk = kube::api::GroupVersionKind {
        group: "databases.spotahome.com".to_string(),
        version: "v1".to_string(),
        kind: "RedisFailover".to_string(),
    };

    let api_resource = kube::discovery::ApiResource::from_gvk(&gvk);
    let api: Api<kube::api::DynamicObject> = Api::all_with(client.clone(), &api_resource);

    api.list(&Default::default()).await.is_ok()
}

/// Apply Spotahome RedisFailover using dynamic API
async fn apply_redis_failover(
    client: &kube::Client,
    redis: &RedisFailover,
) -> Result<(), OperatorError> {
    let gvk = kube::api::GroupVersionKind {
        group: "databases.spotahome.com".to_string(),
        version: "v1".to_string(),
        kind: "RedisFailover".to_string(),
    };

    let api_resource = kube::discovery::ApiResource::from_gvk(&gvk);
    let api: Api<kube::api::DynamicObject> =
        Api::namespaced_with(client.clone(), &redis.metadata.namespace, &api_resource);

    let obj: kube::api::DynamicObject = serde_json::from_value(serde_json::to_value(redis)?)?;

    let patch_params = PatchParams::apply(FIELD_MANAGER);
    api.patch(&redis.metadata.name, &patch_params, &Patch::Apply(&obj))
        .await?;

    info!(
        "Applied RedisFailover {}/{}",
        redis.metadata.namespace, redis.metadata.name
    );

    Ok(())
}

/// Apply simple Redis StatefulSet and Service
async fn apply_simple_redis(
    client: &kube::Client,
    statefulset: &StatefulSet,
    service: &Service,
) -> Result<(), OperatorError> {
    let namespace = statefulset
        .metadata
        .namespace
        .as_ref()
        .cloned()
        .unwrap_or_else(|| "default".to_string());
    let name = statefulset
        .metadata
        .name
        .as_ref()
        .cloned()
        .unwrap_or_default();

    // Apply Service first
    let svc_api: Api<Service> = Api::namespaced(client.clone(), &namespace);
    let svc_name = service.metadata.name.as_ref().cloned().unwrap_or_default();

    match svc_api.get(&svc_name).await {
        Ok(_) => {
            svc_api
                .patch(
                    &svc_name,
                    &PatchParams::apply(FIELD_MANAGER),
                    &Patch::Apply(service),
                )
                .await?;
        }
        Err(kube::Error::Api(err)) if err.code == 404 => {
            svc_api.create(&PostParams::default(), service).await?;
        }
        Err(e) => return Err(e.into()),
    }

    // Apply StatefulSet
    let sts_api: Api<StatefulSet> = Api::namespaced(client.clone(), &namespace);

    match sts_api.get(&name).await {
        Ok(_) => {
            // StatefulSet exists - volumeClaimTemplates are immutable, so create a version without them for patching
            let mut patch_sts = statefulset.clone();
            if let Some(ref mut spec) = patch_sts.spec {
                spec.volume_claim_templates = None;
            }
            sts_api
                .patch(
                    &name,
                    &PatchParams::apply(FIELD_MANAGER),
                    &Patch::Apply(patch_sts),
                )
                .await?;
        }
        Err(kube::Error::Api(err)) if err.code == 404 => {
            sts_api.create(&PostParams::default(), statefulset).await?;
        }
        Err(e) => return Err(e.into()),
    }

    info!("Applied simple Redis StatefulSet {}/{}", namespace, name);

    Ok(())
}

/// Delete Redis resources
async fn delete_redis_resources(
    client: &kube::Client,
    namespace: &str,
    name: &str,
) -> Result<(), OperatorError> {
    // Try to delete Spotahome RedisFailover
    let gvk = kube::api::GroupVersionKind {
        group: "databases.spotahome.com".to_string(),
        version: "v1".to_string(),
        kind: "RedisFailover".to_string(),
    };

    let api_resource = kube::discovery::ApiResource::from_gvk(&gvk);
    let api: Api<kube::api::DynamicObject> =
        Api::namespaced_with(client.clone(), namespace, &api_resource);

    match api.delete(name, &DeleteParams::default()).await {
        Ok(_) => info!("Deleted RedisFailover {}/{}", namespace, name),
        Err(kube::Error::Api(err)) if err.code == 404 => {
            debug!("RedisFailover {}/{} not found", namespace, name);
        }
        Err(e) => warn!("Failed to delete RedisFailover: {}", e),
    }

    // Try to delete StatefulSet
    let sts_api: Api<StatefulSet> = Api::namespaced(client.clone(), namespace);
    match sts_api.delete(name, &DeleteParams::default()).await {
        Ok(_) => info!("Deleted StatefulSet {}/{}", namespace, name),
        Err(kube::Error::Api(err)) if err.code == 404 => {
            debug!("StatefulSet {}/{} not found", namespace, name);
        }
        Err(e) => warn!("Failed to delete StatefulSet: {}", e),
    }

    // Try to delete Service
    let svc_api: Api<Service> = Api::namespaced(client.clone(), namespace);
    match svc_api.delete(name, &DeleteParams::default()).await {
        Ok(_) => info!("Deleted Service {}/{}", namespace, name),
        Err(kube::Error::Api(err)) if err.code == 404 => {
            debug!("Service {}/{} not found", namespace, name);
        }
        Err(e) => warn!("Failed to delete Service: {}", e),
    }

    Ok(())
}

/// Check if Redis resources exist
async fn redis_exists(client: &kube::Client, namespace: &str, name: &str) -> bool {
    // Check for Spotahome RedisFailover
    let gvk = kube::api::GroupVersionKind {
        group: "databases.spotahome.com".to_string(),
        version: "v1".to_string(),
        kind: "RedisFailover".to_string(),
    };

    let api_resource = kube::discovery::ApiResource::from_gvk(&gvk);
    let api: Api<kube::api::DynamicObject> =
        Api::namespaced_with(client.clone(), namespace, &api_resource);

    if api.get(name).await.is_ok() {
        return true;
    }

    // Check for StatefulSet
    let sts_api: Api<StatefulSet> = Api::namespaced(client.clone(), namespace);
    sts_api.get(name).await.is_ok()
}

/// Reconciles Deployments and provisions/deletes Redis clusters based on labels
pub async fn reconcile(
    deployment: Arc<Deployment>,
    ctx: Arc<Context>,
) -> Result<Action, OperatorError> {
    let name = deployment.name_any();
    let namespace = deployment
        .namespace()
        .unwrap_or_else(|| "default".to_string());

    // Extract Redis configuration from labels/annotations
    let config = RedisConfig::from_deployment(&deployment);
    let redis_name = redis_name(&name);

    // Check if deployment is being deleted
    let is_deleting = deployment.metadata.deletion_timestamp.is_some();

    if is_deleting {
        debug!(
            "Deployment {}/{} is being deleted, Redis will be garbage collected",
            namespace, name
        );
        return Ok(Action::await_change());
    }

    // Check if Redis is requested
    if !config.enabled {
        // Redis not requested - check if we need to clean up existing resources
        if redis_exists(&ctx.client, &namespace, &redis_name).await {
            info!(
                "Redis label removed from {}/{}, deleting Redis resources",
                namespace, name
            );
            delete_redis_resources(&ctx.client, &namespace, &redis_name).await?;
        }
        return Ok(Action::requeue(Duration::from_secs(300)));
    }

    info!(
        "Provisioning Redis for Deployment {}/{}: mode={:?}, storage={}, replicas={}, memory={}",
        namespace, name, config.mode, config.storage, config.replicas, config.memory
    );

    // Determine which provider to use
    let provider = match config.provider {
        RedisProvider::Auto => {
            // Auto-detect: prefer Spotahome if available, otherwise simple deployment
            if spotahome_available(&ctx.client).await {
                info!("Auto-detected Spotahome Redis Operator");
                RedisProvider::Spotahome
            } else {
                info!("Spotahome not available, using simple deployment");
                RedisProvider::Deployment
            }
        }
        ref other => other.clone(),
    };

    match provider {
        RedisProvider::Spotahome => {
            let redis = build_redis_failover(&deployment, &config)?;
            apply_redis_failover(&ctx.client, &redis).await?;
        }
        RedisProvider::Deployment => {
            let (statefulset, service) = build_simple_redis(&deployment, &config)?;
            apply_simple_redis(&ctx.client, &statefulset, &service).await?;
        }
        RedisProvider::Dragonfly => {
            // TODO: Implement Dragonfly provisioning
            warn!("Dragonfly provider not yet implemented, falling back to simple deployment");
            let (statefulset, service) = build_simple_redis(&deployment, &config)?;
            apply_simple_redis(&ctx.client, &statefulset, &service).await?;
        }
        RedisProvider::KeyDB => {
            // TODO: Implement KeyDB provisioning
            warn!("KeyDB provider not yet implemented, falling back to simple deployment");
            let (statefulset, service) = build_simple_redis(&deployment, &config)?;
            apply_simple_redis(&ctx.client, &statefulset, &service).await?;
        }
        _ => {
            warn!(
                "Provider {:?} not yet implemented, falling back to simple deployment",
                provider
            );
            let (statefulset, service) = build_simple_redis(&deployment, &config)?;
            apply_simple_redis(&ctx.client, &statefulset, &service).await?;
        }
    }

    // Requeue to check status
    Ok(Action::requeue(Duration::from_secs(60)))
}

/// Error policy for the controller
pub fn error_policy(
    deployment: Arc<Deployment>,
    error: &OperatorError,
    _ctx: Arc<Context>,
) -> Action {
    warn!(
        "RedisProvisioner reconcile error for Deployment {}: {}",
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
    fn test_redis_config_disabled() {
        let deployment = create_test_deployment(BTreeMap::new());
        let config = RedisConfig::from_deployment(&deployment);
        assert!(!config.enabled);
    }

    #[test]
    fn test_redis_config_enabled() {
        let mut labels = BTreeMap::new();
        labels.insert(LABEL_REDIS.to_string(), "true".to_string());

        let deployment = create_test_deployment(labels);
        let config = RedisConfig::from_deployment(&deployment);

        assert!(config.enabled);
        assert_eq!(config.provider, RedisProvider::Auto);
        assert_eq!(config.mode, RedisMode::Standalone);
        assert_eq!(config.storage, "1Gi");
        assert_eq!(config.replicas, 1);
        assert_eq!(config.memory, "256Mi");
        assert_eq!(config.version, "7.2");
    }

    #[test]
    fn test_redis_config_with_annotations() {
        let mut labels = BTreeMap::new();
        labels.insert(LABEL_REDIS.to_string(), "true".to_string());
        labels.insert(LABEL_PROVIDER.to_string(), "spotahome".to_string());

        let mut deployment = create_test_deployment(labels);
        let mut annotations = BTreeMap::new();
        annotations.insert(ANNO_STORAGE.to_string(), "5Gi".to_string());
        annotations.insert(ANNO_REPLICAS.to_string(), "3".to_string());
        annotations.insert(ANNO_MODE.to_string(), "sentinel".to_string());
        annotations.insert(ANNO_MEMORY.to_string(), "512Mi".to_string());
        deployment.metadata.annotations = Some(annotations);

        let config = RedisConfig::from_deployment(&deployment);

        assert!(config.enabled);
        assert_eq!(config.provider, RedisProvider::Spotahome);
        assert_eq!(config.mode, RedisMode::Sentinel);
        assert_eq!(config.storage, "5Gi");
        assert_eq!(config.replicas, 3);
        assert_eq!(config.memory, "512Mi");
    }

    #[test]
    fn test_redis_name() {
        assert_eq!(redis_name("my-app"), "my-app-redis");
    }

    #[test]
    fn test_redis_mode_from_str() {
        assert_eq!(RedisMode::from_str("standalone"), RedisMode::Standalone);
        assert_eq!(RedisMode::from_str("sentinel"), RedisMode::Sentinel);
        assert_eq!(RedisMode::from_str("cluster"), RedisMode::Cluster);
        assert_eq!(RedisMode::from_str("invalid"), RedisMode::Standalone);
    }

    #[test]
    fn test_redis_provider_from_str() {
        assert_eq!(RedisProvider::from_str("auto"), RedisProvider::Auto);
        assert_eq!(RedisProvider::from_str("spotahome"), RedisProvider::Spotahome);
        assert_eq!(RedisProvider::from_str("dragonfly"), RedisProvider::Dragonfly);
        assert_eq!(RedisProvider::from_str("keydb"), RedisProvider::KeyDB);
        assert_eq!(RedisProvider::from_str("deployment"), RedisProvider::Deployment);
        assert_eq!(RedisProvider::from_str("simple"), RedisProvider::Deployment);
    }
}
