//! Cluster Controller
//!
//! Provisions Kubernetes clusters (GKE, EKS, AKS) via Crossplane with
//! Vault-based authentication for cloud credentials.

use crate::operator::types::cluster::*;
use crate::operator::vault::VaultClient;
use crate::operator::{Context, OperatorError, Result};
use chrono::Utc;
use k8s_openapi::api::core::v1::Secret;
use k8s_openapi::ByteString;
use kube::api::{Patch, PatchParams};
use kube::runtime::controller::Action;
use kube::{Api, ResourceExt};
use serde_json::json;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

/// Reconcile a Cluster resource
pub async fn reconcile(
    cluster: Arc<Cluster>,
    ctx: Arc<Context>,
) -> std::result::Result<Action, OperatorError> {
    let name = cluster.name_any();
    let namespace = cluster.namespace().unwrap_or_else(|| "default".to_string());

    info!(
        "Reconciling Cluster {}/{} for provider {:?}",
        namespace, name, cluster.spec.provider
    );

    let api: Api<Cluster> = Api::namespaced(ctx.client.clone(), &namespace);

    // Handle deletion
    if cluster.metadata.deletion_timestamp.is_some() {
        return handle_deletion(&cluster, &ctx, &namespace).await;
    }

    // Step 1: Fetch credentials from Vault
    update_phase(&api, &name, ClusterPhase::FetchingCredentials, "Fetching cloud credentials from Vault").await?;

    let (credentials_secret_name, credential_status) = match fetch_vault_credentials(&cluster, &ctx, &namespace).await {
        Ok(result) => result,
        Err(e) => {
            update_phase(&api, &name, ClusterPhase::Failed, &format!("Failed to fetch credentials: {}", e)).await?;
            return Ok(Action::requeue(Duration::from_secs(60)));
        }
    };

    // Step 2: Create/update ProviderConfig
    update_phase(&api, &name, ClusterPhase::CreatingProviderConfig, "Creating Crossplane ProviderConfig").await?;

    let provider_config_name = match create_provider_config(&cluster, &ctx, &namespace, &credentials_secret_name).await {
        Ok(name) => name,
        Err(e) => {
            update_phase(&api, &name, ClusterPhase::Failed, &format!("Failed to create ProviderConfig: {}", e)).await?;
            return Ok(Action::requeue(Duration::from_secs(60)));
        }
    };

    // Step 3: Create cluster
    update_phase(&api, &name, ClusterPhase::CreatingCluster, "Creating cloud cluster").await?;

    let cluster_resource = match cluster.spec.provider {
        ClusterCloudProvider::Gcp => create_gke_cluster(&cluster, &ctx, &namespace, &provider_config_name).await?,
        ClusterCloudProvider::Aws => create_eks_cluster(&cluster, &ctx, &namespace, &provider_config_name).await?,
        ClusterCloudProvider::Azure => create_aks_cluster(&cluster, &ctx, &namespace, &provider_config_name).await?,
    };

    // Step 4: Create node pools
    update_phase(&api, &name, ClusterPhase::CreatingNodePools, "Creating node pools").await?;

    let node_pool_resources = create_node_pools(&cluster, &ctx, &namespace, &provider_config_name).await?;

    // Step 5: Check readiness
    let cluster_ready = check_cluster_ready(&ctx, &cluster_resource).await.unwrap_or(false);
    let node_pools_ready = check_node_pools_ready(&ctx, &node_pool_resources).await.unwrap_or_default();
    let all_ready = cluster_ready && node_pools_ready.iter().all(|np| np.ready);

    // Step 6: Update final status
    let phase = if all_ready {
        ClusterPhase::Ready
    } else {
        ClusterPhase::CreatingNodePools
    };

    update_status_full(
        &api,
        &name,
        phase.clone(),
        &cluster_resource,
        &node_pool_resources,
        &node_pools_ready,
        &provider_config_name,
        credential_status,
        all_ready,
    ).await?;

    // Determine requeue interval
    let requeue_duration = if all_ready {
        if cluster.spec.vault_auth.mode == VaultAuthMode::Dynamic {
            Duration::from_secs(300) // Check every 5 minutes for credential refresh
        } else {
            Duration::from_secs(600) // Check every 10 minutes for static
        }
    } else {
        Duration::from_secs(30) // Fast requeue while creating
    };

    Ok(Action::requeue(requeue_duration))
}

/// Handle cluster deletion
async fn handle_deletion(
    cluster: &Cluster,
    _ctx: &Context,
    _namespace: &str,
) -> std::result::Result<Action, OperatorError> {
    let name = cluster.name_any();
    info!("Handling deletion for Cluster {}", name);

    // Crossplane handles deletion via its deletion policy
    // We just need to let it propagate

    Ok(Action::requeue(Duration::from_secs(30)))
}

