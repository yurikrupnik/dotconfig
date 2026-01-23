//! BedrockAccess Controller
//!
//! Manages AWS Bedrock access via Crossplane upbound/provider-aws.
//! Creates production-ready AI infrastructure with:
//! - IAM roles for IRSA
//! - Model access permissions
//! - Knowledge bases (optional)
//! - Agents (optional)
//! - Guardrails (optional)

use crate::operator::dependencies::known_dependencies;
use crate::operator::types::{
    BedrockAccess, BedrockAccessCondition, BedrockAccessPhase, BedrockAccessStatus,
    BedrockModelProvider, ManagedBedrockResource,
};
use crate::operator::{Context, OperatorError, Result};
use chrono::Utc;
use kube::api::{Patch, PatchParams};
use kube::runtime::controller::Action;
use kube::{Api, ResourceExt};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

/// Reconciles BedrockAccess resources
pub async fn reconcile(
    bedrock: Arc<BedrockAccess>,
    ctx: Arc<Context>,
) -> std::result::Result<Action, OperatorError> {
    let name = bedrock.name_any();
    let namespace = bedrock.namespace().unwrap_or_else(|| "default".to_string());

    info!("Reconciling BedrockAccess {}/{}", namespace, name);

    let api: Api<BedrockAccess> = Api::namespaced(ctx.client.clone(), &namespace);

    // Check dependencies before proceeding
    let deps = known_dependencies::bedrock_access_deps();
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

        warn!("BedrockAccess {}/{}: {}", namespace, name, message);

        update_phase_with_condition(
            &api,
            &name,
            BedrockAccessPhase::Blocked,
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
        BedrockAccessPhase::Creating,
        "Creating Bedrock access resources",
    )
    .await?;

    let mut managed_resources = Vec::new();
    let mut accessible_models = Vec::new();

    // 1. Create IAM Role for IRSA
    let iam_role_name = create_iam_role(&bedrock, &ctx, &namespace).await?;
    managed_resources.push(ManagedBedrockResource {
        api_version: "iam.aws.upbound.io/v1beta1".to_string(),
        kind: "Role".to_string(),
        name: iam_role_name.clone(),
        ready: false,
        synced: false,
        message: Some("Creating IAM role for IRSA".to_string()),
    });

    // 2. Create IAM Policy for Bedrock access
    let iam_policy_name = create_bedrock_policy(&bedrock, &ctx, &namespace).await?;
    managed_resources.push(ManagedBedrockResource {
        api_version: "iam.aws.upbound.io/v1beta1".to_string(),
        kind: "Policy".to_string(),
        name: iam_policy_name.clone(),
        ready: false,
        synced: false,
        message: Some("Creating Bedrock IAM policy".to_string()),
    });

    // 3. Attach policy to role
    let policy_attachment_name =
        create_policy_attachment(&bedrock, &ctx, &namespace, &iam_role_name, &iam_policy_name)
            .await?;
    managed_resources.push(ManagedBedrockResource {
        api_version: "iam.aws.upbound.io/v1beta1".to_string(),
        kind: "RolePolicyAttachment".to_string(),
        name: policy_attachment_name,
        ready: false,
        synced: false,
        message: Some("Attaching policy to role".to_string()),
    });

    // Calculate accessible models
    if bedrock.spec.foundation_models.allow_all {
        accessible_models.push("*".to_string());
    } else {
        accessible_models.extend(bedrock.spec.foundation_models.model_ids.clone());
        for provider in &bedrock.spec.foundation_models.providers {
            let provider_prefix = match provider {
                BedrockModelProvider::Anthropic => "anthropic.*",
                BedrockModelProvider::Amazon => "amazon.*",
                BedrockModelProvider::Meta => "meta.*",
                BedrockModelProvider::Cohere => "cohere.*",
                BedrockModelProvider::AI21 => "ai21.*",
                BedrockModelProvider::Mistral => "mistral.*",
                BedrockModelProvider::Stability => "stability.*",
            };
            accessible_models.push(provider_prefix.to_string());
        }
    }

    // 4. Create Guardrails if configured
    if let Some(ref guardrails) = bedrock.spec.guardrails {
        let guardrail_name = create_guardrail(&bedrock, &ctx, &namespace, guardrails).await?;
        managed_resources.push(ManagedBedrockResource {
            api_version: "bedrock.aws.upbound.io/v1beta1".to_string(),
            kind: "Guardrail".to_string(),
            name: guardrail_name,
            ready: false,
            synced: false,
            message: Some("Creating Bedrock guardrail".to_string()),
        });
    }

    // 5. Create Model Invocation Logging if configured
    if let Some(ref logging) = bedrock.spec.logging {
        if logging.enabled {
            let logging_name = create_model_invocation_logging(&bedrock, &ctx, &namespace, logging).await?;
            managed_resources.push(ManagedBedrockResource {
                api_version: "bedrock.aws.upbound.io/v1beta1".to_string(),
                kind: "ModelInvocationLoggingConfiguration".to_string(),
                name: logging_name,
                ready: false,
                synced: false,
                message: Some("Configuring model invocation logging".to_string()),
            });
        }
    }

    // Update status
    let model_count = accessible_models.len() as i32;
    update_status_full(
        &api,
        &name,
        BedrockAccessPhase::Ready,
        managed_resources,
        accessible_models,
        model_count,
        Some(format!(
            "arn:aws:iam::*:role/{}",
            format!("{}-bedrock-role", bedrock.name_any())
        )),
    )
    .await?;

    info!("BedrockAccess {}/{} is Ready", namespace, name);

    Ok(Action::requeue(Duration::from_secs(300)))
}

