//! Inquiry management API routes

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;
use validator::Validate;

use attune_common::{
    mq::{InquiryRespondedPayload, MessageEnvelope, MessageType},
    repositories::{
        execution::ExecutionRepository,
        inquiry::{
            CreateInquiryInput, InquiryRepository, InquirySearchFilters, UpdateInquiryInput,
        },
        Create, Delete, FindById, Update,
    },
};

use crate::auth::RequireAuth;
use crate::{
    dto::{
        common::{PaginatedResponse, PaginationParams},
        inquiry::{
            CreateInquiryRequest, InquiryQueryParams, InquiryRespondRequest, InquiryResponse,
            InquirySummary, UpdateInquiryRequest,
        },
        ApiResponse, SuccessResponse,
    },
    middleware::{ApiError, ApiResult},
    state::AppState,
};

/// List all inquiries with pagination and optional filters
#[utoipa::path(
    get,
    path = "/api/v1/inquiries",
    tag = "inquiries",
    params(InquiryQueryParams),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "List of inquiries", body = PaginatedResponse<InquirySummary>),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn list_inquiries(
    _user: RequireAuth,
    State(state): State<Arc<AppState>>,
    Query(query): Query<InquiryQueryParams>,
) -> ApiResult<impl IntoResponse> {
    // All filtering and pagination happen in a single SQL query.
    // Filters are combinable (AND), not mutually exclusive.
    let limit = query.limit.unwrap_or(50).min(500) as u32;
    let offset = query.offset.unwrap_or(0) as u32;

    let filters = InquirySearchFilters {
        status: query.status,
        execution: query.execution,
        assigned_to: query.assigned_to,
        limit,
        offset,
    };

    let result = InquiryRepository::search(&state.db, &filters).await?;

    let paginated_inquiries: Vec<InquirySummary> =
        result.rows.into_iter().map(InquirySummary::from).collect();

    let pagination_params = PaginationParams {
        page: (offset / limit.max(1)) + 1,
        page_size: limit,
    };

    let response = PaginatedResponse::new(paginated_inquiries, &pagination_params, result.total);

    Ok((StatusCode::OK, Json(response)))
}

/// Get a single inquiry by ID
#[utoipa::path(
    get,
    path = "/api/v1/inquiries/{id}",
    tag = "inquiries",
    params(
        ("id" = i64, Path, description = "Inquiry ID")
    ),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Inquiry details", body = ApiResponse<InquiryResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Inquiry not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_inquiry(
    _user: RequireAuth,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    let inquiry = InquiryRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Inquiry with ID {} not found", id)))?;

    let response = ApiResponse::new(InquiryResponse::from(inquiry));

    Ok((StatusCode::OK, Json(response)))
}

/// List inquiries by status
#[utoipa::path(
    get,
    path = "/api/v1/inquiries/status/{status}",
    tag = "inquiries",
    params(
        ("status" = String, Path, description = "Inquiry status (pending, responded, timeout, canceled)"),
        PaginationParams
    ),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "List of inquiries with specified status", body = PaginatedResponse<InquirySummary>),
        (status = 400, description = "Invalid status"),
        (status = 401, description = "Unauthorized"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn list_inquiries_by_status(
    _user: RequireAuth,
    State(state): State<Arc<AppState>>,
    Path(status_str): Path<String>,
    Query(pagination): Query<PaginationParams>,
) -> ApiResult<impl IntoResponse> {
    // Parse status from string
    let status = match status_str.to_lowercase().as_str() {
        "pending" => attune_common::models::enums::InquiryStatus::Pending,
        "responded" => attune_common::models::enums::InquiryStatus::Responded,
        "timeout" => attune_common::models::enums::InquiryStatus::Timeout,
        "canceled" => attune_common::models::enums::InquiryStatus::Cancelled,
        _ => {
            return Err(ApiError::BadRequest(format!(
            "Invalid inquiry status: '{}'. Valid values are: pending, responded, timeout, canceled",
            status_str
        )))
        }
    };

    // Use the search method for SQL-side filtering + pagination.
    let filters = InquirySearchFilters {
        status: Some(status),
        execution: None,
        assigned_to: None,
        limit: pagination.limit(),
        offset: pagination.offset(),
    };

    let result = InquiryRepository::search(&state.db, &filters).await?;

    let paginated_inquiries: Vec<InquirySummary> =
        result.rows.into_iter().map(InquirySummary::from).collect();

    let response = PaginatedResponse::new(paginated_inquiries, &pagination, result.total);

    Ok((StatusCode::OK, Json(response)))
}

/// List inquiries for a specific execution
#[utoipa::path(
    get,
    path = "/api/v1/executions/{execution_id}/inquiries",
    tag = "inquiries",
    params(
        ("execution_id" = i64, Path, description = "Execution ID"),
        PaginationParams
    ),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "List of inquiries for execution", body = PaginatedResponse<InquirySummary>),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Execution not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn list_inquiries_by_execution(
    _user: RequireAuth,
    State(state): State<Arc<AppState>>,
    Path(execution_id): Path<i64>,
    Query(pagination): Query<PaginationParams>,
) -> ApiResult<impl IntoResponse> {
    // Verify execution exists
    let _execution = ExecutionRepository::find_by_id(&state.db, execution_id)
        .await?
        .ok_or_else(|| {
            ApiError::NotFound(format!("Execution with ID {} not found", execution_id))
        })?;

    // Use the search method for SQL-side filtering + pagination.
    let filters = InquirySearchFilters {
        status: None,
        execution: Some(execution_id),
        assigned_to: None,
        limit: pagination.limit(),
        offset: pagination.offset(),
    };

    let result = InquiryRepository::search(&state.db, &filters).await?;

    let paginated_inquiries: Vec<InquirySummary> =
        result.rows.into_iter().map(InquirySummary::from).collect();

    let response = PaginatedResponse::new(paginated_inquiries, &pagination, result.total);

    Ok((StatusCode::OK, Json(response)))
}

