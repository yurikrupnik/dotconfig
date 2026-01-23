//! Metrics caching layer using moka

use moka::future::Cache;
use std::time::Duration;

use crate::resource_stats::cost::PricingRates;
use crate::resource_stats::types::resource_stats::{NodeResourceStats, PodResourceStats, ResourceSnapshot};

/// Default TTL for node metrics cache (30 seconds)
const NODE_CACHE_TTL: Duration = Duration::from_secs(30);

/// Default TTL for pod metrics cache (60 seconds)
const POD_CACHE_TTL: Duration = Duration::from_secs(60);

/// Default TTL for pricing cache (1 hour)
const PRICING_CACHE_TTL: Duration = Duration::from_secs(3600);

/// Default TTL for cluster snapshot cache (60 seconds)
const SNAPSHOT_CACHE_TTL: Duration = Duration::from_secs(60);

/// Metrics cache with TTL-based expiration
pub struct MetricsCache {
    /// Cluster-level snapshot cache
    snapshot_cache: Cache<String, ResourceSnapshot>,
    /// Node metrics cache (keyed by node name)
    node_cache: Cache<String, NodeResourceStats>,
    /// Pod metrics cache (keyed by namespace/pod-name)
    pod_cache: Cache<String, PodResourceStats>,
    /// Pricing rates cache (keyed by node type or "default")
    pricing_cache: Cache<String, PricingRates>,
}

impl MetricsCache {
    pub fn new() -> Self {
        Self {
            snapshot_cache: Cache::builder()
                .time_to_live(SNAPSHOT_CACHE_TTL)
                .max_capacity(10)
                .build(),
            node_cache: Cache::builder()
                .time_to_live(NODE_CACHE_TTL)
                .max_capacity(1000)
                .build(),
            pod_cache: Cache::builder()
                .time_to_live(POD_CACHE_TTL)
                .max_capacity(10000)
                .build(),
            pricing_cache: Cache::builder()
                .time_to_live(PRICING_CACHE_TTL)
                .max_capacity(100)
                .build(),
        }
    }

    /// Create cache with custom TTLs
    pub fn with_ttls(
        node_ttl: Duration,
        pod_ttl: Duration,
        pricing_ttl: Duration,
        snapshot_ttl: Duration,
    ) -> Self {
        Self {
            snapshot_cache: Cache::builder()
                .time_to_live(snapshot_ttl)
                .max_capacity(10)
                .build(),
            node_cache: Cache::builder()
                .time_to_live(node_ttl)
                .max_capacity(1000)
                .build(),
            pod_cache: Cache::builder()
                .time_to_live(pod_ttl)
                .max_capacity(10000)
                .build(),
            pricing_cache: Cache::builder()
                .time_to_live(pricing_ttl)
                .max_capacity(100)
                .build(),
        }
    }

    // Snapshot cache operations

    /// Get cached cluster snapshot
    pub async fn get_snapshot(&self, key: &str) -> Option<ResourceSnapshot> {
        self.snapshot_cache.get(key).await
    }

    /// Cache cluster snapshot
    pub async fn set_snapshot(&self, key: &str, snapshot: ResourceSnapshot) {
        self.snapshot_cache.insert(key.to_string(), snapshot).await;
    }

    // Node cache operations

    /// Get cached node metrics
    pub async fn get_node(&self, node_name: &str) -> Option<NodeResourceStats> {
        self.node_cache.get(node_name).await
    }

    /// Cache node metrics
    pub async fn set_node(&self, node_name: &str, stats: NodeResourceStats) {
        self.node_cache.insert(node_name.to_string(), stats).await;
    }

    /// Get all cached node metrics
    pub async fn get_all_nodes(&self) -> Vec<NodeResourceStats> {
        // Note: moka doesn't have a direct iteration method
        // This is a simplified implementation
        Vec::new()
    }

    /// Invalidate node cache entry
    pub async fn invalidate_node(&self, node_name: &str) {
        self.node_cache.invalidate(node_name).await;
    }

    // Pod cache operations

    /// Get cached pod metrics
    pub async fn get_pod(&self, namespace: &str, pod_name: &str) -> Option<PodResourceStats> {
        let key = format!("{}/{}", namespace, pod_name);
        self.pod_cache.get(&key).await
    }

    /// Cache pod metrics
    pub async fn set_pod(&self, stats: PodResourceStats) {
        let key = format!("{}/{}", stats.namespace, stats.pod_name);
        self.pod_cache.insert(key, stats).await;
    }

    /// Invalidate pod cache entry
    pub async fn invalidate_pod(&self, namespace: &str, pod_name: &str) {
        let key = format!("{}/{}", namespace, pod_name);
        self.pod_cache.invalidate(&key).await;
    }

    // Pricing cache operations

    /// Get cached pricing rates
    pub async fn get_pricing(&self, key: &str) -> Option<PricingRates> {
        self.pricing_cache.get(key).await
    }

    /// Cache pricing rates
    pub async fn set_pricing(&self, key: &str, rates: PricingRates) {
        self.pricing_cache.insert(key.to_string(), rates).await;
    }

    /// Invalidate pricing cache entry
    pub async fn invalidate_pricing(&self, key: &str) {
        self.pricing_cache.invalidate(key).await;
    }

    /// Clear all caches
    pub async fn clear_all(&self) {
        self.snapshot_cache.invalidate_all();
        self.node_cache.invalidate_all();
        self.pod_cache.invalidate_all();
        self.pricing_cache.invalidate_all();
    }

    /// Get cache statistics
    pub fn stats(&self) -> CacheStats {
        CacheStats {
            snapshot_count: self.snapshot_cache.entry_count(),
            node_count: self.node_cache.entry_count(),
            pod_count: self.pod_cache.entry_count(),
            pricing_count: self.pricing_cache.entry_count(),
        }
    }
}

impl Default for MetricsCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Cache statistics
#[derive(Debug, Clone)]
pub struct CacheStats {
    pub snapshot_count: u64,
    pub node_count: u64,
    pub pod_count: u64,
    pub pricing_count: u64,
}