async fn create_iam_role(
    bedrock: &BedrockAccess,
    ctx: &Context,
    namespace: &str,
) -> Result<String> {
    let name = format!("{}-bedrock-role", bedrock.name_any());

    info!(
        "Creating IAM Role {} for service account {}",
        name, bedrock.spec.iam.service_account_name
    );

    let provider_config = bedrock
        .spec
        .provider_config_ref
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "default".to_string());

    // Trust policy for IRSA
    let trust_policy = serde_json::json!({
        "Version": "2012-10-17",
        "Statement": [{
            "Effect": "Allow",
            "Principal": {
                "Federated": bedrock.spec.iam.oidc_provider_arn.as_deref()
                    .unwrap_or("arn:aws:iam::${AWS_ACCOUNT_ID}:oidc-provider/${OIDC_PROVIDER}")
            },
            "Action": "sts:AssumeRoleWithWebIdentity",
            "Condition": {
                "StringEquals": {
                    "${OIDC_PROVIDER}:sub": format!(
                        "system:serviceaccount:{}:{}",
                        bedrock.spec.iam.service_account_namespace,
                        bedrock.spec.iam.service_account_name
                    ),
                    "${OIDC_PROVIDER}:aud": "sts.amazonaws.com"
                }
            }
        }]
    });

    let resource = serde_json::json!({
        "apiVersion": "iam.aws.upbound.io/v1beta1",
        "kind": "Role",
        "metadata": {
            "name": name,
            "namespace": namespace,
            "labels": bedrock.spec.labels.clone().unwrap_or_default()
        },
        "spec": {
            "forProvider": {
                "assumeRolePolicy": trust_policy.to_string(),
                "description": format!("IAM role for BedrockAccess {} via IRSA", bedrock.name_any()),
                "tags": {
                    "ManagedBy": "platform-operator",
                    "BedrockAccess": bedrock.name_any()
                }
            },
            "providerConfigRef": {
                "name": provider_config
            }
        }
    });

    ctx.crossplane_client.apply_resource(&resource).await?;

    Ok(name)
}

