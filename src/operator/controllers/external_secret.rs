use crate::operator::{Context, OperatorError, Result};
use crate::operator::types::{
    ExternalSecretConfig, ExternalSecretConfigStatus, ExternalSecretCondition,
    ExternalSecretPhase, ExternalSecretRef, SecretProviderType, SecretSyncStatus,
};
use chrono::Utc;
use kube::api::{Patch, PatchParams};
use kube::runtime::controller::Action;
use kube::{Api, ResourceExt};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

/// Reconciles ExternalSecretConfig resources
pub async fn reconcile(
    config: Arc<ExternalSecretConfig>,
    ctx: Arc<Context>,
) -> std::result::Result<Action, OperatorError> {
    let name = config.name_any();
    let namespace = config.namespace().unwrap_or_else(|| "default".to_string());

    info!("Reconciling ExternalSecretConfig {}/{}", namespace, name);

    let api: Api<ExternalSecretConfig> = Api::namespaced(ctx.client.clone(), &namespace);

    // Update status to Creating
    update_phase(&api, &name, ExternalSecretPhase::Creating, "Creating ESO resources").await?;

    // Create SecretStore or ClusterSecretStore
    let store_name = create_secret_store(&config, &ctx, &namespace).await?;

    // Create ExternalSecrets or ClusterExternalSecrets
    let external_secrets = create_external_secrets(&config, &ctx, &namespace, &store_name).await?;

    // Update status to Ready
    update_status_full(&api, &name, &store_name, external_secrets).await?;

    info!("ExternalSecretConfig {}/{} is Ready", namespace, name);

    // Requeue to check sync status
    Ok(Action::requeue(Duration::from_secs(60)))
}

async fn create_secret_store(
    config: &ExternalSecretConfig,
    ctx: &Context,
    namespace: &str,
) -> Result<String> {
    let provider = &config.spec.provider;
    let store_name = provider.name.clone();

    info!("Creating SecretStore {} for provider {:?}", store_name, provider.provider_type);

    let (api_version, kind) = if config.spec.cluster_scoped {
        ("external-secrets.io/v1beta1", "ClusterSecretStore")
    } else {
        ("external-secrets.io/v1beta1", "SecretStore")
    };

    // Build provider spec based on type
    let provider_spec = build_provider_spec(provider)?;

    let store = if config.spec.cluster_scoped {
        serde_json::json!({
            "apiVersion": api_version,
            "kind": kind,
            "metadata": {
                "name": store_name
            },
            "spec": {
                "provider": provider_spec,
                "conditions": config.spec.namespace_selector.as_ref().map(|ns| {
                    serde_json::json!([{
                        "namespaceSelector": ns
                    }])
                })
            }
        })
    } else {
        serde_json::json!({
            "apiVersion": api_version,
            "kind": kind,
            "metadata": {
                "name": store_name,
                "namespace": namespace
            },
            "spec": {
                "provider": provider_spec
            }
        })
    };

    // Apply the store
    apply_eso_resource(&ctx.client, &store).await?;

    Ok(store_name)
}

