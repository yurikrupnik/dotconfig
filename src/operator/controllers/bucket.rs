use crate::operator::{Context, OperatorError, Result};
use crate::operator::types::{
    Bucket, BucketStatus, BucketPhase, BucketCondition, ManagedBucketResource,
    CloudProvider, BucketRole, StorageClass, BucketDeletionPolicy,
};
use chrono::Utc;
use kube::api::{Patch, PatchParams};
use kube::runtime::controller::Action;
use kube::{Api, ResourceExt};
use std::sync::Arc;
use std::time::Duration;
use tracing::{info, warn};

/// Reconciles Bucket resources - creates appropriate Crossplane managed resources
/// for the specified cloud provider (GCP, AWS, or Azure)
pub async fn reconcile(
    bucket: Arc<Bucket>,
    ctx: Arc<Context>,
) -> std::result::Result<Action, OperatorError> {
    let name = bucket.name_any();
    let namespace = bucket.namespace().unwrap_or_else(|| "default".to_string());

    info!("Reconciling Bucket {}/{} for provider {:?}", namespace, name, bucket.spec.provider);

    let api: Api<Bucket> = Api::namespaced(ctx.client.clone(), &namespace);

    // Update status to Creating
    update_phase(&api, &name, BucketPhase::Creating, "Creating bucket resources").await?;

    // Create provider-specific resources
    let managed_resource = match bucket.spec.provider {
        CloudProvider::Gcp => create_gcp_bucket(&bucket, &ctx, &namespace).await?,
        CloudProvider::Aws => create_aws_bucket(&bucket, &ctx, &namespace).await?,
        CloudProvider::Azure => create_azure_bucket(&bucket, &ctx, &namespace).await?,
    };

    // Create IAM bindings if specified
    if let Some(access_control) = &bucket.spec.access_control {
        create_iam_bindings(&bucket, &ctx, &namespace, &access_control.iam_bindings).await?;
    }

    // Check if bucket is ready
    let is_ready = check_bucket_ready(&ctx, &managed_resource).await?;

    let phase = if is_ready {
        BucketPhase::Ready
    } else {
        BucketPhase::Creating
    };

    // Update status
    update_status_full(&api, &name, phase, &managed_resource, is_ready).await?;

    if is_ready {
        info!("Bucket {}/{} is Ready", namespace, name);
        Ok(Action::requeue(Duration::from_secs(300)))
    } else {
        Ok(Action::requeue(Duration::from_secs(30)))
    }
}

// ============ GCP Bucket Creation ============