/// Fetch credentials from Vault
async fn fetch_vault_credentials(
    cluster: &Cluster,
    ctx: &Context,
    namespace: &str,
) -> Result<(String, VaultCredentialStatus)> {
    let vault_config = &cluster.spec.vault_auth;
    let name = cluster.name_any();

    // Load CA cert if specified
    let ca_cert = if let Some(ca_ref) = &vault_config.ca_cert {
        let secret_api: Api<Secret> = Api::namespaced(
            ctx.client.clone(),
            ca_ref.namespace.as_deref().unwrap_or(namespace),
        );
        let secret = secret_api.get(&ca_ref.name).await?;
        secret.data
            .and_then(|d| d.get(&ca_ref.key).cloned())
            .map(|b| b.0)
    } else {
        None
    };

    // Create Vault client
    let mut vault_client = VaultClient::new(&vault_config.server, ca_cert.as_deref()).await?;

    // Authenticate to Vault
    authenticate_vault(&mut vault_client, &vault_config.vault_auth, ctx, namespace).await?;

    // Fetch credentials based on mode
    let (credentials, credential_status) = match vault_config.mode {
        VaultAuthMode::Dynamic => fetch_dynamic_credentials(&vault_client, cluster).await?,
        VaultAuthMode::Static => fetch_static_credentials(&vault_client, cluster).await?,
    };

    // Create/update Kubernetes secret with credentials
    let secret_name = format!("{}-vault-creds", name);
    create_credential_secret(ctx, namespace, &secret_name, &credentials, &cluster.spec.provider).await?;

    Ok((secret_name, credential_status))
}

/// Authenticate to Vault using configured method
async fn authenticate_vault(
    vault_client: &mut VaultClient,
    auth_config: &VaultAuthMethod,
    ctx: &Context,
    namespace: &str,
) -> Result<()> {
    if let Some(token_auth) = &auth_config.token {
        let secret_api: Api<Secret> = Api::namespaced(
            ctx.client.clone(),
            token_auth.secret_ref.namespace.as_deref().unwrap_or(namespace),
        );
        let secret = secret_api.get(&token_auth.secret_ref.name).await?;
        let token = secret.data
            .and_then(|d| d.get(&token_auth.secret_ref.key).cloned())
            .map(|b| String::from_utf8_lossy(&b.0).to_string())
            .ok_or_else(|| OperatorError::Config("Token not found in secret".into()))?;
        vault_client.set_token(token);
    } else if let Some(k8s_auth) = &auth_config.kubernetes {
        // Read service account token
        let jwt = get_service_account_jwt(ctx, k8s_auth.service_account_ref.as_ref(), namespace).await?;
        vault_client.auth_kubernetes(&k8s_auth.mount_path, &k8s_auth.role, &jwt).await?;
    } else if let Some(approle_auth) = &auth_config.app_role {
        let role_id = get_secret_value(ctx, &approle_auth.role_id, namespace).await?;
        let secret_id = get_secret_value(ctx, &approle_auth.secret_id, namespace).await?;
        vault_client.auth_approle(&approle_auth.mount_path, &role_id, &secret_id).await?;
    } else {
        return Err(OperatorError::Config("No Vault authentication method configured".into()));
    }

    Ok(())
}

/// Get secret value from Kubernetes secret
async fn get_secret_value(
    ctx: &Context,
    selector: &ClusterSecretKeySelector,
    default_namespace: &str,
) -> Result<String> {
    let secret_api: Api<Secret> = Api::namespaced(
        ctx.client.clone(),
        selector.namespace.as_deref().unwrap_or(default_namespace),
    );
    let secret = secret_api.get(&selector.name).await?;
    secret.data
        .and_then(|d| d.get(&selector.key).cloned())
        .map(|b| String::from_utf8_lossy(&b.0).to_string())
        .ok_or_else(|| OperatorError::Config(format!("Key {} not found in secret {}", selector.key, selector.name)))
}

/// Get service account JWT for Kubernetes auth
async fn get_service_account_jwt(
    _ctx: &Context,
    _sa_ref: Option<&ClusterServiceAccountRef>,
    _namespace: &str,
) -> Result<String> {
    // Read the service account token from the mounted path
    // In a real implementation, you'd either:
    // 1. Use the operator's own SA token if no ref is provided
    // 2. Create a TokenRequest for the specified SA

    // For now, read the operator's own token
    let token = tokio::fs::read_to_string("/var/run/secrets/kubernetes.io/serviceaccount/token")
        .await
        .map_err(|e| OperatorError::Io(e))?;

    Ok(token)
}

