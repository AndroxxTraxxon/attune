//! Pack DTOs for API requests and responses

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use utoipa::ToSchema;
use validator::Validate;

/// Request DTO for creating a new pack
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct CreatePackRequest {
    /// Unique reference identifier (e.g., "core", "aws", "slack")
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "slack")]
    pub r#ref: String,

    /// Human-readable label
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "Slack Integration")]
    pub label: String,

    /// Pack description
    #[schema(example = "Integration with Slack for messaging and notifications")]
    pub description: Option<String>,

    /// Pack version (semver format recommended)
    #[validate(length(min = 1, max = 50))]
    #[schema(example = "1.0.0")]
    pub version: String,

    /// Configuration schema (JSON Schema)
    #[serde(default = "default_empty_object")]
    #[schema(value_type = Object, example = json!({"type": "object", "properties": {"api_token": {"type": "string"}}}))]
    pub conf_schema: JsonValue,

    /// Pack configuration values
    #[serde(default = "default_empty_object")]
    #[schema(value_type = Object, example = json!({"api_token": "xoxb-..."}))]
    pub config: JsonValue,

    /// Pack metadata
    #[serde(default = "default_empty_object")]
    #[schema(value_type = Object, example = json!({"author": "Attune Team"}))]
    pub meta: JsonValue,

    /// Tags for categorization
    #[serde(default)]
    #[schema(example = json!(["messaging", "collaboration"]))]
    pub tags: Vec<String>,

    /// Runtime dependencies (refs of required packs)
    #[serde(default)]
    #[schema(example = json!(["core"]))]
    pub runtime_deps: Vec<String>,

    /// Whether this is a standard/built-in pack
    #[serde(default)]
    #[schema(example = false)]
    pub is_standard: bool,
}

/// Request DTO for registering a pack from local filesystem
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct RegisterPackRequest {
    /// Local filesystem path to the pack directory
    #[validate(length(min = 1))]
    #[schema(example = "/path/to/packs/mypack")]
    pub path: String,

    /// Skip running pack tests during registration
    #[serde(default)]
    #[schema(example = false)]
    pub skip_tests: bool,

    /// Force registration even if tests fail
    #[serde(default)]
    #[schema(example = false)]
    pub force: bool,
}

/// Request DTO for installing a pack from remote source
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct InstallPackRequest {
    /// Repository URL or source location
    #[validate(length(min = 1))]
    #[schema(example = "https://github.com/attune/pack-slack.git")]
    pub source: String,

    /// Git branch, tag, or commit reference
    #[schema(example = "main")]
    pub ref_spec: Option<String>,

    /// Force reinstall if pack already exists
    #[serde(default)]
    #[schema(example = false)]
    pub force: bool,

    /// Skip running pack tests during installation
    #[serde(default)]
    #[schema(example = false)]
    pub skip_tests: bool,

    /// Skip dependency validation (not recommended)
    #[serde(default)]
    #[schema(example = false)]
    pub skip_deps: bool,
}

/// Response for pack install/register operations with test results
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PackInstallResponse {
    /// The installed/registered pack
    pub pack: PackResponse,

    /// Test execution result (if tests were run)
    pub test_result: Option<attune_common::models::pack_test::PackTestResult>,

    /// Whether tests were skipped
    pub tests_skipped: bool,
}

/// Request DTO for updating a pack
#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct UpdatePackRequest {
    /// Human-readable label
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "Slack Integration v2")]
    pub label: Option<String>,

    /// Pack description
    #[schema(example = "Enhanced Slack integration with new features")]
    pub description: Option<String>,

    /// Pack version
    #[validate(length(min = 1, max = 50))]
    #[schema(example = "2.0.0")]
    pub version: Option<String>,

    /// Configuration schema
    #[schema(value_type = Object, nullable = true)]
    pub conf_schema: Option<JsonValue>,

    /// Pack configuration values
    #[schema(value_type = Object, nullable = true)]
    pub config: Option<JsonValue>,

    /// Pack metadata
    #[schema(value_type = Object, nullable = true)]
    pub meta: Option<JsonValue>,

    /// Tags for categorization
    #[schema(example = json!(["messaging", "collaboration", "webhooks"]))]
    pub tags: Option<Vec<String>>,

    /// Runtime dependencies
    #[schema(example = json!(["core", "http"]))]
    pub runtime_deps: Option<Vec<String>>,

    /// Whether this is a standard pack
    #[schema(example = false)]
    pub is_standard: Option<bool>,
}

