//! JWT-based service account authentication for Google Workspace
//!
//! Implements domain-wide delegation using service account credentials.

use crate::operator::{OperatorError, Result};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::Serialize;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

use super::types::{ServiceAccountKey, TokenResponse};

/// JWT claims for service account authentication
#[derive(Debug, Serialize)]
struct JwtClaims {
    /// Issuer (service account email)
    iss: String,
    /// Subject (admin email to impersonate)
    sub: String,
    /// Scope (space-separated OAuth scopes)
    scope: String,
    /// Audience (token endpoint)
    aud: String,
    /// Issued at (Unix timestamp)
    iat: u64,
    /// Expiration (Unix timestamp)
    exp: u64,
}

/// Cached access token
#[derive(Clone, Debug)]
struct CachedToken {
    access_token: String,
    expires_at: SystemTime,
}

/// Service account authentication with token caching
pub struct ServiceAccountAuth {
    /// Service account email
    service_account_email: String,
    /// Private key for JWT signing
    private_key: EncodingKey,
    /// Admin email to impersonate (domain-wide delegation)
    admin_email: String,
    /// OAuth scopes
    scopes: Vec<String>,
    /// Token endpoint URL
    token_uri: String,
    /// Cached access token
    cached_token: RwLock<Option<CachedToken>>,
    /// HTTP client for token exchange
    http_client: reqwest::Client,
}

impl ServiceAccountAuth {
    /// Create a new service account auth from key JSON
    pub fn new(
        key: &ServiceAccountKey,
        admin_email: String,
        scopes: Vec<String>,
    ) -> Result<Self> {
        let private_key = EncodingKey::from_rsa_pem(key.private_key.as_bytes())
            .map_err(|e| OperatorError::Config(format!("Invalid private key: {}", e)))?;

        Ok(Self {
            service_account_email: key.client_email.clone(),
            private_key,
            admin_email,
            scopes,
            token_uri: key.token_uri.clone(),
            cached_token: RwLock::new(None),
            http_client: reqwest::Client::new(),
        })
    }

    /// Get a valid access token (from cache or by refreshing)
    pub async fn get_token(&self) -> Result<String> {
        // Check if we have a valid cached token
        {
            let cached = self.cached_token.read().await;
            if let Some(token) = cached.as_ref() {
                // Add a 60-second buffer before expiry
                let buffer = Duration::from_secs(60);
                if token.expires_at > SystemTime::now() + buffer {
                    return Ok(token.access_token.clone());
                }
            }
        }

        // Need to refresh token
        let new_token = self.refresh_token().await?;

        // Cache the new token
        {
            let mut cached = self.cached_token.write().await;
            *cached = Some(new_token.clone());
        }

        Ok(new_token.access_token)
    }

    /// Refresh the access token using JWT assertion
    async fn refresh_token(&self) -> Result<CachedToken> {
        let jwt = self.create_jwt_assertion()?;
        let token_response = self.exchange_jwt_for_token(&jwt).await?;

        let expires_at = SystemTime::now() + Duration::from_secs(token_response.expires_in);

        Ok(CachedToken {
            access_token: token_response.access_token,
            expires_at,
        })
    }

    /// Create a signed JWT assertion
    fn create_jwt_assertion(&self) -> Result<String> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| OperatorError::Config(format!("System time error: {}", e)))?
            .as_secs();

        let claims = JwtClaims {
            iss: self.service_account_email.clone(),
            sub: self.admin_email.clone(),
            scope: self.scopes.join(" "),
            aud: self.token_uri.clone(),
            iat: now,
            exp: now + 3600, // 1-hour validity
        };

        let header = Header::new(Algorithm::RS256);

        encode(&header, &claims, &self.private_key)
            .map_err(|e| OperatorError::Config(format!("JWT encoding failed: {}", e)))
    }

    /// Exchange JWT assertion for access token
    async fn exchange_jwt_for_token(&self, jwt: &str) -> Result<TokenResponse> {
        let params = [
            ("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer"),
            ("assertion", jwt),
        ];

        let response = self
            .http_client
            .post(&self.token_uri)
            .form(&params)
            .send()
            .await
            .map_err(|e| OperatorError::Config(format!("Token request failed: {}", e)))?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            return Err(OperatorError::Config(format!(
                "Token exchange failed: {}",
                error_text
            )));
        }

        response
            .json::<TokenResponse>()
            .await
            .map_err(|e| OperatorError::Config(format!("Failed to parse token response: {}", e)))
    }

    /// Get the admin email being impersonated
    pub fn admin_email(&self) -> &str {
        &self.admin_email
    }

    /// Get the service account email
    pub fn service_account_email(&self) -> &str {
        &self.service_account_email
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_claims_serialization() {
        let claims = JwtClaims {
            iss: "sa@project.iam.gserviceaccount.com".to_string(),
            sub: "admin@company.com".to_string(),
            scope: "https://www.googleapis.com/auth/admin.directory.user.readonly".to_string(),
            aud: "https://oauth2.googleapis.com/token".to_string(),
            iat: 1234567890,
            exp: 1234571490,
        };

        let json = serde_json::to_string(&claims).unwrap();
        assert!(json.contains("iss"));
        assert!(json.contains("sub"));
        assert!(json.contains("scope"));
    }
}
