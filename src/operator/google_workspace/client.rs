//! Google Workspace Directory API client
//!
//! Provides methods for listing and managing users and groups.

use crate::operator::{OperatorError, Result};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

use super::auth::ServiceAccountAuth;
use super::types::*;

/// Base URL for Google Admin Directory API
const DIRECTORY_API_BASE: &str = "https://admin.googleapis.com/admin/directory/v1";

/// Google Workspace Directory API client
pub struct GoogleWorkspaceClient {
    /// HTTP client
    http_client: reqwest::Client,
    /// Service account authentication
    auth: Arc<RwLock<ServiceAccountAuth>>,
    /// Google Workspace domain
    domain: String,
    /// Customer ID
    customer_id: String,
}

impl GoogleWorkspaceClient {
    /// Create a new client from service account key
    pub fn new(
        key: &ServiceAccountKey,
        admin_email: String,
        domain: String,
        customer_id: String,
        scopes: Vec<String>,
    ) -> Result<Self> {
        let auth = ServiceAccountAuth::new(key, admin_email, scopes)?;

        Ok(Self {
            http_client: reqwest::Client::new(),
            auth: Arc::new(RwLock::new(auth)),
            domain,
            customer_id,
        })
    }

    /// Get an access token for API calls
    async fn get_token(&self) -> Result<String> {
        let auth = self.auth.read().await;
        auth.get_token().await
    }

    /// List all users in the domain with pagination
    pub async fn list_users(&self) -> Result<Vec<DirectoryUser>> {
        let mut all_users = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let mut url = format!(
                "{}/users?domain={}&maxResults=500",
                DIRECTORY_API_BASE, self.domain
            );

            if let Some(token) = &page_token {
                url.push_str(&format!("&pageToken={}", token));
            }

            let token = self.get_token().await?;
            let response = self
                .http_client
                .get(&url)
                .bearer_auth(&token)
                .send()
                .await
                .map_err(|e| OperatorError::Config(format!("Failed to list users: {}", e)))?;

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                return Err(OperatorError::Config(format!(
                    "Failed to list users ({}): {}",
                    status, error_text
                )));
            }

            let list_response: UsersListResponse = response
                .json()
                .await
                .map_err(|e| OperatorError::Config(format!("Failed to parse users: {}", e)))?;

            all_users.extend(list_response.users);

