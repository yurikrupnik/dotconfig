//! EmailService Controller
//!
//! Manages AWS SES resources via Crossplane upbound/provider-aws-ses.
//! Creates production-ready email sending infrastructure with:
//! - Domain verification with DKIM
//! - MAIL FROM domain for SPF alignment
//! - Configuration sets for tracking
//! - IAM roles via IRSA
//! - Bounce/complaint notification handling

use crate::operator::dependencies::known_dependencies;
use crate::operator::types::{
    DnsRecord, DnsRecordPurpose, EmailEnvironment, EmailService, EmailServiceCondition,
    EmailServicePhase, EmailServiceStatus, EventDestinationType, ManagedSesResource,
    SecurityLevel, SecurityStatus, SecurityWarning, TlsPolicy,
};
use crate::operator::{Context, OperatorError, Result};
use chrono::Utc;
use kube::api::{Patch, PatchParams};
use kube::runtime::controller::Action;
use kube::{Api, ResourceExt};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

/// Reconciles EmailService resources
pub async fn reconcile(
    email: Arc<EmailService>,
    ctx: Arc<Context>,
) -> std::result::Result<Action, OperatorError> {
    let name = email.name_any();
    let namespace = email.namespace().unwrap_or_else(|| "default".to_string());

    info!("Reconciling EmailService {}/{}", namespace, name);

    let api: Api<EmailService> = Api::namespaced(ctx.client.clone(), &namespace);

    // Check dependencies before proceeding
    let deps = known_dependencies::email_service_deps();
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

        warn!("EmailService {}/{}: {}", namespace, name, message);

        update_phase_with_condition(
            &api,
            &name,
            EmailServicePhase::Blocked,
            "DependencyMissing",
            &message,
        )
        .await?;

        return Ok(Action::requeue(Duration::from_secs(60)));
    }

    // Validate spec for security requirements
    if let Err(errors) = email.spec.validate() {
        let message = format!("Security validation failed: {}", errors.join("; "));
        warn!("EmailService {}/{}: {}", namespace, name, message);

        update_phase_with_condition(
            &api,
            &name,
            EmailServicePhase::Failed,
            "ValidationFailed",
            &message,
        )
        .await?;

        return Ok(Action::await_change());
    }

    // Update status to Creating
    update_phase(&api, &name, EmailServicePhase::Creating, "Creating SES resources").await?;

    // Create SES resources via Crossplane
    let mut managed_resources = Vec::new();
    let mut dns_records = Vec::new();
    let mut security_warnings = Vec::new();

    // 1. Create Domain Identity
    let domain_identity_name = create_domain_identity(&email, &ctx, &namespace).await?;
    managed_resources.push(ManagedSesResource {
        api_version: "ses.aws.upbound.io/v1beta1".to_string(),
        kind: "DomainIdentity".to_string(),
        name: domain_identity_name.clone(),
        ready: false,
        synced: false,
        message: Some("Creating domain identity".to_string()),
    });

    // Add domain verification DNS record
    dns_records.push(DnsRecord {
        record_type: "TXT".to_string(),
        name: format!("_amazonses.{}", email.spec.domain),
        value: "<verification-token>".to_string(), // Will be populated from Crossplane status
        purpose: DnsRecordPurpose::DomainVerification,
        verified: false,
    });

    // 2. Create Domain DKIM
    let dkim_name = create_domain_dkim(&email, &ctx, &namespace).await?;
    managed_resources.push(ManagedSesResource {
        api_version: "ses.aws.upbound.io/v1beta1".to_string(),
        kind: "DomainDKIM".to_string(),
        name: dkim_name,
        ready: false,
        synced: false,
        message: Some("Creating DKIM signing".to_string()),
    });

    // Add DKIM DNS records (3 CNAME records)
    for i in 1..=3 {
        dns_records.push(DnsRecord {
            record_type: "CNAME".to_string(),
            name: format!("<dkim-token-{}>.{}", i, email.spec.domain),
            value: format!("<dkim-token-{}>.dkim.amazonses.com", i),
            purpose: DnsRecordPurpose::Dkim,
            verified: false,
        });
    }

    // 3. Create MAIL FROM domain if configured
    if let Some(ref mail_from) = email.spec.mail_from {
        let mail_from_name = create_mail_from_domain(&email, &ctx, &namespace, mail_from).await?;
        managed_resources.push(ManagedSesResource {
            api_version: "ses.aws.upbound.io/v1beta1".to_string(),
            kind: "DomainMailFrom".to_string(),
            name: mail_from_name,
            ready: false,
            synced: false,
            message: Some("Creating MAIL FROM domain".to_string()),
        });

        // Add MAIL FROM DNS records
        let mail_from_domain = format!("{}.{}", mail_from.subdomain, email.spec.domain);
        dns_records.push(DnsRecord {
            record_type: "MX".to_string(),
            name: mail_from_domain.clone(),
            value: format!("10 feedback-smtp.{}.amazonses.com", email.spec.region),
            purpose: DnsRecordPurpose::MailFrom,
            verified: false,
        });
        dns_records.push(DnsRecord {
            record_type: "TXT".to_string(),
            name: mail_from_domain,
            value: "v=spf1 include:amazonses.com ~all".to_string(),
            purpose: DnsRecordPurpose::Spf,
            verified: false,
        });
    }

    // 4. Create Configuration Set
    let config_set_name = create_configuration_set(&email, &ctx, &namespace).await?;
    managed_resources.push(ManagedSesResource {
        api_version: "ses.aws.upbound.io/v1beta2".to_string(),
        kind: "ConfigurationSet".to_string(),
        name: config_set_name.clone(),
        ready: false,
        synced: false,
        message: Some("Creating configuration set".to_string()),
    });

    // 5. Create Event Destinations
    for event_dest in &email.spec.event_destinations {
        let dest_name =
            create_event_destination(&email, &ctx, &namespace, event_dest, &config_set_name)
                .await?;
        managed_resources.push(ManagedSesResource {
            api_version: "ses.aws.upbound.io/v1beta1".to_string(),
            kind: "EventDestination".to_string(),
            name: dest_name,
            ready: false,
            synced: false,
            message: Some(format!("Creating event destination: {}", event_dest.name)),
        });
    }

    // 6. Create Notification Topics if configured
    if let Some(ref notifications) = email.spec.notifications {
        // Create bounce notification topic
        if notifications.bounce_topic.create_topic_name.is_some() {
            let topic_name =
                create_notification_topic(&email, &ctx, &namespace, "bounce").await?;
            managed_resources.push(ManagedSesResource {
                api_version: "sns.aws.upbound.io/v1beta1".to_string(),
                kind: "Topic".to_string(),
                name: topic_name,
                ready: false,
                synced: false,
                message: Some("Creating bounce notification topic".to_string()),
            });
        }

        // Create complaint notification topic
        if notifications.complaint_topic.create_topic_name.is_some() {
            let topic_name =
                create_notification_topic(&email, &ctx, &namespace, "complaint").await?;
            managed_resources.push(ManagedSesResource {
                api_version: "sns.aws.upbound.io/v1beta1".to_string(),
                kind: "Topic".to_string(),
                name: topic_name,
                ready: false,
                synced: false,
                message: Some("Creating complaint notification topic".to_string()),
            });
        }
    }

    // 7. Create IAM Role for IRSA
    let iam_role_name = create_iam_role(&email, &ctx, &namespace).await?;
    managed_resources.push(ManagedSesResource {
        api_version: "iam.aws.upbound.io/v1beta1".to_string(),
        kind: "Role".to_string(),
        name: iam_role_name.clone(),
        ready: false,
        synced: false,
        message: Some("Creating IAM role for IRSA".to_string()),
    });

    // 8. Create IAM Policy
    let iam_policy_name = create_iam_policy(&email, &ctx, &namespace).await?;
    managed_resources.push(ManagedSesResource {
        api_version: "iam.aws.upbound.io/v1beta1".to_string(),
        kind: "Policy".to_string(),
        name: iam_policy_name,
        ready: false,
        synced: false,
        message: Some("Creating IAM policy".to_string()),
    });

    // 9. Create Email Templates
    for template in &email.spec.templates {
        let template_name = create_email_template(&email, &ctx, &namespace, template).await?;
        managed_resources.push(ManagedSesResource {
            api_version: "ses.aws.upbound.io/v1beta1".to_string(),
            kind: "Template".to_string(),
            name: template_name,
            ready: false,
            synced: false,
            message: Some(format!("Creating template: {}", template.name)),
        });
    }

    // Add security warnings for non-prod environments
    if email.spec.environment == EmailEnvironment::Dev {
        security_warnings.push(SecurityWarning {
            severity: SecurityLevel::Warning,
            code: "DEV_MODE".to_string(),
            message: "Running in development mode - relaxed security settings".to_string(),
            remediation: "Set environment to 'prod' for production use".to_string(),
            first_seen: Some(Utc::now().to_rfc3339()),
        });
    }

    // Warn if TLS is optional (shouldn't happen due to validation, but just in case)
    if email.spec.configuration_set.tls_policy == TlsPolicy::Optional {
        security_warnings.push(SecurityWarning {
            severity: SecurityLevel::Critical,
            code: "TLS_OPTIONAL".to_string(),
            message: "TLS is not required - emails may be sent without encryption".to_string(),
            remediation: "Set tlsPolicy to REQUIRE in configuration set".to_string(),
            first_seen: Some(Utc::now().to_rfc3339()),
        });
    }

    // Add DMARC recommendation
    dns_records.push(DnsRecord {
        record_type: "TXT".to_string(),
        name: format!("_dmarc.{}", email.spec.domain),
        value: format!(
            "v=DMARC1; p=quarantine; rua=mailto:dmarc@{}",
            email.spec.domain
        ),
        purpose: DnsRecordPurpose::Dmarc,
        verified: false,
    });

    // Update status
    let security_status = SecurityStatus {
        overall: if security_warnings.is_empty() {
            SecurityLevel::Healthy
        } else if security_warnings
            .iter()
            .any(|w| w.severity == SecurityLevel::Critical)
        {
            SecurityLevel::Critical
        } else {
            SecurityLevel::Warning
        },
        last_checked: Some(Utc::now().to_rfc3339()),
        dmarc: None,
        spf: None,
        warnings: security_warnings,
    };

    update_status_full(
        &api,
        &name,
        EmailServicePhase::AwaitingVerification,
        managed_resources,
        dns_records,
        security_status,
        Some(config_set_name),
    )
    .await?;

    info!(
        "EmailService {}/{} created - awaiting DNS verification",
        namespace, name
    );

    // Requeue to check verification status
    Ok(Action::requeue(Duration::from_secs(60)))
}

