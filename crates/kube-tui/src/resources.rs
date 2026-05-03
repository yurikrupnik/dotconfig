use std::collections::HashMap;
use k8s_openapi::api::apps::v1::Deployment;
use k8s_openapi::api::apps::v1::DaemonSet;
use k8s_openapi::api::apps::v1::StatefulSet;
use k8s_openapi::api::apps::v1::ReplicaSet;
use k8s_openapi::api::batch::v1::Job;
use k8s_openapi::api::batch::v1::CronJob;
use k8s_openapi::api::core::v1::{
    ConfigMap, Namespace, Node, Pod, Secret, Service, ServiceAccount, PersistentVolumeClaim,
};
use k8s_openapi::api::networking::v1::Ingress;
use kube::config::{KubeConfigOptions, Kubeconfig};
use kube::{Api, Client, Config};

use crate::app::ResourceRow;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResourceType {
    Pods,
    Deployments,
    Services,
    Nodes,
    Namespaces,
    ConfigMaps,
    Secrets,
    Ingresses,
    DaemonSets,
    StatefulSets,
    ReplicaSets,
    Jobs,
    CronJobs,
    ServiceAccounts,
    PersistentVolumeClaims,
}

impl ResourceType {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Pods => "Pods",
            Self::Deployments => "Deployments",
            Self::Services => "Services",
            Self::Nodes => "Nodes",
            Self::Namespaces => "Namespaces",
            Self::ConfigMaps => "ConfigMaps",
            Self::Secrets => "Secrets",
            Self::Ingresses => "Ingresses",
            Self::DaemonSets => "DaemonSets",
            Self::StatefulSets => "StatefulSets",
            Self::ReplicaSets => "ReplicaSets",
            Self::Jobs => "Jobs",
            Self::CronJobs => "CronJobs",
            Self::ServiceAccounts => "ServiceAccounts",
            Self::PersistentVolumeClaims => "PVCs",
        }
    }

    pub fn columns(&self) -> Vec<&'static str> {
        match self {
            Self::Pods => vec!["NAME", "NAMESPACE", "STATUS", "READY", "RESTARTS", "AGE"],
            Self::Deployments => vec!["NAME", "NAMESPACE", "READY", "STATUS", "AGE"],
            Self::Services => vec!["NAME", "NAMESPACE", "TYPE", "CLUSTER-IP", "AGE"],
            Self::Nodes => vec!["NAME", "STATUS", "ROLES", "VERSION", "AGE"],
            Self::Namespaces => vec!["NAME", "STATUS", "AGE"],
            Self::ConfigMaps => vec!["NAME", "NAMESPACE", "DATA", "AGE"],
            Self::Secrets => vec!["NAME", "NAMESPACE", "TYPE", "DATA", "AGE"],
            Self::Ingresses => vec!["NAME", "NAMESPACE", "HOSTS", "AGE"],
            Self::DaemonSets => vec!["NAME", "NAMESPACE", "DESIRED", "READY", "AGE"],
            Self::StatefulSets => vec!["NAME", "NAMESPACE", "READY", "AGE"],
            Self::ReplicaSets => vec!["NAME", "NAMESPACE", "DESIRED", "READY", "AGE"],
            Self::Jobs => vec!["NAME", "NAMESPACE", "COMPLETIONS", "AGE"],
            Self::CronJobs => vec!["NAME", "NAMESPACE", "SCHEDULE", "AGE"],
            Self::ServiceAccounts => vec!["NAME", "NAMESPACE", "SECRETS", "AGE"],
            Self::PersistentVolumeClaims => vec!["NAME", "NAMESPACE", "STATUS", "CAPACITY", "AGE"],
        }
    }

    pub fn from_command(cmd: &str) -> Option<Self> {
        match cmd.to_lowercase().as_str() {
            "po" | "pod" | "pods" => Some(Self::Pods),
            "deploy" | "deployment" | "deployments" => Some(Self::Deployments),
            "svc" | "service" | "services" => Some(Self::Services),
            "no" | "node" | "nodes" => Some(Self::Nodes),
            "ns" | "namespace" | "namespaces" => Some(Self::Namespaces),
            "cm" | "configmap" | "configmaps" => Some(Self::ConfigMaps),
            "secret" | "secrets" => Some(Self::Secrets),
            "ing" | "ingress" | "ingresses" => Some(Self::Ingresses),
            "ds" | "daemonset" | "daemonsets" => Some(Self::DaemonSets),
            "sts" | "statefulset" | "statefulsets" => Some(Self::StatefulSets),
            "rs" | "replicaset" | "replicasets" => Some(Self::ReplicaSets),
            "job" | "jobs" => Some(Self::Jobs),
            "cj" | "cronjob" | "cronjobs" => Some(Self::CronJobs),
            "sa" | "serviceaccount" | "serviceaccounts" => Some(Self::ServiceAccounts),
            "pvc" | "persistentvolumeclaim" | "persistentvolumeclaims" => {
                Some(Self::PersistentVolumeClaims)
            }
            _ => None,
        }
    }

    /// Whether this resource is namespaced
    pub fn is_namespaced(&self) -> bool {
        !matches!(self, Self::Nodes | Self::Namespaces)
    }
}

