//! Artifact DTOs for API requests and responses

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use utoipa::{IntoParams, ToSchema};

use attune_common::models::enums::{ArtifactType, OwnerType, RetentionPolicyType};

// ============================================================================
// Artifact DTOs
// ============================================================================

/// Request DTO for creating a new artifact
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateArtifactRequest {
    /// Artifact reference (unique identifier, e.g. "build.log", "test.results")
    #[schema(example = "mypack.build_log")]
    pub r#ref: String,

    /// Owner scope type
    #[schema(example = "action")]
    pub scope: OwnerType,

    /// Owner identifier (ref string of the owning entity)
    #[schema(example = "mypack.deploy")]
    pub owner: String,

    /// Artifact type
    #[schema(example = "file_text")]
    pub r#type: ArtifactType,

    /// Retention policy type
    #[serde(default = "default_retention_policy")]
    #[schema(example = "versions")]
    pub retention_policy: RetentionPolicyType,

    /// Retention limit (number of versions, days, hours, or minutes depending on policy)
    #[serde(default = "default_retention_limit")]
    #[schema(example = 5)]
    pub retention_limit: i32,

    /// Human-readable name
    #[schema(example = "Build Log")]
    pub name: Option<String>,

    /// Optional description
    #[schema(example = "Output log from the build action")]
    pub description: Option<String>,

    /// MIME content type (e.g. "text/plain", "application/json")
    #[schema(example = "text/plain")]
    pub content_type: Option<String>,

    /// Execution ID that produced this artifact
    #[schema(example = 42)]
    pub execution: Option<i64>,

    /// Initial structured data (for progress-type artifacts or metadata)
    #[schema(value_type = Option<Object>)]
    pub data: Option<JsonValue>,
}

fn default_retention_policy() -> RetentionPolicyType {
    RetentionPolicyType::Versions
}

fn default_retention_limit() -> i32 {
    5
}

/// Request DTO for updating an existing artifact
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateArtifactRequest {
    /// Updated owner scope
    pub scope: Option<OwnerType>,

    /// Updated owner identifier
    pub owner: Option<String>,

    /// Updated artifact type
    pub r#type: Option<ArtifactType>,

    /// Updated retention policy
    pub retention_policy: Option<RetentionPolicyType>,

    /// Updated retention limit
    pub retention_limit: Option<i32>,

    /// Updated name
    pub name: Option<String>,

    /// Updated description
    pub description: Option<String>,

    /// Updated content type
    pub content_type: Option<String>,

    /// Updated structured data (replaces existing data entirely)
    pub data: Option<JsonValue>,
}

/// Request DTO for appending to a progress-type artifact
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct AppendProgressRequest {
    /// The entry to append to the progress data array.
    /// Can be any JSON value (string, object, number, etc.)
    #[schema(value_type = Object, example = json!({"step": "compile", "status": "done", "duration_ms": 1234}))]
    pub entry: JsonValue,
}

/// Request DTO for setting the full data payload on an artifact
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct SetDataRequest {
    /// The data to set (replaces existing data entirely)
    #[schema(value_type = Object)]
    pub data: JsonValue,
}

/// Response DTO for artifact information
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ArtifactResponse {
    /// Artifact ID
    #[schema(example = 1)]
    pub id: i64,

    /// Artifact reference
    #[schema(example = "mypack.build_log")]
    pub r#ref: String,

    /// Owner scope type
    pub scope: OwnerType,

    /// Owner identifier
    #[schema(example = "mypack.deploy")]
    pub owner: String,

    /// Artifact type
    pub r#type: ArtifactType,

    /// Retention policy
    pub retention_policy: RetentionPolicyType,

    /// Retention limit
    #[schema(example = 5)]
    pub retention_limit: i32,

    /// Human-readable name
    #[schema(example = "Build Log")]
    pub name: Option<String>,

    /// Description
    pub description: Option<String>,

    /// MIME content type
    #[schema(example = "text/plain")]
    pub content_type: Option<String>,

    /// Size of the latest version in bytes
    pub size_bytes: Option<i64>,

    /// Execution that produced this artifact
    pub execution: Option<i64>,

    /// Structured data (progress entries, metadata, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<JsonValue>,

    /// Creation timestamp
    pub created: DateTime<Utc>,

    /// Last update timestamp
    pub updated: DateTime<Utc>,
}

/// Simplified artifact for list endpoints
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ArtifactSummary {
    /// Artifact ID
    pub id: i64,

    /// Artifact reference
    pub r#ref: String,

    /// Artifact type
    pub r#type: ArtifactType,

    /// Human-readable name
    pub name: Option<String>,

    /// MIME content type
    pub content_type: Option<String>,

    /// Size of latest version in bytes
    pub size_bytes: Option<i64>,

    /// Execution that produced this artifact
    pub execution: Option<i64>,

    /// Owner scope
    pub scope: OwnerType,

    /// Owner identifier
    pub owner: String,

    /// Creation timestamp
    pub created: DateTime<Utc>,

    /// Last update timestamp
    pub updated: DateTime<Utc>,
}

/// Query parameters for filtering artifacts
#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct ArtifactQueryParams {
    /// Filter by owner scope type
    pub scope: Option<OwnerType>,

    /// Filter by owner identifier
    pub owner: Option<String>,

    /// Filter by artifact type
    pub r#type: Option<ArtifactType>,

    /// Filter by execution ID
    pub execution: Option<i64>,

    /// Search by name (case-insensitive substring match)
    pub name: Option<String>,

    /// Page number (1-based)
    #[serde(default = "default_page")]
    #[param(example = 1, minimum = 1)]
    pub page: u32,

    /// Items per page
    #[serde(default = "default_per_page")]
    #[param(example = 20, minimum = 1, maximum = 100)]
    pub per_page: u32,
}

