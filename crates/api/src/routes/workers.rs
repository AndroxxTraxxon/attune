use std::{collections::HashMap, sync::Arc};

use attune_common::{
    models::WorkerStatus,
    rbac::{Action, AuthorizationContext, Resource},
};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};

use attune_common::repositories::{
    execution::ExecutionRepository, runtime::WorkerRepository, List,
};

use crate::{
    auth::middleware::RequireAuth,
    authz::{AuthorizationCheck, AuthorizationService},
    dto::{
        common::PaginatedResponse, worker::runtime_support_from_capabilities, WorkerLoadSnapshot,
        WorkerQueryParams, WorkerSummary,
    },
    middleware::ApiResult,
    state::AppState,
};

#[utoipa::path(
    get,
    path = "/api/v1/workers",
    tag = "workers",
    params(WorkerQueryParams),
    responses(
        (status = 200, description = "List workers with runtime support and current load", body = PaginatedResponse<WorkerSummary>)
    ),
    security(("bearer_auth" = []))
)]
pub async fn list_workers(
    State(state): State<Arc<AppState>>,
    RequireAuth(user): RequireAuth,
    Query(query): Query<WorkerQueryParams>,
) -> ApiResult<impl IntoResponse> {
    let identity_id = user.identity_id().map_err(|_| {
        crate::middleware::ApiError::Unauthorized("Invalid user identity".to_string())
    })?;
    AuthorizationService::new(state.db.clone())
        .authorize(
            &user,
            AuthorizationCheck {
                resource: Resource::Workers,
                action: Action::Read,
                context: AuthorizationContext::new(identity_id),
            },
        )
        .await?;

    fn capability_u64(capabilities: Option<&serde_json::Value>, key: &str) -> Option<u64> {
        capabilities
            .and_then(|capabilities| capabilities.get(key))
            .and_then(|value| value.as_u64())
    }

    fn capability_u32(capabilities: Option<&serde_json::Value>, key: &str) -> Option<u32> {
        capability_u64(capabilities, key).and_then(|value| u32::try_from(value).ok())
    }

    fn derive_worker_status(
        status: Option<WorkerStatus>,
        total_active: u64,
        sensor_processes_running: Option<u64>,
        active_rules: Option<u64>,
    ) -> Option<WorkerStatus> {
        match status {
            Some(WorkerStatus::Inactive | WorkerStatus::Error | WorkerStatus::Busy) => status,
            Some(WorkerStatus::Active)
                if total_active > 0
                    || sensor_processes_running.unwrap_or(0) > 0
                    || active_rules.unwrap_or(0) > 0 =>
            {
                Some(WorkerStatus::Busy)
            }
            _ => status,
        }
    }

    let workers = WorkerRepository::list(&state.db).await?;
    let total = workers.len() as u64;
    let start = query.offset() as usize;
    let end = (start + query.limit() as usize).min(workers.len());
    let page_items = if start >= workers.len() {
        Vec::new()
    } else {
        workers[start..end].to_vec()
    };

    let worker_ids = page_items
        .iter()
        .map(|worker| worker.id)
        .collect::<Vec<_>>();
    let load_by_worker = ExecutionRepository::current_load_by_worker_ids(&state.db, &worker_ids)
        .await?
        .into_iter()
        .map(|load| (load.worker_id, load))
        .collect::<HashMap<_, _>>();

    let summaries = page_items
        .into_iter()
        .map(|worker| {
            let capabilities = worker.capabilities.as_ref();
            let max_concurrent_executions =
                capability_u32(capabilities, "max_concurrent_executions");
            let max_concurrent_sensors = capability_u32(capabilities, "max_concurrent_sensors");
            let sensor_processes_monitored =
                capability_u64(capabilities, "sensor_processes_monitored");
            let sensor_processes_running = capability_u64(capabilities, "sensor_processes_running");
            let active_rules = capability_u64(capabilities, "active_rules");
            let queue_depth = worker
                .capabilities
                .as_ref()
                .and_then(|capabilities| capabilities.get("health"))
                .and_then(|health| health.get("queue_depth"))
                .and_then(|value| value.as_i64())
                .and_then(|value| i32::try_from(value).ok());
            let load = load_by_worker.get(&worker.id);
            let total_active = load
                .map(|value| value.total_active.max(0) as u64)
                .unwrap_or(0);
            let utilization_percent = max_concurrent_executions
                .filter(|value| *value > 0)
                .map(|max| ((total_active as f64 / max as f64) * 100.0).round() as u32)
                .or_else(|| {
                    max_concurrent_sensors
                        .filter(|value| *value > 0)
                        .zip(sensor_processes_running)
                        .map(|(max, running)| {
                            ((running as f64 / max as f64) * 100.0).round() as u32
                        })
                });
            let status = derive_worker_status(
                worker.status,
                total_active,
                sensor_processes_running,
                active_rules,
            );

            WorkerSummary {
                id: worker.id,
                name: worker.name,
                worker_type: worker.worker_type,
                worker_role: worker.worker_role,
                host: worker.host,
                port: worker.port,
                status,
                last_heartbeat: worker.last_heartbeat,
                supported_runtimes: runtime_support_from_capabilities(worker.capabilities.as_ref()),
                load: WorkerLoadSnapshot {
                    requested: load.map(|value| value.requested.max(0) as u64).unwrap_or(0),
                    scheduling: load
                        .map(|value| value.scheduling.max(0) as u64)
                        .unwrap_or(0),
                    scheduled: load.map(|value| value.scheduled.max(0) as u64).unwrap_or(0),
                    running: load.map(|value| value.running.max(0) as u64).unwrap_or(0),
                    canceling: load.map(|value| value.canceling.max(0) as u64).unwrap_or(0),
                    total_active,
                    max_concurrent_executions,
                    utilization_percent,
                    queue_depth,
                    sensor_processes_monitored,
                    sensor_processes_running,
                    active_rules,
                    max_concurrent_sensors,
                },
                created: worker.created,
                updated: worker.updated,
            }
        })
        .collect::<Vec<_>>();

    Ok((
        StatusCode::OK,
        Json(PaginatedResponse::new(
            summaries,
            &query.pagination(),
            total,
        )),
    ))
}

pub fn routes() -> Router<Arc<AppState>> {
    Router::new().route("/workers", get(list_workers))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_worker_routes_structure() {
        let _router = routes();
    }
}