/// Fetch dynamic credentials from Vault secrets engine
async fn fetch_dynamic_credentials(
    vault_client: &VaultClient,
    cluster: &Cluster,
) -> Result<(serde_json::Value, VaultCredentialStatus)> {
    let dynamic_config = cluster.spec.vault_auth.dynamic.as_ref()
        .ok_or_else(|| OperatorError::Config("Dynamic config required for dynamic mode".into()))?;

    let now = Utc::now();

    let credentials = match cluster.spec.provider {
        ClusterCloudProvider::Gcp => {
            let (creds, _lease) = vault_client.get_gcp_credentials(
                &dynamic_config.secrets_engine_path,
                &dynamic_config.role,
            ).await?;
            serde_json::to_value(creds)?
        }
        ClusterCloudProvider::Aws => {
            let (creds, _lease) = vault_client.get_aws_credentials(
                &dynamic_config.secrets_engine_path,
                &dynamic_config.role,
            ).await?;
            serde_json::to_value(creds)?
        }
        ClusterCloudProvider::Azure => {
            let (creds, _lease) = vault_client.get_azure_credentials(
                &dynamic_config.secrets_engine_path,
                &dynamic_config.role,
            ).await?;
            serde_json::to_value(creds)?
        }
    };

    let status = VaultCredentialStatus {
        source: "dynamic".to_string(),
        last_refresh_time: Some(now.to_rfc3339()),
        expires_at: cluster.spec.vault_auth.ttl.clone(),
        credential_secret: None,
    };

    Ok((credentials, status))
}

/// Fetch static credentials from Vault KV
async fn fetch_static_credentials(
    vault_client: &VaultClient,
    cluster: &Cluster,
) -> Result<(serde_json::Value, VaultCredentialStatus)> {
    let static_config = cluster.spec.vault_auth.static_secrets.as_ref()
        .ok_or_else(|| OperatorError::Config("Static config required for static mode".into()))?;

    let secrets = vault_client.read_kv_secret(
        &static_config.kv_mount_path,
        &static_config.secret_path,
        &static_config.kv_version,
    ).await?;

    let credentials = match cluster.spec.provider {
        ClusterCloudProvider::Gcp => {
            let key = static_config.keys.gcp_credentials.as_ref()
                .ok_or_else(|| OperatorError::Config("GCP credentials key not specified".into()))?;
            json!({
                "service_account_key": secrets.get(key)
            })
        }
        ClusterCloudProvider::Aws => {
            let access_key = static_config.keys.aws_access_key_id.as_ref()
                .ok_or_else(|| OperatorError::Config("AWS access key ID key not specified".into()))?;
            let secret_key = static_config.keys.aws_secret_access_key.as_ref()
                .ok_or_else(|| OperatorError::Config("AWS secret access key key not specified".into()))?;
            json!({
                "access_key": secrets.get(access_key),
                "secret_key": secrets.get(secret_key)
            })
        }
        ClusterCloudProvider::Azure => {
            let client_id = static_config.keys.azure_client_id.as_ref()
                .ok_or_else(|| OperatorError::Config("Azure client ID key not specified".into()))?;
            let client_secret = static_config.keys.azure_client_secret.as_ref()
                .ok_or_else(|| OperatorError::Config("Azure client secret key not specified".into()))?;
            let tenant_id = static_config.keys.azure_tenant_id.as_ref()
                .ok_or_else(|| OperatorError::Config("Azure tenant ID key not specified".into()))?;
            json!({
                "client_id": secrets.get(client_id),
                "client_secret": secrets.get(client_secret),
                "tenant_id": secrets.get(tenant_id),
                "subscription_id": static_config.keys.azure_subscription_id.as_ref()
                    .and_then(|k| secrets.get(k))
            })
        }
    };

    let status = VaultCredentialStatus {
        source: "static".to_string(),
        last_refresh_time: Some(Utc::now().to_rfc3339()),
        expires_at: None,
        credential_secret: None,
    };

    Ok((credentials, status))
}

