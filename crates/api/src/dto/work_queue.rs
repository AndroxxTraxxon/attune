//! Work queue DTOs for API requests and responses.

use std::borrow::Cow;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use utoipa::{IntoParams, ToSchema};
use validator::{Validate, ValidationError};

use attune_common::{
    models::{
        Id, WorkQueue, WorkQueueBatchMode, WorkQueueItem, WorkQueueItemStatus,
        WorkQueueUpdateStrategy,
    },
    queue_definition::validate_work_queue_config,
    schema::RefValidator,
};

use crate::dto::runtime::NullableStringPatch;

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct CreateWorkQueueRequest {
    #[validate(custom(function = "validate_queue_ref_field"))]
    #[schema(example = "core.inbox")]
    pub r#ref: String,

    #[validate(custom(function = "validate_pack_ref_field"))]
    #[schema(example = "core", nullable = true)]
    pub pack_ref: Option<String>,

    #[validate(length(min = 1, max = 255))]
    #[schema(example = "Core Inbox")]
    pub label: String,

    #[schema(
        example = "Dispatches inbound work items to the core processor",
        nullable = true
    )]
    pub description: Option<String>,

    #[schema(example = true, default = true)]
    #[serde(default = "default_true")]
    pub enabled: bool,

    #[validate(custom(function = "validate_action_ref_field"))]
    #[schema(example = "core.process_item")]
    pub dispatch_action_ref: String,

    #[schema(example = 0, default = 0)]
    #[serde(default)]
    pub default_priority: i32,

    #[schema(example = false, default = false)]
    #[serde(default)]
    pub allow_pending_update: bool,

    #[serde(default)]
    pub update_strategy: WorkQueueUpdateStrategy,

    #[serde(default)]
    pub batch_mode: WorkQueueBatchMode,

    #[validate(custom(function = "validate_queue_config_field"))]
    #[schema(value_type = Object, example = json!({"dispatch": {"concurrency": {"source": "literal", "value": 5}}}))]
    #[serde(default = "default_json_object")]
    pub config: JsonValue,
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct UpdateWorkQueueRequest {
    #[validate(custom(function = "validate_pack_ref_patch"))]
    pub pack_ref: Option<NullableStringPatch>,

    #[validate(length(min = 1, max = 255))]
    #[schema(example = "Core Inbox (Updated)")]
    pub label: Option<String>,

    pub description: Option<NullableStringPatch>,

    #[schema(example = false)]
    pub enabled: Option<bool>,

    #[validate(custom(function = "validate_action_ref_field"))]
    #[schema(example = "core.process_item")]
    pub dispatch_action_ref: Option<String>,

    #[schema(example = 10)]
    pub default_priority: Option<i32>,

    #[schema(example = true)]
    pub allow_pending_update: Option<bool>,

    pub update_strategy: Option<WorkQueueUpdateStrategy>,

    pub batch_mode: Option<WorkQueueBatchMode>,

    #[validate(custom(function = "validate_queue_config_field"))]
    #[schema(value_type = Object, nullable = true)]
    pub config: Option<JsonValue>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct WorkQueueResponse {
    #[schema(example = 1)]
    pub id: Id,
    #[schema(example = "core.inbox")]
    pub r#ref: String,
    #[schema(example = 1, nullable = true)]
    pub pack: Option<Id>,
    #[schema(example = "core", nullable = true)]
    pub pack_ref: Option<String>,
    #[schema(example = false)]
    pub is_adhoc: bool,
    #[schema(example = "Core Inbox")]
    pub label: String,
    #[schema(
        example = "Dispatches inbound work items to the core processor",
        nullable = true
    )]
    pub description: Option<String>,
    #[schema(example = true)]
    pub enabled: bool,
    #[schema(example = 42, nullable = true)]
    pub dispatch_action: Option<Id>,
    #[schema(example = "core.process_item")]
    pub dispatch_action_ref: String,
    #[schema(example = 0)]
    pub default_priority: i32,
    #[schema(example = false)]
    pub allow_pending_update: bool,
    pub update_strategy: WorkQueueUpdateStrategy,
    pub batch_mode: WorkQueueBatchMode,
    #[schema(value_type = Object)]
    pub config: JsonValue,
    #[schema(example = "2024-01-13T10:30:00Z")]
    pub created: DateTime<Utc>,
    #[schema(example = "2024-01-13T10:30:00Z")]
    pub updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct WorkQueueSummary {
    #[schema(example = 1)]
    pub id: Id,
    #[schema(example = "core.inbox")]
    pub r#ref: String,
    #[schema(example = "core", nullable = true)]
    pub pack_ref: Option<String>,
    #[schema(example = false)]
    pub is_adhoc: bool,
    #[schema(example = "Core Inbox")]
    pub label: String,
    #[schema(
        example = "Dispatches inbound work items to the core processor",
        nullable = true
    )]
    pub description: Option<String>,
    #[schema(example = true)]
    pub enabled: bool,
    #[schema(example = "core.process_item")]
    pub dispatch_action_ref: String,
    #[schema(example = "2024-01-13T10:30:00Z")]
    pub created: DateTime<Utc>,
    #[schema(example = "2024-01-13T10:30:00Z")]
    pub updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, IntoParams)]
