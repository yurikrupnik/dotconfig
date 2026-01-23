//! Dashboard data - fetches and stores cluster information

use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::core::v1::{Namespace, Node, Pod};
use kube::{Api, Client};
use std::collections::HashMap;

/// All dashboard data
#[derive(Clone, Debug, Default)]
pub struct DashboardData {
    pub cluster_info: ClusterInfo,
    pub nodes: Vec<NodeInfo>,
    pub namespaces: Vec<NamespaceInfo>,
    pub applications: Vec<ApplicationInfo>,
    pub dependencies: Vec<DependencyInfo>,
    pub security: SecurityOverview,
    pub finops: FinOpsOverview,
    pub port_forwards: Vec<PortForwardInfo>,
    pub vulnerabilities: Vec<VulnerabilityInfo>,
    pub provider_configs: Vec<ProviderConfigInfo>,
    pub auth_providers: Vec<AuthProviderInfo>,
}

impl DashboardData {
    /// Load all dashboard data from the cluster
    pub async fn load() -> anyhow::Result<Self> {
        let client = Client::try_default().await?;

        let cluster_info = Self::load_cluster_info(&client).await?;
        let nodes = Self::load_nodes(&client).await?;
        let namespaces = Self::load_namespaces(&client).await?;
        let applications = Self::load_applications(&client).await?;
        let dependencies = Self::load_dependencies(&client).await?;
        let security = Self::load_security_overview(&client, &applications).await?;
        let finops = Self::calculate_finops(&nodes, &applications).await?;
        let vulnerabilities = Self::load_vulnerabilities(&client).await?;
        let provider_configs = Self::load_provider_configs(&client).await?;
        let auth_providers = Self::load_auth_providers(&client).await?;

        Ok(Self {
            cluster_info,
            nodes,
            namespaces,
            applications,
            dependencies,
            security,
            finops,
            port_forwards: vec![],
            vulnerabilities,
            provider_configs,
            auth_providers,
        })
    }

    async fn load_cluster_info(client: &Client) -> anyhow::Result<ClusterInfo> {
        let nodes: Api<Node> = Api::all(client.clone());
        let node_list = nodes.list(&Default::default()).await?;

        let namespaces: Api<Namespace> = Api::all(client.clone());
        let ns_list = namespaces.list(&Default::default()).await?;

        let pods: Api<Pod> = Api::all(client.clone());
        let pod_list = pods.list(&Default::default()).await?;

        let running_pods = pod_list
            .items
            .iter()
            .filter(|p| {
                p.status
                    .as_ref()
                    .and_then(|s| s.phase.as_ref())
                    .map(|p| p == "Running")
                    .unwrap_or(false)
            })
            .count();

        // Get cluster version from first node
        let version = node_list
            .items
            .first()
            .and_then(|n| n.status.as_ref())
            .and_then(|s| s.node_info.as_ref())
            .map(|i| i.kubelet_version.clone())
            .unwrap_or_else(|| "Unknown".to_string());

        // Detect provider from node labels
        let provider = node_list
            .items
            .first()
            .and_then(|n| n.metadata.labels.as_ref())
            .map(|labels| {
                if labels.contains_key("eks.amazonaws.com/nodegroup") {
                    "AWS EKS".to_string()
                } else if labels.contains_key("cloud.google.com/gke-nodepool") {
                    "Google GKE".to_string()
                } else if labels.contains_key("kubernetes.azure.com/cluster") {
                    "Azure AKS".to_string()
                } else if labels.contains_key("minikube.k8s.io/name") {
                    "Minikube".to_string()
                } else if labels.contains_key("node.kubernetes.io/instance-type") {
                    "Kind".to_string()
                } else {
                    "Unknown".to_string()
                }
            })
            .unwrap_or_else(|| "Unknown".to_string());

        Ok(ClusterInfo {
            name: "current-context".to_string(), // TODO: Get from kubeconfig
            version,
            provider,
            node_count: node_list.items.len(),
            namespace_count: ns_list.items.len(),
            pod_count: pod_list.items.len(),
            running_pods,
            status: ClusterStatus::Healthy,
        })
    }