fn build_provider_spec(provider: &crate::operator::types::SecretProviderSpec) -> Result<serde_json::Value> {
    match provider.provider_type {
        SecretProviderType::Gcp => {
            let gcp = provider.gcp.as_ref()
                .ok_or_else(|| OperatorError::Config("GCP provider config required".into()))?;

            Ok(serde_json::json!({
                "gcpsm": {
                    "projectID": gcp.project_id,
                    "auth": {
                        "secretRef": {
                            "secretAccessKeySecretRef": {
                                "name": gcp.auth.secret_ref.name,
                                "key": gcp.auth.secret_ref.key,
                                "namespace": gcp.auth.secret_ref.namespace
                            }
                        }
                    }
                }
            }))
        }
        SecretProviderType::Aws => {
            let aws = provider.aws.as_ref()
                .ok_or_else(|| OperatorError::Config("AWS provider config required".into()))?;

            let mut spec = serde_json::json!({
                "aws": {
                    "service": match aws.service {
                        crate::operator::types::AwsService::SecretsManager => "SecretsManager",
                        crate::operator::types::AwsService::ParameterStore => "ParameterStore",
                    },
                    "region": aws.region
                }
            });

            if let Some(auth) = &aws.auth {
                if auth.use_irsa {
                    // IRSA doesn't need explicit credentials
                } else if let Some(secret_ref) = &auth.secret_ref {
                    spec["aws"]["auth"] = serde_json::json!({
                        "secretRef": {
                            "accessKeyIDSecretRef": {
                                "name": secret_ref.access_key_id.name,
                                "key": secret_ref.access_key_id.key
                            },
                            "secretAccessKeySecretRef": {
                                "name": secret_ref.secret_access_key.name,
                                "key": secret_ref.secret_access_key.key
                            }
                        }
                    });
                }
            }

            Ok(spec)
        }
        SecretProviderType::Azure => {
            let azure = provider.azure.as_ref()
                .ok_or_else(|| OperatorError::Config("Azure provider config required".into()))?;

            let mut spec = serde_json::json!({
                "azurekv": {
                    "tenantId": azure.tenant_id,
                    "vaultUrl": azure.vault_url
                }
            });

            if azure.auth.use_workload_identity {
                spec["azurekv"]["authType"] = serde_json::json!("WorkloadIdentity");
            } else if let Some(secret_ref) = &azure.auth.secret_ref {
                spec["azurekv"]["authSecretRef"] = serde_json::json!({
                    "clientId": {
                        "name": secret_ref.client_id.name,
                        "key": secret_ref.client_id.key
                    },
                    "clientSecret": {
                        "name": secret_ref.client_secret.name,
                        "key": secret_ref.client_secret.key
                    }
                });
            }

            Ok(spec)
        }
        SecretProviderType::Vault => {
            let vault = provider.vault.as_ref()
                .ok_or_else(|| OperatorError::Config("Vault provider config required".into()))?;

            let mut spec = serde_json::json!({
                "vault": {
                    "server": vault.server,
                    "path": vault.path,
                    "version": vault.version
                }
            });

            if let Some(token_ref) = &vault.auth.token_secret_ref {
                spec["vault"]["auth"] = serde_json::json!({
                    "tokenSecretRef": {
                        "name": token_ref.name,
                        "key": token_ref.key
                    }
                });
            } else if let Some(k8s_auth) = &vault.auth.kubernetes {
                spec["vault"]["auth"] = serde_json::json!({
                    "kubernetes": {
                        "mountPath": k8s_auth.mount_path,
                        "role": k8s_auth.role,
                        "serviceAccountRef": k8s_auth.service_account_ref.as_ref().map(|sa| {
                            serde_json::json!({
                                "name": sa.name,
                                "namespace": sa.namespace
                            })
                        })
                    }
                });
            } else if let Some(app_role) = &vault.auth.app_role {
                spec["vault"]["auth"] = serde_json::json!({
                    "appRole": {
                        "path": app_role.path,
                        "roleId": {
                            "name": app_role.role_id.name,
                            "key": app_role.role_id.key
                        },
                        "secretRef": {
                            "name": app_role.secret_id.name,
                            "key": app_role.secret_id.key
                        }
                    }
                });
            }

            Ok(spec)
        }
        SecretProviderType::OnePassword => {
            let op = provider.onepassword.as_ref()
                .ok_or_else(|| OperatorError::Config("1Password provider config required".into()))?;

            Ok(serde_json::json!({
                "onepassword": {
                    "connectHost": op.connect_host,
                    "vaults": op.vaults,
                    "auth": {
                        "secretRef": {
                            "connectTokenSecretRef": {
                                "name": op.auth.secret_ref.name,
                                "key": op.auth.secret_ref.key
                            }
                        }
                    }
                }
            }))
        }
    }
}