/// Create Kubernetes secret with credentials
async fn create_credential_secret(
    ctx: &Context,
    namespace: &str,
    secret_name: &str,
    credentials: &serde_json::Value,
    provider: &ClusterCloudProvider,
) -> Result<()> {
    let secret_api: Api<Secret> = Api::namespaced(ctx.client.clone(), namespace);

    let mut data = BTreeMap::new();

    match provider {
        ClusterCloudProvider::Gcp => {
            // GCP expects credentials as JSON
            if let Some(sa_key) = credentials.get("service_account_key") {
                data.insert(
                    "credentials".to_string(),
                    ByteString(sa_key.to_string().into_bytes()),
                );
            } else if let Some(token) = credentials.get("access_token") {
                data.insert(
                    "credentials".to_string(),
                    ByteString(json!({"access_token": token}).to_string().into_bytes()),
                );
            }
        }
        ClusterCloudProvider::Aws => {
            // AWS expects credentials file format
            let creds_content = format!(
                "[default]\naws_access_key_id = {}\naws_secret_access_key = {}{}",
                credentials.get("access_key").and_then(|v| v.as_str()).unwrap_or(""),
                credentials.get("secret_key").and_then(|v| v.as_str()).unwrap_or(""),
                credentials.get("security_token")
                    .and_then(|v| v.as_str())
                    .map(|t| format!("\naws_session_token = {}", t))
                    .unwrap_or_default()
            );
            data.insert(
                "credentials".to_string(),
                ByteString(creds_content.into_bytes()),
            );
        }
        ClusterCloudProvider::Azure => {
            // Azure expects JSON credentials
            data.insert(
                "credentials".to_string(),
                ByteString(credentials.to_string().into_bytes()),
            );
        }
    }

    let secret = Secret {
        metadata: kube::core::ObjectMeta {
            name: Some(secret_name.to_string()),
            namespace: Some(namespace.to_string()),
            ..Default::default()
        },
        data: Some(data),
        ..Default::default()
    };

    secret_api
        .patch(
            secret_name,
            &PatchParams::apply("platform-operator"),
            &Patch::Apply(&secret),
        )
        .await?;

    info!("Created/updated credential secret: {}", secret_name);
    Ok(())
}

/// Create Crossplane ProviderConfig
async fn create_provider_config(
    cluster: &Cluster,
    ctx: &Context,
    namespace: &str,
    credentials_secret: &str,
) -> Result<String> {
    let name = cluster.name_any();
    let provider_config_name = format!("{}-provider-config", name);

    let provider_config = match cluster.spec.provider {
        ClusterCloudProvider::Gcp => {
            let project_id = cluster.spec.parameters.gcp.as_ref()
                .map(|g| g.project_id.clone())
                .ok_or_else(|| OperatorError::Config("GCP project ID required".into()))?;

            json!({
                "apiVersion": "gcp.upbound.io/v1beta1",
                "kind": "ProviderConfig",
                "metadata": {
                    "name": provider_config_name
                },
                "spec": {
                    "projectID": project_id,
                    "credentials": {
                        "source": "Secret",
                        "secretRef": {
                            "namespace": namespace,
                            "name": credentials_secret,
                            "key": "credentials"
                        }
                    }
                }
            })
        }
        ClusterCloudProvider::Aws => {
            json!({
                "apiVersion": "aws.upbound.io/v1beta1",
                "kind": "ProviderConfig",
                "metadata": {
                    "name": provider_config_name
                },
                "spec": {
                    "credentials": {
                        "source": "Secret",
                        "secretRef": {
                            "namespace": namespace,
                            "name": credentials_secret,
                            "key": "credentials"
                        }
                    }
                }
            })
        }
        ClusterCloudProvider::Azure => {
            let subscription_id = cluster.spec.parameters.azure.as_ref()
                .and_then(|a| a.subscription_id.clone())
                .ok_or_else(|| OperatorError::Config("Azure subscription ID required".into()))?;

            json!({
                "apiVersion": "azure.upbound.io/v1beta1",
                "kind": "ProviderConfig",
                "metadata": {
                    "name": provider_config_name
                },
                "spec": {
                    "subscriptionID": subscription_id,
                    "credentials": {
                        "source": "Secret",
                        "secretRef": {
                            "namespace": namespace,
                            "name": credentials_secret,
                            "key": "credentials"
                        }
                    }
                }
            })
        }
    };

    ctx.crossplane_client.apply_resource(&provider_config).await?;
    info!("Created/updated ProviderConfig: {}", provider_config_name);

    Ok(provider_config_name)
}

// ============ GKE Implementation ============