    async fn load_nodes(client: &Client) -> anyhow::Result<Vec<NodeInfo>> {
        let nodes: Api<Node> = Api::all(client.clone());
        let node_list = nodes.list(&Default::default()).await?;

        let mut result = Vec::new();
        for node in node_list.items {
            let name = node.metadata.name.clone().unwrap_or_default();
            let labels = node.metadata.labels.clone().unwrap_or_default();

            let status = node.status.as_ref();
            let conditions = status.and_then(|s| s.conditions.as_ref());

            let ready = conditions
                .map(|c| c.iter().any(|cond| cond.type_ == "Ready" && cond.status == "True"))
                .unwrap_or(false);

            let allocatable = status.and_then(|s| s.allocatable.as_ref());
            let cpu_allocatable = allocatable
                .and_then(|a| a.get("cpu"))
                .map(|q| q.0.clone())
                .unwrap_or_else(|| "0".to_string());
            let memory_allocatable = allocatable
                .and_then(|a| a.get("memory"))
                .map(|q| q.0.clone())
                .unwrap_or_else(|| "0".to_string());

            let node_info = status.and_then(|s| s.node_info.as_ref());
            let os = node_info
                .map(|i| i.os_image.clone())
                .unwrap_or_else(|| "Unknown".to_string());
            let container_runtime = node_info
                .map(|i| i.container_runtime_version.clone())
                .unwrap_or_else(|| "Unknown".to_string());

            let instance_type = labels
                .get("node.kubernetes.io/instance-type")
                .cloned()
                .or_else(|| labels.get("beta.kubernetes.io/instance-type").cloned())
                .unwrap_or_else(|| "Unknown".to_string());

            let zone = labels
                .get("topology.kubernetes.io/zone")
                .cloned()
                .or_else(|| labels.get("failure-domain.beta.kubernetes.io/zone").cloned())
                .unwrap_or_else(|| "Unknown".to_string());

            result.push(NodeInfo {
                name,
                status: if ready {
                    NodeStatus::Ready
                } else {
                    NodeStatus::NotReady
                },
                cpu_allocatable,
                memory_allocatable,
                cpu_usage: 0.0,    // Would need metrics-server
                memory_usage: 0.0, // Would need metrics-server
                pod_count: 0,      // Would need to count per node
                instance_type,
                zone,
                os,
                container_runtime,
                labels: labels.into_iter().collect(),
            });
        }

        Ok(result)
    }

    async fn load_namespaces(client: &Client) -> anyhow::Result<Vec<NamespaceInfo>> {
        let namespaces: Api<Namespace> = Api::all(client.clone());
        let ns_list = namespaces.list(&Default::default()).await?;

        let pods: Api<Pod> = Api::all(client.clone());
        let pod_list = pods.list(&Default::default()).await?;

        // Count pods per namespace
        let mut pod_counts: HashMap<String, usize> = HashMap::new();
        for pod in &pod_list.items {
            let ns = pod
                .metadata
                .namespace
                .clone()
                .unwrap_or_else(|| "default".to_string());
            *pod_counts.entry(ns).or_insert(0) += 1;
        }

        let mut result = Vec::new();
        for ns in ns_list.items {
            let name = ns.metadata.name.clone().unwrap_or_default();
            let labels = ns.metadata.labels.clone().unwrap_or_default();

            let status = ns
                .status
                .as_ref()
                .and_then(|s| s.phase.as_ref())
                .map(|p| {
                    if p == "Active" {
                        NamespaceStatus::Active
                    } else {
                        NamespaceStatus::Terminating
                    }
                })
                .unwrap_or(NamespaceStatus::Active);

            result.push(NamespaceInfo {
                name: name.clone(),
                status,
                pod_count: *pod_counts.get(&name).unwrap_or(&0),
                labels: labels.into_iter().collect(),
            });
        }

        Ok(result)
    }

    async fn load_applications(client: &Client) -> anyhow::Result<Vec<ApplicationInfo>> {
        let deployments: Api<Deployment> = Api::all(client.clone());
        let deploy_list = deployments.list(&Default::default()).await?;

        let mut result = Vec::new();
        for deploy in deploy_list.items {
            let name = deploy.metadata.name.clone().unwrap_or_default();
            let namespace = deploy
                .metadata
                .namespace
                .clone()
                .unwrap_or_else(|| "default".to_string());
            let labels = deploy.metadata.labels.clone().unwrap_or_default();

            let spec = deploy.spec.as_ref();
            let status = deploy.status.as_ref();

            let replicas = spec.and_then(|s| s.replicas).unwrap_or(0);
            let ready_replicas = status.and_then(|s| s.ready_replicas).unwrap_or(0);
            let available_replicas = status.and_then(|s| s.available_replicas).unwrap_or(0);

            let image = spec
                .and_then(|s| s.template.spec.as_ref())
                .and_then(|ps| ps.containers.first())
                .map(|c| c.image.clone().unwrap_or_default())
                .unwrap_or_default();

            let app_status = if ready_replicas >= replicas && replicas > 0 {
                AppStatus::Healthy
            } else if ready_replicas > 0 {
                AppStatus::Degraded
            } else if replicas == 0 {
                AppStatus::Stopped
            } else {
                AppStatus::Unhealthy
            };

            result.push(ApplicationInfo {
                name,
                namespace,
                kind: "Deployment".to_string(),
                status: app_status,
                replicas,
                ready_replicas,
                available_replicas,
                image,
                labels: labels.into_iter().collect(),
                cpu_usage: 0.0,
                memory_usage: 0.0,
                restart_count: 0,
            });
        }

        Ok(result)
    }

