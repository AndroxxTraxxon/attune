//! Trigger and Sensor DTOs for API requests and responses

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use utoipa::ToSchema;
use validator::Validate;

/// Request DTO for creating a new trigger
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct CreateTriggerRequest {
    /// Unique reference identifier (e.g., "core.webhook", "system.timer")
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "core.webhook")]
    pub r#ref: String,

    /// Optional pack reference this trigger belongs to
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "core")]
    pub pack_ref: Option<String>,

    /// Human-readable label
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "Webhook Trigger")]
    pub label: String,

    /// Trigger description
    #[schema(example = "Triggers when a webhook is received")]
    pub description: Option<String>,

    /// Parameter schema (StackStorm-style) defining trigger configuration with inline required/secret
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Object, nullable = true, example = json!({"url": {"type": "string", "description": "Webhook URL", "required": true}}))]
    pub param_schema: Option<JsonValue>,

    /// Output schema (flat format) defining event data structure with inline required/secret
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Object, nullable = true, example = json!({"payload": {"type": "object", "description": "Event payload data", "required": true}}))]
    pub out_schema: Option<JsonValue>,

    /// Whether the trigger is enabled
    #[serde(default = "default_true")]
    #[schema(example = true)]
    pub enabled: bool,
}

/// Request DTO for updating a trigger
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct UpdateTriggerRequest {
    /// Human-readable label
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "Webhook Trigger (Updated)")]
    pub label: Option<String>,

    /// Trigger description
    #[schema(example = "Updated webhook trigger description")]
    pub description: Option<TriggerStringPatch>,

    /// Parameter schema (StackStorm-style with inline required/secret)
    #[schema(value_type = Object, nullable = true)]
    pub param_schema: Option<TriggerJsonPatch>,

    /// Output schema
    #[schema(value_type = Object, nullable = true)]
    pub out_schema: Option<TriggerJsonPatch>,

    /// Whether the trigger is enabled
    #[schema(example = true)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(tag = "op", content = "value", rename_all = "snake_case")]
pub enum TriggerStringPatch {
    Set(String),
    Clear,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(tag = "op", content = "value", rename_all = "snake_case")]
pub enum TriggerJsonPatch {
    Set(JsonValue),
    Clear,
}

/// Response DTO for trigger information
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TriggerResponse {
    /// Trigger ID
    #[schema(example = 1)]
    pub id: i64,

    /// Unique reference identifier
    #[schema(example = "core.webhook")]
    pub r#ref: String,

    /// Pack ID (optional)
    #[schema(example = 1)]
    pub pack: Option<i64>,

    /// Pack reference (optional)
    #[schema(example = "core")]
    pub pack_ref: Option<String>,

    /// Human-readable label
    #[schema(example = "Webhook Trigger")]
    pub label: String,

    /// Trigger description
    #[schema(example = "Triggers when a webhook is received")]
    pub description: Option<String>,

    /// Whether the trigger is enabled
    #[schema(example = true)]
    pub enabled: bool,

    /// Parameter schema (StackStorm-style with inline required/secret)
    #[schema(value_type = Object, nullable = true)]
    pub param_schema: Option<JsonValue>,

    /// Output schema
    #[schema(value_type = Object, nullable = true)]
    pub out_schema: Option<JsonValue>,

    /// Whether webhooks are enabled for this trigger
    #[schema(example = false)]
    pub webhook_enabled: bool,

    /// Webhook key (only present if webhooks are enabled)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(example = "wh_k7j2n9p4m8q1r5w3x6z0a2b5c8d1e4f7g9h2")]
    pub webhook_key: Option<String>,

    /// Whether this is an ad-hoc trigger (not from pack installation)
    #[schema(example = false)]
    pub is_adhoc: bool,

    /// Creation timestamp
    #[schema(example = "2024-01-13T10:30:00Z")]
    pub created: DateTime<Utc>,

    /// Last update timestamp
    #[schema(example = "2024-01-13T10:30:00Z")]
    pub updated: DateTime<Utc>,
}

/// Simplified trigger response (for list endpoints)
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct TriggerSummary {
    /// Trigger ID
    #[schema(example = 1)]
    pub id: i64,

    /// Unique reference identifier
    #[schema(example = "core.webhook")]
    pub r#ref: String,

    /// Pack reference (optional)
    #[schema(example = "core")]
    pub pack_ref: Option<String>,

    /// Human-readable label
    #[schema(example = "Webhook Trigger")]
    pub label: String,

    /// Trigger description
    #[schema(example = "Triggers when a webhook is received")]
    pub description: Option<String>,

    /// Whether the trigger is enabled
    #[schema(example = true)]
    pub enabled: bool,

    /// Whether webhooks are enabled for this trigger
    #[schema(example = false)]
    pub webhook_enabled: bool,

    /// Creation timestamp
    #[schema(example = "2024-01-13T10:30:00Z")]
    pub created: DateTime<Utc>,

    /// Last update timestamp
    #[schema(example = "2024-01-13T10:30:00Z")]
    pub updated: DateTime<Utc>,
}

