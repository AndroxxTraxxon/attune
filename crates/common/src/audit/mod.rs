//! Audit log types, emission pipeline, and query repository.
//!
//! Records security- and compliance-relevant events across Attune services
//! (API requests, auth, RBAC denials, secret access, admin/config changes,
//! execution lifecycle, pack registration).
//!
//! # Architecture
//!
//! Emission is non-blocking: callers construct an [`AuditEvent`] (typically
//! via [`AuditEventBuilder`]) and pass it to [`AuditEmitter::emit`], which
//! sends it on an unbounded mpsc channel. A background [`AuditWriter`] task
//! batch-inserts events into the `audit_event` hypertable.
//!
//! On channel-receiver-dropped or DB error we log the failure but never
//! propagate it back to the caller — audit emission must never break the
//! request path.
//!
//! # Secret masking
//!
//! Callers that need to capture user-supplied parameters should use
//! [`AuditEventBuilder::with_redacted_params`], which preserves the keys
//! but replaces all values with `"***"`. For schema-aware redaction the
//! caller can pre-process the parameter map and pass the result via
//! [`AuditEventBuilder::with_details`].

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use sqlx::FromRow;
use std::net::IpAddr;
use std::str::FromStr;
use uuid::Uuid;

use crate::models::Id;

pub mod emitter;
pub mod repository;
pub mod writer;

pub use emitter::AuditEmitter;
pub use repository::{AuditEventFilters, AuditRepository};
pub use writer::{spawn_writer, AuditWriterHandle};

/// Top-level category for an audit event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "audit_category_enum", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum AuditCategory {
    /// HTTP request/response audit
    Api,
    /// login/logout/token-refresh/token-expiry
    Auth,
    /// authorization decisions (denials always; allows optional)
    Rbac,
    /// key reads (especially decrypts), creates, updates, deletes
    Secret,
    /// identity, role, permission-set changes; pack/rule toggles
    Admin,
    /// execution lifecycle (requested, started, completed, failed, cancelled)
    Execution,
    /// pack uploads, registration, deletion
    Pack,
}

/// Outcome of an audited action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "audit_outcome_enum", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum AuditOutcome {
    Success,
    Failure,
    Denied,
}

/// A single audit log record.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct AuditEvent {
    pub id: Id,
    pub created: DateTime<Utc>,

    pub category: AuditCategory,
    pub event_type: String,
    pub outcome: AuditOutcome,

    pub actor_identity: Option<Id>,
    pub actor_login: Option<String>,
    pub actor_token_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub actor_ip: Option<String>,
    pub actor_user_agent: Option<String>,

    pub request_id: Option<Uuid>,

    pub resource_type: Option<String>,
    pub resource_id: Option<Id>,
    pub resource_ref: Option<String>,

    pub http_method: Option<String>,
    pub http_path: Option<String>,
    pub http_status: Option<i32>,
    pub duration_ms: Option<i32>,

    pub details: Option<JsonValue>,
    pub correlation_chain: Option<JsonValue>,
}

/// Borrow-friendly value object passed through the emission channel.
///
/// Constructed by [`AuditEventBuilder::build`].  Has no `id`/`created`;
/// those are assigned by the database.
#[derive(Debug, Clone)]
pub struct PendingAuditEvent {
    pub category: AuditCategory,
    pub event_type: String,
    pub outcome: AuditOutcome,

    pub actor_identity: Option<Id>,
    pub actor_login: Option<String>,
    pub actor_token_type: Option<String>,
    pub actor_ip: Option<IpAddr>,
    pub actor_user_agent: Option<String>,

    pub request_id: Option<Uuid>,

    pub resource_type: Option<String>,
    pub resource_id: Option<Id>,
    pub resource_ref: Option<String>,

    pub http_method: Option<String>,
    pub http_path: Option<String>,
    pub http_status: Option<i32>,
    pub duration_ms: Option<i32>,

    pub details: Option<JsonValue>,
    pub correlation_chain: Option<JsonValue>,
}

/// Fluent builder for [`PendingAuditEvent`].
#[derive(Debug, Clone)]
pub struct AuditEventBuilder {
    inner: PendingAuditEvent,
}

impl AuditEventBuilder {
    pub fn new(
        category: AuditCategory,
        event_type: impl Into<String>,
        outcome: AuditOutcome,
    ) -> Self {
        Self {
            inner: PendingAuditEvent {
                category,
                event_type: event_type.into(),
                outcome,
                actor_identity: None,
                actor_login: None,
                actor_token_type: None,
                actor_ip: None,
                actor_user_agent: None,
                request_id: None,
                resource_type: None,
                resource_id: None,
                resource_ref: None,
                http_method: None,
                http_path: None,
                http_status: None,
                duration_ms: None,
                details: None,
                correlation_chain: None,
            },
        }
    }