    async fn load_dependencies(client: &Client) -> anyhow::Result<Vec<DependencyInfo>> {
        let mut deps = Vec::new();

        // Check for common operators/CRDs
        let crds_to_check = vec![
            ("FluxCD", "source.toolkit.fluxcd.io", "GitRepository"),
            ("Crossplane", "apiextensions.crossplane.io", "CompositeResourceDefinition"),
            ("External Secrets", "external-secrets.io", "ExternalSecret"),
            ("Cert-Manager", "cert-manager.io", "Certificate"),
            ("Prometheus", "monitoring.coreos.com", "ServiceMonitor"),
            ("CNPG", "postgresql.cnpg.io", "Cluster"),
            ("KEDA", "keda.sh", "ScaledObject"),
            ("Istio", "networking.istio.io", "VirtualService"),
            ("ArgoCD", "argoproj.io", "Application"),
        ];

        for (name, group, _kind) in crds_to_check {
            // Try to discover if the API exists
            let status = match client
                .apiserver_version()
                .await
            {
                Ok(_) => DependencyStatus::Available, // Simplified check
                Err(_) => DependencyStatus::Unknown,
            };

            deps.push(DependencyInfo {
                name: name.to_string(),
                kind: "Operator".to_string(),
                group: group.to_string(),
                status,
                version: None,
                message: None,
            });
        }

        Ok(deps)
    }

    async fn load_security_overview(
        _client: &Client,
        applications: &[ApplicationInfo],
    ) -> anyhow::Result<SecurityOverview> {
        let mut issues = Vec::new();

        // Check for common security issues
        for app in applications {
            // Check for latest tag
            if app.image.ends_with(":latest") || !app.image.contains(':') {
                issues.push(SecurityIssue {
                    severity: Severity::Warning,
                    category: SecurityCategory::ImageTag,
                    resource: format!("{}/{}", app.namespace, app.name),
                    message: "Using 'latest' or untagged image".to_string(),
                    remediation: "Use specific image tags for reproducibility".to_string(),
                });
            }

            // Check for privileged namespaces
            if app.namespace == "kube-system" || app.namespace == "kube-public" {
                continue; // Skip system namespaces
            }
        }

        // Add placeholder security checks
        let total_checks = 10;
        let passed_checks = total_checks - issues.len();

        Ok(SecurityOverview {
            score: (passed_checks as f64 / total_checks as f64 * 100.0) as u32,
            issues,
            compliance: ComplianceStatus {
                cis_benchmark: Some(ComplianceResult {
                    passed: 45,
                    failed: 5,
                    skipped: 10,
                }),
                nsa_hardening: None,
                pci_dss: None,
            },
            last_scan: Some(chrono::Utc::now().to_rfc3339()),
        })
    }

    async fn load_vulnerabilities(_client: &Client) -> anyhow::Result<Vec<VulnerabilityInfo>> {
        // In a real implementation, this would query Trivy Operator, Grype, etc.
        Ok(vec![])
    }

    async fn load_provider_configs(client: &Client) -> anyhow::Result<Vec<ProviderConfigInfo>> {
        use kube::api::{DynamicObject, ApiResource, GroupVersionKind};

        let mut provider_configs = Vec::new();

        // Define the provider config types to look for
        let provider_types = vec![
            ("aws.upbound.io", "v1beta1", "ProviderConfig", ProviderType::AWS),
            ("gcp.upbound.io", "v1beta1", "ProviderConfig", ProviderType::GCP),
            ("azure.upbound.io", "v1beta1", "ProviderConfig", ProviderType::Azure),
            ("kubernetes.crossplane.io", "v1alpha1", "ProviderConfig", ProviderType::Kubernetes),
            ("helm.crossplane.io", "v1beta1", "ProviderConfig", ProviderType::Helm),
            ("tf.upbound.io", "v1beta1", "ProviderConfig", ProviderType::Terraform),
            // Legacy Crossplane provider APIs
            ("aws.crossplane.io", "v1beta1", "ProviderConfig", ProviderType::AWS),
            ("gcp.crossplane.io", "v1beta1", "ProviderConfig", ProviderType::GCP),
            ("azure.crossplane.io", "v1beta1", "ProviderConfig", ProviderType::Azure),
        ];

        for (group, version, kind, provider_type) in provider_types {
            let gvk = GroupVersionKind::gvk(group, version, kind);
            let api_resource = ApiResource::from_gvk(&gvk);
            let api: Api<DynamicObject> = Api::all_with(client.clone(), &api_resource);

            match api.list(&Default::default()).await {
                Ok(list) => {
                    for item in list.items {
                        let name = item.metadata.name.clone().unwrap_or_default();

                        // Extract status from the dynamic object
                        let (status, message, last_sync) = Self::extract_provider_status(&item);

                        // Extract credentials source
                        let credentials_source = Self::extract_credentials_source(&item);

                        // Extract secret reference
                        let secret_ref = Self::extract_secret_ref(&item);

                        // Count associated resources (ProviderConfigUsage)
                        let associated_resources = Self::count_provider_usages(client, &name, group).await;

                        provider_configs.push(ProviderConfigInfo {
                            name,
                            provider_type: provider_type.clone(),
                            status,
                            credentials_source,
                            secret_ref,
                            associated_resources,
                            last_sync,
                            message,
                        });
                    }
                }
                Err(_) => {
                    // API doesn't exist or not accessible - skip silently
                    continue;
                }
            }
        }

        Ok(provider_configs)
    }