            match list_response.next_page_token {
                Some(token) => page_token = Some(token),
                None => break,
            }
        }

        info!("Listed {} users from Google Workspace", all_users.len());
        Ok(all_users)
    }

    /// Get a specific user by email
    pub async fn get_user(&self, email: &str) -> Result<Option<DirectoryUser>> {
        let url = format!("{}/users/{}", DIRECTORY_API_BASE, email);

        let token = self.get_token().await?;
        let response = self
            .http_client
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| OperatorError::Config(format!("Failed to get user: {}", e)))?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(OperatorError::Config(format!(
                "Failed to get user ({}): {}",
                status, error_text
            )));
        }

        let user: DirectoryUser = response
            .json()
            .await
            .map_err(|e| OperatorError::Config(format!("Failed to parse user: {}", e)))?;

        Ok(Some(user))
    }

    /// List all groups in the domain with pagination
    pub async fn list_groups(&self) -> Result<Vec<DirectoryGroup>> {
        let mut all_groups = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let mut url = format!(
                "{}/groups?domain={}&maxResults=200",
                DIRECTORY_API_BASE, self.domain
            );

            if let Some(token) = &page_token {
                url.push_str(&format!("&pageToken={}", token));
            }

            let token = self.get_token().await?;
            let response = self
                .http_client
                .get(&url)
                .bearer_auth(&token)
                .send()
                .await
                .map_err(|e| OperatorError::Config(format!("Failed to list groups: {}", e)))?;

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                return Err(OperatorError::Config(format!(
                    "Failed to list groups ({}): {}",
                    status, error_text
                )));
            }

            let list_response: GroupsListResponse = response
                .json()
                .await
                .map_err(|e| OperatorError::Config(format!("Failed to parse groups: {}", e)))?;

            all_groups.extend(list_response.groups);

            match list_response.next_page_token {
                Some(token) => page_token = Some(token),
                None => break,
            }
        }

        info!("Listed {} groups from Google Workspace", all_groups.len());
        Ok(all_groups)
    }

    /// Get members of a specific group
    pub async fn get_group_members(&self, group_key: &str) -> Result<Vec<GroupMember>> {
        let mut all_members = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let mut url = format!(
                "{}/groups/{}/members?maxResults=200",
                DIRECTORY_API_BASE, group_key
            );

            if let Some(token) = &page_token {
                url.push_str(&format!("&pageToken={}", token));
            }

            let token = self.get_token().await?;
            let response = self
                .http_client
                .get(&url)
                .bearer_auth(&token)
                .send()
                .await
                .map_err(|e| {
                    OperatorError::Config(format!("Failed to list group members: {}", e))
                })?;

            if response.status() == reqwest::StatusCode::NOT_FOUND {
                warn!("Group not found: {}", group_key);
                return Ok(Vec::new());
            }

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                return Err(OperatorError::Config(format!(
                    "Failed to list group members ({}): {}",
                    status, error_text
                )));
            }

            let list_response: MembersListResponse = response
                .json()
                .await
                .map_err(|e| OperatorError::Config(format!("Failed to parse members: {}", e)))?;

            all_members.extend(list_response.members);

            match list_response.next_page_token {
                Some(token) => page_token = Some(token),
                None => break,
            }
        }

        debug!(
            "Listed {} members for group {}",
            all_members.len(),
            group_key
        );
        Ok(all_members)
    }

    /// Check if a user is a member of a specific group
    pub async fn is_member_of_group(&self, user_email: &str, group_email: &str) -> Result<bool> {
        let url = format!(
            "{}/groups/{}/hasMember/{}",
            DIRECTORY_API_BASE, group_email, user_email
        );

        let token = self.get_token().await?;
        let response = self
            .http_client
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| OperatorError::Config(format!("Failed to check membership: {}", e)))?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(false);
        }

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(OperatorError::Config(format!(
                "Failed to check membership ({}): {}",
                status, error_text
            )));
        }

        #[derive(serde::Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct HasMemberResponse {
            is_member: bool,
        }

        let result: HasMemberResponse = response
            .json()
            .await
            .map_err(|e| OperatorError::Config(format!("Failed to parse membership: {}", e)))?;

        Ok(result.is_member)
    }

    /// Get all groups a user belongs to
    pub async fn get_user_groups(&self, user_email: &str) -> Result<Vec<DirectoryGroup>> {
        let mut all_groups = Vec::new();
        let mut page_token: Option<String> = None;

        loop {
            let mut url = format!(
                "{}/groups?userKey={}&maxResults=200",
                DIRECTORY_API_BASE, user_email
            );

            if let Some(token) = &page_token {
                url.push_str(&format!("&pageToken={}", token));
            }

            let token = self.get_token().await?;
            let response = self
                .http_client
                .get(&url)
                .bearer_auth(&token)
                .send()
                .await
                .map_err(|e| OperatorError::Config(format!("Failed to get user groups: {}", e)))?;

            if !response.status().is_success() {
                let status = response.status();
                let error_text = response.text().await.unwrap_or_default();
                return Err(OperatorError::Config(format!(
                    "Failed to get user groups ({}): {}",
                    status, error_text
                )));
            }

            let list_response: GroupsListResponse = response
                .json()
                .await
                .map_err(|e| OperatorError::Config(format!("Failed to parse groups: {}", e)))?;

            all_groups.extend(list_response.groups);

            match list_response.next_page_token {
                Some(token) => page_token = Some(token),
                None => break,
            }
        }

        Ok(all_groups)
    }

    /// Get the domain this client is configured for
    pub fn domain(&self) -> &str {
        &self.domain
    }

    /// Get the customer ID
    pub fn customer_id(&self) -> &str {
        &self.customer_id
    }
}