async fn create_gke_cluster(
    cluster: &Cluster,
    ctx: &Context,
    namespace: &str,
    provider_config_name: &str,
) -> Result<ManagedResourceRef> {
    let name = cluster.name_any();
    let params = &cluster.spec.parameters;
    let gcp_config = params.gcp.as_ref()
        .ok_or_else(|| OperatorError::Config("GCP configuration required".into()))?;

    info!("Creating GKE cluster: {}", name);

    let mut for_provider = json!({
        "location": params.region,
        "initialNodeCount": 1,
        "removeDefaultNodePool": true,
    });

    // Release channel
    if let Some(channel) = &gcp_config.release_channel {
        let channel_str = match channel {
            GkeReleaseChannel::Rapid => "RAPID",
            GkeReleaseChannel::Regular => "REGULAR",
            GkeReleaseChannel::Stable => "STABLE",
            GkeReleaseChannel::Unspecified => "UNSPECIFIED",
        };
        for_provider["releaseChannel"] = json!([{"channel": channel_str}]);
    }

    // Workload identity
    if params.workload_identity_enabled {
        for_provider["workloadIdentityConfig"] = json!([{
            "workloadPool": format!("{}.svc.id.goog", gcp_config.project_id)
        }]);
    }

    // Dataplane V2
    if gcp_config.dataplane_v2 {
        for_provider["datapathProvider"] = json!("ADVANCED_DATAPATH");
    }

    // Network policy
    if params.network_policy_enabled {
        for_provider["networkPolicy"] = json!([{"enabled": true, "provider": "CALICO"}]);
    }

    // Private cluster
    if let Some(network) = &cluster.spec.network {
        if network.private_cluster {
            for_provider["privateClusterConfig"] = json!([{
                "enablePrivateNodes": network.private_nodes,
                "enablePrivateEndpoint": false,
                "masterIpv4CidrBlock": "172.16.0.0/28"
            }]);
        }
    }

    // Secrets encryption
    if let Some(enc) = &params.secrets_encryption {
        if enc.enabled {
            for_provider["databaseEncryption"] = json!([{
                "state": "ENCRYPTED",
                "keyName": enc.kms_key_id
            }]);
        }
    }

    let gke_cluster = json!({
        "apiVersion": "container.gcp.upbound.io/v1beta1",
        "kind": "Cluster",
        "metadata": {
            "name": format!("{}-gke", name),
            "labels": cluster.spec.labels
        },
        "spec": {
            "forProvider": for_provider,
            "providerConfigRef": {
                "name": provider_config_name
            },
            "deletionPolicy": match cluster.spec.deletion_policy {
                ClusterDeletionPolicy::Delete => "Delete",
                ClusterDeletionPolicy::Orphan => "Orphan",
            },
            "writeConnectionSecretToRef": cluster.spec.write_connection_secret_to_ref.as_ref().map(|s| {
                json!({
                    "name": s.name,
                    "namespace": s.namespace.as_deref().unwrap_or(namespace)
                })
            })
        }
    });

    ctx.crossplane_client.apply_resource(&gke_cluster).await?;

    Ok(ManagedResourceRef {
        api_version: "container.gcp.upbound.io/v1beta1".to_string(),
        kind: "Cluster".to_string(),
        name: format!("{}-gke", name),
    })
}

// ============ EKS Implementation ============

async fn create_eks_cluster(
    cluster: &Cluster,
    ctx: &Context,
    namespace: &str,
    provider_config_name: &str,
) -> Result<ManagedResourceRef> {
    let name = cluster.name_any();
    let params = &cluster.spec.parameters;

    info!("Creating EKS cluster: {}", name);

    let mut for_provider = json!({
        "region": params.region,
        "version": params.kubernetes_version,
    });

    // Role ARN
    if let Some(aws) = &params.aws {
        if let Some(role_arn) = &aws.role_arn {
            for_provider["roleArn"] = json!(role_arn);
        }
    }

    // VPC config
    if let Some(network) = &cluster.spec.network {
        let mut vpc_config = json!({});

        if let Some(vpc) = &network.vpc {
            if !vpc.subnet_ids.is_empty() {
                vpc_config["subnetIds"] = json!(vpc.subnet_ids);
            }
        }

        if let Some(aws) = &params.aws {
            if let Some(endpoint) = &aws.endpoint_access {
                vpc_config["endpointPrivateAccess"] = json!(endpoint.private_access);
                vpc_config["endpointPublicAccess"] = json!(endpoint.public_access);
                if !endpoint.public_access_cidrs.is_empty() {
                    vpc_config["publicAccessCidrs"] = json!(endpoint.public_access_cidrs);
                }
            }
        }

        for_provider["vpcConfig"] = json!([vpc_config]);
    }

    // Encryption
    if let Some(enc) = &params.secrets_encryption {
        if enc.enabled {
            for_provider["encryptionConfig"] = json!([{
                "provider": [{"keyArn": enc.kms_key_id}],
                "resources": ["secrets"]
            }]);
        }
    }

    // Logging
    if let Some(addons) = &cluster.spec.addons {
        if addons.logging {
            for_provider["enabledClusterLogTypes"] = json!([
                "api", "audit", "authenticator", "controllerManager", "scheduler"
            ]);
        }
    }

    let eks_cluster = json!({
        "apiVersion": "eks.aws.upbound.io/v1beta1",
        "kind": "Cluster",
        "metadata": {
            "name": format!("{}-eks", name),
            "labels": cluster.spec.labels
        },
        "spec": {
            "forProvider": for_provider,
            "providerConfigRef": {
                "name": provider_config_name
            },
            "deletionPolicy": match cluster.spec.deletion_policy {
                ClusterDeletionPolicy::Delete => "Delete",
                ClusterDeletionPolicy::Orphan => "Orphan",
            },
            "writeConnectionSecretToRef": cluster.spec.write_connection_secret_to_ref.as_ref().map(|s| {
                json!({
                    "name": s.name,
                    "namespace": s.namespace.as_deref().unwrap_or(namespace)
                })
            })
        }
    });

    ctx.crossplane_client.apply_resource(&eks_cluster).await?;

    Ok(ManagedResourceRef {
        api_version: "eks.aws.upbound.io/v1beta1".to_string(),
        kind: "Cluster".to_string(),
        name: format!("{}-eks", name),
    })
}