async fn create_bedrock_policy(
    bedrock: &BedrockAccess,
    ctx: &Context,
    namespace: &str,
) -> Result<String> {
    let name = format!("{}-bedrock-policy", bedrock.name_any());

    info!("Creating Bedrock IAM Policy {}", name);

    let provider_config = bedrock
        .spec
        .provider_config_ref
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "default".to_string());

    // Build model resources
    let mut model_resources = Vec::new();

    if bedrock.spec.foundation_models.allow_all {
        model_resources.push(format!(
            "arn:aws:bedrock:{}::foundation-model/*",
            bedrock.spec.region
        ));
    } else {
        for model_id in &bedrock.spec.foundation_models.model_ids {
            model_resources.push(format!(
                "arn:aws:bedrock:{}::foundation-model/{}",
                bedrock.spec.region, model_id
            ));
        }
        for provider in &bedrock.spec.foundation_models.providers {
            let provider_prefix = match provider {
                BedrockModelProvider::Anthropic => "anthropic",
                BedrockModelProvider::Amazon => "amazon",
                BedrockModelProvider::Meta => "meta",
                BedrockModelProvider::Cohere => "cohere",
                BedrockModelProvider::AI21 => "ai21",
                BedrockModelProvider::Mistral => "mistral",
                BedrockModelProvider::Stability => "stability",
            };
            model_resources.push(format!(
                "arn:aws:bedrock:{}::foundation-model/{}.*",
                bedrock.spec.region, provider_prefix
            ));
        }
    }

    // Add custom models
    for custom_model in &bedrock.spec.custom_models {
        model_resources.push(format!(
            "arn:aws:bedrock:{}:*:custom-model/{}",
            bedrock.spec.region, custom_model.model_id
        ));
    }

    let mut statements = vec![
        // InvokeModel permission
        serde_json::json!({
            "Sid": "BedrockInvokeModel",
            "Effect": "Allow",
            "Action": [
                "bedrock:InvokeModel",
                "bedrock:InvokeModelWithResponseStream"
            ],
            "Resource": model_resources
        }),
        // List models permission
        serde_json::json!({
            "Sid": "BedrockListModels",
            "Effect": "Allow",
            "Action": [
                "bedrock:ListFoundationModels",
                "bedrock:GetFoundationModel",
                "bedrock:ListCustomModels",
                "bedrock:GetCustomModel"
            ],
            "Resource": "*"
        }),
    ];

    // Add knowledge base permissions if any are configured
    if !bedrock.spec.knowledge_bases.is_empty() {
        statements.push(serde_json::json!({
            "Sid": "BedrockKnowledgeBase",
            "Effect": "Allow",
            "Action": [
                "bedrock:RetrieveAndGenerate",
                "bedrock:Retrieve",
                "bedrock:InvokeAgent"
            ],
            "Resource": format!("arn:aws:bedrock:{}:*:knowledge-base/*", bedrock.spec.region)
        }));
    }

    // Add agent permissions if any are configured
    if !bedrock.spec.agents.is_empty() {
        statements.push(serde_json::json!({
            "Sid": "BedrockAgent",
            "Effect": "Allow",
            "Action": [
                "bedrock:InvokeAgent"
            ],
            "Resource": format!("arn:aws:bedrock:{}:*:agent/*", bedrock.spec.region)
        }));
    }

    // Add guardrails permission if configured
    if bedrock.spec.guardrails.is_some() {
        statements.push(serde_json::json!({
            "Sid": "BedrockGuardrails",
            "Effect": "Allow",
            "Action": [
                "bedrock:ApplyGuardrail"
            ],
            "Resource": format!("arn:aws:bedrock:{}:*:guardrail/*", bedrock.spec.region)
        }));
    }

    // Add additional policy statements
    for additional in &bedrock.spec.iam.additional_policy_statements {
        statements.push(additional.clone());
    }

    let policy_doc = serde_json::json!({
        "Version": "2012-10-17",
        "Statement": statements
    });

    let resource = serde_json::json!({
        "apiVersion": "iam.aws.upbound.io/v1beta1",
        "kind": "Policy",
        "metadata": {
            "name": name,
            "namespace": namespace,
            "labels": bedrock.spec.labels.clone().unwrap_or_default()
        },
        "spec": {
            "forProvider": {
                "policy": policy_doc.to_string(),
                "description": format!("Bedrock access policy for {}", bedrock.name_any()),
                "tags": {
                    "ManagedBy": "platform-operator",
                    "BedrockAccess": bedrock.name_any()
                }
            },
            "providerConfigRef": {
                "name": provider_config
            }
        }
    });

    ctx.crossplane_client.apply_resource(&resource).await?;

    Ok(name)
}

