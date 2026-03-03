//! Artifact management API routes
//!
//! Provides endpoints for:
//! - CRUD operations on artifacts (metadata + data)
//! - File upload (binary) and download for file-type artifacts
//! - JSON content versioning for structured artifacts
//! - Progress append for progress-type artifacts (streaming updates)
//! - Listing artifacts by execution
//! - Version history and retrieval

use axum::{
    body::Body,
    extract::{Multipart, Path, Query, State},
    http::{header, StatusCode},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;

use attune_common::models::enums::ArtifactType;
use attune_common::repositories::{
    artifact::{
        ArtifactRepository, ArtifactSearchFilters, ArtifactVersionRepository, CreateArtifactInput,
        CreateArtifactVersionInput, UpdateArtifactInput,
    },
    Create, Delete, FindById, FindByRef, Update,
};

use crate::{
    auth::middleware::RequireAuth,
    dto::{
        artifact::{
            AppendProgressRequest, ArtifactQueryParams, ArtifactResponse, ArtifactSummary,
            ArtifactVersionResponse, ArtifactVersionSummary, CreateArtifactRequest,
            CreateVersionJsonRequest, SetDataRequest, UpdateArtifactRequest,
        },
        common::{PaginatedResponse, PaginationParams},
        ApiResponse, SuccessResponse,
    },
    middleware::{ApiError, ApiResult},
    state::AppState,
};

// ============================================================================
// Artifact CRUD
// ============================================================================

/// List artifacts with pagination and optional filters
#[utoipa::path(
    get,
    path = "/api/v1/artifacts",
    tag = "artifacts",
    params(ArtifactQueryParams),
    responses(
        (status = 200, description = "List of artifacts", body = PaginatedResponse<ArtifactSummary>),
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_artifacts(
    RequireAuth(_user): RequireAuth,
    State(state): State<Arc<AppState>>,
    Query(query): Query<ArtifactQueryParams>,
) -> ApiResult<impl IntoResponse> {
    let filters = ArtifactSearchFilters {
        scope: query.scope,
        owner: query.owner.clone(),
        r#type: query.r#type,
        execution: query.execution,
        name_contains: query.name.clone(),
        limit: query.limit(),
        offset: query.offset(),
    };

    let result = ArtifactRepository::search(&state.db, &filters).await?;

    let items: Vec<ArtifactSummary> = result.rows.into_iter().map(ArtifactSummary::from).collect();

    let pagination = PaginationParams {
        page: query.page,
        page_size: query.per_page,
    };

    let response = PaginatedResponse::new(items, &pagination, result.total as u64);
    Ok((StatusCode::OK, Json(response)))
}

/// Get a single artifact by ID
#[utoipa::path(
    get,
    path = "/api/v1/artifacts/{id}",
    tag = "artifacts",
    params(("id" = i64, Path, description = "Artifact ID")),
    responses(
        (status = 200, description = "Artifact details", body = inline(ApiResponse<ArtifactResponse>)),
        (status = 404, description = "Artifact not found"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_artifact(
    RequireAuth(_user): RequireAuth,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    let artifact = ArtifactRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Artifact with ID {} not found", id)))?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::new(ArtifactResponse::from(artifact))),
    ))
}

/// Get a single artifact by ref
#[utoipa::path(
    get,
    path = "/api/v1/artifacts/ref/{ref}",
    tag = "artifacts",
    params(("ref" = String, Path, description = "Artifact reference")),
    responses(
        (status = 200, description = "Artifact details", body = inline(ApiResponse<ArtifactResponse>)),
        (status = 404, description = "Artifact not found"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_artifact_by_ref(
    RequireAuth(_user): RequireAuth,
    State(state): State<Arc<AppState>>,
    Path(artifact_ref): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let artifact = ArtifactRepository::find_by_ref(&state.db, &artifact_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Artifact '{}' not found", artifact_ref)))?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::new(ArtifactResponse::from(artifact))),
    ))
}

/// Create a new artifact
#[utoipa::path(
    post,
    path = "/api/v1/artifacts",
    tag = "artifacts",
    request_body = CreateArtifactRequest,
    responses(
        (status = 201, description = "Artifact created", body = inline(ApiResponse<ArtifactResponse>)),
        (status = 400, description = "Validation error"),
        (status = 409, description = "Artifact with same ref already exists"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn create_artifact(
    RequireAuth(_user): RequireAuth,
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateArtifactRequest>,
) -> ApiResult<impl IntoResponse> {
    // Validate ref is not empty
    if request.r#ref.trim().is_empty() {
        return Err(ApiError::BadRequest(
            "Artifact ref must not be empty".to_string(),
        ));
    }

    // Check for duplicate ref
    if ArtifactRepository::find_by_ref(&state.db, &request.r#ref)
        .await?
        .is_some()
    {
        return Err(ApiError::Conflict(format!(
            "Artifact with ref '{}' already exists",
            request.r#ref
        )));
    }

    let input = CreateArtifactInput {
        r#ref: request.r#ref,
        scope: request.scope,
        owner: request.owner,
        r#type: request.r#type,
        retention_policy: request.retention_policy,
        retention_limit: request.retention_limit,
        name: request.name,
        description: request.description,
        content_type: request.content_type,
        execution: request.execution,
        data: request.data,
    };

    let artifact = ArtifactRepository::create(&state.db, input).await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::with_message(
            ArtifactResponse::from(artifact),
            "Artifact created successfully",
        )),
    ))
}

/// Update an existing artifact
#[utoipa::path(
    put,
    path = "/api/v1/artifacts/{id}",
    tag = "artifacts",
    params(("id" = i64, Path, description = "Artifact ID")),
    request_body = UpdateArtifactRequest,
    responses(
        (status = 200, description = "Artifact updated", body = inline(ApiResponse<ArtifactResponse>)),
        (status = 404, description = "Artifact not found"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn update_artifact(
    RequireAuth(_user): RequireAuth,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(request): Json<UpdateArtifactRequest>,
) -> ApiResult<impl IntoResponse> {
    // Verify artifact exists
    ArtifactRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Artifact with ID {} not found", id)))?;

    let input = UpdateArtifactInput {
        r#ref: None, // Ref is immutable after creation
        scope: request.scope,
        owner: request.owner,
        r#type: request.r#type,
        retention_policy: request.retention_policy,
        retention_limit: request.retention_limit,
        name: request.name,
        description: request.description,
        content_type: request.content_type,
        size_bytes: None, // Managed by version creation trigger
        data: request.data,
    };

    let updated = ArtifactRepository::update(&state.db, id, input).await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::with_message(
            ArtifactResponse::from(updated),
            "Artifact updated successfully",
        )),
    ))
}

/// Delete an artifact (cascades to all versions)
#[utoipa::path(
    delete,
    path = "/api/v1/artifacts/{id}",
    tag = "artifacts",
    params(("id" = i64, Path, description = "Artifact ID")),
    responses(
        (status = 200, description = "Artifact deleted", body = SuccessResponse),
        (status = 404, description = "Artifact not found"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn delete_artifact(
    RequireAuth(_user): RequireAuth,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    let deleted = ArtifactRepository::delete(&state.db, id).await?;
    if !deleted {
        return Err(ApiError::NotFound(format!(
            "Artifact with ID {} not found",
            id
        )));
    }

    Ok((
        StatusCode::OK,
        Json(SuccessResponse::new("Artifact deleted successfully")),
    ))
}

// ============================================================================
// Artifacts by Execution
// ============================================================================

/// List all artifacts for a given execution
#[utoipa::path(
    get,
    path = "/api/v1/executions/{execution_id}/artifacts",
    tag = "artifacts",
    params(("execution_id" = i64, Path, description = "Execution ID")),
    responses(
        (status = 200, description = "List of artifacts for execution", body = inline(ApiResponse<Vec<ArtifactSummary>>)),
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_artifacts_by_execution(
    RequireAuth(_user): RequireAuth,
    State(state): State<Arc<AppState>>,
    Path(execution_id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    let artifacts = ArtifactRepository::find_by_execution(&state.db, execution_id).await?;
    let items: Vec<ArtifactSummary> = artifacts.into_iter().map(ArtifactSummary::from).collect();

    Ok((StatusCode::OK, Json(ApiResponse::new(items))))
}

// ============================================================================
// Progress Artifacts
// ============================================================================

/// Append an entry to a progress-type artifact's data array.
///
/// The entry is atomically appended to `artifact.data` (initialized as `[]` if null).
/// This is the primary mechanism for actions to stream progress updates.
#[utoipa::path(
    post,
    path = "/api/v1/artifacts/{id}/progress",
    tag = "artifacts",
    params(("id" = i64, Path, description = "Artifact ID (must be progress type)")),
    request_body = AppendProgressRequest,
    responses(
        (status = 200, description = "Entry appended", body = inline(ApiResponse<ArtifactResponse>)),
        (status = 400, description = "Artifact is not a progress type"),
        (status = 404, description = "Artifact not found"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn append_progress(
    RequireAuth(_user): RequireAuth,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(request): Json<AppendProgressRequest>,
) -> ApiResult<impl IntoResponse> {
    let artifact = ArtifactRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Artifact with ID {} not found", id)))?;

    if artifact.r#type != ArtifactType::Progress {
        return Err(ApiError::BadRequest(format!(
            "Artifact '{}' is type {:?}, not progress. Use version endpoints for file artifacts.",
            artifact.r#ref, artifact.r#type
        )));
    }

    let updated = ArtifactRepository::append_progress(&state.db, id, &request.entry).await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::with_message(
            ArtifactResponse::from(updated),
            "Progress entry appended",
        )),
    ))
}

/// Set the full data payload on an artifact (replaces existing data).
///
/// Useful for resetting progress, updating metadata, or setting structured content.
#[utoipa::path(
    put,
    path = "/api/v1/artifacts/{id}/data",
    tag = "artifacts",
    params(("id" = i64, Path, description = "Artifact ID")),
    request_body = SetDataRequest,
    responses(
        (status = 200, description = "Data set", body = inline(ApiResponse<ArtifactResponse>)),
        (status = 404, description = "Artifact not found"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn set_artifact_data(
    RequireAuth(_user): RequireAuth,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(request): Json<SetDataRequest>,
) -> ApiResult<impl IntoResponse> {
    // Verify exists
    ArtifactRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Artifact with ID {} not found", id)))?;

    let updated = ArtifactRepository::set_data(&state.db, id, &request.data).await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::with_message(
            ArtifactResponse::from(updated),
            "Artifact data updated",
        )),
    ))
}

// ============================================================================
// Version Management
// ============================================================================

/// List all versions for an artifact (without binary content)
#[utoipa::path(
    get,
    path = "/api/v1/artifacts/{id}/versions",
    tag = "artifacts",
    params(("id" = i64, Path, description = "Artifact ID")),
    responses(
        (status = 200, description = "List of versions", body = inline(ApiResponse<Vec<ArtifactVersionSummary>>)),
        (status = 404, description = "Artifact not found"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_versions(
    RequireAuth(_user): RequireAuth,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    // Verify artifact exists
    ArtifactRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Artifact with ID {} not found", id)))?;

    let versions = ArtifactVersionRepository::list_by_artifact(&state.db, id).await?;
    let items: Vec<ArtifactVersionSummary> = versions
        .into_iter()
        .map(ArtifactVersionSummary::from)
        .collect();

    Ok((StatusCode::OK, Json(ApiResponse::new(items))))
}

/// Get a specific version's metadata and JSON content (no binary)
#[utoipa::path(
    get,
    path = "/api/v1/artifacts/{id}/versions/{version}",
    tag = "artifacts",
    params(
        ("id" = i64, Path, description = "Artifact ID"),
        ("version" = i32, Path, description = "Version number"),
    ),
    responses(
        (status = 200, description = "Version details", body = inline(ApiResponse<ArtifactVersionResponse>)),
        (status = 404, description = "Artifact or version not found"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_version(
    RequireAuth(_user): RequireAuth,
    State(state): State<Arc<AppState>>,
    Path((id, version)): Path<(i64, i32)>,
) -> ApiResult<impl IntoResponse> {
    // Verify artifact exists
    ArtifactRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Artifact with ID {} not found", id)))?;

    let ver = ArtifactVersionRepository::find_by_version(&state.db, id, version)
        .await?
        .ok_or_else(|| {
            ApiError::NotFound(format!("Version {} not found for artifact {}", version, id))
        })?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::new(ArtifactVersionResponse::from(ver))),
    ))
}

/// Get the latest version's metadata and JSON content
#[utoipa::path(
    get,
    path = "/api/v1/artifacts/{id}/versions/latest",
    tag = "artifacts",
    params(("id" = i64, Path, description = "Artifact ID")),
    responses(
        (status = 200, description = "Latest version", body = inline(ApiResponse<ArtifactVersionResponse>)),
        (status = 404, description = "Artifact not found or no versions"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_latest_version(
    RequireAuth(_user): RequireAuth,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    ArtifactRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Artifact with ID {} not found", id)))?;

    let ver = ArtifactVersionRepository::find_latest(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("No versions found for artifact {}", id)))?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::new(ArtifactVersionResponse::from(ver))),
    ))
}

/// Create a new version with JSON content
#[utoipa::path(
    post,
    path = "/api/v1/artifacts/{id}/versions",
    tag = "artifacts",
    params(("id" = i64, Path, description = "Artifact ID")),
    request_body = CreateVersionJsonRequest,
    responses(
        (status = 201, description = "Version created", body = inline(ApiResponse<ArtifactVersionResponse>)),
        (status = 404, description = "Artifact not found"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn create_version_json(
    RequireAuth(_user): RequireAuth,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(request): Json<CreateVersionJsonRequest>,
) -> ApiResult<impl IntoResponse> {
    ArtifactRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Artifact with ID {} not found", id)))?;

    let input = CreateArtifactVersionInput {
        artifact: id,
        content_type: Some(
            request
                .content_type
                .unwrap_or_else(|| "application/json".to_string()),
        ),
        content: None,
        content_json: Some(request.content),
        meta: request.meta,
        created_by: request.created_by,
    };

    let version = ArtifactVersionRepository::create(&state.db, input).await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::with_message(
            ArtifactVersionResponse::from(version),
            "Version created successfully",
        )),
    ))
}

