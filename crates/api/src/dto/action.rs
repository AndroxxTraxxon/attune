//! Action DTOs for API requests and responses

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::BTreeMap;
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
    #[schema(example = "Posts a message to a Slack channel")]
    pub description: Option<String>,

    /// Entry point for action execution (e.g., path to script, function name)
    #[validate(length(min = 1, max = 1024))]
    #[schema(example = "/actions/slack/post_message.py")]
    pub entrypoint: String,

    /// Optional runtime ID for this action
    #[schema(example = 1)]
    pub runtime: Option<i64>,

    /// Optional runtime reference for this action
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "core.python", nullable = true)]
    pub runtime_ref: Option<String>,

    /// Optional semver version constraint for the runtime (e.g., ">=3.12", ">=3.12,<4.0", "~18.0")
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = ">=3.12", nullable = true)]
    pub runtime_version_constraint: Option<String>,

    /// Additional worker runtime requirements keyed by runtime name/alias. Use "*" for any available version.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    #[schema(value_type = Object, example = json!({"node": "*", "python": ">=3.12"}), default = json!({}))]
    pub required_worker_runtimes: BTreeMap<String, String>,

    /// Parameter schema (StackStorm-style) defining expected inputs with inline required/secret
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Object, nullable = true, example = json!({"channel": {"type": "string", "description": "Slack channel", "required": true}, "message": {"type": "string", "description": "Message text", "required": true}}))]
    pub param_schema: Option<JsonValue>,

    /// Output schema (flat format) defining expected outputs with inline required/secret
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Object, nullable = true, example = json!({"message_id": {"type": "string", "description": "ID of the sent message", "required": true}}))]
    pub out_schema: Option<JsonValue>,

    /// Hint that this action may invoke the Attune MCP server and spawn child executions.
    /// When true, consumers (UI, CLI, timeline charts) render subtask views eagerly.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = false, default = false, nullable = true)]
    pub accesses_mcp: Option<bool>,
}

/// Request DTO for updating an action
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct UpdateActionRequest {
    /// Human-readable label
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "Post Message to Slack (Updated)")]
    pub label: Option<String>,

    /// Action description
    #[schema(example = "Posts a message to a Slack channel with enhanced features")]
    pub description: Option<String>,

    /// Entry point for action execution
    #[validate(length(min = 1, max = 1024))]
    #[schema(example = "/actions/slack/post_message_v2.py")]
    pub entrypoint: Option<String>,

    /// Runtime ID
    #[schema(example = 1)]
    pub runtime: Option<i64>,

    /// Runtime reference
    #[schema(example = "core.python", nullable = true)]
    pub runtime_ref: Option<String>,

    /// Optional semver version constraint patch for the runtime.
    pub runtime_version_constraint: Option<RuntimeVersionConstraintPatch>,

    /// Additional worker runtime requirements keyed by runtime name/alias. Use "*" for any available version.
    #[schema(value_type = Object, example = json!({"node": "*", "python": ">=3.12"}), nullable = true)]
    pub required_worker_runtimes: Option<BTreeMap<String, String>>,

    /// Parameter schema (StackStorm-style with inline required/secret)
    #[schema(value_type = Object, nullable = true)]
    pub param_schema: Option<JsonValue>,

    /// Output schema
    #[schema(value_type = Object, nullable = true)]
    pub out_schema: Option<JsonValue>,

    /// Hint that this action may invoke the Attune MCP server and spawn child executions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(example = false, nullable = true)]
    pub accesses_mcp: Option<bool>,
}

/// Explicit patch operation for a nullable runtime version constraint.
#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(tag = "op", content = "value", rename_all = "snake_case")]
pub enum RuntimeVersionConstraintPatch {
    Set(String),
    Clear,
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
    pub description: Option<String>,

    /// Entry point
    #[schema(example = "/actions/slack/post_message.py")]
    pub entrypoint: String,

    /// Runtime ID
    #[schema(example = 1)]
    pub runtime: Option<i64>,

    /// Runtime reference (stable identifier, e.g., "core.python")
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "core.python", nullable = true)]
    pub runtime_ref: Option<String>,

    /// Semver version constraint for the runtime (e.g., ">=3.12", ">=3.12,<4.0", "~18.0")
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = ">=3.12", nullable = true)]
    pub runtime_version_constraint: Option<String>,

    /// Additional worker runtime requirements keyed by runtime name/alias. Use "*" for any available version.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[schema(value_type = Object, example = json!({"node": "*", "python": ">=3.12"}))]
    pub required_worker_runtimes: BTreeMap<String, String>,

    /// Parameter schema (StackStorm-style with inline required/secret)
    #[schema(value_type = Object, nullable = true)]
    pub param_schema: Option<JsonValue>,

    /// Output schema
    #[schema(value_type = Object, nullable = true)]
    pub out_schema: Option<JsonValue>,

    /// Workflow definition ID (non-null if this action is a workflow)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 42, nullable = true)]
    pub workflow_def: Option<i64>,

    /// Whether this is an ad-hoc action (not from pack installation)
    #[schema(example = false)]
    pub is_adhoc: bool,

    /// Hint that this action may invoke the Attune MCP server and spawn child executions.
    #[schema(example = false, default = false)]
    pub accesses_mcp: bool,

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
    pub description: Option<String>,

    /// Entry point
    #[schema(example = "/actions/slack/post_message.py")]
    pub entrypoint: String,

    /// Runtime ID
    #[schema(example = 1)]
    pub runtime: Option<i64>,

    /// Runtime reference (stable identifier, e.g., "core.python")
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "core.python", nullable = true)]
    pub runtime_ref: Option<String>,

    /// Semver version constraint for the runtime
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = ">=3.12", nullable = true)]
    pub runtime_version_constraint: Option<String>,

    /// Additional worker runtime requirements keyed by runtime name/alias. Use "*" for any available version.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    #[schema(value_type = Object, example = json!({"node": "*", "python": ">=3.12"}))]
    pub required_worker_runtimes: BTreeMap<String, String>,

    /// Workflow definition ID (non-null if this action is a workflow)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = 42, nullable = true)]
    pub workflow_def: Option<i64>,

    /// Hint that this action may invoke the Attune MCP server and spawn child executions.
    #[schema(example = false, default = false)]
    pub accesses_mcp: bool,

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
        let required_worker_runtimes = action.required_worker_runtime_constraints();
        Self {
            id: action.id,
            r#ref: action.r#ref,
            pack: action.pack,
            pack_ref: action.pack_ref,
            label: action.label,
            description: action.description,
            entrypoint: action.entrypoint,
            runtime: action.runtime,
            runtime_ref: None,
            runtime_version_constraint: action.runtime_version_constraint,
            required_worker_runtimes,
            param_schema: action.param_schema,
            out_schema: action.out_schema,
            workflow_def: action.workflow_def,
            is_adhoc: action.is_adhoc,
            accesses_mcp: action.accesses_mcp,
            created: action.created,
            updated: action.updated,
        }
    }
}

