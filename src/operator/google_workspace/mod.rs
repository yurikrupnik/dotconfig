//! Google Workspace Directory API integration
//!
//! This module provides:
//! - JWT-based service account authentication with domain-wide delegation
//! - Directory API client for users and groups
//! - Types for API responses

pub mod auth;
pub mod client;
pub mod types;

pub use auth::ServiceAccountAuth;
pub use client::GoogleWorkspaceClient;
pub use types::*;