async fn create_gcp_bucket(
    bucket: &Bucket,
    ctx: &Context,
    namespace: &str,
) -> Result<ManagedBucketResource> {
    let name = bucket.name_any();
    let params = &bucket.spec.parameters;

    info!("Creating GCP Cloud Storage bucket: {}", name);

    // GCP Bucket resource (Crossplane provider-gcp-storage)
    let mut for_provider = serde_json::json!({
        "location": params.to_gcp_location(),
        "storageClass": params.to_gcp_storage_class(),
        "versioning": [{
            "enabled": params.versioning
        }],
        "publicAccessPrevention": if params.public_access_prevention {
            "enforced"
        } else {
            "inherited"
        },
        "uniformBucketLevelAccess": params.uniform_bucket_level_access,
        "forceDestroy": params.force_destroy,
        "requesterPays": params.requester_pays,
    });

    // Add GCP project if specified
    if let Some(project) = &params.gcp_project {
        for_provider["project"] = serde_json::json!(project);
    }

    // Add GCP autoclass if specified
    if let Some(autoclass) = &params.gcp_autoclass {
        for_provider["autoclass"] = serde_json::json!([{
            "enabled": autoclass.enabled,
            "terminalStorageClass": autoclass.terminal_storage_class
        }]);
    }

    // Add GCP RPO if specified
    if let Some(rpo) = &params.gcp_rpo {
        for_provider["rpo"] = serde_json::json!(rpo);
    }

    // Add a retention policy if specified
    if let Some(retention) = &params.retention_policy {
        for_provider["retentionPolicy"] = serde_json::json!([{
            "retentionPeriod": retention.retention_days * 86400, // Convert days to seconds
            "isLocked": retention.locked
        }]);
    }

    // Add a soft delete policy if specified
    if let Some(soft_delete) = &params.soft_delete {
        for_provider["softDeletePolicy"] = serde_json::json!([{
            "retentionDurationSeconds": soft_delete.retention_days * 86400
        }]);
    }

    // Add lifecycle rules
    let lifecycle_rules: Vec<serde_json::Value> = params.lifecycle_rules.iter().map(|rule| {
        let mut lr = serde_json::json!({});
        if let Some(transition) = &rule.transition {
            lr["action"] = serde_json::json!({
                "type": "SetStorageClass",
                "storageClass": match transition.storage_class {
                    StorageClass::Standard => "STANDARD",
                    StorageClass::InfrequentAccess => "NEARLINE",
                    StorageClass::Archive => "ARCHIVE",
                    StorageClass::ColdArchive => "COLDLINE",
                    StorageClass::Intelligent => "STANDARD",
                }
            });
            lr["condition"] = serde_json::json!({
                "age": transition.days
            });
        }
        if let Some(exp) = &rule.expiration {
            lr["action"] = serde_json::json!({"type": "Delete"});
            lr["condition"] = serde_json::json!({"age": exp.days});
        }
        lr
    }).collect();
    for_provider["lifecycleRule"] = serde_json::json!(lifecycle_rules);

    // Add CORS if specified
    if let Some(cors_rules) = &params.cors {
        for_provider["cors"] = serde_json::json!(cors_rules.iter().map(|c| serde_json::json!({
            "origin": c.allowed_origins,
            "method": c.allowed_methods,
            "responseHeader": c.allowed_headers,
            "maxAgeSeconds": c.max_age_seconds
        })).collect::<Vec<_>>());
    }

    // Add encryption if specified
    if let Some(enc) = &params.encryption {
        for_provider["encryption"] = serde_json::json!({
            "defaultKmsKeyName": enc.kms_key_id
        });
    }

    // Add logging if specified
    if let Some(log) = &params.logging {
        for_provider["logging"] = serde_json::json!({
            "logBucket": log.target_bucket,
            "logObjectPrefix": log.target_prefix
        });
    }

    // Add labels/tags
    if let Some(tags) = &params.tags {
        for_provider["labels"] = serde_json::json!(tags);
    }

    // Add website config if specified
    if let Some(w) = &params.website {
        for_provider["website"] = serde_json::json!({
            "mainPageSuffix": w.index_document,
            "notFoundPage": w.error_document
        });
    }

    // Build the full GCP Bucket resource
    let gcp_bucket = serde_json::json!({
        "apiVersion": "storage.gcp.upbound.io/v1beta1",
        "kind": "Bucket",
        "metadata": {
            "name": format!("{}-gcp", name),
            "namespace": namespace,
            "labels": bucket.spec.labels
        },
        "spec": {
            "forProvider": for_provider,
            "providerConfigRef": bucket.spec.provider_config_ref.as_ref().map(|p| {
                serde_json::json!({"name": p.name})
            }).unwrap_or(serde_json::json!({"name": "default"})),
            "deletionPolicy": match bucket.spec.deletion_policy {
                BucketDeletionPolicy::Delete => "Delete",
                BucketDeletionPolicy::Orphan => "Orphan",
            },
            "writeConnectionSecretToRef": bucket.spec.write_connection_secret_to_ref.as_ref().map(|s| {
                serde_json::json!({
                    "name": s.name,
                    "namespace": s.namespace.as_deref().unwrap_or(namespace)
                })
            })
        }
    });

    ctx.crossplane_client.apply_resource(&gcp_bucket).await?;

    Ok(ManagedBucketResource {
        api_version: "storage.gcp.upbound.io/v1beta1".to_string(),
        kind: "Bucket".to_string(),
        name: format!("{}-gcp", name),
    })
}

// ============ AWS S3 Bucket Creation ============