/// Create a new inquiry
#[utoipa::path(
    post,
    path = "/api/v1/inquiries",
    tag = "inquiries",
    request_body = CreateInquiryRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 201, description = "Inquiry created successfully", body = ApiResponse<InquiryResponse>),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Execution not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn create_inquiry(
    _user: RequireAuth,
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateInquiryRequest>,
) -> ApiResult<impl IntoResponse> {
    // Validate request
    request.validate()?;

    // Verify execution exists
    let _execution = ExecutionRepository::find_by_id(&state.db, request.execution)
        .await?
        .ok_or_else(|| {
            ApiError::NotFound(format!("Execution with ID {} not found", request.execution))
        })?;

    // Create inquiry input
    let inquiry_input = CreateInquiryInput {
        execution: request.execution,
        prompt: request.prompt,
        response_schema: request.response_schema,
        assigned_to: request.assigned_to,
        status: attune_common::models::enums::InquiryStatus::Pending,
        response: None,
        timeout_at: request.timeout_at,
    };

    let inquiry = InquiryRepository::create(&state.db, inquiry_input).await?;

    let response = ApiResponse::with_message(
        InquiryResponse::from(inquiry),
        "Inquiry created successfully",
    );

    Ok((StatusCode::CREATED, Json(response)))
}

/// Update an existing inquiry
#[utoipa::path(
    put,
    path = "/api/v1/inquiries/{id}",
    tag = "inquiries",
    params(
        ("id" = i64, Path, description = "Inquiry ID")
    ),
    request_body = UpdateInquiryRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Inquiry updated successfully", body = ApiResponse<InquiryResponse>),
        (status = 400, description = "Invalid request"),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Inquiry not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn update_inquiry(
    _user: RequireAuth,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(request): Json<UpdateInquiryRequest>,
) -> ApiResult<impl IntoResponse> {
    // Validate request
    request.validate()?;

    // Verify inquiry exists
    let _existing = InquiryRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Inquiry with ID {} not found", id)))?;

    // Create update input
    let update_input = UpdateInquiryInput {
        status: request.status,
        response: request.response,
        responded_at: None, // Let the database handle this if needed
        assigned_to: request.assigned_to,
    };

    let updated_inquiry = InquiryRepository::update(&state.db, id, update_input).await?;

    let response = ApiResponse::with_message(
        InquiryResponse::from(updated_inquiry),
        "Inquiry updated successfully",
    );

    Ok((StatusCode::OK, Json(response)))
}

