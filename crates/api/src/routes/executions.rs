//! Execution management API routes

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    routing::get,
    Json, Router,
};
use futures::stream::{Stream, StreamExt};
use std::sync::Arc;
use tokio_stream::wrappers::BroadcastStream;

use attune_common::models::enums::ExecutionStatus;
use attune_common::mq::{ExecutionRequestedPayload, MessageEnvelope, MessageType};
use attune_common::repositories::{
    action::ActionRepository,
    execution::{CreateExecutionInput, ExecutionRepository},
    Create, EnforcementRepository, FindById, FindByRef, List,
};

use crate::{
    auth::middleware::RequireAuth,
    dto::{
        common::{PaginatedResponse, PaginationParams},
        execution::{
            CreateExecutionRequest, ExecutionQueryParams, ExecutionResponse, ExecutionSummary,
        },
        ApiResponse,
    },
    middleware::{ApiError, ApiResult},
    state::AppState,
};

/// Create a new execution (manual execution)
///
/// This endpoint allows directly executing an action without a trigger or rule.
/// The execution is queued and will be picked up by the executor service.
#[utoipa::path(
    post,
    path = "/api/v1/executions/execute",
    tag = "executions",
    request_body = CreateExecutionRequest,
    responses(
        (status = 201, description = "Execution created and queued", body = ExecutionResponse),
        (status = 404, description = "Action not found"),
        (status = 400, description = "Invalid request"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn create_execution(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Json(request): Json<CreateExecutionRequest>,
) -> ApiResult<impl IntoResponse> {
    // Validate that the action exists
    let action = ActionRepository::find_by_ref(&state.db, &request.action_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Action '{}' not found", request.action_ref)))?;

    // Create execution input
    let execution_input = CreateExecutionInput {
        action: Some(action.id),
        action_ref: action.r#ref.clone(),
        config: request
            .parameters
            .as_ref()
            .and_then(|p| serde_json::from_value(p.clone()).ok()),
        parent: None,
        enforcement: None,
        executor: None,
        status: ExecutionStatus::Requested,
        result: None,
        workflow_task: None, // Non-workflow execution
    };

    // Insert into database
    let created_execution = ExecutionRepository::create(&state.db, execution_input).await?;

    // Publish ExecutionRequested message to queue
    let payload = ExecutionRequestedPayload {
        execution_id: created_execution.id,
        action_id: Some(action.id),
        action_ref: action.r#ref.clone(),
        parent_id: None,
        enforcement_id: None,
        config: request.parameters,
    };

    let message = MessageEnvelope::new(MessageType::ExecutionRequested, payload)
        .with_source("api-service")
        .with_correlation_id(uuid::Uuid::new_v4());

    if let Some(publisher) = &state.publisher {
        publisher.publish_envelope(&message).await.map_err(|e| {
            ApiError::InternalServerError(format!("Failed to publish message: {}", e))
        })?;
    }

    let response = ExecutionResponse::from(created_execution);

    Ok((StatusCode::CREATED, Json(ApiResponse::new(response))))
}

/// List all executions with pagination and optional filters
#[utoipa::path(
    get,
    path = "/api/v1/executions",
    tag = "executions",
    params(ExecutionQueryParams),
    responses(
        (status = 200, description = "List of executions", body = PaginatedResponse<ExecutionSummary>),
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_executions(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Query(query): Query<ExecutionQueryParams>,
) -> ApiResult<impl IntoResponse> {
    // Get executions based on filters
    let executions = if let Some(status) = query.status {
        // Filter by status
        ExecutionRepository::find_by_status(&state.db, status).await?
    } else if let Some(enforcement_id) = query.enforcement {
        // Filter by enforcement
        ExecutionRepository::find_by_enforcement(&state.db, enforcement_id).await?
    } else {
        // Get all executions
        ExecutionRepository::list(&state.db).await?
    };

    // Apply additional filters in memory (could be optimized with database queries)
    let mut filtered_executions = executions;

    if let Some(action_ref) = &query.action_ref {
        filtered_executions.retain(|e| e.action_ref == *action_ref);
    }

    if let Some(pack_name) = &query.pack_name {
        filtered_executions.retain(|e| {
            // action_ref format is "pack.action"
            e.action_ref.starts_with(&format!("{}.", pack_name))
        });
    }

    if let Some(result_search) = &query.result_contains {
        let search_lower = result_search.to_lowercase();
        filtered_executions.retain(|e| {
            if let Some(result) = &e.result {
                // Convert result to JSON string and search case-insensitively
                let result_str = serde_json::to_string(result).unwrap_or_default();
                result_str.to_lowercase().contains(&search_lower)
            } else {
                false
            }
        });
    }

    if let Some(parent_id) = query.parent {
        filtered_executions.retain(|e| e.parent == Some(parent_id));
    }

    if let Some(executor_id) = query.executor {
        filtered_executions.retain(|e| e.executor == Some(executor_id));
    }

    // Fetch enforcements for all executions to populate rule_ref and trigger_ref
    let enforcement_ids: Vec<i64> = filtered_executions
        .iter()
        .filter_map(|e| e.enforcement)
        .collect();

    let enforcement_map: std::collections::HashMap<i64, _> = if !enforcement_ids.is_empty() {
        let enforcements = EnforcementRepository::list(&state.db).await?;
        enforcements.into_iter().map(|enf| (enf.id, enf)).collect()
    } else {
        std::collections::HashMap::new()
    };

    // Filter by rule_ref if specified
    if let Some(rule_ref) = &query.rule_ref {
        filtered_executions.retain(|e| {
            e.enforcement
                .and_then(|enf_id| enforcement_map.get(&enf_id))
                .map(|enf| enf.rule_ref == *rule_ref)
                .unwrap_or(false)
        });
    }

    // Filter by trigger_ref if specified
    if let Some(trigger_ref) = &query.trigger_ref {
        filtered_executions.retain(|e| {
            e.enforcement
                .and_then(|enf_id| enforcement_map.get(&enf_id))
                .map(|enf| enf.trigger_ref == *trigger_ref)
                .unwrap_or(false)
        });
    }

    // Calculate pagination
    let total = filtered_executions.len() as u64;
    let start = query.offset() as usize;
    let end = (start + query.limit() as usize).min(filtered_executions.len());

    // Get paginated slice and populate rule_ref/trigger_ref from enforcements
    let paginated_executions: Vec<ExecutionSummary> = filtered_executions[start..end]
        .iter()
        .map(|e| {
            let mut summary = ExecutionSummary::from(e.clone());
            if let Some(enf_id) = e.enforcement {
                if let Some(enforcement) = enforcement_map.get(&enf_id) {
                    summary.rule_ref = Some(enforcement.rule_ref.clone());
                    summary.trigger_ref = Some(enforcement.trigger_ref.clone());
                }
            }
            summary
        })
        .collect();

    // Convert query params to pagination params for response
    let pagination_params = PaginationParams {
        page: query.page,
        page_size: query.per_page,
    };

    let response = PaginatedResponse::new(paginated_executions, &pagination_params, total);

    Ok((StatusCode::OK, Json(response)))
}

/// Get a single execution by ID
#[utoipa::path(
    get,
    path = "/api/v1/executions/{id}",
    tag = "executions",
    params(
        ("id" = i64, Path, description = "Execution ID")
    ),
    responses(
        (status = 200, description = "Execution details", body = inline(ApiResponse<ExecutionResponse>)),
        (status = 404, description = "Execution not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_execution(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    let execution = ExecutionRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Execution with ID {} not found", id)))?;

    let response = ApiResponse::new(ExecutionResponse::from(execution));

    Ok((StatusCode::OK, Json(response)))
}

/// List executions by status
#[utoipa::path(
    get,
    path = "/api/v1/executions/status/{status}",
    tag = "executions",
    params(
        ("status" = String, Path, description = "Execution status (requested, scheduling, scheduled, running, completed, failed, canceling, cancelled, timeout, abandoned)"),
        PaginationParams
    ),
    responses(
        (status = 200, description = "List of executions with specified status", body = PaginatedResponse<ExecutionSummary>),
        (status = 400, description = "Invalid status"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_executions_by_status(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(status_str): Path<String>,
    Query(pagination): Query<PaginationParams>,
) -> ApiResult<impl IntoResponse> {
    // Parse status from string
    let status = match status_str.to_lowercase().as_str() {
        "requested" => attune_common::models::enums::ExecutionStatus::Requested,
        "scheduling" => attune_common::models::enums::ExecutionStatus::Scheduling,
        "scheduled" => attune_common::models::enums::ExecutionStatus::Scheduled,
        "running" => attune_common::models::enums::ExecutionStatus::Running,
        "completed" => attune_common::models::enums::ExecutionStatus::Completed,
        "failed" => attune_common::models::enums::ExecutionStatus::Failed,
        "canceling" => attune_common::models::enums::ExecutionStatus::Canceling,
        "cancelled" => attune_common::models::enums::ExecutionStatus::Cancelled,
        "timeout" => attune_common::models::enums::ExecutionStatus::Timeout,
        "abandoned" => attune_common::models::enums::ExecutionStatus::Abandoned,
        _ => {
            return Err(ApiError::BadRequest(format!(
                "Invalid execution status: {}",
                status_str
            )))
        }
    };

    // Get executions by status
    let executions = ExecutionRepository::find_by_status(&state.db, status).await?;

    // Calculate pagination
    let total = executions.len() as u64;
    let start = ((pagination.page - 1) * pagination.limit()) as usize;
    let end = (start + pagination.limit() as usize).min(executions.len());

    // Get paginated slice
    let paginated_executions: Vec<ExecutionSummary> = executions[start..end]
        .iter()
        .map(|e| ExecutionSummary::from(e.clone()))
        .collect();

    let response = PaginatedResponse::new(paginated_executions, &pagination, total);

    Ok((StatusCode::OK, Json(response)))
}

/// List executions by enforcement ID
#[utoipa::path(
    get,
    path = "/api/v1/executions/enforcement/{enforcement_id}",
    tag = "executions",
    params(
        ("enforcement_id" = i64, Path, description = "Enforcement ID"),
        PaginationParams
    ),
    responses(
        (status = 200, description = "List of executions for enforcement", body = PaginatedResponse<ExecutionSummary>),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_executions_by_enforcement(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(enforcement_id): Path<i64>,
    Query(pagination): Query<PaginationParams>,
) -> ApiResult<impl IntoResponse> {
    // Get executions by enforcement
    let executions = ExecutionRepository::find_by_enforcement(&state.db, enforcement_id).await?;

    // Calculate pagination
    let total = executions.len() as u64;
    let start = ((pagination.page - 1) * pagination.limit()) as usize;
    let end = (start + pagination.limit() as usize).min(executions.len());

    // Get paginated slice
    let paginated_executions: Vec<ExecutionSummary> = executions[start..end]
        .iter()
        .map(|e| ExecutionSummary::from(e.clone()))
        .collect();

    let response = PaginatedResponse::new(paginated_executions, &pagination, total);

    Ok((StatusCode::OK, Json(response)))
}

/// Get execution statistics
#[utoipa::path(
    get,
    path = "/api/v1/executions/stats",
    tag = "executions",
    responses(
        (status = 200, description = "Execution statistics", body = inline(Object)),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_execution_stats(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
) -> ApiResult<impl IntoResponse> {
    // Get all executions (limited by repository to 1000)
    let executions = ExecutionRepository::list(&state.db).await?;

    // Calculate statistics
    let total = executions.len();
    let completed = executions
        .iter()
        .filter(|e| e.status == attune_common::models::enums::ExecutionStatus::Completed)
        .count();
    let failed = executions
        .iter()
        .filter(|e| e.status == attune_common::models::enums::ExecutionStatus::Failed)
        .count();
    let running = executions
        .iter()
        .filter(|e| e.status == attune_common::models::enums::ExecutionStatus::Running)
        .count();
    let pending = executions
        .iter()
        .filter(|e| {
            matches!(
                e.status,
                attune_common::models::enums::ExecutionStatus::Requested
                    | attune_common::models::enums::ExecutionStatus::Scheduling
                    | attune_common::models::enums::ExecutionStatus::Scheduled
            )
        })
        .count();

    let stats = serde_json::json!({
        "total": total,
        "completed": completed,
        "failed": failed,
        "running": running,
        "pending": pending,
        "cancelled": executions.iter().filter(|e| e.status == attune_common::models::enums::ExecutionStatus::Cancelled).count(),
        "timeout": executions.iter().filter(|e| e.status == attune_common::models::enums::ExecutionStatus::Timeout).count(),
        "abandoned": executions.iter().filter(|e| e.status == attune_common::models::enums::ExecutionStatus::Abandoned).count(),
    });

    let response = ApiResponse::new(stats);

    Ok((StatusCode::OK, Json(response)))
}

/// Create execution routes
/// Stream execution updates via Server-Sent Events
///
/// This endpoint streams real-time updates for execution status changes.
/// Optionally filter by execution_id to watch a specific execution.
///
/// Note: Authentication is done via `token` query parameter since EventSource
/// doesn't support custom headers.
#[utoipa::path(
    get,
    path = "/api/v1/executions/stream",
    tag = "executions",
    params(
        ("execution_id" = Option<i64>, Query, description = "Optional execution ID to filter updates"),
        ("token" = String, Query, description = "JWT access token for authentication")
    ),
    responses(
        (status = 200, description = "SSE stream of execution updates", content_type = "text/event-stream"),
        (status = 401, description = "Unauthorized - invalid or missing token"),
    )
)]
pub async fn stream_execution_updates(
    State(state): State<Arc<AppState>>,
    Query(params): Query<StreamExecutionParams>,
) -> Result<Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>>, ApiError> {
    // Validate token from query parameter
    use crate::auth::jwt::validate_token;

    let token = params.token.as_ref().ok_or(ApiError::Unauthorized(
        "Missing authentication token".to_string(),
    ))?;

    validate_token(token, &state.jwt_config)
        .map_err(|_| ApiError::Unauthorized("Invalid authentication token".to_string()))?;
    let rx = state.broadcast_tx.subscribe();
    let stream = BroadcastStream::new(rx);

    let filtered_stream = stream.filter_map(move |msg| {
        async move {
            match msg {
                Ok(notification) => {
                    // Parse the notification as JSON
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&notification) {
                        // Check if it's an execution update
                        if let Some(entity_type) = value.get("entity_type").and_then(|v| v.as_str())
                        {
                            if entity_type == "execution" {
                                // If filtering by execution_id, check if it matches
                                if let Some(filter_id) = params.execution_id {
                                    if let Some(entity_id) =
                                        value.get("entity_id").and_then(|v| v.as_i64())
                                    {
                                        if entity_id != filter_id {
                                            return None; // Skip this event
                                        }
                                    }
                                }

                                // Send the notification as an SSE event
                                return Some(Ok(Event::default().data(notification)));
                            }
                        }
                    }
                    None
                }
                Err(_) => None, // Skip broadcast errors
            }
        }
    });

    Ok(Sse::new(filtered_stream).keep_alive(KeepAlive::default()))
}

#[derive(serde::Deserialize)]
pub struct StreamExecutionParams {
    pub execution_id: Option<i64>,
    pub token: Option<String>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/executions", get(list_executions))
        .route("/executions/execute", axum::routing::post(create_execution))
        .route("/executions/stats", get(get_execution_stats))
        .route("/executions/stream", get(stream_execution_updates))
        .route("/executions/{id}", get(get_execution))
        .route(
            "/executions/status/{status}",
            get(list_executions_by_status),
        )
        .route(
            "/enforcements/{enforcement_id}/executions",
            get(list_executions_by_enforcement),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execution_routes_structure() {
        // Just verify the router can be constructed
        let _router = routes();
    }
}
