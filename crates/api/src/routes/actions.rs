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
            ActionResponse, ActionSummary, CreateActionRequest, QueueStatsResponse,
            RuntimeVersionConstraintPatch, UpdateActionRequest,
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

    // If runtime is specified, we could verify it exists (future enhancement)
    // For now, the database foreign key constraint will handle invalid runtime IDs

    // Create action input
    let action_input = CreateActionInput {
        r#ref: request.r#ref,
        pack: pack.id,
        pack_ref: pack.r#ref.clone(),
        label: request.label,
        description: request.description,
        entrypoint: request.entrypoint,
        runtime: request.runtime,
        runtime_version_constraint: request.runtime_version_constraint,
        required_worker_runtimes: json!(request.required_worker_runtimes),
        param_schema: request.param_schema,
        out_schema: request.out_schema,
        is_adhoc: true, // Actions created via API are ad-hoc (not from pack installation)
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

    // Create update input
    let update_input = UpdateActionInput {
        label: request.label,
        description: request.description.map(Patch::Set),
        entrypoint: request.entrypoint,
        runtime: request.runtime,
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
        .route(
            "/actions/{ref}",
            get(get_action).put(update_action).delete(delete_action),
        )
        .route("/actions/{ref}/queue-stats", get(get_queue_stats))
        .route("/packs/{pack_ref}/actions", get(list_actions_by_pack))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_action_routes_structure() {
        // Just verify the router can be constructed
        let _router = routes();
    }
}
