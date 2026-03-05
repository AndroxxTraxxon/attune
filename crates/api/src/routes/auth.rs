//! Authentication routes

use axum::{
    extract::State,
    routing::{get, post},
    Json, Router,
};

use validator::Validate;

use attune_common::repositories::{
    identity::{CreateIdentityInput, IdentityRepository},
    Create, FindById,
};

use crate::{
    auth::{
        hash_password,
        jwt::{
            generate_access_token, generate_refresh_token, generate_sensor_token, validate_token,
            TokenType,
        },
        middleware::RequireAuth,
        verify_password,
    },
    dto::{
        ApiResponse, ChangePasswordRequest, CurrentUserResponse, LoginRequest, RefreshTokenRequest,
        RegisterRequest, SuccessResponse, TokenResponse,
    },
    middleware::error::ApiError,
    state::SharedState,
};

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

/// Request body for creating sensor tokens
#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateSensorTokenRequest {
    /// Sensor reference (e.g., "core.timer")
    #[validate(length(min = 1, max = 255))]
    pub sensor_ref: String,

    /// List of trigger types this sensor can create events for
    #[validate(length(min = 1))]
    pub trigger_types: Vec<String>,

    /// Optional TTL in seconds (default: 86400 = 24 hours, max: 259200 = 72 hours)
    #[validate(range(min = 3600, max = 259200))]
    pub ttl_seconds: Option<i64>,
}

/// Response for sensor token creation
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct SensorTokenResponse {
    pub identity_id: i64,
    pub sensor_ref: String,
    pub token: String,
    pub expires_at: String,
    pub trigger_types: Vec<String>,
}

/// Create authentication routes
pub fn routes() -> Router<SharedState> {
    Router::new()
        .route("/login", post(login))
        .route("/register", post(register))
        .route("/refresh", post(refresh_token))
        .route("/me", get(get_current_user))
        .route("/change-password", post(change_password))
        .route("/sensor-token", post(create_sensor_token))
        .route("/internal/sensor-token", post(create_sensor_token_internal))
}

/// Login endpoint
///
/// POST /auth/login
#[utoipa::path(
    post,
    path = "/auth/login",
    tag = "auth",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Successfully logged in", body = inline(ApiResponse<TokenResponse>)),
        (status = 401, description = "Invalid credentials"),
        (status = 400, description = "Validation error")
    )
)]
pub async fn login(
    State(state): State<SharedState>,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<ApiResponse<TokenResponse>>, ApiError> {
    // Validate request
    payload
        .validate()
        .map_err(|e| ApiError::ValidationError(format!("Invalid login request: {}", e)))?;

    // Find identity by login
    let identity = IdentityRepository::find_by_login(&state.db, &payload.login)
        .await?
        .ok_or_else(|| ApiError::Unauthorized("Invalid login or password".to_string()))?;

    // Check if identity has a password set
    let password_hash = identity
        .password_hash
        .as_ref()
        .ok_or_else(|| ApiError::Unauthorized("Invalid login or password".to_string()))?;

    // Verify password
    let is_valid = verify_password(&payload.password, password_hash)
        .map_err(|_| ApiError::Unauthorized("Invalid login or password".to_string()))?;

    if !is_valid {
        return Err(ApiError::Unauthorized(
            "Invalid login or password".to_string(),
        ));
    }

    // Generate tokens
    let access_token = generate_access_token(identity.id, &identity.login, &state.jwt_config)?;
    let refresh_token = generate_refresh_token(identity.id, &identity.login, &state.jwt_config)?;

    let response = TokenResponse::new(
        access_token,
        refresh_token,
        state.jwt_config.access_token_expiration,
    )
    .with_user(
        identity.id,
        identity.login.clone(),
        identity.display_name.clone(),
    );

    Ok(Json(ApiResponse::new(response)))
}

/// Register endpoint
///
/// POST /auth/register
#[utoipa::path(
    post,
    path = "/auth/register",
    tag = "auth",
    request_body = RegisterRequest,
    responses(
        (status = 200, description = "Successfully registered", body = inline(ApiResponse<TokenResponse>)),
        (status = 409, description = "User already exists"),
        (status = 400, description = "Validation error")
    )
)]
pub async fn register(
    State(state): State<SharedState>,
    Json(payload): Json<RegisterRequest>,
) -> Result<Json<ApiResponse<TokenResponse>>, ApiError> {
    // Validate request
    payload
        .validate()
        .map_err(|e| ApiError::ValidationError(format!("Invalid registration request: {}", e)))?;

    // Check if login already exists
    if IdentityRepository::find_by_login(&state.db, &payload.login)
        .await?
        .is_some()
    {
        return Err(ApiError::Conflict(format!(
            "Identity with login '{}' already exists",
            payload.login
        )));
    }

    // Hash password
    let password_hash = hash_password(&payload.password)?;

    // Create identity with password hash
    let input = CreateIdentityInput {
        login: payload.login.clone(),
        display_name: payload.display_name,
        password_hash: Some(password_hash),
        attributes: serde_json::json!({}),
    };

    let identity = IdentityRepository::create(&state.db, input).await?;

    // Generate tokens
    let access_token = generate_access_token(identity.id, &identity.login, &state.jwt_config)?;
    let refresh_token = generate_refresh_token(identity.id, &identity.login, &state.jwt_config)?;

    let response = TokenResponse::new(
        access_token,
        refresh_token,
        state.jwt_config.access_token_expiration,
    )
    .with_user(
        identity.id,
        identity.login.clone(),
        identity.display_name.clone(),
    );

    Ok(Json(ApiResponse::new(response)))
}

