//! CRD type definitions for Resource Stats Operator

pub mod cost_config;
pub mod resource_stats;

pub use cost_config::{CostConfig, CostConfigSpec, CostConfigStatus};
pub use resource_stats::{ResourceStats, ResourceStatsSpec, ResourceStatsStatus};
