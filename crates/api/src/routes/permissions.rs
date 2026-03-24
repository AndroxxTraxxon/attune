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
    models::identity::{Identity, IdentityRoleAssignment},
    rbac::{Action, AuthorizationContext, Resource},
    repositories::{
        identity::{
            CreateIdentityInput, CreateIdentityRoleAssignmentInput,
            CreatePermissionAssignmentInput, CreatePermissionSetRoleAssignmentInput,
            IdentityRepository, IdentityRoleAssignmentRepository, PermissionAssignmentRepository,
            PermissionSetRepository, PermissionSetRoleAssignmentRepository, UpdateIdentityInput,
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
        ApiResponse, CreateIdentityRequest, CreateIdentityRoleAssignmentRequest,
        CreatePermissionAssignmentRequest, CreatePermissionSetRoleAssignmentRequest,
        IdentityResponse, IdentityRoleAssignmentResponse, IdentitySummary,
        PermissionAssignmentResponse, PermissionSetQueryParams,
        PermissionSetRoleAssignmentResponse, PermissionSetSummary, SuccessResponse,
        UpdateIdentityRequest,
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
        identities[start..end].to_vec()
    };

    let mut summaries = Vec::with_capacity(page_items.len());
    for identity in page_items {
        let role_assignments =
            IdentityRoleAssignmentRepository::find_by_identity(&state.db, identity.id).await?;
        let roles = role_assignments.into_iter().map(|ra| ra.role).collect();
        let mut summary = IdentitySummary::from(identity);
        summary.roles = roles;
        summaries.push(summary);
    }

    Ok((
        StatusCode::OK,
        Json(PaginatedResponse::new(summaries, &query, total)),
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
    let roles = IdentityRoleAssignmentRepository::find_by_identity(&state.db, identity_id).await?;
    let assignments =
        PermissionAssignmentRepository::find_by_identity(&state.db, identity_id).await?;
    let permission_sets = PermissionSetRepository::find_by_identity(&state.db, identity_id).await?;
    let permission_set_refs = permission_sets
        .into_iter()
        .map(|ps| (ps.id, ps.r#ref))
        .collect::<std::collections::HashMap<_, _>>();

    Ok((
        StatusCode::OK,
        Json(ApiResponse::new(IdentityResponse {
            id: identity.id,
            login: identity.login,
            display_name: identity.display_name,
            frozen: identity.frozen,
            attributes: identity.attributes,
            roles: roles
                .into_iter()
                .map(IdentityRoleAssignmentResponse::from)
                .collect(),
            direct_permissions: assignments
                .into_iter()
                .filter_map(|assignment| {
                    permission_set_refs.get(&assignment.permset).cloned().map(
                        |permission_set_ref| PermissionAssignmentResponse {
                            id: assignment.id,
                            identity_id: assignment.identity,
                            permission_set_id: assignment.permset,
                            permission_set_ref,
                            created: assignment.created,
                        },
                    )
                })
                .collect(),
        })),
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
            frozen: request.frozen,
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

    let mut response = Vec::with_capacity(permission_sets.len());
    for permission_set in permission_sets {
        let permission_set_ref = permission_set.r#ref.clone();
        let roles = PermissionSetRoleAssignmentRepository::find_by_permission_set(
            &state.db,
            permission_set.id,
        )
        .await?;
        response.push(PermissionSetSummary {
            id: permission_set.id,
            r#ref: permission_set.r#ref,
            pack_ref: permission_set.pack_ref,
            label: permission_set.label,
            description: permission_set.description,
            grants: permission_set.grants,
            roles: roles
                .into_iter()
                .map(|assignment| PermissionSetRoleAssignmentResponse {
                    id: assignment.id,
                    permission_set_id: assignment.permset,
                    permission_set_ref: Some(permission_set_ref.clone()),
                    role: assignment.role,
                    created: assignment.created,
                })
                .collect(),
        });
    }

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

#[utoipa::path(
    post,
    path = "/api/v1/identities/{id}/roles",
    tag = "permissions",
    params(
        ("id" = i64, Path, description = "Identity ID")
    ),
    request_body = CreateIdentityRoleAssignmentRequest,
    responses(
        (status = 201, description = "Identity role assignment created", body = inline(ApiResponse<IdentityRoleAssignmentResponse>)),
        (status = 404, description = "Identity not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn create_identity_role_assignment(
    State(state): State<Arc<AppState>>,
    RequireAuth(user): RequireAuth,
    Path(identity_id): Path<i64>,
    Json(request): Json<CreateIdentityRoleAssignmentRequest>,
) -> ApiResult<impl IntoResponse> {
    authorize_permissions(&state, &user, Resource::Permissions, Action::Manage).await?;
    request.validate()?;

    IdentityRepository::find_by_id(&state.db, identity_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Identity '{}' not found", identity_id)))?;

    let assignment = IdentityRoleAssignmentRepository::create(
        &state.db,
        CreateIdentityRoleAssignmentInput {
            identity: identity_id,
            role: request.role,
            source: "manual".to_string(),
            managed: false,
        },
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::new(IdentityRoleAssignmentResponse::from(
            assignment,
        ))),
    ))
}

#[utoipa::path(
    delete,
    path = "/api/v1/identities/roles/{id}",
    tag = "permissions",
    params(
        ("id" = i64, Path, description = "Identity role assignment ID")
    ),
    responses(
        (status = 200, description = "Identity role assignment deleted", body = inline(ApiResponse<SuccessResponse>)),
        (status = 404, description = "Identity role assignment not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn delete_identity_role_assignment(
    State(state): State<Arc<AppState>>,
    RequireAuth(user): RequireAuth,
    Path(assignment_id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    authorize_permissions(&state, &user, Resource::Permissions, Action::Manage).await?;

    let assignment = IdentityRoleAssignmentRepository::find_by_id(&state.db, assignment_id)
        .await?
        .ok_or_else(|| {
            ApiError::NotFound(format!(
                "Identity role assignment '{}' not found",
                assignment_id
            ))
        })?;

    if assignment.managed {
        return Err(ApiError::BadRequest(
            "Managed role assignments must be updated through the identity provider sync"
                .to_string(),
        ));
    }

    IdentityRoleAssignmentRepository::delete(&state.db, assignment_id).await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::new(SuccessResponse::new(
            "Identity role assignment deleted successfully",
        ))),
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/permissions/sets/{id}/roles",
    tag = "permissions",
    params(
        ("id" = i64, Path, description = "Permission set ID")
    ),
    request_body = CreatePermissionSetRoleAssignmentRequest,
    responses(
        (status = 201, description = "Permission set role assignment created", body = inline(ApiResponse<PermissionSetRoleAssignmentResponse>)),
        (status = 404, description = "Permission set not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn create_permission_set_role_assignment(
    State(state): State<Arc<AppState>>,
    RequireAuth(user): RequireAuth,
    Path(permission_set_id): Path<i64>,
    Json(request): Json<CreatePermissionSetRoleAssignmentRequest>,
) -> ApiResult<impl IntoResponse> {
    authorize_permissions(&state, &user, Resource::Permissions, Action::Manage).await?;
    request.validate()?;

    let permission_set = PermissionSetRepository::find_by_id(&state.db, permission_set_id)
        .await?
        .ok_or_else(|| {
            ApiError::NotFound(format!("Permission set '{}' not found", permission_set_id))
        })?;

    let assignment = PermissionSetRoleAssignmentRepository::create(
        &state.db,
        CreatePermissionSetRoleAssignmentInput {
            permset: permission_set_id,
            role: request.role,
        },
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::new(PermissionSetRoleAssignmentResponse {
            id: assignment.id,
            permission_set_id: assignment.permset,
            permission_set_ref: Some(permission_set.r#ref),
            role: assignment.role,
            created: assignment.created,
        })),
    ))
}

#[utoipa::path(
    delete,
    path = "/api/v1/permissions/sets/roles/{id}",
    tag = "permissions",
    params(
        ("id" = i64, Path, description = "Permission set role assignment ID")
    ),
    responses(
        (status = 200, description = "Permission set role assignment deleted", body = inline(ApiResponse<SuccessResponse>)),
        (status = 404, description = "Permission set role assignment not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn delete_permission_set_role_assignment(
    State(state): State<Arc<AppState>>,
    RequireAuth(user): RequireAuth,
    Path(assignment_id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    authorize_permissions(&state, &user, Resource::Permissions, Action::Manage).await?;

    PermissionSetRoleAssignmentRepository::find_by_id(&state.db, assignment_id)
        .await?
        .ok_or_else(|| {
            ApiError::NotFound(format!(
                "Permission set role assignment '{}' not found",
                assignment_id
            ))
        })?;

    PermissionSetRoleAssignmentRepository::delete(&state.db, assignment_id).await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::new(SuccessResponse::new(
            "Permission set role assignment deleted successfully",
        ))),
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/identities/{id}/freeze",
    tag = "permissions",
    params(
        ("id" = i64, Path, description = "Identity ID")
    ),
    responses(
        (status = 200, description = "Identity frozen", body = inline(ApiResponse<SuccessResponse>)),
        (status = 404, description = "Identity not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn freeze_identity(
    State(state): State<Arc<AppState>>,
    RequireAuth(user): RequireAuth,
    Path(identity_id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    set_identity_frozen(&state, &user, identity_id, true).await
}

#[utoipa::path(
    post,
    path = "/api/v1/identities/{id}/unfreeze",
    tag = "permissions",
    params(
        ("id" = i64, Path, description = "Identity ID")
    ),
    responses(
        (status = 200, description = "Identity unfrozen", body = inline(ApiResponse<SuccessResponse>)),
        (status = 404, description = "Identity not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn unfreeze_identity(
    State(state): State<Arc<AppState>>,
    RequireAuth(user): RequireAuth,
    Path(identity_id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    set_identity_frozen(&state, &user, identity_id, false).await
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
            "/identities/{id}/roles",
            post(create_identity_role_assignment),
        )
        .route(
            "/identities/{id}/permissions",
            get(list_identity_permissions),
        )
        .route("/identities/{id}/freeze", post(freeze_identity))
        .route("/identities/{id}/unfreeze", post(unfreeze_identity))
        .route(
            "/identities/roles/{id}",
            delete(delete_identity_role_assignment),
        )
        .route("/permissions/sets", get(list_permission_sets))
        .route(
            "/permissions/sets/{id}/roles",
            post(create_permission_set_role_assignment),
        )
        .route(
            "/permissions/sets/roles/{id}",
            delete(delete_permission_set_role_assignment),
        )
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
            frozen: value.frozen,
            attributes: value.attributes,
            roles: Vec::new(),
        }
    }
}

impl From<IdentityRoleAssignment> for IdentityRoleAssignmentResponse {
    fn from(value: IdentityRoleAssignment) -> Self {
        Self {
            id: value.id,
            identity_id: value.identity,
            role: value.role,
            source: value.source,
            managed: value.managed,
            created: value.created,
            updated: value.updated,
        }
    }
}

impl From<Identity> for IdentityResponse {
    fn from(value: Identity) -> Self {
        Self {
            id: value.id,
            login: value.login,
            display_name: value.display_name,
            frozen: value.frozen,
            attributes: value.attributes,
            roles: Vec::new(),
            direct_permissions: Vec::new(),
        }
    }
}

async fn set_identity_frozen(
    state: &Arc<AppState>,
    user: &crate::auth::middleware::AuthenticatedUser,
    identity_id: i64,
    frozen: bool,
) -> ApiResult<impl IntoResponse> {
    authorize_permissions(state, user, Resource::Identities, Action::Update).await?;

    let caller_identity_id = user
        .identity_id()
        .map_err(|_| ApiError::Unauthorized("Invalid user identity".to_string()))?;
    if caller_identity_id == identity_id && frozen {
        return Err(ApiError::BadRequest(
            "Refusing to freeze the currently authenticated identity".to_string(),
        ));
    }

    IdentityRepository::find_by_id(&state.db, identity_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Identity '{}' not found", identity_id)))?;

    IdentityRepository::update(
        &state.db,
        identity_id,
        UpdateIdentityInput {
            display_name: None,
            password_hash: None,
            attributes: None,
            frozen: Some(frozen),
        },
    )
    .await?;

    let message = if frozen {
        "Identity frozen successfully"
    } else {
        "Identity unfrozen successfully"
    };

    Ok((
        StatusCode::OK,
        Json(ApiResponse::new(SuccessResponse::new(message))),
    ))
}
