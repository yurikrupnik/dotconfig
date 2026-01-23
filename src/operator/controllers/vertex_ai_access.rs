//! VertexAIAccess Controller
//!
//! Manages Google Vertex AI access via Crossplane upbound/provider-gcp.
//! Creates production-ready AI infrastructure with:
//! - GCP Service Accounts with Workload Identity
//! - IAM bindings for Vertex AI
//! - Vector Search indexes (optional)
//! - Feature Store (optional)
//! - Model endpoints (optional)

use crate::operator::dependencies::known_dependencies;
use crate::operator::types::{
    ManagedVertexResource, VertexAIAccess, VertexAIAccessCondition, VertexAIAccessPhase,
    VertexAIAccessStatus,
};
use crate::operator::{Context, OperatorError, Result};
use chrono::Utc;
use kube::api::{Patch, PatchParams};
use kube::runtime::controller::Action;
use kube::{Api, ResourceExt};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

/// Reconciles VertexAIAccess resources
pub async fn reconcile(
    vertex: Arc<VertexAIAccess>,
    ctx: Arc<Context>,
) -> std::result::Result<Action, OperatorError> {
    let name = vertex.name_any();
    let namespace = vertex.namespace().unwrap_or_else(|| "default".to_string());

    info!("Reconciling VertexAIAccess {}/{}", namespace, name);

    let api: Api<VertexAIAccess> = Api::namespaced(ctx.client.clone(), &namespace);

    // Check dependencies before proceeding
    let deps = known_dependencies::vertex_ai_access_deps();
    let missing = ctx.get_missing_dependencies(&deps).await;

    if !missing.is_empty() {
        let missing_names: Vec<_> = missing
            .iter()
            .map(|r| {
                let hint = r
                    .dependency
                    .install_hint
                    .as_deref()
                    .unwrap_or("See documentation");
                format!("{} ({})", r.dependency.name, hint)
            })
            .collect();

        let message = format!("Missing required dependencies: {}", missing_names.join(", "));

        warn!("VertexAIAccess {}/{}: {}", namespace, name, message);

        update_phase_with_condition(
            &api,
            &name,
            VertexAIAccessPhase::Blocked,
            "DependencyMissing",
            &message,
        )
        .await?;

        return Ok(Action::requeue(Duration::from_secs(60)));
    }

    // Update status to Creating
    update_phase(
        &api,
        &name,
        VertexAIAccessPhase::Creating,
        "Creating Vertex AI access resources",
    )
    .await?;

    let mut managed_resources = Vec::new();

    // 1. Create or use GCP Service Account
    let service_account_name = if let Some(ref existing) =
        vertex.spec.workload_identity.existing_gcp_service_account
    {
        existing.clone()
    } else {
        let sa_name = create_service_account(&vertex, &ctx, &namespace).await?;
        managed_resources.push(ManagedVertexResource {
            api_version: "cloudplatform.gcp.upbound.io/v1beta1".to_string(),
            kind: "ServiceAccount".to_string(),
            name: sa_name.clone(),
            ready: false,
            synced: false,
            message: Some("Creating GCP service account".to_string()),
        });
        sa_name
    };

    // 2. Create IAM Member bindings for Vertex AI
    let iam_bindings = create_vertex_iam_bindings(&vertex, &ctx, &namespace, &service_account_name).await?;
    for binding in iam_bindings {
        managed_resources.push(ManagedVertexResource {
            api_version: "cloudplatform.gcp.upbound.io/v1beta1".to_string(),
            kind: "ProjectIAMMember".to_string(),
            name: binding,
            ready: false,
            synced: false,
            message: Some("Creating IAM binding".to_string()),
        });
    }

    // 3. Create Workload Identity binding
    let wi_binding_name =
        create_workload_identity_binding(&vertex, &ctx, &namespace, &service_account_name).await?;
    managed_resources.push(ManagedVertexResource {
        api_version: "cloudplatform.gcp.upbound.io/v1beta1".to_string(),
        kind: "ServiceAccountIAMBinding".to_string(),
        name: wi_binding_name,
        ready: false,
        synced: false,
        message: Some("Creating Workload Identity binding".to_string()),
    });

    // 4. Create Vector Search Indexes if configured
    for index_spec in &vertex.spec.vector_search_indexes {
        let index_name = create_vector_search_index(&vertex, &ctx, &namespace, index_spec).await?;
        managed_resources.push(ManagedVertexResource {
            api_version: "vertexai.gcp.upbound.io/v1beta1".to_string(),
            kind: "Index".to_string(),
            name: index_name,
            ready: false,
            synced: false,
            message: Some(format!("Creating Vector Search index: {}", index_spec.display_name)),
        });

        // Create Index Endpoint if configured
        if let Some(ref endpoint_spec) = index_spec.index_endpoint {
            let endpoint_name =
                create_index_endpoint(&vertex, &ctx, &namespace, endpoint_spec).await?;
            managed_resources.push(ManagedVertexResource {
                api_version: "vertexai.gcp.upbound.io/v1beta1".to_string(),
                kind: "IndexEndpoint".to_string(),
                name: endpoint_name,
                ready: false,
                synced: false,
                message: Some(format!("Creating Index Endpoint: {}", endpoint_spec.display_name)),
            });
        }
    }

    // 5. Create Endpoints if configured
    for endpoint_spec in &vertex.spec.endpoints {
        let endpoint_name = create_endpoint(&vertex, &ctx, &namespace, endpoint_spec).await?;
        managed_resources.push(ManagedVertexResource {
            api_version: "vertexai.gcp.upbound.io/v1beta1".to_string(),
            kind: "Endpoint".to_string(),
            name: endpoint_name,
            ready: false,
            synced: false,
            message: Some(format!("Creating Endpoint: {}", endpoint_spec.display_name)),
        });
    }

    // 6. Create Feature Store if configured
    if let Some(ref feature_store) = vertex.spec.feature_store {
        let fs_name = create_feature_store(&vertex, &ctx, &namespace, feature_store).await?;
        managed_resources.push(ManagedVertexResource {
            api_version: "vertexai.gcp.upbound.io/v1beta1".to_string(),
            kind: "Featurestore".to_string(),
            name: fs_name,
            ready: false,
            synced: false,
            message: Some(format!("Creating Feature Store: {}", feature_store.name)),
        });
    }

    // Update status
    let service_account_email = format!(
        "{}@{}.iam.gserviceaccount.com",
        service_account_name, vertex.spec.project_id
    );

    update_status_full(
        &api,
        &name,
        VertexAIAccessPhase::Ready,
        managed_resources,
        Some(service_account_email),
        true,
    )
    .await?;

    info!("VertexAIAccess {}/{} is Ready", namespace, name);

    Ok(Action::requeue(Duration::from_secs(300)))
}