async fn create_aws_bucket(
    bucket: &Bucket,
    ctx: &Context,
    namespace: &str,
) -> Result<ManagedBucketResource> {
    let name = bucket.name_any();
    let params = &bucket.spec.parameters;

    info!("Creating AWS S3 bucket: {}", name);

    // AWS S3 Bucket (Crossplane provider-aws-s3)
    let aws_bucket = serde_json::json!({
        "apiVersion": "s3.aws.upbound.io/v1beta1",
        "kind": "Bucket",
        "metadata": {
            "name": format!("{}-aws", name),
            "namespace": namespace,
            "labels": bucket.spec.labels
        },
        "spec": {
            "forProvider": {
                "region": params.to_aws_region(),
                "tags": params.tags
            },
            "providerConfigRef": bucket.spec.provider_config_ref.as_ref().map(|p| {
                serde_json::json!({"name": p.name})
            }).unwrap_or(serde_json::json!({"name": "default"})),
            "deletionPolicy": match bucket.spec.deletion_policy {
                BucketDeletionPolicy::Delete => "Delete",
                BucketDeletionPolicy::Orphan => "Orphan",
            }
        }
    });

    ctx.crossplane_client.apply_resource(&aws_bucket).await?;

    // AWS S3 Bucket Versioning (separate resource)
    if params.versioning {
        let versioning = serde_json::json!({
            "apiVersion": "s3.aws.upbound.io/v1beta1",
            "kind": "BucketVersioning",
            "metadata": {
                "name": format!("{}-aws-versioning", name),
                "namespace": namespace
            },
            "spec": {
                "forProvider": {
                    "bucketRef": {
                        "name": format!("{}-aws", name)
                    },
                    "region": params.to_aws_region(),
                    "versioningConfiguration": [{
                        "status": "Enabled"
                    }]
                },
                "providerConfigRef": bucket.spec.provider_config_ref.as_ref().map(|p| {
                    serde_json::json!({"name": p.name})
                }).unwrap_or(serde_json::json!({"name": "default"}))
            }
        });
        ctx.crossplane_client.apply_resource(&versioning).await?;
    }

    // AWS S3 Bucket Public Access Block
    if params.public_access_prevention {
        let public_access_block = serde_json::json!({
            "apiVersion": "s3.aws.upbound.io/v1beta1",
            "kind": "BucketPublicAccessBlock",
            "metadata": {
                "name": format!("{}-aws-pab", name),
                "namespace": namespace
            },
            "spec": {
                "forProvider": {
                    "bucketRef": {
                        "name": format!("{}-aws", name)
                    },
                    "region": params.to_aws_region(),
                    "blockPublicAcls": true,
                    "blockPublicPolicy": true,
                    "ignorePublicAcls": true,
                    "restrictPublicBuckets": true
                },
                "providerConfigRef": bucket.spec.provider_config_ref.as_ref().map(|p| {
                    serde_json::json!({"name": p.name})
                }).unwrap_or(serde_json::json!({"name": "default"}))
            }
        });
        ctx.crossplane_client.apply_resource(&public_access_block).await?;
    }

    // AWS S3 Bucket Server-Side Encryption
    if params.encryption.is_some() {
        let encryption = &params.encryption.as_ref().unwrap();
        let sse = serde_json::json!({
            "apiVersion": "s3.aws.upbound.io/v1beta1",
            "kind": "BucketServerSideEncryptionConfiguration",
            "metadata": {
                "name": format!("{}-aws-sse", name),
                "namespace": namespace
            },
            "spec": {
                "forProvider": {
                    "bucketRef": {
                        "name": format!("{}-aws", name)
                    },
                    "region": params.to_aws_region(),
                    "rule": [{
                        "applyServerSideEncryptionByDefault": [{
                            "sseAlgorithm": if encryption.kms_key_id.is_some() { "aws:kms" } else { "AES256" },
                            "kmsMasterKeyId": encryption.kms_key_id
                        }],
                        "bucketKeyEnabled": true
                    }]
                },
                "providerConfigRef": bucket.spec.provider_config_ref.as_ref().map(|p| {
                    serde_json::json!({"name": p.name})
                }).unwrap_or(serde_json::json!({"name": "default"}))
            }
        });
        ctx.crossplane_client.apply_resource(&sse).await?;
    }

    // AWS S3 Bucket Lifecycle Configuration
    if !params.lifecycle_rules.is_empty() {
        let lifecycle = serde_json::json!({
            "apiVersion": "s3.aws.upbound.io/v1beta1",
            "kind": "BucketLifecycleConfiguration",
            "metadata": {
                "name": format!("{}-aws-lifecycle", name),
                "namespace": namespace
            },
            "spec": {
                "forProvider": {
                    "bucketRef": {
                        "name": format!("{}-aws", name)
                    },
                    "region": params.to_aws_region(),
                    "rule": params.lifecycle_rules.iter().enumerate().map(|(i, rule)| {
                        let mut r = serde_json::json!({
                            "id": rule.id.clone().unwrap_or(format!("rule-{}", i)),
                            "status": if rule.enabled { "Enabled" } else { "Disabled" }
                        });
                        if let Some(prefix) = &rule.prefix {
                            r["filter"] = serde_json::json!([{"prefix": prefix}]);
                        }
                        if let Some(transition) = &rule.transition {
                            r["transition"] = serde_json::json!([{
                                "days": transition.days,
                                "storageClass": match transition.storage_class {
                                    StorageClass::Standard => "STANDARD",
                                    StorageClass::InfrequentAccess => "STANDARD_IA",
                                    StorageClass::Archive => "GLACIER",
                                    StorageClass::ColdArchive => "DEEP_ARCHIVE",
                                    StorageClass::Intelligent => "INTELLIGENT_TIERING",
                                }
                            }]);
                        }
                        if let Some(exp) = &rule.expiration {
                            r["expiration"] = serde_json::json!([{
                                "days": exp.days,
                                "expiredObjectDeleteMarker": exp.expired_object_delete_marker
                            }]);
                        }
                        if let Some(nve) = rule.noncurrent_version_expiration {
                            r["noncurrentVersionExpiration"] = serde_json::json!([{
                                "noncurrentDays": nve
                            }]);
                        }
                        r
                    }).collect::<Vec<_>>()
                },
                "providerConfigRef": bucket.spec.provider_config_ref.as_ref().map(|p| {
                    serde_json::json!({"name": p.name})
                }).unwrap_or(serde_json::json!({"name": "default"}))
            }
        });
        ctx.crossplane_client.apply_resource(&lifecycle).await?;
    }

    // AWS S3 CORS Configuration
    if let Some(cors_rules) = &params.cors {
        let cors = serde_json::json!({
            "apiVersion": "s3.aws.upbound.io/v1beta1",
            "kind": "BucketCorsConfiguration",
            "metadata": {
                "name": format!("{}-aws-cors", name),
                "namespace": namespace
            },
            "spec": {
                "forProvider": {
                    "bucketRef": {
                        "name": format!("{}-aws", name)
                    },
                    "region": params.to_aws_region(),
                    "corsRule": cors_rules.iter().map(|c| serde_json::json!({
                        "allowedHeaders": c.allowed_headers,
                        "allowedMethods": c.allowed_methods,
                        "allowedOrigins": c.allowed_origins,
                        "exposeHeaders": c.expose_headers,
                        "maxAgeSeconds": c.max_age_seconds
                    })).collect::<Vec<_>>()
                },
                "providerConfigRef": bucket.spec.provider_config_ref.as_ref().map(|p| {
                    serde_json::json!({"name": p.name})
                }).unwrap_or(serde_json::json!({"name": "default"}))
            }
        });
        ctx.crossplane_client.apply_resource(&cors).await?;
    }

    Ok(ManagedBucketResource {
        api_version: "s3.aws.upbound.io/v1beta1".to_string(),
        kind: "Bucket".to_string(),
        name: format!("{}-aws", name),
    })
}