/// Upload a binary file as a new version (multipart/form-data)
///
/// The file is sent as a multipart form field named `file`. Optional fields:
/// - `content_type`: MIME type override (auto-detected from filename if omitted)
/// - `meta`: JSON metadata string
/// - `created_by`: Creator identifier
#[utoipa::path(
    post,
    path = "/api/v1/artifacts/{id}/versions/upload",
    tag = "artifacts",
    params(("id" = i64, Path, description = "Artifact ID")),
    request_body(content = String, content_type = "multipart/form-data"),
    responses(
        (status = 201, description = "File version created", body = inline(ApiResponse<ArtifactVersionResponse>)),
        (status = 400, description = "Missing file field"),
        (status = 404, description = "Artifact not found"),
        (status = 413, description = "File too large"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn upload_version(
    RequireAuth(_user): RequireAuth,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    mut multipart: Multipart,
) -> ApiResult<impl IntoResponse> {
    ArtifactRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Artifact with ID {} not found", id)))?;

    let mut file_data: Option<Vec<u8>> = None;
    let mut content_type: Option<String> = None;
    let mut meta: Option<serde_json::Value> = None;
    let mut created_by: Option<String> = None;
    let mut file_content_type: Option<String> = None;

    // 50 MB limit
    const MAX_FILE_SIZE: usize = 50 * 1024 * 1024;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::BadRequest(format!("Multipart error: {}", e)))?
    {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "file" => {
                // Capture content type from the multipart field itself
                file_content_type = field.content_type().map(|s| s.to_string());

                let bytes = field
                    .bytes()
                    .await
                    .map_err(|e| ApiError::BadRequest(format!("Failed to read file: {}", e)))?;

                if bytes.len() > MAX_FILE_SIZE {
                    return Err(ApiError::BadRequest(format!(
                        "File exceeds maximum size of {} bytes",
                        MAX_FILE_SIZE
                    )));
                }

                file_data = Some(bytes.to_vec());
            }
            "content_type" => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| ApiError::BadRequest(format!("Failed to read field: {}", e)))?;
                if !text.is_empty() {
                    content_type = Some(text);
                }
            }
            "meta" => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| ApiError::BadRequest(format!("Failed to read field: {}", e)))?;
                if !text.is_empty() {
                    meta =
                        Some(serde_json::from_str(&text).map_err(|e| {
                            ApiError::BadRequest(format!("Invalid meta JSON: {}", e))
                        })?);
                }
            }
            "created_by" => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| ApiError::BadRequest(format!("Failed to read field: {}", e)))?;
                if !text.is_empty() {
                    created_by = Some(text);
                }
            }
            _ => {
                // Skip unknown fields
            }
        }
    }

    let file_bytes = file_data.ok_or_else(|| {
        ApiError::BadRequest("Missing required 'file' field in multipart upload".to_string())
    })?;

    // Resolve content type: explicit > multipart header > fallback
    let resolved_ct = content_type
        .or(file_content_type)
        .unwrap_or_else(|| "application/octet-stream".to_string());

    let input = CreateArtifactVersionInput {
        artifact: id,
        content_type: Some(resolved_ct),
        content: Some(file_bytes),
        content_json: None,
        meta,
        created_by,
    };

    let version = ArtifactVersionRepository::create(&state.db, input).await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::with_message(
            ArtifactVersionResponse::from(version),
            "File version uploaded successfully",
        )),
    ))
}