async fn create_domain_identity(
    email: &EmailService,
    ctx: &Context,
    namespace: &str,
) -> Result<String> {
    let name = format!("{}-domain", email.name_any());

    info!(
        "Creating DomainIdentity {} for domain {}",
        name, email.spec.domain
    );

    let provider_config = email
        .spec
        .provider_config_ref
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "default".to_string());

    let resource = serde_json::json!({
        "apiVersion": "ses.aws.upbound.io/v1beta1",
        "kind": "DomainIdentity",
        "metadata": {
            "name": name,
            "namespace": namespace,
            "labels": email.spec.labels.clone().unwrap_or_default()
        },
        "spec": {
            "forProvider": {
                "domain": email.spec.domain,
                "region": email.spec.region
            },
            "providerConfigRef": {
                "name": provider_config
            },
            "deletionPolicy": match email.spec.deletion_policy {
                crate::operator::types::crossplane_resource::DeletionPolicy::Delete => "Delete",
                crate::operator::types::crossplane_resource::DeletionPolicy::Orphan => "Orphan",
            }
        }
    });

    ctx.crossplane_client.apply_resource(&resource).await?;

    Ok(name)
}

async fn create_domain_dkim(
    email: &EmailService,
    ctx: &Context,
    namespace: &str,
) -> Result<String> {
    let name = format!("{}-dkim", email.name_any());
    let domain_identity_name = format!("{}-domain", email.name_any());

    info!("Creating DomainDKIM {} for {}", name, email.spec.domain);

    let provider_config = email
        .spec
        .provider_config_ref
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "default".to_string());

    let resource = serde_json::json!({
        "apiVersion": "ses.aws.upbound.io/v1beta1",
        "kind": "DomainDKIM",
        "metadata": {
            "name": name,
            "namespace": namespace,
            "labels": email.spec.labels.clone().unwrap_or_default()
        },
        "spec": {
            "forProvider": {
                "domainSelector": {
                    "matchLabels": {
                        "crossplane.io/claim-name": domain_identity_name
                    }
                },
                "region": email.spec.region
            },
            "providerConfigRef": {
                "name": provider_config
            }
        }
    });

    ctx.crossplane_client.apply_resource(&resource).await?;

    Ok(name)
}

