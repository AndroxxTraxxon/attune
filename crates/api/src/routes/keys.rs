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

use attune_common::repositories::{
    action::ActionRepository,
    key::{CreateKeyInput, KeyRepository, KeySearchFilters, UpdateKeyInput},
    pack::PackRepository,
    trigger::SensorRepository,
    Create, Delete, FindByRef, Update,
};
use attune_common::{
    models::{key::Key, OwnerType},
    rbac::{Action, AuthorizationContext, Resource},
};

use crate::auth::{jwt::TokenType, RequireAuth};
use crate::{
    authz::{AuthorizationCheck, AuthorizationService},
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
    user: RequireAuth,
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
    let mut rows = result.rows;

    if user.0.claims.token_type == TokenType::Access {
        let identity_id = user
            .0
            .identity_id()
            .map_err(|_| ApiError::Unauthorized("Invalid user identity".to_string()))?;
        let authz = AuthorizationService::new(state.db.clone());
        let grants = authz.effective_grants(&user.0).await?;

        // Ensure the principal can read at least some key records.
        let can_read_any_key = grants
            .iter()
            .any(|g| g.resource == Resource::Keys && g.actions.contains(&Action::Read));
        if !can_read_any_key {
            return Err(ApiError::Forbidden(
                "Insufficient permissions: keys:read".to_string(),
            ));
        }

        rows.retain(|key| {
            let ctx = key_authorization_context(identity_id, key);
            AuthorizationService::is_allowed(&grants, Resource::Keys, Action::Read, &ctx)
        });
    }

    let paginated_keys: Vec<KeySummary> = rows.into_iter().map(KeySummary::from).collect();

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
    user: RequireAuth,
    State(state): State<Arc<AppState>>,
    Path(key_ref): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let mut key = KeyRepository::find_by_ref(&state.db, &key_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Key '{}' not found", key_ref)))?;

    // For encrypted keys, track whether this caller is permitted to see the value.
    // Non-Access tokens (sensor, execution) always get full access.
    let can_decrypt = if user.0.claims.token_type == TokenType::Access {
        let identity_id = user
            .0
            .identity_id()
            .map_err(|_| ApiError::Unauthorized("Invalid user identity".to_string()))?;
        let authz = AuthorizationService::new(state.db.clone());

        // Basic read check — hide behind 404 to prevent enumeration.
        authz
            .authorize(
                &user.0,
                AuthorizationCheck {
                    resource: Resource::Keys,
                    action: Action::Read,
                    context: key_authorization_context(identity_id, &key),
                },
            )
            .await
            .map_err(|_| ApiError::NotFound(format!("Key '{}' not found", key_ref)))?;

        // For encrypted keys, separately check Keys::Decrypt.
        // Failing this is not an error — we just return the value as null.
        if key.encrypted {
            authz
                .authorize(
                    &user.0,
                    AuthorizationCheck {
                        resource: Resource::Keys,
                        action: Action::Decrypt,
                        context: key_authorization_context(identity_id, &key),
                    },
                )
                .await
                .is_ok()
        } else {
            true
        }
    } else {
        true
    };

    // Decrypt value if encrypted and caller has permission.
    // If they lack Keys::Decrypt, return null rather than the ciphertext.
    if key.encrypted {
        if can_decrypt {
            let encryption_key =
                state
                    .config
                    .security
                    .encryption_key
                    .as_ref()
                    .ok_or_else(|| {
                        ApiError::InternalServerError(
                            "Encryption key not configured on server".to_string(),
                        )
                    })?;

            let decrypted_value = attune_common::crypto::decrypt_json(&key.value, encryption_key)
                .map_err(|e| {
                tracing::error!("Failed to decrypt key '{}': {}", key_ref, e);
                ApiError::InternalServerError(format!("Failed to decrypt key: {}", e))
            })?;

            key.value = decrypted_value;
        } else {
            key.value = serde_json::Value::Null;
        }
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
    user: RequireAuth,
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateKeyRequest>,
) -> ApiResult<impl IntoResponse> {
    // Validate request
    request.validate()?;

    if user.0.claims.token_type == TokenType::Access {
        let identity_id = user
            .0
            .identity_id()
            .map_err(|_| ApiError::Unauthorized("Invalid user identity".to_string()))?;
        let authz = AuthorizationService::new(state.db.clone());
        let mut ctx = AuthorizationContext::new(identity_id);
        ctx.owner_identity_id = request.owner_identity;
        ctx.owner_type = Some(request.owner_type);
        ctx.owner_ref = requested_key_owner_ref(&request);
        ctx.encrypted = Some(request.encrypted);
        ctx.target_ref = Some(request.r#ref.clone());

        authz
            .authorize(
                &user.0,
                AuthorizationCheck {
                    resource: Resource::Keys,
                    action: Action::Create,
                    context: ctx,
                },
            )
            .await?;
    }

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

        let encrypted_value = attune_common::crypto::encrypt_json(&request.value, encryption_key)
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
        key.value =
            attune_common::crypto::decrypt_json(&key.value, encryption_key).map_err(|e| {
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
    user: RequireAuth,
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

    if user.0.claims.token_type == TokenType::Access {
        let identity_id = user
            .0
            .identity_id()
            .map_err(|_| ApiError::Unauthorized("Invalid user identity".to_string()))?;
        let authz = AuthorizationService::new(state.db.clone());
        authz
            .authorize(
                &user.0,
                AuthorizationCheck {
                    resource: Resource::Keys,
                    action: Action::Update,
                    context: key_authorization_context(identity_id, &existing),
                },
            )
            .await?;
    }

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

            let encrypted_value = attune_common::crypto::encrypt_json(&new_value, encryption_key)
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

        updated_key.value = attune_common::crypto::decrypt_json(&updated_key.value, encryption_key)
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
    user: RequireAuth,
    State(state): State<Arc<AppState>>,
    Path(key_ref): Path<String>,
) -> ApiResult<impl IntoResponse> {
    // Verify key exists
    let key = KeyRepository::find_by_ref(&state.db, &key_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Key '{}' not found", key_ref)))?;

    if user.0.claims.token_type == TokenType::Access {
        let identity_id = user
            .0
            .identity_id()
            .map_err(|_| ApiError::Unauthorized("Invalid user identity".to_string()))?;
        let authz = AuthorizationService::new(state.db.clone());
        authz
            .authorize(
                &user.0,
                AuthorizationCheck {
                    resource: Resource::Keys,
                    action: Action::Delete,
                    context: key_authorization_context(identity_id, &key),
                },
            )
            .await?;
    }

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

fn key_authorization_context(identity_id: i64, key: &Key) -> AuthorizationContext {
    let mut ctx = AuthorizationContext::new(identity_id);
    ctx.target_id = Some(key.id);
    ctx.target_ref = Some(key.r#ref.clone());
    ctx.owner_identity_id = key.owner_identity;
    ctx.owner_type = Some(key.owner_type);
    ctx.owner_ref = key_owner_ref(
        key.owner_type,
        key.owner.as_deref(),
        key.owner_pack_ref.as_deref(),
        key.owner_action_ref.as_deref(),
        key.owner_sensor_ref.as_deref(),
    );
    ctx.encrypted = Some(key.encrypted);
    ctx
}

fn requested_key_owner_ref(request: &CreateKeyRequest) -> Option<String> {
    key_owner_ref(
        request.owner_type,
        request.owner.as_deref(),
        request.owner_pack_ref.as_deref(),
        request.owner_action_ref.as_deref(),
        request.owner_sensor_ref.as_deref(),
    )
}

fn key_owner_ref(
    owner_type: OwnerType,
    owner: Option<&str>,
    owner_pack_ref: Option<&str>,
    owner_action_ref: Option<&str>,
    owner_sensor_ref: Option<&str>,
) -> Option<String> {
    match owner_type {
        OwnerType::Pack => owner_pack_ref.map(str::to_string),
        OwnerType::Action => owner_action_ref.map(str::to_string),
        OwnerType::Sensor => owner_sensor_ref.map(str::to_string),
        _ => owner.map(str::to_string),
    }
}
