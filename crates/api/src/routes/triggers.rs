//! Trigger and Sensor management API routes

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use std::sync::Arc;
use validator::Validate;

use attune_common::repositories::{
    pack::PackRepository,
    runtime::RuntimeRepository,
    trigger::{
        CreateSensorInput, CreateTriggerInput, SensorRepository, SensorSearchFilters,
        TriggerRepository, TriggerSearchFilters, UpdateSensorInput, UpdateTriggerInput,
    },
    Create, Delete, FindByRef, Update,
};

use crate::{
    auth::middleware::RequireAuth,
    dto::{
        common::{PaginatedResponse, PaginationParams},
        trigger::{
            CreateSensorRequest, CreateTriggerRequest, SensorResponse, SensorSummary,
            TriggerResponse, TriggerSummary, UpdateSensorRequest, UpdateTriggerRequest,
        },
        ApiResponse, SuccessResponse,
    },
    middleware::{ApiError, ApiResult},
    state::AppState,
};

// ============================================================================
// TRIGGER ENDPOINTS
// ============================================================================

/// List all triggers with pagination
#[utoipa::path(
    get,
    path = "/api/v1/triggers",
    tag = "triggers",
    params(PaginationParams),
    responses(
        (status = 200, description = "List of triggers", body = PaginatedResponse<TriggerSummary>),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn list_triggers(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Query(pagination): Query<PaginationParams>,
) -> ApiResult<impl IntoResponse> {
    let filters = TriggerSearchFilters {
        pack: None,
        enabled: None,
        limit: pagination.limit(),
        offset: pagination.offset(),
    };

    let result = TriggerRepository::list_search(&state.db, &filters).await?;

    let paginated_triggers: Vec<TriggerSummary> =
        result.rows.into_iter().map(TriggerSummary::from).collect();

    let response = PaginatedResponse::new(paginated_triggers, &pagination, result.total);

    Ok((StatusCode::OK, Json(response)))
}

/// List enabled triggers
#[utoipa::path(
    get,
    path = "/api/v1/triggers/enabled",
    tag = "triggers",
    params(PaginationParams),
    responses(
        (status = 200, description = "List of enabled triggers", body = PaginatedResponse<TriggerSummary>),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn list_enabled_triggers(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Query(pagination): Query<PaginationParams>,
) -> ApiResult<impl IntoResponse> {
    let filters = TriggerSearchFilters {
        pack: None,
        enabled: Some(true),
        limit: pagination.limit(),
        offset: pagination.offset(),
    };

    let result = TriggerRepository::list_search(&state.db, &filters).await?;

    let paginated_triggers: Vec<TriggerSummary> =
        result.rows.into_iter().map(TriggerSummary::from).collect();

    let response = PaginatedResponse::new(paginated_triggers, &pagination, result.total);

    Ok((StatusCode::OK, Json(response)))
}

/// List triggers by pack reference
#[utoipa::path(
    get,
    path = "/api/v1/packs/{pack_ref}/triggers",
    tag = "triggers",
    params(
        ("pack_ref" = String, Path, description = "Pack reference"),
        PaginationParams
    ),
    responses(
        (status = 200, description = "List of triggers in pack", body = PaginatedResponse<TriggerSummary>),
        (status = 404, description = "Pack not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn list_triggers_by_pack(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(pack_ref): Path<String>,
    Query(pagination): Query<PaginationParams>,
) -> ApiResult<impl IntoResponse> {
    // Verify pack exists
    let pack = PackRepository::find_by_ref(&state.db, &pack_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Pack '{}' not found", pack_ref)))?;

    let filters = TriggerSearchFilters {
        pack: Some(pack.id),
        enabled: None,
        limit: pagination.limit(),
        offset: pagination.offset(),
    };

    let result = TriggerRepository::list_search(&state.db, &filters).await?;

    let paginated_triggers: Vec<TriggerSummary> =
        result.rows.into_iter().map(TriggerSummary::from).collect();

    let response = PaginatedResponse::new(paginated_triggers, &pagination, result.total);

    Ok((StatusCode::OK, Json(response)))
}

/// Get a single trigger by reference
#[utoipa::path(
    get,
    path = "/api/v1/triggers/{ref}",
    tag = "triggers",
    params(
        ("ref" = String, Path, description = "Trigger reference")
    ),
    responses(
        (status = 200, description = "Trigger details", body = ApiResponse<TriggerResponse>),
        (status = 404, description = "Trigger not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_trigger(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(trigger_ref): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let trigger = TriggerRepository::find_by_ref(&state.db, &trigger_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Trigger '{}' not found", trigger_ref)))?;

    let response = ApiResponse::new(TriggerResponse::from(trigger));

    Ok((StatusCode::OK, Json(response)))
}

/// Create a new trigger
#[utoipa::path(
    post,
    path = "/api/v1/triggers",
    tag = "triggers",
    request_body = CreateTriggerRequest,
    responses(
        (status = 201, description = "Trigger created successfully", body = ApiResponse<TriggerResponse>),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Pack not found"),
        (status = 409, description = "Trigger with same ref already exists"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn create_trigger(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Json(request): Json<CreateTriggerRequest>,
) -> ApiResult<impl IntoResponse> {
    // Validate request
    request.validate()?;

    // Check if trigger with same ref already exists
    if TriggerRepository::find_by_ref(&state.db, &request.r#ref)
        .await?
        .is_some()
    {
        return Err(ApiError::Conflict(format!(
            "Trigger with ref '{}' already exists",
            request.r#ref
        )));
    }

    // If pack_ref is provided, verify pack exists and get its ID
    let (pack_id, pack_ref) = if let Some(ref pack_ref_str) = request.pack_ref {
        let pack = PackRepository::find_by_ref(&state.db, pack_ref_str)
            .await?
            .ok_or_else(|| ApiError::NotFound(format!("Pack '{}' not found", pack_ref_str)))?;
        (Some(pack.id), Some(pack.r#ref.clone()))
    } else {
        (None, None)
    };

    // Create trigger input
    let trigger_input = CreateTriggerInput {
        r#ref: request.r#ref,
        pack: pack_id,
        pack_ref,
        label: request.label,
        description: request.description,
        enabled: request.enabled,
        param_schema: request.param_schema,
        out_schema: request.out_schema,
        is_adhoc: true, // Triggers created via API are ad-hoc (not from pack installation)
    };

    let trigger = TriggerRepository::create(&state.db, trigger_input).await?;

    let response = ApiResponse::with_message(
        TriggerResponse::from(trigger),
        "Trigger created successfully",
    );

    Ok((StatusCode::CREATED, Json(response)))
}

/// Update an existing trigger
#[utoipa::path(
    put,
    path = "/api/v1/triggers/{ref}",
    tag = "triggers",
    params(
        ("ref" = String, Path, description = "Trigger reference")
    ),
    request_body = UpdateTriggerRequest,
    responses(
        (status = 200, description = "Trigger updated successfully", body = ApiResponse<TriggerResponse>),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Trigger not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn update_trigger(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(trigger_ref): Path<String>,
    Json(request): Json<UpdateTriggerRequest>,
) -> ApiResult<impl IntoResponse> {
    // Validate request
    request.validate()?;

    // Check if trigger exists
    let existing_trigger = TriggerRepository::find_by_ref(&state.db, &trigger_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Trigger '{}' not found", trigger_ref)))?;

    // Create update input
    let update_input = UpdateTriggerInput {
        label: request.label,
        description: request.description,
        enabled: request.enabled,
        param_schema: request.param_schema,
        out_schema: request.out_schema,
    };

    let trigger = TriggerRepository::update(&state.db, existing_trigger.id, update_input).await?;

    let response = ApiResponse::with_message(
        TriggerResponse::from(trigger),
        "Trigger updated successfully",
    );

    Ok((StatusCode::OK, Json(response)))
}

/// Delete a trigger
#[utoipa::path(
    delete,
    path = "/api/v1/triggers/{ref}",
    tag = "triggers",
    params(
        ("ref" = String, Path, description = "Trigger reference")
    ),
    responses(
        (status = 200, description = "Trigger deleted successfully", body = SuccessResponse),
        (status = 404, description = "Trigger not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn delete_trigger(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(trigger_ref): Path<String>,
) -> ApiResult<impl IntoResponse> {
    // Check if trigger exists
    let trigger = TriggerRepository::find_by_ref(&state.db, &trigger_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Trigger '{}' not found", trigger_ref)))?;

    // Delete the trigger
    let deleted = TriggerRepository::delete(&state.db, trigger.id).await?;

    if !deleted {
        return Err(ApiError::NotFound(format!(
            "Trigger '{}' not found",
            trigger_ref
        )));
    }

    let response = SuccessResponse::new(format!("Trigger '{}' deleted successfully", trigger_ref));

    Ok((StatusCode::OK, Json(response)))
}

/// Enable a trigger
#[utoipa::path(
    post,
    path = "/api/v1/triggers/{ref}/enable",
    tag = "triggers",
    params(
        ("ref" = String, Path, description = "Trigger reference")
    ),
    responses(
        (status = 200, description = "Trigger enabled successfully", body = ApiResponse<TriggerResponse>),
        (status = 404, description = "Trigger not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn enable_trigger(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(trigger_ref): Path<String>,
) -> ApiResult<impl IntoResponse> {
    // Check if trigger exists
    let existing_trigger = TriggerRepository::find_by_ref(&state.db, &trigger_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Trigger '{}' not found", trigger_ref)))?;

    // Update trigger to enabled
    let update_input = UpdateTriggerInput {
        label: None,
        description: None,
        enabled: Some(true),
        param_schema: None,
        out_schema: None,
    };

    let trigger = TriggerRepository::update(&state.db, existing_trigger.id, update_input).await?;

    let response = ApiResponse::with_message(
        TriggerResponse::from(trigger),
        "Trigger enabled successfully",
    );

    Ok((StatusCode::OK, Json(response)))
}

/// Disable a trigger
#[utoipa::path(
    post,
    path = "/api/v1/triggers/{ref}/disable",
    tag = "triggers",
    params(
        ("ref" = String, Path, description = "Trigger reference")
    ),
    responses(
        (status = 200, description = "Trigger disabled successfully", body = ApiResponse<TriggerResponse>),
        (status = 404, description = "Trigger not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn disable_trigger(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(trigger_ref): Path<String>,
) -> ApiResult<impl IntoResponse> {
    // Check if trigger exists
    let existing_trigger = TriggerRepository::find_by_ref(&state.db, &trigger_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Trigger '{}' not found", trigger_ref)))?;

    // Update trigger to disabled
    let update_input = UpdateTriggerInput {
        label: None,
        description: None,
        enabled: Some(false),
        param_schema: None,
        out_schema: None,
    };

    let trigger = TriggerRepository::update(&state.db, existing_trigger.id, update_input).await?;

    let response = ApiResponse::with_message(
        TriggerResponse::from(trigger),
        "Trigger disabled successfully",
    );

    Ok((StatusCode::OK, Json(response)))
}

// ============================================================================
// SENSOR ENDPOINTS
// ============================================================================

/// List all sensors with pagination
#[utoipa::path(
    get,
    path = "/api/v1/sensors",
    tag = "sensors",
    params(PaginationParams),
    responses(
        (status = 200, description = "List of sensors", body = PaginatedResponse<SensorSummary>),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn list_sensors(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Query(pagination): Query<PaginationParams>,
) -> ApiResult<impl IntoResponse> {
    let filters = SensorSearchFilters {
        pack: None,
        trigger: None,
        enabled: None,
        limit: pagination.limit(),
        offset: pagination.offset(),
    };

    let result = SensorRepository::list_search(&state.db, &filters).await?;

    let paginated_sensors: Vec<SensorSummary> =
        result.rows.into_iter().map(SensorSummary::from).collect();

    let response = PaginatedResponse::new(paginated_sensors, &pagination, result.total);

    Ok((StatusCode::OK, Json(response)))
}

/// List enabled sensors
#[utoipa::path(
    get,
    path = "/api/v1/sensors/enabled",
    tag = "sensors",
    params(PaginationParams),
    responses(
        (status = 200, description = "List of enabled sensors", body = PaginatedResponse<SensorSummary>),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn list_enabled_sensors(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Query(pagination): Query<PaginationParams>,
) -> ApiResult<impl IntoResponse> {
    let filters = SensorSearchFilters {
        pack: None,
        trigger: None,
        enabled: Some(true),
        limit: pagination.limit(),
        offset: pagination.offset(),
    };

    let result = SensorRepository::list_search(&state.db, &filters).await?;

    let paginated_sensors: Vec<SensorSummary> =
        result.rows.into_iter().map(SensorSummary::from).collect();

    let response = PaginatedResponse::new(paginated_sensors, &pagination, result.total);

    Ok((StatusCode::OK, Json(response)))
}

/// List sensors by pack reference
#[utoipa::path(
    get,
    path = "/api/v1/packs/{pack_ref}/sensors",
    tag = "sensors",
    params(
        ("pack_ref" = String, Path, description = "Pack reference"),
        PaginationParams
    ),
    responses(
        (status = 200, description = "List of sensors in pack", body = PaginatedResponse<SensorSummary>),
        (status = 404, description = "Pack not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn list_sensors_by_pack(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(pack_ref): Path<String>,
    Query(pagination): Query<PaginationParams>,
) -> ApiResult<impl IntoResponse> {
    // Verify pack exists
    let pack = PackRepository::find_by_ref(&state.db, &pack_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Pack '{}' not found", pack_ref)))?;

    let filters = SensorSearchFilters {
        pack: Some(pack.id),
        trigger: None,
        enabled: None,
        limit: pagination.limit(),
        offset: pagination.offset(),
    };

    let result = SensorRepository::list_search(&state.db, &filters).await?;

    let paginated_sensors: Vec<SensorSummary> =
        result.rows.into_iter().map(SensorSummary::from).collect();

    let response = PaginatedResponse::new(paginated_sensors, &pagination, result.total);

    Ok((StatusCode::OK, Json(response)))
}

/// List sensors by trigger reference
#[utoipa::path(
    get,
    path = "/api/v1/triggers/{trigger_ref}/sensors",
    tag = "sensors",
    params(
        ("trigger_ref" = String, Path, description = "Trigger reference"),
        PaginationParams
    ),
    responses(
        (status = 200, description = "List of sensors for trigger", body = PaginatedResponse<SensorSummary>),
        (status = 404, description = "Trigger not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn list_sensors_by_trigger(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(trigger_ref): Path<String>,
    Query(pagination): Query<PaginationParams>,
) -> ApiResult<impl IntoResponse> {
    // Verify trigger exists
    let trigger = TriggerRepository::find_by_ref(&state.db, &trigger_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Trigger '{}' not found", trigger_ref)))?;

    let filters = SensorSearchFilters {
        pack: None,
        trigger: Some(trigger.id),
        enabled: None,
        limit: pagination.limit(),
        offset: pagination.offset(),
    };

    let result = SensorRepository::list_search(&state.db, &filters).await?;

    let paginated_sensors: Vec<SensorSummary> =
        result.rows.into_iter().map(SensorSummary::from).collect();

    let response = PaginatedResponse::new(paginated_sensors, &pagination, result.total);

    Ok((StatusCode::OK, Json(response)))
}

/// Get a single sensor by reference
#[utoipa::path(
    get,
    path = "/api/v1/sensors/{ref}",
    tag = "sensors",
    params(
        ("ref" = String, Path, description = "Sensor reference")
    ),
    responses(
        (status = 200, description = "Sensor details", body = ApiResponse<SensorResponse>),
        (status = 404, description = "Sensor not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_sensor(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(sensor_ref): Path<String>,
) -> ApiResult<impl IntoResponse> {
    let sensor = SensorRepository::find_by_ref(&state.db, &sensor_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Sensor '{}' not found", sensor_ref)))?;

    let response = ApiResponse::new(SensorResponse::from(sensor));

    Ok((StatusCode::OK, Json(response)))
}

/// Create a new sensor
#[utoipa::path(
    post,
    path = "/api/v1/sensors",
    tag = "sensors",
    request_body = CreateSensorRequest,
    responses(
        (status = 201, description = "Sensor created successfully", body = ApiResponse<SensorResponse>),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Pack, runtime, or trigger not found"),
        (status = 409, description = "Sensor with same ref already exists"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn create_sensor(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Json(request): Json<CreateSensorRequest>,
) -> ApiResult<impl IntoResponse> {
    // Validate request
    request.validate()?;

    // Check if sensor with same ref already exists
    if SensorRepository::find_by_ref(&state.db, &request.r#ref)
        .await?
        .is_some()
    {
        return Err(ApiError::Conflict(format!(
            "Sensor with ref '{}' already exists",
            request.r#ref
        )));
    }

    // Verify pack exists and get its ID
    let pack = PackRepository::find_by_ref(&state.db, &request.pack_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Pack '{}' not found", request.pack_ref)))?;

    // Verify runtime exists and get its ID
    let runtime = RuntimeRepository::find_by_ref(&state.db, &request.runtime_ref)
        .await?
        .ok_or_else(|| {
            ApiError::NotFound(format!("Runtime '{}' not found", request.runtime_ref))
        })?;

    // Verify trigger exists and get its ID
    let trigger = TriggerRepository::find_by_ref(&state.db, &request.trigger_ref)
        .await?
        .ok_or_else(|| {
            ApiError::NotFound(format!("Trigger '{}' not found", request.trigger_ref))
        })?;

    // Create sensor input
    let sensor_input = CreateSensorInput {
        r#ref: request.r#ref,
        pack: Some(pack.id),
        pack_ref: Some(pack.r#ref.clone()),
        label: request.label,
        description: request.description,
        entrypoint: request.entrypoint,
        runtime: runtime.id,
        runtime_ref: runtime.r#ref.clone(),
        runtime_version_constraint: None,
        trigger: trigger.id,
        trigger_ref: trigger.r#ref.clone(),
        enabled: request.enabled,
        param_schema: request.param_schema,
        config: request.config,
    };

    let sensor = SensorRepository::create(&state.db, sensor_input).await?;

    let response =
        ApiResponse::with_message(SensorResponse::from(sensor), "Sensor created successfully");

    Ok((StatusCode::CREATED, Json(response)))
}

/// Update an existing sensor
#[utoipa::path(
    put,
    path = "/api/v1/sensors/{ref}",
    tag = "sensors",
    params(
        ("ref" = String, Path, description = "Sensor reference")
    ),
    request_body = UpdateSensorRequest,
    responses(
        (status = 200, description = "Sensor updated successfully", body = ApiResponse<SensorResponse>),
        (status = 400, description = "Invalid request"),
        (status = 404, description = "Sensor not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn update_sensor(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(sensor_ref): Path<String>,
    Json(request): Json<UpdateSensorRequest>,
) -> ApiResult<impl IntoResponse> {
    // Validate request
    request.validate()?;

    // Check if sensor exists
    let existing_sensor = SensorRepository::find_by_ref(&state.db, &sensor_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Sensor '{}' not found", sensor_ref)))?;

    // Create update input
    let update_input = UpdateSensorInput {
        label: request.label,
        description: request.description,
        entrypoint: request.entrypoint,
        runtime: None,
        runtime_ref: None,
        runtime_version_constraint: None,
        trigger: None,
        trigger_ref: None,
        enabled: request.enabled,
        param_schema: request.param_schema,
        config: None,
    };

    let sensor = SensorRepository::update(&state.db, existing_sensor.id, update_input).await?;

    let response =
        ApiResponse::with_message(SensorResponse::from(sensor), "Sensor updated successfully");

    Ok((StatusCode::OK, Json(response)))
}

/// Delete a sensor
#[utoipa::path(
    delete,
    path = "/api/v1/sensors/{ref}",
    tag = "sensors",
    params(
        ("ref" = String, Path, description = "Sensor reference")
    ),
    responses(
        (status = 200, description = "Sensor deleted successfully", body = SuccessResponse),
        (status = 404, description = "Sensor not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn delete_sensor(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(sensor_ref): Path<String>,
) -> ApiResult<impl IntoResponse> {
    // Check if sensor exists
    let sensor = SensorRepository::find_by_ref(&state.db, &sensor_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Sensor '{}' not found", sensor_ref)))?;

    // Delete the sensor
    let deleted = SensorRepository::delete(&state.db, sensor.id).await?;

    if !deleted {
        return Err(ApiError::NotFound(format!(
            "Sensor '{}' not found",
            sensor_ref
        )));
    }

    let response = SuccessResponse::new(format!("Sensor '{}' deleted successfully", sensor_ref));

    Ok((StatusCode::OK, Json(response)))
}

/// Enable a sensor
#[utoipa::path(
    post,
    path = "/api/v1/sensors/{ref}/enable",
    tag = "sensors",
    params(
        ("ref" = String, Path, description = "Sensor reference")
    ),
    responses(
        (status = 200, description = "Sensor enabled successfully", body = ApiResponse<SensorResponse>),
        (status = 404, description = "Sensor not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn enable_sensor(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(sensor_ref): Path<String>,
) -> ApiResult<impl IntoResponse> {
    // Check if sensor exists
    let existing_sensor = SensorRepository::find_by_ref(&state.db, &sensor_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Sensor '{}' not found", sensor_ref)))?;

    // Update sensor to enabled
    let update_input = UpdateSensorInput {
        label: None,
        description: None,
        entrypoint: None,
        runtime: None,
        runtime_ref: None,
        runtime_version_constraint: None,
        trigger: None,
        trigger_ref: None,
        enabled: Some(true),
        param_schema: None,
        config: None,
    };

    let sensor = SensorRepository::update(&state.db, existing_sensor.id, update_input).await?;

    let response =
        ApiResponse::with_message(SensorResponse::from(sensor), "Sensor enabled successfully");

    Ok((StatusCode::OK, Json(response)))
}

/// Disable a sensor
#[utoipa::path(
    post,
    path = "/api/v1/sensors/{ref}/disable",
    tag = "sensors",
    params(
        ("ref" = String, Path, description = "Sensor reference")
    ),
    responses(
        (status = 200, description = "Sensor disabled successfully", body = ApiResponse<SensorResponse>),
        (status = 404, description = "Sensor not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn disable_sensor(
    State(state): State<Arc<AppState>>,
    RequireAuth(_user): RequireAuth,
    Path(sensor_ref): Path<String>,
) -> ApiResult<impl IntoResponse> {
    // Check if sensor exists
    let existing_sensor = SensorRepository::find_by_ref(&state.db, &sensor_ref)
        .await?
        .ok_or_else(|| ApiError::NotFound(format!("Sensor '{}' not found", sensor_ref)))?;

    // Update sensor to disabled
    let update_input = UpdateSensorInput {
        label: None,
        description: None,
        entrypoint: None,
        runtime: None,
        runtime_ref: None,
        runtime_version_constraint: None,
        trigger: None,
        trigger_ref: None,
        enabled: Some(false),
        param_schema: None,
        config: None,
    };

    let sensor = SensorRepository::update(&state.db, existing_sensor.id, update_input).await?;

    let response =
        ApiResponse::with_message(SensorResponse::from(sensor), "Sensor disabled successfully");

    Ok((StatusCode::OK, Json(response)))
}

/// Create trigger and sensor routes
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        // Trigger routes
        .route("/triggers", get(list_triggers).post(create_trigger))
        .route("/triggers/enabled", get(list_enabled_triggers))
        .route(
            "/triggers/{ref}",
            get(get_trigger).put(update_trigger).delete(delete_trigger),
        )
        .route("/triggers/{ref}/enable", post(enable_trigger))
        .route("/triggers/{ref}/disable", post(disable_trigger))
        .route("/packs/{pack_ref}/triggers", get(list_triggers_by_pack))
        // Sensor routes
        .route("/sensors", get(list_sensors).post(create_sensor))
        .route("/sensors/enabled", get(list_enabled_sensors))
        .route(
            "/sensors/{ref}",
            get(get_sensor).put(update_sensor).delete(delete_sensor),
        )
        .route("/sensors/{ref}/enable", post(enable_sensor))
        .route("/sensors/{ref}/disable", post(disable_sensor))
        .route("/packs/{pack_ref}/sensors", get(list_sensors_by_pack))
        .route(
            "/triggers/{trigger_ref}/sensors",
            get(list_sensors_by_trigger),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trigger_sensor_routes_structure() {
        // Just verify the router can be constructed
        let _router = routes();
    }
}
