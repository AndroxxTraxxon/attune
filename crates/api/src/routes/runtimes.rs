//! Runtime management API routes

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
    runtime::{CreateRuntimeInput, RuntimeRepository, UpdateRuntimeInput},
    Create, Delete, FindByRef, List, Patch, Update,
};

use crate::{
    auth::middleware::RequireAuth,
    dto::{
        common::{PaginatedResponse, PaginationParams},
        runtime::{
            CreateRuntimeRequest, NullableJsonPatch, NullableStringPatch, RuntimeResponse,
            RuntimeSummary, UpdateRuntimeRequest,
        },
        ApiResponse, SuccessResponse,
    },
    middleware::{ApiError, ApiResult},
    state::AppState,
};

#[utoipa::path(
    get,
    path = "/api/v1/runtimes",
    tag = "runtimes",
    params(PaginationParams),
    responses(
        (status = 200, description = "List of runtimes", body = PaginatedResponse<RuntimeSummary>)
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_runtimes(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Query(pagination): Query<PaginationParams>,
) -> ApiResult<impl IntoResponse> {
    let all_runtimes = RuntimeRepository::list(&state.db).await?;
    let total = all_runtimes.len() as u64;
    let rows: Vec<_> = all_runtimes
        .into_iter()
        .skip(pagination.offset() as usize)
        .take(pagination.limit() as usize)
        .collect();

    let response = PaginatedResponse::new(
        rows.into_iter().map(RuntimeSummary::from).collect(),
        &pagination,
        total,
    );

    Ok((StatusCode::OK, Json(response)))
}

#[utoipa::path(
    get,
    path = "/api/v1/packs/{pack_ref}/runtimes",
    tag = "runtimes",
    params(
        ("pack_ref" = String, Path, description = "Pack reference identifier"),
        PaginationParams
    ),
    responses(
        (status = 200, description = "List of runtimes for a pack", body = PaginatedResponse<RuntimeSummary>),
        (status = 404, description = "Pack not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_runtimes_by_pack(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(pack_ref): Path<String>,
    Query(pagination): Query<PaginationParams>,
) -> ApiResult<impl IntoResponse> {
    let pack = PackRepository::find_by_ref(&state.db, &pack_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Pack '{}' not found", pack_ref)))?;

    let all_runtimes = RuntimeRepository::find_by_pack(&state.db, pack.id).await?;
    let total = all_runtimes.len() as u64;
    let rows: Vec<_> = all_runtimes
        .into_iter()
        .skip(pagination.offset() as usize)
        .take(pagination.limit() as usize)
        .collect();

    let response = PaginatedResponse::new(
        rows.into_iter().map(RuntimeSummary::from).collect(),
        &pagination,
        total,
    );

    Ok((StatusCode::OK, Json(response)))
}

#[utoipa::path(
    get,
    path = "/api/v1/runtimes/{ref}",
    tag = "runtimes",
    params(("ref" = String, Path, description = "Runtime reference identifier")),
    responses(
        (status = 200, description = "Runtime details", body = ApiResponse<RuntimeResponse>),
        (status = 404, description = "Runtime not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn get_runtime(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(runtime_ref): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let runtime = RuntimeRepository::find_by_ref(&state.db, &runtime_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Runtime '{}' not found", runtime_ref)))?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::new(RuntimeResponse::from(runtime))),
    ))
}

