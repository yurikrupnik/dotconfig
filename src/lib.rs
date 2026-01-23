//! Dotconfig library
//!
//! This library provides the platform operator module for Kubernetes operations.

pub mod operator;
pub mod resource_stats;

#[cfg(feature = "tui")]
pub mod cluster_dashboard;