fn age_string(creation: Option<&k8s_openapi::apimachinery::pkg::apis::meta::v1::Time>) -> String {
    let Some(ts) = creation else {
        return "-".to_string();
    };
    let now = chrono::Utc::now();
    let created = chrono::DateTime::<chrono::Utc>::from_timestamp(
        ts.0.as_second(),
        ts.0.subsec_nanosecond().max(0) as u32,
    )
    .unwrap_or(now);
    let dur = now.signed_duration_since(created);

    if dur.num_days() > 0 {
        format!("{}d", dur.num_days())
    } else if dur.num_hours() > 0 {
        format!("{}h", dur.num_hours())
    } else if dur.num_minutes() > 0 {
        format!("{}m", dur.num_minutes())
    } else {
        format!("{}s", dur.num_seconds().max(0))
    }
}

pub async fn create_client(context: Option<&str>) -> anyhow::Result<Client> {
    if let Some(ctx_name) = context {
        let kubeconfig = Kubeconfig::read()?;
        let options = KubeConfigOptions {
            context: Some(ctx_name.to_string()),
            ..Default::default()
        };
        let config = Config::from_custom_kubeconfig(kubeconfig, &options).await?;
        Ok(Client::try_from(config)?)
    } else {
        Ok(Client::try_default().await?)
    }
}

pub fn get_all_contexts() -> anyhow::Result<(Vec<String>, String)> {
    let kubeconfig = Kubeconfig::read()?;
    let contexts: Vec<String> = kubeconfig.contexts.iter().map(|c| c.name.clone()).collect();
    let current = kubeconfig.current_context.unwrap_or_default();
    Ok((contexts, current))
}

pub fn set_context(name: &str) -> anyhow::Result<()> {
    let mut kubeconfig = Kubeconfig::read()?;
    kubeconfig.current_context = Some(name.to_string());
    let path = std::env::var("KUBECONFIG").unwrap_or_else(|_| {
        let home = std::env::var("HOME").unwrap_or_default();
        format!("{}/.kube/config", home)
    });
    let yaml = serde_yaml::to_string(&kubeconfig)?;
    std::fs::write(&path, yaml)?;
    Ok(())
}

pub async fn fetch_namespaces(client: &Client) -> anyhow::Result<Vec<String>> {
    let api: Api<Namespace> = Api::all(client.clone());
    let list = api.list(&Default::default()).await?;
    Ok(list
        .items
        .iter()
        .filter_map(|n| n.metadata.name.clone())
        .collect())
}

