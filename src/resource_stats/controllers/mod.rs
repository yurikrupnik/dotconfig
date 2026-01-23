//! Controllers for Resource Stats CRDs

pub mod cost_config;
pub mod resource_stats;

pub use cost_config::reconcile as reconcile_cost_config;
pub use cost_config::error_policy as cost_config_error_policy;
pub use resource_stats::reconcile as reconcile_resource_stats;
pub use resource_stats::error_policy as resource_stats_error_policy;
