//! Webhook-related DTOs for API requests and responses

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use utoipa::ToSchema;

/// Request body for webhook receiver endpoint
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WebhookReceiverRequest {
    /// Webhook payload (arbitrary JSON)
    pub payload: JsonValue,

    /// Optional headers from the webhook request (for logging/debugging)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub headers: Option<JsonValue>,

    /// Optional source IP address
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_ip: Option<String>,

    /// Optional user agent
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_agent: Option<String>,
}

/// Response from webhook receiver endpoint
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct WebhookReceiverResponse {
    /// ID of the event created from this webhook
    pub event_id: i64,

    /// Reference of the trigger that received this webhook
    pub trigger_ref: String,

    /// Timestamp when the webhook was received
    pub received_at: DateTime<Utc>,

    /// Success message
    pub message: String,
}