pub async fn fetch_resources(
    client: &Client,
    resource_type: ResourceType,
    namespace: &str,
) -> anyhow::Result<Vec<ResourceRow>> {
    match resource_type {
        ResourceType::Pods => fetch_pods(client, namespace).await,
        ResourceType::Deployments => fetch_deployments(client, namespace).await,
        ResourceType::Services => fetch_services(client, namespace).await,
        ResourceType::Nodes => fetch_nodes(client).await,
        ResourceType::Namespaces => fetch_namespace_rows(client).await,
        ResourceType::ConfigMaps => fetch_configmaps(client, namespace).await,
        ResourceType::Secrets => fetch_secrets(client, namespace).await,
        ResourceType::Ingresses => fetch_ingresses(client, namespace).await,
        ResourceType::DaemonSets => fetch_daemonsets(client, namespace).await,
        ResourceType::StatefulSets => fetch_statefulsets(client, namespace).await,
        ResourceType::ReplicaSets => fetch_replicasets(client, namespace).await,
        ResourceType::Jobs => fetch_jobs(client, namespace).await,
        ResourceType::CronJobs => fetch_cronjobs(client, namespace).await,
        ResourceType::ServiceAccounts => fetch_serviceaccounts(client, namespace).await,
        ResourceType::PersistentVolumeClaims => fetch_pvcs(client, namespace).await,
    }
}

fn api_for<K: kube::Resource<Scope = k8s_openapi::NamespaceResourceScope>>(
    client: &Client,
    namespace: &str,
) -> Api<K>
where
    K: k8s_openapi::serde::de::DeserializeOwned
        + Clone
        + std::fmt::Debug
        + kube::Resource<DynamicType = ()>,
{
    if namespace.is_empty() || namespace == "<all>" {
        Api::all(client.clone())
    } else {
        Api::namespaced(client.clone(), namespace)
    }
}

async fn fetch_pods(client: &Client, namespace: &str) -> anyhow::Result<Vec<ResourceRow>> {
    let api: Api<Pod> = api_for(client, namespace);
    let list = api.list(&Default::default()).await?;
    Ok(list
        .items
        .into_iter()
        .map(|pod| {
            let meta = &pod.metadata;
            let status = pod.status.as_ref();
            let phase = status
                .and_then(|s| s.phase.clone())
                .unwrap_or_else(|| "Unknown".into());

            let container_statuses = status
                .map(|s| s.container_statuses.clone().unwrap_or_default())
                .unwrap_or_default();
            let total = container_statuses.len();
            let ready_count = container_statuses.iter().filter(|c| c.ready).count();
            let restarts: i32 = container_statuses.iter().map(|c| c.restart_count).sum();

            ResourceRow {
                name: meta.name.clone().unwrap_or_default(),
                namespace: meta.namespace.clone().unwrap_or_default(),
                status: phase,
                ready: format!("{}/{}", ready_count, total),
                restarts: restarts.to_string(),
                age: age_string(meta.creation_timestamp.as_ref()),
                extra: HashMap::new(),
            }
        })
        .collect())
}

async fn fetch_deployments(client: &Client, namespace: &str) -> anyhow::Result<Vec<ResourceRow>> {
    let api: Api<Deployment> = api_for(client, namespace);
    let list = api.list(&Default::default()).await?;
    Ok(list
        .items
        .into_iter()
        .map(|d| {
            let meta = &d.metadata;
            let status = d.status.as_ref();
            let ready = status.and_then(|s| s.ready_replicas).unwrap_or(0);
            let desired = d.spec.as_ref().and_then(|s| s.replicas).unwrap_or(0);
            let available = status.and_then(|s| s.available_replicas).unwrap_or(0);
            let st = if available == desired {
                "Available"
            } else {
                "Progressing"
            };
            ResourceRow {
                name: meta.name.clone().unwrap_or_default(),
                namespace: meta.namespace.clone().unwrap_or_default(),
                status: st.to_string(),
                ready: format!("{}/{}", ready, desired),
                restarts: String::new(),
                age: age_string(meta.creation_timestamp.as_ref()),
                extra: HashMap::new(),
            }
        })
        .collect())
}

