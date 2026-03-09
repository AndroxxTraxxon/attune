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
use attune_common::mq::{
    ExecutionCancelRequestedPayload, ExecutionRequestedPayload, MessageEnvelope, MessageType,
    Publisher,
};
use attune_common::repositories::{
    action::ActionRepository,
    execution::{
        CreateExecutionInput, ExecutionRepository, ExecutionSearchFilters, UpdateExecutionInput,
    },
    workflow::{WorkflowDefinitionRepository, WorkflowExecutionRepository},
    Create, FindById, FindByRef, Update,
};
use attune_common::workflow::{CancellationPolicy, WorkflowDefinition};
use sqlx::Row;

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
        env_vars: request
            .env_vars
            .as_ref()
            .and_then(|e| serde_json::from_value(e.clone()).ok()),
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

    if let Some(publisher) = state.get_publisher().await {
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
    // All filtering, pagination, and the enforcement JOIN happen in a single
    // SQL query — no in-memory filtering or post-fetch lookups.
    let filters = ExecutionSearchFilters {
        status: query.status,
        action_ref: query.action_ref.clone(),
        pack_name: query.pack_name.clone(),
        rule_ref: query.rule_ref.clone(),
        trigger_ref: query.trigger_ref.clone(),
        executor: query.executor,
        result_contains: query.result_contains.clone(),
        enforcement: query.enforcement,
        parent: query.parent,
        top_level_only: query.top_level_only == Some(true),
        limit: query.limit(),
        offset: query.offset(),
    };

    let result = ExecutionRepository::search(&state.db, &filters).await?;

    let paginated_executions: Vec<ExecutionSummary> = result
        .rows
        .into_iter()
        .map(ExecutionSummary::from)
        .collect();

    let pagination_params = PaginationParams {
        page: query.page,
        page_size: query.per_page,
    };

    let response = PaginatedResponse::new(paginated_executions, &pagination_params, result.total);

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

    // Use the search method for SQL-side filtering + pagination.
    let filters = ExecutionSearchFilters {
        status: Some(status),
        limit: pagination.limit(),
        offset: pagination.offset(),
        ..Default::default()
    };

    let result = ExecutionRepository::search(&state.db, &filters).await?;

    let paginated_executions: Vec<ExecutionSummary> = result
        .rows
        .into_iter()
        .map(ExecutionSummary::from)
        .collect();

    let response = PaginatedResponse::new(paginated_executions, &pagination, result.total);

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
    // Use the search method for SQL-side filtering + pagination.
    let filters = ExecutionSearchFilters {
        enforcement: Some(enforcement_id),
        limit: pagination.limit(),
        offset: pagination.offset(),
        ..Default::default()
    };

    let result = ExecutionRepository::search(&state.db, &filters).await?;

    let paginated_executions: Vec<ExecutionSummary> = result
        .rows
        .into_iter()
        .map(ExecutionSummary::from)
        .collect();

    let response = PaginatedResponse::new(paginated_executions, &pagination, result.total);

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
    // Use a single SQL query with COUNT + GROUP BY instead of fetching all rows.
    let rows = sqlx::query(
        "SELECT status::text AS status, COUNT(*) AS cnt FROM execution GROUP BY status",
    )
    .fetch_all(&state.db)
    .await?;

    let mut completed: i64 = 0;
    let mut failed: i64 = 0;
    let mut running: i64 = 0;
    let mut pending: i64 = 0;
    let mut cancelled: i64 = 0;
    let mut timeout: i64 = 0;
    let mut abandoned: i64 = 0;
    let mut total: i64 = 0;

    for row in &rows {
        let status: &str = row.get("status");
        let cnt: i64 = row.get("cnt");
        total += cnt;
        match status {
            "completed" => completed = cnt,
            "failed" => failed = cnt,
            "running" => running = cnt,
            "requested" | "scheduling" | "scheduled" => pending += cnt,
            "cancelled" | "canceling" => cancelled += cnt,
            "timeout" => timeout = cnt,
            "abandoned" => abandoned = cnt,
            _ => {}
        }
    }

    let stats = serde_json::json!({
        "total": total,
        "completed": completed,
        "failed": failed,
        "running": running,
        "pending": pending,
        "cancelled": cancelled,
        "timeout": timeout,
        "abandoned": abandoned,
    });

    let response = ApiResponse::new(stats);

    Ok((StatusCode::OK, Json(response)))
}

