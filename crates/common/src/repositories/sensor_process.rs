//! Sensor process repository for durable supervisor state.

use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::{Executor, Postgres};

use crate::models::{Id, JsonDict, SensorProcess, SensorProcessStatus};
use crate::{Error, Result};

use super::{FindById, List, Repository};

pub struct SensorProcessRepository;

impl Repository for SensorProcessRepository {
    type Entity = SensorProcess;

    fn table_name() -> &'static str {
        "sensor_process"
    }
}

pub const SELECT_COLUMNS: &str = "id, sensor, sensor_ref, worker, worker_name, status, pid, \
     consecutive_failures, last_exit_code, last_signal, last_started_at, last_stopped_at, \
     next_restart_at, stderr_excerpt, log_artifact_ref, active_rule_count, \
     last_alerted_failure_count, last_alerted_at, meta, created, updated";

#[derive(Debug, Clone)]
pub struct UpsertSensorProcessStartInput {
    pub sensor: Id,
    pub sensor_ref: String,
    pub worker: Id,
    pub worker_name: String,
    pub status: SensorProcessStatus,
    pub pid: Option<i32>,
    pub started_at: Option<DateTime<Utc>>,
    pub active_rule_count: i32,
    pub log_artifact_ref: Option<String>,
    pub meta: Option<JsonDict>,
    pub reset_failure_count: bool,
}

#[derive(Debug, Clone)]
pub struct MarkSensorProcessStoppedInput {
    pub sensor: Id,
    pub worker: Id,
    pub stopped_at: Option<DateTime<Utc>>,
    pub active_rule_count: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct MarkSensorProcessFailedInput {
    pub sensor: Id,
    pub worker: Id,
    pub status: SensorProcessStatus,
    pub exit_code: Option<i32>,
    pub signal: Option<i32>,
    pub stopped_at: Option<DateTime<Utc>>,
    pub stderr_excerpt: Option<String>,
    pub active_rule_count: i32,
    pub next_restart_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone)]
pub struct RecordSensorProcessAlertedInput {
    pub sensor: Id,
    pub worker: Id,
    pub failure_count: i32,
    pub alerted_at: Option<DateTime<Utc>>,
}

#[async_trait::async_trait]
impl FindById for SensorProcessRepository {
    async fn find_by_id<'e, E>(executor: E, id: Id) -> Result<Option<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let process = sqlx::query_as::<_, SensorProcess>(&format!(
            "SELECT {SELECT_COLUMNS} FROM sensor_process WHERE id = $1"
        ))
        .bind(id)
        .fetch_optional(executor)
        .await?;

        Ok(process)
    }
}

#[async_trait::async_trait]
impl List for SensorProcessRepository {
    async fn list<'e, E>(executor: E) -> Result<Vec<Self::Entity>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let processes = sqlx::query_as::<_, SensorProcess>(&format!(
            "SELECT {SELECT_COLUMNS} FROM sensor_process ORDER BY sensor_ref ASC, worker_name ASC"
        ))
        .fetch_all(executor)
        .await?;

        Ok(processes)
    }
}

