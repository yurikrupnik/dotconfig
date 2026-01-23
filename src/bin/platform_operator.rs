//! Platform Operator Binary
//!
//! Kubernetes operator for managing:
//! - PlatformApp: Install applications via Helm and/or KCL manifests
//! - GitOpsApp: Manage FluxCD GitRepository and Kustomization resources
//! - CrossplaneResource: Manage Crossplane composite resources and claims
//! - ExternalSecretConfig: Manage External Secrets Operator resources (SecretStore, ExternalSecret)
//! - Bucket: Multi-cloud bucket abstraction (GCP Cloud Storage, AWS S3, Azure Blob)
//! - Cluster: Multi-cloud Kubernetes cluster provisioning (GKE, EKS, AKS) via Crossplane with Vault auth
//! - DependencyLabeler: Auto-label Deployments based on env var dependencies (databases, auth, monitoring)
//! - PostgresProvisioner: Auto-provision CNPG clusters when postgres label is detected
//! - MongoProvisioner: Auto-provision MongoDB (Percona/Community/Deployment/Atlas) when mongo label is detected
//! - RedisProvisioner: Auto-provision Redis (Spotahome/Dragonfly/KeyDB/Deployment) when redis label is detected
//! - GoogleWorkspaceSync: Sync users and groups from Google Workspace for RBAC
//! - EmailService: Production-ready AWS SES via Crossplane (DKIM, MAIL FROM, IAM/IRSA)
//! - BedrockAccess: AWS Bedrock AI model access with IRSA (Claude, Titan, etc.)
//! - VertexAIAccess: Google Vertex AI access with Workload Identity (Gemini, PaLM, etc.)
//! - Admission Webhooks: Ownership injection and RBAC validation