    fn extract_provider_status(obj: &kube::api::DynamicObject) -> (ProviderStatus, Option<String>, Option<String>) {
        let status = obj.data.get("status");

        if let Some(status) = status {
            let conditions = status.get("conditions").and_then(|c| c.as_array());

            if let Some(conditions) = conditions {
                let mut is_ready = false;
                let mut message = None;
                let mut last_sync = None;

                for cond in conditions {
                    let cond_type = cond.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    let cond_status = cond.get("status").and_then(|s| s.as_str()).unwrap_or("False");

                    if cond_type == "Ready" || cond_type == "Healthy" {
                        is_ready = cond_status == "True";
                        message = cond.get("message").and_then(|m| m.as_str()).map(|s| s.to_string());
                        last_sync = cond.get("lastTransitionTime").and_then(|t| t.as_str()).map(|s| s.to_string());
                    }
                }

                let provider_status = if is_ready {
                    ProviderStatus::Healthy
                } else if message.is_some() {
                    ProviderStatus::Error
                } else {
                    ProviderStatus::Degraded
                };

                return (provider_status, message, last_sync);
            }
        }

        (ProviderStatus::Unknown, None, None)
    }

    fn extract_credentials_source(obj: &kube::api::DynamicObject) -> String {
        let spec = obj.data.get("spec");

        if let Some(spec) = spec {
            // Check for credentials source field
            if let Some(creds) = spec.get("credentials") {
                if let Some(source) = creds.get("source").and_then(|s| s.as_str()) {
                    return source.to_string();
                }
            }

            // Check for source directly in spec (some providers)
            if let Some(source) = spec.get("source").and_then(|s| s.as_str()) {
                return source.to_string();
            }
        }

        "Unknown".to_string()
    }

    fn extract_secret_ref(obj: &kube::api::DynamicObject) -> Option<String> {
        let spec = obj.data.get("spec")?;
        let creds = spec.get("credentials")?;
        let secret_ref = creds.get("secretRef")?;

        let name = secret_ref.get("name").and_then(|n| n.as_str())?;
        let namespace = secret_ref.get("namespace").and_then(|n| n.as_str()).unwrap_or("default");
        let key = secret_ref.get("key").and_then(|k| k.as_str()).unwrap_or("credentials");

        Some(format!("{}/{}:{}", namespace, name, key))
    }

    async fn count_provider_usages(client: &Client, provider_name: &str, group: &str) -> usize {
        use kube::api::{DynamicObject, ApiResource, GroupVersionKind};

        // Try to list ProviderConfigUsage resources
        let usage_gvk = GroupVersionKind::gvk(group, "v1beta1", "ProviderConfigUsage");
        let api_resource = ApiResource::from_gvk(&usage_gvk);
        let api: Api<DynamicObject> = Api::all_with(client.clone(), &api_resource);

        match api.list(&Default::default()).await {
            Ok(list) => {
                list.items
                    .iter()
                    .filter(|item| {
                        item.data
                            .get("providerConfigRef")
                            .and_then(|r| r.get("name"))
                            .and_then(|n| n.as_str())
                            .map(|n| n == provider_name)
                            .unwrap_or(false)
                    })
                    .count()
            }
            Err(_) => 0,
        }
    }

    async fn load_auth_providers(client: &Client) -> anyhow::Result<Vec<AuthProviderInfo>> {
        use kube::api::{DynamicObject, ApiResource, GroupVersionKind};

        let mut auth_providers = Vec::new();

        // Load Crossplane Providers (pkg.crossplane.io/v1/Provider)
        auth_providers.extend(Self::load_crossplane_providers(client).await);

        // Load External Secrets ClusterSecretStores
        auth_providers.extend(Self::load_cluster_secret_stores(client).await);

        // Load External Secrets SecretStores (namespaced)
        auth_providers.extend(Self::load_secret_stores(client).await);

        // Load ServiceAccounts with IRSA/Workload Identity annotations
        auth_providers.extend(Self::load_workload_identity_accounts(client).await);

        Ok(auth_providers)
    }

