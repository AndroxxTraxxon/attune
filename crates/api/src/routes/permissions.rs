use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use std::sync::Arc;
use validator::Validate;

use attune_common::{
    models::identity::{Identity, PermissionSet},
    rbac::{Action, AuthorizationContext, Resource},
    repositories::{
        identity::{
            CreateIdentityInput, CreatePermissionAssignmentInput, IdentityRepository,
            PermissionAssignmentRepository, PermissionSetRepository, UpdateIdentityInput,
        },
        Create, Delete, FindById, FindByRef, List, Update,
    },
};

use crate::{
    auth::hash_password,
    auth::middleware::RequireAuth,
    authz::{AuthorizationCheck, AuthorizationService},
    dto::{
        common::{PaginatedResponse, PaginationParams},
        ApiResponse, CreateIdentityRequest, CreatePermissionAssignmentRequest, IdentityResponse,
        IdentitySummary, PermissionAssignmentResponse, PermissionSetQueryParams,
        PermissionSetSummary, SuccessResponse, UpdateIdentityRequest,
    },
    middleware::{ApiError, ApiResult},
    state::AppState,
};

#[utoipa::path(
    get,
    path = "/api/v1/identities",
    tag = "permissions",
    params(PaginationParams),
    responses(
        (status = 200, description = "List identities", body = PaginatedResponse<IdentitySummary>)
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_identities(
    State(state): State<Arc<AppState>>,
    RequireAuth(user): RequireAuth,
    Query(query): Query<PaginationParams>,
) -> ApiResult<impl IntoResponse> {
    authorize_permissions(&state, &user, Resource::Identities, Action::Read).await?;

    let identities = IdentityRepository::list(&state.db).await?;
    let total = identities.len() as u64;
    let start = query.offset() as usize;
    let end = (start + query.limit() as usize).min(identities.len());
    let page_items = if start >= identities.len() {
        Vec::new()
    } else {
        identities[start..end]
            .iter()
            .cloned()
            .map(IdentitySummary::from)
            .collect()
    };

    Ok((
        StatusCode::OK,
        Json(PaginatedResponse::new(page_items, &query, total)),
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/identities/{id}",
    tag = "permissions",
    params(
        ("id" = i64, Path, description = "Identity ID")
    ),
    responses(
        (status = 200, description = "Identity details", body = inline(ApiResponse<IdentityResponse>)),
        (status = 404, description = "Identity not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_identity(
    State(state): State<Arc<AppState>>,
    RequireAuth(user): RequireAuth,
    Path(identity_id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    authorize_permissions(&state, &user, Resource::Identities, Action::Read).await?;

    let identity = IdentityRepository::find_by_id(&state.db, identity_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Identity '{}' not found", identity_id)))?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::new(IdentityResponse::from(identity))),
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/identities",
    tag = "permissions",
    request_body = CreateIdentityRequest,
    responses(
        (status = 201, description = "Identity created", body = inline(ApiResponse<IdentityResponse>)),
        (status = 409, description = "Identity already exists")
    ),
    security(("bearer_auth" = []))
)]
pub async fn create_identity(
    State(state): State<Arc<AppState>>,
    RequireAuth(user): RequireAuth,
    Json(request): Json<CreateIdentityRequest>,
) -> ApiResult<impl IntoResponse> {
    authorize_permissions(&state, &user, Resource::Identities, Action::Create).await?;
    request.validate()?;

    let password_hash = match request.password {
        Some(password) => Some(hash_password(&password)?),
        None => None,
    };

    let identity = IdentityRepository::create(
        &state.db,
        CreateIdentityInput {
            login: request.login,
            display_name: request.display_name,
            password_hash,
            attributes: request.attributes,
        },
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::new(IdentityResponse::from(identity))),
    ))
}

#[utoipa::path(
    put,
    path = "/api/v1/identities/{id}",
    tag = "permissions",
    params(
        ("id" = i64, Path, description = "Identity ID")
    ),
    request_body = UpdateIdentityRequest,
    responses(
        (status = 200, description = "Identity updated", body = inline(ApiResponse<IdentityResponse>)),
        (status = 404, description = "Identity not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn update_identity(
    State(state): State<Arc<AppState>>,
    RequireAuth(user): RequireAuth,
    Path(identity_id): Path<i64>,
    Json(request): Json<UpdateIdentityRequest>,
) -> ApiResult<impl IntoResponse> {
    authorize_permissions(&state, &user, Resource::Identities, Action::Update).await?;

    IdentityRepository::find_by_id(&state.db, identity_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Identity '{}' not found", identity_id)))?;

    let password_hash = match request.password {
        Some(password) => Some(hash_password(&password)?),
        None => None,
    };

    let identity = IdentityRepository::update(
        &state.db,
        identity_id,
        UpdateIdentityInput {
            display_name: request.display_name,
            password_hash,
            attributes: request.attributes,
        },
    )
    .await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::new(IdentityResponse::from(identity))),
    ))
}

#[utoipa::path(
    delete,
    path = "/api/v1/identities/{id}",
    tag = "permissions",
    params(
        ("id" = i64, Path, description = "Identity ID")
    ),
    responses(
        (status = 200, description = "Identity deleted", body = inline(ApiResponse<SuccessResponse>)),
        (status = 404, description = "Identity not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn delete_identity(
    State(state): State<Arc<AppState>>,
    RequireAuth(user): RequireAuth,
    Path(identity_id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    authorize_permissions(&state, &user, Resource::Identities, Action::Delete).await?;

    let caller_identity_id = user
        .identity_id()
        .map_err(|_| ApiError::Unauthorized("Invalid user identity".to_string()))?;
    if caller_identity_id == identity_id {
        return Err(ApiError::BadRequest(
            "Refusing to delete the currently authenticated identity".to_string(),
        ));
    }

    let deleted = IdentityRepository::delete(&state.db, identity_id).await?;
    if !deleted {
        return Err(ApiError::NotFound(format!(
            "Identity '{}' not found",
            identity_id
        )));
    }

    Ok((
        StatusCode::OK,
        Json(ApiResponse::new(SuccessResponse::new(
            "Identity deleted successfully",
        ))),
    ))
}

#[utoipa::path(
    get,
    path = "/api/v1/permissions/sets",
    tag = "permissions",
    params(PermissionSetQueryParams),
    responses(
        (status = 200, description = "List permission sets", body = Vec<PermissionSetSummary>)
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_permission_sets(
    State(state): State<Arc<AppState>>,
    RequireAuth(user): RequireAuth,
    Query(query): Query<PermissionSetQueryParams>,
) -> ApiResult<impl IntoResponse> {
    authorize_permissions(&state, &user, Resource::Permissions, Action::Read).await?;

    let mut permission_sets = PermissionSetRepository::list(&state.db).await?;
    if let Some(pack_ref) = &query.pack_ref {
        permission_sets.retain(|ps| ps.pack_ref.as_deref() == Some(pack_ref.as_str()));
    }

    let response: Vec<PermissionSetSummary> = permission_sets
        .into_iter()
        .map(PermissionSetSummary::from)
        .collect();

    Ok((StatusCode::OK, Json(response)))
}

#[utoipa::path(
    get,
    path = "/api/v1/identities/{id}/permissions",
    tag = "permissions",
    params(
        ("id" = i64, Path, description = "Identity ID")
    ),
    responses(
        (status = 200, description = "List permission assignments for an identity", body = Vec<PermissionAssignmentResponse>),
        (status = 404, description = "Identity not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_identity_permissions(
    State(state): State<Arc<AppState>>,
    RequireAuth(user): RequireAuth,
    Path(identity_id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    authorize_permissions(&state, &user, Resource::Permissions, Action::Read).await?;

    IdentityRepository::find_by_id(&state.db, identity_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Identity '{}' not found", identity_id)))?;

    let assignments =
        PermissionAssignmentRepository::find_by_identity(&state.db, identity_id).await?;
    let permission_sets = PermissionSetRepository::find_by_identity(&state.db, identity_id).await?;

    let permission_set_refs = permission_sets
        .into_iter()
        .map(|ps| (ps.id, ps.r#ref))
        .collect::<std::collections::HashMap<_, _>>();

    let response: Vec<PermissionAssignmentResponse> = assignments
        .into_iter()
        .filter_map(|assignment| {
            permission_set_refs
                .get(&assignment.permset)
                .cloned()
                .map(|permission_set_ref| PermissionAssignmentResponse {
                    id: assignment.id,
                    identity_id: assignment.identity,
                    permission_set_id: assignment.permset,
                    permission_set_ref,
                    created: assignment.created,
                })
        })
        .collect();

    Ok((StatusCode::OK, Json(response)))
}

#[utoipa::path(
    post,
    path = "/api/v1/permissions/assignments",
    tag = "permissions",
    request_body = CreatePermissionAssignmentRequest,
    responses(
        (status = 201, description = "Permission assignment created", body = inline(ApiResponse<PermissionAssignmentResponse>)),
        (status = 404, description = "Identity or permission set not found"),
        (status = 409, description = "Assignment already exists")
    ),
    security(("bearer_auth" = []))
)]
pub async fn create_permission_assignment(
    State(state): State<Arc<AppState>>,
    RequireAuth(user): RequireAuth,
    Json(request): Json<CreatePermissionAssignmentRequest>,
) -> ApiResult<impl IntoResponse> {
    authorize_permissions(&state, &user, Resource::Permissions, Action::Manage).await?;

    let identity = resolve_identity(&state, &request).await?;
    let permission_set =
        PermissionSetRepository::find_by_ref(&state.db, &request.permission_set_ref)
            .await?
            .ok_or_else(|| {
                ApiError::NotFound(format!(
                    "Permission set '{}' not found",
                    request.permission_set_ref
                ))
            })?;

    let assignment = PermissionAssignmentRepository::create(
        &state.db,
        CreatePermissionAssignmentInput {
            identity: identity.id,
            permset: permission_set.id,
        },
    )
    .await?;

    let response = PermissionAssignmentResponse {
        id: assignment.id,
        identity_id: assignment.identity,
        permission_set_id: assignment.permset,
        permission_set_ref: permission_set.r#ref,
        created: assignment.created,
    };

    Ok((StatusCode::CREATED, Json(ApiResponse::new(response))))
}

#[utoipa::path(
    delete,
    path = "/api/v1/permissions/assignments/{id}",
    tag = "permissions",
    params(
        ("id" = i64, Path, description = "Permission assignment ID")
    ),
    responses(
        (status = 200, description = "Permission assignment deleted", body = inline(ApiResponse<SuccessResponse>)),
        (status = 404, description = "Assignment not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn delete_permission_assignment(
    State(state): State<Arc<AppState>>,
    RequireAuth(user): RequireAuth,
    Path(assignment_id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    authorize_permissions(&state, &user, Resource::Permissions, Action::Manage).await?;

    let existing = PermissionAssignmentRepository::find_by_id(&state.db, assignment_id)
        .await?
        .ok_or_else(|| {
            ApiError::NotFound(format!(
                "Permission assignment '{}' not found",
                assignment_id
            ))
        })?;

    let deleted = PermissionAssignmentRepository::delete(&state.db, existing.id).await?;
    if !deleted {
        return Err(ApiError::NotFound(format!(
            "Permission assignment '{}' not found",
            assignment_id
        )));
    }

    Ok((
        StatusCode::OK,
        Json(ApiResponse::new(SuccessResponse::new(
            "Permission assignment deleted successfully",
        ))),
    ))
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/identities", get(list_identities).post(create_identity))
        .route(
            "/identities/{id}",
            get(get_identity)
                .put(update_identity)
                .delete(delete_identity),
        )
        .route(
            "/identities/{id}/permissions",
            get(list_identity_permissions),
        )
        .route("/permissions/sets", get(list_permission_sets))
        .route(
            "/permissions/assignments",
            post(create_permission_assignment),
        )
        .route(
            "/permissions/assignments/{id}",
            delete(delete_permission_assignment),
        )
}

async fn authorize_permissions(
    state: &Arc<AppState>,
    user: &crate::auth::middleware::AuthenticatedUser,
    resource: Resource,
    action: Action,
) -> ApiResult<()> {
    let identity_id = user
        .identity_id()
        .map_err(|_| ApiError::Unauthorized("Invalid user identity".to_string()))?;
    let authz = AuthorizationService::new(state.db.clone());
    authz
        .authorize(
            user,
            AuthorizationCheck {
                resource,
                action,
                context: AuthorizationContext::new(identity_id),
            },
        )
        .await
}

async fn resolve_identity(
    state: &Arc<AppState>,
    request: &CreatePermissionAssignmentRequest,
) -> ApiResult<Identity> {
    match (request.identity_id, request.identity_login.as_deref()) {
        (Some(identity_id), None) => IdentityRepository::find_by_id(&state.db, identity_id)
            .await?
            .ok_or_else(|| ApiError::NotFound(format!("Identity '{}' not found", identity_id))),
        (None, Some(identity_login)) => {
            IdentityRepository::find_by_login(&state.db, identity_login)
                .await?
                .ok_or_else(|| {
                    ApiError::NotFound(format!("Identity '{}' not found", identity_login))
                })
        }
        (Some(_), Some(_)) => Err(ApiError::BadRequest(
            "Provide either identity_id or identity_login, not both".to_string(),
        )),
        (None, None) => Err(ApiError::BadRequest(
            "Either identity_id or identity_login is required".to_string(),
        )),
    }
}

impl From<Identity> for IdentitySummary {
    fn from(value: Identity) -> Self {
        Self {
            id: value.id,
            login: value.login,
            display_name: value.display_name,
            attributes: value.attributes,
        }
    }
}

impl From<PermissionSet> for PermissionSetSummary {
    fn from(value: PermissionSet) -> Self {
        Self {
            id: value.id,
            r#ref: value.r#ref,
            pack_ref: value.pack_ref,
            label: value.label,
            description: value.description,
            grants: value.grants,
        }
    }
}