// ============ AKS Implementation ============

async fn create_aks_cluster(
    cluster: &Cluster,
    ctx: &Context,
    namespace: &str,
    provider_config_name: &str,
) -> Result<ManagedResourceRef> {
    let name = cluster.name_any();
    let params = &cluster.spec.parameters;
    let azure_config = params.azure.as_ref()
        .ok_or_else(|| OperatorError::Config("Azure configuration required".into()))?;

    info!("Creating AKS cluster: {}", name);

    // Find system node pool
    let system_pool = cluster.spec.node_pools.iter()
        .find(|np| np.system_pool)
        .or_else(|| cluster.spec.node_pools.first())
        .ok_or_else(|| OperatorError::Config("At least one node pool required for AKS".into()))?;

    let mut for_provider = json!({
        "location": params.region,
        "resourceGroupName": azure_config.resource_group,
        "dnsPrefix": azure_config.dns_prefix.clone()
            .unwrap_or_else(|| format!("{}-dns", name)),
        "kubernetesVersion": params.kubernetes_version,
        "skuTier": match azure_config.sku_tier {
            AksSKUTier::Free => "Free",
            AksSKUTier::Standard => "Standard",
        },
        "defaultNodePool": [{
            "name": system_pool.name.clone(),
            "vmSize": system_pool.machine_type.clone(),
            "nodeCount": system_pool.node_count.unwrap_or(1),
            "enableAutoScaling": system_pool.autoscaling.as_ref().map(|a| a.enabled).unwrap_or(false),
            "minCount": system_pool.autoscaling.as_ref().map(|a| a.min_count),
            "maxCount": system_pool.autoscaling.as_ref().map(|a| a.max_count),
            "osDiskSizeGb": system_pool.disk_size_gb,
        }]
    });

    // Identity
    let identity_type = azure_config.identity_type.as_ref()
        .map(|t| match t {
            AksIdentityType::SystemAssigned => "SystemAssigned",
            AksIdentityType::UserAssigned => "UserAssigned",
        })
        .unwrap_or("SystemAssigned");

    for_provider["identity"] = json!([{"type": identity_type}]);

    // Azure AD
    if let Some(aad) = &azure_config.azure_ad_config {
        if aad.enabled {
            for_provider["azureActiveDirectoryRoleBasedAccessControl"] = json!([{
                "managed": true,
                "adminGroupObjectIds": aad.admin_group_object_ids,
                "azureRbacEnabled": aad.azure_rbac_enabled
            }]);
        }
    }

    // Network profile
    if let Some(network) = &cluster.spec.network {
        let mut network_profile = json!({"networkPlugin": "azure"});

        if let Some(pod_cidr) = &network.pod_cidr {
            network_profile["podCidr"] = json!(pod_cidr);
        }
        if let Some(service_cidr) = &network.service_cidr {
            network_profile["serviceCidr"] = json!(service_cidr);
        }

        for_provider["networkProfile"] = json!([network_profile]);

        if network.private_cluster {
            for_provider["apiServerAccessProfile"] = json!([{
                "enablePrivateCluster": true
            }]);
        }
    }

    let aks_cluster = json!({
        "apiVersion": "containerservice.azure.upbound.io/v1beta1",
        "kind": "KubernetesCluster",
        "metadata": {
            "name": format!("{}-aks", name),
            "labels": cluster.spec.labels
        },
        "spec": {
            "forProvider": for_provider,
            "providerConfigRef": {
                "name": provider_config_name
            },
            "deletionPolicy": match cluster.spec.deletion_policy {
                ClusterDeletionPolicy::Delete => "Delete",
                ClusterDeletionPolicy::Orphan => "Orphan",
            },
            "writeConnectionSecretToRef": cluster.spec.write_connection_secret_to_ref.as_ref().map(|s| {
                json!({
                    "name": s.name,
                    "namespace": s.namespace.as_deref().unwrap_or(namespace)
                })
            })
        }
    });

    ctx.crossplane_client.apply_resource(&aks_cluster).await?;

    Ok(ManagedResourceRef {
        api_version: "containerservice.azure.upbound.io/v1beta1".to_string(),
        kind: "KubernetesCluster".to_string(),
        name: format!("{}-aks", name),
    })
}