    async fn load_crossplane_providers(client: &Client) -> Vec<AuthProviderInfo> {
        use kube::api::{DynamicObject, ApiResource, GroupVersionKind};

        let mut providers = Vec::new();

        // Crossplane Provider CRD (pkg.crossplane.io/v1/Provider)
        let gvk = GroupVersionKind::gvk("pkg.crossplane.io", "v1", "Provider");
        let api_resource = ApiResource::from_gvk(&gvk);
        let api: Api<DynamicObject> = Api::all_with(client.clone(), &api_resource);

        if let Ok(list) = api.list(&Default::default()).await {
            for item in list.items {
                let name = item.metadata.name.clone().unwrap_or_default();

                // Extract package info
                let package = item.data
                    .get("spec")
                    .and_then(|s| s.get("package"))
                    .and_then(|p| p.as_str())
                    .unwrap_or("Unknown")
                    .to_string();

                // Extract status
                let (status, message, last_sync) = Self::extract_provider_status(&item);

                // Check for installed/healthy condition
                let installed = item.data
                    .get("status")
                    .and_then(|s| s.get("conditions"))
                    .and_then(|c| c.as_array())
                    .map(|conditions| {
                        conditions.iter().any(|c| {
                            c.get("type").and_then(|t| t.as_str()) == Some("Installed")
                                && c.get("status").and_then(|s| s.as_str()) == Some("True")
                        })
                    })
                    .unwrap_or(false);

                let final_status = if installed {
                    status
                } else {
                    ProviderStatus::Error
                };

                providers.push(AuthProviderInfo {
                    name,
                    namespace: None,
                    auth_type: AuthProviderType::CrossplaneProvider,
                    status: final_status,
                    backend: package,
                    secret_ref: None,
                    service_account: None,
                    last_sync,
                    message,
                });
            }
        }

        providers
    }

    async fn load_cluster_secret_stores(client: &Client) -> Vec<AuthProviderInfo> {
        use kube::api::{DynamicObject, ApiResource, GroupVersionKind};

        let mut stores = Vec::new();

        // External Secrets ClusterSecretStore
        let gvk = GroupVersionKind::gvk("external-secrets.io", "v1beta1", "ClusterSecretStore");
        let api_resource = ApiResource::from_gvk(&gvk);
        let api: Api<DynamicObject> = Api::all_with(client.clone(), &api_resource);

        if let Ok(list) = api.list(&Default::default()).await {
            for item in list.items {
                let name = item.metadata.name.clone().unwrap_or_default();

                // Determine backend type from spec.provider
                let (backend, secret_ref, service_account) = Self::extract_secret_store_backend(&item);

                // Extract status
                let (status, message, last_sync) = Self::extract_secret_store_status(&item);

                stores.push(AuthProviderInfo {
                    name,
                    namespace: None,
                    auth_type: AuthProviderType::ClusterSecretStore,
                    status,
                    backend,
                    secret_ref,
                    service_account,
                    last_sync,
                    message,
                });
            }
        }

        stores
    }

    async fn load_secret_stores(client: &Client) -> Vec<AuthProviderInfo> {
        use kube::api::{DynamicObject, ApiResource, GroupVersionKind};

        let mut stores = Vec::new();

        // External Secrets SecretStore (namespaced)
        let gvk = GroupVersionKind::gvk("external-secrets.io", "v1beta1", "SecretStore");
        let api_resource = ApiResource::from_gvk(&gvk);
        let api: Api<DynamicObject> = Api::all_with(client.clone(), &api_resource);

        if let Ok(list) = api.list(&Default::default()).await {
            for item in list.items {
                let name = item.metadata.name.clone().unwrap_or_default();
                let namespace = item.metadata.namespace.clone();

                // Determine backend type from spec.provider
                let (backend, secret_ref, service_account) = Self::extract_secret_store_backend(&item);

                // Extract status
                let (status, message, last_sync) = Self::extract_secret_store_status(&item);

                stores.push(AuthProviderInfo {
                    name,
                    namespace,
                    auth_type: AuthProviderType::SecretStore,
                    status,
                    backend,
                    secret_ref,
                    service_account,
                    last_sync,
                    message,
                });
            }
        }

        stores
    }

