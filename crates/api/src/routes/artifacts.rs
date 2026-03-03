//! Artifact management API routes
//!
//! Provides endpoints for:
//! - CRUD operations on artifacts (metadata + data)
//! - File-backed version creation (execution writes file to shared volume)
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
use tracing::warn;

use attune_common::models::enums::{ArtifactType, ArtifactVisibility};
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
            CreateFileVersionRequest, CreateVersionJsonRequest, SetDataRequest,
            UpdateArtifactRequest,
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
        visibility: query.visibility,
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

    // Type-aware visibility default: progress artifacts are public by default
    // (they're informational status indicators), everything else is private.
    let visibility = request.visibility.unwrap_or_else(|| {
        if request.r#type == ArtifactType::Progress {
            ArtifactVisibility::Public
        } else {
            ArtifactVisibility::Private
        }
    });

    let input = CreateArtifactInput {
        r#ref: request.r#ref,
        scope: request.scope,
        owner: request.owner,
        r#type: request.r#type,
        visibility,
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
        visibility: request.visibility,
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

/// Delete an artifact (cascades to all versions, including disk files)
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
    let artifact = ArtifactRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Artifact with ID {} not found", id)))?;

    // Before deleting DB rows, clean up any file-backed versions on disk
    let file_versions =
        ArtifactVersionRepository::find_file_versions_by_artifact(&state.db, id).await?;
    if !file_versions.is_empty() {
        let artifacts_dir = &state.config.artifacts_dir;
        cleanup_version_files(artifacts_dir, &file_versions);
        // Also try to remove the artifact's parent directory if it's now empty
        let ref_dir = ref_to_dir_path(&artifact.r#ref);
        let full_ref_dir = std::path::Path::new(artifacts_dir).join(&ref_dir);
        cleanup_empty_parents(&full_ref_dir, artifacts_dir);
    }

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
        file_path: None,
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

/// Create a new file-backed version (no file content in request).
///
/// This endpoint allocates a version number and computes a `file_path` on the
/// shared artifact volume. The caller (execution process) is expected to write
/// the file content directly to `$ATTUNE_ARTIFACTS_DIR/{file_path}` after
/// receiving the response. The worker finalizes `size_bytes` after execution.
///
/// Only applicable to file-type artifacts (FileBinary, FileDatatable, FileText, Log).
#[utoipa::path(
    post,
    path = "/api/v1/artifacts/{id}/versions/file",
    tag = "artifacts",
    params(("id" = i64, Path, description = "Artifact ID")),
    request_body = CreateFileVersionRequest,
    responses(
        (status = 201, description = "File version allocated", body = inline(ApiResponse<ArtifactVersionResponse>)),
        (status = 400, description = "Artifact type is not file-based"),
        (status = 404, description = "Artifact not found"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn create_version_file(
    RequireAuth(_user): RequireAuth,
    State(state): State<Arc<AppState>>,
    Path(id): Path<i64>,
    Json(request): Json<CreateFileVersionRequest>,
) -> ApiResult<impl IntoResponse> {
    let artifact = ArtifactRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Artifact with ID {} not found", id)))?;

    // Validate this is a file-type artifact
    if !is_file_backed_type(artifact.r#type) {
        return Err(ApiError::BadRequest(format!(
            "Artifact '{}' is type {:?}, which does not support file-backed versions. \
             Use POST /versions for JSON or POST /versions/upload for DB-stored files.",
            artifact.r#ref, artifact.r#type,
        )));
    }

    let content_type = request
        .content_type
        .unwrap_or_else(|| default_content_type_for_artifact(artifact.r#type));

    // We need the version number to compute the file path. The DB function
    // `next_artifact_version()` is called inside the INSERT, so we create the
    // row first with file_path = NULL, then compute the path from the returned
    // version number and update the row. This avoids a race condition where two
    // concurrent requests could compute the same version number.
    let input = CreateArtifactVersionInput {
        artifact: id,
        content_type: Some(content_type.clone()),
        content: None,
        content_json: None,
        file_path: None, // Will be set in the update below
        meta: request.meta,
        created_by: request.created_by,
    };

    let version = ArtifactVersionRepository::create(&state.db, input).await?;

    // Compute the file path from the artifact ref and version number
    let file_path = compute_file_path(&artifact.r#ref, version.version, &content_type);

    // Create the parent directory on disk
    let artifacts_dir = &state.config.artifacts_dir;
    let full_path = std::path::Path::new(artifacts_dir).join(&file_path);
    if let Some(parent) = full_path.parent() {
        tokio::fs::create_dir_all(parent).await.map_err(|e| {
            ApiError::InternalServerError(format!(
                "Failed to create artifact directory '{}': {}",
                parent.display(),
                e,
            ))
        })?;
    }

    // Update the version row with the computed file_path
    sqlx::query("UPDATE artifact_version SET file_path = $1 WHERE id = $2")
        .bind(&file_path)
        .execute(&state.db)
        .await
        .map_err(|e| {
            ApiError::InternalServerError(format!(
                "Failed to set file_path on version {}: {}",
                version.id, e,
            ))
        })?;

    // Return the version with file_path populated
    let mut response = ArtifactVersionResponse::from(version);
    response.file_path = Some(file_path);

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::with_message(
            response,
            "File version allocated — write content to $ATTUNE_ARTIFACTS_DIR/<file_path>",
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
        file_path: None,
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

/// Download the binary content of a specific version.
///
/// For file-backed versions, reads from the shared artifact volume on disk.
/// For DB-stored versions, reads from the BYTEA/JSON content column.
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

    // First try without content (cheaper query) to check for file_path
    let ver = ArtifactVersionRepository::find_by_version(&state.db, id, version)
        .await?
        .ok_or_else(|| {
            ApiError::NotFound(format!("Version {} not found for artifact {}", version, id))
        })?;

    // File-backed version: read from disk
    if let Some(ref file_path) = ver.file_path {
        return serve_file_from_disk(
            &state.config.artifacts_dir,
            file_path,
            &artifact.r#ref,
            version,
            ver.content_type.as_deref(),
        )
        .await;
    }

    // DB-stored version: need to fetch with content
    let ver = ArtifactVersionRepository::find_by_version_with_content(&state.db, id, version)
        .await?
        .ok_or_else(|| {
            ApiError::NotFound(format!("Version {} not found for artifact {}", version, id))
        })?;

    serve_db_content(&artifact.r#ref, version, &ver)
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

    // First try without content (cheaper query) to check for file_path
    let ver = ArtifactVersionRepository::find_latest(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("No versions found for artifact {}", id)))?;

    let version = ver.version;

    // File-backed version: read from disk
    if let Some(ref file_path) = ver.file_path {
        return serve_file_from_disk(
            &state.config.artifacts_dir,
            file_path,
            &artifact.r#ref,
            version,
            ver.content_type.as_deref(),
        )
        .await;
    }

    // DB-stored version: need to fetch with content
    let ver = ArtifactVersionRepository::find_latest_with_content(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("No versions found for artifact {}", id)))?;

    serve_db_content(&artifact.r#ref, ver.version, &ver)
}

/// Delete a specific version by version number (including disk file if file-backed)
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
    let artifact = ArtifactRepository::find_by_id(&state.db, id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Artifact with ID {} not found", id)))?;

    // Find the version by artifact + version number
    let ver = ArtifactVersionRepository::find_by_version(&state.db, id, version)
        .await?
        .ok_or_else(|| {
            ApiError::NotFound(format!("Version {} not found for artifact {}", version, id))
        })?;

    // Clean up disk file if file-backed
    if let Some(ref file_path) = ver.file_path {
        let artifacts_dir = &state.config.artifacts_dir;
        let full_path = std::path::Path::new(artifacts_dir).join(file_path);
        if full_path.exists() {
            if let Err(e) = tokio::fs::remove_file(&full_path).await {
                warn!(
                    "Failed to delete artifact file '{}': {}. DB row will still be deleted.",
                    full_path.display(),
                    e
                );
            }
        }
        // Try to clean up empty parent directories
        let ref_dir = ref_to_dir_path(&artifact.r#ref);
        let full_ref_dir = std::path::Path::new(artifacts_dir).join(&ref_dir);
        cleanup_empty_parents(&full_ref_dir, artifacts_dir);
    }

    ArtifactVersionRepository::delete(&state.db, ver.id).await?;

    Ok((
        StatusCode::OK,
        Json(SuccessResponse::new("Version deleted successfully")),
    ))
}

// ============================================================================
// Helpers
// ============================================================================

/// Returns true for artifact types that should use file-backed storage on disk.
fn is_file_backed_type(artifact_type: ArtifactType) -> bool {
    matches!(
        artifact_type,
        ArtifactType::FileBinary
            | ArtifactType::FileText
            | ArtifactType::FileDataTable
            | ArtifactType::FileImage
    )
}

/// Convert an artifact ref to a directory path by replacing dots with path separators.
/// e.g., "mypack.build_log" -> "mypack/build_log"
fn ref_to_dir_path(artifact_ref: &str) -> String {
    artifact_ref.replace('.', "/")
}

/// Compute the relative file path for a file-backed artifact version.
///
/// Pattern: `{ref_slug}/v{version}.{ext}`
/// e.g., `mypack/build_log/v1.txt`
pub fn compute_file_path(artifact_ref: &str, version: i32, content_type: &str) -> String {
    let ref_path = ref_to_dir_path(artifact_ref);
    let ext = extension_from_content_type(content_type);
    format!("{}/v{}.{}", ref_path, version, ext)
}

/// Return a sensible default content type for a given artifact type.
fn default_content_type_for_artifact(artifact_type: ArtifactType) -> String {
    match artifact_type {
        ArtifactType::FileText => "text/plain".to_string(),
        ArtifactType::FileDataTable => "text/csv".to_string(),
        ArtifactType::FileImage => "image/png".to_string(),
        ArtifactType::FileBinary => "application/octet-stream".to_string(),
        _ => "application/octet-stream".to_string(),
    }
}

/// Serve a file-backed artifact version from disk.
async fn serve_file_from_disk(
    artifacts_dir: &str,
    file_path: &str,
    artifact_ref: &str,
    version: i32,
    content_type: Option<&str>,
) -> ApiResult<axum::response::Response> {
    let full_path = std::path::Path::new(artifacts_dir).join(file_path);

    if !full_path.exists() {
        return Err(ApiError::NotFound(format!(
            "File for version {} of artifact '{}' not found on disk (expected at '{}')",
            version, artifact_ref, file_path,
        )));
    }

    let bytes = tokio::fs::read(&full_path).await.map_err(|e| {
        ApiError::InternalServerError(format!(
            "Failed to read artifact file '{}': {}",
            full_path.display(),
            e,
        ))
    })?;

    let ct = content_type
        .unwrap_or("application/octet-stream")
        .to_string();
    let filename = format!(
        "{}_v{}.{}",
        artifact_ref.replace('.', "_"),
        version,
        extension_from_content_type(&ct),
    );

    Ok((
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
        .into_response())
}

/// Serve a DB-stored artifact version (BYTEA or JSON content).
fn serve_db_content(
    artifact_ref: &str,
    version: i32,
    ver: &attune_common::models::artifact_version::ArtifactVersion,
) -> ApiResult<axum::response::Response> {
    // For binary content
    if let Some(ref bytes) = ver.content {
        let ct = ver
            .content_type
            .clone()
            .unwrap_or_else(|| "application/octet-stream".to_string());

        let filename = format!(
            "{}_v{}.{}",
            artifact_ref.replace('.', "_"),
            version,
            extension_from_content_type(&ct),
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
            Body::from(bytes.clone()),
        )
            .into_response());
    }

    // For JSON content, serialize and return
    if let Some(ref json) = ver.content_json {
        let bytes = serde_json::to_vec_pretty(json).map_err(|e| {
            ApiError::InternalServerError(format!("Failed to serialize JSON: {}", e))
        })?;

        let ct = ver
            .content_type
            .clone()
            .unwrap_or_else(|| "application/json".to_string());

        let filename = format!("{}_v{}.json", artifact_ref.replace('.', "_"), version);

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
        "Version {} of artifact '{}' has no downloadable content",
        version, artifact_ref,
    )))
}

/// Delete disk files for a set of file-backed artifact versions.
/// Logs warnings on failure but does not propagate errors.
fn cleanup_version_files(
    artifacts_dir: &str,
    versions: &[attune_common::models::artifact_version::ArtifactVersion],
) {
    for ver in versions {
        if let Some(ref file_path) = ver.file_path {
            let full_path = std::path::Path::new(artifacts_dir).join(file_path);
            if full_path.exists() {
                if let Err(e) = std::fs::remove_file(&full_path) {
                    warn!(
                        "Failed to delete artifact file '{}': {}",
                        full_path.display(),
                        e,
                    );
                }
            }
        }
    }
}

/// Attempt to remove empty parent directories up to (but not including) the
/// artifacts_dir root. This is best-effort cleanup.
fn cleanup_empty_parents(dir: &std::path::Path, stop_at: &str) {
    let stop_path = std::path::Path::new(stop_at);
    let mut current = dir.to_path_buf();
    while current != stop_path && current.starts_with(stop_path) {
        match std::fs::read_dir(&current) {
            Ok(mut entries) => {
                if entries.next().is_some() {
                    // Directory is not empty, stop climbing
                    break;
                }
                if let Err(e) = std::fs::remove_dir(&current) {
                    warn!(
                        "Failed to remove empty directory '{}': {}",
                        current.display(),
                        e,
                    );
                    break;
                }
            }
            Err(_) => break,
        }
        match current.parent() {
            Some(parent) => current = parent.to_path_buf(),
            None => break,
        }
    }
}

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
        .route("/artifacts/{id}/versions/file", post(create_version_file))
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

    #[test]
    fn test_compute_file_path() {
        assert_eq!(
            compute_file_path("mypack.build_log", 1, "text/plain"),
            "mypack/build_log/v1.txt"
        );
        assert_eq!(
            compute_file_path("mypack.build_log", 3, "application/json"),
            "mypack/build_log/v3.json"
        );
        assert_eq!(
            compute_file_path("core.test.results", 2, "text/csv"),
            "core/test/results/v2.csv"
        );
        assert_eq!(
            compute_file_path("simple", 1, "application/octet-stream"),
            "simple/v1.bin"
        );
    }

    #[test]
    fn test_ref_to_dir_path() {
        assert_eq!(ref_to_dir_path("mypack.build_log"), "mypack/build_log");
        assert_eq!(ref_to_dir_path("simple"), "simple");
        assert_eq!(ref_to_dir_path("a.b.c.d"), "a/b/c/d");
    }

    #[test]
    fn test_is_file_backed_type() {
        assert!(is_file_backed_type(ArtifactType::FileBinary));
        assert!(is_file_backed_type(ArtifactType::FileText));
        assert!(is_file_backed_type(ArtifactType::FileDataTable));
        assert!(is_file_backed_type(ArtifactType::FileImage));
        assert!(!is_file_backed_type(ArtifactType::Progress));
        assert!(!is_file_backed_type(ArtifactType::Url));
    }

    #[test]
    fn test_default_content_type_for_artifact() {
        assert_eq!(
            default_content_type_for_artifact(ArtifactType::FileText),
            "text/plain"
        );
        assert_eq!(
            default_content_type_for_artifact(ArtifactType::FileDataTable),
            "text/csv"
        );
        assert_eq!(
            default_content_type_for_artifact(ArtifactType::FileImage),
            "image/png"
        );
        assert_eq!(
            default_content_type_for_artifact(ArtifactType::FileBinary),
            "application/octet-stream"
        );
    }
}