/// Refresh token endpoint
///
/// POST /auth/refresh
#[utoipa::path(
    post,
    path = "/auth/refresh",
    tag = "auth",
    request_body = RefreshTokenRequest,
    responses(
        (status = 200, description = "Successfully refreshed token", body = inline(ApiResponse<TokenResponse>)),
        (status = 401, description = "Invalid or expired refresh token"),
        (status = 400, description = "Validation error")
    )
)]
pub async fn refresh_token(
    State(state): State<SharedState>,
    Json(payload): Json<RefreshTokenRequest>,
) -> Result<Json<ApiResponse<TokenResponse>>, ApiError> {
    // Validate request
    payload
        .validate()
        .map_err(|e| ApiError::ValidationError(format!("Invalid refresh token request: {}", e)))?;

    // Validate refresh token
    let claims = validate_token(&payload.refresh_token, &state.jwt_config)
        .map_err(|_| ApiError::Unauthorized("Invalid or expired refresh token".to_string()))?;

    // Ensure it's a refresh token
    if claims.token_type != TokenType::Refresh {
        return Err(ApiError::Unauthorized("Invalid token type".to_string()));
    }

    // Parse identity ID
    let identity_id: i64 = claims
        .sub
        .parse()
        .map_err(|_| ApiError::Unauthorized("Invalid token".to_string()))?;

    // Verify identity still exists
    let identity = IdentityRepository::find_by_id(&state.db, identity_id)
        .await?
        .ok_or_else(|| ApiError::Unauthorized("Identity not found".to_string()))?;

    // Generate new tokens
    let access_token = generate_access_token(identity.id, &identity.login, &state.jwt_config)?;
    let refresh_token = generate_refresh_token(identity.id, &identity.login, &state.jwt_config)?;

    let response = TokenResponse::new(
        access_token,
        refresh_token,
        state.jwt_config.access_token_expiration,
    );

    Ok(Json(ApiResponse::new(response)))
}

