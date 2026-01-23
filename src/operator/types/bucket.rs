use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Multi-cloud Bucket CRD - abstracts AWS S3, GCP Cloud Storage, and Azure Blob Storage
#[derive(CustomResource, Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[kube(
    group = "platform.yurikrupnik.com",
    version = "v1alpha1",
    kind = "Bucket",
    namespaced,
    status = "BucketStatus",
    shortname = "bkt",
    printcolumn = r#"{"name":"Phase", "type":"string", "jsonPath":".status.phase"}"#,
    printcolumn = r#"{"name":"Provider", "type":"string", "jsonPath":".spec.provider"}"#,
    printcolumn = r#"{"name":"Location", "type":"string", "jsonPath":".spec.parameters.location"}"#,
    printcolumn = r#"{"name":"Ready", "type":"boolean", "jsonPath":".status.ready"}"#,
    printcolumn = r#"{"name":"Age", "type":"date", "jsonPath":".metadata.creationTimestamp"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct BucketSpec {
    /// Cloud provider: aws, gcp, or azure
    pub provider: CloudProvider,

    /// Provider-agnostic bucket parameters
    pub parameters: BucketParameters,

    /// Access control configuration (IAM, policies)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub access_control: Option<BucketAccessControl>,

    /// Provider configuration reference (for Crossplane)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_config_ref: Option<BucketProviderConfigRef>,

    /// Where to write connection details (bucket endpoint, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub write_connection_secret_to_ref: Option<BucketConnectionSecretRef>,

    /// Deletion policy: Delete or Orphan the cloud resource
    #[serde(default)]
    pub deletion_policy: BucketDeletionPolicy,

    /// Labels to apply to the bucket
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<BTreeMap<String, String>>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum CloudProvider {
    #[default]
    Gcp,
    Aws,
    Azure,
}