async fn create_mail_from_domain(
    email: &EmailService,
    ctx: &Context,
    namespace: &str,
    mail_from: &crate::operator::types::MailFromConfig,
) -> Result<String> {
    let name = format!("{}-mailfrom", email.name_any());
    let mail_from_domain = format!("{}.{}", mail_from.subdomain, email.spec.domain);

    info!("Creating DomainMailFrom {} -> {}", name, mail_from_domain);

    let provider_config = email
        .spec
        .provider_config_ref
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "default".to_string());

    let behavior = match mail_from.behavior_on_mx_failure {
        crate::operator::types::MxFailureBehavior::RejectMessage => "RejectMessage",
        crate::operator::types::MxFailureBehavior::UseDefaultValue => "UseDefaultValue",
    };

    let resource = serde_json::json!({
        "apiVersion": "ses.aws.upbound.io/v1beta1",
        "kind": "DomainMailFrom",
        "metadata": {
            "name": name,
            "namespace": namespace,
            "labels": email.spec.labels.clone().unwrap_or_default()
        },
        "spec": {
            "forProvider": {
                "domain": email.spec.domain,
                "mailFromDomain": mail_from_domain,
                "behaviorOnMxFailure": behavior,
                "region": email.spec.region
            },
            "providerConfigRef": {
                "name": provider_config
            }
        }
    });

    ctx.crossplane_client.apply_resource(&resource).await?;

    Ok(name)
}