impl SensorProcessRepository {
    pub async fn upsert_starting_or_running<'e, E>(
        executor: E,
        input: UpsertSensorProcessStartInput,
    ) -> Result<SensorProcess>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        if !matches!(
            input.status,
            SensorProcessStatus::Starting | SensorProcessStatus::Running
        ) {
            return Err(Error::validation(
                "sensor process start upsert status must be starting or running",
            ));
        }

        let meta = input.meta.unwrap_or_else(|| json!({}));
        let process = sqlx::query_as::<_, SensorProcess>(&format!(
            "INSERT INTO sensor_process (
                 sensor, sensor_ref, worker, worker_name, status, pid,
                 consecutive_failures, last_exit_code, last_signal, last_started_at,
                 last_stopped_at, next_restart_at, stderr_excerpt, log_artifact_ref,
                 active_rule_count, meta
             )
             VALUES ($1, $2, $3, $4, $5, $6, 0, NULL, NULL, COALESCE($7, NOW()),
                     NULL, NULL, NULL, $8, $9, $10)
             ON CONFLICT (sensor, worker) DO UPDATE SET
                 sensor_ref = EXCLUDED.sensor_ref,
                 worker_name = EXCLUDED.worker_name,
                 status = EXCLUDED.status,
                 pid = EXCLUDED.pid,
                 consecutive_failures = CASE
                     WHEN $11 THEN 0
                     ELSE sensor_process.consecutive_failures
                 END,
                 last_alerted_failure_count = CASE
                     WHEN $11 THEN 0
                     ELSE sensor_process.last_alerted_failure_count
                 END,
                 last_alerted_at = CASE
                     WHEN $11 THEN NULL
                     ELSE sensor_process.last_alerted_at
                 END,
                 last_exit_code = NULL,
                 last_signal = NULL,
                 last_started_at = EXCLUDED.last_started_at,
                 last_stopped_at = NULL,
                 next_restart_at = NULL,
                 stderr_excerpt = NULL,
                 log_artifact_ref = EXCLUDED.log_artifact_ref,
                 active_rule_count = EXCLUDED.active_rule_count,
                 meta = EXCLUDED.meta,
                 updated = NOW()
             RETURNING {SELECT_COLUMNS}"
        ))
        .bind(input.sensor)
        .bind(&input.sensor_ref)
        .bind(input.worker)
        .bind(&input.worker_name)
        .bind(input.status)
        .bind(input.pid)
        .bind(input.started_at)
        .bind(&input.log_artifact_ref)
        .bind(input.active_rule_count)
        .bind(&meta)
        .bind(input.reset_failure_count)
        .fetch_one(executor)
        .await?;

        Ok(process)
    }

    pub async fn mark_stopped<'e, E>(
        executor: E,
        input: MarkSensorProcessStoppedInput,
    ) -> Result<Option<SensorProcess>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let process = sqlx::query_as::<_, SensorProcess>(&format!(
            "UPDATE sensor_process
             SET status = $1,
                 pid = NULL,
                 last_stopped_at = COALESCE($2, NOW()),
                 next_restart_at = NULL,
                 active_rule_count = COALESCE($3, active_rule_count),
                 updated = NOW()
             WHERE sensor = $4 AND worker = $5
             RETURNING {SELECT_COLUMNS}"
        ))
        .bind(SensorProcessStatus::Stopped)
        .bind(input.stopped_at)
        .bind(input.active_rule_count)
        .bind(input.sensor)
        .bind(input.worker)
        .fetch_optional(executor)
        .await?;

        Ok(process)
    }

    pub async fn mark_failed_or_backoff<'e, E>(
        executor: E,
        input: MarkSensorProcessFailedInput,
    ) -> Result<Option<SensorProcess>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        if !matches!(
            input.status,
            SensorProcessStatus::Failed | SensorProcessStatus::Backoff
        ) {
            return Err(Error::validation(
                "sensor process failure transition status must be failed or backoff",
            ));
        }

        let process = sqlx::query_as::<_, SensorProcess>(&format!(
            "UPDATE sensor_process
             SET status = $1,
                 pid = NULL,
                 consecutive_failures = consecutive_failures + 1,
                 last_exit_code = $2,
                 last_signal = $3,
                 last_stopped_at = COALESCE($4, NOW()),
                 stderr_excerpt = $5,
                 active_rule_count = $6,
                 next_restart_at = $7,
                 updated = NOW()
             WHERE sensor = $8 AND worker = $9
             RETURNING {SELECT_COLUMNS}"
        ))
        .bind(input.status)
        .bind(input.exit_code)
        .bind(input.signal)
        .bind(input.stopped_at)
        .bind(&input.stderr_excerpt)
        .bind(input.active_rule_count)
        .bind(input.next_restart_at)
        .bind(input.sensor)
        .bind(input.worker)
        .fetch_optional(executor)
        .await?;

        Ok(process)
    }

    pub async fn record_alerted<'e, E>(
        executor: E,
        input: RecordSensorProcessAlertedInput,
    ) -> Result<Option<SensorProcess>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let process = sqlx::query_as::<_, SensorProcess>(&format!(
            "UPDATE sensor_process
             SET last_alerted_failure_count = $1,
                 last_alerted_at = COALESCE($2, NOW()),
                 updated = NOW()
             WHERE sensor = $3 AND worker = $4
             RETURNING {SELECT_COLUMNS}"
        ))
        .bind(input.failure_count)
        .bind(input.alerted_at)
        .bind(input.sensor)
        .bind(input.worker)
        .fetch_optional(executor)
        .await?;

        Ok(process)
    }

    pub async fn find_by_sensor<'e, E>(executor: E, sensor: Id) -> Result<Vec<SensorProcess>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let processes = sqlx::query_as::<_, SensorProcess>(&format!(
            "SELECT {SELECT_COLUMNS}
             FROM sensor_process
             WHERE sensor = $1
             ORDER BY worker_name ASC"
        ))
        .bind(sensor)
        .fetch_all(executor)
        .await?;

        Ok(processes)
    }

    pub async fn find_by_sensor_ref<'e, E>(
        executor: E,
        sensor_ref: &str,
    ) -> Result<Vec<SensorProcess>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let processes = sqlx::query_as::<_, SensorProcess>(&format!(
            "SELECT {SELECT_COLUMNS}
             FROM sensor_process
             WHERE sensor_ref = $1
             ORDER BY worker_name ASC"
        ))
        .bind(sensor_ref)
        .fetch_all(executor)
        .await?;

        Ok(processes)
    }

    pub async fn find_by_sensor_and_worker<'e, E>(
        executor: E,
        sensor: Id,
        worker: Id,
    ) -> Result<Option<SensorProcess>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let process = sqlx::query_as::<_, SensorProcess>(&format!(
            "SELECT {SELECT_COLUMNS}
             FROM sensor_process
             WHERE sensor = $1 AND worker = $2"
        ))
        .bind(sensor)
        .bind(worker)
        .fetch_optional(executor)
        .await?;

        Ok(process)
    }

    pub async fn list_by_worker<'e, E>(executor: E, worker: Id) -> Result<Vec<SensorProcess>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let processes = sqlx::query_as::<_, SensorProcess>(&format!(
            "SELECT {SELECT_COLUMNS}
             FROM sensor_process
             WHERE worker = $1
             ORDER BY sensor_ref ASC"
        ))
        .bind(worker)
        .fetch_all(executor)
        .await?;

        Ok(processes)
    }

    pub async fn list_ready_for_restart<'e, E>(
        executor: E,
        now: DateTime<Utc>,
    ) -> Result<Vec<SensorProcess>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let processes = sqlx::query_as::<_, SensorProcess>(&format!(
            "SELECT {SELECT_COLUMNS}
             FROM sensor_process
             WHERE status = $1
               AND next_restart_at IS NOT NULL
               AND next_restart_at <= $2
             ORDER BY next_restart_at ASC, sensor_ref ASC, worker_name ASC"
        ))
        .bind(SensorProcessStatus::Backoff)
        .bind(now)
        .fetch_all(executor)
        .await?;

        Ok(processes)
    }
}