/// Download the binary content of a specific version
#[utoipa::path(
    get,
    path = "/api/v1/artifacts/{id}/versions/{version}/download",
    tag = "artifacts",
    params(
        ("id" = i64, Path, description = "Artifact ID"),
        ("version" = i32, Path, description = "Version number"),
    ),
    responses(
        (status = 200, description = "Binary file content", content_type = "application/octet-stream"),
        (status = 404, description = "Artifact, version, or content not found"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn download_version(
    RequireAuth(_user): RequireAuth,
    State(state): State<Arc<AppState>>,
    Path((id, version)): Path<(i64, i32)>,
) -> ApiResult<impl IntoResponse> {
    let artifact = ArtifactRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Artifact with ID {} not found", id)))?;

    let ver = ArtifactVersionRepository::find_by_version_with_content(&state.db, id, version)
        .await?
        .ok_or_else(|| {
            ApiError::NotFound(format!("Version {} not found for artifact {}", version, id))
        })?;

    // For binary content
    if let Some(bytes) = ver.content {
        let ct = ver
            .content_type
            .unwrap_or_else(|| "application/octet-stream".to_string());

        let filename = format!(
            "{}_v{}.{}",
            artifact.r#ref.replace('.', "_"),
            version,
            extension_from_content_type(&ct)
        );

        return Ok((
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, ct),
                (
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{}\"", filename),
                ),
            ],
            Body::from(bytes),
        )
            .into_response());
    }

    // For JSON content, serialize and return
    if let Some(json) = ver.content_json {
        let bytes = serde_json::to_vec_pretty(&json).map_err(|e| {
            ApiError::InternalServerError(format!("Failed to serialize JSON: {}", e))
        })?;

        let ct = ver
            .content_type
            .unwrap_or_else(|| "application/json".to_string());

        let filename = format!("{}_v{}.json", artifact.r#ref.replace('.', "_"), version,);

        return Ok((
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, ct),
                (
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{}\"", filename),
                ),
            ],
            Body::from(bytes),
        )
            .into_response());
    }

    Err(ApiError::NotFound(format!(
        "Version {} of artifact {} has no downloadable content",
        version, id
    )))
}