async fn fetch_services(client: &Client, namespace: &str) -> anyhow::Result<Vec<ResourceRow>> {
    let api: Api<Service> = api_for(client, namespace);
    let list = api.list(&Default::default()).await?;
    Ok(list
        .items
        .into_iter()
        .map(|s| {
            let meta = &s.metadata;
            let spec = s.spec.as_ref();
            let svc_type = spec
                .and_then(|sp| sp.type_.clone())
                .unwrap_or_else(|| "ClusterIP".into());
            let cluster_ip = spec
                .and_then(|sp| sp.cluster_ip.clone())
                .unwrap_or_else(|| "-".into());
            let mut extra = HashMap::new();
            extra.insert("type".into(), svc_type);
            extra.insert("cluster_ip".into(), cluster_ip);
            ResourceRow {
                name: meta.name.clone().unwrap_or_default(),
                namespace: meta.namespace.clone().unwrap_or_default(),
                status: String::new(),
                ready: String::new(),
                restarts: String::new(),
                age: age_string(meta.creation_timestamp.as_ref()),
                extra,
            }
        })
        .collect())
}

async fn fetch_nodes(client: &Client) -> anyhow::Result<Vec<ResourceRow>> {
    let api: Api<Node> = Api::all(client.clone());
    let list = api.list(&Default::default()).await?;
    Ok(list
        .items
        .into_iter()
        .map(|n| {
            let meta = &n.metadata;
            let conditions = n
                .status
                .as_ref()
                .and_then(|s| s.conditions.clone())
                .unwrap_or_default();
            let ready = conditions
                .iter()
                .find(|c| c.type_ == "Ready")
                .map(|c| {
                    if c.status == "True" {
                        "Ready"
                    } else {
                        "NotReady"
                    }
                })
                .unwrap_or("Unknown");
            let labels = meta.labels.clone().unwrap_or_default();
            let roles = labels
                .keys()
                .filter(|k| k.starts_with("node-role.kubernetes.io/"))
                .map(|k| k.trim_start_matches("node-role.kubernetes.io/"))
                .collect::<Vec<_>>()
                .join(",");
            let roles = if roles.is_empty() {
                "<none>".to_string()
            } else {
                roles
            };
            let version = n
                .status
                .as_ref()
                .and_then(|s| s.node_info.as_ref())
                .map(|i| i.kubelet_version.clone())
                .unwrap_or_default();
            let mut extra = HashMap::new();
            extra.insert("roles".into(), roles);
            extra.insert("version".into(), version);
            ResourceRow {
                name: meta.name.clone().unwrap_or_default(),
                namespace: String::new(),
                status: ready.to_string(),
                ready: String::new(),
                restarts: String::new(),
                age: age_string(meta.creation_timestamp.as_ref()),
                extra,
            }
        })
        .collect())
}

async fn fetch_namespace_rows(client: &Client) -> anyhow::Result<Vec<ResourceRow>> {
    let api: Api<Namespace> = Api::all(client.clone());
    let list = api.list(&Default::default()).await?;
    Ok(list
        .items
        .into_iter()
        .map(|n| {
            let meta = &n.metadata;
            let phase = n
                .status
                .as_ref()
                .and_then(|s| s.phase.clone())
                .unwrap_or_else(|| "Active".into());
            ResourceRow {
                name: meta.name.clone().unwrap_or_default(),
                namespace: String::new(),
                status: phase,
                ready: String::new(),
                restarts: String::new(),
                age: age_string(meta.creation_timestamp.as_ref()),
                extra: HashMap::new(),
            }
        })
        .collect())
}

async fn fetch_configmaps(client: &Client, namespace: &str) -> anyhow::Result<Vec<ResourceRow>> {
    let api: Api<ConfigMap> = api_for(client, namespace);
    let list = api.list(&Default::default()).await?;
    Ok(list
        .items
        .into_iter()
        .map(|cm| {
            let meta = &cm.metadata;
            let data_count = cm.data.as_ref().map(|d| d.len()).unwrap_or(0);
            let mut extra = HashMap::new();
            extra.insert("data".into(), data_count.to_string());
            ResourceRow {
                name: meta.name.clone().unwrap_or_default(),
                namespace: meta.namespace.clone().unwrap_or_default(),
                status: String::new(),
                ready: String::new(),
                restarts: String::new(),
                age: age_string(meta.creation_timestamp.as_ref()),
                extra,
            }
        })
        .collect())
}

