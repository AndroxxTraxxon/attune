//! Action DTOs for API requests and responses

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use utoipa::ToSchema;
use validator::Validate;

/// Request DTO for creating a new action
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct CreateActionRequest {
    /// Unique reference identifier (e.g., "core.http", "aws.ec2.start_instance")
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "slack.post_message")]
    pub r#ref: String,

    /// Pack reference this action belongs to
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "slack")]
    pub pack_ref: String,

    /// Human-readable label
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "Post Message to Slack")]
    pub label: String,

    /// Action description
    #[validate(length(min = 1))]
    #[schema(example = "Posts a message to a Slack channel")]
    pub description: String,

    /// Entry point for action execution (e.g., path to script, function name)
    #[validate(length(min = 1, max = 1024))]
    #[schema(example = "/actions/slack/post_message.py")]
    pub entrypoint: String,

    /// Optional runtime ID for this action
    #[schema(example = 1)]
    pub runtime: Option<i64>,

    /// Parameter schema (StackStorm-style) defining expected inputs with inline required/secret
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Object, nullable = true, example = json!({"channel": {"type": "string", "description": "Slack channel", "required": true}, "message": {"type": "string", "description": "Message text", "required": true}}))]
    pub param_schema: Option<JsonValue>,

    /// Output schema (flat format) defining expected outputs with inline required/secret
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Object, nullable = true, example = json!({"message_id": {"type": "string", "description": "ID of the sent message", "required": true}}))]
    pub out_schema: Option<JsonValue>,
}

/// Request DTO for updating an action
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct UpdateActionRequest {
    /// Human-readable label
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "Post Message to Slack (Updated)")]
    pub label: Option<String>,

    /// Action description
    #[validate(length(min = 1))]
    #[schema(example = "Posts a message to a Slack channel with enhanced features")]
    pub description: Option<String>,

    /// Entry point for action execution
    #[validate(length(min = 1, max = 1024))]
    #[schema(example = "/actions/slack/post_message_v2.py")]
    pub entrypoint: Option<String>,

    /// Runtime ID
    #[schema(example = 1)]
    pub runtime: Option<i64>,

    /// Parameter schema (StackStorm-style with inline required/secret)
    #[schema(value_type = Object, nullable = true)]
    pub param_schema: Option<JsonValue>,

    /// Output schema
    #[schema(value_type = Object, nullable = true)]
    pub out_schema: Option<JsonValue>,
}

/// Response DTO for action information
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ActionResponse {
    /// Action ID
    #[schema(example = 1)]
    pub id: i64,

    /// Unique reference identifier
    #[schema(example = "slack.post_message")]
    pub r#ref: String,

    /// Pack ID
    #[schema(example = 1)]
    pub pack: i64,

    /// Pack reference
    #[schema(example = "slack")]
    pub pack_ref: String,

    /// Human-readable label
    #[schema(example = "Post Message to Slack")]
    pub label: String,

    /// Action description
    #[schema(example = "Posts a message to a Slack channel")]
    pub description: String,

    /// Entry point
    #[schema(example = "/actions/slack/post_message.py")]
    pub entrypoint: String,

    /// Runtime ID
    #[schema(example = 1)]
    pub runtime: Option<i64>,

    /// Parameter schema (StackStorm-style with inline required/secret)
    #[schema(value_type = Object, nullable = true)]
    pub param_schema: Option<JsonValue>,

    /// Output schema
    #[schema(value_type = Object, nullable = true)]
    pub out_schema: Option<JsonValue>,

    /// Whether this is an ad-hoc action (not from pack installation)
    #[schema(example = false)]
    pub is_adhoc: bool,

    /// Creation timestamp
    #[schema(example = "2024-01-13T10:30:00Z")]
    pub created: DateTime<Utc>,

    /// Last update timestamp
    #[schema(example = "2024-01-13T10:30:00Z")]
    pub updated: DateTime<Utc>,
}