// ============ Node Pool Creation ============

async fn create_node_pools(
    cluster: &Cluster,
    ctx: &Context,
    _namespace: &str,
    provider_config_name: &str,
) -> Result<Vec<ManagedResourceRef>> {
    let mut resources = Vec::new();

    for pool in &cluster.spec.node_pools {
        // Skip system pool for AKS (created with cluster)
        if cluster.spec.provider == ClusterCloudProvider::Azure && pool.system_pool {
            continue;
        }

        let resource = match cluster.spec.provider {
            ClusterCloudProvider::Gcp => create_gke_node_pool(cluster, ctx, provider_config_name, pool).await?,
            ClusterCloudProvider::Aws => create_eks_node_group(cluster, ctx, provider_config_name, pool).await?,
            ClusterCloudProvider::Azure => create_aks_node_pool(cluster, ctx, provider_config_name, pool).await?,
        };

        resources.push(resource);
    }

    Ok(resources)
}

async fn create_gke_node_pool(
    cluster: &Cluster,
    ctx: &Context,
    provider_config_name: &str,
    pool: &NodePoolSpec,
) -> Result<ManagedResourceRef> {
    let cluster_name = cluster.name_any();
    let pool_name = format!("{}-{}", cluster_name, pool.name);
    let params = &cluster.spec.parameters;

    let mut for_provider = json!({
        "location": params.region,
        "cluster": format!("{}-gke", cluster_name),
        "nodeCount": pool.node_count.unwrap_or(1),
        "nodeConfig": [{
            "machineType": pool.machine_type,
            "diskSizeGb": pool.disk_size_gb,
            "preemptible": pool.spot,
            "labels": pool.labels,
        }],
        "management": [{
            "autoUpgrade": pool.auto_upgrade,
            "autoRepair": pool.auto_repair
        }]
    });

    if let Some(autoscaling) = &pool.autoscaling {
        if autoscaling.enabled {
            for_provider["autoscaling"] = json!([{
                "minNodeCount": autoscaling.min_count,
                "maxNodeCount": autoscaling.max_count
            }]);
        }
    }

    let node_pool = json!({
        "apiVersion": "container.gcp.upbound.io/v1beta1",
        "kind": "NodePool",
        "metadata": {"name": pool_name},
        "spec": {
            "forProvider": for_provider,
            "providerConfigRef": {"name": provider_config_name}
        }
    });

    ctx.crossplane_client.apply_resource(&node_pool).await?;

    Ok(ManagedResourceRef {
        api_version: "container.gcp.upbound.io/v1beta1".to_string(),
        kind: "NodePool".to_string(),
        name: pool_name,
    })
}

async fn create_eks_node_group(
    cluster: &Cluster,
    ctx: &Context,
    provider_config_name: &str,
    pool: &NodePoolSpec,
) -> Result<ManagedResourceRef> {
    let cluster_name = cluster.name_any();
    let pool_name = format!("{}-{}", cluster_name, pool.name);
    let params = &cluster.spec.parameters;

    let for_provider = json!({
        "region": params.region,
        "clusterNameRef": {"name": format!("{}-eks", cluster_name)},
        "instanceTypes": [pool.machine_type],
        "scalingConfig": [{
            "desiredSize": pool.node_count.unwrap_or(1),
            "minSize": pool.autoscaling.as_ref().map(|a| a.min_count).unwrap_or(1),
            "maxSize": pool.autoscaling.as_ref().map(|a| a.max_count).unwrap_or(pool.node_count.unwrap_or(1))
        }],
        "diskSize": pool.disk_size_gb,
        "capacityType": if pool.spot { "SPOT" } else { "ON_DEMAND" },
        "labels": pool.labels,
    });

    let node_group = json!({
        "apiVersion": "eks.aws.upbound.io/v1beta1",
        "kind": "NodeGroup",
        "metadata": {"name": pool_name},
        "spec": {
            "forProvider": for_provider,
            "providerConfigRef": {"name": provider_config_name}
        }
    });

    ctx.crossplane_client.apply_resource(&node_group).await?;

    Ok(ManagedResourceRef {
        api_version: "eks.aws.upbound.io/v1beta1".to_string(),
        kind: "NodeGroup".to_string(),
        name: pool_name,
    })
}