    pub fn actor_identity(mut self, id: Id) -> Self {
        self.inner.actor_identity = Some(id);
        self
    }

    pub fn actor_login(mut self, login: impl Into<String>) -> Self {
        self.inner.actor_login = Some(login.into());
        self
    }

    pub fn actor_token_type(mut self, token_type: impl Into<String>) -> Self {
        self.inner.actor_token_type = Some(token_type.into());
        self
    }

    pub fn actor_ip(mut self, ip: IpAddr) -> Self {
        self.inner.actor_ip = Some(ip);
        self
    }

    /// Parse a string IP address; silently dropped if it cannot be parsed.
    pub fn actor_ip_str(mut self, ip: &str) -> Self {
        if let Ok(parsed) = IpAddr::from_str(ip) {
            self.inner.actor_ip = Some(parsed);
        }
        self
    }

    pub fn actor_user_agent(mut self, ua: impl Into<String>) -> Self {
        self.inner.actor_user_agent = Some(ua.into());
        self
    }

    pub fn request_id(mut self, id: Uuid) -> Self {
        self.inner.request_id = Some(id);
        self
    }

    pub fn resource(mut self, resource_type: impl Into<String>) -> Self {
        self.inner.resource_type = Some(resource_type.into());
        self
    }

    pub fn resource_id(mut self, id: Id) -> Self {
        self.inner.resource_id = Some(id);
        self
    }

    pub fn resource_ref(mut self, r: impl Into<String>) -> Self {
        self.inner.resource_ref = Some(r.into());
        self
    }

    pub fn http(
        mut self,
        method: impl Into<String>,
        path: impl Into<String>,
        status: i32,
        duration_ms: i32,
    ) -> Self {
        self.inner.http_method = Some(method.into());
        self.inner.http_path = Some(path.into());
        self.inner.http_status = Some(status);
        self.inner.duration_ms = Some(duration_ms);
        self
    }

    pub fn with_details(mut self, details: JsonValue) -> Self {
        self.inner.details = Some(details);
        self
    }

    /// Capture the parameter map's key set, replacing every value with `"***"`.
    /// The serialized result is stored under `details.params`.
    pub fn with_redacted_params(mut self, params: &JsonValue) -> Self {
        let redacted = redact_value(params);
        let merged = match self.inner.details.take() {
            Some(JsonValue::Object(mut obj)) => {
                obj.insert("params".to_string(), redacted);
                JsonValue::Object(obj)
            }
            _ => json!({ "params": redacted }),
        };
        self.inner.details = Some(merged);
        self
    }

    pub fn with_correlation_chain(mut self, chain: JsonValue) -> Self {
        self.inner.correlation_chain = Some(chain);
        self
    }

    pub fn build(self) -> PendingAuditEvent {
        self.inner
    }
}

/// Recursively redact every leaf value in a JSON document, preserving structure.
/// Strings, numbers, bools, and nulls become the string `"***"`. Arrays and
/// objects are walked.
fn redact_value(v: &JsonValue) -> JsonValue {
    match v {
        JsonValue::Object(map) => JsonValue::Object(
            map.iter()
                .map(|(k, val)| (k.clone(), redact_value(val)))
                .collect(),
        ),
        JsonValue::Array(items) => JsonValue::Array(items.iter().map(redact_value).collect()),
        _ => JsonValue::String("***".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redact_preserves_keys() {
        let input = json!({
            "username": "alice",
            "password": "hunter2",
            "nested": { "token": "abc123", "count": 5 },
            "arr": [1, 2, "secret"],
        });
        let out = redact_value(&input);
        let expected = json!({
            "username": "***",
            "password": "***",
            "nested": { "token": "***", "count": "***" },
            "arr": ["***", "***", "***"],
        });
        assert_eq!(out, expected);
    }

    #[test]
    fn builder_roundtrip() {
        let evt = AuditEventBuilder::new(
            AuditCategory::Auth,
            "auth.login.success",
            AuditOutcome::Success,
        )
        .actor_identity(42)
        .actor_login("alice")
        .actor_token_type("access")
        .actor_ip_str("127.0.0.1")
        .request_id(Uuid::nil())
        .with_details(json!({ "method": "local" }))
        .build();

        assert_eq!(evt.category, AuditCategory::Auth);
        assert_eq!(evt.event_type, "auth.login.success");
        assert_eq!(evt.actor_identity, Some(42));
        assert_eq!(evt.actor_login.as_deref(), Some("alice"));
        assert_eq!(evt.actor_token_type.as_deref(), Some("access"));
        assert!(evt.actor_ip.is_some());
    }

    #[test]
    fn builder_actor_ip_str_drops_invalid() {
        let evt = AuditEventBuilder::new(AuditCategory::Api, "api.request", AuditOutcome::Success)
            .actor_ip_str("not-an-ip")
            .build();
        assert!(evt.actor_ip.is_none());
    }
}