/// Simplified action response (for list endpoints)
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ActionSummary {
    /// Action ID
    #[schema(example = 1)]
    pub id: i64,

    /// Unique reference identifier
    #[schema(example = "slack.post_message")]
    pub r#ref: String,

    /// Pack reference
    #[schema(example = "slack")]
    pub pack_ref: String,

    /// Human-readable label
    #[schema(example = "Post Message to Slack")]
    pub label: String,

    /// Action description
    #[schema(example = "Posts a message to a Slack channel")]
    pub description: String,

    /// Entry point
    #[schema(example = "/actions/slack/post_message.py")]
    pub entrypoint: String,

    /// Runtime ID
    #[schema(example = 1)]
    pub runtime: Option<i64>,

    /// Creation timestamp
    #[schema(example = "2024-01-13T10:30:00Z")]
    pub created: DateTime<Utc>,

    /// Last update timestamp
    #[schema(example = "2024-01-13T10:30:00Z")]
    pub updated: DateTime<Utc>,
}

/// Convert from Action model to ActionResponse
impl From<attune_common::models::action::Action> for ActionResponse {
    fn from(action: attune_common::models::action::Action) -> Self {
        Self {
            id: action.id,
            r#ref: action.r#ref,
            pack: action.pack,
            pack_ref: action.pack_ref,
            label: action.label,
            description: action.description,
            entrypoint: action.entrypoint,
            runtime: action.runtime,
            param_schema: action.param_schema,
            out_schema: action.out_schema,
            is_adhoc: action.is_adhoc,
            created: action.created,
            updated: action.updated,
        }
    }
}

/// Convert from Action model to ActionSummary
impl From<attune_common::models::action::Action> for ActionSummary {
    fn from(action: attune_common::models::action::Action) -> Self {
        Self {
            id: action.id,
            r#ref: action.r#ref,
            pack_ref: action.pack_ref,
            label: action.label,
            description: action.description,
            entrypoint: action.entrypoint,
            runtime: action.runtime,
            created: action.created,
            updated: action.updated,
        }
    }
}

/// Response DTO for queue statistics
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct QueueStatsResponse {
    /// Action ID
    #[schema(example = 1)]
    pub action_id: i64,

    /// Action reference
    #[schema(example = "slack.post_message")]
    pub action_ref: String,

    /// Number of executions waiting in queue
    #[schema(example = 5)]
    pub queue_length: i32,

    /// Number of currently running executions
    #[schema(example = 2)]
    pub active_count: i32,

    /// Maximum concurrent executions allowed
    #[schema(example = 3)]
    pub max_concurrent: i32,

    /// Timestamp of oldest queued execution (if any)
    #[schema(example = "2024-01-13T10:30:00Z")]
    pub oldest_enqueued_at: Option<DateTime<Utc>>,

    /// Total executions enqueued since queue creation
    #[schema(example = 100)]
    pub total_enqueued: i64,

    /// Total executions completed since queue creation
    #[schema(example = 95)]
    pub total_completed: i64,

    /// Timestamp of last statistics update
    #[schema(example = "2024-01-13T10:30:00Z")]
    pub last_updated: DateTime<Utc>,
}

/// Convert from QueueStats repository model to QueueStatsResponse
impl From<attune_common::repositories::queue_stats::QueueStats> for QueueStatsResponse {
    fn from(stats: attune_common::repositories::queue_stats::QueueStats) -> Self {
        Self {
            action_id: stats.action_id,
            action_ref: String::new(), // Will be populated by the handler
            queue_length: stats.queue_length,
            active_count: stats.active_count,
            max_concurrent: stats.max_concurrent,
            oldest_enqueued_at: stats.oldest_enqueued_at,
            total_enqueued: stats.total_enqueued,
            total_completed: stats.total_completed,
            last_updated: stats.last_updated,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_action_request_validation() {
        let req = CreateActionRequest {
            r#ref: "".to_string(), // Invalid: empty
            pack_ref: "test-pack".to_string(),
            label: "Test Action".to_string(),
            description: "Test description".to_string(),
            entrypoint: "/actions/test.py".to_string(),
            runtime: None,
            param_schema: None,
            out_schema: None,
        };

        assert!(req.validate().is_err());
    }

    #[test]
    fn test_create_action_request_valid() {
        let req = CreateActionRequest {
            r#ref: "test.action".to_string(),
            pack_ref: "test-pack".to_string(),
            label: "Test Action".to_string(),
            description: "Test description".to_string(),
            entrypoint: "/actions/test.py".to_string(),
            runtime: None,
            param_schema: None,
            out_schema: None,
        };

        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_update_action_request_all_none() {
        let req = UpdateActionRequest {
            label: None,
            description: None,
            entrypoint: None,
            runtime: None,
            param_schema: None,
            out_schema: None,
        };

        // Should be valid even with all None values
        assert!(req.validate().is_ok());
    }
}