async fn create_external_secrets(
    config: &ExternalSecretConfig,
    ctx: &Context,
    namespace: &str,
    store_name: &str,
) -> Result<Vec<ExternalSecretRef>> {
    let mut refs = Vec::new();

    for secret_spec in &config.spec.secrets {
        info!("Creating ExternalSecret {}", secret_spec.name);

        let (api_version, kind, store_kind) = if config.spec.cluster_scoped {
            ("external-secrets.io/v1beta1", "ClusterExternalSecret", "ClusterSecretStore")
        } else {
            ("external-secrets.io/v1beta1", "ExternalSecret", "SecretStore")
        };

        let refresh_interval = secret_spec.refresh_interval
            .as_ref()
            .unwrap_or(&config.spec.refresh_interval);

        let target_secret = secret_spec.target_secret_name
            .as_ref()
            .unwrap_or(&secret_spec.name);

        // Build data and dataFrom specs
        let data: Vec<serde_json::Value> = secret_spec.data.iter().map(|d| {
            let mut remote_ref = serde_json::json!({
                "key": d.remote_ref.key
            });
            if let Some(prop) = &d.remote_ref.property {
                remote_ref["property"] = serde_json::json!(prop);
            }
            if let Some(ver) = &d.remote_ref.version {
                remote_ref["version"] = serde_json::json!(ver);
            }
            serde_json::json!({
                "secretKey": d.secret_key,
                "remoteRef": remote_ref
            })
        }).collect();

        let data_from: Vec<serde_json::Value> = secret_spec.data_from.iter().map(|df| {
            if let Some(extract) = &df.extract {
                let mut spec = serde_json::json!({
                    "extract": {
                        "key": extract.key
                    }
                });
                if let Some(ver) = &extract.version {
                    spec["extract"]["version"] = serde_json::json!(ver);
                }
                spec
            } else if let Some(find) = &df.find {
                let mut spec = serde_json::json!({"find": {}});
                if let Some(name) = &find.name {
                    spec["find"]["name"] = serde_json::json!({"regexp": name.regexp});
                }
                if let Some(tags) = &find.tags {
                    spec["find"]["tags"] = serde_json::to_value(tags).unwrap_or_default();
                }
                spec
            } else {
                serde_json::json!({})
            }
        }).collect();

        let external_secret_spec = serde_json::json!({
            "refreshInterval": refresh_interval,
            "secretStoreRef": {
                "name": store_name,
                "kind": store_kind
            },
            "target": {
                "name": target_secret,
                "creationPolicy": "Owner"
            },
            "data": if data.is_empty() { None } else { Some(data) },
            "dataFrom": if data_from.is_empty() { None } else { Some(data_from) }
        });

        let external_secret = if config.spec.cluster_scoped {
            serde_json::json!({
                "apiVersion": api_version,
                "kind": kind,
                "metadata": {
                    "name": secret_spec.name
                },
                "spec": {
                    "externalSecretName": secret_spec.name,
                    "namespaceSelector": config.spec.namespace_selector,
                    "refreshTime": refresh_interval,
                    "externalSecretSpec": external_secret_spec
                }
            })
        } else {
            let target_ns = secret_spec.target_namespace.as_deref().unwrap_or(namespace);
            serde_json::json!({
                "apiVersion": api_version,
                "kind": kind,
                "metadata": {
                    "name": secret_spec.name,
                    "namespace": target_ns
                },
                "spec": external_secret_spec
            })
        };

        apply_eso_resource(&ctx.client, &external_secret).await?;

        refs.push(ExternalSecretRef {
            name: secret_spec.name.clone(),
            namespace: if config.spec.cluster_scoped {
                None
            } else {
                Some(secret_spec.target_namespace.clone().unwrap_or_else(|| namespace.to_string()))
            },
            status: SecretSyncStatus::Synced,
            target_secret: Some(target_secret.clone()),
        });
    }

    Ok(refs)
}

