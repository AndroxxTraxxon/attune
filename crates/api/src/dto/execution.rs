//! Execution DTOs for API requests and responses

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use utoipa::{IntoParams, ToSchema};

use attune_common::models::enums::ExecutionStatus;

/// Request DTO for creating a manual execution
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateExecutionRequest {
    /// Action reference to execute
    #[schema(example = "slack.post_message")]
    pub action_ref: String,

    /// Execution parameters/configuration
    #[schema(value_type = Object, example = json!({"channel": "#alerts", "message": "Manual test"}))]
    pub parameters: Option<JsonValue>,

    /// Environment variables for this execution
    #[schema(value_type = Object, example = json!({"DEBUG": "true", "LOG_LEVEL": "info"}))]
    pub env_vars: Option<JsonValue>,
}

/// Response DTO for execution information
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ExecutionResponse {
    /// Execution ID
    #[schema(example = 1)]
    pub id: i64,

    /// Action ID (optional, may be null for ad-hoc executions)
    #[schema(example = 1)]
    pub action: Option<i64>,

    /// Action reference
    #[schema(example = "slack.post_message")]
    pub action_ref: String,

    /// Execution configuration/parameters
    #[schema(value_type = Object, example = json!({"channel": "#alerts", "message": "System error detected"}))]
    pub config: Option<JsonValue>,

    /// Parent execution ID (for nested/child executions)
    #[schema(example = 1)]
    pub parent: Option<i64>,

    /// Enforcement ID (rule enforcement that triggered this)
    #[schema(example = 1)]
    pub enforcement: Option<i64>,

    /// Executor ID (worker/executor that ran this)
    #[schema(example = 1)]
    pub executor: Option<i64>,

    /// Execution status
    #[schema(example = "succeeded")]
    pub status: ExecutionStatus,

    /// Execution result/output
    #[schema(value_type = Object, example = json!({"message_id": "1234567890.123456"}))]
    pub result: Option<JsonValue>,

    /// Creation timestamp
    #[schema(example = "2024-01-13T10:30:00Z")]
    pub created: DateTime<Utc>,

    /// Last update timestamp
    #[schema(example = "2024-01-13T10:35:00Z")]
    pub updated: DateTime<Utc>,
}

/// Simplified execution response (for list endpoints)
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ExecutionSummary {
    /// Execution ID
    #[schema(example = 1)]
    pub id: i64,

    /// Action reference
    #[schema(example = "slack.post_message")]
    pub action_ref: String,

    /// Execution status
    #[schema(example = "succeeded")]
    pub status: ExecutionStatus,

    /// Parent execution ID
    #[schema(example = 1)]
    pub parent: Option<i64>,

    /// Enforcement ID
    #[schema(example = 1)]
    pub enforcement: Option<i64>,

    /// Rule reference (if triggered by a rule)
    #[schema(example = "core.on_timer")]
    pub rule_ref: Option<String>,

    /// Trigger reference (if triggered by a trigger)
    #[schema(example = "core.timer")]
    pub trigger_ref: Option<String>,

    /// Creation timestamp
    #[schema(example = "2024-01-13T10:30:00Z")]
    pub created: DateTime<Utc>,

    /// Last update timestamp
    #[schema(example = "2024-01-13T10:35:00Z")]
    pub updated: DateTime<Utc>,
}