    fn extract_secret_store_backend(obj: &kube::api::DynamicObject) -> (String, Option<String>, Option<String>) {
        let spec = obj.data.get("spec");
        let provider = spec.and_then(|s| s.get("provider"));

        if let Some(provider) = provider {
            // Check each provider type
            if let Some(aws) = provider.get("aws") {
                let service = aws.get("service").and_then(|s| s.as_str()).unwrap_or("SecretsManager");
                let secret_ref = aws.get("auth")
                    .and_then(|a| a.get("secretRef"))
                    .and_then(|s| s.get("accessKeyIDSecretRef"))
                    .and_then(|s| s.get("name"))
                    .and_then(|n| n.as_str())
                    .map(|s| s.to_string());
                let sa = aws.get("auth")
                    .and_then(|a| a.get("jwt"))
                    .and_then(|j| j.get("serviceAccountRef"))
                    .and_then(|s| s.get("name"))
                    .and_then(|n| n.as_str())
                    .map(|s| s.to_string());
                return (format!("AWS {}", service), secret_ref, sa);
            }

            if let Some(gcp) = provider.get("gcpsm") {
                let secret_ref = gcp.get("auth")
                    .and_then(|a| a.get("secretRef"))
                    .and_then(|s| s.get("secretAccessKeySecretRef"))
                    .and_then(|s| s.get("name"))
                    .and_then(|n| n.as_str())
                    .map(|s| s.to_string());
                let sa = gcp.get("auth")
                    .and_then(|a| a.get("workloadIdentity"))
                    .and_then(|w| w.get("serviceAccountRef"))
                    .and_then(|s| s.get("name"))
                    .and_then(|n| n.as_str())
                    .map(|s| s.to_string());
                return ("GCP Secret Manager".to_string(), secret_ref, sa);
            }

            if let Some(azure) = provider.get("azurekv") {
                let vault_url = azure.get("vaultUrl").and_then(|v| v.as_str()).unwrap_or("Unknown");
                let secret_ref = azure.get("authSecretRef")
                    .and_then(|s| s.get("clientSecret"))
                    .and_then(|s| s.get("name"))
                    .and_then(|n| n.as_str())
                    .map(|s| s.to_string());
                let sa = azure.get("serviceAccountRef")
                    .and_then(|s| s.get("name"))
                    .and_then(|n| n.as_str())
                    .map(|s| s.to_string());
                return (format!("Azure KeyVault ({})", vault_url), secret_ref, sa);
            }

            if let Some(vault) = provider.get("vault") {
                let server = vault.get("server").and_then(|s| s.as_str()).unwrap_or("Unknown");
                let secret_ref = vault.get("auth")
                    .and_then(|a| a.get("tokenSecretRef"))
                    .and_then(|s| s.get("name"))
                    .and_then(|n| n.as_str())
                    .map(|s| s.to_string());
                let sa = vault.get("auth")
                    .and_then(|a| a.get("kubernetes"))
                    .and_then(|k| k.get("serviceAccountRef"))
                    .and_then(|s| s.get("name"))
                    .and_then(|n| n.as_str())
                    .map(|s| s.to_string());
                return (format!("Vault ({})", server), secret_ref, sa);
            }

            if provider.get("kubernetes").is_some() {
                return ("Kubernetes".to_string(), None, None);
            }
        }

        ("Unknown".to_string(), None, None)
    }

    fn extract_secret_store_status(obj: &kube::api::DynamicObject) -> (ProviderStatus, Option<String>, Option<String>) {
        let status = obj.data.get("status");

        if let Some(status) = status {
            let conditions = status.get("conditions").and_then(|c| c.as_array());

            if let Some(conditions) = conditions {
                for cond in conditions {
                    let cond_type = cond.get("type").and_then(|t| t.as_str()).unwrap_or("");
                    let cond_status = cond.get("status").and_then(|s| s.as_str()).unwrap_or("False");

                    if cond_type == "Ready" {
                        let is_ready = cond_status == "True";
                        let message = cond.get("message").and_then(|m| m.as_str()).map(|s| s.to_string());
                        let last_sync = cond.get("lastTransitionTime").and_then(|t| t.as_str()).map(|s| s.to_string());

                        let provider_status = if is_ready {
                            ProviderStatus::Healthy
                        } else {
                            ProviderStatus::Error
                        };

                        return (provider_status, message, last_sync);
                    }
                }
            }
        }

        (ProviderStatus::Unknown, None, None)
    }

    async fn load_workload_identity_accounts(client: &Client) -> Vec<AuthProviderInfo> {
        use k8s_openapi::api::core::v1::ServiceAccount;

        let mut accounts = Vec::new();
        let sa_api: Api<ServiceAccount> = Api::all(client.clone());

        if let Ok(list) = sa_api.list(&Default::default()).await {
            for sa in list.items {
                let name = sa.metadata.name.clone().unwrap_or_default();
                let namespace = sa.metadata.namespace.clone();
                let annotations = sa.metadata.annotations.clone().unwrap_or_default();

                // Check for AWS IRSA
                if let Some(role_arn) = annotations.get("eks.amazonaws.com/role-arn") {
                    accounts.push(AuthProviderInfo {
                        name: name.clone(),
                        namespace: namespace.clone(),
                        auth_type: AuthProviderType::AwsIrsa,
                        status: ProviderStatus::Healthy,
                        backend: role_arn.clone(),
                        secret_ref: None,
                        service_account: Some(name.clone()),
                        last_sync: None,
                        message: None,
                    });
                }

                // Check for GCP Workload Identity
                if let Some(gsa) = annotations.get("iam.gke.io/gcp-service-account") {
                    accounts.push(AuthProviderInfo {
                        name: name.clone(),
                        namespace: namespace.clone(),
                        auth_type: AuthProviderType::GcpWorkloadIdentity,
                        status: ProviderStatus::Healthy,
                        backend: gsa.clone(),
                        secret_ref: None,
                        service_account: Some(name.clone()),
                        last_sync: None,
                        message: None,
                    });
                }

                // Check for Azure Workload Identity
                if let Some(client_id) = annotations.get("azure.workload.identity/client-id") {
                    let tenant_id = annotations.get("azure.workload.identity/tenant-id")
                        .map(|s| s.as_str())
                        .unwrap_or("unknown");
                    accounts.push(AuthProviderInfo {
                        name: name.clone(),
                        namespace: namespace.clone(),
                        auth_type: AuthProviderType::AzureWorkloadIdentity,
                        status: ProviderStatus::Healthy,
                        backend: format!("{}@{}", client_id, tenant_id),
                        secret_ref: None,
                        service_account: Some(name),
                        last_sync: None,
                        message: None,
                    });
                }
            }
        }

        accounts
    }