// ============ Azure Blob Storage Creation ============

async fn create_azure_bucket(
    bucket: &Bucket,
    ctx: &Context,
    namespace: &str,
) -> Result<ManagedBucketResource> {
    let name = bucket.name_any();
    let params = &bucket.spec.parameters;

    info!("Creating Azure Blob Storage container: {}", name);

    // Azure requires a Storage Account first, then a Container
    // Determine account tier and replication type
    let account_tier = params.azure_account_tier.as_ref()
        .map(|t| match t {
            crate::operator::types::bucket::AzureAccountTier::Standard => "Standard",
            crate::operator::types::bucket::AzureAccountTier::Premium => "Premium",
        })
        .unwrap_or("Standard");

    let replication_type = params.azure_replication_type.as_ref()
        .map(|r| match r {
            crate::operator::types::bucket::AzureReplicationType::Lrs => "LRS",
            crate::operator::types::bucket::AzureReplicationType::Grs => "GRS",
            crate::operator::types::bucket::AzureReplicationType::Ragrs => "RAGRS",
            crate::operator::types::bucket::AzureReplicationType::Zrs => "ZRS",
            crate::operator::types::bucket::AzureReplicationType::Gzrs => "GZRS",
            crate::operator::types::bucket::AzureReplicationType::Ragzrs => "RAGZRS",
        })
        .unwrap_or("LRS");

    let min_tls_version = params.azure_min_tls_version.as_deref().unwrap_or("TLS1_2");

    // Build blob properties with soft delete if specified
    let mut blob_properties = serde_json::json!({
        "versioningEnabled": params.versioning
    });
    if let Some(soft_delete) = &params.soft_delete {
        blob_properties["deleteRetentionPolicy"] = serde_json::json!([{
            "days": soft_delete.retention_days
        }]);
    } else {
        blob_properties["deleteRetentionPolicy"] = serde_json::json!([{
            "days": 7
        }]);
    }

    // Build forProvider for Storage Account
    let mut azure_for_provider = serde_json::json!({
        "location": params.to_azure_location(),
        "accountTier": account_tier,
        "accountReplicationType": replication_type,
        "accessTier": params.to_azure_access_tier(),
        "enableHttpsTrafficOnly": params.azure_https_only,
        "minTlsVersion": min_tls_version,
        "allowNestedItemsToBePublic": !params.public_access_prevention,
        "blobProperties": [blob_properties]
    });

    // Add resource group reference
    if let Some(rg) = &params.azure_resource_group {
        azure_for_provider["resourceGroupName"] = serde_json::json!(rg);
    } else {
        azure_for_provider["resourceGroupNameRef"] = serde_json::json!({
            "name": "default-rg"
        });
    }

    // Add tags if specified
    if let Some(tags) = &params.tags {
        azure_for_provider["tags"] = serde_json::json!(tags);
    }

    // Create Storage Account
    let storage_account = serde_json::json!({
        "apiVersion": "storage.azure.upbound.io/v1beta1",
        "kind": "Account",
        "metadata": {
            "name": format!("{}-azure-sa", name.replace("-", "")),
            "namespace": namespace,
            "labels": bucket.spec.labels
        },
        "spec": {
            "forProvider": azure_for_provider,
            "providerConfigRef": bucket.spec.provider_config_ref.as_ref().map(|p| {
                serde_json::json!({"name": p.name})
            }).unwrap_or(serde_json::json!({"name": "default"})),
            "deletionPolicy": match bucket.spec.deletion_policy {
                BucketDeletionPolicy::Delete => "Delete",
                BucketDeletionPolicy::Orphan => "Orphan",
            }
        }
    });

    ctx.crossplane_client.apply_resource(&storage_account).await?;

    // Create Blob Container
    let container = serde_json::json!({
        "apiVersion": "storage.azure.upbound.io/v1beta1",
        "kind": "Container",
        "metadata": {
            "name": format!("{}-azure-container", name),
            "namespace": namespace
        },
        "spec": {
            "forProvider": {
                "storageAccountNameRef": {
                    "name": format!("{}-azure-sa", name.replace("-", ""))
                },
                "containerAccessType": "private"
            },
            "providerConfigRef": bucket.spec.provider_config_ref.as_ref().map(|p| {
                serde_json::json!({"name": p.name})
            }).unwrap_or(serde_json::json!({"name": "default"}))
        }
    });

    ctx.crossplane_client.apply_resource(&container).await?;

    Ok(ManagedBucketResource {
        api_version: "storage.azure.upbound.io/v1beta1".to_string(),
        kind: "Container".to_string(),
        name: format!("{}-azure-container", name),
    })
}