/// Query parameters for filtering executions
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ExecutionQueryParams {
    /// Filter by execution status
    #[param(example = "succeeded")]
    pub status: Option<ExecutionStatus>,

    /// Filter by action reference
    #[param(example = "slack.post_message")]
    pub action_ref: Option<String>,

    /// Filter by pack name
    #[param(example = "core")]
    pub pack_name: Option<String>,

    /// Filter by rule reference
    #[param(example = "core.on_timer")]
    pub rule_ref: Option<String>,

    /// Filter by trigger reference
    #[param(example = "core.timer")]
    pub trigger_ref: Option<String>,

    /// Filter by executor ID
    #[param(example = 1)]
    pub executor: Option<i64>,

    /// Search in result JSON (case-insensitive substring match)
    #[param(example = "error")]
    pub result_contains: Option<String>,

    /// Filter by enforcement ID
    #[param(example = 1)]
    pub enforcement: Option<i64>,

    /// Filter by parent execution ID
    #[param(example = 1)]
    pub parent: Option<i64>,

    /// Page number (for pagination)
    #[serde(default = "default_page")]
    #[param(example = 1, minimum = 1)]
    pub page: u32,

    /// Items per page (for pagination)
    #[serde(default = "default_per_page")]
    #[param(example = 50, minimum = 1, maximum = 100)]
    pub per_page: u32,
}

impl ExecutionQueryParams {
    /// Get the SQL offset value
    pub fn offset(&self) -> u32 {
        (self.page.saturating_sub(1)) * self.per_page
    }

    /// Get the limit value (with max cap)
    pub fn limit(&self) -> u32 {
        self.per_page.min(100)
    }
}

/// Convert from Execution model to ExecutionResponse
impl From<attune_common::models::execution::Execution> for ExecutionResponse {
    fn from(execution: attune_common::models::execution::Execution) -> Self {
        Self {
            id: execution.id,
            action: execution.action,
            action_ref: execution.action_ref,
            config: execution
                .config
                .map(|c| serde_json::to_value(c).unwrap_or(JsonValue::Null)),
            parent: execution.parent,
            enforcement: execution.enforcement,
            executor: execution.executor,
            status: execution.status,
            result: execution
                .result
                .map(|r| serde_json::to_value(r).unwrap_or(JsonValue::Null)),
            created: execution.created,
            updated: execution.updated,
        }
    }
}

/// Convert from Execution model to ExecutionSummary
impl From<attune_common::models::execution::Execution> for ExecutionSummary {
    fn from(execution: attune_common::models::execution::Execution) -> Self {
        Self {
            id: execution.id,
            action_ref: execution.action_ref,
            status: execution.status,
            parent: execution.parent,
            enforcement: execution.enforcement,
            rule_ref: None,    // Populated separately via enforcement lookup
            trigger_ref: None, // Populated separately via enforcement lookup
            created: execution.created,
            updated: execution.updated,
        }
    }
}

fn default_page() -> u32 {
    1
}

fn default_per_page() -> u32 {
    20
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_params_defaults() {
        let json = r#"{}"#;
        let params: ExecutionQueryParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.page, 1);
        assert_eq!(params.per_page, 20);
        assert!(params.status.is_none());
    }

    #[test]
    fn test_query_params_with_filters() {
        let json = r#"{
            "status": "completed",
            "action_ref": "test.action",
            "page": 2,
            "per_page": 50
        }"#;
        let params: ExecutionQueryParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.page, 2);
        assert_eq!(params.per_page, 50);
        assert_eq!(params.status, Some(ExecutionStatus::Completed));
        assert_eq!(params.action_ref, Some("test.action".to_string()));
    }

    #[test]
    fn test_query_params_offset() {
        let params = ExecutionQueryParams {
            status: None,
            action_ref: None,
            enforcement: None,
            parent: None,
            pack_name: None,
            rule_ref: None,
            trigger_ref: None,
            executor: None,
            result_contains: None,
            page: 3,
            per_page: 20,
        };
        assert_eq!(params.offset(), 40); // (3-1) * 20
    }

    #[test]
    fn test_query_params_limit_cap() {
        let params = ExecutionQueryParams {
            status: None,
            action_ref: None,
            enforcement: None,
            parent: None,
            pack_name: None,
            rule_ref: None,
            trigger_ref: None,
            executor: None,
            result_contains: None,
            page: 1,
            per_page: 200, // Exceeds max
        };
        assert_eq!(params.limit(), 100); // Capped at 100
    }
}