async fn create_service_account(
    vertex: &VertexAIAccess,
    ctx: &Context,
    namespace: &str,
) -> Result<String> {
    let name = vertex
        .spec
        .workload_identity
        .gcp_service_account_name
        .clone()
        .unwrap_or_else(|| format!("{}-vertexai-sa", vertex.name_any()));

    info!("Creating GCP Service Account {}", name);

    let provider_config = vertex
        .spec
        .provider_config_ref
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "default".to_string());

    let resource = serde_json::json!({
        "apiVersion": "cloudplatform.gcp.upbound.io/v1beta1",
        "kind": "ServiceAccount",
        "metadata": {
            "name": name,
            "namespace": namespace,
            "labels": vertex.spec.labels.clone().unwrap_or_default()
        },
        "spec": {
            "forProvider": {
                "accountId": name,
                "displayName": format!("Vertex AI access for {}", vertex.name_any()),
                "description": format!("Service account for VertexAIAccess {} managed by platform-operator", vertex.name_any()),
                "project": vertex.spec.project_id
            },
            "providerConfigRef": {
                "name": provider_config
            }
        }
    });

    ctx.crossplane_client.apply_resource(&resource).await?;

    Ok(name)
}

async fn create_vertex_iam_bindings(
    vertex: &VertexAIAccess,
    ctx: &Context,
    namespace: &str,
    service_account_name: &str,
) -> Result<Vec<String>> {
    let provider_config = vertex
        .spec
        .provider_config_ref
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "default".to_string());

    let service_account_email = format!(
        "serviceAccount:{}@{}.iam.gserviceaccount.com",
        service_account_name, vertex.spec.project_id
    );

    // Determine required roles based on spec
    let mut roles = vec![
        "roles/aiplatform.user", // Basic Vertex AI access
    ];

    // Add roles based on features enabled
    if vertex.spec.generative_ai.gemini_enabled
        || vertex.spec.generative_ai.palm_enabled
        || vertex.spec.generative_ai.imagen_enabled
    {
        roles.push("roles/aiplatform.user");
    }

    if vertex.spec.generative_ai.tuning_enabled {
        roles.push("roles/aiplatform.customCodeServiceAgent");
    }

    if !vertex.spec.vector_search_indexes.is_empty() {
        roles.push("roles/aiplatform.featurestoreAdmin");
    }

    if vertex.spec.feature_store.is_some() {
        roles.push("roles/aiplatform.featurestoreAdmin");
    }

    if !vertex.spec.endpoints.is_empty() {
        roles.push("roles/aiplatform.endpointAdmin");
    }

    if vertex.spec.experiments_enabled {
        roles.push("roles/aiplatform.metadataAdmin");
    }

    if vertex.spec.model_registry_enabled {
        roles.push("roles/aiplatform.modelAdmin");
    }

    if vertex.spec.pipelines.is_some() {
        roles.push("roles/aiplatform.admin");
    }

    // Add additional roles
    for role in &vertex.spec.workload_identity.additional_roles {
        if !roles.contains(&role.as_str()) {
            roles.push(role.as_str());
        }
    }

    // Deduplicate roles
    roles.sort();
    roles.dedup();

    let mut binding_names = Vec::new();

    for role in roles {
        let role_short = role.split('/').last().unwrap_or(role);
        let binding_name = format!("{}-{}", vertex.name_any(), role_short);

        info!("Creating IAM binding {} for role {}", binding_name, role);

        let resource = serde_json::json!({
            "apiVersion": "cloudplatform.gcp.upbound.io/v1beta1",
            "kind": "ProjectIAMMember",
            "metadata": {
                "name": binding_name,
                "namespace": namespace,
                "labels": vertex.spec.labels.clone().unwrap_or_default()
            },
            "spec": {
                "forProvider": {
                    "project": vertex.spec.project_id,
                    "role": role,
                    "member": service_account_email
                },
                "providerConfigRef": {
                    "name": provider_config
                }
            }
        });

        ctx.crossplane_client.apply_resource(&resource).await?;
        binding_names.push(binding_name);
    }

    Ok(binding_names)
}