async fn create_aks_node_pool(
    cluster: &Cluster,
    ctx: &Context,
    provider_config_name: &str,
    pool: &NodePoolSpec,
) -> Result<ManagedResourceRef> {
    let cluster_name = cluster.name_any();
    let pool_name = format!("{}-{}", cluster_name, pool.name);

    let for_provider = json!({
        "kubernetesClusterIdRef": {"name": format!("{}-aks", cluster_name)},
        "vmSize": pool.machine_type,
        "nodeCount": pool.node_count.unwrap_or(1),
        "enableAutoScaling": pool.autoscaling.as_ref().map(|a| a.enabled).unwrap_or(false),
        "minCount": pool.autoscaling.as_ref().map(|a| a.min_count),
        "maxCount": pool.autoscaling.as_ref().map(|a| a.max_count),
        "osDiskSizeGb": pool.disk_size_gb,
        "priority": if pool.spot { "Spot" } else { "Regular" },
        "nodeLabels": pool.labels,
    });

    let node_pool = json!({
        "apiVersion": "containerservice.azure.upbound.io/v1beta1",
        "kind": "KubernetesClusterNodePool",
        "metadata": {"name": pool_name},
        "spec": {
            "forProvider": for_provider,
            "providerConfigRef": {"name": provider_config_name}
        }
    });

    ctx.crossplane_client.apply_resource(&node_pool).await?;

    Ok(ManagedResourceRef {
        api_version: "containerservice.azure.upbound.io/v1beta1".to_string(),
        kind: "KubernetesClusterNodePool".to_string(),
        name: pool_name,
    })
}

// ============ Status Helpers ============

async fn update_phase(
    api: &Api<Cluster>,
    name: &str,
    phase: ClusterPhase,
    message: &str,
) -> Result<()> {
    let status = ClusterStatus {
        phase,
        message: Some(message.to_string()),
        last_reconcile_time: Some(Utc::now().to_rfc3339()),
        ..Default::default()
    };

    let patch = json!({ "status": status });
    api.patch_status(name, &PatchParams::default(), &Patch::Merge(&patch)).await?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn update_status_full(
    api: &Api<Cluster>,
    name: &str,
    phase: ClusterPhase,
    cluster_resource: &ManagedResourceRef,
    node_pool_resources: &[ManagedResourceRef],
    node_pool_statuses: &[NodePoolStatus],
    provider_config_name: &str,
    credential_status: VaultCredentialStatus,
    ready: bool,
) -> Result<()> {
    let status = ClusterStatus {
        phase,
        ready,
        synced: ready,
        node_pools: node_pool_statuses.to_vec(),
        managed_resources: Some(ManagedClusterResources {
            provider_config: provider_config_name.to_string(),
            cluster: cluster_resource.clone(),
            node_pools: node_pool_resources.to_vec(),
        }),
        vault_credential_status: Some(credential_status),
        last_reconcile_time: Some(Utc::now().to_rfc3339()),
        ..Default::default()
    };

    let patch = json!({ "status": status });
    api.patch_status(name, &PatchParams::default(), &Patch::Merge(&patch)).await?;

    Ok(())
}

async fn check_cluster_ready(ctx: &Context, resource: &ManagedResourceRef) -> Result<bool> {
    match ctx.crossplane_client.get_xr_status(
        &resource.api_version,
        &resource.kind,
        &resource.name,
    ).await {
        Ok(Some(status)) => Ok(status.ready && status.synced),
        Ok(None) => Ok(false),
        Err(_) => Ok(false),
    }
}

async fn check_node_pools_ready(
    ctx: &Context,
    resources: &[ManagedResourceRef],
) -> Result<Vec<NodePoolStatus>> {
    let mut statuses = Vec::new();

    for resource in resources {
        let ready = match ctx.crossplane_client.get_xr_status(
            &resource.api_version,
            &resource.kind,
            &resource.name,
        ).await {
            Ok(Some(status)) => status.ready && status.synced,
            _ => false,
        };

        statuses.push(NodePoolStatus {
            name: resource.name.clone(),
            ready,
            node_count: 0,
            message: None,
        });
    }

    Ok(statuses)
}

/// Error policy for the controller
pub fn error_policy(
    cluster: Arc<Cluster>,
    error: &OperatorError,
    _ctx: Arc<Context>,
) -> Action {
    warn!(
        "Reconcile error for Cluster {}: {}",
        cluster.name_any(),
        error
    );

    Action::requeue(Duration::from_secs(30))
}