async fn create_configuration_set(
    email: &EmailService,
    ctx: &Context,
    namespace: &str,
) -> Result<String> {
    let name = email
        .spec
        .configuration_set
        .name
        .clone()
        .unwrap_or_else(|| format!("{}-config", email.name_any()));

    info!("Creating ConfigurationSet {}", name);

    let provider_config = email
        .spec
        .provider_config_ref
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "default".to_string());

    let tls_policy = match email.spec.configuration_set.tls_policy {
        TlsPolicy::Require => "REQUIRE",
        TlsPolicy::Optional => "OPTIONAL",
    };

    let resource = serde_json::json!({
        "apiVersion": "ses.aws.upbound.io/v1beta2",
        "kind": "ConfigurationSet",
        "metadata": {
            "name": name,
            "namespace": namespace,
            "labels": email.spec.labels.clone().unwrap_or_default()
        },
        "spec": {
            "forProvider": {
                "deliveryOptions": {
                    "tlsPolicy": tls_policy
                },
                "reputationMetricsEnabled": email.spec.configuration_set.reputation_metrics_enabled,
                "sendingEnabled": email.spec.configuration_set.sending_enabled,
                "region": email.spec.region
            },
            "providerConfigRef": {
                "name": provider_config
            }
        }
    });

    ctx.crossplane_client.apply_resource(&resource).await?;

    Ok(name)
}

