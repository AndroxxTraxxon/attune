//! Pack management API routes

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use std::path::PathBuf;
use std::sync::Arc;
use validator::Validate;

use attune_common::models::pack_test::PackTestResult;
use attune_common::repositories::{
    pack::{CreatePackInput, UpdatePackInput},
    Create, Delete, FindById, FindByRef, PackRepository, PackTestRepository, Pagination, Update,
};
use attune_common::workflow::{PackWorkflowService, PackWorkflowServiceConfig};

use crate::{
    auth::middleware::RequireAuth,
    dto::{
        common::{PaginatedResponse, PaginationParams},
        pack::{
            BuildPackEnvsRequest, BuildPackEnvsResponse, CreatePackRequest, DownloadPacksRequest,
            DownloadPacksResponse, GetPackDependenciesRequest, GetPackDependenciesResponse,
            InstallPackRequest, PackInstallResponse, PackResponse, PackSummary,
            PackWorkflowSyncResponse, PackWorkflowValidationResponse, RegisterPackRequest,
            RegisterPacksRequest, RegisterPacksResponse, UpdatePackRequest, WorkflowSyncResult,
        },
        ApiResponse, SuccessResponse,
    },
    middleware::{ApiError, ApiResult},
    state::AppState,
};

/// List all packs with pagination
#[utoipa::path(
    get,
    path = "/api/v1/packs",
    tag = "packs",
    params(PaginationParams),
    responses(
        (status = 200, description = "List of packs", body = PaginatedResponse<PackSummary>),
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_packs(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Query(pagination): Query<PaginationParams>,
) -> ApiResult<impl IntoResponse> {
    // Convert to repository pagination (0-based)
    let repo_pagination = Pagination::new(
        (pagination.page.saturating_sub(1)) as i64,
        pagination.limit() as i64,
    );

    // Get packs from repository with pagination
    let packs = PackRepository::list_paginated(&state.db, repo_pagination).await?;

    // Get total count for pagination
    let total = PackRepository::count(&state.db).await?;

    // Convert to summaries
    let summaries: Vec<PackSummary> = packs.into_iter().map(PackSummary::from).collect();

    let response = PaginatedResponse::new(summaries, &pagination, total as u64);

    Ok((StatusCode::OK, Json(response)))
}

/// Get a single pack by reference
#[utoipa::path(
    get,
    path = "/api/v1/packs/{ref}",
    tag = "packs",
    params(
        ("ref" = String, Path, description = "Pack reference identifier")
    ),
    responses(
        (status = 200, description = "Pack details", body = inline(ApiResponse<PackResponse>)),
        (status = 404, description = "Pack not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_pack(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(pack_ref): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let pack = PackRepository::find_by_ref(&state.db, &pack_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Pack '{}' not found", pack_ref)))?;

    let response = ApiResponse::new(PackResponse::from(pack));

    Ok((StatusCode::OK, Json(response)))
}

/// Create a new pack
#[utoipa::path(
    post,
    path = "/api/v1/packs",
    tag = "packs",
    request_body = CreatePackRequest,
    responses(
        (status = 201, description = "Pack created successfully", body = inline(ApiResponse<PackResponse>)),
        (status = 400, description = "Validation error"),
        (status = 409, description = "Pack with same ref already exists")
    ),
    security(("bearer_auth" = []))
)]
pub async fn create_pack(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Json(request): Json<CreatePackRequest>,
) -> ApiResult<impl IntoResponse> {
    // Validate request
    request.validate()?;

    // Check if pack with same ref already exists
    if PackRepository::exists_by_ref(&state.db, &request.r#ref).await? {
        return Err(ApiError::Conflict(format!(
            "Pack with ref '{}' already exists",
            request.r#ref
        )));
    }

    // Create pack input
    let pack_input = CreatePackInput {
        r#ref: request.r#ref,
        label: request.label,
        description: request.description,
        version: request.version,
        conf_schema: request.conf_schema,
        config: request.config,
        meta: request.meta,
        tags: request.tags,
        runtime_deps: request.runtime_deps,
        is_standard: request.is_standard,
        installers: serde_json::json!({}),
    };

    let pack = PackRepository::create(&state.db, pack_input).await?;

    // Auto-sync workflows after pack creation
    let packs_base_dir = PathBuf::from(&state.config.packs_base_dir);

    let service_config = PackWorkflowServiceConfig {
        packs_base_dir,
        skip_validation_errors: true, // Don't fail pack creation on workflow errors
        update_existing: true,
        max_file_size: 1024 * 1024,
    };

    let workflow_service = PackWorkflowService::new(state.db.clone(), service_config);

    // Attempt to sync workflows but don't fail if it errors
    match workflow_service.sync_pack_workflows(&pack.r#ref).await {
        Ok(sync_result) => {
            if sync_result.registered_count > 0 {
                tracing::info!(
                    "Auto-synced {} workflows for pack '{}'",
                    sync_result.registered_count,
                    pack.r#ref
                );
            }
        }
        Err(e) => {
            tracing::warn!(
                "Failed to auto-sync workflows for pack '{}': {}",
                pack.r#ref,
                e
            );
        }
    }

    let response = ApiResponse::with_message(PackResponse::from(pack), "Pack created successfully");

    Ok((StatusCode::CREATED, Json(response)))
}

