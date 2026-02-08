//! Data models for Attune services
//!
//! This module contains the data models that map to the database schema.
//! Models are organized by functional area and use SQLx for database operations.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::FromRow;

// Re-export common types
pub use action::*;
pub use enums::*;
pub use event::*;
pub use execution::*;
pub use identity::*;
pub use inquiry::*;
pub use key::*;
pub use notification::*;
pub use pack::*;
pub use pack_test::*;
pub use rule::*;
pub use runtime::*;
pub use trigger::*;
pub use workflow::*;

/// Common ID type used throughout the system
pub type Id = i64;

/// JSON dictionary type
pub type JsonDict = JsonValue;

/// JSON schema type
pub type JsonSchema = JsonValue;

/// Enumeration types
pub mod enums {
    use serde::{Deserialize, Serialize};
    use sqlx::Type;
    use std::fmt;
    use std::str::FromStr;
    use utoipa::ToSchema;

    /// How parameters should be delivered to an action
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
    #[serde(rename_all = "lowercase")]
    pub enum ParameterDelivery {
        /// Pass parameters via stdin (secure, recommended for most cases)
        Stdin,
        /// Pass parameters via temporary file (secure, best for large payloads)
        File,
    }

    impl Default for ParameterDelivery {
        fn default() -> Self {
            Self::Stdin
        }
    }

