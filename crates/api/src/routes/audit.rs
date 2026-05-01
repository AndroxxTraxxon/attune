//! Audit log query API routes.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use std::sync::Arc;
use uuid::Uuid;

use attune_common::{
    audit::{AuditEventFilters, AuditRepository},
    rbac::{Action, AuthorizationContext, Resource},
};

use crate::auth::RequireAuth;
use crate::{
    authz::{AuthorizationCheck, AuthorizationService},
    dto::{
        audit::{AuditEventQueryParams, AuditEventResponse, AuditEventSummary},
        common::{PaginatedResponse, PaginationParams},
        ApiResponse,
    },
    middleware::{ApiError, ApiResult},
    state::AppState,
};

fn audit_check(action: Action) -> AuthorizationCheck {
    AuthorizationCheck {
        resource: Resource::AuditLog,
        action,
        context: AuthorizationContext::new(0),
    }
}

/// List audit events with optional filters.
#[utoipa::path(
    get,
    path = "/api/v1/audit-events",
    tag = "audit",
    params(AuditEventQueryParams),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Audit event list", body = PaginatedResponse<AuditEventSummary>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn list_audit_events(
    user: RequireAuth,
    State(state): State<Arc<AppState>>,
    Query(query): Query<AuditEventQueryParams>,
) -> ApiResult<impl IntoResponse> {
    let authz = AuthorizationService::new(state.db.clone());
    authz.authorize(&user.0, audit_check(Action::Read)).await?;

    let filters = AuditEventFilters {
        category: query.category,
        event_type: query.event_type.clone(),
        actor_login_contains: query.actor_login.clone(),
        outcome: query.outcome,
        actor_identity: query.actor_identity,
        resource_type: query.resource_type.clone(),
        resource_id: query.resource_id,
        resource_ref: query.resource_ref.clone(),
        request_id: query.request_id,
        http_status: query.http_status,
        http_method: query.http_method.clone(),
        http_path_contains: query.http_path.clone(),
        created_after: query.created_after,
        created_before: query.created_before,
        limit: Some(query.limit() as i64),
        offset: Some(query.offset() as i64),
        include_total: query.include_total.unwrap_or(false),
    };

    let result = AuditRepository::search_with_meta(&state.db, &filters).await?;

    let rows: Vec<AuditEventSummary> = result
        .rows
        .into_iter()
        .map(AuditEventSummary::from)
        .collect();

    let pagination_params = PaginationParams {
        page: query.page,
        page_size: query.per_page,
    };

    let response = if let Some(total) = result.total {
        PaginatedResponse::new(rows, &pagination_params, total)
    } else {
        PaginatedResponse::without_totals(rows, &pagination_params, result.has_next)
    };

    Ok((StatusCode::OK, Json(response)))
}

/// Get a single audit event by ID.
#[utoipa::path(
    get,
    path = "/api/v1/audit-events/{id}",
    tag = "audit",
    params(("id" = i64, Path, description = "Audit event ID")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Audit event details", body = ApiResponse<AuditEventResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Audit event not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_audit_event(
    user: RequireAuth,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    let authz = AuthorizationService::new(state.db.clone());
    authz.authorize(&user.0, audit_check(Action::Read)).await?;

    let event = AuditRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Audit event {} not found", id)))?;

    let response = ApiResponse::new(AuditEventResponse::from(event));
    Ok((StatusCode::OK, Json(response)))
}

/// Get all audit events sharing a request_id (full request lineage).
#[utoipa::path(
    get,
    path = "/api/v1/audit-events/by-request/{request_id}",
    tag = "audit",
    params(("request_id" = String, Path, description = "Correlation UUID")),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Audit events for the request", body = ApiResponse<Vec<AuditEventResponse>>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_audit_events_by_request(
    user: RequireAuth,
    State(state): State<Arc<AppState>>,
    Path(request_id): Path<Uuid>,
) -> ApiResult<impl IntoResponse> {
    let authz = AuthorizationService::new(state.db.clone());
    authz.authorize(&user.0, audit_check(Action::Read)).await?;

    let events = AuditRepository::find_by_request_id(&state.db, request_id).await?;
    let rows: Vec<AuditEventResponse> = events.into_iter().map(AuditEventResponse::from).collect();

    Ok((StatusCode::OK, Json(ApiResponse::new(rows))))
}

/// Register audit-event routes.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/audit-events", get(list_audit_events))
        .route("/audit-events/{id}", get(get_audit_event))
        .route(
            "/audit-events/by-request/{request_id}",
            get(get_audit_events_by_request),
        )
}