/// Download the latest version's content
#[utoipa::path(
    get,
    path = "/api/v1/artifacts/{id}/download",
    tag = "artifacts",
    params(("id" = i64, Path, description = "Artifact ID")),
    responses(
        (status = 200, description = "Binary file content of latest version", content_type = "application/octet-stream"),
        (status = 404, description = "Artifact not found or no versions"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn download_latest(
    RequireAuth(_user): RequireAuth,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
) -> ApiResult<impl IntoResponse> {
    let artifact = ArtifactRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Artifact with ID {} not found", id)))?;

    let ver = ArtifactVersionRepository::find_latest_with_content(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("No versions found for artifact {}", id)))?;

    let version = ver.version;

    // For binary content
    if let Some(bytes) = ver.content {
        let ct = ver
            .content_type
            .unwrap_or_else(|| "application/octet-stream".to_string());

        let filename = format!(
            "{}_v{}.{}",
            artifact.r#ref.replace('.', "_"),
            version,
            extension_from_content_type(&ct)
        );

        return Ok((
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, ct),
                (
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{}\"", filename),
                ),
            ],
            Body::from(bytes),
        )
            .into_response());
    }

    // For JSON content
    if let Some(json) = ver.content_json {
        let bytes = serde_json::to_vec_pretty(&json).map_err(|e| {
            ApiError::InternalServerError(format!("Failed to serialize JSON: {}", e))
        })?;

        let ct = ver
            .content_type
            .unwrap_or_else(|| "application/json".to_string());

        let filename = format!("{}_v{}.json", artifact.r#ref.replace('.', "_"), version,);

        return Ok((
            StatusCode::OK,
            [
                (header::CONTENT_TYPE, ct),
                (
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{}\"", filename),
                ),
            ],
            Body::from(bytes),
        )
            .into_response());
    }

    Err(ApiError::NotFound(format!(
        "Latest version of artifact {} has no downloadable content",
        id
    )))
}