async fn fetch_secrets(client: &Client, namespace: &str) -> anyhow::Result<Vec<ResourceRow>> {
    let api: Api<Secret> = api_for(client, namespace);
    let list = api.list(&Default::default()).await?;
    Ok(list
        .items
        .into_iter()
        .map(|s| {
            let meta = &s.metadata;
            let secret_type = s.type_.clone().unwrap_or_else(|| "Opaque".into());
            let data_count = s.data.as_ref().map(|d| d.len()).unwrap_or(0);
            let mut extra = HashMap::new();
            extra.insert("type".into(), secret_type);
            extra.insert("data".into(), data_count.to_string());
            ResourceRow {
                name: meta.name.clone().unwrap_or_default(),
                namespace: meta.namespace.clone().unwrap_or_default(),
                status: String::new(),
                ready: String::new(),
                restarts: String::new(),
                age: age_string(meta.creation_timestamp.as_ref()),
                extra,
            }
        })
        .collect())
}

async fn fetch_ingresses(client: &Client, namespace: &str) -> anyhow::Result<Vec<ResourceRow>> {
    let api: Api<Ingress> = api_for(client, namespace);
    let list = api.list(&Default::default()).await?;
    Ok(list
        .items
        .into_iter()
        .map(|ing| {
            let meta = &ing.metadata;
            let hosts = ing
                .spec
                .as_ref()
                .and_then(|s| s.rules.as_ref())
                .map(|rules| {
                    rules
                        .iter()
                        .filter_map(|r| r.host.clone())
                        .collect::<Vec<_>>()
                        .join(",")
                })
                .unwrap_or_else(|| "-".into());
            let mut extra = HashMap::new();
            extra.insert("hosts".into(), hosts);
            ResourceRow {
                name: meta.name.clone().unwrap_or_default(),
                namespace: meta.namespace.clone().unwrap_or_default(),
                status: String::new(),
                ready: String::new(),
                restarts: String::new(),
                age: age_string(meta.creation_timestamp.as_ref()),
                extra,
            }
        })
        .collect())
}

async fn fetch_daemonsets(client: &Client, namespace: &str) -> anyhow::Result<Vec<ResourceRow>> {
    let api: Api<DaemonSet> = api_for(client, namespace);
    let list = api.list(&Default::default()).await?;
    Ok(list
        .items
        .into_iter()
        .map(|ds| {
            let meta = &ds.metadata;
            let status = ds.status.as_ref();
            let desired = status.map(|s| s.desired_number_scheduled).unwrap_or(0);
            let ready = status.map(|s| s.number_ready).unwrap_or(0);
            let mut extra = HashMap::new();
            extra.insert("desired".into(), desired.to_string());
            ResourceRow {
                name: meta.name.clone().unwrap_or_default(),
                namespace: meta.namespace.clone().unwrap_or_default(),
                status: String::new(),
                ready: format!("{}/{}", ready, desired),
                restarts: String::new(),
                age: age_string(meta.creation_timestamp.as_ref()),
                extra,
            }
        })
        .collect())
}

async fn fetch_statefulsets(
    client: &Client,
    namespace: &str,
) -> anyhow::Result<Vec<ResourceRow>> {
    let api: Api<StatefulSet> = api_for(client, namespace);
    let list = api.list(&Default::default()).await?;
    Ok(list
        .items
        .into_iter()
        .map(|sts| {
            let meta = &sts.metadata;
            let status = sts.status.as_ref();
            let ready = status.and_then(|s| s.ready_replicas).unwrap_or(0);
            let desired = sts.spec.as_ref().and_then(|s| s.replicas).unwrap_or(0);
            ResourceRow {
                name: meta.name.clone().unwrap_or_default(),
                namespace: meta.namespace.clone().unwrap_or_default(),
                status: String::new(),
                ready: format!("{}/{}", ready, desired),
                restarts: String::new(),
                age: age_string(meta.creation_timestamp.as_ref()),
                extra: HashMap::new(),
            }
        })
        .collect())
}