/// Respond to an inquiry (user-facing endpoint)
#[utoipa::path(
    post,
    path = "/api/v1/inquiries/{id}/respond",
    tag = "inquiries",
    params(
        ("id" = i64, Path, description = "Inquiry ID")
    ),
    request_body = InquiryRespondRequest,
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Response submitted successfully", body = ApiResponse<InquiryResponse>),
        (status = 400, description = "Invalid request or inquiry cannot be responded to"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Not authorized to respond to this inquiry"),
        (status = 404, description = "Inquiry not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn respond_to_inquiry(
    user: RequireAuth,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(request): Json<InquiryRespondRequest>,
) -> ApiResult<impl IntoResponse> {
    // Validate request
    request.validate()?;

    // Verify inquiry exists and is in pending status
    let inquiry = InquiryRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Inquiry with ID {} not found", id)))?;

    // Check if inquiry is still pending
    if inquiry.status != attune_common::models::enums::InquiryStatus::Pending {
        return Err(ApiError::BadRequest(format!(
            "Cannot respond to inquiry with status '{:?}'. Only pending inquiries can be responded to.",
            inquiry.status
        )));
    }

    // Check if inquiry is assigned to this user (optional enforcement)
    if let Some(assigned_to) = inquiry.assigned_to {
        let user_id = user
            .0
            .identity_id()
            .map_err(|_| ApiError::InternalServerError("Invalid user identity".to_string()))?;
        if assigned_to != user_id {
            return Err(ApiError::Forbidden(
                "You are not authorized to respond to this inquiry".to_string(),
            ));
        }
    }

    // Check if inquiry has timed out
    if let Some(timeout_at) = inquiry.timeout_at {
        if timeout_at < chrono::Utc::now() {
            // Update inquiry to timeout status
            let timeout_input = UpdateInquiryInput {
                status: Some(attune_common::models::enums::InquiryStatus::Timeout),
                response: None,
                responded_at: None,
                assigned_to: None,
            };
            let _ = InquiryRepository::update(&state.db, id, timeout_input).await?;

            return Err(ApiError::BadRequest(
                "Inquiry has timed out and can no longer be responded to".to_string(),
            ));
        }
    }

    // TODO: Validate response against response_schema if present
    // For now, just accept the response as-is

    // Create update input with response
    let update_input = UpdateInquiryInput {
        status: Some(attune_common::models::enums::InquiryStatus::Responded),
        response: Some(request.response.clone()),
        responded_at: Some(chrono::Utc::now()),
        assigned_to: None,
    };

    let updated_inquiry = InquiryRepository::update(&state.db, id, update_input).await?;

    // Publish InquiryResponded message if publisher is available
    if let Some(publisher) = state.get_publisher().await {
        let user_id = user
            .0
            .identity_id()
            .map_err(|_| ApiError::InternalServerError("Invalid user identity".to_string()))?;

        let payload = InquiryRespondedPayload {
            inquiry_id: id,
            execution_id: inquiry.execution,
            response: request.response.clone(),
            responded_by: Some(user_id),
            responded_at: chrono::Utc::now(),
        };

        let envelope =
            MessageEnvelope::new(MessageType::InquiryResponded, payload).with_source("api");

        if let Err(e) = publisher.publish_envelope(&envelope).await {
            tracing::error!("Failed to publish InquiryResponded message: {}", e);
            // Don't fail the request - inquiry is already saved
        } else {
            tracing::info!("Published InquiryResponded message for inquiry {}", id);
        }
    } else {
        tracing::warn!("No publisher available to publish InquiryResponded message");
    }

    let response = ApiResponse::with_message(
        InquiryResponse::from(updated_inquiry),
        "Response submitted successfully",
    );

    Ok((StatusCode::OK, Json(response)))
}

/// Delete an inquiry
#[utoipa::path(
    delete,
    path = "/api/v1/inquiries/{id}",
    tag = "inquiries",
    params(
        ("id" = i64, Path, description = "Inquiry ID")
    ),
    security(("bearer_auth" = [])),
    responses(
        (status = 200, description = "Inquiry deleted successfully", body = SuccessResponse),
        (status = 401, description = "Unauthorized"),
        (status = 404, description = "Inquiry not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn delete_inquiry(
    _user: RequireAuth,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    // Verify inquiry exists
    let _inquiry = InquiryRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Inquiry with ID {} not found", id)))?;

    // Delete the inquiry
    let deleted = InquiryRepository::delete(&state.db, id).await?;

    if !deleted {
        return Err(ApiError::NotFound(format!(
            "Inquiry with ID {} not found",
            id
        )));
    }

    let response = SuccessResponse::new("Inquiry deleted successfully");

    Ok((StatusCode::OK, Json(response)))
}

/// Register inquiry routes
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/inquiries", get(list_inquiries).post(create_inquiry))
        .route(
            "/inquiries/{id}",
            get(get_inquiry).put(update_inquiry).delete(delete_inquiry),
        )
        .route("/inquiries/status/{status}", get(list_inquiries_by_status))
        .route(
            "/executions/{execution_id}/inquiries",
            get(list_inquiries_by_execution),
        )
        .route("/inquiries/{id}/respond", post(respond_to_inquiry))
}