    impl fmt::Display for ParameterDelivery {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::Stdin => write!(f, "stdin"),
                Self::File => write!(f, "file"),
            }
        }
    }

    impl FromStr for ParameterDelivery {
        type Err = String;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s.to_lowercase().as_str() {
                "stdin" => Ok(Self::Stdin),
                "file" => Ok(Self::File),
                _ => Err(format!("Invalid parameter delivery method: {}", s)),
            }
        }
    }

    impl sqlx::Type<sqlx::Postgres> for ParameterDelivery {
        fn type_info() -> sqlx::postgres::PgTypeInfo {
            <String as sqlx::Type<sqlx::Postgres>>::type_info()
        }
    }

    impl<'r> sqlx::Decode<'r, sqlx::Postgres> for ParameterDelivery {
        fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
            let s = <String as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
            s.parse().map_err(|e: String| e.into())
        }
    }

    impl<'q> sqlx::Encode<'q, sqlx::Postgres> for ParameterDelivery {
        fn encode_by_ref(
            &self,
            buf: &mut sqlx::postgres::PgArgumentBuffer,
        ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
            Ok(<String as sqlx::Encode<sqlx::Postgres>>::encode(self.to_string(), buf)?)
        }
    }

    /// Format for parameter serialization
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
    #[serde(rename_all = "lowercase")]
    pub enum ParameterFormat {
        /// KEY='VALUE' format (one per line)
        Dotenv,
        /// JSON object
        Json,
        /// YAML format
        Yaml,
    }

    impl Default for ParameterFormat {
        fn default() -> Self {
            Self::Json
        }
    }

    impl fmt::Display for ParameterFormat {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            match self {
                Self::Json => write!(f, "json"),
                Self::Dotenv => write!(f, "dotenv"),
                Self::Yaml => write!(f, "yaml"),
            }
        }
    }

    impl FromStr for ParameterFormat {
        type Err = String;

        fn from_str(s: &str) -> Result<Self, Self::Err> {
            match s.to_lowercase().as_str() {
                "json" => Ok(Self::Json),
                "dotenv" => Ok(Self::Dotenv),
                "yaml" => Ok(Self::Yaml),
                _ => Err(format!("Invalid parameter format: {}", s)),
            }
        }
    }

    impl sqlx::Type<sqlx::Postgres> for ParameterFormat {
        fn type_info() -> sqlx::postgres::PgTypeInfo {
            <String as sqlx::Type<sqlx::Postgres>>::type_info()
        }
    }

    impl<'r> sqlx::Decode<'r, sqlx::Postgres> for ParameterFormat {
        fn decode(value: sqlx::postgres::PgValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
            let s = <String as sqlx::Decode<sqlx::Postgres>>::decode(value)?;
            s.parse().map_err(|e: String| e.into())
        }
    }

    impl<'q> sqlx::Encode<'q, sqlx::Postgres> for ParameterFormat {
        fn encode_by_ref(
            &self,
            buf: &mut sqlx::postgres::PgArgumentBuffer,
        ) -> Result<sqlx::encode::IsNull, sqlx::error::BoxDynError> {
            Ok(<String as sqlx::Encode<sqlx::Postgres>>::encode(self.to_string(), buf)?)
        }
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
    #[sqlx(type_name = "worker_type_enum", rename_all = "lowercase")]
    #[serde(rename_all = "lowercase")]
    pub enum WorkerType {
        Local,
        Remote,
        Container,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
    #[sqlx(type_name = "worker_status_enum", rename_all = "lowercase")]
    #[serde(rename_all = "lowercase")]
    pub enum WorkerStatus {
        Active,
        Inactive,
        Busy,
        Error,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
    #[sqlx(type_name = "worker_role_enum", rename_all = "lowercase")]
    #[serde(rename_all = "lowercase")]
    pub enum WorkerRole {
        Action,
        Sensor,
        Hybrid,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
    #[sqlx(type_name = "enforcement_status_enum", rename_all = "lowercase")]
    #[serde(rename_all = "lowercase")]
    pub enum EnforcementStatus {
        Created,
        Processed,
        Disabled,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
    #[sqlx(type_name = "enforcement_condition_enum", rename_all = "lowercase")]
    #[serde(rename_all = "lowercase")]
    pub enum EnforcementCondition {
        Any,
        All,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
    #[sqlx(type_name = "execution_status_enum", rename_all = "lowercase")]
    #[serde(rename_all = "lowercase")]
    pub enum ExecutionStatus {
        Requested,
        Scheduling,
        Scheduled,
        Running,
        Completed,
        Failed,
        Canceling,
        Cancelled,
        Timeout,
        Abandoned,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
    #[sqlx(type_name = "inquiry_status_enum", rename_all = "lowercase")]
    #[serde(rename_all = "lowercase")]
    pub enum InquiryStatus {
        Pending,
        Responded,
        Timeout,
        Cancelled,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
    #[sqlx(type_name = "policy_method_enum", rename_all = "lowercase")]
    #[serde(rename_all = "lowercase")]
    pub enum PolicyMethod {
        Cancel,
        Enqueue,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
    #[sqlx(type_name = "owner_type_enum", rename_all = "lowercase")]
    #[serde(rename_all = "lowercase")]
    pub enum OwnerType {
        System,
        Identity,
        Pack,
        Action,
        Sensor,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
    #[sqlx(type_name = "notification_status_enum", rename_all = "lowercase")]
    #[serde(rename_all = "lowercase")]
    pub enum NotificationState {
        Created,
        Queued,
        Processing,
        Error,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
    #[sqlx(type_name = "artifact_type_enum", rename_all = "snake_case")]
    #[serde(rename_all = "snake_case")]
    pub enum ArtifactType {
        FileBinary,
        #[serde(rename = "file_datatable")]
        #[sqlx(rename = "file_datatable")]
        FileDataTable,
        FileImage,
        FileText,
        Other,
        Progress,
        Url,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
    #[sqlx(type_name = "artifact_retention_enum", rename_all = "lowercase")]
    #[serde(rename_all = "lowercase")]
    pub enum RetentionPolicyType {
        Versions,
        Days,
        Hours,
        Minutes,
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, ToSchema)]
    #[sqlx(type_name = "workflow_task_status_enum", rename_all = "lowercase")]
    #[serde(rename_all = "lowercase")]
    pub enum WorkflowTaskStatus {
        Pending,
        Running,
        Completed,
        Failed,
        Skipped,
        Cancelled,
    }
}

/// Pack model
pub mod pack {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Pack {
        pub id: Id,
        pub r#ref: String,
        pub label: String,
        pub description: Option<String>,
        pub version: String,
        pub conf_schema: JsonSchema,
        pub config: JsonDict,
        pub meta: JsonDict,
        pub tags: Vec<String>,
        pub runtime_deps: Vec<String>,
        pub is_standard: bool,
        pub installers: JsonDict,
        // Installation metadata (nullable for non-installed packs)
        pub source_type: Option<String>,
        pub source_url: Option<String>,
        pub source_ref: Option<String>,
        pub checksum: Option<String>,
        pub checksum_verified: Option<bool>,
        pub installed_at: Option<DateTime<Utc>>,
        pub installed_by: Option<Id>,
        pub installation_method: Option<String>,
        pub storage_path: Option<String>,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }
}

/// Runtime model
pub mod runtime {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Runtime {
        pub id: Id,
        pub r#ref: String,
        pub pack: Option<Id>,
        pub pack_ref: Option<String>,
        pub description: Option<String>,
        pub name: String,
        pub distributions: JsonDict,
        pub installation: Option<JsonDict>,
        pub installers: JsonDict,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Worker {
        pub id: Id,
        pub name: String,
        pub worker_type: WorkerType,
        pub worker_role: WorkerRole,
        pub runtime: Option<Id>,
        pub host: Option<String>,
        pub port: Option<i32>,
        pub status: Option<WorkerStatus>,
        pub capabilities: Option<JsonDict>,
        pub meta: Option<JsonDict>,
        pub last_heartbeat: Option<DateTime<Utc>>,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }
}

/// Trigger model
pub mod trigger {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Trigger {
        pub id: Id,
        pub r#ref: String,
        pub pack: Option<Id>,
        pub pack_ref: Option<String>,
        pub label: String,
        pub description: Option<String>,
        pub enabled: bool,
        pub param_schema: Option<JsonSchema>,
        pub out_schema: Option<JsonSchema>,
        pub webhook_enabled: bool,
        pub webhook_key: Option<String>,
        pub webhook_config: Option<JsonDict>,
        pub is_adhoc: bool,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Sensor {
        pub id: Id,
        pub r#ref: String,
        pub pack: Option<Id>,
        pub pack_ref: Option<String>,
        pub label: String,
        pub description: String,
        pub entrypoint: String,
        pub runtime: Id,
        pub runtime_ref: String,
        pub trigger: Id,
        pub trigger_ref: String,
        pub enabled: bool,
        pub param_schema: Option<JsonSchema>,
        pub config: Option<JsonValue>,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }
}

/// Action model
pub mod action {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Action {
        pub id: Id,
        pub r#ref: String,
        pub pack: Id,
        pub pack_ref: String,
        pub label: String,
        pub description: String,
        pub entrypoint: String,
        pub runtime: Option<Id>,
        pub param_schema: Option<JsonSchema>,
        pub out_schema: Option<JsonSchema>,
        pub is_workflow: bool,
        pub workflow_def: Option<Id>,
        pub is_adhoc: bool,
        #[sqlx(default)]
        pub parameter_delivery: ParameterDelivery,
        #[sqlx(default)]
        pub parameter_format: ParameterFormat,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Policy {
        pub id: Id,
        pub r#ref: String,
        pub pack: Option<Id>,
        pub pack_ref: Option<String>,
        pub action: Option<Id>,
        pub action_ref: Option<String>,
        pub parameters: Vec<String>,
        pub method: PolicyMethod,
        pub threshold: i32,
        pub name: String,
        pub description: Option<String>,
        pub tags: Vec<String>,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }
}

/// Rule model
pub mod rule {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Rule {
        pub id: Id,
        pub r#ref: String,
        pub pack: Id,
        pub pack_ref: String,
        pub label: String,
        pub description: String,
        pub action: Id,
        pub action_ref: String,
        pub trigger: Id,
        pub trigger_ref: String,
        pub conditions: JsonValue,
        pub action_params: JsonValue,
        pub trigger_params: JsonValue,
        pub enabled: bool,
        pub is_adhoc: bool,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }

    /// Webhook event log for auditing and analytics
    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct WebhookEventLog {
        pub id: Id,
        pub trigger_id: Id,
        pub trigger_ref: String,
        pub webhook_key: String,
        pub event_id: Option<Id>,
        pub source_ip: Option<String>,
        pub user_agent: Option<String>,
        pub payload_size_bytes: Option<i32>,
        pub headers: Option<JsonValue>,
        pub status_code: i32,
        pub error_message: Option<String>,
        pub processing_time_ms: Option<i32>,
        pub hmac_verified: Option<bool>,
        pub rate_limited: bool,
        pub ip_allowed: Option<bool>,
        pub created: DateTime<Utc>,
    }
}

pub mod event {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Event {
        pub id: Id,
        pub trigger: Option<Id>,
        pub trigger_ref: String,
        pub config: Option<JsonDict>,
        pub payload: Option<JsonDict>,
        pub source: Option<Id>,
        pub source_ref: Option<String>,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
        pub rule: Option<Id>,
        pub rule_ref: Option<String>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Enforcement {
        pub id: Id,
        pub rule: Option<Id>,
        pub rule_ref: String,
        pub trigger_ref: String,
        pub config: Option<JsonDict>,
        pub event: Option<Id>,
        pub status: EnforcementStatus,
        pub payload: JsonDict,
        pub condition: EnforcementCondition,
        pub conditions: JsonValue,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }
}

/// Execution model
pub mod execution {
    use super::*;

    /// Workflow-specific task metadata
    /// Stored as JSONB in the execution table's workflow_task column
    ///
    /// This metadata is only populated for workflow task executions.
    /// It provides a direct link to the workflow_execution record for efficient queries.
    ///
    /// Note: The `workflow_execution` field here is separate from `Execution.parent`.
    /// - `parent`: Generic execution hierarchy (used for all execution types)
    /// - `workflow_execution`: Specific link to workflow orchestration state
    ///
    /// See docs/execution-hierarchy.md for detailed explanation.
    #[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
    #[cfg_attr(test, derive(Eq))]
    pub struct WorkflowTaskMetadata {
        /// ID of the workflow_execution record (orchestration state)
        pub workflow_execution: Id,

        /// Task name within the workflow
        pub task_name: String,

        /// Index for with-items iteration (0-based)
        pub task_index: Option<i32>,

        /// Batch number for batched with-items processing
        pub task_batch: Option<i32>,

        /// Current retry attempt count
        pub retry_count: i32,

        /// Maximum retries allowed
        pub max_retries: i32,

        /// Scheduled time for next retry
        pub next_retry_at: Option<DateTime<Utc>>,

        /// Timeout in seconds
        pub timeout_seconds: Option<i32>,

        /// Whether task timed out
        pub timed_out: bool,

        /// Task execution duration in milliseconds
        pub duration_ms: Option<i64>,

        /// When task started executing
        pub started_at: Option<DateTime<Utc>>,

        /// When task completed
        pub completed_at: Option<DateTime<Utc>>,
    }

    /// Represents an action execution with support for hierarchical relationships
    ///
    /// Executions support two types of parent-child relationships:
    ///
    /// 1. **Generic hierarchy** (`parent` field):
    ///    - Used for all execution types (workflows, actions, nested workflows)
    ///    - Enables generic tree traversal queries
    ///    - Example: action spawning child actions
    ///
    /// 2. **Workflow-specific** (`workflow_task` metadata):
    ///    - Only populated for workflow task executions
    ///    - Provides direct link to workflow orchestration state
    ///    - Example: task within a workflow execution
    ///
    /// For workflow tasks, both fields are populated and serve different purposes.
    /// See docs/execution-hierarchy.md for detailed explanation.
    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Execution {
        pub id: Id,
        pub action: Option<Id>,
        pub action_ref: String,
        pub config: Option<JsonDict>,

        /// Environment variables for this execution (string -> string mapping)
        /// These are set as environment variables in the action's process.
        /// Separate from parameters which are passed via stdin/file.
        pub env_vars: Option<JsonDict>,

        /// Parent execution ID (generic hierarchy for all execution types)
        ///
        /// Used for:
        /// - Workflow tasks: parent is the workflow's execution
        /// - Child actions: parent is the spawning action
        /// - Nested workflows: parent is the outer workflow
        pub parent: Option<Id>,

        pub enforcement: Option<Id>,
        pub executor: Option<Id>,
        pub status: ExecutionStatus,
        pub result: Option<JsonDict>,

        /// Workflow task metadata (only populated for workflow task executions)
        ///
        /// Provides direct access to workflow orchestration state without JOINs.
        /// The `workflow_execution` field within this metadata is separate from
        /// the `parent` field above, as they serve different query patterns.
        #[sqlx(json)]
        pub workflow_task: Option<WorkflowTaskMetadata>,

        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }

    impl Execution {
        /// Check if this execution is a workflow task
        ///
        /// Returns `true` if this execution represents a task within a workflow,
        /// as opposed to a standalone action execution or the workflow itself.
        pub fn is_workflow_task(&self) -> bool {
            self.workflow_task.is_some()
        }

        /// Get the workflow execution ID if this is a workflow task
        ///
        /// Returns the ID of the workflow_execution record that contains
        /// the orchestration state (task graph, variables, etc.) for this task.
        pub fn workflow_execution_id(&self) -> Option<Id> {
            self.workflow_task.as_ref().map(|wt| wt.workflow_execution)
        }

        /// Check if this execution has child executions
        ///
        /// Note: This only checks if the parent field is populated.
        /// To actually query for children, use ExecutionRepository::find_by_parent().
        pub fn is_parent(&self) -> bool {
            // This would need a query to check, so we provide a helper for the inverse
            self.parent.is_some()
        }

        /// Get the task name if this is a workflow task
        pub fn task_name(&self) -> Option<&str> {
            self.workflow_task.as_ref().map(|wt| wt.task_name.as_str())
        }
    }
}

/// Inquiry model
pub mod inquiry {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Inquiry {
        pub id: Id,
        pub execution: Id,
        pub prompt: String,
        pub response_schema: Option<JsonSchema>,
        pub assigned_to: Option<Id>,
        pub status: InquiryStatus,
        pub response: Option<JsonDict>,
        pub timeout_at: Option<DateTime<Utc>>,
        pub responded_at: Option<DateTime<Utc>>,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }
}

/// Identity and permissions
pub mod identity {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Identity {
        pub id: Id,
        pub login: String,
        pub display_name: Option<String>,
        pub password_hash: Option<String>,
        pub attributes: JsonDict,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct PermissionSet {
        pub id: Id,
        pub r#ref: String,
        pub pack: Option<Id>,
        pub pack_ref: Option<String>,
        pub label: Option<String>,
        pub description: Option<String>,
        pub grants: JsonValue,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct PermissionAssignment {
        pub id: Id,
        pub identity: Id,
        pub permset: Id,
        pub created: DateTime<Utc>,
    }
}

/// Key/Value storage
pub mod key {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Key {
        pub id: Id,
        pub r#ref: String,
        pub owner_type: OwnerType,
        pub owner: Option<String>,
        pub owner_identity: Option<Id>,
        pub owner_pack: Option<Id>,
        pub owner_pack_ref: Option<String>,
        pub owner_action: Option<Id>,
        pub owner_action_ref: Option<String>,
        pub owner_sensor: Option<Id>,
        pub owner_sensor_ref: Option<String>,
        pub name: String,
        pub encrypted: bool,
        pub encryption_key_hash: Option<String>,
        pub value: String,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }
}

/// Notification model
pub mod notification {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Notification {
        pub id: Id,
        pub channel: String,
        pub entity_type: String,
        pub entity: String,
        pub activity: String,
        pub state: NotificationState,
        pub content: Option<JsonDict>,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }
}

/// Artifact model
pub mod artifact {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct Artifact {
        pub id: Id,
        pub r#ref: String,
        pub scope: OwnerType,
        pub owner: String,
        pub r#type: ArtifactType,
        pub retention_policy: RetentionPolicyType,
        pub retention_limit: i32,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }
}

/// Workflow orchestration models
pub mod workflow {
    use super::*;

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct WorkflowDefinition {
        pub id: Id,
        pub r#ref: String,
        pub pack: Id,
        pub pack_ref: String,
        pub label: String,
        pub description: Option<String>,
        pub version: String,
        pub param_schema: Option<JsonSchema>,
        pub out_schema: Option<JsonSchema>,
        pub definition: JsonDict,
        pub tags: Vec<String>,
        pub enabled: bool,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }

    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    pub struct WorkflowExecution {
        pub id: Id,
        pub execution: Id,
        pub workflow_def: Id,
        pub current_tasks: Vec<String>,
        pub completed_tasks: Vec<String>,
        pub failed_tasks: Vec<String>,
        pub skipped_tasks: Vec<String>,
        pub variables: JsonDict,
        pub task_graph: JsonDict,
        pub status: ExecutionStatus,
        pub error_message: Option<String>,
        pub paused: bool,
        pub pause_reason: Option<String>,
        pub created: DateTime<Utc>,
        pub updated: DateTime<Utc>,
    }
}

/// Pack testing models
pub mod pack_test {
    use super::*;
    use utoipa::ToSchema;

    /// Pack test execution record
    #[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
    #[serde(rename_all = "camelCase")]
    pub struct PackTestExecution {
        pub id: Id,
        pub pack_id: Id,
        pub pack_version: String,
        pub execution_time: DateTime<Utc>,
        pub trigger_reason: String,
        pub total_tests: i32,
        pub passed: i32,
        pub failed: i32,
        pub skipped: i32,
        pub pass_rate: f64,
        pub duration_ms: i64,
        pub result: JsonValue,
        pub created: DateTime<Utc>,
    }

    /// Pack test result structure (not from DB, used for test execution)
    #[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
    #[serde(rename_all = "camelCase")]
    pub struct PackTestResult {
        pub pack_ref: String,
        pub pack_version: String,
        pub execution_time: DateTime<Utc>,
        pub status: String,
        pub total_tests: i32,
        pub passed: i32,
        pub failed: i32,
        pub skipped: i32,
        pub pass_rate: f64,
        pub duration_ms: i64,
        pub test_suites: Vec<TestSuiteResult>,
    }

    /// Test suite result (collection of test cases)
    #[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
    #[serde(rename_all = "camelCase")]
    pub struct TestSuiteResult {
        pub name: String,
        pub runner_type: String,
        pub total: i32,
        pub passed: i32,
        pub failed: i32,
        pub skipped: i32,
        pub duration_ms: i64,
        pub test_cases: Vec<TestCaseResult>,
    }

    /// Individual test case result
    #[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
    #[serde(rename_all = "camelCase")]
    pub struct TestCaseResult {
        pub name: String,
        pub status: TestStatus,
        pub duration_ms: i64,
        pub error_message: Option<String>,
        pub stdout: Option<String>,
        pub stderr: Option<String>,
    }

    /// Test status enum
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
    #[serde(rename_all = "lowercase")]
    pub enum TestStatus {
        Passed,
        Failed,
        Skipped,
        Error,
    }

    /// Pack test summary view
    #[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
    #[serde(rename_all = "camelCase")]
    pub struct PackTestSummary {
        pub pack_id: Id,
        pub pack_ref: String,
        pub pack_label: String,
        pub test_execution_id: Id,
        pub pack_version: String,
        pub test_time: DateTime<Utc>,
        pub trigger_reason: String,
        pub total_tests: i32,
        pub passed: i32,
        pub failed: i32,
        pub skipped: i32,
        pub pass_rate: f64,
        pub duration_ms: i64,
    }

    /// Pack latest test view
    #[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
    #[serde(rename_all = "camelCase")]
    pub struct PackLatestTest {
        pub pack_id: Id,
        pub pack_ref: String,
        pub pack_label: String,
        pub test_execution_id: Id,
        pub pack_version: String,
        pub test_time: DateTime<Utc>,
        pub trigger_reason: String,
        pub total_tests: i32,
        pub passed: i32,
        pub failed: i32,
        pub skipped: i32,
        pub pass_rate: f64,
        pub duration_ms: i64,
    }

    /// Pack test statistics
    #[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
    #[serde(rename_all = "camelCase")]
    pub struct PackTestStats {
        pub total_executions: i64,
        pub successful_executions: i64,
        pub failed_executions: i64,
        pub avg_pass_rate: Option<f64>,
        pub avg_duration_ms: Option<i64>,
        pub last_test_time: Option<DateTime<Utc>>,
        pub last_test_passed: Option<bool>,
    }
}
