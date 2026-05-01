//! Audit event data transfer objects.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use attune_common::audit::{AuditCategory, AuditEvent, AuditOutcome};
use attune_common::models::Id;

/// Full audit event with all fields.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuditEventResponse {
    #[schema(example = 1)]
    pub id: Id,

    /// Event creation timestamp.
    #[schema(example = "2024-01-13T10:30:00Z")]
    pub created: DateTime<Utc>,

    /// High-level category.
    #[schema(example = "auth")]
    pub category: String,

    /// Dotted event-type identifier (e.g., `auth.login.success`).
    #[schema(example = "auth.login.success")]
    pub event_type: String,

    /// Outcome (`success`, `failure`, or `denied`).
    #[schema(example = "success")]
    pub outcome: String,

    /// Identity ID of the actor (NULL for anonymous/pre-auth).
    pub actor_identity: Option<Id>,

    /// Snapshot of `identity.login` at time of the event.
    pub actor_login: Option<String>,

    /// Token type (`access`, `execution`, `sensor`, `refresh`).
    pub actor_token_type: Option<String>,

    /// Source IP of the request.
    pub actor_ip: Option<String>,

    /// User-Agent header from the request.
    pub actor_user_agent: Option<String>,

    /// Correlation request ID assigned by the API middleware.
    pub request_id: Option<Uuid>,

    /// Logical resource type (e.g., `pack`, `key`, `execution`).
    pub resource_type: Option<String>,

    /// Resource ID.
    pub resource_id: Option<Id>,

    /// Resource reference snapshot (forensic).
    pub resource_ref: Option<String>,

    /// HTTP method (NULL for non-API events).
    pub http_method: Option<String>,

    /// HTTP path.
    pub http_path: Option<String>,

    /// HTTP status code.
    pub http_status: Option<i32>,

    /// Request duration in milliseconds.
    pub duration_ms: Option<i32>,

    /// Event-specific structured metadata. Secrets are redacted.
    #[schema(value_type = Object, nullable = true)]
    pub details: Option<JsonValue>,

    /// Optional cascade chain (rule_id, enforcement_id, execution_id, …).
    #[schema(value_type = Object, nullable = true)]
    pub correlation_chain: Option<JsonValue>,
}

impl From<AuditEvent> for AuditEventResponse {
    fn from(e: AuditEvent) -> Self {
        Self {
            id: e.id,
            created: e.created,
            category: serde_plain_string(&e.category),
            event_type: e.event_type,
            outcome: serde_plain_string(&e.outcome),
            actor_identity: e.actor_identity,
            actor_login: e.actor_login,
            actor_token_type: e.actor_token_type,
            actor_ip: e.actor_ip,
            actor_user_agent: e.actor_user_agent,
            request_id: e.request_id,
            resource_type: e.resource_type,
            resource_id: e.resource_id,
            resource_ref: e.resource_ref,
            http_method: e.http_method,
            http_path: e.http_path,
            http_status: e.http_status,
            duration_ms: e.duration_ms,
            details: e.details,
            correlation_chain: e.correlation_chain,
        }
    }
}

/// Compact summary of an audit event for list views.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AuditEventSummary {
    pub id: Id,
    pub created: DateTime<Utc>,
    pub category: String,
    pub event_type: String,
    pub outcome: String,
    pub actor_identity: Option<Id>,
    pub actor_login: Option<String>,
    pub resource_type: Option<String>,
    pub resource_ref: Option<String>,
    pub http_method: Option<String>,
    pub http_path: Option<String>,
    pub http_status: Option<i32>,
    pub request_id: Option<Uuid>,
}

impl From<AuditEvent> for AuditEventSummary {
    fn from(e: AuditEvent) -> Self {
        Self {
            id: e.id,
            created: e.created,
            category: serde_plain_string(&e.category),
            event_type: e.event_type,
            outcome: serde_plain_string(&e.outcome),
            actor_identity: e.actor_identity,
            actor_login: e.actor_login,
            resource_type: e.resource_type,
            resource_ref: e.resource_ref,
            http_method: e.http_method,
            http_path: e.http_path,
            http_status: e.http_status,
            request_id: e.request_id,
        }
    }
}

/// Query parameters for filtering the audit log.
#[derive(Debug, Clone, Serialize, Deserialize, IntoParams, Default)]
pub struct AuditEventQueryParams {
    /// Top-level category (`api`, `auth`, `rbac`, `secret`, `admin`,
    /// `execution`, `pack`).
    pub category: Option<AuditCategory>,

    /// Exact match on the dotted event-type identifier.
    pub event_type: Option<String>,

    /// Outcome (`success`, `failure`, `denied`).
    pub outcome: Option<AuditOutcome>,

    /// Filter by actor identity ID.
    pub actor_identity: Option<Id>,

    /// Substring match (case-insensitive) on actor login.
    pub actor_login: Option<String>,

    /// Logical resource type (`pack`, `key`, `action`, …).
    pub resource_type: Option<String>,

    /// Filter by resource ID.
    pub resource_id: Option<Id>,

    /// Exact match on resource ref.
    pub resource_ref: Option<String>,

    /// Filter by request_id correlation UUID.
    pub request_id: Option<Uuid>,

    /// HTTP status code (typed-API events only).
    pub http_status: Option<i32>,

    /// HTTP method (`GET`, `POST`, …).
    pub http_method: Option<String>,

    /// Substring match on the HTTP path.
    pub http_path: Option<String>,

    /// Lower bound on `created` (inclusive, RFC3339).
    pub created_after: Option<DateTime<Utc>>,

    /// Upper bound on `created` (exclusive, RFC3339).
    pub created_before: Option<DateTime<Utc>>,

    /// Include exact total count in pagination metadata.
    #[serde(default)]
    pub include_total: Option<bool>,

    /// Page number (1-indexed).
    #[serde(default = "default_page")]
    #[param(example = 1, minimum = 1)]
    pub page: u32,

    /// Items per page.
    #[serde(default = "default_per_page")]
    #[param(example = 50, minimum = 1, maximum = 500)]
    pub per_page: u32,
}

fn default_page() -> u32 {
    1
}

fn default_per_page() -> u32 {
    50
}

impl AuditEventQueryParams {
    pub fn offset(&self) -> u32 {
        (self.page.saturating_sub(1)) * self.per_page
    }

    pub fn limit(&self) -> u32 {
        self.per_page.clamp(1, 500)
    }
}

fn serde_plain_string<T: Serialize>(value: &T) -> String {
    // Enums in this module derive `serde(rename_all = "lowercase")`, so
    // serialising to JSON yields a quoted lowercase identifier.
    serde_json::to_value(value)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}