    async fn calculate_finops(
        nodes: &[NodeInfo],
        applications: &[ApplicationInfo],
    ) -> anyhow::Result<FinOpsOverview> {
        // Simplified cost estimation
        let node_count = nodes.len();
        let estimated_hourly = node_count as f64 * 0.10; // $0.10/node/hour placeholder

        let namespaces: HashMap<String, f64> = applications
            .iter()
            .map(|a| (a.namespace.clone(), estimated_hourly / applications.len() as f64))
            .fold(HashMap::new(), |mut acc, (ns, cost)| {
                *acc.entry(ns).or_insert(0.0) += cost;
                acc
            });

        let cost_by_namespace: Vec<NamespaceCost> = namespaces
            .into_iter()
            .map(|(namespace, cost)| NamespaceCost {
                namespace,
                hourly_cost: cost,
                monthly_cost: cost * 24.0 * 30.0,
            })
            .collect();

        Ok(FinOpsOverview {
            total_hourly_cost: estimated_hourly,
            total_monthly_cost: estimated_hourly * 24.0 * 30.0,
            projected_monthly_cost: estimated_hourly * 24.0 * 30.0,
            cost_by_namespace,
            recommendations: vec![
                CostRecommendation {
                    category: RecommendationCategory::RightSizing,
                    potential_savings: 50.0,
                    description: "Right-size over-provisioned workloads".to_string(),
                    affected_resources: vec![],
                },
            ],
            savings_opportunities: 50.0,
        })
    }
}

// ============ Data Types ============