/// Delete a specific version by version number
#[utoipa::path(
    delete,
    path = "/api/v1/artifacts/{id}/versions/{version}",
    tag = "artifacts",
    params(
        ("id" = i64, Path, description = "Artifact ID"),
        ("version" = i32, Path, description = "Version number"),
    ),
    responses(
        (status = 200, description = "Version deleted", body = SuccessResponse),
        (status = 404, description = "Artifact or version not found"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn delete_version(
    RequireAuth(_user): RequireAuth,
    State(state): State<Arc<AppState>>,
    Path((id, version)): Path<(i64, i32)>,
) -> ApiResult<impl IntoResponse> {
    // Verify artifact exists
    ArtifactRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Artifact with ID {} not found", id)))?;

    // Find the version by artifact + version number
    let ver = ArtifactVersionRepository::find_by_version(&state.db, id, version)
        .await?
        .ok_or_else(|| {
            ApiError::NotFound(format!("Version {} not found for artifact {}", version, id))
        })?;

    ArtifactVersionRepository::delete(&state.db, ver.id).await?;

    Ok((
        StatusCode::OK,
        Json(SuccessResponse::new("Version deleted successfully")),
    ))
}

// ============================================================================
// Helpers
// ============================================================================

/// Derive a simple file extension from a MIME content type
fn extension_from_content_type(ct: &str) -> &str {
    match ct {
        "text/plain" => "txt",
        "text/html" => "html",
        "text/css" => "css",
        "text/csv" => "csv",
        "text/xml" => "xml",
        "application/json" => "json",
        "application/xml" => "xml",
        "application/pdf" => "pdf",
        "application/zip" => "zip",
        "application/gzip" => "gz",
        "application/octet-stream" => "bin",
        "image/png" => "png",
        "image/jpeg" => "jpg",
        "image/gif" => "gif",
        "image/svg+xml" => "svg",
        "image/webp" => "webp",
        _ => "bin",
    }
}

// ============================================================================
// Router
// ============================================================================

/// Register artifact routes
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        // Artifact CRUD
        .route("/artifacts", get(list_artifacts).post(create_artifact))
        .route(
            "/artifacts/{id}",
            get(get_artifact)
                .put(update_artifact)
                .delete(delete_artifact),
        )
        .route("/artifacts/ref/{ref}", get(get_artifact_by_ref))
        // Progress / data
        .route("/artifacts/{id}/progress", post(append_progress))
        .route(
            "/artifacts/{id}/data",
            axum::routing::put(set_artifact_data),
        )
        // Download (latest)
        .route("/artifacts/{id}/download", get(download_latest))
        // Version management
        .route(
            "/artifacts/{id}/versions",
            get(list_versions).post(create_version_json),
        )
        .route("/artifacts/{id}/versions/latest", get(get_latest_version))
        .route("/artifacts/{id}/versions/upload", post(upload_version))
        .route(
            "/artifacts/{id}/versions/{version}",
            get(get_version).delete(delete_version),
        )
        .route(
            "/artifacts/{id}/versions/{version}/download",
            get(download_version),
        )
        // By execution
        .route(
            "/executions/{execution_id}/artifacts",
            get(list_artifacts_by_execution),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_artifact_routes_structure() {
        let _router = routes();
    }

    #[test]
    fn test_extension_from_content_type() {
        assert_eq!(extension_from_content_type("text/plain"), "txt");
        assert_eq!(extension_from_content_type("application/json"), "json");
        assert_eq!(extension_from_content_type("image/png"), "png");
        assert_eq!(extension_from_content_type("unknown/type"), "bin");
    }
}