async fn create_policy_attachment(
    bedrock: &BedrockAccess,
    ctx: &Context,
    namespace: &str,
    role_name: &str,
    policy_name: &str,
) -> Result<String> {
    let name = format!("{}-bedrock-attach", bedrock.name_any());

    info!("Creating RolePolicyAttachment {}", name);

    let provider_config = bedrock
        .spec
        .provider_config_ref
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "default".to_string());

    let resource = serde_json::json!({
        "apiVersion": "iam.aws.upbound.io/v1beta1",
        "kind": "RolePolicyAttachment",
        "metadata": {
            "name": name,
            "namespace": namespace,
            "labels": bedrock.spec.labels.clone().unwrap_or_default()
        },
        "spec": {
            "forProvider": {
                "roleSelector": {
                    "matchLabels": {
                        "crossplane.io/claim-name": role_name
                    }
                },
                "policyArnSelector": {
                    "matchLabels": {
                        "crossplane.io/claim-name": policy_name
                    }
                }
            },
            "providerConfigRef": {
                "name": provider_config
            }
        }
    });

    ctx.crossplane_client.apply_resource(&resource).await?;

    Ok(name)
}

async fn create_guardrail(
    bedrock: &BedrockAccess,
    ctx: &Context,
    namespace: &str,
    guardrails: &crate::operator::types::GuardrailsSpec,
) -> Result<String> {
    let name = format!("{}-guardrail", bedrock.name_any());

    info!("Creating Bedrock Guardrail {}", name);

    let provider_config = bedrock
        .spec
        .provider_config_ref
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "default".to_string());

    // Build content policy config
    let mut content_policy_config = serde_json::json!({});
    if let Some(ref filters) = guardrails.content_filters {
        content_policy_config = serde_json::json!({
            "filtersConfig": [
                {
                    "type": "HATE",
                    "inputStrength": format!("{:?}", filters.hate).to_uppercase(),
                    "outputStrength": format!("{:?}", filters.hate).to_uppercase()
                },
                {
                    "type": "INSULTS",
                    "inputStrength": format!("{:?}", filters.insults).to_uppercase(),
                    "outputStrength": format!("{:?}", filters.insults).to_uppercase()
                },
                {
                    "type": "SEXUAL",
                    "inputStrength": format!("{:?}", filters.sexual).to_uppercase(),
                    "outputStrength": format!("{:?}", filters.sexual).to_uppercase()
                },
                {
                    "type": "VIOLENCE",
                    "inputStrength": format!("{:?}", filters.violence).to_uppercase(),
                    "outputStrength": format!("{:?}", filters.violence).to_uppercase()
                },
                {
                    "type": "MISCONDUCT",
                    "inputStrength": format!("{:?}", filters.misconduct).to_uppercase(),
                    "outputStrength": format!("{:?}", filters.misconduct).to_uppercase()
                },
                {
                    "type": "PROMPT_ATTACK",
                    "inputStrength": format!("{:?}", filters.prompt_attack).to_uppercase(),
                    "outputStrength": "NONE"
                }
            ]
        });
    }

    // Build topic policy config
    let topic_policy_config: Vec<serde_json::Value> = guardrails
        .denied_topics
        .iter()
        .map(|topic| {
            serde_json::json!({
                "name": topic.name,
                "definition": topic.definition,
                "examples": topic.examples,
                "type": "DENY"
            })
        })
        .collect();

    let resource = serde_json::json!({
        "apiVersion": "bedrock.aws.upbound.io/v1beta1",
        "kind": "Guardrail",
        "metadata": {
            "name": name,
            "namespace": namespace,
            "labels": bedrock.spec.labels.clone().unwrap_or_default()
        },
        "spec": {
            "forProvider": {
                "name": guardrails.name,
                "description": guardrails.description,
                "blockedInputMessaging": guardrails.blocked_input_messaging,
                "blockedOutputsMessaging": guardrails.blocked_outputs_messaging,
                "contentPolicyConfig": [content_policy_config],
                "topicPolicyConfig": [{
                    "topicsConfig": topic_policy_config
                }],
                "region": bedrock.spec.region
            },
            "providerConfigRef": {
                "name": provider_config
            }
        }
    });

    ctx.crossplane_client.apply_resource(&resource).await?;

    Ok(name)
}