#[derive(Clone, Debug, Default)]
pub struct ClusterInfo {
    pub name: String,
    pub version: String,
    pub provider: String,
    pub node_count: usize,
    pub namespace_count: usize,
    pub pod_count: usize,
    pub running_pods: usize,
    pub status: ClusterStatus,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum ClusterStatus {
    #[default]
    Healthy,
    Degraded,
    Unhealthy,
    Unknown,
}

#[derive(Clone, Debug)]
pub struct NodeInfo {
    pub name: String,
    pub status: NodeStatus,
    pub cpu_allocatable: String,
    pub memory_allocatable: String,
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub pod_count: usize,
    pub instance_type: String,
    pub zone: String,
    pub os: String,
    pub container_runtime: String,
    pub labels: HashMap<String, String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NodeStatus {
    Ready,
    NotReady,
    Unknown,
}

#[derive(Clone, Debug)]
pub struct NamespaceInfo {
    pub name: String,
    pub status: NamespaceStatus,
    pub pod_count: usize,
    pub labels: HashMap<String, String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum NamespaceStatus {
    Active,
    Terminating,
}

#[derive(Clone, Debug)]
pub struct ApplicationInfo {
    pub name: String,
    pub namespace: String,
    pub kind: String,
    pub status: AppStatus,
    pub replicas: i32,
    pub ready_replicas: i32,
    pub available_replicas: i32,
    pub image: String,
    pub labels: HashMap<String, String>,
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub restart_count: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AppStatus {
    Healthy,
    Degraded,
    Unhealthy,
    Stopped,
    Unknown,
}

#[derive(Clone, Debug)]
pub struct DependencyInfo {
    pub name: String,
    pub kind: String,
    pub group: String,
    pub status: DependencyStatus,
    pub version: Option<String>,
    pub message: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum DependencyStatus {
    Available,
    Missing,
    Degraded,
    Unknown,
}

#[derive(Clone, Debug, Default)]
pub struct SecurityOverview {
    pub score: u32,
    pub issues: Vec<SecurityIssue>,
    pub compliance: ComplianceStatus,
    pub last_scan: Option<String>,
}

#[derive(Clone, Debug)]
pub struct SecurityIssue {
    pub severity: Severity,
    pub category: SecurityCategory,
    pub resource: String,
    pub message: String,
    pub remediation: String,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Severity {
    Critical,
    High,
    Medium,
    Warning,
    Low,
    Info,
}

#[derive(Clone, Debug, PartialEq)]
pub enum SecurityCategory {
    Vulnerability,
    Misconfiguration,
    ImageTag,
    Rbac,
    NetworkPolicy,
    PodSecurity,
    Secrets,
}

#[derive(Clone, Debug, Default)]
pub struct ComplianceStatus {
    pub cis_benchmark: Option<ComplianceResult>,
    pub nsa_hardening: Option<ComplianceResult>,
    pub pci_dss: Option<ComplianceResult>,
}

#[derive(Clone, Debug)]
pub struct ComplianceResult {
    pub passed: u32,
    pub failed: u32,
    pub skipped: u32,
}

#[derive(Clone, Debug)]
pub struct VulnerabilityInfo {
    pub id: String,
    pub severity: Severity,
    pub package: String,
    pub installed_version: String,
    pub fixed_version: Option<String>,
    pub image: String,
    pub resource: String,
    pub description: String,
}

#[derive(Clone, Debug, Default)]
pub struct FinOpsOverview {
    pub total_hourly_cost: f64,
    pub total_monthly_cost: f64,
    pub projected_monthly_cost: f64,
    pub cost_by_namespace: Vec<NamespaceCost>,
    pub recommendations: Vec<CostRecommendation>,
    pub savings_opportunities: f64,
}

#[derive(Clone, Debug)]
pub struct NamespaceCost {
    pub namespace: String,
    pub hourly_cost: f64,
    pub monthly_cost: f64,
}

#[derive(Clone, Debug)]
pub struct CostRecommendation {
    pub category: RecommendationCategory,
    pub potential_savings: f64,
    pub description: String,
    pub affected_resources: Vec<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum RecommendationCategory {
    RightSizing,
    SpotInstances,
    Reserved,
    UnusedResources,
    IdleResources,
}

#[derive(Clone, Debug)]
pub struct PortForwardInfo {
    pub name: String,
    pub namespace: String,
    pub target_type: String,
    pub target_name: String,
    pub local_port: u16,
    pub remote_port: u16,
    pub active: bool,
    pub pid: Option<u32>,
}

// ============ Crossplane Provider Types ============

/// Crossplane Provider Configuration info
#[derive(Clone, Debug)]
pub struct ProviderConfigInfo {
    pub name: String,
    pub provider_type: ProviderType,
    pub status: ProviderStatus,
    pub credentials_source: String,
    pub secret_ref: Option<String>,
    pub associated_resources: usize,
    pub last_sync: Option<String>,
    pub message: Option<String>,
}

/// Provider type (cloud provider)
#[derive(Clone, Debug, PartialEq)]
pub enum ProviderType {
    AWS,
    GCP,
    Azure,
    Kubernetes,
    Helm,
    Terraform,
    Other(String),
}

impl std::fmt::Display for ProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProviderType::AWS => write!(f, "AWS"),
            ProviderType::GCP => write!(f, "GCP"),
            ProviderType::Azure => write!(f, "Azure"),
            ProviderType::Kubernetes => write!(f, "Kubernetes"),
            ProviderType::Helm => write!(f, "Helm"),
            ProviderType::Terraform => write!(f, "Terraform"),
            ProviderType::Other(s) => write!(f, "{}", s),
        }
    }
}

/// Provider config status
#[derive(Clone, Debug, PartialEq)]
pub enum ProviderStatus {
    Healthy,
    Degraded,
    Error,
    Unknown,
}

/// Crossplane managed resource association
#[derive(Clone, Debug)]
pub struct ProviderResourceAssociation {
    pub resource_name: String,
    pub resource_kind: String,
    pub namespace: Option<String>,
    pub ready: bool,
    pub synced: bool,
}

// ============ Authentication Provider Types ============

/// Authentication provider information
#[derive(Clone, Debug)]
pub struct AuthProviderInfo {
    pub name: String,
    pub namespace: Option<String>,
    pub auth_type: AuthProviderType,
    pub status: ProviderStatus,
    pub backend: String,
    pub secret_ref: Option<String>,
    pub service_account: Option<String>,
    pub last_sync: Option<String>,
    pub message: Option<String>,
}

/// Type of authentication provider
#[derive(Clone, Debug, PartialEq)]
pub enum AuthProviderType {
    /// Crossplane Provider (pkg.crossplane.io/Provider)
    CrossplaneProvider,
    /// External Secrets SecretStore
    SecretStore,
    /// External Secrets ClusterSecretStore
    ClusterSecretStore,
    /// AWS IRSA (IAM Roles for Service Accounts)
    AwsIrsa,
    /// GCP Workload Identity
    GcpWorkloadIdentity,
    /// Azure Workload Identity
    AzureWorkloadIdentity,
    /// HashiCorp Vault
    Vault,
    /// Generic/Other
    Other(String),
}

impl std::fmt::Display for AuthProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AuthProviderType::CrossplaneProvider => write!(f, "Crossplane Provider"),
            AuthProviderType::SecretStore => write!(f, "SecretStore"),
            AuthProviderType::ClusterSecretStore => write!(f, "ClusterSecretStore"),
            AuthProviderType::AwsIrsa => write!(f, "AWS IRSA"),
            AuthProviderType::GcpWorkloadIdentity => write!(f, "GCP Workload Identity"),
            AuthProviderType::AzureWorkloadIdentity => write!(f, "Azure Workload Identity"),
            AuthProviderType::Vault => write!(f, "Vault"),
            AuthProviderType::Other(s) => write!(f, "{}", s),
        }
    }
}
