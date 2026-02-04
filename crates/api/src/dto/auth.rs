//! Authentication DTOs

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use validator::Validate;

/// Login request
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct LoginRequest {
    /// Identity login (username)
    #[validate(length(min = 1, max = 255))]
    #[schema(example = "admin")]
    pub login: String,

    /// Password
    #[validate(length(min = 1))]
    #[schema(example = "changeme123")]
    pub password: String,
}

/// Register request
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct RegisterRequest {
    /// Identity login (username)
    #[validate(length(min = 3, max = 255))]
    #[schema(example = "newuser")]
    pub login: String,

    /// Password
    #[validate(length(min = 8, max = 128))]
    #[schema(example = "SecurePass123!")]
    pub password: String,

    /// Display name (optional)
    #[validate(length(max = 255))]
    #[schema(example = "New User")]
    pub display_name: Option<String>,
}

/// Token response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TokenResponse {
    /// Access token (JWT)
    #[schema(example = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...")]
    pub access_token: String,

    /// Refresh token
    #[schema(example = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...")]
    pub refresh_token: String,

    /// Token type (always "Bearer")
    #[schema(example = "Bearer")]
    pub token_type: String,

    /// Access token expiration in seconds
    #[schema(example = 3600)]
    pub expires_in: i64,

    /// User information
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<UserInfo>,
}

/// User information included in token response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UserInfo {
    /// Identity ID
    #[schema(example = 1)]
    pub id: i64,

    /// Identity login
    #[schema(example = "admin")]
    pub login: String,

    /// Display name
    #[schema(example = "Administrator")]
    pub display_name: Option<String>,
}

impl TokenResponse {
    pub fn new(access_token: String, refresh_token: String, expires_in: i64) -> Self {
        Self {
            access_token,
            refresh_token,
            token_type: "Bearer".to_string(),
            expires_in,
            user: None,
        }
    }

    pub fn with_user(mut self, id: i64, login: String, display_name: Option<String>) -> Self {
        self.user = Some(UserInfo {
            id,
            login,
            display_name,
        });
        self
    }
}

/// Refresh token request
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct RefreshTokenRequest {
    /// Refresh token
    #[validate(length(min = 1))]
    #[schema(example = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9...")]
    pub refresh_token: String,
}

/// Change password request
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct ChangePasswordRequest {
    /// Current password
    #[validate(length(min = 1))]
    #[schema(example = "OldPassword123!")]
    pub current_password: String,

    /// New password
    #[validate(length(min = 8, max = 128))]
    #[schema(example = "NewPassword456!")]
    pub new_password: String,
}

/// Current user response
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CurrentUserResponse {
    /// Identity ID
    #[schema(example = 1)]
    pub id: i64,

    /// Identity login
    #[schema(example = "admin")]
    pub login: String,

    /// Display name
    #[schema(example = "Administrator")]
    pub display_name: Option<String>,
}
