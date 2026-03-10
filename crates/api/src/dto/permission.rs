use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use utoipa::{IntoParams, ToSchema};
use validator::Validate;

#[derive(Debug, Clone, Deserialize, IntoParams)]
pub struct PermissionSetQueryParams {
    #[serde(default)]
    pub pack_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct IdentitySummary {
    pub id: i64,
    pub login: String,
    pub display_name: Option<String>,
    pub attributes: JsonValue,
}

pub type IdentityResponse = IdentitySummary;

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PermissionSetSummary {
    pub id: i64,
    pub r#ref: String,
    pub pack_ref: Option<String>,
    pub label: Option<String>,
    pub description: Option<String>,
    pub grants: JsonValue,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
pub struct PermissionAssignmentResponse {
    pub id: i64,
    pub identity_id: i64,
    pub permission_set_id: i64,
    pub permission_set_ref: String,
    pub created: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct CreatePermissionAssignmentRequest {
    pub identity_id: Option<i64>,
    pub identity_login: Option<String>,
    pub permission_set_ref: String,
}

#[derive(Debug, Clone, Deserialize, Validate, ToSchema)]
pub struct CreateIdentityRequest {
    #[validate(length(min = 3, max = 255))]
    pub login: String,
    #[validate(length(max = 255))]
    pub display_name: Option<String>,
    #[validate(length(min = 8, max = 128))]
    pub password: Option<String>,
    #[serde(default)]
    pub attributes: JsonValue,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
pub struct UpdateIdentityRequest {
    pub display_name: Option<String>,
    pub password: Option<String>,
    pub attributes: Option<JsonValue>,
}
