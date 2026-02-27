//! Entity history API routes
//!
//! Provides read-only access to the TimescaleDB entity history hypertables.
//! History records are written by PostgreSQL triggers — these endpoints only query them.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use std::sync::Arc;

use attune_common::models::entity_history::HistoryEntityType;
use attune_common::repositories::entity_history::EntityHistoryRepository;

use crate::{
    auth::middleware::RequireAuth,
    dto::{
        common::{PaginatedResponse, PaginationMeta, PaginationParams},
        history::{HistoryQueryParams, HistoryRecordResponse},
    },
    middleware::{ApiError, ApiResult},
    state::AppState,
};

/// List history records for a given entity type.
///
/// Supported entity types: `execution`, `worker`.
/// Returns a paginated list of change records ordered by time descending.
#[utoipa::path(
    get,
    path = "/api/v1/history/{entity_type}",
    tag = "history",
    params(
        ("entity_type" = String, Path, description = "Entity type: execution or worker"),
        HistoryQueryParams,
    ),
    responses(
        (status = 200, description = "Paginated list of history records", body = PaginatedResponse<HistoryRecordResponse>),
        (status = 400, description = "Invalid entity type"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_entity_history(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(entity_type_str): Path<String>,
    Query(query): Query<HistoryQueryParams>,
) -> ApiResult<impl IntoResponse> {
    let entity_type = parse_entity_type(&entity_type_str)?;

    let repo_params = query.to_repo_params();

    let (records, total) = tokio::try_join!(
        EntityHistoryRepository::query(&state.db, entity_type, &repo_params),
        EntityHistoryRepository::count(&state.db, entity_type, &repo_params),
    )?;

    let data: Vec<HistoryRecordResponse> = records.into_iter().map(Into::into).collect();

    let pagination_params = PaginationParams {
        page: query.page,
        page_size: query.page_size,
    };

    let response = PaginatedResponse {
        data,
        pagination: PaginationMeta::new(
            pagination_params.page,
            pagination_params.page_size,
            total as u64,
        ),
    };

    Ok((StatusCode::OK, Json(response)))
}

/// Get history for a specific execution by ID.
///
/// Returns all change records for the given execution, ordered by time descending.
#[utoipa::path(
    get,
    path = "/api/v1/executions/{id}/history",
    tag = "history",
    params(
        ("id" = i64, Path, description = "Execution ID"),
        HistoryQueryParams,
    ),
    responses(
        (status = 200, description = "History records for the execution", body = PaginatedResponse<HistoryRecordResponse>),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_execution_history(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(id): Path<i64>,
    Query(query): Query<HistoryQueryParams>,
) -> ApiResult<impl IntoResponse> {
    get_entity_history_by_id(&state, HistoryEntityType::Execution, id, query).await
}

/// Get history for a specific worker by ID.
///
/// Returns all change records for the given worker, ordered by time descending.
#[utoipa::path(
    get,
    path = "/api/v1/workers/{id}/history",
    tag = "history",
    params(
        ("id" = i64, Path, description = "Worker ID"),
        HistoryQueryParams,
    ),
    responses(
        (status = 200, description = "History records for the worker", body = PaginatedResponse<HistoryRecordResponse>),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_worker_history(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(id): Path<i64>,
    Query(query): Query<HistoryQueryParams>,
) -> ApiResult<impl IntoResponse> {
    get_entity_history_by_id(&state, HistoryEntityType::Worker, id, query).await
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Parse and validate the entity type path parameter.
fn parse_entity_type(s: &str) -> Result<HistoryEntityType, ApiError> {
    s.parse::<HistoryEntityType>().map_err(ApiError::BadRequest)
}

/// Shared implementation for `GET /<entities>/:id/history` endpoints.
async fn get_entity_history_by_id(
    state: &AppState,
    entity_type: HistoryEntityType,
    entity_id: i64,
    query: HistoryQueryParams,
) -> ApiResult<impl IntoResponse> {
    // Override entity_id from the path — ignore any entity_id in query params
    let mut repo_params = query.to_repo_params();
    repo_params.entity_id = Some(entity_id);

    let (records, total) = tokio::try_join!(
        EntityHistoryRepository::query(&state.db, entity_type, &repo_params),
        EntityHistoryRepository::count(&state.db, entity_type, &repo_params),
    )?;

    let data: Vec<HistoryRecordResponse> = records.into_iter().map(Into::into).collect();

    let pagination_params = PaginationParams {
        page: query.page,
        page_size: query.page_size,
    };

    let response = PaginatedResponse {
        data,
        pagination: PaginationMeta::new(
            pagination_params.page,
            pagination_params.page_size,
            total as u64,
        ),
    };

    Ok((StatusCode::OK, Json(response)))
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

/// Build the history routes.
///
/// Mounts:
/// - `GET /history/:entity_type`          — generic history query
/// - `GET /executions/:id/history`        — execution-specific history
/// - `GET /workers/:id/history`           — worker-specific history (note: currently no /workers base route exists)
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        // Generic history endpoint
        .route("/history/{entity_type}", get(list_entity_history))
        // Entity-specific convenience endpoints
        .route("/executions/{id}/history", get(get_execution_history))
        .route("/workers/{id}/history", get(get_worker_history))
}