async fn create_workload_identity_binding(
    vertex: &VertexAIAccess,
    ctx: &Context,
    namespace: &str,
    service_account_name: &str,
) -> Result<String> {
    let name = format!("{}-wi-binding", vertex.name_any());

    info!(
        "Creating Workload Identity binding {} for K8s SA {}:{}",
        name,
        vertex.spec.workload_identity.kubernetes_namespace,
        vertex.spec.workload_identity.kubernetes_service_account
    );

    let provider_config = vertex
        .spec
        .provider_config_ref
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "default".to_string());

    let k8s_sa_member = format!(
        "serviceAccount:{}.svc.id.goog[{}/{}]",
        vertex.spec.project_id,
        vertex.spec.workload_identity.kubernetes_namespace,
        vertex.spec.workload_identity.kubernetes_service_account
    );

    let resource = serde_json::json!({
        "apiVersion": "cloudplatform.gcp.upbound.io/v1beta1",
        "kind": "ServiceAccountIAMBinding",
        "metadata": {
            "name": name,
            "namespace": namespace,
            "labels": vertex.spec.labels.clone().unwrap_or_default()
        },
        "spec": {
            "forProvider": {
                "serviceAccountId": format!(
                    "projects/{}/serviceAccounts/{}@{}.iam.gserviceaccount.com",
                    vertex.spec.project_id,
                    service_account_name,
                    vertex.spec.project_id
                ),
                "role": "roles/iam.workloadIdentityUser",
                "members": [k8s_sa_member]
            },
            "providerConfigRef": {
                "name": provider_config
            }
        }
    });

    ctx.crossplane_client.apply_resource(&resource).await?;

    Ok(name)
}