async fn fetch_replicasets(client: &Client, namespace: &str) -> anyhow::Result<Vec<ResourceRow>> {
    let api: Api<ReplicaSet> = api_for(client, namespace);
    let list = api.list(&Default::default()).await?;
    Ok(list
        .items
        .into_iter()
        .map(|rs| {
            let meta = &rs.metadata;
            let status = rs.status.as_ref();
            let ready = status.and_then(|s| s.ready_replicas).unwrap_or(0);
            let desired = rs.spec.as_ref().and_then(|s| s.replicas).unwrap_or(0);
            let mut extra = HashMap::new();
            extra.insert("desired".into(), desired.to_string());
            ResourceRow {
                name: meta.name.clone().unwrap_or_default(),
                namespace: meta.namespace.clone().unwrap_or_default(),
                status: String::new(),
                ready: format!("{}/{}", ready, desired),
                restarts: String::new(),
                age: age_string(meta.creation_timestamp.as_ref()),
                extra,
            }
        })
        .collect())
}

async fn fetch_jobs(client: &Client, namespace: &str) -> anyhow::Result<Vec<ResourceRow>> {
    let api: Api<Job> = api_for(client, namespace);
    let list = api.list(&Default::default()).await?;
    Ok(list
        .items
        .into_iter()
        .map(|j| {
            let meta = &j.metadata;
            let status = j.status.as_ref();
            let succeeded = status.and_then(|s| s.succeeded).unwrap_or(0);
            let completions = j.spec.as_ref().and_then(|s| s.completions).unwrap_or(1);
            let mut extra = HashMap::new();
            extra.insert("completions".into(), format!("{}/{}", succeeded, completions));
            ResourceRow {
                name: meta.name.clone().unwrap_or_default(),
                namespace: meta.namespace.clone().unwrap_or_default(),
                status: String::new(),
                ready: String::new(),
                restarts: String::new(),
                age: age_string(meta.creation_timestamp.as_ref()),
                extra,
            }
        })
        .collect())
}

async fn fetch_cronjobs(client: &Client, namespace: &str) -> anyhow::Result<Vec<ResourceRow>> {
    let api: Api<CronJob> = api_for(client, namespace);
    let list = api.list(&Default::default()).await?;
    Ok(list
        .items
        .into_iter()
        .map(|cj| {
            let meta = &cj.metadata;
            let schedule = cj
                .spec
                .as_ref()
                .map(|s| s.schedule.clone())
                .unwrap_or_else(|| "-".into());
            let mut extra = HashMap::new();
            extra.insert("schedule".into(), schedule);
            ResourceRow {
                name: meta.name.clone().unwrap_or_default(),
                namespace: meta.namespace.clone().unwrap_or_default(),
                status: String::new(),
                ready: String::new(),
                restarts: String::new(),
                age: age_string(meta.creation_timestamp.as_ref()),
                extra,
            }
        })
        .collect())
}

async fn fetch_serviceaccounts(
    client: &Client,
    namespace: &str,
) -> anyhow::Result<Vec<ResourceRow>> {
    let api: Api<ServiceAccount> = api_for(client, namespace);
    let list = api.list(&Default::default()).await?;
    Ok(list
        .items
        .into_iter()
        .map(|sa| {
            let meta = &sa.metadata;
            let secrets_count = sa.secrets.as_ref().map(|s| s.len()).unwrap_or(0);
            let mut extra = HashMap::new();
            extra.insert("secrets".into(), secrets_count.to_string());
            ResourceRow {
                name: meta.name.clone().unwrap_or_default(),
                namespace: meta.namespace.clone().unwrap_or_default(),
                status: String::new(),
                ready: String::new(),
                restarts: String::new(),
                age: age_string(meta.creation_timestamp.as_ref()),
                extra,
            }
        })
        .collect())
}