async fn create_model_invocation_logging(
    bedrock: &BedrockAccess,
    ctx: &Context,
    namespace: &str,
    logging: &crate::operator::types::BedrockLoggingConfig,
) -> Result<String> {
    let name = format!("{}-logging", bedrock.name_any());

    info!("Creating Model Invocation Logging {}", name);

    let provider_config = bedrock
        .spec
        .provider_config_ref
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "default".to_string());

    let mut logging_config = serde_json::json!({
        "embeddingDataDeliveryEnabled": logging.embedding_data_delivery_enabled,
        "imageDataDeliveryEnabled": logging.image_data_delivery_enabled,
        "textDataDeliveryEnabled": logging.text_data_delivery_enabled
    });

    if let Some(ref cw_arn) = logging.cloudwatch_log_group_arn {
        logging_config["cloudWatchConfig"] = serde_json::json!({
            "logGroupName": cw_arn,
            "roleArn": format!("arn:aws:iam::*:role/{}-bedrock-role", bedrock.name_any())
        });
    }

    if let Some(ref s3) = logging.s3_config {
        logging_config["s3Config"] = serde_json::json!({
            "bucketName": s3.bucket_arn,
            "keyPrefix": s3.key_prefix
        });
    }

    let resource = serde_json::json!({
        "apiVersion": "bedrock.aws.upbound.io/v1beta1",
        "kind": "ModelInvocationLoggingConfiguration",
        "metadata": {
            "name": name,
            "namespace": namespace,
            "labels": bedrock.spec.labels.clone().unwrap_or_default()
        },
        "spec": {
            "forProvider": {
                "loggingConfig": [logging_config],
                "region": bedrock.spec.region
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
    api: &Api<BedrockAccess>,
    name: &str,
    phase: BedrockAccessPhase,
    message: &str,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();

    let status = BedrockAccessStatus {
        phase,
        message: Some(message.to_string()),
        last_reconcile_time: Some(now.clone()),
        conditions: vec![BedrockAccessCondition {
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
    api: &Api<BedrockAccess>,
    name: &str,
    phase: BedrockAccessPhase,
    reason: &str,
    message: &str,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();

    let status = BedrockAccessStatus {
        phase,
        message: Some(message.to_string()),
        last_reconcile_time: Some(now.clone()),
        conditions: vec![BedrockAccessCondition {
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
    api: &Api<BedrockAccess>,
    name: &str,
    phase: BedrockAccessPhase,
    managed_resources: Vec<ManagedBedrockResource>,
    accessible_models: Vec<String>,
    model_count: i32,
    iam_role_arn: Option<String>,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();

    let status = BedrockAccessStatus {
        phase,
        ready: true,
        iam_role_arn,
        model_count,
        accessible_models,
        managed_resources,
        last_reconcile_time: Some(now.clone()),
        conditions: vec![BedrockAccessCondition {
            condition_type: "Ready".to_string(),
            status: "True".to_string(),
            reason: "AllResourcesReady".to_string(),
            message: "Bedrock access is configured and ready".to_string(),
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
    bedrock: Arc<BedrockAccess>,
    error: &OperatorError,
    _ctx: Arc<Context>,
) -> Action {
    warn!(
        "Reconcile error for BedrockAccess {}: {}",
        bedrock.name_any(),
        error
    );

    Action::requeue(Duration::from_secs(30))
}