async fn create_vector_search_index(
    vertex: &VertexAIAccess,
    ctx: &Context,
    namespace: &str,
    index_spec: &crate::operator::types::VectorSearchIndexSpec,
) -> Result<String> {
    let name = format!(
        "{}-idx-{}",
        vertex.name_any(),
        index_spec.display_name.to_lowercase().replace(' ', "-")
    );

    info!("Creating Vector Search Index {}", name);

    let provider_config = vertex
        .spec
        .provider_config_ref
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "default".to_string());

    let distance_measure = match index_spec.distance_measure_type {
        crate::operator::types::DistanceMeasureType::DotProductDistance => "DOT_PRODUCT_DISTANCE",
        crate::operator::types::DistanceMeasureType::SquaredL2Distance => "SQUARED_L2_DISTANCE",
        crate::operator::types::DistanceMeasureType::CosineDistance => "COSINE_DISTANCE",
    };

    let shard_size = match index_spec.shard_size {
        crate::operator::types::ShardSize::ShardSizeSmall => "SHARD_SIZE_SMALL",
        crate::operator::types::ShardSize::ShardSizeMedium => "SHARD_SIZE_MEDIUM",
        crate::operator::types::ShardSize::ShardSizeLarge => "SHARD_SIZE_LARGE",
    };

    let resource = serde_json::json!({
        "apiVersion": "vertexai.gcp.upbound.io/v1beta1",
        "kind": "Index",
        "metadata": {
            "name": name,
            "namespace": namespace,
            "labels": vertex.spec.labels.clone().unwrap_or_default()
        },
        "spec": {
            "forProvider": {
                "displayName": index_spec.display_name,
                "description": index_spec.description,
                "region": vertex.spec.region,
                "project": vertex.spec.project_id,
                "indexUpdateMethod": "STREAM_UPDATE",
                "metadata": [{
                    "contentsDeltaUri": "",
                    "config": [{
                        "dimensions": index_spec.dimensions,
                        "approximateNeighborsCount": index_spec.approximate_neighbors_count,
                        "distanceMeasureType": distance_measure,
                        "shardSize": shard_size,
                        "algorithmConfig": [{
                            "treeAhConfig": index_spec.algorithm_config.tree_ah_config.as_ref().map(|c| {
                                serde_json::json!({
                                    "leafNodeEmbeddingCount": c.leaf_node_embedding_count,
                                    "leafNodesToSearchPercent": c.leaf_nodes_to_search_percent
                                })
                            })
                        }]
                    }]
                }]
            },
            "providerConfigRef": {
                "name": provider_config
            }
        }
    });

    ctx.crossplane_client.apply_resource(&resource).await?;

    Ok(name)
}

async fn create_index_endpoint(
    vertex: &VertexAIAccess,
    ctx: &Context,
    namespace: &str,
    endpoint_spec: &crate::operator::types::IndexEndpointSpec,
) -> Result<String> {
    let name = format!(
        "{}-idxep-{}",
        vertex.name_any(),
        endpoint_spec.display_name.to_lowercase().replace(' ', "-")
    );

    info!("Creating Vector Search Index Endpoint {}", name);

    let provider_config = vertex
        .spec
        .provider_config_ref
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "default".to_string());

    let resource = serde_json::json!({
        "apiVersion": "vertexai.gcp.upbound.io/v1beta1",
        "kind": "IndexEndpoint",
        "metadata": {
            "name": name,
            "namespace": namespace,
            "labels": vertex.spec.labels.clone().unwrap_or_default()
        },
        "spec": {
            "forProvider": {
                "displayName": endpoint_spec.display_name,
                "region": vertex.spec.region,
                "project": vertex.spec.project_id,
                "publicEndpointEnabled": endpoint_spec.public_endpoint_enabled
            },
            "providerConfigRef": {
                "name": provider_config
            }
        }
    });

    ctx.crossplane_client.apply_resource(&resource).await?;

    Ok(name)
}

async fn create_endpoint(
    vertex: &VertexAIAccess,
    ctx: &Context,
    namespace: &str,
    endpoint_spec: &crate::operator::types::EndpointSpec,
) -> Result<String> {
    let name = format!(
        "{}-ep-{}",
        vertex.name_any(),
        endpoint_spec.display_name.to_lowercase().replace(' ', "-")
    );

    info!("Creating Vertex AI Endpoint {}", name);

    let provider_config = vertex
        .spec
        .provider_config_ref
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "default".to_string());

    let resource = serde_json::json!({
        "apiVersion": "vertexai.gcp.upbound.io/v1beta1",
        "kind": "Endpoint",
        "metadata": {
            "name": name,
            "namespace": namespace,
            "labels": vertex.spec.labels.clone().unwrap_or_default()
        },
        "spec": {
            "forProvider": {
                "displayName": endpoint_spec.display_name,
                "description": endpoint_spec.description,
                "region": vertex.spec.region,
                "project": vertex.spec.project_id,
                "network": endpoint_spec.network
            },
            "providerConfigRef": {
                "name": provider_config
            }
        }
    });

    ctx.crossplane_client.apply_resource(&resource).await?;

    Ok(name)
}

