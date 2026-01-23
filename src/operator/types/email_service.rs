//! EmailService CRD for provisioning production-ready AWS SES via Crossplane
//!
//! This CRD abstracts the complexity of setting up SES with:
//! - Domain verification with DKIM (required)
//! - MAIL FROM domain for SPF alignment
//! - Configuration sets for tracking and reputation
//! - Bounce/complaint notification handling
//! - IAM via IRSA (no static credentials)
//!
//! Security requirements enforced:
//! - DKIM must be enabled (non-negotiable)
//! - TLS required for all connections
//! - Suppression list enabled by default
//! - Alert thresholds below AWS suspension levels

use kube::CustomResource;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// EmailService CRD for provisioning production-ready AWS SES via Crossplane
#[derive(CustomResource, Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[kube(
    group = "platform.yurikrupnik.com",
    version = "v1alpha1",
    kind = "EmailService",
    namespaced,
    status = "EmailServiceStatus",
    shortname = "email",
    printcolumn = r#"{"name":"Phase", "type":"string", "jsonPath":".status.phase"}"#,
    printcolumn = r#"{"name":"Domain", "type":"string", "jsonPath":".spec.domain"}"#,
    printcolumn = r#"{"name":"Env", "type":"string", "jsonPath":".spec.environment"}"#,
    printcolumn = r#"{"name":"Verified", "type":"boolean", "jsonPath":".status.domainVerified"}"#,
    printcolumn = r#"{"name":"DKIM", "type":"boolean", "jsonPath":".status.dkimVerified"}"#,
    printcolumn = r#"{"name":"Security", "type":"string", "jsonPath":".status.securityStatus.overall"}"#,
    printcolumn = r#"{"name":"Age", "type":"date", "jsonPath":".metadata.creationTimestamp"}"#
)]
#[serde(rename_all = "camelCase")]
pub struct EmailServiceSpec {
    // ============ Core Identity ============

    /// Domain to verify for sending (must be subdomain, e.g., "mail.example.com")
    /// Security: Using a subdomain isolates email reputation from main domain
    pub domain: String,

    /// AWS region for SES resources
    pub region: String,

    /// Environment tier affecting defaults and security requirements
    #[serde(default)]
    pub environment: EmailEnvironment,

    // ============ Authentication (Security Critical) ============

    /// DKIM configuration - REQUIRED for email authentication
    /// Security: DKIM signing prevents email spoofing
    #[serde(default)]
    pub dkim: DkimConfig,

    /// Custom MAIL FROM domain for SPF alignment
    /// Security: Required for proper SPF alignment in production
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mail_from: Option<MailFromConfig>,

    // ============ IAM Configuration (IRSA Only) ============

    /// IAM configuration using IRSA (IAM Roles for Service Accounts)
    /// Security: Static credentials are not allowed
    pub iam: IamConfig,

    // ============ Tracking & Abuse Prevention ============

    /// Configuration set for tracking and reputation monitoring
    #[serde(default)]
    pub configuration_set: ConfigurationSetSpec,

    /// Event destinations for monitoring (CloudWatch, SNS, Kinesis)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub event_destinations: Vec<EventDestinationSpec>,

    /// Bounce/complaint notification configuration
    /// Security: Required for production to maintain sender reputation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notifications: Option<NotificationSpec>,

    /// Alert thresholds for reputation monitoring
    /// Security: Must be below AWS suspension levels
    #[serde(default)]
    pub alert_thresholds: AlertThresholds,

    // ============ Email Templates ============

    /// Email templates to create
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub templates: Vec<EmailTemplateSpec>,

    // ============ Advanced Features ============

    /// Dedicated IP pool (production high-volume senders only)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dedicated_ips: Option<DedicatedIpConfig>,

    /// Individual email identities (for dev/testing)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub email_identities: Vec<String>,

    // ============ Crossplane Configuration ============

    /// Crossplane ProviderConfig reference
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_config_ref: Option<ProviderConfigRef>,

    /// Secret to write connection details to
    #[serde(skip_serializing_if = "Option::is_none")]
    pub write_connection_secret_to_ref: Option<ConnectionSecretRef>,

    /// Deletion policy: Delete or Orphan AWS resources
    #[serde(default)]
    pub deletion_policy: DeletionPolicy,

    /// Labels to apply to all managed resources
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<BTreeMap<String, String>>,
}

// ============ Environment ============

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum EmailEnvironment {
    /// Development: Sandbox mode, relaxed monitoring
    #[default]
    Dev,
    /// Staging: Production-like with lower limits
    Staging,
    /// Production: Full security requirements enforced
    Prod,
}