async fn create_event_destination(
    email: &EmailService,
    ctx: &Context,
    namespace: &str,
    event_dest: &crate::operator::types::EventDestinationSpec,
    config_set_name: &str,
) -> Result<String> {
    let name = format!("{}-{}", email.name_any(), event_dest.name);

    info!("Creating EventDestination {}", name);

    let provider_config = email
        .spec
        .provider_config_ref
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "default".to_string());

    let matching_types: Vec<&str> = event_dest
        .matching_event_types
        .iter()
        .map(|t| match t {
            crate::operator::types::SesEventType::Send => "send",
            crate::operator::types::SesEventType::Reject => "reject",
            crate::operator::types::SesEventType::Bounce => "bounce",
            crate::operator::types::SesEventType::Complaint => "complaint",
            crate::operator::types::SesEventType::Delivery => "delivery",
            crate::operator::types::SesEventType::Open => "open",
            crate::operator::types::SesEventType::Click => "click",
            crate::operator::types::SesEventType::RenderingFailure => "renderingFailure",
            crate::operator::types::SesEventType::DeliveryDelay => "deliveryDelay",
        })
        .collect();

    let destination = match &event_dest.destination {
        EventDestinationType::CloudWatch(cw) => {
            let dimensions: Vec<serde_json::Value> = cw
                .dimension_configurations
                .iter()
                .map(|d| {
                    serde_json::json!({
                        "dimensionName": d.dimension_name,
                        "dimensionValueSource": match d.dimension_value_source {
                            crate::operator::types::DimensionValueSource::MessageTag => "messageTag",
                            crate::operator::types::DimensionValueSource::EmailHeader => "emailHeader",
                            crate::operator::types::DimensionValueSource::LinkTag => "linkTag",
                        },
                        "defaultDimensionValue": d.default_dimension_value
                    })
                })
                .collect();
            serde_json::json!({
                "cloudwatchDestination": {
                    "dimensionConfiguration": dimensions
                }
            })
        }
        EventDestinationType::Sns(sns) => {
            serde_json::json!({
                "snsDestination": {
                    "topicArn": sns.topic
                }
            })
        }
        EventDestinationType::KinesisFirehose(kf) => {
            serde_json::json!({
                "kinesisDestination": {
                    "streamArn": kf.delivery_stream_arn,
                    "roleArn": kf.iam_role_arn
                }
            })
        }
    };

    let mut for_provider = serde_json::json!({
        "configurationSetName": config_set_name,
        "enabled": event_dest.enabled,
        "matchingTypes": matching_types,
        "region": email.spec.region
    });

    // Merge destination config
    if let serde_json::Value::Object(ref mut map) = for_provider {
        if let serde_json::Value::Object(dest_map) = destination {
            for (k, v) in dest_map {
                map.insert(k, v);
            }
        }
    }

    let resource = serde_json::json!({
        "apiVersion": "ses.aws.upbound.io/v1beta1",
        "kind": "EventDestination",
        "metadata": {
            "name": name,
            "namespace": namespace,
            "labels": email.spec.labels.clone().unwrap_or_default()
        },
        "spec": {
            "forProvider": for_provider,
            "providerConfigRef": {
                "name": provider_config
            }
        }
    });

    ctx.crossplane_client.apply_resource(&resource).await?;

    Ok(name)
}

async fn create_notification_topic(
    email: &EmailService,
    ctx: &Context,
    namespace: &str,
    topic_type: &str,
) -> Result<String> {
    let name = format!("{}-{}-notifications", email.name_any(), topic_type);

    info!("Creating SNS Topic {} for {} notifications", name, topic_type);

    let provider_config = email
        .spec
        .provider_config_ref
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "default".to_string());

    let resource = serde_json::json!({
        "apiVersion": "sns.aws.upbound.io/v1beta1",
        "kind": "Topic",
        "metadata": {
            "name": name,
            "namespace": namespace,
            "labels": email.spec.labels.clone().unwrap_or_default()
        },
        "spec": {
            "forProvider": {
                "name": format!("{}-{}-{}", email.spec.domain.replace('.', "-"), email.name_any(), topic_type),
                "region": email.spec.region
            },
            "providerConfigRef": {
                "name": provider_config
            }
        }
    });

    ctx.crossplane_client.apply_resource(&resource).await?;

    Ok(name)
}

