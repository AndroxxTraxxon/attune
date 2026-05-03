//! Action management API routes

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde_json::json;
use std::sync::Arc;
use validator::Validate;

use attune_common::rbac::{Action, AuthorizationContext, Resource};
use attune_common::repositories::{
    action::{ActionRepository, ActionSearchFilters, CreateActionInput, UpdateActionInput},
    pack::PackRepository,
    queue_stats::QueueStatsRepository,
    runtime::RuntimeRepository,
    Create, Delete, FindByRef, Patch, Update,
};

use crate::{
    auth::middleware::RequireAuth,
    authz::{AuthorizationCheck, AuthorizationService},
    dto::{
        action::{
            ActionResponse, ActionSearchHit, ActionSearchParams, ActionSummary,
            CreateActionRequest, QueueStatsResponse, RuntimeVersionConstraintPatch,
            UpdateActionRequest,
        },
        common::{PaginatedResponse, PaginationParams},
        ApiResponse, SuccessResponse,
    },
    middleware::{ApiError, ApiResult},
    state::AppState,
};

/// List all actions with pagination
#[utoipa::path(
    get,
    path = "/api/v1/actions",
    tag = "actions",
    params(PaginationParams),
    responses(
        (status = 200, description = "List of actions", body = PaginatedResponse<ActionSummary>),
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_actions(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Query(pagination): Query<PaginationParams>,
) -> ApiResult<impl IntoResponse> {
    // All filtering and pagination happen in a single SQL query.
    let filters = ActionSearchFilters {
        pack: None,
        packs: Vec::new(),
        query: None,
        limit: pagination.limit(),
        offset: pagination.offset(),
    };

    let result = ActionRepository::list_search(&state.db, &filters).await?;

    let runtime_refs =
        fetch_runtime_refs(&state, result.rows.iter().filter_map(|a| a.runtime)).await?;
    let paginated_actions: Vec<ActionSummary> = result
        .rows
        .into_iter()
        .map(|a| {
            let mut summary = ActionSummary::from(a);
            summary.runtime_ref = summary
                .runtime
                .and_then(|id| runtime_refs.get(&id).cloned());
            summary
        })
        .collect();

    let response = PaginatedResponse::new(paginated_actions, &pagination, result.total);

    Ok((StatusCode::OK, Json(response)))
}

/// List actions by pack reference
#[utoipa::path(
    get,
    path = "/api/v1/packs/{pack_ref}/actions",
    tag = "actions",
    params(
        ("pack_ref" = String, Path, description = "Pack reference identifier"),
        PaginationParams
    ),
    responses(
        (status = 200, description = "List of actions for pack", body = PaginatedResponse<ActionSummary>),
        (status = 404, description = "Pack not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_actions_by_pack(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(pack_ref): Path<String>,
    Query(pagination): Query<PaginationParams>,
) -> ApiResult<impl IntoResponse> {
    // Verify pack exists
    let pack = PackRepository::find_by_ref(&state.db, &pack_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Pack '{}' not found", pack_ref)))?;

    // All filtering and pagination happen in a single SQL query.
    let filters = ActionSearchFilters {
        pack: Some(pack.id),
        packs: Vec::new(),
        query: None,
        limit: pagination.limit(),
        offset: pagination.offset(),
    };

    let result = ActionRepository::list_search(&state.db, &filters).await?;

    let runtime_refs =
        fetch_runtime_refs(&state, result.rows.iter().filter_map(|a| a.runtime)).await?;
    let paginated_actions: Vec<ActionSummary> = result
        .rows
        .into_iter()
        .map(|a| {
            let mut summary = ActionSummary::from(a);
            summary.runtime_ref = summary
                .runtime
                .and_then(|id| runtime_refs.get(&id).cloned());
            summary
        })
        .collect();

    let response = PaginatedResponse::new(paginated_actions, &pagination, result.total);

    Ok((StatusCode::OK, Json(response)))
}

/// Get a single action by reference
#[utoipa::path(
    get,
    path = "/api/v1/actions/{ref}",
    tag = "actions",
    params(
        ("ref" = String, Path, description = "Action reference identifier")
    ),
    responses(
        (status = 200, description = "Action details", body = inline(ApiResponse<ActionResponse>)),
        (status = 404, description = "Action not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_action(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(action_ref): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let action = ActionRepository::find_by_ref(&state.db, &action_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Action '{}' not found", action_ref)))?;

    let mut response_body = ActionResponse::from(action);
    response_body.runtime_ref = resolve_runtime_ref(&state, response_body.runtime).await?;
    let response = ApiResponse::new(response_body);

    Ok((StatusCode::OK, Json(response)))
}

/// Create a new action
#[utoipa::path(
    post,
    path = "/api/v1/actions",
    tag = "actions",
    request_body = CreateActionRequest,
    responses(
        (status = 201, description = "Action created successfully", body = inline(ApiResponse<ActionResponse>)),
        (status = 400, description = "Validation error"),
        (status = 404, description = "Pack not found"),
        (status = 409, description = "Action with same ref already exists")
    ),
    security(("bearer_auth" = []))
)]
pub async fn create_action(
    State(state): State<Arc<AppState>>,
    RequireAuth(user): RequireAuth,
    Json(request): Json<CreateActionRequest>,
) -> ApiResult<impl IntoResponse> {
    // Validate request
    request.validate()?;

    // Check if action with same ref already exists
    if ActionRepository::find_by_ref(&state.db, &request.r#ref)
        .await?
        .is_some()
    {
        return Err(ApiError::Conflict(format!(
            "Action with ref '{}' already exists",
            request.r#ref
        )));
    }

    // Verify pack exists and get its ID
    let pack = PackRepository::find_by_ref(&state.db, &request.pack_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Pack '{}' not found", request.pack_ref)))?;

    if user.claims.token_type == crate::auth::jwt::TokenType::Access {
        let identity_id = user
            .identity_id()
            .map_err(|_| ApiError::Unauthorized("Invalid user identity".to_string()))?;
        let authz = AuthorizationService::new(state.db.clone());
        let mut ctx = AuthorizationContext::new(identity_id);
        ctx.pack_ref = Some(pack.r#ref.clone());
        ctx.target_ref = Some(request.r#ref.clone());
        authz
            .authorize(
                &user,
                AuthorizationCheck {
                    resource: Resource::Actions,
                    action: Action::Create,
                    context: ctx,
                },
            )
            .await?;
    }

    let runtime =
        resolve_runtime_id(&state, request.runtime, request.runtime_ref.as_deref()).await?;

    // Create action input
    let action_input = CreateActionInput {
        r#ref: request.r#ref,
        pack: pack.id,
        pack_ref: pack.r#ref.clone(),
        label: request.label,
        description: request.description,
        entrypoint: request.entrypoint,
        runtime,
        runtime_version_constraint: request.runtime_version_constraint,
        required_worker_runtimes: json!(request.required_worker_runtimes),
        param_schema: request.param_schema,
        out_schema: request.out_schema,
        is_adhoc: true, // Actions created via API are ad-hoc (not from pack installation)
        accesses_mcp: request.accesses_mcp.unwrap_or(false),
    };

    let action = ActionRepository::create(&state.db, action_input).await?;

    let mut response_body = ActionResponse::from(action);
    response_body.runtime_ref = resolve_runtime_ref(&state, response_body.runtime).await?;
    let response = ApiResponse::with_message(response_body, "Action created successfully");

    Ok((StatusCode::CREATED, Json(response)))
}

/// Update an existing action
#[utoipa::path(
    put,
    path = "/api/v1/actions/{ref}",
    tag = "actions",
    params(
        ("ref" = String, Path, description = "Action reference identifier")
    ),
    request_body = UpdateActionRequest,
    responses(
        (status = 200, description = "Action updated successfully", body = inline(ApiResponse<ActionResponse>)),
        (status = 400, description = "Validation error"),
        (status = 404, description = "Action not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn update_action(
    State(state): State<Arc<AppState>>,
    RequireAuth(user): RequireAuth,
    Path(action_ref): Path<String>,
    Json(request): Json<UpdateActionRequest>,
) -> ApiResult<impl IntoResponse> {
    // Validate request
    request.validate()?;

    // Check if action exists
    let existing_action = ActionRepository::find_by_ref(&state.db, &action_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Action '{}' not found", action_ref)))?;

    if user.claims.token_type == crate::auth::jwt::TokenType::Access {
        let identity_id = user
            .identity_id()
            .map_err(|_| ApiError::Unauthorized("Invalid user identity".to_string()))?;
        let authz = AuthorizationService::new(state.db.clone());
        let mut ctx = AuthorizationContext::new(identity_id);
        ctx.target_id = Some(existing_action.id);
        ctx.target_ref = Some(existing_action.r#ref.clone());
        ctx.pack_ref = Some(existing_action.pack_ref.clone());
        authz
            .authorize(
                &user,
                AuthorizationCheck {
                    resource: Resource::Actions,
                    action: Action::Update,
                    context: ctx,
                },
            )
            .await?;
    }

    let runtime =
        resolve_runtime_id(&state, request.runtime, request.runtime_ref.as_deref()).await?;

    // Create update input
    let update_input = UpdateActionInput {
        label: request.label,
        description: request.description.map(Patch::Set),
        entrypoint: request.entrypoint,
        runtime,
        runtime_version_constraint: request.runtime_version_constraint.map(|patch| match patch {
            RuntimeVersionConstraintPatch::Set(value) => Patch::Set(value),
            RuntimeVersionConstraintPatch::Clear => Patch::Clear,
        }),
        required_worker_runtimes: request
            .required_worker_runtimes
            .map(|runtimes| json!(runtimes)),
        param_schema: request.param_schema,
        out_schema: request.out_schema,
        parameter_delivery: None,
        parameter_format: None,
        output_format: None,
        accesses_mcp: request.accesses_mcp,
    };

    let action = ActionRepository::update(&state.db, existing_action.id, update_input).await?;

    let mut response_body = ActionResponse::from(action);
    response_body.runtime_ref = resolve_runtime_ref(&state, response_body.runtime).await?;
    let response = ApiResponse::with_message(response_body, "Action updated successfully");

    Ok((StatusCode::OK, Json(response)))
}

/// Delete an action
#[utoipa::path(
    delete,
    path = "/api/v1/actions/{ref}",
    tag = "actions",
    params(
        ("ref" = String, Path, description = "Action reference identifier")
    ),
    responses(
        (status = 200, description = "Action deleted successfully", body = SuccessResponse),
        (status = 404, description = "Action not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn delete_action(
    State(state): State<Arc<AppState>>,
    RequireAuth(user): RequireAuth,
    Path(action_ref): Path<String>,
) -> ApiResult<impl IntoResponse> {
    // Check if action exists
    let action = ActionRepository::find_by_ref(&state.db, &action_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Action '{}' not found", action_ref)))?;

    if user.claims.token_type == crate::auth::jwt::TokenType::Access {
        let identity_id = user
            .identity_id()
            .map_err(|_| ApiError::Unauthorized("Invalid user identity".to_string()))?;
        let authz = AuthorizationService::new(state.db.clone());
        let mut ctx = AuthorizationContext::new(identity_id);
        ctx.target_id = Some(action.id);
        ctx.target_ref = Some(action.r#ref.clone());
        ctx.pack_ref = Some(action.pack_ref.clone());
        authz
            .authorize(
                &user,
                AuthorizationCheck {
                    resource: Resource::Actions,
                    action: Action::Delete,
                    context: ctx,
                },
            )
            .await?;
    }

    // Delete the action
    let deleted = ActionRepository::delete(&state.db, action.id).await?;

    if !deleted {
        return Err(ApiError::NotFound(format!(
            "Action '{}' not found",
            action_ref
        )));
    }

    let response = SuccessResponse::new(format!("Action '{}' deleted successfully", action_ref));

    Ok((StatusCode::OK, Json(response)))
}

/// Get queue statistics for an action
#[utoipa::path(
    get,
    path = "/api/v1/actions/{ref}/queue-stats",
    tag = "actions",
    params(
        ("ref" = String, Path, description = "Action reference identifier")
    ),
    responses(
        (status = 200, description = "Queue statistics", body = inline(ApiResponse<QueueStatsResponse>)),
        (status = 404, description = "Action not found or no queue statistics available")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_queue_stats(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(action_ref): Path<String>,
) -> ApiResult<impl IntoResponse> {
    // Find the action by reference
    let action = ActionRepository::find_by_ref(&state.db, &action_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Action '{}' not found", action_ref)))?;

    // Get queue statistics from database
    let queue_stats = QueueStatsRepository::find_by_action(&state.db, action.id)
        .await?
        .ok_or_else(|| {
            ApiError::NotFound(format!(
                "No queue statistics available for action '{}'",
                action_ref
            ))
        })?;

    // Convert to response DTO and populate action_ref
    let mut response_stats = QueueStatsResponse::from(queue_stats);
    response_stats.action_ref = action.r#ref.clone();

    let response = ApiResponse::new(response_stats);

    Ok((StatusCode::OK, Json(response)))
}

/// Create action routes
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/actions", get(list_actions).post(create_action))
        .route("/actions/search", get(search_actions))
        .route(
            "/actions/{ref}",
            get(get_action).put(update_action).delete(delete_action),
        )
        .route("/actions/{ref}/queue-stats", get(get_queue_stats))
        .route("/packs/{pack_ref}/actions", get(list_actions_by_pack))
}

/// Search for actions by keyword and pack filter.
///
/// Returns lean `ActionSearchHit` rows optimized for action discovery — useful
/// for AI agents and human browsing of large action catalogs. Whitespace-separated
/// tokens in `q` are AND-matched (each token must appear in at least one of
/// `ref`, `label`, `description`, or `pack_ref`).
#[utoipa::path(
    get,
    path = "/api/v1/actions/search",
    tag = "actions",
    params(ActionSearchParams, PaginationParams),
    responses(
        (status = 200, description = "Matching actions", body = PaginatedResponse<ActionSearchHit>),
        (status = 404, description = "One or more pack refs not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn search_actions(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Query(search): Query<ActionSearchParams>,
    Query(pagination): Query<PaginationParams>,
) -> ApiResult<impl IntoResponse> {
    // Resolve pack refs (comma-separated) to IDs in a single batched query.
    // Unknown packs return 404 so callers don't get silently empty results
    // from typos.
    let pack_ids: Vec<i64> = if let Some(ref packs_str) = search.packs {
        let refs: Vec<&str> = packs_str
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        if refs.is_empty() {
            Vec::new()
        } else {
            let id_map = PackRepository::find_ids_by_refs(&state.db, &refs).await?;
            let mut ids = Vec::with_capacity(refs.len());
            for pack_ref in &refs {
                let id = id_map
                    .get(*pack_ref)
                    .ok_or_else(|| ApiError::NotFound(format!("Pack '{}' not found", pack_ref)))?;
                ids.push(*id);
            }
            ids
        }
    } else {
        Vec::new()
    };

    let query = search
        .q
        .as_ref()
        .map(|q| q.trim().to_string())
        .filter(|q| !q.is_empty());

    let filters = ActionSearchFilters {
        pack: None,
        packs: pack_ids,
        query,
        limit: pagination.limit(),
        offset: pagination.offset(),
    };

    let result = ActionRepository::list_search(&state.db, &filters).await?;

    let runtime_refs =
        fetch_runtime_refs(&state, result.rows.iter().filter_map(|a| a.runtime)).await?;
    let hits: Vec<ActionSearchHit> = result
        .rows
        .into_iter()
        .map(|a| {
            let runtime_id = a.runtime;
            let mut hit = ActionSearchHit::from(a);
            hit.runtime_ref = runtime_id.and_then(|id| runtime_refs.get(&id).cloned());
            hit
        })
        .collect();

    let response = PaginatedResponse::new(hits, &pagination, result.total);

    Ok((StatusCode::OK, Json(response)))
}

/// Bulk-resolve runtime IDs to refs.
async fn fetch_runtime_refs(
    state: &Arc<AppState>,
    ids: impl IntoIterator<Item = i64>,
) -> ApiResult<std::collections::HashMap<i64, String>> {
    let unique: Vec<i64> = {
        let mut set = std::collections::HashSet::new();
        ids.into_iter().filter(|id| set.insert(*id)).collect()
    };
    if unique.is_empty() {
        return Ok(std::collections::HashMap::new());
    }
    Ok(RuntimeRepository::find_refs_by_ids(&state.db, &unique).await?)
}

/// Resolve a single runtime ID to its ref.
async fn resolve_runtime_ref(
    state: &Arc<AppState>,
    runtime_id: Option<i64>,
) -> ApiResult<Option<String>> {
    let Some(id) = runtime_id else {
        return Ok(None);
    };
    let refs = RuntimeRepository::find_refs_by_ids(&state.db, &[id]).await?;
    Ok(refs.get(&id).cloned())
}

async fn resolve_runtime_id(
    state: &Arc<AppState>,
    runtime_id: Option<i64>,
    runtime_ref: Option<&str>,
) -> ApiResult<Option<i64>> {
    if runtime_id.is_some() {
        return Ok(runtime_id);
    }
    let Some(runtime_ref) = runtime_ref else {
        return Ok(None);
    };
    let runtime = RuntimeRepository::find_by_ref(&state.db, runtime_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Runtime '{}' not found", runtime_ref)))?;
    Ok(Some(runtime.id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_routes_structure() {
        // Just verify the router can be constructed
        let _router = routes();
    }
}