impl ArtifactQueryParams {
    pub fn offset(&self) -> u32 {
        (self.page.saturating_sub(1)) * self.per_page
    }

    pub fn limit(&self) -> u32 {
        self.per_page.min(100)
    }
}

fn default_page() -> u32 {
    1
}

fn default_per_page() -> u32 {
    20
}

// ============================================================================
// ArtifactVersion DTOs
// ============================================================================

/// Request DTO for creating a new artifact version with JSON content
#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreateVersionJsonRequest {
    /// Structured JSON content for this version
    #[schema(value_type = Object)]
    pub content: JsonValue,

    /// MIME content type override (defaults to "application/json")
    pub content_type: Option<String>,

    /// Free-form metadata about this version
    #[schema(value_type = Option<Object>)]
    pub meta: Option<JsonValue>,

    /// Who created this version (e.g. action ref, identity, "system")
    pub created_by: Option<String>,
}

/// Response DTO for an artifact version (without binary content)
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ArtifactVersionResponse {
    /// Version ID
    pub id: i64,

    /// Parent artifact ID
    pub artifact: i64,

    /// Version number (1-based)
    pub version: i32,

    /// MIME content type
    pub content_type: Option<String>,

    /// Size of content in bytes
    pub size_bytes: Option<i64>,

    /// Structured JSON content (if this version has JSON data)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_json: Option<JsonValue>,

    /// Free-form metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub meta: Option<JsonValue>,

    /// Who created this version
    pub created_by: Option<String>,

    /// Creation timestamp
    pub created: DateTime<Utc>,
}

/// Simplified version for list endpoints
#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct ArtifactVersionSummary {
    /// Version ID
    pub id: i64,

    /// Version number
    pub version: i32,

    /// MIME content type
    pub content_type: Option<String>,

    /// Size of content in bytes
    pub size_bytes: Option<i64>,

    /// Who created this version
    pub created_by: Option<String>,

    /// Creation timestamp
    pub created: DateTime<Utc>,
}

// ============================================================================
// Conversions
// ============================================================================

impl From<attune_common::models::artifact::Artifact> for ArtifactResponse {
    fn from(a: attune_common::models::artifact::Artifact) -> Self {
        Self {
            id: a.id,
            r#ref: a.r#ref,
            scope: a.scope,
            owner: a.owner,
            r#type: a.r#type,
            retention_policy: a.retention_policy,
            retention_limit: a.retention_limit,
            name: a.name,
            description: a.description,
            content_type: a.content_type,
            size_bytes: a.size_bytes,
            execution: a.execution,
            data: a.data,
            created: a.created,
            updated: a.updated,
        }
    }
}

impl From<attune_common::models::artifact::Artifact> for ArtifactSummary {
    fn from(a: attune_common::models::artifact::Artifact) -> Self {
        Self {
            id: a.id,
            r#ref: a.r#ref,
            r#type: a.r#type,
            name: a.name,
            content_type: a.content_type,
            size_bytes: a.size_bytes,
            execution: a.execution,
            scope: a.scope,
            owner: a.owner,
            created: a.created,
            updated: a.updated,
        }
    }
}

impl From<attune_common::models::artifact_version::ArtifactVersion> for ArtifactVersionResponse {
    fn from(v: attune_common::models::artifact_version::ArtifactVersion) -> Self {
        Self {
            id: v.id,
            artifact: v.artifact,
            version: v.version,
            content_type: v.content_type,
            size_bytes: v.size_bytes,
            content_json: v.content_json,
            meta: v.meta,
            created_by: v.created_by,
            created: v.created,
        }
    }
}

impl From<attune_common::models::artifact_version::ArtifactVersion> for ArtifactVersionSummary {
    fn from(v: attune_common::models::artifact_version::ArtifactVersion) -> Self {
        Self {
            id: v.id,
            version: v.version,
            content_type: v.content_type,
            size_bytes: v.size_bytes,
            created_by: v.created_by,
            created: v.created,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_query_params_defaults() {
        let json = r#"{}"#;
        let params: ArtifactQueryParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.page, 1);
        assert_eq!(params.per_page, 20);
        assert!(params.scope.is_none());
        assert!(params.r#type.is_none());
    }

    #[test]
    fn test_query_params_offset() {
        let params = ArtifactQueryParams {
            scope: None,
            owner: None,
            r#type: None,
            execution: None,
            name: None,
            page: 3,
            per_page: 20,
        };
        assert_eq!(params.offset(), 40);
    }

    #[test]
    fn test_query_params_limit_cap() {
        let params = ArtifactQueryParams {
            scope: None,
            owner: None,
            r#type: None,
            execution: None,
            name: None,
            page: 1,
            per_page: 200,
        };
        assert_eq!(params.limit(), 100);
    }

    #[test]
    fn test_create_request_defaults() {
        let json = r#"{
            "ref": "test.artifact",
            "scope": "system",
            "owner": "",
            "type": "file_text"
        }"#;
        let req: CreateArtifactRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.retention_policy, RetentionPolicyType::Versions);
        assert_eq!(req.retention_limit, 5);
    }

    #[test]
    fn test_append_progress_request() {
        let json = r#"{"entry": {"step": "build", "status": "done"}}"#;
        let req: AppendProgressRequest = serde_json::from_str(json).unwrap();
        assert!(req.entry.is_object());
    }
}