async fn create_iam_role(email: &EmailService, ctx: &Context, namespace: &str) -> Result<String> {
    let name = format!("{}-role", email.name_any());

    info!(
        "Creating IAM Role {} for service account {}",
        name, email.spec.iam.service_account_name
    );

    let provider_config = email
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
                "Federated": "arn:aws:iam::${AWS_ACCOUNT_ID}:oidc-provider/${OIDC_PROVIDER}"
            },
            "Action": "sts:AssumeRoleWithWebIdentity",
            "Condition": {
                "StringEquals": {
                    "${OIDC_PROVIDER}:sub": format!(
                        "system:serviceaccount:{}:{}",
                        email.spec.iam.service_account_namespace,
                        email.spec.iam.service_account_name
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
            "labels": email.spec.labels.clone().unwrap_or_default()
        },
        "spec": {
            "forProvider": {
                "assumeRolePolicy": trust_policy.to_string(),
                "description": format!("IAM role for EmailService {} via IRSA", email.name_any()),
                "tags": {
                    "ManagedBy": "platform-operator",
                    "EmailService": email.name_any()
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

async fn create_iam_policy(
    email: &EmailService,
    ctx: &Context,
    namespace: &str,
) -> Result<String> {
    let name = format!("{}-policy", email.name_any());

    info!("Creating IAM Policy {} with SES permissions", name);

    let provider_config = email
        .spec
        .provider_config_ref
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "default".to_string());

    // Build permissions based on spec
    let perms = &email.spec.iam.permissions;
    let mut actions = Vec::new();

    if perms.allow_send_email {
        actions.push("ses:SendEmail");
    }
    if perms.allow_templated_email {
        actions.push("ses:SendTemplatedEmail");
    }
    if perms.allow_raw_email {
        actions.push("ses:SendRawEmail");
    }
    if perms.allow_bulk_email {
        actions.push("ses:SendBulkTemplatedEmail");
    }

    // Build resource restrictions
    let resource = if perms.restrict_to_identity {
        format!("arn:aws:ses:{}:*:identity/{}", email.spec.region, email.spec.domain)
    } else {
        "*".to_string()
    };

    let mut statements = vec![serde_json::json!({
        "Effect": "Allow",
        "Action": actions,
        "Resource": resource
    })];

    // Add configuration set restriction if required
    if perms.require_configuration_set {
        let config_set_name = email
            .spec
            .configuration_set
            .name
            .clone()
            .unwrap_or_else(|| format!("{}-config", email.name_any()));

        statements.push(serde_json::json!({
            "Effect": "Allow",
            "Action": "ses:SendEmail",
            "Resource": "*",
            "Condition": {
                "StringEquals": {
                    "ses:ConfigurationSetName": config_set_name
                }
            }
        }));
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
            "labels": email.spec.labels.clone().unwrap_or_default()
        },
        "spec": {
            "forProvider": {
                "policy": policy_doc.to_string(),
                "description": format!("SES sending policy for EmailService {}", email.name_any()),
                "tags": {
                    "ManagedBy": "platform-operator",
                    "EmailService": email.name_any()
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

async fn create_email_template(
    email: &EmailService,
    ctx: &Context,
    namespace: &str,
    template: &crate::operator::types::EmailTemplateSpec,
) -> Result<String> {
    let name = format!("{}-template-{}", email.name_any(), template.name);

    info!("Creating Email Template {}", name);

    let provider_config = email
        .spec
        .provider_config_ref
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_else(|| "default".to_string());

    let resource = serde_json::json!({
        "apiVersion": "ses.aws.upbound.io/v1beta1",
        "kind": "Template",
        "metadata": {
            "name": name,
            "namespace": namespace,
            "labels": email.spec.labels.clone().unwrap_or_default()
        },
        "spec": {
            "forProvider": {
                "name": template.name,
                "subject": template.subject,
                "html": template.html_body,
                "text": template.text_body,
                "region": email.spec.region
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
    api: &Api<EmailService>,
    name: &str,
    phase: EmailServicePhase,
    message: &str,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();

    let status = EmailServiceStatus {
        phase,
        message: Some(message.to_string()),
        last_reconcile_time: Some(now.clone()),
        conditions: vec![EmailServiceCondition {
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
    api: &Api<EmailService>,
    name: &str,
    phase: EmailServicePhase,
    reason: &str,
    message: &str,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();

    let status = EmailServiceStatus {
        phase,
        message: Some(message.to_string()),
        last_reconcile_time: Some(now.clone()),
        conditions: vec![EmailServiceCondition {
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
    api: &Api<EmailService>,
    name: &str,
    phase: EmailServicePhase,
    managed_resources: Vec<ManagedSesResource>,
    dns_records: Vec<DnsRecord>,
    security_status: SecurityStatus,
    configuration_set_name: Option<String>,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();

    let all_ready = managed_resources.iter().all(|r| r.ready);

    let status = EmailServiceStatus {
        phase,
        managed_resources,
        dns_records,
        security_status,
        configuration_set_name,
        last_reconcile_time: Some(now.clone()),
        conditions: vec![EmailServiceCondition {
            condition_type: if all_ready { "Ready" } else { "Progressing" }.to_string(),
            status: if all_ready { "True" } else { "False" }.to_string(),
            reason: if all_ready {
                "AllResourcesReady"
            } else {
                "CreatingResources"
            }
            .to_string(),
            message: if all_ready {
                "All SES resources are ready and verified".to_string()
            } else {
                "Creating SES resources - DNS verification pending".to_string()
            },
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
    email: Arc<EmailService>,
    error: &OperatorError,
    _ctx: Arc<Context>,
) -> Action {
    warn!(
        "Reconcile error for EmailService {}: {}",
        email.name_any(),
        error
    );

    Action::requeue(Duration::from_secs(30))
}