pub struct WorkQueueQueryParams {
    #[param(example = true)]
    pub enabled: Option<bool>,

    #[param(example = false)]
    pub is_adhoc: Option<bool>,

    #[param(example = "inbox")]
    pub search: Option<String>,

    #[serde(default = "default_page")]
    #[param(example = 1, minimum = 1)]
    pub page: u32,

    #[serde(default = "default_per_page")]
    #[param(example = 50, minimum = 1, maximum = 100)]
    pub per_page: u32,
}

impl WorkQueueQueryParams {
    pub fn offset(&self) -> u32 {
        (self.page - 1) * self.per_page
    }

    pub fn limit(&self) -> u32 {
        self.per_page
    }
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct EnqueueWorkQueueItemRequest {
    #[validate(custom(function = "validate_item_key_field"))]
    #[schema(example = "order-123", nullable = true)]
    pub item_key: Option<String>,

    #[schema(example = 5, nullable = true)]
    pub priority: Option<i32>,

    #[schema(value_type = Object, example = json!({"order_id": 123, "customer": "alice"}))]
    pub payload: JsonValue,

    #[schema(value_type = Object, example = json!({"source": "api"}))]
    #[serde(default = "default_json_object")]
    pub metadata: JsonValue,

    #[validate(length(min = 1, max = 255))]
    #[schema(example = "api", default = "api")]
    #[serde(default = "default_enqueue_source")]
    pub enqueue_source: String,
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct UpdateWorkQueueItemRequest {
    #[validate(custom(function = "validate_optional_item_key_patch"))]
    pub item_key: Option<NullableStringPatch>,

    #[schema(example = 10)]
    pub priority: Option<i32>,

    #[schema(value_type = Object, nullable = true, example = json!({"status": "retrying"}))]
    pub payload: Option<JsonValue>,

    #[schema(value_type = Object, nullable = true, example = json!({"attempt": 2}))]
    pub metadata: Option<JsonValue>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct WorkQueueItemResponse {
    #[schema(example = 1)]
    pub id: Id,
    #[schema(example = 10)]
    pub queue: Id,
    #[schema(example = "core.inbox")]
    pub queue_ref: String,
    #[schema(example = "order-123", nullable = true)]
    pub item_key: Option<String>,
    #[schema(example = 5)]
    pub priority: i32,
    pub status: WorkQueueItemStatus,
    #[schema(value_type = Object)]
    pub payload: JsonValue,
    #[schema(value_type = Object)]
    pub metadata: JsonValue,
    #[schema(example = "api")]
    pub enqueue_source: String,
    #[schema(example = 42, nullable = true)]
    pub requested_by_identity: Option<Id>,
    #[schema(example = 99, nullable = true)]
    pub requested_by_execution: Option<Id>,
    #[schema(example = 100, nullable = true)]
    pub requested_by_enforcement: Option<Id>,
    #[schema(example = 101, nullable = true)]
    pub leased_execution: Option<Id>,
    #[schema(nullable = true)]
    pub lease_token: Option<uuid::Uuid>,
    #[schema(example = "2024-01-13T10:30:00Z", nullable = true)]
    pub lease_expires_at: Option<DateTime<Utc>>,
    #[schema(example = 0)]
    pub attempt_count: i32,
    #[schema(value_type = Object, nullable = true)]
    pub last_error: Option<JsonValue>,
    #[schema(value_type = Object, nullable = true)]
    pub ack_summary: Option<JsonValue>,
    #[schema(example = "2024-01-13T10:30:00Z")]
    pub created: DateTime<Utc>,
    #[schema(example = "2024-01-13T10:30:00Z")]
    pub updated: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, IntoParams)]
pub struct WorkQueueItemQueryParams {
    #[param(example = "order-123")]
    pub item_key: Option<String>,

    #[param(example = "api")]
    pub enqueue_source: Option<String>,

    #[serde(default)]
    #[param(value_type = Vec<WorkQueueItemStatus>, example = json!(["queued", "retry"]))]
    pub statuses: Vec<WorkQueueItemStatus>,

    #[serde(default = "default_page")]
    #[param(example = 1, minimum = 1)]
    pub page: u32,

    #[serde(default = "default_per_page")]
    #[param(example = 50, minimum = 1, maximum = 100)]
    pub per_page: u32,
}

impl WorkQueueItemQueryParams {
    pub fn offset(&self) -> u32 {
        (self.page - 1) * self.per_page
    }

    pub fn limit(&self) -> u32 {
        self.per_page
    }
}

impl From<WorkQueue> for WorkQueueResponse {
    fn from(queue: WorkQueue) -> Self {
        Self {
            id: queue.id,
            r#ref: queue.r#ref,
            pack: queue.pack,
            pack_ref: queue.pack_ref,
            is_adhoc: queue.is_adhoc,
            label: queue.label,
            description: queue.description,
            enabled: queue.enabled,
            dispatch_action: queue.dispatch_action,
            dispatch_action_ref: queue.dispatch_action_ref,
            default_priority: queue.default_priority,
            allow_pending_update: queue.allow_pending_update,
            update_strategy: queue.update_strategy,
            batch_mode: queue.batch_mode,
            config: queue.config,
            created: queue.created,
            updated: queue.updated,
        }
    }
}

impl From<WorkQueue> for WorkQueueSummary {
    fn from(queue: WorkQueue) -> Self {
        Self {
            id: queue.id,
            r#ref: queue.r#ref,
            pack_ref: queue.pack_ref,
            is_adhoc: queue.is_adhoc,
            label: queue.label,
            description: queue.description,
            enabled: queue.enabled,
            dispatch_action_ref: queue.dispatch_action_ref,
            created: queue.created,
            updated: queue.updated,
        }
    }
}

impl From<WorkQueueItem> for WorkQueueItemResponse {
    fn from(item: WorkQueueItem) -> Self {
        Self {
            id: item.id,
            queue: item.queue,
            queue_ref: item.queue_ref,
            item_key: item.item_key,
            priority: item.priority,
            status: item.status,
            payload: item.payload,
            metadata: item.metadata,
            enqueue_source: item.enqueue_source,
            requested_by_identity: item.requested_by_identity,
            requested_by_execution: item.requested_by_execution,
            requested_by_enforcement: item.requested_by_enforcement,
            leased_execution: item.leased_execution,
            lease_token: item.lease_token,
            lease_expires_at: item.lease_expires_at,
            attempt_count: item.attempt_count,
            last_error: item.last_error,
            ack_summary: item.ack_summary,
            created: item.created,
            updated: item.updated,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_json_object() -> JsonValue {
    serde_json::json!({})
}

fn default_enqueue_source() -> String {
    "api".to_string()
}

fn default_page() -> u32 {
    1
}

fn default_per_page() -> u32 {
    50
}

fn validation_error(code: &'static str, message: String) -> ValidationError {
    let mut error = ValidationError::new(code);
    error.message = Some(Cow::Owned(message));
    error
}

fn validate_queue_ref_field(value: &str) -> Result<(), ValidationError> {
    RefValidator::validate_work_queue_ref(value)
        .map_err(|e| validation_error("queue_ref", e.to_string()))
}

fn validate_action_ref_field(value: &str) -> Result<(), ValidationError> {
    RefValidator::validate_component_ref(value)
        .map_err(|e| validation_error("dispatch_action_ref", e.to_string()))
}

fn validate_pack_ref_field(value: &String) -> Result<(), ValidationError> {
    RefValidator::validate_pack_ref(value).map_err(|e| validation_error("pack_ref", e.to_string()))
}

fn validate_pack_ref_patch(value: &NullableStringPatch) -> Result<(), ValidationError> {
    if let NullableStringPatch::Set(value) = value {
        RefValidator::validate_pack_ref(value)
            .map_err(|e| validation_error("pack_ref", e.to_string()))?;
    }
    Ok(())
}

fn validate_item_key_field(value: &String) -> Result<(), ValidationError> {
    validate_item_key(value)
}

fn validate_optional_item_key_patch(value: &NullableStringPatch) -> Result<(), ValidationError> {
    if let NullableStringPatch::Set(value) = value {
        validate_item_key(value)?;
    }
    Ok(())
}

fn validate_item_key(value: &str) -> Result<(), ValidationError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(validation_error(
            "item_key",
            "item_key cannot be empty".to_string(),
        ));
    }
    if trimmed.len() > 255 {
        return Err(validation_error(
            "item_key",
            "item_key must be at most 255 characters".to_string(),
        ));
    }
    Ok(())
}

fn validate_queue_config_field(value: &JsonValue) -> Result<(), ValidationError> {
    validate_work_queue_config(value)
        .map(|_| ())
        .map_err(|e| validation_error("config", e.to_string()))
}