async fn apply_eso_resource(client: &kube::Client, resource: &serde_json::Value) -> Result<()> {
    let api_version = resource.get("apiVersion")
        .and_then(|v| v.as_str())
        .ok_or_else(|| OperatorError::Config("Missing apiVersion".into()))?;

    let kind = resource.get("kind")
        .and_then(|v| v.as_str())
        .ok_or_else(|| OperatorError::Config("Missing kind".into()))?;

    let name = resource.get("metadata")
        .and_then(|m| m.get("name"))
        .and_then(|n| n.as_str())
        .ok_or_else(|| OperatorError::Config("Missing metadata.name".into()))?;

    let namespace = resource.get("metadata")
        .and_then(|m| m.get("namespace"))
        .and_then(|n| n.as_str());

    let (group, version) = parse_api_version(api_version)?;

    let gvk = kube::api::GroupVersionKind {
        group: group.to_string(),
        version: version.to_string(),
        kind: kind.to_string(),
    };

    let api_resource = kube::discovery::ApiResource::from_gvk(&gvk);

    let api: Api<kube::api::DynamicObject> = if let Some(ns) = namespace {
        Api::namespaced_with(client.clone(), ns, &api_resource)
    } else {
        Api::all_with(client.clone(), &api_resource)
    };

    let patch_params = PatchParams::apply("platform-operator.yurikrupnik.com");
    let obj: kube::api::DynamicObject = serde_json::from_value(resource.clone())?;

    api.patch(name, &patch_params, &Patch::Apply(&obj)).await?;

    Ok(())
}

fn parse_api_version(api_version: &str) -> Result<(&str, &str)> {
    let parts: Vec<&str> = api_version.split('/').collect();
    match parts.len() {
        1 => Ok(("", parts[0])),
        2 => Ok((parts[0], parts[1])),
        _ => Err(OperatorError::Config(format!("Invalid apiVersion: {}", api_version))),
    }
}

async fn update_phase(
    api: &Api<ExternalSecretConfig>,
    name: &str,
    phase: ExternalSecretPhase,
    message: &str,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();

    let status = ExternalSecretConfigStatus {
        phase,
        synced: false,
        conditions: vec![ExternalSecretCondition {
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

    api.patch_status(name, &PatchParams::apply("platform-operator"), &Patch::Merge(&patch))
        .await?;

    Ok(())
}

async fn update_status_full(
    api: &Api<ExternalSecretConfig>,
    name: &str,
    store_name: &str,
    external_secrets: Vec<ExternalSecretRef>,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();
    let all_synced = external_secrets.iter().all(|es| es.status == SecretSyncStatus::Synced);

    let status = ExternalSecretConfigStatus {
        phase: ExternalSecretPhase::Ready,
        synced: all_synced,
        secret_store_name: Some(store_name.to_string()),
        external_secrets,
        last_sync_time: Some(now.clone()),
        conditions: vec![ExternalSecretCondition {
            condition_type: "Ready".to_string(),
            status: "True".to_string(),
            reason: "Succeeded".to_string(),
            message: "All ESO resources created successfully".to_string(),
            last_transition_time: now,
        }],
        ..Default::default()
    };

    let patch = serde_json::json!({
        "status": status
    });

    api.patch_status(name, &PatchParams::apply("platform-operator"), &Patch::Merge(&patch))
        .await?;

    Ok(())
}

/// Error policy for the controller
pub fn error_policy(
    config: Arc<ExternalSecretConfig>,
    error: &OperatorError,
    _ctx: Arc<Context>,
) -> Action {
    warn!(
        "Reconcile error for ExternalSecretConfig {}: {}",
        config.name_any(),
        error
    );

    Action::requeue(Duration::from_secs(30))
}