/// Convert from Action model to ActionSummary
impl From<attune_common::models::action::Action> for ActionSummary {
    fn from(action: attune_common::models::action::Action) -> Self {
        let required_worker_runtimes = action.required_worker_runtime_constraints();
        Self {
            id: action.id,
            r#ref: action.r#ref,
            pack_ref: action.pack_ref,
            label: action.label,
            description: action.description,
            entrypoint: action.entrypoint,
            runtime: action.runtime,
            runtime_ref: None,
            runtime_version_constraint: action.runtime_version_constraint,
            required_worker_runtimes,
            workflow_def: action.workflow_def,
            accesses_mcp: action.accesses_mcp,
            created: action.created,
            updated: action.updated,
        }
    }
}

/// Lean search hit for action discovery — designed to minimize context bloat
/// for AI agents and humans browsing large action catalogs. Excludes ID,
/// timestamps, schemas, and runtime internals.
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ActionSearchHit {
    /// Action reference (globally unique identifier, e.g., "slack.post_message")
    #[schema(example = "slack.post_message")]
    pub r#ref: String,

    /// Pack reference
    #[schema(example = "slack")]
    pub pack_ref: String,

    /// Human-readable label
    #[schema(example = "Post Message to Slack")]
    pub label: String,

    /// Action description
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "Posts a message to a Slack channel", nullable = true)]
    pub description: Option<String>,

    /// Runtime reference (e.g., "core.python"). None for workflow actions.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "core.python", nullable = true)]
    pub runtime_ref: Option<String>,

    /// True when this action is a workflow (orchestrates child executions)
    #[schema(example = false)]
    pub is_workflow: bool,

    /// Hint that this action may invoke the Attune MCP server and spawn child executions.
    #[schema(example = false)]
    pub accesses_mcp: bool,
}

/// Convert from Action model to ActionSearchHit (runtime_ref populated by handler)
impl From<attune_common::models::action::Action> for ActionSearchHit {
    fn from(action: attune_common::models::action::Action) -> Self {
        Self {
            r#ref: action.r#ref,
            pack_ref: action.pack_ref,
            label: action.label,
            description: action.description,
            runtime_ref: None,
            is_workflow: action.workflow_def.is_some(),
            accesses_mcp: action.accesses_mcp,
        }
    }
}

/// Query parameters for `GET /api/v1/actions/search`.
#[derive(Debug, Clone, Deserialize, utoipa::IntoParams)]
pub struct ActionSearchParams {
    /// Keyword query. Whitespace-separated tokens are AND-matched against
    /// `ref`, `label`, `description`, and `pack_ref` (case-insensitive substring).
    #[param(example = "slack post message")]
    pub q: Option<String>,

    /// Restrict to one or more pack refs. Comma-separated (e.g., `core,slack,jira`)
    /// or repeated query params (e.g., `?packs=core&packs=slack`).
    #[param(example = "core,slack")]
    pub packs: Option<String>,
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
            description: Some("Test description".to_string()),
            entrypoint: "/actions/test.py".to_string(),
            runtime: None,
            runtime_ref: None,
            runtime_version_constraint: None,
            required_worker_runtimes: BTreeMap::new(),
            param_schema: None,
            out_schema: None,
            accesses_mcp: None,
        };

        assert!(req.validate().is_err());
    }

    #[test]
    fn test_create_action_request_valid() {
        let req = CreateActionRequest {
            r#ref: "test.action".to_string(),
            pack_ref: "test-pack".to_string(),
            label: "Test Action".to_string(),
            description: Some("Test description".to_string()),
            entrypoint: "/actions/test.py".to_string(),
            runtime: None,
            runtime_ref: None,
            runtime_version_constraint: None,
            required_worker_runtimes: BTreeMap::new(),
            param_schema: None,
            out_schema: None,
            accesses_mcp: None,
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
            runtime_ref: None,
            runtime_version_constraint: None,
            required_worker_runtimes: None,
            param_schema: None,
            out_schema: None,
            accesses_mcp: None,
        };

        // Should be valid even with all None values
        assert!(req.validate().is_ok());
    }
}