// ============ DKIM Configuration ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DkimConfig {
    /// Enable DKIM signing - MUST be true (security requirement)
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Signing key length: minimum 2048 bits
    #[serde(default)]
    pub signing_key_length: DkimKeyLength,
}

impl Default for DkimConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            signing_key_length: DkimKeyLength::Rsa2048Bit,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
pub enum DkimKeyLength {
    #[serde(rename = "RSA_2048_BIT")]
    #[default]
    Rsa2048Bit,
    #[serde(rename = "RSA_4096_BIT")]
    Rsa4096Bit,
}

// ============ MAIL FROM Configuration ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MailFromConfig {
    /// Subdomain for MAIL FROM (e.g., "mail" creates mail.example.com)
    pub subdomain: String,

    /// Behavior when MX lookup fails
    /// Security: Must be REJECT_MESSAGE in production for SPF alignment
    #[serde(default)]
    pub behavior_on_mx_failure: MxFailureBehavior,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MxFailureBehavior {
    /// Reject the message (recommended for SPF alignment)
    #[default]
    RejectMessage,
    /// Use default SES domain (breaks SPF alignment)
    UseDefaultValue,
}

// ============ IAM Configuration (IRSA Only) ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IamConfig {
    /// Service account name for IRSA
    pub service_account_name: String,

    /// Service account namespace
    pub service_account_namespace: String,

    /// Sending permissions (least privilege)
    #[serde(default)]
    pub permissions: SendingPermissions,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SendingPermissions {
    /// Allow ses:SendEmail
    #[serde(default = "default_true")]
    pub allow_send_email: bool,

    /// Allow ses:SendTemplatedEmail
    #[serde(default = "default_true")]
    pub allow_templated_email: bool,

    /// Allow ses:SendRawEmail (for MIME construction)
    #[serde(default)]
    pub allow_raw_email: bool,

    /// Allow ses:SendBulkTemplatedEmail (requires explicit approval)
    #[serde(default)]
    pub allow_bulk_email: bool,

    /// Restrict sending to this identity only
    #[serde(default = "default_true")]
    pub restrict_to_identity: bool,

    /// Force use of configuration set
    #[serde(default = "default_true")]
    pub require_configuration_set: bool,
}

impl Default for SendingPermissions {
    fn default() -> Self {
        Self {
            allow_send_email: true,
            allow_templated_email: true,
            allow_raw_email: false,
            allow_bulk_email: false,
            restrict_to_identity: true,
            require_configuration_set: true,
        }
    }
}

// ============ Configuration Set ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfigurationSetSpec {
    /// Configuration set name (defaults to EmailService name)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Enable reputation metrics (required for production)
    #[serde(default = "default_true")]
    pub reputation_metrics_enabled: bool,

    /// Enable sending
    #[serde(default = "default_true")]
    pub sending_enabled: bool,

    /// TLS policy - must be REQUIRE (security requirement)
    #[serde(default)]
    pub tls_policy: TlsPolicy,

    /// Suppression list configuration
    #[serde(default)]
    pub suppression_list: SuppressionListConfig,
}