/// Cancel a running execution
///
/// This endpoint requests cancellation of an execution. The execution must be in a
/// cancellable state (requested, scheduling, scheduled, running, or canceling).
/// For running executions, the worker will send SIGINT to the process, then SIGTERM
/// after a 10-second grace period if it hasn't stopped.
///
/// **Workflow cascading**: When a workflow (parent) execution is cancelled, all of
/// its incomplete child task executions are also cancelled. Children that haven't
/// reached a worker yet are set to Cancelled immediately; children that are running
/// receive a cancel MQ message so their worker can gracefully stop the process.
/// The workflow_execution record is also marked as Cancelled to prevent the
/// scheduler from dispatching any further tasks.
#[utoipa::path(
    post,
    path = "/api/v1/executions/{id}/cancel",
    tag = "executions",
    params(
        ("id" = i64, Path, description = "Execution ID")
    ),
    responses(
        (status = 200, description = "Cancellation requested", body = inline(ApiResponse<ExecutionResponse>)),
        (status = 404, description = "Execution not found"),
        (status = 409, description = "Execution is not in a cancellable state"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn cancel_execution(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    // Load the execution
    let execution = ExecutionRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Execution with ID {} not found", id)))?;

    // Check if the execution is in a cancellable state
    let cancellable = matches!(
        execution.status,
        ExecutionStatus::Requested
            | ExecutionStatus::Scheduling
            | ExecutionStatus::Scheduled
            | ExecutionStatus::Running
            | ExecutionStatus::Canceling
    );

    if !cancellable {
        return Err(ApiError::Conflict(format!(
            "Execution {} is in status '{}' and cannot be cancelled",
            id,
            format!("{:?}", execution.status).to_lowercase()
        )));
    }

    // If already canceling, just return the current state
    if execution.status == ExecutionStatus::Canceling {
        let response = ApiResponse::new(ExecutionResponse::from(execution));
        return Ok((StatusCode::OK, Json(response)));
    }

    let publisher = state.get_publisher().await;

    // For executions that haven't reached a worker yet, cancel immediately
    if matches!(
        execution.status,
        ExecutionStatus::Requested | ExecutionStatus::Scheduling | ExecutionStatus::Scheduled
    ) {
        let update = UpdateExecutionInput {
            status: Some(ExecutionStatus::Cancelled),
            result: Some(
                serde_json::json!({"error": "Cancelled by user before execution started"}),
            ),
            ..Default::default()
        };
        let updated = ExecutionRepository::update(&state.db, id, update).await?;

        // Cascade to workflow children if this is a workflow execution
        cancel_workflow_children(&state.db, publisher.as_deref(), id).await;

        let response = ApiResponse::new(ExecutionResponse::from(updated));
        return Ok((StatusCode::OK, Json(response)));
    }

    // For running executions, set status to Canceling and send cancel message to the worker
    let update = UpdateExecutionInput {
        status: Some(ExecutionStatus::Canceling),
        ..Default::default()
    };
    let updated = ExecutionRepository::update(&state.db, id, update).await?;

    // Send cancel request to the worker via MQ
    if let Some(worker_id) = execution.executor {
        send_cancel_to_worker(publisher.as_deref(), id, worker_id).await;
    } else {
        tracing::warn!(
            "Execution {} has no executor/worker assigned; marked as canceling but no MQ message sent",
            id
        );
    }

    // Cascade to workflow children if this is a workflow execution
    cancel_workflow_children(&state.db, publisher.as_deref(), id).await;

    let response = ApiResponse::new(ExecutionResponse::from(updated));
    Ok((StatusCode::OK, Json(response)))
}

/// Send a cancel MQ message to a specific worker for a specific execution.
async fn send_cancel_to_worker(publisher: Option<&Publisher>, execution_id: i64, worker_id: i64) {
    let payload = ExecutionCancelRequestedPayload {
        execution_id,
        worker_id,
    };

    let envelope = MessageEnvelope::new(MessageType::ExecutionCancelRequested, payload)
        .with_source("api-service")
        .with_correlation_id(uuid::Uuid::new_v4());

    if let Some(publisher) = publisher {
        let routing_key = format!("execution.cancel.worker.{}", worker_id);
        let exchange = "attune.executions";
        if let Err(e) = publisher
            .publish_envelope_with_routing(&envelope, exchange, &routing_key)
            .await
        {
            tracing::error!(
                "Failed to publish cancel request for execution {}: {}",
                execution_id,
                e
            );
        }
    } else {
        tracing::warn!(
            "No MQ publisher available to send cancel request for execution {}",
            execution_id
        );
    }
}

/// Resolve the [`CancellationPolicy`] for a workflow parent execution.
///
/// Looks up the `workflow_execution` → `workflow_definition` chain and
/// deserialises the stored definition to extract the policy.  Returns
/// [`CancellationPolicy::AllowFinish`] (the default) when any lookup
/// step fails so that the safest behaviour is used as a fallback.
async fn resolve_cancellation_policy(
    db: &sqlx::PgPool,
    parent_execution_id: i64,
) -> CancellationPolicy {
    let wf_exec =
        match WorkflowExecutionRepository::find_by_execution(db, parent_execution_id).await {
            Ok(Some(wf)) => wf,
            _ => return CancellationPolicy::default(),
        };

    let wf_def = match WorkflowDefinitionRepository::find_by_id(db, wf_exec.workflow_def).await {
        Ok(Some(def)) => def,
        _ => return CancellationPolicy::default(),
    };

    // Deserialise the stored JSON definition to extract the policy field.
    match serde_json::from_value::<WorkflowDefinition>(wf_def.definition) {
        Ok(def) => def.cancellation_policy,
        Err(e) => {
            tracing::warn!(
                "Failed to deserialise workflow definition for workflow_def {}: {}. \
                 Falling back to AllowFinish cancellation policy.",
                wf_exec.workflow_def,
                e
            );
            CancellationPolicy::default()
        }
    }
}

/// Cancel all incomplete child executions of a workflow parent execution.
///
/// This handles the workflow cascade: when a workflow execution is cancelled,
/// its child task executions must also be cancelled to prevent further work.
/// Additionally, the `workflow_execution` record is marked Cancelled so the
/// scheduler's `advance_workflow` will short-circuit and not dispatch new tasks.
///
/// Behaviour depends on the workflow's [`CancellationPolicy`]:
///
/// - **`AllowFinish`** (default): Children in pre-running states (Requested,
///   Scheduling, Scheduled) are set to Cancelled immediately.  Running children
///   are left alone and will complete naturally; `advance_workflow` sees the
///   cancelled `workflow_execution` and will not dispatch further tasks.
///
/// - **`CancelRunning`**: Pre-running children are cancelled as above.
///   Running children also receive a cancel MQ message so their worker can
///   gracefully stop the process (SIGINT → SIGTERM → SIGKILL).
async fn cancel_workflow_children(
    db: &sqlx::PgPool,
    publisher: Option<&Publisher>,
    parent_execution_id: i64,
) {
    // Determine the cancellation policy from the workflow definition.
    let policy = resolve_cancellation_policy(db, parent_execution_id).await;

    cancel_workflow_children_with_policy(db, publisher, parent_execution_id, policy).await;
}

/// Inner implementation that carries the resolved [`CancellationPolicy`]
/// through recursive calls so that nested child workflows inherit the
/// top-level policy.
async fn cancel_workflow_children_with_policy(
    db: &sqlx::PgPool,
    publisher: Option<&Publisher>,
    parent_execution_id: i64,
    policy: CancellationPolicy,
) {
    // Find all child executions that are still incomplete
    let children: Vec<attune_common::models::Execution> = match sqlx::query_as::<
        _,
        attune_common::models::Execution,
    >(&format!(
        "SELECT {} FROM execution WHERE parent = $1 AND status NOT IN ('completed', 'failed', 'timeout', 'cancelled', 'abandoned')",
        attune_common::repositories::execution::SELECT_COLUMNS
    ))
    .bind(parent_execution_id)
    .fetch_all(db)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!(
                "Failed to fetch child executions for parent {}: {}",
                parent_execution_id,
                e
            );
            return;
        }
    };

    if children.is_empty() {
        return;
    }

    tracing::info!(
        "Cascading cancellation from execution {} to {} child execution(s) (policy: {:?})",
        parent_execution_id,
        children.len(),
        policy,
    );

    for child in &children {
        let child_id = child.id;

        if matches!(
            child.status,
            ExecutionStatus::Requested | ExecutionStatus::Scheduling | ExecutionStatus::Scheduled
        ) {
            // Pre-running: cancel immediately in DB (both policies)
            let update = UpdateExecutionInput {
                status: Some(ExecutionStatus::Cancelled),
                result: Some(serde_json::json!({
                    "error": "Cancelled: parent workflow execution was cancelled"
                })),
                ..Default::default()
            };
            if let Err(e) = ExecutionRepository::update(db, child_id, update).await {
                tracing::error!("Failed to cancel child execution {}: {}", child_id, e);
            } else {
                tracing::info!("Cancelled pre-running child execution {}", child_id);
            }
        } else if matches!(
            child.status,
            ExecutionStatus::Running | ExecutionStatus::Canceling
        ) {
            match policy {
                CancellationPolicy::CancelRunning => {
                    // Running: set to Canceling and send MQ message to the worker
                    if child.status != ExecutionStatus::Canceling {
                        let update = UpdateExecutionInput {
                            status: Some(ExecutionStatus::Canceling),
                            ..Default::default()
                        };
                        if let Err(e) = ExecutionRepository::update(db, child_id, update).await {
                            tracing::error!(
                                "Failed to set child execution {} to canceling: {}",
                                child_id,
                                e
                            );
                        }
                    }

                    if let Some(worker_id) = child.executor {
                        send_cancel_to_worker(publisher, child_id, worker_id).await;
                    }
                }
                CancellationPolicy::AllowFinish => {
                    // Running tasks are allowed to complete naturally.
                    // advance_workflow will see the cancelled workflow_execution
                    // and will not dispatch any further tasks.
                    tracing::info!(
                        "AllowFinish policy: leaving running child execution {} alone",
                        child_id
                    );
                }
            }
        }

        // Recursively cancel grandchildren (nested workflows)
        // Use Box::pin to allow the recursive async call
        Box::pin(cancel_workflow_children_with_policy(
            db, publisher, child_id, policy,
        ))
        .await;
    }

    // Also mark any associated workflow_execution record as Cancelled so that
    // advance_workflow short-circuits and does not dispatch new tasks.
    // A workflow_execution is linked to the parent execution via its `execution` column.
    if let Ok(Some(wf_exec)) =
        WorkflowExecutionRepository::find_by_execution(db, parent_execution_id).await
    {
        if !matches!(
            wf_exec.status,
            ExecutionStatus::Completed | ExecutionStatus::Failed | ExecutionStatus::Cancelled
        ) {
            let wf_update = attune_common::repositories::workflow::UpdateWorkflowExecutionInput {
                status: Some(ExecutionStatus::Cancelled),
                error_message: Some(
                    "Cancelled: parent workflow execution was cancelled".to_string(),
                ),
                current_tasks: Some(vec![]),
                completed_tasks: None,
                failed_tasks: None,
                skipped_tasks: None,
                variables: None,
                paused: None,
                pause_reason: None,
            };
            if let Err(e) = WorkflowExecutionRepository::update(db, wf_exec.id, wf_update).await {
                tracing::error!("Failed to cancel workflow_execution {}: {}", wf_exec.id, e);
            } else {
                tracing::info!(
                    "Cancelled workflow_execution {} for parent execution {}",
                    wf_exec.id,
                    parent_execution_id
                );
            }
        }
    }

    // If no children are still running (all were pre-running or were
    // cancelled), finalize the parent execution as Cancelled immediately.
    // Without this, the parent would stay stuck in "Canceling" because no
    // task completion would trigger advance_workflow to finalize it.
    let still_running: Vec<attune_common::models::Execution> = match sqlx::query_as::<
        _,
        attune_common::models::Execution,
    >(&format!(
        "SELECT {} FROM execution WHERE parent = $1 AND status IN ('running', 'canceling', 'scheduling', 'scheduled', 'requested')",
        attune_common::repositories::execution::SELECT_COLUMNS
    ))
    .bind(parent_execution_id)
    .fetch_all(db)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            tracing::error!(
                "Failed to check remaining children for parent {}: {}",
                parent_execution_id,
                e
            );
            return;
        }
    };

    if still_running.is_empty() {
        // No children left in flight — finalize the parent execution now.
        let update = UpdateExecutionInput {
            status: Some(ExecutionStatus::Cancelled),
            result: Some(serde_json::json!({
                "error": "Workflow cancelled",
                "succeeded": false,
            })),
            ..Default::default()
        };
        if let Err(e) = ExecutionRepository::update(db, parent_execution_id, update).await {
            tracing::error!(
                "Failed to finalize parent execution {} as Cancelled: {}",
                parent_execution_id,
                e
            );
        } else {
            tracing::info!(
                "Finalized parent execution {} as Cancelled (no running children remain)",
                parent_execution_id
            );
        }
    }
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
            "/executions/{id}/cancel",
            axum::routing::post(cancel_execution),
        )
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
