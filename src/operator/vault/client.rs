//! Vault HTTP Client
//!
//! Implements Vault authentication and credential fetching for
//! GCP, AWS, and Azure cloud providers.

use crate::operator::{OperatorError, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tracing::{debug, info, warn};

/// Vault client for authentication and secret retrieval
pub struct VaultClient {
    client: Client,
    address: String,
    token: Option<String>,
}

/// GCP credentials from Vault
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GcpCredentials {
    /// OAuth2 access token (for dynamic mode)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_token: Option<String>,
    /// Service account JSON key (for static mode)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service_account_key: Option<String>,
    /// Token expiry time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<String>,
}

/// AWS credentials from Vault
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwsCredentials {
    pub access_key: String,
    pub secret_key: String,
    /// STS session token (for dynamic mode)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub security_token: Option<String>,
    /// IAM role ARN
    #[serde(skip_serializing_if = "Option::is_none")]
    pub arn: Option<String>,
}

/// Azure credentials from Vault
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AzureCredentials {
    pub client_id: String,
    pub client_secret: String,
    pub tenant_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subscription_id: Option<String>,
}

/// Vault lease information for dynamic credentials
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultLease {
    pub lease_id: String,
    pub lease_duration: i64,
    pub renewable: bool,
}

/// Vault authentication response
#[derive(Debug, Deserialize)]
struct VaultAuthResponse {
    auth: VaultAuth,
}

#[derive(Debug, Deserialize)]
struct VaultAuth {
    client_token: String,
    #[allow(dead_code)]
    lease_duration: i64,
    #[allow(dead_code)]
    renewable: bool,
}

/// Vault secret response wrapper
#[derive(Debug, Deserialize)]
struct VaultSecretResponse {
    data: serde_json::Value,
    #[serde(default)]
    lease_id: String,
    #[serde(default)]
    lease_duration: i64,
    #[serde(default)]
    renewable: bool,
}

/// Vault KV v2 response wrapper
#[derive(Debug, Deserialize)]
struct VaultKvV2Response {
    data: VaultKvV2Data,
}

#[derive(Debug, Deserialize)]
struct VaultKvV2Data {
    data: serde_json::Value,
}

impl VaultClient {
    /// Create a new Vault client
    pub async fn new(address: &str, ca_cert: Option<&[u8]>) -> Result<Self> {
        let mut builder = Client::builder()
            .timeout(Duration::from_secs(30))
            .danger_accept_invalid_certs(false);

        if let Some(ca) = ca_cert {
            let cert = reqwest::Certificate::from_pem(ca)
                .map_err(|e| OperatorError::Config(format!("Invalid CA cert: {}", e)))?;
            builder = builder.add_root_certificate(cert);
        }

        let client = builder
            .build()
            .map_err(|e| OperatorError::Config(format!("Failed to create HTTP client: {}", e)))?;

        Ok(Self {
            client,
            address: address.trim_end_matches('/').to_string(),
            token: None,
        })
    }

    /// Set the Vault token directly
    pub fn set_token(&mut self, token: String) {
        self.token = Some(token);
    }

    /// Get the current token
    pub fn token(&self) -> Option<&str> {
        self.token.as_deref()
    }

    /// Authenticate using Kubernetes auth method
    pub async fn auth_kubernetes(
        &mut self,
        mount_path: &str,
        role: &str,
        jwt: &str,
    ) -> Result<()> {
        let url = format!("{}/v1/auth/{}/login", self.address, mount_path);

        info!("Authenticating to Vault using Kubernetes auth at {}", mount_path);

        let response = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "role": role,
                "jwt": jwt
            }))
            .send()
            .await
            .map_err(|e| OperatorError::GoogleWorkspace(format!("Vault request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(OperatorError::GoogleWorkspace(format!(
                "Vault Kubernetes auth failed: {} - {}",
                status, body
            )));
        }

        let auth_response: VaultAuthResponse = response
            .json()
            .await
            .map_err(|e| OperatorError::GoogleWorkspace(format!("Failed to parse Vault response: {}", e)))?;

        self.token = Some(auth_response.auth.client_token);
        info!("Successfully authenticated to Vault");

        Ok(())
    }

    /// Authenticate using AppRole auth method
    pub async fn auth_approle(
        &mut self,
        mount_path: &str,
        role_id: &str,
        secret_id: &str,
    ) -> Result<()> {
        let url = format!("{}/v1/auth/{}/login", self.address, mount_path);

        info!("Authenticating to Vault using AppRole at {}", mount_path);

        let response = self
            .client
            .post(&url)
            .json(&serde_json::json!({
                "role_id": role_id,
                "secret_id": secret_id
            }))
            .send()
            .await
            .map_err(|e| OperatorError::GoogleWorkspace(format!("Vault request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(OperatorError::GoogleWorkspace(format!(
                "Vault AppRole auth failed: {} - {}",
                status, body
            )));
        }

        let auth_response: VaultAuthResponse = response
            .json()
            .await
            .map_err(|e| OperatorError::GoogleWorkspace(format!("Failed to parse Vault response: {}", e)))?;

        self.token = Some(auth_response.auth.client_token);
        info!("Successfully authenticated to Vault using AppRole");

        Ok(())
    }

    /// Get GCP credentials from dynamic secrets engine
    pub async fn get_gcp_credentials(
        &self,
        secrets_path: &str,
        role: &str,
    ) -> Result<(GcpCredentials, Option<VaultLease>)> {
        let token = self.token.as_ref()
            .ok_or_else(|| OperatorError::Config("Vault token not set".into()))?;

        // GCP secrets engine uses /token/<role> endpoint for OAuth tokens
        let url = format!("{}/v1/{}/token/{}", self.address, secrets_path, role);

        debug!("Fetching GCP credentials from Vault: {}", url);

        let response = self
            .client
            .get(&url)
            .header("X-Vault-Token", token)
            .send()
            .await
            .map_err(|e| OperatorError::GoogleWorkspace(format!("Vault request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(OperatorError::GoogleWorkspace(format!(
                "Failed to get GCP credentials: {} - {}",
                status, body
            )));
        }

        let secret: VaultSecretResponse = response
            .json()
            .await
            .map_err(|e| OperatorError::GoogleWorkspace(format!("Failed to parse Vault response: {}", e)))?;

        let credentials = GcpCredentials {
            access_token: secret.data.get("token").and_then(|v| v.as_str()).map(String::from),
            service_account_key: None,
            expires_at: secret.data.get("expires_at_seconds")
                .and_then(|v| v.as_i64())
                .map(|ts| chrono::DateTime::from_timestamp(ts, 0)
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default()),
        };

        let lease = if !secret.lease_id.is_empty() {
            Some(VaultLease {
                lease_id: secret.lease_id,
                lease_duration: secret.lease_duration,
                renewable: secret.renewable,
            })
        } else {
            None
        };

        info!("Successfully fetched GCP credentials from Vault");
        Ok((credentials, lease))
    }

    /// Get AWS credentials from dynamic secrets engine
    pub async fn get_aws_credentials(
        &self,
        secrets_path: &str,
        role: &str,
    ) -> Result<(AwsCredentials, Option<VaultLease>)> {
        let token = self.token.as_ref()
            .ok_or_else(|| OperatorError::Config("Vault token not set".into()))?;

        // AWS secrets engine uses /creds/<role> endpoint
        let url = format!("{}/v1/{}/creds/{}", self.address, secrets_path, role);

        debug!("Fetching AWS credentials from Vault: {}", url);

        let response = self
            .client
            .get(&url)
            .header("X-Vault-Token", token)
            .send()
            .await
            .map_err(|e| OperatorError::GoogleWorkspace(format!("Vault request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(OperatorError::GoogleWorkspace(format!(
                "Failed to get AWS credentials: {} - {}",
                status, body
            )));
        }

        let secret: VaultSecretResponse = response
            .json()
            .await
            .map_err(|e| OperatorError::GoogleWorkspace(format!("Failed to parse Vault response: {}", e)))?;

        let credentials = AwsCredentials {
            access_key: secret.data.get("access_key")
                .and_then(|v| v.as_str())
                .ok_or_else(|| OperatorError::Config("Missing access_key in Vault response".into()))?
                .to_string(),
            secret_key: secret.data.get("secret_key")
                .and_then(|v| v.as_str())
                .ok_or_else(|| OperatorError::Config("Missing secret_key in Vault response".into()))?
                .to_string(),
            security_token: secret.data.get("security_token").and_then(|v| v.as_str()).map(String::from),
            arn: secret.data.get("arn").and_then(|v| v.as_str()).map(String::from),
        };

        let lease = if !secret.lease_id.is_empty() {
            Some(VaultLease {
                lease_id: secret.lease_id,
                lease_duration: secret.lease_duration,
                renewable: secret.renewable,
            })
        } else {
            None
        };

        info!("Successfully fetched AWS credentials from Vault");
        Ok((credentials, lease))
    }

    /// Get Azure credentials from dynamic secrets engine
    pub async fn get_azure_credentials(
        &self,
        secrets_path: &str,
        role: &str,
    ) -> Result<(AzureCredentials, Option<VaultLease>)> {
        let token = self.token.as_ref()
            .ok_or_else(|| OperatorError::Config("Vault token not set".into()))?;

        // Azure secrets engine uses /creds/<role> endpoint
        let url = format!("{}/v1/{}/creds/{}", self.address, secrets_path, role);

        debug!("Fetching Azure credentials from Vault: {}", url);

        let response = self
            .client
            .get(&url)
            .header("X-Vault-Token", token)
            .send()
            .await
            .map_err(|e| OperatorError::GoogleWorkspace(format!("Vault request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(OperatorError::GoogleWorkspace(format!(
                "Failed to get Azure credentials: {} - {}",
                status, body
            )));
        }

        let secret: VaultSecretResponse = response
            .json()
            .await
            .map_err(|e| OperatorError::GoogleWorkspace(format!("Failed to parse Vault response: {}", e)))?;

        let credentials = AzureCredentials {
            client_id: secret.data.get("client_id")
                .and_then(|v| v.as_str())
                .ok_or_else(|| OperatorError::Config("Missing client_id in Vault response".into()))?
                .to_string(),
            client_secret: secret.data.get("client_secret")
                .and_then(|v| v.as_str())
                .ok_or_else(|| OperatorError::Config("Missing client_secret in Vault response".into()))?
                .to_string(),
            tenant_id: String::new(), // Must be provided from cluster config
            subscription_id: None,
        };

        let lease = if !secret.lease_id.is_empty() {
            Some(VaultLease {
                lease_id: secret.lease_id,
                lease_duration: secret.lease_duration,
                renewable: secret.renewable,
            })
        } else {
            None
        };

        info!("Successfully fetched Azure credentials from Vault");
        Ok((credentials, lease))
    }

    /// Read static secrets from KV v1 store
    pub async fn read_kv_v1_secret(
        &self,
        mount_path: &str,
        secret_path: &str,
    ) -> Result<serde_json::Value> {
        let token = self.token.as_ref()
            .ok_or_else(|| OperatorError::Config("Vault token not set".into()))?;

        let url = format!("{}/v1/{}/{}", self.address, mount_path, secret_path);

        debug!("Reading KV v1 secret from Vault: {}", url);

        let response = self
            .client
            .get(&url)
            .header("X-Vault-Token", token)
            .send()
            .await
            .map_err(|e| OperatorError::GoogleWorkspace(format!("Vault request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(OperatorError::GoogleWorkspace(format!(
                "Failed to read KV v1 secret: {} - {}",
                status, body
            )));
        }

        let secret: VaultSecretResponse = response
            .json()
            .await
            .map_err(|e| OperatorError::GoogleWorkspace(format!("Failed to parse Vault response: {}", e)))?;

        Ok(secret.data)
    }

    /// Read static secrets from KV v2 store
    pub async fn read_kv_v2_secret(
        &self,
        mount_path: &str,
        secret_path: &str,
    ) -> Result<serde_json::Value> {
        let token = self.token.as_ref()
            .ok_or_else(|| OperatorError::Config("Vault token not set".into()))?;

        // KV v2 uses /data/ path
        let url = format!("{}/v1/{}/data/{}", self.address, mount_path, secret_path);

        debug!("Reading KV v2 secret from Vault: {}", url);

        let response = self
            .client
            .get(&url)
            .header("X-Vault-Token", token)
            .send()
            .await
            .map_err(|e| OperatorError::GoogleWorkspace(format!("Vault request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(OperatorError::GoogleWorkspace(format!(
                "Failed to read KV v2 secret: {} - {}",
                status, body
            )));
        }

        let secret: VaultKvV2Response = response
            .json()
            .await
            .map_err(|e| OperatorError::GoogleWorkspace(format!("Failed to parse Vault response: {}", e)))?;

        Ok(secret.data.data)
    }

    /// Read static secrets (auto-detect KV version)
    pub async fn read_kv_secret(
        &self,
        mount_path: &str,
        secret_path: &str,
        kv_version: &str,
    ) -> Result<serde_json::Value> {
        match kv_version {
            "v1" | "1" => self.read_kv_v1_secret(mount_path, secret_path).await,
            "v2" | "2" | _ => self.read_kv_v2_secret(mount_path, secret_path).await,
        }
    }

    /// Renew a lease
    pub async fn renew_lease(&self, lease_id: &str) -> Result<i64> {
        let token = self.token.as_ref()
            .ok_or_else(|| OperatorError::Config("Vault token not set".into()))?;

        let url = format!("{}/v1/sys/leases/renew", self.address);

        debug!("Renewing Vault lease: {}", lease_id);

        let response = self
            .client
            .post(&url)
            .header("X-Vault-Token", token)
            .json(&serde_json::json!({"lease_id": lease_id}))
            .send()
            .await
            .map_err(|e| OperatorError::GoogleWorkspace(format!("Vault request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            warn!("Failed to renew Vault lease: {} - {}", status, body);
            return Err(OperatorError::GoogleWorkspace(format!(
                "Failed to renew lease: {} - {}",
                status, body
            )));
        }

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| OperatorError::GoogleWorkspace(format!("Failed to parse Vault response: {}", e)))?;

        let new_duration = body.get("lease_duration")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        info!("Successfully renewed Vault lease, new duration: {}s", new_duration);
        Ok(new_duration)
    }

    /// Revoke a lease
    pub async fn revoke_lease(&self, lease_id: &str) -> Result<()> {
        let token = self.token.as_ref()
            .ok_or_else(|| OperatorError::Config("Vault token not set".into()))?;

        let url = format!("{}/v1/sys/leases/revoke", self.address);

        debug!("Revoking Vault lease: {}", lease_id);

        let response = self
            .client
            .post(&url)
            .header("X-Vault-Token", token)
            .json(&serde_json::json!({"lease_id": lease_id}))
            .send()
            .await
            .map_err(|e| OperatorError::GoogleWorkspace(format!("Vault request failed: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            warn!("Failed to revoke Vault lease: {} - {}", status, body);
            return Err(OperatorError::GoogleWorkspace(format!(
                "Failed to revoke lease: {} - {}",
                status, body
            )));
        }

        info!("Successfully revoked Vault lease");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gcp_credentials_serialization() {
        let creds = GcpCredentials {
            access_token: Some("test-token".into()),
            service_account_key: None,
            expires_at: Some("2024-01-01T00:00:00Z".into()),
        };
        let json = serde_json::to_string(&creds).unwrap();
        assert!(json.contains("test-token"));
    }

    #[test]
    fn test_aws_credentials_serialization() {
        let creds = AwsCredentials {
            access_key: "AKIA...".into(),
            secret_key: "secret".into(),
            security_token: Some("token".into()),
            arn: None,
        };
        let json = serde_json::to_string(&creds).unwrap();
        assert!(json.contains("AKIA"));
    }
}
