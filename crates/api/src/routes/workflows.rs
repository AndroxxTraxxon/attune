//! Workflow management API routes

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
    pack::PackRepository,
    workflow::{
        CreateWorkflowDefinitionInput, UpdateWorkflowDefinitionInput, WorkflowDefinitionRepository,
    },
    Create, Delete, FindByRef, List, Update,
};

use crate::{
    auth::middleware::RequireAuth,
    dto::{
        common::{PaginatedResponse, PaginationParams},
        workflow::{
            CreateWorkflowRequest, UpdateWorkflowRequest, WorkflowResponse, WorkflowSearchParams,
            WorkflowSummary,
        },
        ApiResponse, SuccessResponse,
    },
    middleware::{ApiError, ApiResult},
    state::AppState,
};

/// List all workflows with pagination and filtering
#[utoipa::path(
    get,
    path = "/api/v1/workflows",
    tag = "workflows",
    params(PaginationParams, WorkflowSearchParams),
    responses(
        (status = 200, description = "List of workflows", body = PaginatedResponse<WorkflowSummary>),
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_workflows(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Query(pagination): Query<PaginationParams>,
    Query(search_params): Query<WorkflowSearchParams>,
) -> ApiResult<impl IntoResponse> {
    // Validate search params
    search_params.validate()?;

    // Get workflows based on filters
    let mut workflows = if let Some(tags_str) = &search_params.tags {
        // Filter by tags
        let tags: Vec<&str> = tags_str.split(',').map(|s| s.trim()).collect();
        let mut results = Vec::new();
        for tag in tags {
            let mut tag_results = WorkflowDefinitionRepository::find_by_tag(&state.db, tag).await?;
            results.append(&mut tag_results);
        }
        // Remove duplicates by ID
        results.sort_by_key(|w| w.id);
        results.dedup_by_key(|w| w.id);
        results
    } else if search_params.enabled == Some(true) {
        // Filter by enabled status (only return enabled workflows)
        WorkflowDefinitionRepository::find_enabled(&state.db).await?
    } else {
        // Get all workflows
        WorkflowDefinitionRepository::list(&state.db).await?
    };

    // Apply enabled filter if specified and not already filtered by it
    if let Some(enabled) = search_params.enabled {
        if search_params.tags.is_some() {
            // If we filtered by tags, also apply enabled filter
            workflows.retain(|w| w.enabled == enabled);
        }
    }

    // Apply search filter if provided
    if let Some(search_term) = &search_params.search {
        let search_lower = search_term.to_lowercase();
        workflows.retain(|w| {
            w.label.to_lowercase().contains(&search_lower)
                || w.description
                    .as_ref()
                    .map(|d| d.to_lowercase().contains(&search_lower))
                    .unwrap_or(false)
        });
    }

    // Apply pack_ref filter if provided
    if let Some(pack_ref) = &search_params.pack_ref {
        workflows.retain(|w| w.pack_ref == *pack_ref);
    }

    // Calculate pagination
    let total = workflows.len() as u64;
    let start = ((pagination.page - 1) * pagination.limit()) as usize;
    let end = (start + pagination.limit() as usize).min(workflows.len());

    // Get paginated slice
    let paginated_workflows: Vec<WorkflowSummary> = workflows[start..end]
        .iter()
        .map(|w| WorkflowSummary::from(w.clone()))
        .collect();

    let response = PaginatedResponse::new(paginated_workflows, &pagination, total);

    Ok((StatusCode::OK, Json(response)))
}

/// List workflows by pack reference
#[utoipa::path(
    get,
    path = "/api/v1/packs/{pack_ref}/workflows",
    tag = "workflows",
    params(
        ("pack_ref" = String, Path, description = "Pack reference identifier"),
        PaginationParams
    ),
    responses(
        (status = 200, description = "List of workflows for pack", body = PaginatedResponse<WorkflowSummary>),
        (status = 404, description = "Pack not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_workflows_by_pack(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(pack_ref): Path<String>,
    Query(pagination): Query<PaginationParams>,
) -> ApiResult<impl IntoResponse> {
    // Verify pack exists
    let pack = PackRepository::find_by_ref(&state.db, &pack_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Pack '{}' not found", pack_ref)))?;

    // Get workflows for this pack
    let workflows = WorkflowDefinitionRepository::find_by_pack(&state.db, pack.id).await?;

    // Calculate pagination
    let total = workflows.len() as u64;
    let start = ((pagination.page - 1) * pagination.limit()) as usize;
    let end = (start + pagination.limit() as usize).min(workflows.len());

    // Get paginated slice
    let paginated_workflows: Vec<WorkflowSummary> = workflows[start..end]
        .iter()
        .map(|w| WorkflowSummary::from(w.clone()))
        .collect();

    let response = PaginatedResponse::new(paginated_workflows, &pagination, total);

    Ok((StatusCode::OK, Json(response)))
}

/// Get a single workflow by reference
#[utoipa::path(
    get,
    path = "/api/v1/workflows/{ref}",
    tag = "workflows",
    params(
        ("ref" = String, Path, description = "Workflow reference identifier")
    ),
    responses(
        (status = 200, description = "Workflow details", body = inline(ApiResponse<WorkflowResponse>)),
        (status = 404, description = "Workflow not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_workflow(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(workflow_ref): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let workflow = WorkflowDefinitionRepository::find_by_ref(&state.db, &workflow_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Workflow '{}' not found", workflow_ref)))?;

    let response = ApiResponse::new(WorkflowResponse::from(workflow));

    Ok((StatusCode::OK, Json(response)))
}

/// Create a new workflow
#[utoipa::path(
    post,
    path = "/api/v1/workflows",
    tag = "workflows",
    request_body = CreateWorkflowRequest,
    responses(
        (status = 201, description = "Workflow created successfully", body = inline(ApiResponse<WorkflowResponse>)),
        (status = 400, description = "Validation error"),
        (status = 404, description = "Pack not found"),
        (status = 409, description = "Workflow with same ref already exists")
    ),
    security(("bearer_auth" = []))
)]
pub async fn create_workflow(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Json(request): Json<CreateWorkflowRequest>,
) -> ApiResult<impl IntoResponse> {
    // Validate request
    request.validate()?;

    // Check if workflow with same ref already exists
    if let Some(_) = WorkflowDefinitionRepository::find_by_ref(&state.db, &request.r#ref).await? {
        return Err(ApiError::Conflict(format!(
            "Workflow with ref '{}' already exists",
            request.r#ref
        )));
    }

    // Verify pack exists and get its ID
    let pack = PackRepository::find_by_ref(&state.db, &request.pack_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Pack '{}' not found", request.pack_ref)))?;

    // Create workflow input
    let workflow_input = CreateWorkflowDefinitionInput {
        r#ref: request.r#ref,
        pack: pack.id,
        pack_ref: pack.r#ref.clone(),
        label: request.label,
        description: request.description,
        version: request.version,
        param_schema: request.param_schema,
        out_schema: request.out_schema,
        definition: request.definition,
        tags: request.tags.unwrap_or_default(),
        enabled: request.enabled.unwrap_or(true),
    };

    let workflow = WorkflowDefinitionRepository::create(&state.db, workflow_input).await?;

    let response = ApiResponse::with_message(
        WorkflowResponse::from(workflow),
        "Workflow created successfully",
    );

    Ok((StatusCode::CREATED, Json(response)))
}

/// Update an existing workflow
#[utoipa::path(
    put,
    path = "/api/v1/workflows/{ref}",
    tag = "workflows",
    params(
        ("ref" = String, Path, description = "Workflow reference identifier")
    ),
    request_body = UpdateWorkflowRequest,
    responses(
        (status = 200, description = "Workflow updated successfully", body = inline(ApiResponse<WorkflowResponse>)),
        (status = 400, description = "Validation error"),
        (status = 404, description = "Workflow not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn update_workflow(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(workflow_ref): Path<String>,
    Json(request): Json<UpdateWorkflowRequest>,
) -> ApiResult<impl IntoResponse> {
    // Validate request
    request.validate()?;

    // Check if workflow exists
    let existing_workflow = WorkflowDefinitionRepository::find_by_ref(&state.db, &workflow_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Workflow '{}' not found", workflow_ref)))?;

    // Create update input
    let update_input = UpdateWorkflowDefinitionInput {
        label: request.label,
        description: request.description,
        version: request.version,
        param_schema: request.param_schema,
        out_schema: request.out_schema,
        definition: request.definition,
        tags: request.tags,
        enabled: request.enabled,
    };

    let workflow =
        WorkflowDefinitionRepository::update(&state.db, existing_workflow.id, update_input).await?;

    let response = ApiResponse::with_message(
        WorkflowResponse::from(workflow),
        "Workflow updated successfully",
    );

    Ok((StatusCode::OK, Json(response)))
}

/// Delete a workflow
#[utoipa::path(
    delete,
    path = "/api/v1/workflows/{ref}",
    tag = "workflows",
    params(
        ("ref" = String, Path, description = "Workflow reference identifier")
    ),
    responses(
        (status = 200, description = "Workflow deleted successfully", body = SuccessResponse),
        (status = 404, description = "Workflow not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn delete_workflow(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(workflow_ref): Path<String>,
) -> ApiResult<impl IntoResponse> {
    // Check if workflow exists
    let workflow = WorkflowDefinitionRepository::find_by_ref(&state.db, &workflow_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Workflow '{}' not found", workflow_ref)))?;

    // Delete the workflow
    let deleted = WorkflowDefinitionRepository::delete(&state.db, workflow.id).await?;

    if !deleted {
        return Err(ApiError::NotFound(format!(
            "Workflow '{}' not found",
            workflow_ref
        )));
    }

    let response =
        SuccessResponse::new(format!("Workflow '{}' deleted successfully", workflow_ref));

    Ok((StatusCode::OK, Json(response)))
}

/// Create workflow routes
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/workflows", get(list_workflows).post(create_workflow))
        .route(
            "/workflows/{ref}",
            get(get_workflow)
                .put(update_workflow)
                .delete(delete_workflow),
        )
        .route("/packs/{pack_ref}/workflows", get(list_workflows_by_pack))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workflow_routes_structure() {
        // Just verify the router can be constructed
        let _router = routes();
    }
}