/// Update an existing pack
#[utoipa::path(
    put,
    path = "/api/v1/packs/{ref}",
    tag = "packs",
    params(
        ("ref" = String, Path, description = "Pack reference identifier")
    ),
    request_body = UpdatePackRequest,
    responses(
        (status = 200, description = "Pack updated successfully", body = inline(ApiResponse<PackResponse>)),
        (status = 400, description = "Validation error"),
        (status = 404, description = "Pack not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn update_pack(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(pack_ref): Path<String>,
    Json(request): Json<UpdatePackRequest>,
) -> ApiResult<impl IntoResponse> {
    // Validate request
    request.validate()?;

    // Check if pack exists
    let existing_pack = PackRepository::find_by_ref(&state.db, &pack_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Pack '{}' not found", pack_ref)))?;

    // Create update input
    let update_input = UpdatePackInput {
        label: request.label,
        description: request.description,
        version: request.version,
        conf_schema: request.conf_schema,
        config: request.config,
        meta: request.meta,
        tags: request.tags,
        runtime_deps: request.runtime_deps,
        is_standard: request.is_standard,
        installers: None,
    };

    let pack = PackRepository::update(&state.db, existing_pack.id, update_input).await?;

    // Auto-sync workflows after pack update
    let packs_base_dir = PathBuf::from(&state.config.packs_base_dir);

    let service_config = PackWorkflowServiceConfig {
        packs_base_dir,
        skip_validation_errors: true, // Don't fail pack update on workflow errors
        update_existing: true,
        max_file_size: 1024 * 1024,
    };

    let workflow_service = PackWorkflowService::new(state.db.clone(), service_config);

    // Attempt to sync workflows but don't fail if it errors
    match workflow_service.sync_pack_workflows(&pack.r#ref).await {
        Ok(sync_result) => {
            if sync_result.registered_count > 0 {
                tracing::info!(
                    "Auto-synced {} workflows for pack '{}'",
                    sync_result.registered_count,
                    pack.r#ref
                );
            }
        }
        Err(e) => {
            tracing::warn!(
                "Failed to auto-sync workflows for pack '{}': {}",
                pack.r#ref,
                e
            );
        }
    }

    let response = ApiResponse::with_message(PackResponse::from(pack), "Pack updated successfully");

    Ok((StatusCode::OK, Json(response)))
}

/// Delete a pack
#[utoipa::path(
    delete,
    path = "/api/v1/packs/{ref}",
    tag = "packs",
    params(
        ("ref" = String, Path, description = "Pack reference identifier")
    ),
    responses(
        (status = 200, description = "Pack deleted successfully", body = SuccessResponse),
        (status = 404, description = "Pack not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn delete_pack(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(pack_ref): Path<String>,
) -> ApiResult<impl IntoResponse> {
    // Check if pack exists
    let pack = PackRepository::find_by_ref(&state.db, &pack_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Pack '{}' not found", pack_ref)))?;

    // Delete the pack
    let deleted = PackRepository::delete(&state.db, pack.id).await?;

    if !deleted {
        return Err(ApiError::NotFound(format!("Pack '{}' not found", pack_ref)));
    }

    let response = SuccessResponse::new(format!("Pack '{}' deleted successfully", pack_ref));

    Ok((StatusCode::OK, Json(response)))
}

/// Helper function to execute pack tests and store results
async fn execute_and_store_pack_tests(
    state: &AppState,
    pack_id: i64,
    pack_ref: &str,
    pack_version: &str,
    trigger_type: &str,
) -> Result<attune_common::models::pack_test::PackTestResult, ApiError> {
    use attune_common::test_executor::{TestConfig, TestExecutor};
    use serde_yaml_ng;

    // Load pack.yaml from filesystem
    let packs_base_dir = PathBuf::from(&state.config.packs_base_dir);
    let pack_dir = packs_base_dir.join(pack_ref);

    if !pack_dir.exists() {
        return Err(ApiError::NotFound(format!(
            "Pack directory not found: {}",
            pack_dir.display()
        )));
    }

    let pack_yaml_path = pack_dir.join("pack.yaml");
    if !pack_yaml_path.exists() {
        return Err(ApiError::NotFound(format!(
            "pack.yaml not found for pack '{}'",
            pack_ref
        )));
    }

    // Parse pack.yaml
    let pack_yaml_content = tokio::fs::read_to_string(&pack_yaml_path)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Failed to read pack.yaml: {}", e)))?;

    let pack_yaml: serde_yaml_ng::Value = serde_yaml_ng::from_str(&pack_yaml_content)
        .map_err(|e| ApiError::InternalServerError(format!("Failed to parse pack.yaml: {}", e)))?;

    // Extract test configuration
    let testing_config = pack_yaml.get("testing").ok_or_else(|| {
        ApiError::BadRequest(format!(
            "No testing configuration found in pack.yaml for pack '{}'",
            pack_ref
        ))
    })?;

    let test_config: TestConfig =
        serde_yaml_ng::from_value(testing_config.clone()).map_err(|e| {
            ApiError::InternalServerError(format!("Failed to parse test configuration: {}", e))
        })?;

    if !test_config.enabled {
        return Err(ApiError::BadRequest(format!(
            "Testing is disabled for pack '{}'",
            pack_ref
        )));
    }

    // Create test executor
    let executor = TestExecutor::new(packs_base_dir);

    // Execute tests
    let result = executor
        .execute_pack_tests(pack_ref, pack_version, &test_config)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Test execution failed: {}", e)))?;

    // Store test results in database
    let pack_test_repo = PackTestRepository::new(state.db.clone());
    pack_test_repo
        .create(pack_id, pack_version, trigger_type, &result)
        .await
        .map_err(|e| {
            tracing::warn!("Failed to store test results: {}", e);
            ApiError::DatabaseError(format!("Failed to store test results: {}", e))
        })?;

    Ok(result)
}

/// Register a pack from local filesystem
#[utoipa::path(
    post,
    path = "/api/v1/packs/register",
    tag = "packs",
    request_body = RegisterPackRequest,
    responses(
        (status = 201, description = "Pack registered successfully", body = ApiResponse<PackInstallResponse>),
        (status = 400, description = "Invalid request or tests failed", body = ApiResponse<String>),
        (status = 409, description = "Pack already exists", body = ApiResponse<String>),
    ),
    security(("bearer_auth" = []))
)]
pub async fn register_pack(
    State(state): State<Arc<AppState>>,
    RequireAuth(user): RequireAuth,
    Json(request): Json<crate::dto::pack::RegisterPackRequest>,
) -> ApiResult<impl IntoResponse> {
    // Validate request
    request.validate()?;

    // Call internal registration logic
    let pack_id = register_pack_internal(
        state.clone(),
        user.claims.sub,
        request.path.clone(),
        request.force,
        request.skip_tests,
    )
    .await?;

    // Fetch the registered pack
    let pack = PackRepository::find_by_id(&state.db, pack_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Pack with ID {} not found", pack_id)))?;

    let response =
        ApiResponse::with_message(PackResponse::from(pack), "Pack registered successfully");

    Ok((StatusCode::CREATED, Json(response)))
}

/// Internal helper function for pack registration logic
async fn register_pack_internal(
    state: Arc<AppState>,
    _user_id: String,
    path: String,
    force: bool,
    skip_tests: bool,
) -> Result<i64, ApiError> {
    use std::fs;

    // Verify pack directory exists
    let pack_path = PathBuf::from(&path);
    if !pack_path.exists() || !pack_path.is_dir() {
        return Err(ApiError::BadRequest(format!(
            "Pack directory does not exist: {}",
            path
        )));
    }

    // Read pack.yaml
    let pack_yaml_path = pack_path.join("pack.yaml");
    if !pack_yaml_path.exists() {
        return Err(ApiError::BadRequest(format!(
            "pack.yaml not found in directory: {}",
            path
        )));
    }

    let pack_yaml_content = fs::read_to_string(&pack_yaml_path)
        .map_err(|e| ApiError::InternalServerError(format!("Failed to read pack.yaml: {}", e)))?;

    let pack_yaml: serde_yaml_ng::Value = serde_yaml_ng::from_str(&pack_yaml_content)
        .map_err(|e| ApiError::InternalServerError(format!("Failed to parse pack.yaml: {}", e)))?;

    // Extract pack metadata
    let pack_ref = pack_yaml
        .get("ref")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::BadRequest("Missing 'ref' field in pack.yaml".to_string()))?
        .to_string();

    let label = pack_yaml
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(&pack_ref)
        .to_string();

    let version = pack_yaml
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| ApiError::BadRequest("Missing 'version' field in pack.yaml".to_string()))?
        .to_string();

    let description = pack_yaml
        .get("description")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Check if pack already exists
    if !force {
        if PackRepository::exists_by_ref(&state.db, &pack_ref).await? {
            return Err(ApiError::Conflict(format!(
                "Pack '{}' already exists. Use force=true to reinstall.",
                pack_ref
            )));
        }
    } else {
        // Delete existing pack if force is true
        if let Some(existing_pack) = PackRepository::find_by_ref(&state.db, &pack_ref).await? {
            PackRepository::delete(&state.db, existing_pack.id).await?;
            tracing::info!("Deleted existing pack '{}' for forced reinstall", pack_ref);
        }
    }

    // Create pack input
    let pack_input = CreatePackInput {
        r#ref: pack_ref.clone(),
        label,
        description,
        version: version.clone(),
        conf_schema: pack_yaml
            .get("config_schema")
            .and_then(|v| serde_json::to_value(v).ok())
            .unwrap_or_else(|| serde_json::json!({})),
        config: serde_json::json!({}),
        meta: pack_yaml
            .get("metadata")
            .and_then(|v| serde_json::to_value(v).ok())
            .unwrap_or_else(|| serde_json::json!({})),
        tags: pack_yaml
            .get("keywords")
            .and_then(|v| v.as_sequence())
            .map(|seq| {
                seq.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default(),
        runtime_deps: pack_yaml
            .get("dependencies")
            .and_then(|v| v.as_sequence())
            .map(|seq| {
                seq.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default(),
        is_standard: false,
        installers: serde_json::json!({}),
    };

    let pack = PackRepository::create(&state.db, pack_input).await?;

    // Auto-sync workflows after pack creation
    let packs_base_dir = PathBuf::from(&state.config.packs_base_dir);
    let service_config = PackWorkflowServiceConfig {
        packs_base_dir: packs_base_dir.clone(),
        skip_validation_errors: true,
        update_existing: true,
        max_file_size: 1024 * 1024,
    };

    let workflow_service = PackWorkflowService::new(state.db.clone(), service_config);

    // Attempt to sync workflows but don't fail if it errors
    match workflow_service.sync_pack_workflows(&pack.r#ref).await {
        Ok(sync_result) => {
            if sync_result.registered_count > 0 {
                tracing::info!(
                    "Auto-synced {} workflows for pack '{}'",
                    sync_result.registered_count,
                    pack.r#ref
                );
            }
        }
        Err(e) => {
            tracing::warn!(
                "Failed to auto-sync workflows for pack '{}': {}",
                pack.r#ref,
                e
            );
        }
    }

    // Execute tests if not skipped
    if !skip_tests {
        match execute_and_store_pack_tests(&state, pack.id, &pack.r#ref, &pack.version, "register")
            .await
        {
            Ok(result) => {
                let test_passed = result.status == "passed";

                if !test_passed && !force {
                    // Tests failed and force is not set - rollback pack creation
                    let _ = PackRepository::delete(&state.db, pack.id).await;
                    return Err(ApiError::BadRequest(format!(
                        "Pack registration failed: tests did not pass. Use force=true to register anyway."
                    )));
                }

                if !test_passed && force {
                    tracing::warn!(
                        "Pack '{}' tests failed but force=true, continuing with registration",
                        pack.r#ref
                    );
                }
            }
            Err(e) => {
                tracing::warn!("Failed to execute tests for pack '{}': {}", pack.r#ref, e);
                // If tests can't be executed and force is not set, fail the registration
                if !force {
                    let _ = PackRepository::delete(&state.db, pack.id).await;
                    return Err(ApiError::BadRequest(format!(
                        "Pack registration failed: could not execute tests. Error: {}. Use force=true to register anyway.",
                        e
                    )));
                }
            }
        }
    }

    Ok(pack.id)
}

/// Install a pack from remote source (git repository)
#[utoipa::path(
    post,
    path = "/api/v1/packs/install",
    tag = "packs",
    request_body = InstallPackRequest,
    responses(
        (status = 201, description = "Pack installed successfully", body = ApiResponse<PackInstallResponse>),
        (status = 400, description = "Invalid request or tests failed", body = ApiResponse<String>),
        (status = 409, description = "Pack already exists", body = ApiResponse<String>),
        (status = 501, description = "Not implemented yet", body = ApiResponse<String>),
    ),
    security(("bearer_auth" = []))
)]
pub async fn install_pack(
    State(state): State<Arc<AppState>>,
    RequireAuth(user): RequireAuth,
    Json(request): Json<crate::dto::pack::InstallPackRequest>,
) -> ApiResult<(
    StatusCode,
    Json<crate::dto::ApiResponse<PackInstallResponse>>,
)> {
    use attune_common::pack_registry::{
        calculate_directory_checksum, DependencyValidator, PackInstaller, PackStorage,
    };
    use attune_common::repositories::List;

    tracing::info!("Installing pack from source: {}", request.source);

    // Get user ID early to avoid borrow issues
    let user_id = user.identity_id().ok();
    let user_sub = user.claims.sub.clone();

    // Create temp directory for installations
    let temp_dir = std::env::temp_dir().join("attune-pack-installs");

    // Load registry configuration
    let registry_config = if state.config.pack_registry.enabled {
        Some(state.config.pack_registry.clone())
    } else {
        None
    };

    // Create installer
    let installer = PackInstaller::new(&temp_dir, registry_config)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Failed to create installer: {}", e)))?;

    // Detect source type and create PackSource
    let source = detect_pack_source(&request.source, request.ref_spec.as_deref())?;
    let source_type = get_source_type(&source);

    // Install the pack (to temporary location)
    let installed = installer.install(source.clone()).await?;

    tracing::info!("Pack downloaded to: {:?}", installed.path);

    // Validate dependencies if not skipping
    if !request.skip_deps {
        tracing::info!("Validating pack dependencies...");

        // Load pack.yaml for dependency information
        let pack_yaml_path = installed.path.join("pack.yaml");
        if !pack_yaml_path.exists() {
            return Err(ApiError::BadRequest(format!(
                "pack.yaml not found in installed pack at: {}",
                installed.path.display()
            )));
        }

        let pack_yaml_content = std::fs::read_to_string(&pack_yaml_path).map_err(|e| {
            ApiError::InternalServerError(format!("Failed to read pack.yaml: {}", e))
        })?;

        let pack_yaml: serde_yaml_ng::Value =
            serde_yaml_ng::from_str(&pack_yaml_content).map_err(|e| {
                ApiError::InternalServerError(format!("Failed to parse pack.yaml: {}", e))
            })?;

        let mut validator = DependencyValidator::new();

        // Extract runtime dependencies from pack.yaml
        let mut runtime_deps: Vec<String> = Vec::new();

        if let Some(python_version) = pack_yaml.get("python").and_then(|v| v.as_str()) {
            runtime_deps.push(format!("python3>={}", python_version));
        }

        if let Some(nodejs_version) = pack_yaml.get("nodejs").and_then(|v| v.as_str()) {
            runtime_deps.push(format!("nodejs>={}", nodejs_version));
        }

        // Extract pack dependencies (ref, version)
        let pack_deps: Vec<(String, String)> = pack_yaml
            .get("dependencies")
            .and_then(|v| v.as_sequence())
            .map(|seq| {
                seq.iter()
                    .filter_map(|v| v.as_str().map(|s| (s.to_string(), "*".to_string())))
                    .collect()
            })
            .unwrap_or_default();

        // Get installed packs from database
        let installed_packs_list = PackRepository::list(&state.db).await?;
        let installed_packs: std::collections::HashMap<String, String> = installed_packs_list
            .into_iter()
            .map(|p| (p.r#ref, p.version))
            .collect();

        match validator
            .validate(&runtime_deps, &pack_deps, &installed_packs)
            .await
        {
            Ok(validation) => {
                if !validation.valid {
                    tracing::warn!("Pack dependency validation failed: {:?}", validation.errors);

                    // Return validation errors to user
                    return Err(ApiError::BadRequest(format!(
                        "Pack dependency validation failed:\n  - {}",
                        validation.errors.join("\n  - ")
                    )));
                }
                tracing::info!("All dependencies validated successfully");
            }
            Err(e) => {
                tracing::error!("Dependency validation error: {}", e);
                return Err(ApiError::InternalServerError(format!(
                    "Failed to validate dependencies: {}",
                    e
                )));
            }
        }
    } else {
        tracing::info!("Skipping dependency validation (disabled by user)");
    }

    // Register the pack in database (from temp location)
    let register_request = crate::dto::pack::RegisterPackRequest {
        path: installed.path.to_string_lossy().to_string(),
        force: request.force,
        skip_tests: request.skip_tests,
    };

    let pack_id = register_pack_internal(
        state.clone(),
        user_sub,
        register_request.path.clone(),
        register_request.force,
        register_request.skip_tests,
    )
    .await?;

    // Fetch the registered pack to get pack_ref and version
    let pack = PackRepository::find_by_id(&state.db, pack_id)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Pack with ID {} not found", pack_id)))?;

    // Move pack to permanent storage
    let storage = PackStorage::new(&state.config.packs_base_dir);
    let final_path = storage
        .install_pack(&installed.path, &pack.r#ref, Some(&pack.version))
        .map_err(|e| {
            ApiError::InternalServerError(format!("Failed to move pack to storage: {}", e))
        })?;

    tracing::info!("Pack installed to permanent storage: {:?}", final_path);

    // Calculate checksum of installed pack
    let checksum = calculate_directory_checksum(&final_path)
        .map_err(|e| {
            tracing::warn!("Failed to calculate checksum: {}", e);
            e
        })
        .ok();

    // Store installation metadata
    let (source_url, source_ref) =
        get_source_metadata(&source, &request.source, request.ref_spec.as_deref());

    PackRepository::update_installation_metadata(
        &state.db,
        pack_id,
        source_type.to_string(),
        source_url,
        source_ref,
        checksum.clone(),
        installed.checksum.is_some() && checksum.is_some(),
        user_id,
        "api".to_string(),
        final_path.to_string_lossy().to_string(),
    )
    .await
    .map_err(|e| {
        tracing::warn!("Failed to store installation metadata: {}", e);
        ApiError::DatabaseError(format!("Failed to store installation metadata: {}", e))
    })?;

    // Clean up temp directory
    let _ = installer.cleanup(&installed.path).await;

    let response = PackInstallResponse {
        pack: PackResponse::from(pack),
        test_result: None, // TODO: Include test results
        tests_skipped: register_request.skip_tests,
    };

    Ok((StatusCode::OK, Json(crate::dto::ApiResponse::new(response))))
}

fn detect_pack_source(
    source: &str,
    ref_spec: Option<&str>,
) -> Result<attune_common::pack_registry::PackSource, ApiError> {
    use attune_common::pack_registry::PackSource;
    use std::path::Path;

    // Check if it's a URL
    if source.starts_with("http://") || source.starts_with("https://") {
        if source.ends_with(".git") || ref_spec.is_some() {
            return Ok(PackSource::Git {
                url: source.to_string(),
                git_ref: ref_spec.map(String::from),
            });
        }
        return Ok(PackSource::Archive {
            url: source.to_string(),
        });
    }

    // Check if it's a git SSH URL
    if source.starts_with("git@") || source.contains("git://") {
        return Ok(PackSource::Git {
            url: source.to_string(),
            git_ref: ref_spec.map(String::from),
        });
    }

    // Check if it's a local path
    let path = Path::new(source);
    if path.exists() {
        if path.is_file() {
            return Ok(PackSource::LocalArchive {
                path: path.to_path_buf(),
            });
        }
        return Ok(PackSource::LocalDirectory {
            path: path.to_path_buf(),
        });
    }

    // Otherwise assume it's a registry reference
    // Parse version if present (format: "pack@version" or "pack")
    let (pack_ref, version) = if let Some(at_pos) = source.find('@') {
        let (pack, ver) = source.split_at(at_pos);
        (pack.to_string(), Some(ver[1..].to_string()))
    } else {
        (source.to_string(), None)
    };

    Ok(PackSource::Registry { pack_ref, version })
}

/// Get source type string from PackSource
fn get_source_type(source: &attune_common::pack_registry::PackSource) -> &'static str {
    use attune_common::pack_registry::PackSource;
    match source {
        PackSource::Git { .. } => "git",
        PackSource::Archive { .. } => "archive",
        PackSource::LocalDirectory { .. } => "local_directory",
        PackSource::LocalArchive { .. } => "local_archive",
        PackSource::Registry { .. } => "registry",
    }
}

/// Extract source URL and ref from PackSource
fn get_source_metadata(
    source: &attune_common::pack_registry::PackSource,
    original_source: &str,
    _ref_spec: Option<&str>,
) -> (Option<String>, Option<String>) {
    use attune_common::pack_registry::PackSource;
    match source {
        PackSource::Git { url, git_ref } => (Some(url.clone()), git_ref.clone()),
        PackSource::Archive { url } => (Some(url.clone()), None),
        PackSource::LocalDirectory { path } => (Some(path.to_string_lossy().to_string()), None),
        PackSource::LocalArchive { path } => (Some(path.to_string_lossy().to_string()), None),
        PackSource::Registry {
            pack_ref: _,
            version,
        } => (Some(original_source.to_string()), version.clone()),
    }
}

/// Sync workflows from filesystem to database for a pack
#[utoipa::path(
    post,
    path = "/api/v1/packs/{ref}/workflows/sync",
    tag = "packs",
    params(
        ("ref" = String, Path, description = "Pack reference identifier")
    ),
    responses(
        (status = 200, description = "Workflows synced successfully", body = inline(ApiResponse<PackWorkflowSyncResponse>)),
        (status = 404, description = "Pack not found"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn sync_pack_workflows(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(pack_ref): Path<String>,
) -> ApiResult<impl IntoResponse> {
    // Get packs base directory from config
    let packs_base_dir = PathBuf::from(&state.config.packs_base_dir);

    // Create workflow service
    let service_config = PackWorkflowServiceConfig {
        packs_base_dir,
        skip_validation_errors: false,
        update_existing: true,
        max_file_size: 1024 * 1024, // 1MB
    };

    let service = PackWorkflowService::new(state.db.clone(), service_config);

    // Sync workflows
    let result = service.sync_pack_workflows(&pack_ref).await?;

    // Convert to response DTO
    let response = PackWorkflowSyncResponse {
        pack_ref: result.pack_ref,
        loaded_count: result.loaded_count,
        registered_count: result.registered_count,
        workflows: result
            .workflows
            .into_iter()
            .map(|w| WorkflowSyncResult {
                ref_name: w.ref_name,
                created: w.created,
                workflow_def_id: w.workflow_def_id,
                warnings: w.warnings,
            })
            .collect(),
        errors: result.errors,
    };

    Ok((
        StatusCode::OK,
        Json(ApiResponse::with_message(
            response,
            "Pack workflows synced successfully",
        )),
    ))
}

/// Validate workflows for a pack without syncing
#[utoipa::path(
    post,
    path = "/api/v1/packs/{ref}/workflows/validate",
    tag = "packs",
    params(
        ("ref" = String, Path, description = "Pack reference identifier")
    ),
    responses(
        (status = 200, description = "Workflows validated", body = inline(ApiResponse<PackWorkflowValidationResponse>)),
        (status = 404, description = "Pack not found"),
        (status = 500, description = "Internal server error")
    ),
    security(("bearer_auth" = []))
)]
pub async fn validate_pack_workflows(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(pack_ref): Path<String>,
) -> ApiResult<impl IntoResponse> {
    // Get packs base directory from config
    let packs_base_dir = PathBuf::from(&state.config.packs_base_dir);

    // Create workflow service
    let service_config = PackWorkflowServiceConfig {
        packs_base_dir,
        skip_validation_errors: false,
        update_existing: false,
        max_file_size: 1024 * 1024, // 1MB
    };

    let service = PackWorkflowService::new(state.db.clone(), service_config);

    // Validate workflows
    let result = service.validate_pack_workflows(&pack_ref).await?;

    // Convert to response DTO
    let response = PackWorkflowValidationResponse {
        pack_ref: result.pack_ref,
        validated_count: result.validated_count,
        error_count: result.error_count,
        errors: result.errors,
    };

    Ok((
        StatusCode::OK,
        Json(ApiResponse::with_message(
            response,
            "Pack workflows validated",
        )),
    ))
}

/// Execute tests for a pack
#[utoipa::path(
    post,
    path = "/api/v1/packs/{ref}/test",
    tag = "packs",
    params(
        ("ref" = String, Path, description = "Pack reference identifier")
    ),
    responses(
        (status = 200, description = "Tests executed successfully", body = inline(ApiResponse<PackTestResult>)),
        (status = 404, description = "Pack not found"),
        (status = 500, description = "Test execution failed")
    ),
    security(("bearer_auth" = []))
)]
pub async fn test_pack(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(pack_ref): Path<String>,
) -> ApiResult<impl IntoResponse> {
    use attune_common::test_executor::{TestConfig, TestExecutor};
    use serde_yaml_ng;

    // Get pack from database
    let pack = PackRepository::find_by_ref(&state.db, &pack_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Pack '{}' not found", pack_ref)))?;

    // Load pack.yaml from filesystem
    let packs_base_dir = PathBuf::from(&state.config.packs_base_dir);
    let pack_dir = packs_base_dir.join(&pack_ref);

    if !pack_dir.exists() {
        return Err(ApiError::NotFound(format!(
            "Pack directory not found: {}",
            pack_dir.display()
        )));
    }

    let pack_yaml_path = pack_dir.join("pack.yaml");
    if !pack_yaml_path.exists() {
        return Err(ApiError::NotFound(format!(
            "pack.yaml not found for pack '{}'",
            pack_ref
        )));
    }

    // Parse pack.yaml
    let pack_yaml_content = tokio::fs::read_to_string(&pack_yaml_path)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Failed to read pack.yaml: {}", e)))?;

    let pack_yaml: serde_yaml_ng::Value = serde_yaml_ng::from_str(&pack_yaml_content)
        .map_err(|e| ApiError::InternalServerError(format!("Failed to parse pack.yaml: {}", e)))?;

    // Extract test configuration
    let testing_config = pack_yaml.get("testing").ok_or_else(|| {
        ApiError::BadRequest("No testing configuration found in pack.yaml".to_string())
    })?;

    let test_config: TestConfig =
        serde_yaml_ng::from_value(testing_config.clone()).map_err(|e| {
            ApiError::InternalServerError(format!("Failed to parse test configuration: {}", e))
        })?;

    if !test_config.enabled {
        return Err(ApiError::BadRequest(
            "Testing is disabled for this pack".to_string(),
        ));
    }

    // Create test executor
    let executor = TestExecutor::new(packs_base_dir);

    // Execute tests
    let result = executor
        .execute_pack_tests(&pack_ref, &pack.version, &test_config)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Test execution failed: {}", e)))?;

    // Store test results in database
    let pack_test_repo = PackTestRepository::new(state.db.clone());
    pack_test_repo
        .create(pack.id, &pack.version, "manual", &result)
        .await
        .map_err(|e| {
            tracing::warn!("Failed to store test results: {}", e);
            ApiError::DatabaseError(format!("Failed to store test results: {}", e))
        })?;

    let response = ApiResponse::with_message(result, "Pack tests executed successfully");

    Ok((StatusCode::OK, Json(response)))
}

/// Get test history for a pack
#[utoipa::path(
    get,
    path = "/api/v1/packs/{ref}/tests",
    tag = "packs",
    params(
        ("ref" = String, Path, description = "Pack reference identifier"),
        PaginationParams
    ),
    responses(
        (status = 200, description = "Test history retrieved", body = inline(PaginatedResponse<attune_common::models::PackTestExecution>)),
        (status = 404, description = "Pack not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_pack_test_history(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(pack_ref): Path<String>,
    Query(pagination): Query<PaginationParams>,
) -> ApiResult<impl IntoResponse> {
    // Get pack from database
    let pack = PackRepository::find_by_ref(&state.db, &pack_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Pack '{}' not found", pack_ref)))?;

    // Get test executions
    let pack_test_repo = PackTestRepository::new(state.db.clone());
    let test_executions = pack_test_repo
        .list_by_pack(
            pack.id,
            pagination.limit() as i64,
            (pagination.page.saturating_sub(1) * pagination.limit()) as i64,
        )
        .await?;

    // Get total count
    let total = pack_test_repo.count_by_pack(pack.id).await?;

    let response = PaginatedResponse::<attune_common::models::PackTestExecution>::new(
        test_executions,
        &pagination,
        total as u64,
    );

    Ok((StatusCode::OK, Json(response)))
}

/// Get latest test result for a pack
#[utoipa::path(
    get,
    path = "/api/v1/packs/{ref}/tests/latest",
    tag = "packs",
    params(
        ("ref" = String, Path, description = "Pack reference identifier")
    ),
    responses(
        (status = 200, description = "Latest test result retrieved"),
        (status = 404, description = "Pack not found or no tests available")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_pack_latest_test(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(pack_ref): Path<String>,
) -> ApiResult<impl IntoResponse> {
    // Get pack from database
    let pack = PackRepository::find_by_ref(&state.db, &pack_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Pack '{}' not found", pack_ref)))?;

    // Get latest test execution
    let pack_test_repo = PackTestRepository::new(state.db.clone());
    let test_execution = pack_test_repo
        .get_latest_by_pack(pack.id)
        .await?
        .ok_or_else(|| {
            ApiError::NotFound(format!("No test results found for pack '{}'", pack_ref))
        })?;

    let response = ApiResponse::new(test_execution);

    Ok((StatusCode::OK, Json(response)))
}

/// Create pack routes
///
/// Note: Nested resource routes (e.g., /packs/:ref/actions) are defined
/// in their respective modules (actions.rs, triggers.rs, rules.rs) to avoid
/// route conflicts and maintain proper separation of concerns.
/// Download packs from various sources
#[utoipa::path(
    post,
    path = "/api/v1/packs/download",
    tag = "packs",
    request_body = DownloadPacksRequest,
    responses(
        (status = 200, description = "Packs downloaded", body = ApiResponse<DownloadPacksResponse>),
        (status = 400, description = "Invalid request"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn download_packs(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Json(request): Json<DownloadPacksRequest>,
) -> ApiResult<Json<ApiResponse<DownloadPacksResponse>>> {
    use attune_common::pack_registry::PackInstaller;

    // Create temp directory
    let temp_dir = std::env::temp_dir().join("attune-pack-downloads");
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| ApiError::InternalServerError(format!("Failed to create temp dir: {}", e)))?;

    // Create installer
    let registry_config = if state.config.pack_registry.enabled {
        Some(state.config.pack_registry.clone())
    } else {
        None
    };

    let installer = PackInstaller::new(&temp_dir, registry_config)
        .await
        .map_err(|e| ApiError::InternalServerError(format!("Failed to create installer: {}", e)))?;

    let mut downloaded = Vec::new();
    let mut failed = Vec::new();

    for source in &request.packs {
        let pack_source = detect_pack_source(source, request.ref_spec.as_deref())?;
        let source_type_str = get_source_type(&pack_source).to_string();

        match installer.install(pack_source).await {
            Ok(installed) => {
                // Read pack.yaml
                let pack_yaml_path = installed.path.join("pack.yaml");
                if let Ok(content) = std::fs::read_to_string(&pack_yaml_path) {
                    if let Ok(yaml) = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&content) {
                        let pack_ref = yaml
                            .get("ref")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let pack_version = yaml
                            .get("version")
                            .and_then(|v| v.as_str())
                            .unwrap_or("0.0.0")
                            .to_string();

                        downloaded.push(crate::dto::pack::DownloadedPack {
                            source: source.clone(),
                            source_type: source_type_str.clone(),
                            pack_path: installed.path.to_string_lossy().to_string(),
                            pack_ref,
                            pack_version,
                            git_commit: None,
                            checksum: installed.checksum,
                        });
                    }
                }
            }
            Err(e) => {
                failed.push(crate::dto::pack::FailedPack {
                    source: source.clone(),
                    error: e.to_string(),
                });
            }
        }
    }

    let response = DownloadPacksResponse {
        success_count: downloaded.len(),
        failure_count: failed.len(),
        total_count: request.packs.len(),
        downloaded_packs: downloaded,
        failed_packs: failed,
    };

    Ok(Json(ApiResponse::new(response)))
}

/// Get pack dependencies
#[utoipa::path(
    post,
    path = "/api/v1/packs/dependencies",
    tag = "packs",
    request_body = GetPackDependenciesRequest,
    responses(
        (status = 200, description = "Dependencies analyzed", body = ApiResponse<GetPackDependenciesResponse>),
        (status = 400, description = "Invalid request"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_pack_dependencies(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Json(request): Json<GetPackDependenciesRequest>,
) -> ApiResult<Json<ApiResponse<GetPackDependenciesResponse>>> {
    use attune_common::repositories::List;

    let mut dependencies = Vec::new();
    let mut runtime_requirements = std::collections::HashMap::new();
    let mut analyzed_packs = Vec::new();
    let mut errors = Vec::new();

    // Get installed packs
    let installed_packs_list = PackRepository::list(&state.db).await?;
    let installed_refs: std::collections::HashSet<String> =
        installed_packs_list.into_iter().map(|p| p.r#ref).collect();

    for pack_path in &request.pack_paths {
        let pack_yaml_path = std::path::Path::new(pack_path).join("pack.yaml");

        if !pack_yaml_path.exists() {
            errors.push(crate::dto::pack::DependencyError {
                pack_path: pack_path.clone(),
                error: "pack.yaml not found".to_string(),
            });
            continue;
        }

        let content = match std::fs::read_to_string(&pack_yaml_path) {
            Ok(c) => c,
            Err(e) => {
                errors.push(crate::dto::pack::DependencyError {
                    pack_path: pack_path.clone(),
                    error: format!("Failed to read pack.yaml: {}", e),
                });
                continue;
            }
        };

        let yaml: serde_yaml_ng::Value = match serde_yaml_ng::from_str(&content) {
            Ok(y) => y,
            Err(e) => {
                errors.push(crate::dto::pack::DependencyError {
                    pack_path: pack_path.clone(),
                    error: format!("Failed to parse pack.yaml: {}", e),
                });
                continue;
            }
        };

        let pack_ref = yaml
            .get("ref")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        // Extract dependencies
        let mut dep_count = 0;
        if let Some(deps) = yaml.get("dependencies").and_then(|d| d.as_sequence()) {
            for dep in deps {
                if let Some(dep_str) = dep.as_str() {
                    let parts: Vec<&str> = dep_str.splitn(2, '@').collect();
                    let dep_ref = parts[0].to_string();
                    let version_spec = parts.get(1).unwrap_or(&"*").to_string();
                    let already_installed = installed_refs.contains(&dep_ref);

                    dependencies.push(crate::dto::pack::PackDependency {
                        pack_ref: dep_ref.clone(),
                        version_spec: version_spec.clone(),
                        required_by: pack_ref.clone(),
                        already_installed,
                    });
                    dep_count += 1;
                }
            }
        }

        // Extract runtime requirements
        let mut runtime_req = crate::dto::pack::RuntimeRequirements {
            pack_ref: pack_ref.clone(),
            python: None,
            nodejs: None,
        };

        if let Some(python_ver) = yaml.get("python").and_then(|v| v.as_str()) {
            let req_file = std::path::Path::new(pack_path).join("requirements.txt");
            runtime_req.python = Some(crate::dto::pack::PythonRequirements {
                version: Some(python_ver.to_string()),
                requirements_file: if req_file.exists() {
                    Some(req_file.to_string_lossy().to_string())
                } else {
                    None
                },
            });
        }

        if let Some(nodejs_ver) = yaml.get("nodejs").and_then(|v| v.as_str()) {
            let pkg_file = std::path::Path::new(pack_path).join("package.json");
            runtime_req.nodejs = Some(crate::dto::pack::NodeJsRequirements {
                version: Some(nodejs_ver.to_string()),
                package_file: if pkg_file.exists() {
                    Some(pkg_file.to_string_lossy().to_string())
                } else {
                    None
                },
            });
        }

        if runtime_req.python.is_some() || runtime_req.nodejs.is_some() {
            runtime_requirements.insert(pack_ref.clone(), runtime_req);
        }

        analyzed_packs.push(crate::dto::pack::AnalyzedPack {
            pack_ref: pack_ref.clone(),
            pack_path: pack_path.clone(),
            has_dependencies: dep_count > 0,
            dependency_count: dep_count,
        });
    }

    let missing_dependencies: Vec<_> = dependencies
        .iter()
        .filter(|d| !d.already_installed)
        .cloned()
        .collect();

    let response = GetPackDependenciesResponse {
        dependencies,
        runtime_requirements,
        missing_dependencies,
        analyzed_packs,
        errors,
    };

    Ok(Json(ApiResponse::new(response)))
}

/// Build pack environments
#[utoipa::path(
    post,
    path = "/api/v1/packs/build-envs",
    tag = "packs",
    request_body = BuildPackEnvsRequest,
    responses(
        (status = 200, description = "Environments built", body = ApiResponse<BuildPackEnvsResponse>),
        (status = 400, description = "Invalid request"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn build_pack_envs(
    State(_state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Json(request): Json<BuildPackEnvsRequest>,
) -> ApiResult<Json<ApiResponse<BuildPackEnvsResponse>>> {
    use std::path::Path;
    use std::process::Command;

    let start = std::time::Instant::now();
    let mut built_environments = Vec::new();
    let mut failed_environments = Vec::new();
    let mut python_envs_built = 0;
    let mut nodejs_envs_built = 0;

    for pack_path in &request.pack_paths {
        let pack_path_obj = Path::new(pack_path);
        let pack_start = std::time::Instant::now();

        // Read pack.yaml to get pack_ref and runtime requirements
        let pack_yaml_path = pack_path_obj.join("pack.yaml");
        if !pack_yaml_path.exists() {
            failed_environments.push(crate::dto::pack::FailedEnvironment {
                pack_ref: "unknown".to_string(),
                pack_path: pack_path.clone(),
                runtime: "unknown".to_string(),
                error: "pack.yaml not found".to_string(),
            });
            continue;
        }

        let content = match std::fs::read_to_string(&pack_yaml_path) {
            Ok(c) => c,
            Err(e) => {
                failed_environments.push(crate::dto::pack::FailedEnvironment {
                    pack_ref: "unknown".to_string(),
                    pack_path: pack_path.clone(),
                    runtime: "unknown".to_string(),
                    error: format!("Failed to read pack.yaml: {}", e),
                });
                continue;
            }
        };

        let yaml: serde_yaml_ng::Value = match serde_yaml_ng::from_str(&content) {
            Ok(y) => y,
            Err(e) => {
                failed_environments.push(crate::dto::pack::FailedEnvironment {
                    pack_ref: "unknown".to_string(),
                    pack_path: pack_path.clone(),
                    runtime: "unknown".to_string(),
                    error: format!("Failed to parse pack.yaml: {}", e),
                });
                continue;
            }
        };

        let pack_ref = yaml
            .get("ref")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string();

        let mut python_env = None;
        let mut nodejs_env = None;
        let mut has_error = false;

        // Check for Python environment
        if !request.skip_python {
            if let Some(_python_ver) = yaml.get("python").and_then(|v| v.as_str()) {
                let requirements_file = pack_path_obj.join("requirements.txt");

                if requirements_file.exists() {
                    // Check if Python is available
                    match Command::new("python3").arg("--version").output() {
                        Ok(output) if output.status.success() => {
                            let version_str = String::from_utf8_lossy(&output.stdout);
                            let venv_path = pack_path_obj.join("venv");

                            // Check if venv exists or if force_rebuild is set
                            if !venv_path.exists() || request.force_rebuild {
                                tracing::info!(
                                    pack_ref = %pack_ref,
                                    "Python environment would be built here in production"
                                );
                            }

                            // Report environment status (detection mode)
                            python_env = Some(crate::dto::pack::PythonEnvironment {
                                virtualenv_path: venv_path.to_string_lossy().to_string(),
                                requirements_installed: venv_path.exists(),
                                package_count: 0, // Would count from pip freeze in production
                                python_version: version_str.trim().to_string(),
                            });
                            python_envs_built += 1;
                        }
                        _ => {
                            failed_environments.push(crate::dto::pack::FailedEnvironment {
                                pack_ref: pack_ref.clone(),
                                pack_path: pack_path.clone(),
                                runtime: "python".to_string(),
                                error: "Python 3 not available in system".to_string(),
                            });
                            has_error = true;
                        }
                    }
                }
            }
        }

        // Check for Node.js environment
        if !has_error && !request.skip_nodejs {
            if let Some(_nodejs_ver) = yaml.get("nodejs").and_then(|v| v.as_str()) {
                let package_file = pack_path_obj.join("package.json");

                if package_file.exists() {
                    // Check if Node.js is available
                    match Command::new("node").arg("--version").output() {
                        Ok(output) if output.status.success() => {
                            let version_str = String::from_utf8_lossy(&output.stdout);
                            let node_modules = pack_path_obj.join("node_modules");

                            // Check if node_modules exists or if force_rebuild is set
                            if !node_modules.exists() || request.force_rebuild {
                                tracing::info!(
                                    pack_ref = %pack_ref,
                                    "Node.js environment would be built here in production"
                                );
                            }

                            // Report environment status (detection mode)
                            nodejs_env = Some(crate::dto::pack::NodeJsEnvironment {
                                node_modules_path: node_modules.to_string_lossy().to_string(),
                                dependencies_installed: node_modules.exists(),
                                package_count: 0, // Would count from package.json in production
                                nodejs_version: version_str.trim().to_string(),
                            });
                            nodejs_envs_built += 1;
                        }
                        _ => {
                            failed_environments.push(crate::dto::pack::FailedEnvironment {
                                pack_ref: pack_ref.clone(),
                                pack_path: pack_path.clone(),
                                runtime: "nodejs".to_string(),
                                error: "Node.js not available in system".to_string(),
                            });
                            has_error = true;
                        }
                    }
                }
            }
        }

        if !has_error && (python_env.is_some() || nodejs_env.is_some()) {
            built_environments.push(crate::dto::pack::BuiltEnvironment {
                pack_ref,
                pack_path: pack_path.clone(),
                environments: crate::dto::pack::Environments {
                    python: python_env,
                    nodejs: nodejs_env,
                },
                duration_ms: pack_start.elapsed().as_millis() as u64,
            });
        }
    }

    let success_count = built_environments.len();
    let failure_count = failed_environments.len();

    let response = BuildPackEnvsResponse {
        built_environments,
        failed_environments,
        summary: crate::dto::pack::BuildSummary {
            total_packs: request.pack_paths.len(),
            success_count,
            failure_count,
            python_envs_built,
            nodejs_envs_built,
            total_duration_ms: start.elapsed().as_millis() as u64,
        },
    };

    Ok(Json(ApiResponse::new(response)))
}

/// Register multiple packs
#[utoipa::path(
    post,
    path = "/api/v1/packs/register-batch",
    tag = "packs",
    request_body = RegisterPacksRequest,
    responses(
        (status = 200, description = "Packs registered", body = ApiResponse<RegisterPacksResponse>),
        (status = 400, description = "Invalid request"),
    ),
    security(("bearer_auth" = []))
)]
pub async fn register_packs_batch(
    State(state): State<Arc<AppState>>,
    RequireAuth(user): RequireAuth,
    Json(request): Json<RegisterPacksRequest>,
) -> ApiResult<Json<ApiResponse<RegisterPacksResponse>>> {
    let start = std::time::Instant::now();
    let mut registered = Vec::new();
    let mut failed = Vec::new();
    let total_components = 0;

    for pack_path in &request.pack_paths {
        // Call the existing register_pack_internal function
        let register_req = crate::dto::pack::RegisterPackRequest {
            path: pack_path.clone(),
            force: request.force,
            skip_tests: request.skip_tests,
        };

        match register_pack_internal(
            state.clone(),
            user.claims.sub.clone(),
            register_req.path.clone(),
            register_req.force,
            register_req.skip_tests,
        )
        .await
        {
            Ok(pack_id) => {
                // Fetch pack details
                if let Ok(Some(pack)) = PackRepository::find_by_id(&state.db, pack_id).await {
                    // Count components (simplified)
                    registered.push(crate::dto::pack::RegisteredPack {
                        pack_ref: pack.r#ref.clone(),
                        pack_id,
                        pack_version: pack.version.clone(),
                        storage_path: format!("{}/{}", state.config.packs_base_dir, pack.r#ref),
                        components_registered: crate::dto::pack::ComponentCounts {
                            actions: 0,
                            sensors: 0,
                            triggers: 0,
                            rules: 0,
                            workflows: 0,
                            policies: 0,
                        },
                        test_result: None,
                        validation_results: crate::dto::pack::ValidationResults {
                            valid: true,
                            errors: Vec::new(),
                        },
                    });
                }
            }
            Err(e) => {
                failed.push(crate::dto::pack::FailedPackRegistration {
                    pack_ref: "unknown".to_string(),
                    pack_path: pack_path.clone(),
                    error: e.to_string(),
                    error_stage: "registration".to_string(),
                });
            }
        }
    }

    let response = RegisterPacksResponse {
        registered_packs: registered.clone(),
        failed_packs: failed.clone(),
        summary: crate::dto::pack::RegistrationSummary {
            total_packs: request.pack_paths.len(),
            success_count: registered.len(),
            failure_count: failed.len(),
            total_components,
            duration_ms: start.elapsed().as_millis() as u64,
        },
    };

    Ok(Json(ApiResponse::new(response)))
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/packs", get(list_packs).post(create_pack))
        .route("/packs/register", axum::routing::post(register_pack))
        .route(
            "/packs/register-batch",
            axum::routing::post(register_packs_batch),
        )
        .route("/packs/install", axum::routing::post(install_pack))
        .route("/packs/download", axum::routing::post(download_packs))
        .route(
            "/packs/dependencies",
            axum::routing::post(get_pack_dependencies),
        )
        .route("/packs/build-envs", axum::routing::post(build_pack_envs))
        .route(
            "/packs/{ref}",
            get(get_pack).put(update_pack).delete(delete_pack),
        )
        .route(
            "/packs/{ref}/workflows/sync",
            axum::routing::post(sync_pack_workflows),
        )
        .route(
            "/packs/{ref}/workflows/validate",
            axum::routing::post(validate_pack_workflows),
        )
        .route("/packs/{ref}/test", axum::routing::post(test_pack))
        .route("/packs/{ref}/tests", get(get_pack_test_history))
        .route("/packs/{ref}/tests/latest", get(get_pack_latest_test))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pack_routes_structure() {
        // Just verify the router can be constructed
        let _router = routes();
    }
}