use dotconfig::operator::{
    controllers::{
        bedrock_access, bucket, cluster, crossplane_resource, dependency_labeler, email_service,
        external_secret, gitops_app, google_workspace_sync, mongo_provisioner, platform_app,
        postgres_provisioner, redis_provisioner, vertex_ai_access,
    },
    types::{
        BedrockAccess, Bucket, Cluster, CrossplaneResource, EmailService, ExternalSecretConfig,
        GitOpsApp, GoogleWorkspaceConfig, PlatformApp, VertexAIAccess,
    },
    webhooks,
    Context,
};
use futures::StreamExt;
use k8s_openapi::api::apps::v1::Deployment;
use kube::runtime::controller::Controller;
use kube::runtime::watcher::Config as WatcherConfig;
use kube::{Api, Client};
use std::sync::Arc;
use tokio::net::TcpListener;
use tracing::{error, info};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .json()
        .init();

    info!("Starting Platform Operator");

    // Create the operator context
    let ctx = Arc::new(Context::try_default().await?);

    // Create a Kubernetes client
    let client = Client::try_default().await?;

    // Create APIs for each CRD
    let platform_apps: Api<PlatformApp> = Api::all(client.clone());
    let gitops_apps: Api<GitOpsApp> = Api::all(client.clone());
    let crossplane_resources: Api<CrossplaneResource> = Api::all(client.clone());
    let external_secret_configs: Api<ExternalSecretConfig> = Api::all(client.clone());
    let buckets: Api<Bucket> = Api::all(client.clone());
    let clusters: Api<Cluster> = Api::all(client.clone());
    let google_workspace_configs: Api<GoogleWorkspaceConfig> = Api::all(client.clone());
    let email_services: Api<EmailService> = Api::all(client.clone());
    let bedrock_accesses: Api<BedrockAccess> = Api::all(client.clone());
    let vertex_ai_accesses: Api<VertexAIAccess> = Api::all(client.clone());
    let deployments: Api<Deployment> = Api::all(client.clone());
    let deployments_for_postgres: Api<Deployment> = Api::all(client.clone());
    let deployments_for_mongo: Api<Deployment> = Api::all(client.clone());
    let deployments_for_redis: Api<Deployment> = Api::all(client.clone());

    info!("Starting controllers");

    // Run all controllers concurrently
    let platform_controller = Controller::new(platform_apps, WatcherConfig::default())
        .run(
            platform_app::reconcile,
            platform_app::error_policy,
            ctx.clone(),
        )
        .for_each(|res| async move {
            match res {
                Ok((obj, _action)) => {
                    info!("Reconciled PlatformApp: {:?}", obj);
                }
                Err(e) => {
                    error!("PlatformApp reconcile error: {:?}", e);
                }
            }
        });

    let gitops_controller = Controller::new(gitops_apps, WatcherConfig::default())
        .run(
            gitops_app::reconcile,
            gitops_app::error_policy,
            ctx.clone(),
        )
        .for_each(|res| async move {
            match res {
                Ok((obj, _action)) => {
                    info!("Reconciled GitOpsApp: {:?}", obj);
                }
                Err(e) => {
                    error!("GitOpsApp reconcile error: {:?}", e);
                }
            }
        });

    let crossplane_controller = Controller::new(crossplane_resources, WatcherConfig::default())
        .run(
            crossplane_resource::reconcile,
            crossplane_resource::error_policy,
            ctx.clone(),
        )
        .for_each(|res| async move {
            match res {
                Ok((obj, _action)) => {
                    info!("Reconciled CrossplaneResource: {:?}", obj);
                }
                Err(e) => {
                    error!("CrossplaneResource reconcile error: {:?}", e);
                }
            }
        });

    let external_secret_controller = Controller::new(external_secret_configs, WatcherConfig::default())
        .run(
            external_secret::reconcile,
            external_secret::error_policy,
            ctx.clone(),
        )
        .for_each(|res| async move {
            match res {
                Ok((obj, _action)) => {
                    info!("Reconciled ExternalSecretConfig: {:?}", obj);
                }
                Err(e) => {
                    error!("ExternalSecretConfig reconcile error: {:?}", e);
                }
            }
        });

    // Bucket controller - creates provider-specific Crossplane resources (GCP/AWS/Azure)
    let bucket_controller = Controller::new(buckets, WatcherConfig::default())
        .run(
            bucket::reconcile,
            bucket::error_policy,
            ctx.clone(),
        )
        .for_each(|res| async move {
            match res {
                Ok((obj, _action)) => {
                    info!("Reconciled Bucket: {:?}", obj);
                }
                Err(e) => {
                    error!("Bucket reconcile error: {:?}", e);
                }
            }
        });

    // Cluster controller - provisions GKE/EKS/AKS clusters via Crossplane with Vault auth
    let cluster_controller = Controller::new(clusters, WatcherConfig::default())
        .run(
            cluster::reconcile,
            cluster::error_policy,
            ctx.clone(),
        )
        .for_each(|res| async move {
            match res {
                Ok((obj, _action)) => {
                    info!("Reconciled Cluster: {:?}", obj);
                }
                Err(e) => {
                    error!("Cluster reconcile error: {:?}", e);
                }
            }
        });

    let dependency_labeler_controller = Controller::new(deployments, WatcherConfig::default())
        .run(
            dependency_labeler::reconcile,
            dependency_labeler::error_policy,
            ctx.clone(),
        )
        .for_each(|res| async move {
            match res {
                Ok((obj, _action)) => {
                    info!("Processed Deployment for dependency labels: {:?}", obj);
                }
                Err(e) => {
                    error!("DependencyLabeler reconcile error: {:?}", e);
                }
            }
        });

    // PostgreSQL Provisioner - watches for postgres label and provisions CNPG clusters
    let postgres_provisioner_controller =
        Controller::new(deployments_for_postgres, WatcherConfig::default())
            .run(
                postgres_provisioner::reconcile,
                postgres_provisioner::error_policy,
                ctx.clone(),
            )
            .for_each(|res| async move {
                match res {
                    Ok((obj, _action)) => {
                        info!("Processed Deployment for PostgreSQL provisioning: {:?}", obj);
                    }
                    Err(e) => {
                        error!("PostgresProvisioner reconcile error: {:?}", e);
                    }
                }
            });

    // MongoDB Provisioner - watches for mongo label and provisions MongoDB
    // Uses a fallback chain: Percona Operator > Community Operator > Deployment > Helm > Atlas
    let mongo_provisioner_controller =
        Controller::new(deployments_for_mongo, WatcherConfig::default())
            .run(
                mongo_provisioner::reconcile,
                mongo_provisioner::error_policy,
                ctx.clone(),
            )
            .for_each(|res| async move {
                match res {
                    Ok((obj, _action)) => {
                        info!("Processed Deployment for MongoDB provisioning: {:?}", obj);
                    }
                    Err(e) => {
                        error!("MongoProvisioner reconcile error: {:?}", e);
                    }
                }
            });

    // Redis Provisioner - watches for redis label and provisions Redis
    // Uses a fallback chain: Spotahome Operator > Dragonfly > KeyDB > Deployment > Helm
    let redis_provisioner_controller =
        Controller::new(deployments_for_redis, WatcherConfig::default())
            .run(
                redis_provisioner::reconcile,
                redis_provisioner::error_policy,
                ctx.clone(),
            )
            .for_each(|res| async move {
                match res {
                    Ok((obj, _action)) => {
                        info!("Processed Deployment for Redis provisioning: {:?}", obj);
                    }
                    Err(e) => {
                        error!("RedisProvisioner reconcile error: {:?}", e);
                    }
                }
            });

    // Google Workspace Sync - syncs users and groups from Google Workspace
    let google_workspace_controller =
        Controller::new(google_workspace_configs, WatcherConfig::default())
            .run(
                google_workspace_sync::reconcile,
                google_workspace_sync::error_policy,
                ctx.clone(),
            )
            .for_each(|res| async move {
                match res {
                    Ok((obj, _action)) => {
                        info!("Reconciled GoogleWorkspaceConfig: {:?}", obj);
                    }
                    Err(e) => {
                        error!("GoogleWorkspaceSync reconcile error: {:?}", e);
                    }
                }
            });

    // EmailService - provisions production-ready AWS SES via Crossplane
    // Includes: Domain verification, DKIM, MAIL FROM, Configuration Sets, IAM (IRSA)
    let email_service_controller = Controller::new(email_services, WatcherConfig::default())
        .run(
            email_service::reconcile,
            email_service::error_policy,
            ctx.clone(),
        )
        .for_each(|res| async move {
            match res {
                Ok((obj, _action)) => {
                    info!("Reconciled EmailService: {:?}", obj);
                }
                Err(e) => {
                    error!("EmailService reconcile error: {:?}", e);
                }
            }
        });

    // BedrockAccess - provisions AWS Bedrock access via Crossplane
    // Includes: IAM roles for IRSA, model access permissions, guardrails, knowledge bases
    let bedrock_access_controller = Controller::new(bedrock_accesses, WatcherConfig::default())
        .run(
            bedrock_access::reconcile,
            bedrock_access::error_policy,
            ctx.clone(),
        )
        .for_each(|res| async move {
            match res {
                Ok((obj, _action)) => {
                    info!("Reconciled BedrockAccess: {:?}", obj);
                }
                Err(e) => {
                    error!("BedrockAccess reconcile error: {:?}", e);
                }
            }
        });

    // VertexAIAccess - provisions Google Vertex AI access via Crossplane
    // Includes: GCP Service Accounts, Workload Identity, model endpoints, vector search
    let vertex_ai_access_controller = Controller::new(vertex_ai_accesses, WatcherConfig::default())
        .run(
            vertex_ai_access::reconcile,
            vertex_ai_access::error_policy,
            ctx.clone(),
        )
        .for_each(|res| async move {
            match res {
                Ok((obj, _action)) => {
                    info!("Reconciled VertexAIAccess: {:?}", obj);
                }
                Err(e) => {
                    error!("VertexAIAccess reconcile error: {:?}", e);
                }
            }
        });

    // Start webhook server
    let webhook_port = std::env::var("WEBHOOK_PORT").unwrap_or_else(|_| "8443".to_string());
    let webhook_addr = format!("0.0.0.0:{}", webhook_port);
    let webhook_router = webhooks::create_webhook_router(ctx.clone());

    let webhook_server = async {
        let listener = TcpListener::bind(&webhook_addr).await?;
        info!("Webhook server listening on {}", webhook_addr);
        axum::serve(listener, webhook_router).await
    };

    info!("Platform Operator is running");

    // Run all controllers and webhook server
    tokio::select! {
        _ = platform_controller => {
            error!("PlatformApp controller exited");
        }
        _ = gitops_controller => {
            error!("GitOpsApp controller exited");
        }
        _ = crossplane_controller => {
            error!("CrossplaneResource controller exited");
        }
        _ = external_secret_controller => {
            error!("ExternalSecretConfig controller exited");
        }
        _ = bucket_controller => {
            error!("Bucket controller exited");
        }
        _ = cluster_controller => {
            error!("Cluster controller exited");
        }
        _ = dependency_labeler_controller => {
            error!("DependencyLabeler controller exited");
        }
        _ = postgres_provisioner_controller => {
            error!("PostgresProvisioner controller exited");
        }
        _ = mongo_provisioner_controller => {
            error!("MongoProvisioner controller exited");
        }
        _ = redis_provisioner_controller => {
            error!("RedisProvisioner controller exited");
        }
        _ = google_workspace_controller => {
            error!("GoogleWorkspaceSync controller exited");
        }
        _ = email_service_controller => {
            error!("EmailService controller exited");
        }
        _ = bedrock_access_controller => {
            error!("BedrockAccess controller exited");
        }
        _ = vertex_ai_access_controller => {
            error!("VertexAIAccess controller exited");
        }
        result = webhook_server => {
            match result {
                Ok(_) => error!("Webhook server exited unexpectedly"),
                Err(e) => error!("Webhook server error: {:?}", e),
            }
        }
    }

    Ok(())
}
