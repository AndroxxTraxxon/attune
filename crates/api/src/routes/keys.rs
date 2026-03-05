//! Key/Secret management API routes

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use std::sync::Arc;
use validator::Validate;

use attune_common::models::OwnerType;
use attune_common::repositories::{
    action::ActionRepository,
    key::{CreateKeyInput, KeyRepository, KeySearchFilters, UpdateKeyInput},
    pack::PackRepository,
    trigger::SensorRepository,
    Create, Delete, FindByRef, Update,
};

use crate::auth::RequireAuth;
use crate::{
    dto::{
        common::{PaginatedResponse, PaginationParams},
        key::{CreateKeyRequest, KeyQueryParams, KeyResponse, KeySummary, UpdateKeyRequest},
        ApiResponse, SuccessResponse,
    },
    middleware::{ApiError, ApiResult},
    state::AppState,
};

/// List all keys with pagination and optional filters (values redacted)
#[utoipa::path(
    get,
    path = "/api/v1/keys",
    tag = "secrets",
    params(KeyQueryParams),
    responses(
        (status = 200, description = "List of keys (values redacted)", body = PaginatedResponse<KeySummary>),
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_keys(
    _user: RequireAuth,
    State(state): State<Arc<AppState>>,
    Query(query): Query<KeyQueryParams>,
) -> ApiResult<impl IntoResponse> {
    // All filtering and pagination happen in a single SQL query.
    let filters = KeySearchFilters {
        owner_type: query.owner_type,
        owner: query.owner.clone(),
        limit: query.limit(),
        offset: query.offset(),
    };

    let result = KeyRepository::search(&state.db, &filters).await?;

    let paginated_keys: Vec<KeySummary> = result.rows.into_iter().map(KeySummary::from).collect();

    let pagination_params = PaginationParams {
        page: query.page,
        page_size: query.per_page,
    };

    let response = PaginatedResponse::new(paginated_keys, &pagination_params, result.total);

    Ok((StatusCode::OK, Json(response)))
}

/// Get a single key by reference (includes decrypted value)
#[utoipa::path(
    get,
    path = "/api/v1/keys/{ref}",
    tag = "secrets",
    params(
        ("ref" = String, Path, description = "Key reference identifier")
    ),
    responses(
        (status = 200, description = "Key details with decrypted value", body = inline(ApiResponse<KeyResponse>)),
        (status = 404, description = "Key not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_key(
    _user: RequireAuth,
    State(state): State<Arc<AppState>>,
    Path(key_ref): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let mut key = KeyRepository::find_by_ref(&state.db, &key_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Key '{}' not found", key_ref)))?;

    // Decrypt value if encrypted
    if key.encrypted {
        let encryption_key = state
            .config
            .security
            .encryption_key
            .as_ref()
            .ok_or_else(|| {
                ApiError::InternalServerError("Encryption key not configured on server".to_string())
            })?;

        let decrypted_value =
            attune_common::crypto::decrypt(&key.value, encryption_key).map_err(|e| {
                tracing::error!("Failed to decrypt key '{}': {}", key_ref, e);
                ApiError::InternalServerError(format!("Failed to decrypt key: {}", e))
            })?;

        key.value = decrypted_value;
    }

    let response = ApiResponse::new(KeyResponse::from(key));

    Ok((StatusCode::OK, Json(response)))
}

/// Create a new key/secret
#[utoipa::path(
    post,
    path = "/api/v1/keys",
    tag = "secrets",
    request_body = CreateKeyRequest,
    responses(
        (status = 201, description = "Key created successfully", body = inline(ApiResponse<KeyResponse>)),
        (status = 400, description = "Validation error"),
        (status = 409, description = "Key with same ref already exists")
    ),
    security(("bearer_auth" = []))
)]
pub async fn create_key(
    _user: RequireAuth,
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateKeyRequest>,
) -> ApiResult<impl IntoResponse> {
    // Validate request
    request.validate()?;

    // Check if key with same ref already exists
    if KeyRepository::find_by_ref(&state.db, &request.r#ref)
        .await?
        .is_some()
    {
        return Err(ApiError::Conflict(format!(
            "Key with ref '{}' already exists",
            request.r#ref
        )));
    }

    // Auto-resolve owner IDs from refs when only the ref is provided.
    // This makes the API more ergonomic for sensors and other clients that
    // know the owner ref but not the numeric database ID.
    let mut owner_sensor = request.owner_sensor;
    let mut owner_action = request.owner_action;
    let mut owner_pack = request.owner_pack;

    match request.owner_type {
        OwnerType::Sensor => {
            if owner_sensor.is_none() {
                if let Some(ref sensor_ref) = request.owner_sensor_ref {
                    if let Some(sensor) =
                        SensorRepository::find_by_ref(&state.db, sensor_ref).await?
                    {
                        tracing::debug!(
                            "Auto-resolved owner_sensor from ref '{}' to id {}",
                            sensor_ref,
                            sensor.id
                        );
                        owner_sensor = Some(sensor.id);
                    } else {
                        return Err(ApiError::BadRequest(format!(
                            "Sensor with ref '{}' not found",
                            sensor_ref
                        )));
                    }
                }
            }
        }
        OwnerType::Action => {
            if owner_action.is_none() {
                if let Some(ref action_ref) = request.owner_action_ref {
                    if let Some(action) =
                        ActionRepository::find_by_ref(&state.db, action_ref).await?
                    {
                        tracing::debug!(
                            "Auto-resolved owner_action from ref '{}' to id {}",
                            action_ref,
                            action.id
                        );
                        owner_action = Some(action.id);
                    } else {
                        return Err(ApiError::BadRequest(format!(
                            "Action with ref '{}' not found",
                            action_ref
                        )));
                    }
                }
            }
        }
        OwnerType::Pack => {
            if owner_pack.is_none() {
                if let Some(ref pack_ref) = request.owner_pack_ref {
                    if let Some(pack) = PackRepository::find_by_ref(&state.db, pack_ref).await? {
                        tracing::debug!(
                            "Auto-resolved owner_pack from ref '{}' to id {}",
                            pack_ref,
                            pack.id
                        );
                        owner_pack = Some(pack.id);
                    } else {
                        return Err(ApiError::BadRequest(format!(
                            "Pack with ref '{}' not found",
                            pack_ref
                        )));
                    }
                }
            }
        }
        _ => {}
    }

    // Encrypt value if requested
    let (value, encryption_key_hash) = if request.encrypted {
        let encryption_key = state
            .config
            .security
            .encryption_key
            .as_ref()
            .ok_or_else(|| {
                ApiError::BadRequest(
                    "Cannot encrypt: encryption key not configured on server".to_string(),
                )
            })?;

        let encrypted_value = attune_common::crypto::encrypt(&request.value, encryption_key)
            .map_err(|e| {
                tracing::error!("Failed to encrypt key value: {}", e);
                ApiError::InternalServerError(format!("Failed to encrypt value: {}", e))
            })?;

        let key_hash = attune_common::crypto::hash_encryption_key(encryption_key);

        (encrypted_value, Some(key_hash))
    } else {
        // Store in plaintext (not recommended for sensitive data)
        (request.value.clone(), None)
    };

    // Create key input
    let key_input = CreateKeyInput {
        r#ref: request.r#ref,
        owner_type: request.owner_type,
        owner: request.owner,
        owner_identity: request.owner_identity,
        owner_pack,
        owner_pack_ref: request.owner_pack_ref,
        owner_action,
        owner_action_ref: request.owner_action_ref,
        owner_sensor,
        owner_sensor_ref: request.owner_sensor_ref,
        name: request.name,
        encrypted: request.encrypted,
        encryption_key_hash,
        value,
    };

    let mut key = KeyRepository::create(&state.db, key_input).await?;

    // Return decrypted value in response
    if key.encrypted {
        let encryption_key = state.config.security.encryption_key.as_ref().unwrap();
        key.value = attune_common::crypto::decrypt(&key.value, encryption_key).map_err(|e| {
            tracing::error!("Failed to decrypt newly created key: {}", e);
            ApiError::InternalServerError(format!("Failed to decrypt value: {}", e))
        })?;
    }

    let response = ApiResponse::with_message(KeyResponse::from(key), "Key created successfully");

    Ok((StatusCode::CREATED, Json(response)))
}

/// Update an existing key/secret
#[utoipa::path(
    put,
    path = "/api/v1/keys/{ref}",
    tag = "secrets",
    params(
        ("ref" = String, Path, description = "Key reference identifier")
    ),
    request_body = UpdateKeyRequest,
    responses(
        (status = 200, description = "Key updated successfully", body = inline(ApiResponse<KeyResponse>)),
        (status = 400, description = "Validation error"),
        (status = 404, description = "Key not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn update_key(
    _user: RequireAuth,
    State(state): State<Arc<AppState>>,
    Path(key_ref): Path<String>,
    Json(request): Json<UpdateKeyRequest>,
) -> ApiResult<impl IntoResponse> {
    // Validate request
    request.validate()?;

    // Verify key exists
    let existing = KeyRepository::find_by_ref(&state.db, &key_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Key '{}' not found", key_ref)))?;

    // Handle value update with encryption
    let (value, encrypted, encryption_key_hash) = if let Some(new_value) = request.value {
        let should_encrypt = request.encrypted.unwrap_or(existing.encrypted);

        if should_encrypt {
            let encryption_key =
                state
                    .config
                    .security
                    .encryption_key
                    .as_ref()
                    .ok_or_else(|| {
                        ApiError::BadRequest(
                            "Cannot encrypt: encryption key not configured on server".to_string(),
                        )
                    })?;

            let encrypted_value = attune_common::crypto::encrypt(&new_value, encryption_key)
                .map_err(|e| {
                    tracing::error!("Failed to encrypt key value: {}", e);
                    ApiError::InternalServerError(format!("Failed to encrypt value: {}", e))
                })?;

            let key_hash = attune_common::crypto::hash_encryption_key(encryption_key);

            (Some(encrypted_value), Some(should_encrypt), Some(key_hash))
        } else {
            (Some(new_value), Some(false), None)
        }
    } else {
        // No value update, but might be changing encryption status
        (None, request.encrypted, None)
    };

    // Create update input
    let update_input = UpdateKeyInput {
        name: request.name,
        value,
        encrypted,
        encryption_key_hash,
    };

    let mut updated_key = KeyRepository::update(&state.db, existing.id, update_input).await?;

    // Return decrypted value in response
    if updated_key.encrypted {
        let encryption_key = state
            .config
            .security
            .encryption_key
            .as_ref()
            .ok_or_else(|| {
                ApiError::InternalServerError("Encryption key not configured on server".to_string())
            })?;

        updated_key.value = attune_common::crypto::decrypt(&updated_key.value, encryption_key)
            .map_err(|e| {
                tracing::error!("Failed to decrypt updated key '{}': {}", key_ref, e);
                ApiError::InternalServerError(format!("Failed to decrypt value: {}", e))
            })?;
    }

    let response =
        ApiResponse::with_message(KeyResponse::from(updated_key), "Key updated successfully");

    Ok((StatusCode::OK, Json(response)))
}

/// Delete a key/secret
#[utoipa::path(
    delete,
    path = "/api/v1/keys/{ref}",
    tag = "secrets",
    params(
        ("ref" = String, Path, description = "Key reference identifier")
    ),
    responses(
        (status = 200, description = "Key deleted successfully", body = SuccessResponse),
        (status = 404, description = "Key not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn delete_key(
    _user: RequireAuth,
    State(state): State<Arc<AppState>>,
    Path(key_ref): Path<String>,
) -> ApiResult<impl IntoResponse> {
    // Verify key exists
    let key = KeyRepository::find_by_ref(&state.db, &key_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Key '{}' not found", key_ref)))?;

    // Delete the key
    let deleted = KeyRepository::delete(&state.db, key.id).await?;

    if !deleted {
        return Err(ApiError::NotFound(format!("Key '{}' not found", key_ref)));
    }

    let response = SuccessResponse::new("Key deleted successfully");

    Ok((StatusCode::OK, Json(response)))
}

/// Register key/secret routes
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/keys", get(list_keys).post(create_key))
        .route(
            "/keys/{ref}",
            get(get_key).put(update_key).delete(delete_key),
        )
}