async fn fetch_pvcs(client: &Client, namespace: &str) -> anyhow::Result<Vec<ResourceRow>> {
    let api: Api<PersistentVolumeClaim> = api_for(client, namespace);
    let list = api.list(&Default::default()).await?;
    Ok(list
        .items
        .into_iter()
        .map(|pvc| {
            let meta = &pvc.metadata;
            let phase = pvc
                .status
                .as_ref()
                .and_then(|s| s.phase.clone())
                .unwrap_or_else(|| "Pending".into());
            let capacity = pvc
                .status
                .as_ref()
                .and_then(|s| s.capacity.as_ref())
                .and_then(|c| c.get("storage"))
                .map(|q| q.0.clone())
                .unwrap_or_else(|| "-".into());
            let mut extra = HashMap::new();
            extra.insert("capacity".into(), capacity);
            ResourceRow {
                name: meta.name.clone().unwrap_or_default(),
                namespace: meta.namespace.clone().unwrap_or_default(),
                status: phase,
                ready: String::new(),
                restarts: String::new(),
                age: age_string(meta.creation_timestamp.as_ref()),
                extra,
            }
        })
        .collect())
}

pub async fn get_resource_yaml(
    client: &Client,
    resource_type: ResourceType,
    namespace: &str,
    name: &str,
) -> anyhow::Result<String> {
    // Use dynamic API to get raw JSON then convert to YAML
    let api: Api<kube::core::DynamicObject> = {
        let (resource, caps) = match resource_type {
            ResourceType::Pods => (
                kube::api::ApiResource::erase::<Pod>(&()),
                true,
            ),
            ResourceType::Deployments => (
                kube::api::ApiResource::erase::<Deployment>(&()),
                true,
            ),
            ResourceType::Services => (
                kube::api::ApiResource::erase::<Service>(&()),
                true,
            ),
            ResourceType::Nodes => (
                kube::api::ApiResource::erase::<Node>(&()),
                false,
            ),
            ResourceType::Namespaces => (
                kube::api::ApiResource::erase::<Namespace>(&()),
                false,
            ),
            ResourceType::ConfigMaps => (
                kube::api::ApiResource::erase::<ConfigMap>(&()),
                true,
            ),
            ResourceType::Secrets => (
                kube::api::ApiResource::erase::<Secret>(&()),
                true,
            ),
            ResourceType::Ingresses => (
                kube::api::ApiResource::erase::<Ingress>(&()),
                true,
            ),
            ResourceType::DaemonSets => (
                kube::api::ApiResource::erase::<DaemonSet>(&()),
                true,
            ),
            ResourceType::StatefulSets => (
                kube::api::ApiResource::erase::<StatefulSet>(&()),
                true,
            ),
            ResourceType::ReplicaSets => (
                kube::api::ApiResource::erase::<ReplicaSet>(&()),
                true,
            ),
            ResourceType::Jobs => (
                kube::api::ApiResource::erase::<Job>(&()),
                true,
            ),
            ResourceType::CronJobs => (
                kube::api::ApiResource::erase::<CronJob>(&()),
                true,
            ),
            ResourceType::ServiceAccounts => (
                kube::api::ApiResource::erase::<ServiceAccount>(&()),
                true,
            ),
            ResourceType::PersistentVolumeClaims => (
                kube::api::ApiResource::erase::<PersistentVolumeClaim>(&()),
                true,
            ),
        };
        if caps && !namespace.is_empty() && namespace != "<all>" {
            Api::namespaced_with(client.clone(), namespace, &resource)
        } else {
            Api::all_with(client.clone(), &resource)
        }
    };

    let obj = api.get(name).await?;
    Ok(serde_yaml::to_string(&obj)?)
}

pub async fn get_pod_logs(
    client: &Client,
    namespace: &str,
    name: &str,
    tail_lines: i64,
) -> anyhow::Result<Vec<String>> {
    let ns = if namespace.is_empty() { "default" } else { namespace };
    let api: Api<Pod> = Api::namespaced(client.clone(), ns);
    let mut params = kube::api::LogParams::default();
    params.tail_lines = Some(tail_lines);
    let logs = api.logs(name, &params).await?;
    Ok(logs.lines().map(|l| l.to_string()).collect())
}

pub async fn stream_pod_logs(
    client: &Client,
    namespace: &str,
    name: &str,
    tail_lines: i64,
) -> anyhow::Result<Vec<String>> {
    // For now, use non-streaming logs (follow requires async reader integration)
    get_pod_logs(client, namespace, name, tail_lines).await
}