impl Default for ConfigurationSetSpec {
    fn default() -> Self {
        Self {
            name: None,
            reputation_metrics_enabled: true,
            sending_enabled: true,
            tls_policy: TlsPolicy::Require,
            suppression_list: SuppressionListConfig::default(),
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TlsPolicy {
    /// Require TLS (security requirement)
    #[default]
    Require,
    /// Optional TLS (not recommended)
    Optional,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SuppressionListConfig {
    /// Enable suppression list (required for production)
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Reasons to suppress
    #[serde(default = "default_suppression_reasons")]
    pub suppressed_reasons: Vec<SuppressedReason>,
}

impl Default for SuppressionListConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            suppressed_reasons: vec![SuppressedReason::Bounce, SuppressedReason::Complaint],
        }
    }
}

fn default_suppression_reasons() -> Vec<SuppressedReason> {
    vec![SuppressedReason::Bounce, SuppressedReason::Complaint]
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SuppressedReason {
    Bounce,
    Complaint,
}

// ============ Event Destinations ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EventDestinationSpec {
    /// Destination name
    pub name: String,

    /// Enable this destination
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Events to capture
    pub matching_event_types: Vec<SesEventType>,

    /// Destination configuration
    pub destination: EventDestinationType,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SesEventType {
    Send,
    Reject,
    Bounce,
    Complaint,
    Delivery,
    Open,
    Click,
    RenderingFailure,
    DeliveryDelay,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum EventDestinationType {
    /// CloudWatch metrics
    CloudWatch(CloudWatchDestination),
    /// SNS topic
    Sns(SnsDestination),
    /// Kinesis Firehose
    KinesisFirehose(KinesisFirehoseDestination),
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CloudWatchDestination {
    /// Dimension configurations for metrics
    pub dimension_configurations: Vec<CloudWatchDimension>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CloudWatchDimension {
    pub dimension_name: String,
    pub dimension_value_source: DimensionValueSource,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_dimension_value: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum DimensionValueSource {
    MessageTag,
    EmailHeader,
    LinkTag,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SnsDestination {
    /// Existing SNS topic ARN or name to create
    pub topic: String,
    /// Create the topic if it doesn't exist
    #[serde(default)]
    pub create_topic: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KinesisFirehoseDestination {
    /// Delivery stream ARN
    pub delivery_stream_arn: String,
    /// IAM role ARN for Kinesis access
    pub iam_role_arn: String,
}

// ============ Notifications ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NotificationSpec {
    /// Bounce notification topic
    pub bounce_topic: NotificationTopicSpec,

    /// Complaint notification topic
    pub complaint_topic: NotificationTopicSpec,

    /// Delivery notification topic (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery_topic: Option<NotificationTopicSpec>,

    /// Include headers in notifications
    #[serde(default)]
    pub include_headers: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NotificationTopicSpec {
    /// Existing SNS topic ARN
    #[serde(skip_serializing_if = "Option::is_none")]
    pub topic_arn: Option<String>,
    /// Create a new topic with this name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub create_topic_name: Option<String>,
}

// ============ Alert Thresholds ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AlertThresholds {
    /// Bounce rate warning threshold (AWS suspends at 5%)
    #[serde(default = "default_bounce_warning")]
    pub bounce_rate_warning: f64,

    /// Bounce rate critical threshold
    #[serde(default = "default_bounce_critical")]
    pub bounce_rate_critical: f64,

    /// Complaint rate warning threshold (AWS suspends at 0.1%)
    #[serde(default = "default_complaint_warning")]
    pub complaint_rate_warning: f64,

    /// Complaint rate critical threshold
    #[serde(default = "default_complaint_critical")]
    pub complaint_rate_critical: f64,
}

impl Default for AlertThresholds {
    fn default() -> Self {
        Self {
            bounce_rate_warning: 2.0,
            bounce_rate_critical: 4.0,
            complaint_rate_warning: 0.05,
            complaint_rate_critical: 0.08,
        }
    }
}

fn default_bounce_warning() -> f64 { 2.0 }
fn default_bounce_critical() -> f64 { 4.0 }
fn default_complaint_warning() -> f64 { 0.05 }
fn default_complaint_critical() -> f64 { 0.08 }

// ============ Email Templates ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EmailTemplateSpec {
    /// Template name
    pub name: String,
    /// Subject line (supports {{placeholders}})
    pub subject: String,
    /// HTML body
    #[serde(skip_serializing_if = "Option::is_none")]
    pub html_body: Option<String>,
    /// Plain text body
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text_body: Option<String>,
}

// ============ Dedicated IPs ============

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DedicatedIpConfig {
    /// IP pool name
    pub pool_name: String,
    /// Scaling mode
    #[serde(default)]
    pub scaling_mode: IpScalingMode,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IpScalingMode {
    #[default]
    Standard,
    Managed,
}

// ============ Crossplane References ============
// Note: ProviderConfigRef, ConnectionSecretRef, and DeletionPolicy are re-exported
// from crossplane_resource.rs to avoid duplication

pub use super::crossplane_resource::{ConnectionSecretRef, DeletionPolicy, ProviderConfigRef};

// ============ Status ============

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EmailServiceStatus {
    /// Current phase
    pub phase: EmailServicePhase,

    /// Domain verification status
    #[serde(default)]
    pub domain_verified: bool,

    /// DKIM verification status
    #[serde(default)]
    pub dkim_verified: bool,

    /// MAIL FROM verification status
    #[serde(default)]
    pub mail_from_verified: bool,

    /// Configuration set name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub configuration_set_name: Option<String>,

    /// Managed Crossplane resources
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub managed_resources: Vec<ManagedSesResource>,

    /// DNS records that need to be created
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub dns_records: Vec<DnsRecord>,

    /// Security status with warnings
    #[serde(default)]
    pub security_status: SecurityStatus,

    /// Sending quota information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sending_quota: Option<SendingQuota>,

    /// Reputation metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reputation: Option<ReputationMetrics>,

    /// Notification topic ARNs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification_topics: Option<NotificationTopics>,

    /// Conditions
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub conditions: Vec<EmailServiceCondition>,

    /// Observed generation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observed_generation: Option<i64>,

    /// Last reconcile time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_reconcile_time: Option<String>,

    /// Human-readable message
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
pub enum EmailServicePhase {
    #[default]
    Pending,
    Creating,
    AwaitingVerification,
    Ready,
    Failed,
    Blocked,
    Deleting,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ManagedSesResource {
    pub api_version: String,
    pub kind: String,
    pub name: String,
    pub ready: bool,
    #[serde(default)]
    pub synced: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DnsRecord {
    /// Record type (TXT, CNAME, MX)
    pub record_type: String,
    /// Record name
    pub name: String,
    /// Record value
    pub value: String,
    /// Purpose of the record
    pub purpose: DnsRecordPurpose,
    /// Whether the record is verified
    #[serde(default)]
    pub verified: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum DnsRecordPurpose {
    DomainVerification,
    Dkim,
    MailFrom,
    Spf,
    Dmarc,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecurityStatus {
    /// Overall security status
    #[serde(default)]
    pub overall: SecurityLevel,

    /// Last security check time
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_checked: Option<String>,

    /// DMARC status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dmarc: Option<DmarcStatus>,

    /// SPF status
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spf: Option<SpfStatus>,

    /// Security warnings
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<SecurityWarning>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
pub enum SecurityLevel {
    Healthy,
    #[default]
    Warning,
    Critical,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DmarcStatus {
    pub status: VerificationStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_record: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recommended_record: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SpfStatus {
    pub status: VerificationStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_record: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recommended_record: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
pub enum VerificationStatus {
    #[default]
    Pending,
    Valid,
    Warning,
    Missing,
    Invalid,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecurityWarning {
    pub severity: SecurityLevel,
    pub code: String,
    pub message: String,
    pub remediation: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_seen: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SendingQuota {
    pub max_24_hour_send: f64,
    pub max_send_rate: f64,
    pub sent_last_24_hours: f64,
    pub sandbox_mode: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReputationMetrics {
    pub bounce_rate: f64,
    pub complaint_rate: f64,
    pub bounce_rate_status: MetricStatus,
    pub complaint_rate_status: MetricStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema, PartialEq)]
pub enum MetricStatus {
    #[default]
    Healthy,
    Warning,
    Critical,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct NotificationTopics {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bounce_topic_arn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub complaint_topic_arn: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delivery_topic_arn: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EmailServiceCondition {
    #[serde(rename = "type")]
    pub condition_type: String,
    pub status: String,
    pub reason: String,
    pub message: String,
    pub last_transition_time: String,
}

// ============ Helper Functions ============

fn default_true() -> bool {
    true
}

// ============ Validation ============

impl EmailServiceSpec {
    /// Validate the spec for security requirements
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        // DKIM must be enabled (non-negotiable)
        if !self.dkim.enabled {
            errors.push("DKIM must be enabled - this is a security requirement".to_string());
        }

        // Domain must be subdomain
        if self.domain.matches('.').count() < 2 {
            errors.push("Domain must be a subdomain (e.g., mail.example.com), not root domain".to_string());
        }

        // TLS must be required
        if self.configuration_set.tls_policy == TlsPolicy::Optional {
            errors.push("TLS policy must be REQUIRE, not OPTIONAL".to_string());
        }

        // Alert thresholds must be below AWS suspension levels
        if self.alert_thresholds.bounce_rate_critical >= 5.0 {
            errors.push("Bounce rate critical threshold must be below 5% (AWS suspension level)".to_string());
        }
        if self.alert_thresholds.complaint_rate_critical >= 0.1 {
            errors.push("Complaint rate critical threshold must be below 0.1% (AWS suspension level)".to_string());
        }

        // MAIL FROM must reject on MX failure in production
        if self.environment == EmailEnvironment::Prod {
            if let Some(ref mail_from) = self.mail_from {
                if mail_from.behavior_on_mx_failure == MxFailureBehavior::UseDefaultValue {
                    errors.push("MAIL FROM behavior must be REJECT_MESSAGE in production for SPF alignment".to_string());
                }
            }

            // Notifications required in production
            if self.notifications.is_none() {
                errors.push("Bounce/complaint notifications are required in production".to_string());
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