/// Get current user endpoint
///
/// GET /auth/me
#[utoipa::path(
    get,
    path = "/auth/me",
    tag = "auth",
    responses(
        (status = 200, description = "Current user information", body = inline(ApiResponse<CurrentUserResponse>)),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Identity not found")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn get_current_user(
    State(state): State<SharedState>,
    RequireAuth(user): RequireAuth,
) -> Result<Json<ApiResponse<CurrentUserResponse>>, ApiError> {
    let identity_id = user.identity_id()?;

    // Fetch identity from database
    let identity = IdentityRepository::find_by_id(&state.db, identity_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Identity not found".to_string()))?;

    let response = CurrentUserResponse {
        id: identity.id,
        login: identity.login,
        display_name: identity.display_name,
    };

    Ok(Json(ApiResponse::new(response)))
}

/// Change password endpoint
///
/// POST /auth/change-password
#[utoipa::path(
    post,
    path = "/auth/change-password",
    tag = "auth",
    request_body = ChangePasswordRequest,
    responses(
        (status = 200, description = "Password changed successfully", body = inline(ApiResponse<SuccessResponse>)),
        (status = 401, description = "Invalid current password or unauthorized"),
        (status = 400, description = "Validation error"),
        (status = 404, description = "Identity not found")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn change_password(
    State(state): State<SharedState>,
    RequireAuth(user): RequireAuth,
    Json(payload): Json<ChangePasswordRequest>,
) -> Result<Json<ApiResponse<SuccessResponse>>, ApiError> {
    // Validate request
    payload.validate().map_err(|e| {
        ApiError::ValidationError(format!("Invalid change password request: {}", e))
    })?;

    let identity_id = user.identity_id()?;

    // Fetch identity from database
    let identity = IdentityRepository::find_by_id(&state.db, identity_id)
        .await?
        .ok_or_else(|| ApiError::NotFound("Identity not found".to_string()))?;

    // Get current password hash
    let current_password_hash = identity
        .password_hash
        .as_ref()
        .ok_or_else(|| ApiError::Unauthorized("No password set".to_string()))?;

    // Verify current password
    let is_valid = verify_password(&payload.current_password, current_password_hash)
        .map_err(|_| ApiError::Unauthorized("Invalid current password".to_string()))?;

    if !is_valid {
        return Err(ApiError::Unauthorized(
            "Invalid current password".to_string(),
        ));
    }

    // Hash new password
    let new_password_hash = hash_password(&payload.new_password)?;

    // Update identity in database with new password hash
    use attune_common::repositories::identity::UpdateIdentityInput;
    use attune_common::repositories::Update;

    let update_input = UpdateIdentityInput {
        display_name: None,
        password_hash: Some(new_password_hash),
        attributes: None,
    };

    IdentityRepository::update(&state.db, identity_id, update_input).await?;

    Ok(Json(ApiResponse::new(SuccessResponse::new(
        "Password changed successfully",
    ))))
}

/// Create sensor token endpoint (internal use by sensor service)
///
/// POST /auth/sensor-token
#[utoipa::path(
    post,
    path = "/auth/sensor-token",
    tag = "auth",
    request_body = CreateSensorTokenRequest,
    responses(
        (status = 200, description = "Sensor token created successfully", body = inline(ApiResponse<SensorTokenResponse>)),
        (status = 400, description = "Validation error"),
        (status = 401, description = "Unauthorized")
    ),
    security(
        ("bearer_auth" = [])
    )
)]
pub async fn create_sensor_token(
    State(state): State<SharedState>,
    RequireAuth(_user): RequireAuth,
    Json(payload): Json<CreateSensorTokenRequest>,
) -> Result<Json<ApiResponse<SensorTokenResponse>>, ApiError> {
    create_sensor_token_impl(state, payload).await
}

/// Create sensor token endpoint for internal service use (no auth required)
///
/// POST /auth/internal/sensor-token
///
/// This endpoint is intended for internal use by the sensor service to provision
/// tokens for standalone sensors. In production, this should be restricted by
/// network policies or replaced with proper service-to-service authentication.
#[utoipa::path(
    post,
    path = "/auth/internal/sensor-token",
    tag = "auth",
    request_body = CreateSensorTokenRequest,
    responses(
        (status = 200, description = "Sensor token created successfully", body = inline(ApiResponse<SensorTokenResponse>)),
        (status = 400, description = "Validation error")
    )
)]
pub async fn create_sensor_token_internal(
    State(state): State<SharedState>,
    Json(payload): Json<CreateSensorTokenRequest>,
) -> Result<Json<ApiResponse<SensorTokenResponse>>, ApiError> {
    create_sensor_token_impl(state, payload).await
}

/// Shared implementation for sensor token creation
async fn create_sensor_token_impl(
    state: SharedState,
    payload: CreateSensorTokenRequest,
) -> Result<Json<ApiResponse<SensorTokenResponse>>, ApiError> {
    // Validate request
    payload
        .validate()
        .map_err(|e| ApiError::ValidationError(format!("Invalid sensor token request: {}", e)))?;

    // Create or find sensor identity
    let sensor_login = format!("sensor:{}", payload.sensor_ref);

    let identity = match IdentityRepository::find_by_login(&state.db, &sensor_login).await? {
        Some(identity) => identity,
        None => {
            // Create new sensor identity
            let input = CreateIdentityInput {
                login: sensor_login.clone(),
                display_name: Some(format!("Sensor: {}", payload.sensor_ref)),
                password_hash: None, // Sensors don't use passwords
                attributes: serde_json::json!({
                    "type": "sensor",
                    "sensor_ref": payload.sensor_ref,
                    "trigger_types": payload.trigger_types,
                }),
            };
            IdentityRepository::create(&state.db, input).await?
        }
    };

    // Generate sensor token
    let ttl_seconds = payload.ttl_seconds.unwrap_or(86400); // Default: 24 hours
    let token = generate_sensor_token(
        identity.id,
        &payload.sensor_ref,
        payload.trigger_types.clone(),
        &state.jwt_config,
        Some(ttl_seconds),
    )?;

    // Calculate expiration time
    let expires_at = chrono::Utc::now() + chrono::Duration::seconds(ttl_seconds);

    let response = SensorTokenResponse {
        identity_id: identity.id,
        sensor_ref: payload.sensor_ref,
        token,
        expires_at: expires_at.to_rfc3339(),
        trigger_types: payload.trigger_types,
    };

    Ok(Json(ApiResponse::new(response)))
}
