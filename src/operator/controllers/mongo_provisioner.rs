//! MongoDB Provisioner Controller
//!
//! Watches Deployments with the `platform.yurikrupnik.com/mongo` label
//! and provisions MongoDB using a fallback chain:
//!
//! 1. **Percona MongoDB Operator** (CNCF) - preferred for production
//! 2. **MongoDB Community Operator** - alternative CNCF option
//! 3. **Handmade Deployment** - simple single-node for dev/test
//! 4. **Helm Chart** (Bitnami) - full-featured deployment
//! 5. **Cloud Provider** (Atlas via Crossplane) - external managed service
//!
//! Labels:
//! - `platform.yurikrupnik.com/mongo: "true"` - Provision MongoDB
//! - `platform.yurikrupnik.com/mongo-provider: "auto"` - Auto-detect (default)
//! - `platform.yurikrupnik.com/mongo-provider: "percona"` - Force Percona operator
//! - `platform.yurikrupnik.com/mongo-provider: "community"` - Force Community operator
//! - `platform.yurikrupnik.com/mongo-provider: "deployment"` - Force simple deployment
//! - `platform.yurikrupnik.com/mongo-provider: "helm"` - Force Helm chart
//! - `platform.yurikrupnik.com/mongo-provider: "atlas"` - Force MongoDB Atlas
//!
//! Annotations:
//! - `platform.yurikrupnik.com/mongo-database: "mydb"` - Database name
//! - `platform.yurikrupnik.com/mongo-storage: "10Gi"` - Storage size
//! - `platform.yurikrupnik.com/mongo-replicas: "3"` - Replica set members
//! - `platform.yurikrupnik.com/mongo-version: "7.0"` - MongoDB version

use crate::operator::{Context, OperatorError};
use k8s_openapi::api::apps::v1::Deployment as K8sDeployment;
use k8s_openapi::api::core::v1::{
    Container, ContainerPort, PersistentVolumeClaim, PersistentVolumeClaimSpec,
    PodSpec, PodTemplateSpec, ResourceRequirements, Secret, Service, ServicePort, ServiceSpec,
    Volume, VolumeMount, VolumeResourceRequirements,
};
use k8s_openapi::apimachinery::pkg::api::resource::Quantity;
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{LabelSelector, ObjectMeta, OwnerReference};
use kube::api::{DeleteParams, Patch, PatchParams};
use kube::runtime::controller::Action;
use kube::{Api, ResourceExt};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, info, warn};

/// Label constants
const LABEL_MONGO: &str = "platform.yurikrupnik.com/mongo";
const LABEL_PROVIDER: &str = "platform.yurikrupnik.com/mongo-provider";

/// Annotation constants
const ANNO_DATABASE: &str = "platform.yurikrupnik.com/mongo-database";
const ANNO_STORAGE: &str = "platform.yurikrupnik.com/mongo-storage";
const ANNO_REPLICAS: &str = "platform.yurikrupnik.com/mongo-replicas";
const ANNO_VERSION: &str = "platform.yurikrupnik.com/mongo-version";

/// Field manager
const FIELD_MANAGER: &str = "mongo-provisioner.platform.yurikrupnik.com";

/// Available MongoDB provisioning methods
#[derive(Debug, Clone, PartialEq)]
pub enum MongoProvider {
    /// Percona Server for MongoDB Operator
    Percona,
    /// MongoDB Community Kubernetes Operator
    Community,
    /// Simple Deployment (for dev/test)
    Deployment,
    /// Bitnami Helm chart
    Helm,
    /// MongoDB Atlas via Crossplane
    Atlas,
    /// Auto-detect available provider
    Auto,
}

impl From<&str> for MongoProvider {
    fn from(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "percona" | "psmdb" => MongoProvider::Percona,
            "community" | "mongodb-community" => MongoProvider::Community,
            "deployment" | "simple" | "dev" => MongoProvider::Deployment,
            "helm" | "bitnami" => MongoProvider::Helm,
            "atlas" | "cloud" => MongoProvider::Atlas,
            _ => MongoProvider::Auto,
        }
    }
}