// ============ IAM Bindings ============

async fn create_iam_bindings(
    bucket: &Bucket,
    ctx: &Context,
    namespace: &str,
    bindings: &[crate::operator::types::IamBinding],
) -> Result<()> {
    let name = bucket.name_any();

    for (i, binding) in bindings.iter().enumerate() {
        match bucket.spec.provider {
            CloudProvider::Gcp => {
                // GCP BucketIAMMember
                let role = match binding.role {
                    BucketRole::ObjectViewer => "roles/storage.objectViewer",
                    BucketRole::ObjectAdmin => "roles/storage.objectAdmin",
                    BucketRole::BucketAdmin => "roles/storage.admin",
                    BucketRole::ObjectCreator => "roles/storage.objectCreator",
                    BucketRole::Custom => "roles/storage.objectViewer", // Should use raw role
                };

                for (j, member) in binding.members.iter().enumerate() {
                    let iam_member = serde_json::json!({
                        "apiVersion": "storage.gcp.upbound.io/v1beta1",
                        "kind": "BucketIAMMember",
                        "metadata": {
                            "name": format!("{}-iam-{}-{}", name, i, j),
                            "namespace": namespace
                        },
                        "spec": {
                            "forProvider": {
                                "bucketRef": {
                                    "name": format!("{}-gcp", name)
                                },
                                "role": role,
                                "member": member,
                                "condition": binding.condition.as_ref().map(|c| serde_json::json!({
                                    "title": c.title,
                                    "description": c.description,
                                    "expression": c.expression
                                }))
                            },
                            "providerConfigRef": bucket.spec.provider_config_ref.as_ref().map(|p| {
                                serde_json::json!({"name": p.name})
                            }).unwrap_or(serde_json::json!({"name": "default"}))
                        }
                    });
                    ctx.crossplane_client.apply_resource(&iam_member).await?;
                }
            }
            CloudProvider::Aws => {
                // AWS S3 BucketPolicy
                let policy_doc = create_aws_bucket_policy(&name, bindings);
                let bucket_policy = serde_json::json!({
                    "apiVersion": "s3.aws.upbound.io/v1beta1",
                    "kind": "BucketPolicy",
                    "metadata": {
                        "name": format!("{}-aws-policy", name),
                        "namespace": namespace
                    },
                    "spec": {
                        "forProvider": {
                            "bucketRef": {
                                "name": format!("{}-aws", name)
                            },
                            "region": bucket.spec.parameters.to_aws_region(),
                            "policy": serde_json::to_string(&policy_doc).unwrap()
                        },
                        "providerConfigRef": bucket.spec.provider_config_ref.as_ref().map(|p| {
                            serde_json::json!({"name": p.name})
                        }).unwrap_or(serde_json::json!({"name": "default"}))
                    }
                });
                ctx.crossplane_client.apply_resource(&bucket_policy).await?;
                break; // Only create one policy for all bindings
            }
            CloudProvider::Azure => {
                // Azure Role Assignment
                let role_def_id = match binding.role {
                    BucketRole::ObjectViewer => "2a2b9908-6ea1-4ae2-8e65-a410df84e7d1", // Storage Blob Data Reader
                    BucketRole::ObjectAdmin => "ba92f5b4-2d11-453d-a403-e96b0029c9fe", // Storage Blob Data Contributor
                    BucketRole::BucketAdmin => "b7e6dc6d-f1e8-4753-8033-0f276bb0955b", // Storage Blob Data Owner
                    BucketRole::ObjectCreator => "ba92f5b4-2d11-453d-a403-e96b0029c9fe", // Contributor
                    BucketRole::Custom => "2a2b9908-6ea1-4ae2-8e65-a410df84e7d1",
                };

                for (j, member) in binding.members.iter().enumerate() {
                    let role_assignment = serde_json::json!({
                        "apiVersion": "authorization.azure.upbound.io/v1beta1",
                        "kind": "RoleAssignment",
                        "metadata": {
                            "name": format!("{}-role-{}-{}", name, i, j),
                            "namespace": namespace
                        },
                        "spec": {
                            "forProvider": {
                                "principalId": member,
                                "roleDefinitionId": format!("/providers/Microsoft.Authorization/roleDefinitions/{}", role_def_id),
                                "scope": format!("/subscriptions/SUB_ID/resourceGroups/RG/providers/Microsoft.Storage/storageAccounts/{}", name.replace("-", ""))
                            },
                            "providerConfigRef": bucket.spec.provider_config_ref.as_ref().map(|p| {
                                serde_json::json!({"name": p.name})
                            }).unwrap_or(serde_json::json!({"name": "default"}))
                        }
                    });
                    ctx.crossplane_client.apply_resource(&role_assignment).await?;
                }
            }
        }
    }

    Ok(())
}

