//! Vault Client Module
//!
//! Provides HashiCorp Vault integration for fetching cloud credentials
//! using dynamic secrets engines or static KV secrets.

pub mod client;

pub use client::*;