/// Percona MongoDB CRD
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerconaServerMongoDB {
    pub api_version: String,
    pub kind: String,
    pub metadata: ObjectMeta,
    pub spec: PerconaMongoSpec,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerconaMongoSpec {
    pub cr_version: String,
    pub image: String,
    pub replsets: Vec<PerconaReplset>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub secrets: Option<PerconaSecrets>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerconaReplset {
    pub name: String,
    pub size: i32,
    pub volume_spec: PerconaVolumeSpec,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerconaVolumeSpec {
    pub pvc: PerconaPVC,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerconaPVC {
    pub storage_class_name: Option<String>,
    pub resources: PerconaPVCResources,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PerconaPVCResources {
    pub requests: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PerconaSecrets {
    pub users: String,
}

/// MongoDB Community Operator CRD
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MongoDBCommunity {
    pub api_version: String,
    pub kind: String,
    pub metadata: ObjectMeta,
    pub spec: MongoDBCommunitySpec,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MongoDBCommunitySpec {
    pub members: i32,
    #[serde(rename = "type")]
    pub db_type: String,
    pub version: String,
    pub security: MongoDBSecurity,
    pub users: Vec<MongoDBUser>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stateful_set: Option<MongoDBStatefulSet>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MongoDBSecurity {
    pub authentication: MongoDBAuth,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MongoDBAuth {
    pub modes: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MongoDBUser {
    pub name: String,
    pub db: String,
    pub password_secret_ref: MongoDBPasswordRef,
    pub roles: Vec<MongoDBRole>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MongoDBPasswordRef {
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MongoDBRole {
    pub name: String,
    pub db: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MongoDBStatefulSet {
    pub spec: MongoDBStatefulSetSpec,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MongoDBStatefulSetSpec {
    pub volume_claim_templates: Vec<serde_json::Value>,
}

/// Configuration from labels/annotations
#[derive(Debug, Clone)]
struct MongoConfig {
    enabled: bool,
    provider: MongoProvider,
    database: String,
    storage: String,
    replicas: i32,
    version: String,
}

impl MongoConfig {
    fn from_deployment(deployment: &K8sDeployment) -> Self {
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
            .get(LABEL_MONGO)
            .map(|v| v == "true")
            .unwrap_or(false);

        let provider: MongoProvider = labels
            .get(LABEL_PROVIDER)
            .map(|s| s.as_str().into())
            .unwrap_or(MongoProvider::Auto);

        let database = annotations
            .get(ANNO_DATABASE)
            .cloned()
            .unwrap_or_else(|| "app".to_string());

        let storage = annotations
            .get(ANNO_STORAGE)
            .cloned()
            .unwrap_or_else(|| "10Gi".to_string());

        let replicas: i32 = annotations
            .get(ANNO_REPLICAS)
            .and_then(|s| s.parse().ok())
            .unwrap_or(1);

        let version = annotations
            .get(ANNO_VERSION)
            .cloned()
            .unwrap_or_else(|| "7.0".to_string());

        Self {
            enabled,
            provider,
            database,
            storage,
            replicas,
            version,
        }
    }
}

/// Check if a CRD is installed in the cluster
async fn crd_exists(client: &kube::Client, group: &str, kind: &str) -> bool {
    let gvk = kube::api::GroupVersionKind {
        group: group.to_string(),
        version: "v1".to_string(),
        kind: kind.to_string(),
    };

    // Try to discover the API resource
    match kube::discovery::oneshot::pinned_kind(client, &gvk).await {
        Ok(_) => true,
        Err(_) => false,
    }
}

/// Detect the best available provider
async fn detect_provider(client: &kube::Client, config: &MongoConfig) -> MongoProvider {
    if config.provider != MongoProvider::Auto {
        return config.provider.clone();
    }

    // Priority order: Percona > Community > Deployment (always available)

    // Check for Percona operator
    if crd_exists(client, "psmdb.percona.com", "PerconaServerMongoDB").await {
        info!("Detected Percona MongoDB Operator - using it");
        return MongoProvider::Percona;
    }

    // Check for MongoDB Community Operator
    if crd_exists(client, "mongodbcommunity.mongodb.com", "MongoDBCommunity").await {
        info!("Detected MongoDB Community Operator - using it");
        return MongoProvider::Community;
    }

    // Check if Helm is available (check for helm-controller or just use deployment)
    // For now, default to simple deployment for dev environments
    info!("No MongoDB operator found - using simple Deployment");
    MongoProvider::Deployment
}

/// Build resource name
fn resource_name(deployment_name: &str) -> String {
    format!("{}-mongo", deployment_name)
}

/// Create owner reference
fn build_owner_ref(deployment: &K8sDeployment) -> Result<OwnerReference, OperatorError> {
    Ok(OwnerReference {
        api_version: "apps/v1".to_string(),
        kind: "Deployment".to_string(),
        name: deployment.name_any(),
        uid: deployment
            .metadata
            .uid
            .clone()
            .ok_or_else(|| OperatorError::Config("Deployment has no UID".into()))?,
        controller: Some(true),
        block_owner_deletion: Some(true),
    })
}

/// Build common labels
fn build_labels(name: &str, owner: &str) -> BTreeMap<String, String> {
    let mut labels = BTreeMap::new();
    labels.insert("app.kubernetes.io/name".to_string(), name.to_string());
    labels.insert("app.kubernetes.io/component".to_string(), "database".to_string());
    labels.insert("app.kubernetes.io/managed-by".to_string(), "platform-operator".to_string());
    labels.insert("platform.yurikrupnik.com/mongo".to_string(), "true".to_string());
    labels.insert("platform.yurikrupnik.com/owner".to_string(), owner.to_string());
    labels
}

/// Create a simple MongoDB deployment (for dev/test)
fn build_simple_deployment(
    deployment: &K8sDeployment,
    config: &MongoConfig,
) -> Result<(K8sDeployment, Service, Secret, PersistentVolumeClaim), OperatorError> {
    let name = resource_name(&deployment.name_any());
    let namespace = deployment.namespace().unwrap_or_else(|| "default".to_string());
    let owner_ref = build_owner_ref(deployment)?;
    let labels = build_labels(&name, &deployment.name_any());
    let secret_name = format!("{}-auth", name);

    // Password secret
    let secret = Secret {
        metadata: ObjectMeta {
            name: Some(secret_name.clone()),
            namespace: Some(namespace.clone()),
            labels: Some(labels.clone()),
            owner_references: Some(vec![owner_ref.clone()]),
            ..Default::default()
        },
        string_data: Some({
            let mut data = BTreeMap::new();
            data.insert("MONGO_INITDB_ROOT_USERNAME".to_string(), "root".to_string());
            data.insert("MONGO_INITDB_ROOT_PASSWORD".to_string(), "changeme".to_string()); // TODO: Generate random
            data.insert("MONGO_INITDB_DATABASE".to_string(), config.database.clone());
            data
        }),
        ..Default::default()
    };

    // PVC
    let pvc = PersistentVolumeClaim {
        metadata: ObjectMeta {
            name: Some(format!("{}-data", name)),
            namespace: Some(namespace.clone()),
            labels: Some(labels.clone()),
            owner_references: Some(vec![owner_ref.clone()]),
            ..Default::default()
        },
        spec: Some(PersistentVolumeClaimSpec {
            access_modes: Some(vec!["ReadWriteOnce".to_string()]),
            resources: Some(VolumeResourceRequirements {
                requests: Some({
                    let mut req = BTreeMap::new();
                    req.insert("storage".to_string(), Quantity(config.storage.clone()));
                    req
                }),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };

    // Deployment
    let mongo_deployment = K8sDeployment {
        metadata: ObjectMeta {
            name: Some(name.clone()),
            namespace: Some(namespace.clone()),
            labels: Some(labels.clone()),
            owner_references: Some(vec![owner_ref.clone()]),
            ..Default::default()
        },
        spec: Some(k8s_openapi::api::apps::v1::DeploymentSpec {
            replicas: Some(1), // Simple deployment is single-node
            selector: LabelSelector {
                match_labels: Some({
                    let mut sel = BTreeMap::new();
                    sel.insert("app".to_string(), name.clone());
                    sel
                }),
                ..Default::default()
            },
            template: PodTemplateSpec {
                metadata: Some(ObjectMeta {
                    labels: Some({
                        let mut l = labels.clone();
                        l.insert("app".to_string(), name.clone());
                        l
                    }),
                    ..Default::default()
                }),
                spec: Some(PodSpec {
                    containers: vec![Container {
                        name: "mongodb".to_string(),
                        image: Some(format!("mongo:{}", config.version)),
                        ports: Some(vec![ContainerPort {
                            container_port: 27017,
                            name: Some("mongodb".to_string()),
                            ..Default::default()
                        }]),
                        env_from: Some(vec![k8s_openapi::api::core::v1::EnvFromSource {
                            secret_ref: Some(k8s_openapi::api::core::v1::SecretEnvSource {
                                name: secret_name.clone(),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }]),
                        volume_mounts: Some(vec![VolumeMount {
                            name: "data".to_string(),
                            mount_path: "/data/db".to_string(),
                            ..Default::default()
                        }]),
                        resources: Some(ResourceRequirements {
                            requests: Some({
                                let mut req = BTreeMap::new();
                                req.insert("memory".to_string(), Quantity("256Mi".to_string()));
                                req.insert("cpu".to_string(), Quantity("100m".to_string()));
                                req
                            }),
                            limits: Some({
                                let mut lim = BTreeMap::new();
                                lim.insert("memory".to_string(), Quantity("512Mi".to_string()));
                                lim
                            }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }],
                    volumes: Some(vec![Volume {
                        name: "data".to_string(),
                        persistent_volume_claim: Some(
                            k8s_openapi::api::core::v1::PersistentVolumeClaimVolumeSource {
                                claim_name: format!("{}-data", name),
                                ..Default::default()
                            },
                        ),
                        ..Default::default()
                    }]),
                    ..Default::default()
                }),
            },
            ..Default::default()
        }),
        ..Default::default()
    };

    // Service
    let service = Service {
        metadata: ObjectMeta {
            name: Some(name.clone()),
            namespace: Some(namespace.clone()),
            labels: Some(labels.clone()),
            owner_references: Some(vec![owner_ref]),
            ..Default::default()
        },
        spec: Some(ServiceSpec {
            selector: Some({
                let mut sel = BTreeMap::new();
                sel.insert("app".to_string(), name.clone());
                sel
            }),
            ports: Some(vec![ServicePort {
                name: Some("mongodb".to_string()),
                port: 27017,
                target_port: Some(k8s_openapi::apimachinery::pkg::util::intstr::IntOrString::String(
                    "mongodb".to_string(),
                )),
                ..Default::default()
            }]),
            ..Default::default()
        }),
        ..Default::default()
    };

    Ok((mongo_deployment, service, secret, pvc))
}

/// Build Percona MongoDB cluster
fn build_percona_cluster(
    deployment: &K8sDeployment,
    config: &MongoConfig,
) -> Result<PerconaServerMongoDB, OperatorError> {
    let name = resource_name(&deployment.name_any());
    let namespace = deployment.namespace().unwrap_or_else(|| "default".to_string());
    let owner_ref = build_owner_ref(deployment)?;
    let labels = build_labels(&name, &deployment.name_any());

    let mut requests = BTreeMap::new();
    requests.insert("storage".to_string(), config.storage.clone());

    Ok(PerconaServerMongoDB {
        api_version: "psmdb.percona.com/v1".to_string(),
        kind: "PerconaServerMongoDB".to_string(),
        metadata: ObjectMeta {
            name: Some(name.clone()),
            namespace: Some(namespace),
            labels: Some(labels),
            owner_references: Some(vec![owner_ref]),
            ..Default::default()
        },
        spec: PerconaMongoSpec {
            cr_version: "1.16.0".to_string(),
            image: format!("percona/percona-server-mongodb:{}", config.version),
            replsets: vec![PerconaReplset {
                name: "rs0".to_string(),
                size: config.replicas,
                volume_spec: PerconaVolumeSpec {
                    pvc: PerconaPVC {
                        storage_class_name: None,
                        resources: PerconaPVCResources { requests },
                    },
                },
            }],
            secrets: Some(PerconaSecrets {
                users: format!("{}-users", name),
            }),
        },
    })
}

/// Build MongoDB Community resource
fn build_community_cluster(
    deployment: &K8sDeployment,
    config: &MongoConfig,
) -> Result<MongoDBCommunity, OperatorError> {
    let name = resource_name(&deployment.name_any());
    let namespace = deployment.namespace().unwrap_or_else(|| "default".to_string());
    let owner_ref = build_owner_ref(deployment)?;
    let labels = build_labels(&name, &deployment.name_any());

    Ok(MongoDBCommunity {
        api_version: "mongodbcommunity.mongodb.com/v1".to_string(),
        kind: "MongoDBCommunity".to_string(),
        metadata: ObjectMeta {
            name: Some(name.clone()),
            namespace: Some(namespace),
            labels: Some(labels),
            owner_references: Some(vec![owner_ref]),
            ..Default::default()
        },
        spec: MongoDBCommunitySpec {
            members: config.replicas,
            db_type: "ReplicaSet".to_string(),
            version: config.version.clone(),
            security: MongoDBSecurity {
                authentication: MongoDBAuth {
                    modes: vec!["SCRAM".to_string()],
                },
            },
            users: vec![MongoDBUser {
                name: "app".to_string(),
                db: config.database.clone(),
                password_secret_ref: MongoDBPasswordRef {
                    name: format!("{}-app-password", name),
                },
                roles: vec![
                    MongoDBRole {
                        name: "readWrite".to_string(),
                        db: config.database.clone(),
                    },
                    MongoDBRole {
                        name: "dbAdmin".to_string(),
                        db: config.database.clone(),
                    },
                ],
            }],
            stateful_set: None,
        },
    })
}

/// Apply resources using dynamic API
async fn apply_dynamic_resource(
    client: &kube::Client,
    resource: &serde_json::Value,
    namespace: &str,
) -> Result<(), OperatorError> {
    let api_version = resource
        .get("apiVersion")
        .and_then(|v| v.as_str())
        .ok_or_else(|| OperatorError::Config("Missing apiVersion".into()))?;

    let kind = resource
        .get("kind")
        .and_then(|v| v.as_str())
        .ok_or_else(|| OperatorError::Config("Missing kind".into()))?;

    let name = resource
        .get("metadata")
        .and_then(|m| m.get("name"))
        .and_then(|n| n.as_str())
        .ok_or_else(|| OperatorError::Config("Missing metadata.name".into()))?;

    let (group, version) = if api_version.contains('/') {
        let parts: Vec<&str> = api_version.split('/').collect();
        (parts[0], parts[1])
    } else {
        ("", api_version)
    };

    let gvk = kube::api::GroupVersionKind {
        group: group.to_string(),
        version: version.to_string(),
        kind: kind.to_string(),
    };

    let api_resource = kube::discovery::ApiResource::from_gvk(&gvk);
    let api: Api<kube::api::DynamicObject> =
        Api::namespaced_with(client.clone(), namespace, &api_resource);

    let obj: kube::api::DynamicObject = serde_json::from_value(resource.clone())?;

    api.patch(name, &PatchParams::apply(FIELD_MANAGER), &Patch::Apply(&obj))
        .await?;

    info!("Applied {} {}/{}", kind, namespace, name);
    Ok(())
}

/// Delete MongoDB resources
async fn delete_mongo_resources(
    client: &kube::Client,
    namespace: &str,
    name: &str,
) -> Result<(), OperatorError> {
    // Try to delete various resource types
    let resource_name = resource_name(name);

    // Delete Deployment
    let deploy_api: Api<K8sDeployment> = Api::namespaced(client.clone(), namespace);
    let _ = deploy_api
        .delete(&resource_name, &DeleteParams::default())
        .await;

    // Delete Service
    let svc_api: Api<Service> = Api::namespaced(client.clone(), namespace);
    let _ = svc_api
        .delete(&resource_name, &DeleteParams::default())
        .await;

    // Delete Secret
    let secret_api: Api<Secret> = Api::namespaced(client.clone(), namespace);
    let _ = secret_api
        .delete(&format!("{}-auth", resource_name), &DeleteParams::default())
        .await;

    // Delete PVC
    let pvc_api: Api<PersistentVolumeClaim> = Api::namespaced(client.clone(), namespace);
    let _ = pvc_api
        .delete(&format!("{}-data", resource_name), &DeleteParams::default())
        .await;

    info!("Deleted MongoDB resources for {}/{}", namespace, name);
    Ok(())
}

/// Reconcile MongoDB provisioning
pub async fn reconcile(
    deployment: Arc<K8sDeployment>,
    ctx: Arc<Context>,
) -> Result<Action, OperatorError> {
    let name = deployment.name_any();
    let namespace = deployment.namespace().unwrap_or_else(|| "default".to_string());

    let config = MongoConfig::from_deployment(&deployment);

    // Check if being deleted
    if deployment.metadata.deletion_timestamp.is_some() {
        debug!("Deployment {}/{} is being deleted", namespace, name);
        return Ok(Action::await_change());
    }

    // Check if mongo is requested
    if !config.enabled {
        // Check if we need to clean up
        let resource_name = resource_name(&name);
        let deploy_api: Api<K8sDeployment> = Api::namespaced(ctx.client.clone(), &namespace);
        if deploy_api.get(&resource_name).await.is_ok() {
            info!("Mongo label removed from {}/{}, cleaning up", namespace, name);
            delete_mongo_resources(&ctx.client, &namespace, &name).await?;
        }
        return Ok(Action::requeue(Duration::from_secs(300)));
    }

    // Detect best provider
    let provider = detect_provider(&ctx.client, &config).await;

    info!(
        "Provisioning MongoDB for {}/{} using {:?}: database={}, storage={}, replicas={}",
        namespace, name, provider, config.database, config.storage, config.replicas
    );

    match provider {
        MongoProvider::Percona => {
            let cluster = build_percona_cluster(&deployment, &config)?;
            let json = serde_json::to_value(&cluster)?;
            apply_dynamic_resource(&ctx.client, &json, &namespace).await?;
        }
        MongoProvider::Community => {
            let cluster = build_community_cluster(&deployment, &config)?;
            let json = serde_json::to_value(&cluster)?;
            apply_dynamic_resource(&ctx.client, &json, &namespace).await?;
        }
        MongoProvider::Deployment => {
            let (deploy, svc, secret, pvc) = build_simple_deployment(&deployment, &config)?;

            // Apply PVC first
            let pvc_api: Api<PersistentVolumeClaim> =
                Api::namespaced(ctx.client.clone(), &namespace);
            pvc_api
                .patch(
                    pvc.metadata.name.as_ref().unwrap(),
                    &PatchParams::apply(FIELD_MANAGER),
                    &Patch::Apply(&pvc),
                )
                .await?;

            // Apply Secret
            let secret_api: Api<Secret> = Api::namespaced(ctx.client.clone(), &namespace);
            secret_api
                .patch(
                    secret.metadata.name.as_ref().unwrap(),
                    &PatchParams::apply(FIELD_MANAGER),
                    &Patch::Apply(&secret),
                )
                .await?;

            // Apply Deployment
            let deploy_api: Api<K8sDeployment> = Api::namespaced(ctx.client.clone(), &namespace);
            deploy_api
                .patch(
                    deploy.metadata.name.as_ref().unwrap(),
                    &PatchParams::apply(FIELD_MANAGER),
                    &Patch::Apply(&deploy),
                )
                .await?;

            // Apply Service
            let svc_api: Api<Service> = Api::namespaced(ctx.client.clone(), &namespace);
            svc_api
                .patch(
                    svc.metadata.name.as_ref().unwrap(),
                    &PatchParams::apply(FIELD_MANAGER),
                    &Patch::Apply(&svc),
                )
                .await?;

            info!("Applied simple MongoDB deployment for {}/{}", namespace, name);
        }
        MongoProvider::Helm => {
            // TODO: Invoke Helm client
            warn!("Helm provisioning not yet implemented, falling back to deployment");
            let (_deploy, _svc, _secret, _pvc) = build_simple_deployment(&deployment, &config)?;
            // TODO: Apply resources (same as Deployment) when Helm fallback is implemented
        }
        MongoProvider::Atlas => {
            // TODO: Create Crossplane claim for Atlas
            warn!("Atlas provisioning not yet implemented");
        }
        MongoProvider::Auto => {
            unreachable!("Auto should have been resolved");
        }
    }

    Ok(Action::requeue(Duration::from_secs(60)))
}

/// Error policy
pub fn error_policy(
    deployment: Arc<K8sDeployment>,
    error: &OperatorError,
    _ctx: Arc<Context>,
) -> Action {
    warn!(
        "MongoProvisioner error for {}: {}",
        deployment.name_any(),
        error
    );
    Action::requeue(Duration::from_secs(60))
}