fn create_aws_bucket_policy(
    bucket_name: &str,
    bindings: &[crate::operator::types::IamBinding],
) -> serde_json::Value {
    let statements: Vec<serde_json::Value> = bindings.iter().enumerate().map(|(i, binding)| {
        let actions = match binding.role {
            BucketRole::ObjectViewer => vec!["s3:GetObject", "s3:ListBucket"],
            BucketRole::ObjectAdmin => vec!["s3:GetObject", "s3:PutObject", "s3:DeleteObject", "s3:ListBucket"],
            BucketRole::BucketAdmin => vec!["s3:*"],
            BucketRole::ObjectCreator => vec!["s3:PutObject"],
            BucketRole::Custom => vec!["s3:GetObject"],
        };

        serde_json::json!({
            "Sid": format!("Statement{}", i),
            "Effect": "Allow",
            "Principal": {
                "AWS": binding.members
            },
            "Action": actions,
            "Resource": [
                format!("arn:aws:s3:::{}-aws", bucket_name),
                format!("arn:aws:s3:::{}-aws/*", bucket_name)
            ]
        })
    }).collect();

    serde_json::json!({
        "Version": "2012-10-17",
        "Statement": statements
    })
}

// ============ Status Helpers ============

async fn check_bucket_ready(
    ctx: &Context,
    managed_resource: &ManagedBucketResource,
) -> Result<bool> {
    // Query the managed resource status using get_xr_status
    match ctx.crossplane_client.get_xr_status(
        &managed_resource.api_version,
        &managed_resource.kind,
        &managed_resource.name,
    ).await {
        Ok(Some(status)) => Ok(status.ready && status.synced),
        Ok(None) => Ok(false), // Resource not found yet
        Err(_) => Ok(false),
    }
}