async fn create_feature_store(
    vertex: &VertexAIAccess,
    ctx: &Context,
    namespace: &str,
    feature_store: &crate::operator::types::FeatureStoreSpec,
) -> Result<String> {
    let name = format!(
        "{}-fs-{}",
        vertex.name_any(),
        feature_store.name.to_lowercase().replace(' ', "-")
    );

    info!("Creating Vertex AI Feature Store {}", name);

    let provider_config = vertex
        .spec
        .provider_config_ref
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "default".to_string());

    let mut online_serving_config = serde_json::json!({});
    if let Some(ref config) = feature_store.online_serving_config {
        if let Some(fixed) = config.fixed_node_count {
            online_serving_config = serde_json::json!({
                "fixedNodeCount": fixed
            });
        } else if let Some(ref scaling) = config.scaling {
            online_serving_config = serde_json::json!({
                "scaling": {
                    "minNodeCount": scaling.min_node_count,
                    "maxNodeCount": scaling.max_node_count
                }
            });
        }
    }

    let resource = serde_json::json!({
        "apiVersion": "vertexai.gcp.upbound.io/v1beta1",
        "kind": "Featurestore",
        "metadata": {
            "name": name,
            "namespace": namespace,
            "labels": vertex.spec.labels.clone().unwrap_or_default()
        },
        "spec": {
            "forProvider": {
                "name": feature_store.name,
                "region": vertex.spec.region,
                "project": vertex.spec.project_id,
                "onlineServingConfig": [online_serving_config]
            },
            "providerConfigRef": {
                "name": provider_config
            }
        }
    });

    ctx.crossplane_client.apply_resource(&resource).await?;

    Ok(name)
}

async fn update_phase(
    api: &Api<VertexAIAccess>,
    name: &str,
    phase: VertexAIAccessPhase,
    message: &str,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();

    let status = VertexAIAccessStatus {
        phase,
        message: Some(message.to_string()),
        last_reconcile_time: Some(now.clone()),
        conditions: vec![VertexAIAccessCondition {
            condition_type: "Reconciling".to_string(),
            status: "True".to_string(),
            reason: "Reconciling".to_string(),
            message: message.to_string(),
            last_transition_time: now,
        }],
        ..Default::default()
    };

    let patch = serde_json::json!({
        "status": status
    });

    api.patch_status(
        name,
        &PatchParams::apply("platform-operator"),
        &Patch::Merge(&patch),
    )
    .await?;

    Ok(())
}

async fn update_phase_with_condition(
    api: &Api<VertexAIAccess>,
    name: &str,
    phase: VertexAIAccessPhase,
    reason: &str,
    message: &str,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();

    let status = VertexAIAccessStatus {
        phase,
        message: Some(message.to_string()),
        last_reconcile_time: Some(now.clone()),
        conditions: vec![VertexAIAccessCondition {
            condition_type: "Ready".to_string(),
            status: "False".to_string(),
            reason: reason.to_string(),
            message: message.to_string(),
            last_transition_time: now,
        }],
        ..Default::default()
    };

    let patch = serde_json::json!({
        "status": status
    });

    api.patch_status(
        name,
        &PatchParams::apply("platform-operator"),
        &Patch::Merge(&patch),
    )
    .await?;

    Ok(())
}

async fn update_status_full(
    api: &Api<VertexAIAccess>,
    name: &str,
    phase: VertexAIAccessPhase,
    managed_resources: Vec<ManagedVertexResource>,
    service_account_email: Option<String>,
    workload_identity_bound: bool,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();

    let status = VertexAIAccessStatus {
        phase,
        ready: true,
        service_account_email,
        workload_identity_bound,
        managed_resources,
        last_reconcile_time: Some(now.clone()),
        conditions: vec![VertexAIAccessCondition {
            condition_type: "Ready".to_string(),
            status: "True".to_string(),
            reason: "AllResourcesReady".to_string(),
            message: "Vertex AI access is configured and ready".to_string(),
            last_transition_time: now,
        }],
        ..Default::default()
    };

    let patch = serde_json::json!({
        "status": status
    });

    api.patch_status(
        name,
        &PatchParams::apply("platform-operator"),
        &Patch::Merge(&patch),
    )
    .await?;

    Ok(())
}

/// Error policy for the controller
pub fn error_policy(
    vertex: Arc<VertexAIAccess>,
    error: &OperatorError,
    _ctx: Arc<Context>,
) -> Action {
    warn!(
        "Reconcile error for VertexAIAccess {}: {}",
        vertex.name_any(),
        error
    );

    Action::requeue(Duration::from_secs(30))
}