/// Provider-agnostic bucket parameters
/// Maps to forProvider fields for each cloud provider:
/// - GCP: storage.gcp.upbound.io/Bucket (single resource with all config)
/// - AWS: s3.aws.upbound.io/Bucket + companion resources (BucketVersioning, BucketLifecycleConfiguration, etc.)
/// - Azure: storage.azure.upbound.io/Account + Container
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BucketParameters {
    // ============ Location & Storage Class ============

    /// Bucket location/region (REQUIRED)
    /// - GCP: location (e.g., "US", "EU", "us-east1", "europe-west1")
    /// - AWS: region (e.g., "us-east-1", "eu-west-1")
    /// - Azure: location (e.g., "eastus", "westeurope")
    pub location: String,

    /// Storage class/tier
    /// Mapping:
    /// | Unified          | GCP        | AWS                  | Azure   |
    /// |------------------|------------|----------------------|---------|
    /// | Standard         | STANDARD   | STANDARD             | Hot     |
    /// | InfrequentAccess | NEARLINE   | STANDARD_IA          | Cool    |
    /// | Archive          | ARCHIVE    | GLACIER              | Archive |
    /// | ColdArchive      | COLDLINE   | DEEP_ARCHIVE         | Archive |
    /// | Intelligent      | STANDARD   | INTELLIGENT_TIERING  | Hot     |
    #[serde(skip_serializing_if = "Option::is_none")]
    pub storage_class: Option<StorageClass>,

    // ============ Versioning & Object Lock ============

    /// Enable versioning for objects
    /// - GCP: versioning.enabled
    /// - AWS: BucketVersioning resource
    /// - Azure: blobProperties.versioningEnabled
    #[serde(default)]
    pub versioning: bool,

    /// Enable object lock (immutable objects) - AWS only
    /// - AWS: objectLockEnabled (must be set at bucket creation)
    /// - GCP: Use retentionPolicy instead
    /// - Azure: Use immutabilityPolicy instead
    #[serde(default)]
    pub object_lock_enabled: bool,

    // ============ Access Control ============

    /// Public access prevention/block
    /// - GCP: publicAccessPrevention ("enforced" or "inherited")
    /// - AWS: BucketPublicAccessBlock resource
    /// - Azure: allowNestedItemsToBePublic (inverted)
    #[serde(default = "default_true")]
    pub public_access_prevention: bool,

    /// Uniform bucket-level access (GCP only, recommended)
    /// - GCP: uniformBucketLevelAccess
    #[serde(default = "default_true")]
    pub uniform_bucket_level_access: bool,

    // ============ Lifecycle Management ============

    /// Lifecycle rules for automatic object management
    /// - GCP: lifecycleRule array
    /// - AWS: BucketLifecycleConfiguration resource
    /// - Azure: blobProperties with management policies
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lifecycle_rules: Vec<LifecycleRule>,

    // ============ CORS ============

    /// Cross-Origin Resource Sharing configuration
    /// - GCP: cors array (maxAgeSeconds, method, origin, responseHeader)
    /// - AWS: BucketCorsConfiguration resource
    /// - Azure: blobProperties.corsRule array
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cors: Option<Vec<CorsRule>>,

    // ============ Encryption ============

    /// Server-side encryption configuration
    /// - GCP: encryption.defaultKmsKeyName
    /// - AWS: BucketServerSideEncryptionConfiguration resource
    /// - Azure: customerManagedKey or infrastructureEncryptionEnabled
    #[serde(skip_serializing_if = "Option::is_none")]
    pub encryption: Option<EncryptionConfig>,

    // ============ Logging ============

    /// Access logging configuration
    /// - GCP: logging (logBucket, logObjectPrefix)
    /// - AWS: BucketLogging resource
    /// - Azure: Use Azure Monitor / Storage Analytics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub logging: Option<LoggingConfig>,

    // ============ Replication ============

    /// Cross-region replication configuration
    /// - GCP: Not directly supported (use Transfer Service)
    /// - AWS: BucketReplicationConfiguration resource
    /// - Azure: accountReplicationType (GRS, RAGRS, GZRS)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub replication: Option<ReplicationConfig>,

    // ============ Website Hosting ============

    /// Static website hosting configuration
    /// - GCP: website (mainPageSuffix, notFoundPage)
    /// - AWS: BucketWebsiteConfiguration resource
    /// - Azure: staticWebsite (indexDocument, error404Document)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub website: Option<WebsiteConfig>,

    // ============ Event Notifications ============

    /// Event notification configuration
    /// - GCP: Pub/Sub notifications (separate resource)
    /// - AWS: BucketNotification resource (SNS, SQS, Lambda)
    /// - Azure: Event Grid (separate resource)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notifications: Option<Vec<NotificationConfig>>,

    // ============ Tags/Labels ============

    /// Tags/Labels to apply to the bucket resource
    /// - GCP: labels
    /// - AWS: tags
    /// - Azure: tags
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<BTreeMap<String, String>>,

    // ============ Retention & Soft Delete ============

    /// Retention policy (object lock duration)
    /// - GCP: retentionPolicy (retentionPeriod in seconds, isLocked)
    /// - AWS: ObjectLockConfiguration resource
    /// - Azure: immutabilityPolicy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retention_policy: Option<RetentionPolicy>,

    /// Soft delete policy (recover deleted objects)
    /// - GCP: softDeletePolicy (retentionDurationSeconds)
    /// - AWS: Not directly supported (use versioning + lifecycle)
    /// - Azure: blobProperties.deleteRetentionPolicy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub soft_delete: Option<SoftDeletePolicy>,

    // ============ Advanced Options ============

    /// Force destroy (delete all objects when bucket is deleted)
    /// - GCP: forceDestroy
    /// - AWS: forceDestroy
    /// - Azure: N/A (handled by deletion policy)
    #[serde(default)]
    pub force_destroy: bool,

    /// Requester pays (requester pays for data transfer)
    /// - GCP: requesterPays
    /// - AWS: BucketRequestPaymentConfiguration resource
    /// - Azure: N/A
    #[serde(default)]
    pub requester_pays: bool,

    // ============ Azure-Specific (mapped from unified params) ============

    /// Azure: Account tier (Standard or Premium)
    /// Only used when provider is Azure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub azure_account_tier: Option<AzureAccountTier>,

    /// Azure: Account replication type
    /// Only used when provider is Azure
    #[serde(skip_serializing_if = "Option::is_none")]
    pub azure_replication_type: Option<AzureReplicationType>,

    /// Azure: Resource group name (required for Azure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub azure_resource_group: Option<String>,

    /// Azure: Enable HTTPS traffic only
    #[serde(default = "default_true")]
    pub azure_https_only: bool,

    /// Azure: Minimum TLS version
    #[serde(skip_serializing_if = "Option::is_none")]
    pub azure_min_tls_version: Option<String>,

    // ============ GCP-Specific ============

    /// GCP: Project ID (if different from provider default)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gcp_project: Option<String>,

    /// GCP: Enable autoclass (automatic storage class management)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gcp_autoclass: Option<GcpAutoclass>,

    /// GCP: Recovery Point Objective for turbo replication
    /// Values: "DEFAULT" or "ASYNC_TURBO"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gcp_rpo: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RetentionPolicy {
    /// Retention period in days
    pub retention_days: i32,
    /// Lock the retention policy (cannot be shortened once locked)
    #[serde(default)]
    pub locked: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SoftDeletePolicy {
    /// Retention period for soft-deleted objects in days
    pub retention_days: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
pub enum AzureAccountTier {
    Standard,
    Premium,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum AzureReplicationType {
    /// Locally redundant storage
    Lrs,
    /// Geo-redundant storage
    Grs,
    /// Read-access geo-redundant storage
    Ragrs,
    /// Zone-redundant storage
    Zrs,
    /// Geo-zone-redundant storage
    Gzrs,
    /// Read-access geo-zone-redundant storage
    Ragzrs,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GcpAutoclass {
    /// Enable autoclass
    pub enabled: bool,
    /// Terminal storage class (NEARLINE or ARCHIVE)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub terminal_storage_class: Option<String>,
}

// ============ Associated Resources ============

/// Access control and IAM configuration
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BucketAccessControl {
    /// IAM bindings for the bucket
    /// Maps to:
    /// - GCP: storage.BucketIAMBinding / BucketIAMMember
    /// - AWS: s3.BucketPolicy
    /// - Azure: roleAssignment
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub iam_bindings: Vec<IamBinding>,

    /// Bucket policy (provider-specific JSON policy)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bucket_policy: Option<BucketPolicy>,

    /// Access control list (legacy, prefer IAM)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub acl: Option<BucketAcl>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IamBinding {
    /// Role to grant
    /// Mapped values:
    /// - GCP: roles/storage.objectViewer, roles/storage.objectAdmin, etc.
    /// - AWS: arn:aws:iam::aws:policy/AmazonS3ReadOnlyAccess, etc.
    /// - Azure: Storage Blob Data Reader, Storage Blob Data Contributor, etc.
    pub role: BucketRole,

    /// Members to grant the role to
    /// Format varies by provider:
    /// - GCP: user:email, serviceAccount:email, group:email
    /// - AWS: arn:aws:iam::ACCOUNT:user/NAME, arn:aws:iam::ACCOUNT:role/NAME
    /// - Azure: principalId (GUID)
    pub members: Vec<String>,

    /// Condition for the binding (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub condition: Option<IamCondition>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum BucketRole {
    /// Read objects
    ObjectViewer,
    /// Read and write objects
    ObjectAdmin,
    /// Full bucket admin (includes bucket-level permissions)
    BucketAdmin,
    /// Create objects only (write, no read/delete)
    ObjectCreator,
    /// Custom role - use rawRole field
    Custom,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IamCondition {
    /// Condition title
    pub title: String,
    /// Condition description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Condition expression (CEL for GCP, IAM policy condition for AWS)
    pub expression: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BucketPolicy {
    /// Raw policy document (JSON)
    /// For complex policies that don't fit the abstracted model
    pub policy_document: serde_json::Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum BucketAcl {
    Private,
    PublicRead,
    PublicReadWrite,
    AuthenticatedRead,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WebsiteConfig {
    /// Index document (e.g., "index.html")
    pub index_document: String,
    /// Error document (e.g., "404.html")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_document: Option<String>,
    /// Redirect all requests to another host
    #[serde(skip_serializing_if = "Option::is_none")]
    pub redirect_all_requests_to: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NotificationConfig {
    /// Notification ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// Events to trigger on
    /// Mapped values:
    /// - GCP: OBJECT_FINALIZE, OBJECT_DELETE, OBJECT_ARCHIVE, OBJECT_METADATA_UPDATE
    /// - AWS: s3:ObjectCreated:*, s3:ObjectRemoved:*, etc.
    /// - Azure: Microsoft.Storage.BlobCreated, Microsoft.Storage.BlobDeleted
    pub events: Vec<BucketEvent>,

    /// Destination for notifications
    pub destination: NotificationDestination,

    /// Prefix filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,

    /// Suffix filter
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suffix: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum BucketEvent {
    ObjectCreated,
    ObjectDeleted,
    ObjectArchived,
    ObjectMetadataUpdated,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NotificationDestination {
    /// Destination type
    pub destination_type: NotificationDestinationType,
    /// Topic/Queue/Function ARN or name
    pub target: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum NotificationDestinationType {
    /// Pub/Sub topic (GCP) / SNS topic (AWS) / Event Grid topic (Azure)
    Topic,
    /// Cloud Function (GCP) / Lambda (AWS) / Azure Function
    Function,
    /// SQS Queue (AWS only)
    Queue,
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StorageClass {
    /// Standard/Hot storage (default)
    /// - GCP: STANDARD, AWS: STANDARD, Azure: Hot
    Standard,
    /// Infrequent access (30+ day retention)
    /// - GCP: NEARLINE, AWS: STANDARD_IA, Azure: Cool
    InfrequentAccess,
    /// Archive storage (90+ day retention)
    /// - GCP: ARCHIVE, AWS: GLACIER, Azure: Archive
    Archive,
    /// Cold archive (180+ day retention, slower retrieval)
    /// - GCP: COLDLINE, AWS: DEEP_ARCHIVE, Azure: Archive
    ColdArchive,
    /// Intelligent tiering (auto-transition based on access patterns)
    /// - GCP: N/A (use autoclass), AWS: INTELLIGENT_TIERING, Azure: N/A
    Intelligent,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleRule {
    /// Rule name/ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,

    /// Enable this rule
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Prefix filter for objects
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,

    /// Tag filters
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tags: Option<BTreeMap<String, String>>,

    /// Transition to different storage class after N days
    #[serde(skip_serializing_if = "Option::is_none")]
    pub transition: Option<TransitionAction>,

    /// Delete objects after N days
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expiration: Option<ExpirationAction>,

    /// Delete non-current versions after N days
    #[serde(skip_serializing_if = "Option::is_none")]
    pub noncurrent_version_expiration: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TransitionAction {
    /// Days after creation to transition
    pub days: i32,
    /// Target storage class
    pub storage_class: StorageClass,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExpirationAction {
    /// Days after creation to delete
    pub days: i32,
    /// Also delete expired object delete markers
    #[serde(default)]
    pub expired_object_delete_marker: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CorsRule {
    /// Allowed origins
    pub allowed_origins: Vec<String>,
    /// Allowed methods
    pub allowed_methods: Vec<String>,
    /// Allowed headers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_headers: Option<Vec<String>>,
    /// Exposed headers
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expose_headers: Option<Vec<String>>,
    /// Max age in seconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_age_seconds: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EncryptionConfig {
    /// Encryption type
    pub encryption_type: EncryptionType,
    /// KMS key ID/name (for customer-managed keys)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kms_key_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum EncryptionType {
    /// Cloud provider managed encryption
    ProviderManaged,
    /// Customer managed KMS key
    CustomerManaged,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoggingConfig {
    /// Target bucket for access logs
    pub target_bucket: String,
    /// Prefix for log objects
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_prefix: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReplicationConfig {
    /// Destination bucket name
    pub destination_bucket: String,
    /// Destination region (for cross-region replication)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination_region: Option<String>,
    /// Prefix filter for replication
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prefix: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BucketProviderConfigRef {
    /// Name of the ProviderConfig
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BucketConnectionSecretRef {
    /// Name of the secret to create
    pub name: String,
    /// Namespace for the secret
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
pub enum BucketDeletionPolicy {
    #[default]
    Delete,
    Orphan,
}

// ============ Status ============

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BucketStatus {
    /// Current phase
    pub phase: BucketPhase,

    /// Whether the bucket is ready
    #[serde(default)]
    pub ready: bool,

    /// Synced with cloud provider
    #[serde(default)]
    pub synced: bool,

    /// Cloud provider bucket name (may differ from metadata.name)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bucket_name: Option<String>,

    /// Bucket endpoint/URL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,

    /// ARN (AWS) / Self-link (GCP) / Resource ID (Azure)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resource_id: Option<String>,

    /// Underlying Crossplane managed resource name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub managed_resource: Option<ManagedBucketResource>,

    /// Conditions
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<BucketCondition>,

    /// Observed generation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_generation: Option<i64>,

    /// Last reconciliation time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_reconcile_time: Option<String>,

    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
pub enum BucketPhase {
    #[default]
    Pending,
    Creating,
    Ready,
    Failed,
    Deleting,
    Updating,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ManagedBucketResource {
    /// API version of the managed resource
    pub api_version: String,
    /// Kind (e.g., Bucket for GCP, Bucket for AWS S3)
    pub kind: String,
    /// Name
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BucketCondition {
    #[serde(rename = "type")]
    pub condition_type: String,
    pub status: String,
    pub reason: String,
    pub message: String,
    pub last_transition_time: String,
}

// ============ Provider Mapping Helpers ============

impl BucketParameters {
    /// Map storage class to GCP storage class
    pub fn to_gcp_storage_class(&self) -> &str {
        match self.storage_class.as_ref().unwrap_or(&StorageClass::Standard) {
            StorageClass::Standard => "STANDARD",
            StorageClass::InfrequentAccess => "NEARLINE",
            StorageClass::Archive => "ARCHIVE",
            StorageClass::ColdArchive => "COLDLINE",
            StorageClass::Intelligent => "STANDARD", // GCP uses autoclass instead
        }
    }

    /// Map storage class to AWS S3 storage class
    pub fn to_aws_storage_class(&self) -> &str {
        match self.storage_class.as_ref().unwrap_or(&StorageClass::Standard) {
            StorageClass::Standard => "STANDARD",
            StorageClass::InfrequentAccess => "STANDARD_IA",
            StorageClass::Archive => "GLACIER",
            StorageClass::ColdArchive => "DEEP_ARCHIVE",
            StorageClass::Intelligent => "INTELLIGENT_TIERING",
        }
    }

    /// Map storage class to Azure access tier
    pub fn to_azure_access_tier(&self) -> &str {
        match self.storage_class.as_ref().unwrap_or(&StorageClass::Standard) {
            StorageClass::Standard => "Hot",
            StorageClass::InfrequentAccess => "Cool",
            StorageClass::Archive => "Archive",
            StorageClass::ColdArchive => "Archive", // Azure only has Archive tier
            StorageClass::Intelligent => "Hot", // Azure uses lifecycle policies
        }
    }

    /// Map location to GCP region format
    pub fn to_gcp_location(&self) -> &str {
        // GCP uses locations like "us-east1", "EU", "US"
        &self.location
    }

    /// Map location to AWS region format
    pub fn to_aws_region(&self) -> String {
        // Convert GCP-style to AWS-style if needed
        // e.g., "us-east1" -> "us-east-1"
        self.location.replace("east1", "east-1")
            .replace("west1", "west-1")
            .replace("central1", "central-1")
    }

    /// Map location to Azure location format
    pub fn to_azure_location(&self) -> String {
        // Convert to Azure-style: "us-east1" -> "eastus"
        self.location
            .replace("us-", "")
            .replace("europe-", "")
            .replace("-", "")
            .replace("1", "")
    }
}