#[utoipa::path(
    post,
    path = "/api/v1/runtimes",
    tag = "runtimes",
    request_body = CreateRuntimeRequest,
    responses(
        (status = 201, description = "Runtime created successfully", body = ApiResponse<RuntimeResponse>),
        (status = 400, description = "Validation error"),
        (status = 404, description = "Pack not found"),
        (status = 409, description = "Runtime with same ref already exists")
    ),
    security(("bearer_auth" = []))
)]
pub async fn create_runtime(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Json(request): Json<CreateRuntimeRequest>,
) -> ApiResult<impl IntoResponse> {
    request.validate()?;

    if RuntimeRepository::find_by_ref(&state.db, &request.r#ref)
        .await?
        .is_some()
    {
        return Err(ApiError::Conflict(format!(
            "Runtime with ref '{}' already exists",
            request.r#ref
        )));
    }

    let (pack_id, pack_ref) = if let Some(ref pack_ref_str) = request.pack_ref {
        let pack = PackRepository::find_by_ref(&state.db, pack_ref_str)
            .await?
            .ok_or_else(|| ApiError::NotFound(format!("Pack '{}' not found", pack_ref_str)))?;
        (Some(pack.id), Some(pack.r#ref))
    } else {
        (None, None)
    };

    let runtime = RuntimeRepository::create(
        &state.db,
        CreateRuntimeInput {
            r#ref: request.r#ref,
            pack: pack_id,
            pack_ref,
            description: request.description,
            name: request.name,
            distributions: request.distributions,
            installation: request.installation,
            execution_config: request.execution_config,
        },
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::with_message(
            RuntimeResponse::from(runtime),
            "Runtime created successfully",
        )),
    ))
}

#[utoipa::path(
    put,
    path = "/api/v1/runtimes/{ref}",
    tag = "runtimes",
    params(("ref" = String, Path, description = "Runtime reference identifier")),
    request_body = UpdateRuntimeRequest,
    responses(
        (status = 200, description = "Runtime updated successfully", body = ApiResponse<RuntimeResponse>),
        (status = 400, description = "Validation error"),
        (status = 404, description = "Runtime not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn update_runtime(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(runtime_ref): Path<String>,
    Json(request): Json<UpdateRuntimeRequest>,
) -> ApiResult<impl IntoResponse> {
    request.validate()?;

    let existing_runtime = RuntimeRepository::find_by_ref(&state.db, &runtime_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Runtime '{}' not found", runtime_ref)))?;

    let runtime = RuntimeRepository::update(
        &state.db,
        existing_runtime.id,
        UpdateRuntimeInput {
            description: request.description.map(|patch| match patch {
                NullableStringPatch::Set(value) => Patch::Set(value),
                NullableStringPatch::Clear => Patch::Clear,
            }),
            name: request.name,
            distributions: request.distributions,
            installation: request.installation.map(|patch| match patch {
                NullableJsonPatch::Set(value) => Patch::Set(value),
                NullableJsonPatch::Clear => Patch::Clear,
            }),
            execution_config: request.execution_config,
        },
    )
    .await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::with_message(
            RuntimeResponse::from(runtime),
            "Runtime updated successfully",
        )),
    ))
}

#[utoipa::path(
    delete,
    path = "/api/v1/runtimes/{ref}",
    tag = "runtimes",
    params(("ref" = String, Path, description = "Runtime reference identifier")),
    responses(
        (status = 200, description = "Runtime deleted successfully", body = SuccessResponse),
        (status = 404, description = "Runtime not found")
    ),
    security(("bearer_auth" = []))
)]
pub async fn delete_runtime(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(runtime_ref): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let runtime = RuntimeRepository::find_by_ref(&state.db, &runtime_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Runtime '{}' not found", runtime_ref)))?;

    let deleted = RuntimeRepository::delete(&state.db, runtime.id).await?;
    if !deleted {
        return Err(ApiError::NotFound(format!(
            "Runtime '{}' not found",
            runtime_ref
        )));
    }

    Ok((
        StatusCode::OK,
        Json(SuccessResponse::new(format!(
            "Runtime '{}' deleted successfully",
            runtime_ref
        ))),
    ))
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/runtimes", get(list_runtimes).post(create_runtime))
        .route(
            "/runtimes/{ref}",
            get(get_runtime).put(update_runtime).delete(delete_runtime),
        )
        .route("/packs/{pack_ref}/runtimes", get(list_runtimes_by_pack))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_runtime_routes_structure() {
        let _router = routes();
    }
}