/// Request DTO for creating a new sensor
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct CreateSensorRequest {
    /// Unique reference identifier (e.g., "mypack.cpu_monitor")
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "monitoring.cpu_sensor")]
    pub r#ref: String,

    /// Pack reference this sensor belongs to
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "monitoring")]
    pub pack_ref: String,

    /// Human-readable label
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "CPU Monitoring Sensor")]
    pub label: String,

    /// Sensor description
    #[validate(length(min = 1))]
    #[schema(example = "Monitors CPU usage and generates events")]
    pub description: String,

    /// Entry point for sensor execution (e.g., path to script, function name)
    #[validate(length(min = 1, max = 1024))]
    #[schema(example = "/sensors/monitoring/cpu_monitor.py")]
    pub entrypoint: String,

    /// Runtime reference for this sensor
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "python3")]
    pub runtime_ref: String,

    /// Trigger reference this sensor monitors for
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "monitoring.cpu_threshold")]
    pub trigger_ref: String,

    /// Parameter schema (flat format) for sensor configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Object, nullable = true, example = json!({"threshold": {"type": "number", "description": "Alert threshold", "required": true}}))]
    pub param_schema: Option<JsonValue>,

    /// Configuration values for this sensor instance (conforms to param_schema)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Object, nullable = true, example = json!({"interval": 60, "threshold": 80}))]
    pub config: Option<JsonValue>,

    /// Whether the sensor is enabled
    #[serde(default = "default_true")]
    #[schema(example = true)]
    pub enabled: bool,
}

/// Request DTO for updating a sensor
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct UpdateSensorRequest {
    /// Human-readable label
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "CPU Monitoring Sensor (Updated)")]
    pub label: Option<String>,

    /// Sensor description
    #[validate(length(min = 1))]
    #[schema(example = "Enhanced CPU monitoring with alerts")]
    pub description: Option<String>,

    /// Entry point for sensor execution
    #[validate(length(min = 1, max = 1024))]
    #[schema(example = "/sensors/monitoring/cpu_monitor_v2.py")]
    pub entrypoint: Option<String>,

    /// Parameter schema (StackStorm-style with inline required/secret)
    #[schema(value_type = Object, nullable = true)]
    pub param_schema: Option<SensorJsonPatch>,

    /// Whether the sensor is enabled
    #[schema(example = false)]
    pub enabled: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(tag = "op", content = "value", rename_all = "snake_case")]
pub enum SensorJsonPatch {
    Set(JsonValue),
    Clear,
}

/// Response DTO for sensor information
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SensorResponse {
    /// Sensor ID
    #[schema(example = 1)]
    pub id: i64,

    /// Unique reference identifier
    #[schema(example = "monitoring.cpu_sensor")]
    pub r#ref: String,

    /// Pack ID (optional)
    #[schema(example = 1)]
    pub pack: Option<i64>,

    /// Pack reference (optional)
    #[schema(example = "monitoring")]
    pub pack_ref: Option<String>,

    /// Human-readable label
    #[schema(example = "CPU Monitoring Sensor")]
    pub label: String,

    /// Sensor description
    #[schema(example = "Monitors CPU usage and generates events")]
    pub description: String,

    /// Entry point
    #[schema(example = "/sensors/monitoring/cpu_monitor.py")]
    pub entrypoint: String,

    /// Runtime ID
    #[schema(example = 1)]
    pub runtime: i64,

    /// Runtime reference
    #[schema(example = "python3")]
    pub runtime_ref: String,

    /// Trigger ID
    #[schema(example = 1)]
    pub trigger: i64,

    /// Trigger reference
    #[schema(example = "monitoring.cpu_threshold")]
    pub trigger_ref: String,

    /// Whether the sensor is enabled
    #[schema(example = true)]
    pub enabled: bool,

    /// Parameter schema (StackStorm-style with inline required/secret)
    #[schema(value_type = Object, nullable = true)]
    pub param_schema: Option<JsonValue>,

    /// Creation timestamp
    #[schema(example = "2024-01-13T10:30:00Z")]
    pub created: DateTime<Utc>,

    /// Last update timestamp
    #[schema(example = "2024-01-13T10:30:00Z")]
    pub updated: DateTime<Utc>,
}

/// Simplified sensor response (for list endpoints)
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct SensorSummary {
    /// Sensor ID
    #[schema(example = 1)]
    pub id: i64,

    /// Unique reference identifier
    #[schema(example = "monitoring.cpu_sensor")]
    pub r#ref: String,

    /// Pack reference (optional)
    #[schema(example = "monitoring")]
    pub pack_ref: Option<String>,

    /// Human-readable label
    #[schema(example = "CPU Monitoring Sensor")]
    pub label: String,

    /// Sensor description
    #[schema(example = "Monitors CPU usage and generates events")]
    pub description: String,

    /// Trigger reference
    #[schema(example = "monitoring.cpu_threshold")]
    pub trigger_ref: String,

    /// Whether the sensor is enabled
    #[schema(example = true)]
    pub enabled: bool,

    /// Creation timestamp
    #[schema(example = "2024-01-13T10:30:00Z")]
    pub created: DateTime<Utc>,

    /// Last update timestamp
    #[schema(example = "2024-01-13T10:30:00Z")]
    pub updated: DateTime<Utc>,
}

