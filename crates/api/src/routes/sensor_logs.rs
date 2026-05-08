//! Sensor Log Endpoints
//!
//! Provides read access to per-sensor rotating log files (stdout/stderr)
//! via the standard artifact system.
//!
//! Sensor log artifacts are auto-registered by the sensor service with refs
//! in the format `sensor.{sensor_ref}.stdout` / `sensor.{sensor_ref}.stderr`.
//! These endpoints resolve a sensor ref to the matching artifact and delegate
//! to the existing artifact download/stream infrastructure.

use std::sync::Arc;

use attune_common::repositories::{artifact::ArtifactRepository, FindByRef};
use axum::{
    extract::{Path, State},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Serialize;
use tracing::debug;

use crate::{
    auth::middleware::RequireAuth,
    middleware::{ApiError, ApiResult},
    state::AppState,
};

/// Summary of a sensor's available log artifacts.
#[derive(Serialize)]
struct SensorLogSummary {
    sensor_ref: String,
    logs: Vec<SensorLogEntry>,
}

#[derive(Serialize)]
struct SensorLogEntry {
    stream: String,
    artifact_ref: String,
    artifact_id: Option<i64>,
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/v1/sensors/{sensor_ref}/logs", get(list_sensor_logs))
        .route(
            "/api/v1/sensors/{sensor_ref}/logs/{stream}",
            get(get_sensor_log),
        )
}

/// List available log streams for a sensor.
#[utoipa::path(
    get,
    path = "/api/v1/sensors/{sensor_ref}/logs",
    params(
        ("sensor_ref" = String, Path, description = "Sensor reference (e.g., core.timer)")
    ),
    responses(
        (status = 200, description = "Sensor log summary"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer_auth" = []))
)]
async fn list_sensor_logs(
    RequireAuth(_user): RequireAuth,
    State(state): State<Arc<AppState>>,
    Path(sensor_ref): Path<String>,
) -> ApiResult<Json<SensorLogSummary>> {
    let mut logs = Vec::new();

    for stream in &["stdout", "stderr"] {
        let artifact_ref = format!("sensor.{}.{}", sensor_ref, stream);
        let artifact_id = match ArtifactRepository::find_by_ref(&state.db, &artifact_ref).await {
            Ok(Some(a)) => Some(a.id),
            _ => None,
        };

        logs.push(SensorLogEntry {
            stream: stream.to_string(),
            artifact_ref,
            artifact_id,
        });
    }

    Ok(Json(SensorLogSummary { sensor_ref, logs }))
}

/// Download a specific sensor log stream.
///
/// Resolves the sensor ref + stream to a log file on disk and serves
/// the content as plain text.
#[utoipa::path(
    get,
    path = "/api/v1/sensors/{sensor_ref}/logs/{stream}",
    params(
        ("sensor_ref" = String, Path, description = "Sensor reference (e.g., core.timer)"),
        ("stream" = String, Path, description = "Log stream: stdout or stderr")
    ),
    responses(
        (status = 200, description = "Log file content", content_type = "text/plain"),
        (status = 404, description = "Sensor log not found"),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer_auth" = []))
)]
async fn get_sensor_log(
    RequireAuth(_user): RequireAuth,
    State(state): State<Arc<AppState>>,
    Path((sensor_ref, stream)): Path<(String, String)>,
) -> ApiResult<impl IntoResponse> {
    if stream != "stdout" && stream != "stderr" {
        return Err(ApiError::ValidationError(
            "stream must be 'stdout' or 'stderr'".into(),
        ));
    }

    let artifact_ref = format!("sensor.{}.{}", sensor_ref, stream);

    // Verify artifact exists in DB
    let artifact = ArtifactRepository::find_by_ref(&state.db, &artifact_ref)
        .await
        .map_err(|e| ApiError::DatabaseError(format!("DB error: {}", e)))?
        .ok_or_else(|| ApiError::NotFound(format!("Sensor log '{}' not found", artifact_ref)))?;

    debug!(
        "Resolved sensor log '{}' to artifact id={}",
        artifact_ref, artifact.id
    );

    let log_path = std::path::Path::new(&state.config.artifacts_dir)
        .join("sensors")
        .join(&sensor_ref)
        .join(format!("{}.log", stream));

    let content = tokio::fs::read_to_string(&log_path).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ApiError::NotFound(format!(
                "Sensor log file not found at '{}'",
                log_path.display()
            ))
        } else {
            ApiError::InternalServerError(format!("Failed to read sensor log: {}", e))
        }
    })?;

    Ok((
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; charset=utf-8",
        )],
        content,
    ))
}