async fn update_phase(
    api: &Api<Bucket>,
    name: &str,
    phase: BucketPhase,
    message: &str,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();

    let status = BucketStatus {
        phase,
        last_reconcile_time: Some(now.clone()),
        message: Some(message.to_string()),
        conditions: vec![BucketCondition {
            condition_type: "Reconciling".to_string(),
            status: "True".to_string(),
            reason: "Reconciling".to_string(),
            message: message.to_string(),
            last_transition_time: now,
        }],
        ..Default::default()
    };

    let patch = serde_json::json!({"status": status});

    api.patch_status(name, &PatchParams::apply("platform-operator"), &Patch::Merge(&patch))
        .await?;

    Ok(())
}

async fn update_status_full(
    api: &Api<Bucket>,
    name: &str,
    phase: BucketPhase,
    managed_resource: &ManagedBucketResource,
    is_ready: bool,
) -> Result<()> {
    let now = Utc::now().to_rfc3339();

    let status = BucketStatus {
        phase,
        ready: is_ready,
        synced: is_ready,
        managed_resource: Some(managed_resource.clone()),
        last_reconcile_time: Some(now.clone()),
        conditions: vec![BucketCondition {
            condition_type: if is_ready { "Ready" } else { "Synced" }.to_string(),
            status: if is_ready { "True" } else { "False" }.to_string(),
            reason: if is_ready { "Available" } else { "Progressing" }.to_string(),
            message: if is_ready {
                "Bucket is ready".to_string()
            } else {
                "Waiting for bucket to be ready".to_string()
            },
            last_transition_time: now,
        }],
        ..Default::default()
    };

    let patch = serde_json::json!({"status": status});

    api.patch_status(name, &PatchParams::apply("platform-operator"), &Patch::Merge(&patch))
        .await?;

    Ok(())
}

/// Error policy for the controller
pub fn error_policy(
    bucket: Arc<Bucket>,
    error: &OperatorError,
    _ctx: Arc<Context>,
) -> Action {
    warn!(
        "Reconcile error for Bucket {}: {}",
        bucket.name_any(),
        error
    );

    Action::requeue(Duration::from_secs(30))
}