/// Convert from Trigger model to TriggerResponse
impl From<attune_common::models::trigger::Trigger> for TriggerResponse {
    fn from(trigger: attune_common::models::trigger::Trigger) -> Self {
        Self {
            id: trigger.id,
            r#ref: trigger.r#ref,
            pack: trigger.pack,
            pack_ref: trigger.pack_ref,
            label: trigger.label,
            description: trigger.description,
            enabled: trigger.enabled,
            param_schema: trigger.param_schema,
            out_schema: trigger.out_schema,
            webhook_enabled: trigger.webhook_enabled,
            webhook_key: trigger.webhook_key,
            is_adhoc: trigger.is_adhoc,
            created: trigger.created,
            updated: trigger.updated,
        }
    }
}

/// Convert from Trigger model to TriggerSummary
impl From<attune_common::models::trigger::Trigger> for TriggerSummary {
    fn from(trigger: attune_common::models::trigger::Trigger) -> Self {
        Self {
            id: trigger.id,
            r#ref: trigger.r#ref,
            pack_ref: trigger.pack_ref,
            label: trigger.label,
            description: trigger.description,
            enabled: trigger.enabled,
            webhook_enabled: trigger.webhook_enabled,
            created: trigger.created,
            updated: trigger.updated,
        }
    }
}

/// Convert from Sensor model to SensorResponse
impl From<attune_common::models::trigger::Sensor> for SensorResponse {
    fn from(sensor: attune_common::models::trigger::Sensor) -> Self {
        Self {
            id: sensor.id,
            r#ref: sensor.r#ref,
            pack: sensor.pack,
            pack_ref: sensor.pack_ref,
            label: sensor.label,
            description: sensor.description,
            entrypoint: sensor.entrypoint,
            runtime: sensor.runtime,
            runtime_ref: sensor.runtime_ref,
            trigger: sensor.trigger,
            trigger_ref: sensor.trigger_ref,
            enabled: sensor.enabled,
            param_schema: sensor.param_schema,
            created: sensor.created,
            updated: sensor.updated,
        }
    }
}

/// Convert from Sensor model to SensorSummary
impl From<attune_common::models::trigger::Sensor> for SensorSummary {
    fn from(sensor: attune_common::models::trigger::Sensor) -> Self {
        Self {
            id: sensor.id,
            r#ref: sensor.r#ref,
            pack_ref: sensor.pack_ref,
            label: sensor.label,
            description: sensor.description,
            trigger_ref: sensor.trigger_ref,
            enabled: sensor.enabled,
            created: sensor.created,
            updated: sensor.updated,
        }
    }
}

fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_trigger_request_defaults() {
        let json = r#"{
            "ref": "test-trigger",
            "label": "Test Trigger"
        }"#;

        let req: CreateTriggerRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.r#ref, "test-trigger");
        assert_eq!(req.label, "Test Trigger");
        assert!(req.enabled);
        assert!(req.pack_ref.is_none());
        assert!(req.description.is_none());
    }

    #[test]
    fn test_create_trigger_request_validation() {
        let req = CreateTriggerRequest {
            r#ref: "".to_string(), // Invalid: empty
            pack_ref: None,
            label: "Test Trigger".to_string(),
            description: None,
            param_schema: None,
            out_schema: None,
            enabled: true,
        };

        assert!(req.validate().is_err());
    }

    #[test]
    fn test_create_sensor_request_valid() {
        let req = CreateSensorRequest {
            r#ref: "test.sensor".to_string(),
            pack_ref: "test-pack".to_string(),
            label: "Test Sensor".to_string(),
            description: "Test description".to_string(),
            entrypoint: "/sensors/test.py".to_string(),
            runtime_ref: "python3".to_string(),
            trigger_ref: "test.trigger".to_string(),
            param_schema: None,
            config: None,
            enabled: true,
        };

        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_update_trigger_request_all_none() {
        let req = UpdateTriggerRequest {
            label: None,
            description: None,
            param_schema: None,
            out_schema: None,
            enabled: None,
        };

        // Should be valid even with all None values
        assert!(req.validate().is_ok());
    }

    #[test]
    fn test_update_sensor_request_partial() {
        let req = UpdateSensorRequest {
            label: Some("Updated Sensor".to_string()),
            description: None,
            entrypoint: Some("/sensors/test_v2.py".to_string()),
            param_schema: None,
            enabled: Some(false),
        };

        assert!(req.validate().is_ok());
    }
}