/// Response DTO for pack information
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PackResponse {
    /// Pack ID
    #[schema(example = 1)]
    pub id: i64,

    /// Unique reference identifier
    #[schema(example = "slack")]
    pub r#ref: String,

    /// Human-readable label
    #[schema(example = "Slack Integration")]
    pub label: String,

    /// Pack description
    #[schema(example = "Integration with Slack for messaging and notifications")]
    pub description: Option<String>,

    /// Pack version
    #[schema(example = "1.0.0")]
    pub version: String,

    /// Configuration schema
    #[schema(value_type = Object)]
    pub conf_schema: JsonValue,

    /// Pack configuration
    #[schema(value_type = Object)]
    pub config: JsonValue,

    /// Pack metadata
    #[schema(value_type = Object)]
    pub meta: JsonValue,

    /// Tags
    #[schema(example = json!(["messaging", "collaboration"]))]
    pub tags: Vec<String>,

    /// Runtime dependencies
    #[schema(example = json!(["core"]))]
    pub runtime_deps: Vec<String>,

    /// Is standard pack
    #[schema(example = false)]
    pub is_standard: bool,

    /// Creation timestamp
    #[schema(example = "2024-01-13T10:30:00Z")]
    pub created: DateTime<Utc>,

    /// Last update timestamp
    #[schema(example = "2024-01-13T10:30:00Z")]
    pub updated: DateTime<Utc>,
}

/// Simplified pack response (for list endpoints)
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PackSummary {
    /// Pack ID
    #[schema(example = 1)]
    pub id: i64,

    /// Unique reference identifier
    #[schema(example = "slack")]
    pub r#ref: String,

    /// Human-readable label
    #[schema(example = "Slack Integration")]
    pub label: String,

    /// Pack description
    #[schema(example = "Integration with Slack for messaging and notifications")]
    pub description: Option<String>,

    /// Pack version
    #[schema(example = "1.0.0")]
    pub version: String,

    /// Tags
    #[schema(example = json!(["messaging", "collaboration"]))]
    pub tags: Vec<String>,

    /// Is standard pack
    #[schema(example = false)]
    pub is_standard: bool,

    /// Creation timestamp
    #[schema(example = "2024-01-13T10:30:00Z")]
    pub created: DateTime<Utc>,

    /// Last update timestamp
    #[schema(example = "2024-01-13T10:30:00Z")]
    pub updated: DateTime<Utc>,
}

/// Convert from Pack model to PackResponse
impl From<attune_common::models::Pack> for PackResponse {
    fn from(pack: attune_common::models::Pack) -> Self {
        Self {
            id: pack.id,
            r#ref: pack.r#ref,
            label: pack.label,
            description: pack.description,
            version: pack.version,
            conf_schema: pack.conf_schema,
            config: pack.config,
            meta: pack.meta,
            tags: pack.tags,
            runtime_deps: pack.runtime_deps,
            is_standard: pack.is_standard,
            created: pack.created,
            updated: pack.updated,
        }
    }
}

/// Convert from Pack model to PackSummary
impl From<attune_common::models::Pack> for PackSummary {
    fn from(pack: attune_common::models::Pack) -> Self {
        Self {
            id: pack.id,
            r#ref: pack.r#ref,
            label: pack.label,
            description: pack.description,
            version: pack.version,
            tags: pack.tags,
            is_standard: pack.is_standard,
            created: pack.created,
            updated: pack.updated,
        }
    }
}

/// Response for pack workflow sync operation
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PackWorkflowSyncResponse {
    /// Pack reference
    pub pack_ref: String,
    /// Number of workflows loaded from filesystem
    pub loaded_count: usize,
    /// Number of workflows registered/updated in database
    pub registered_count: usize,
    /// Individual workflow registration results
    pub workflows: Vec<WorkflowSyncResult>,
    /// Any errors encountered during sync
    pub errors: Vec<String>,
}

/// Individual workflow sync result
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct WorkflowSyncResult {
    /// Workflow reference name
    pub ref_name: String,
    /// Whether the workflow was created (false = updated)
    pub created: bool,
    /// Workflow definition ID
    pub workflow_def_id: i64,
    /// Any warnings during registration
    pub warnings: Vec<String>,
}

/// Response for pack workflow validation operation
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PackWorkflowValidationResponse {
    /// Pack reference
    pub pack_ref: String,
    /// Number of workflows validated
    pub validated_count: usize,
    /// Number of workflows with errors
    pub error_count: usize,
    /// Validation errors by workflow reference
    pub errors: std::collections::HashMap<String, Vec<String>>,
}

fn default_empty_object() -> JsonValue {
    serde_json::json!({})
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_pack_request_defaults() {
        let json = r#"{
            "ref": "test-pack",
            "label": "Test Pack",
            "version": "1.0.0"
        }"#;

        let req: CreatePackRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.r#ref, "test-pack");
        assert_eq!(req.label, "Test Pack");
        assert_eq!(req.version, "1.0.0");
        assert!(req.tags.is_empty());
        assert!(req.runtime_deps.is_empty());
        assert!(!req.is_standard);
    }

    #[test]
    fn test_create_pack_request_validation() {
        let req = CreatePackRequest {
            r#ref: "".to_string(), // Invalid: empty
            label: "Test".to_string(),
            version: "1.0.0".to_string(),
            description: None,
            conf_schema: default_empty_object(),
            config: default_empty_object(),
            meta: default_empty_object(),
            tags: vec![],
            runtime_deps: vec![],
            is_standard: false,
        };

        assert!(req.validate().is_err());
    }
}
