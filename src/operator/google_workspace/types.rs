//! Google Workspace Directory API response types

use serde::{Deserialize, Serialize};

/// User from Google Workspace Directory API
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryUser {
    /// Unique Google ID
    pub id: String,

    /// Primary email address
    pub primary_email: String,

    /// User name information
    pub name: DirectoryUserName,

    /// Organizational unit path
    #[serde(skip_serializing_if = "Option::is_none")]
    pub org_unit_path: Option<String>,

    /// Whether the user is suspended
    #[serde(default)]
    pub suspended: bool,

    /// Whether the user is an admin
    #[serde(default)]
    pub is_admin: bool,

    /// Whether the user is delegated admin
    #[serde(default)]
    pub is_delegated_admin: bool,

    /// Custom schemas (custom attributes)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub custom_schemas: Option<serde_json::Value>,

    /// Creation time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub creation_time: Option<String>,

    /// Last login time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_login_time: Option<String>,
}

/// User name information
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryUserName {
    /// Full name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub full_name: Option<String>,

    /// Given (first) name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub given_name: Option<String>,

    /// Family (last) name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub family_name: Option<String>,
}

/// Response from listing users
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsersListResponse {
    /// List of users
    #[serde(default)]
    pub users: Vec<DirectoryUser>,

    /// Next page token for pagination
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}

/// Group from Google Workspace Directory API
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DirectoryGroup {
    /// Unique Google ID
    pub id: String,

    /// Group email address
    pub email: String,

    /// Group name
    pub name: String,

    /// Group description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,

    /// Direct members count
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direct_members_count: Option<String>,

    /// Admin created (vs user-created)
    #[serde(default)]
    pub admin_created: bool,
}

/// Response from listing groups
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupsListResponse {
    /// List of groups
    #[serde(default)]
    pub groups: Vec<DirectoryGroup>,

    /// Next page token for pagination
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}

/// Group member from Directory API
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupMember {
    /// Member email
    pub email: String,

    /// Member role (OWNER, MANAGER, MEMBER)
    pub role: String,

    /// Member type (USER, GROUP, CUSTOMER)
    #[serde(rename = "type")]
    pub member_type: String,

    /// Member ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

/// Response from listing group members
#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MembersListResponse {
    /// List of members
    #[serde(default)]
    pub members: Vec<GroupMember>,

    /// Next page token for pagination
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_page_token: Option<String>,
}

/// Service account key JSON structure
#[derive(Clone, Debug, Deserialize)]
pub struct ServiceAccountKey {
    /// Type (should be "service_account")
    #[serde(rename = "type")]
    pub key_type: String,

    /// Project ID
    pub project_id: String,

    /// Private key ID
    pub private_key_id: String,

    /// Private key (PEM format)
    pub private_key: String,

    /// Client email (service account email)
    pub client_email: String,

    /// Client ID
    pub client_id: String,

    /// Auth URI
    pub auth_uri: String,

    /// Token URI
    pub token_uri: String,
}

/// Token response from Google OAuth2
#[derive(Clone, Debug, Deserialize)]
pub struct TokenResponse {
    /// Access token
    pub access_token: String,

    /// Token type (usually "Bearer")
    pub token_type: String,

    /// Expires in (seconds)
    pub expires_in: u64,
}

/// Google API error response
#[derive(Clone, Debug, Deserialize)]
pub struct GoogleApiError {
    pub error: GoogleApiErrorDetails,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GoogleApiErrorDetails {
    pub code: u16,
    pub message: String,
    #[serde(default)]
    pub errors: Vec<GoogleApiErrorItem>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct GoogleApiErrorItem {
    pub domain: String,
    pub reason: String,
    pub message: String,
}
