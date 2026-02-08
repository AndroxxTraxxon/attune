//! Execution Scheduler - Routes executions to available workers
//!
//! This module is responsible for:
//! - Listening for ExecutionRequested messages
//! - Selecting appropriate workers for executions
//! - Queuing executions to worker-specific queues
//! - Updating execution status to Scheduled
//! - Handling worker unavailability and retries

use anyhow::Result;
use attune_common::{
    models::{enums::ExecutionStatus, Action, Execution},
    mq::{Consumer, ExecutionRequestedPayload, MessageEnvelope, MessageType, Publisher},
    repositories::{
        action::ActionRepository,
        execution::ExecutionRepository,
        runtime::{RuntimeRepository, WorkerRepository},
        FindById, FindByRef, Update,
    },
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

/// Payload for execution scheduled messages
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ExecutionScheduledPayload {
    execution_id: i64,
    worker_id: i64,
    action_ref: String,
    config: Option<JsonValue>,
}

/// Execution scheduler that routes executions to workers
pub struct ExecutionScheduler {
    pool: PgPool,
    publisher: Arc<Publisher>,
    consumer: Arc<Consumer>,
}

/// Default heartbeat interval in seconds (should match worker config default)
const DEFAULT_HEARTBEAT_INTERVAL: u64 = 30;

/// Maximum age multiplier for heartbeat staleness check
/// Workers are considered stale if heartbeat is older than HEARTBEAT_INTERVAL * HEARTBEAT_STALENESS_MULTIPLIER
const HEARTBEAT_STALENESS_MULTIPLIER: u64 = 3;

impl ExecutionScheduler {
    /// Create a new execution scheduler
    pub fn new(pool: PgPool, publisher: Arc<Publisher>, consumer: Arc<Consumer>) -> Self {
        Self {
            pool,
            publisher,
            consumer,
        }
    }

    /// Start processing execution requested messages
    pub async fn start(&self) -> Result<()> {
        info!("Starting execution scheduler");

        let pool = self.pool.clone();
        let publisher = self.publisher.clone();

        // Use the handler pattern to consume messages
        self.consumer
            .consume_with_handler(
                move |envelope: MessageEnvelope<ExecutionRequestedPayload>| {
                    let pool = pool.clone();
                    let publisher = publisher.clone();

                    async move {
                        if let Err(e) =
                            Self::process_execution_requested(&pool, &publisher, &envelope).await
                        {
                            error!("Error scheduling execution: {}", e);
                            // Return error to trigger nack with requeue
                            return Err(format!("Failed to schedule execution: {}", e).into());
                        }
                        Ok(())
                    }
                },
            )
            .await?;

        Ok(())
    }

    /// Process an execution requested message
    async fn process_execution_requested(
        pool: &PgPool,
        publisher: &Publisher,
        envelope: &MessageEnvelope<ExecutionRequestedPayload>,
    ) -> Result<()> {
        debug!("Processing execution requested message: {:?}", envelope);

        let execution_id = envelope.payload.execution_id;

        info!("Scheduling execution: {}", execution_id);

        // Fetch execution from database
        let mut execution = ExecutionRepository::find_by_id(pool, execution_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Execution not found: {}", execution_id))?;

        // Fetch action to determine runtime requirements
        let action = Self::get_action_for_execution(pool, &execution).await?;

        // Select appropriate worker
        let worker = Self::select_worker(pool, &action).await?;

        info!(
            "Selected worker {} for execution {}",
            worker.id, execution_id
        );

        // Update execution status to scheduled
        let execution_config = execution.config.clone();
        execution.status = ExecutionStatus::Scheduled;
        ExecutionRepository::update(pool, execution.id, execution.into()).await?;

        // Publish message to worker-specific queue
        Self::queue_to_worker(
            publisher,
            &execution_id,
            &worker.id,
            &envelope.payload.action_ref,
            &execution_config,
            &action,
        )
        .await?;

        info!(
            "Execution {} scheduled to worker {}",
            execution_id, worker.id
        );

        Ok(())
    }

    /// Get the action associated with an execution
    async fn get_action_for_execution(pool: &PgPool, execution: &Execution) -> Result<Action> {
        // Try to get action by ID first
        if let Some(action_id) = execution.action {
            if let Some(action) = ActionRepository::find_by_id(pool, action_id).await? {
                return Ok(action);
            }
        }

        // Fall back to action_ref
        ActionRepository::find_by_ref(pool, &execution.action_ref)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Action not found for execution: {}", execution.id))
    }

    /// Select an appropriate worker for the execution
    async fn select_worker(
        pool: &PgPool,
        action: &Action,
    ) -> Result<attune_common::models::Worker> {
        // Get runtime requirements for the action
        let runtime = if let Some(runtime_id) = action.runtime {
            RuntimeRepository::find_by_id(pool, runtime_id).await?
        } else {
            None
        };

        // Find available action workers (role = 'action')
        let workers = WorkerRepository::find_action_workers(pool).await?;

        if workers.is_empty() {
            return Err(anyhow::anyhow!("No action workers available"));
        }

        // Filter workers by runtime compatibility if runtime is specified
        let compatible_workers: Vec<_> = if let Some(ref runtime) = runtime {
            workers
                .into_iter()
                .filter(|w| Self::worker_supports_runtime(w, &runtime.name))
                .collect()
        } else {
            workers
        };

        if compatible_workers.is_empty() {
            let runtime_name = runtime.as_ref().map(|r| r.name.as_str()).unwrap_or("any");
            return Err(anyhow::anyhow!(
                "No compatible workers found for action: {} (requires runtime: {})",
                action.r#ref,
                runtime_name
            ));
        }

        // Filter by worker status (only active workers)
        let active_workers: Vec<_> = compatible_workers
            .into_iter()
            .filter(|w| w.status == Some(attune_common::models::enums::WorkerStatus::Active))
            .collect();

        if active_workers.is_empty() {
            return Err(anyhow::anyhow!("No active workers available"));
        }

        // Filter by heartbeat freshness (only workers with recent heartbeats)
        let fresh_workers: Vec<_> = active_workers
            .into_iter()
            .filter(|w| Self::is_worker_heartbeat_fresh(w))
            .collect();

        if fresh_workers.is_empty() {
            warn!("No workers with fresh heartbeats available. All active workers have stale heartbeats.");
            return Err(anyhow::anyhow!(
                "No workers with fresh heartbeats available (heartbeat older than {} seconds)",
                DEFAULT_HEARTBEAT_INTERVAL * HEARTBEAT_STALENESS_MULTIPLIER
            ));
        }

        // TODO: Implement intelligent worker selection:
        // - Consider worker load/capacity
        // - Consider worker affinity (same pack, same runtime)
        // - Consider geographic locality
        // - Round-robin or least-connections strategy

        // For now, just select the first available worker
        Ok(fresh_workers
            .into_iter()
            .next()
            .expect("Worker list should not be empty"))
    }

    /// Check if a worker supports a given runtime
    ///
    /// This checks the worker's capabilities.runtimes array for the runtime name.
    /// Falls back to checking the deprecated runtime column if capabilities are not set.
    fn worker_supports_runtime(worker: &attune_common::models::Worker, runtime_name: &str) -> bool {
        // First, try to parse capabilities and check runtimes array
        if let Some(ref capabilities) = worker.capabilities {
            if let Some(runtimes) = capabilities.get("runtimes") {
                if let Some(runtime_array) = runtimes.as_array() {
                    // Check if any runtime in the array matches (case-insensitive)
                    for runtime_value in runtime_array {
                        if let Some(runtime_str) = runtime_value.as_str() {
                            if runtime_str.eq_ignore_ascii_case(runtime_name) {
                                debug!(
                                    "Worker {} supports runtime '{}' via capabilities",
                                    worker.name, runtime_name
                                );
                                return true;
                            }
                        }
                    }
                }
            }
        }

        // Fallback: check deprecated runtime column
        // This is kept for backward compatibility but should be removed in the future
        if worker.runtime.is_some() {
            debug!(
                "Worker {} using deprecated runtime column for matching",
                worker.name
            );
            // Note: This fallback is incomplete because we'd need to look up the runtime name
            // from the ID, which would require an async call. Since we're moving to capabilities,
            // we'll just return false here and require workers to set capabilities properly.
        }

        debug!(
            "Worker {} does not support runtime '{}'",
            worker.name, runtime_name
        );
        false
    }

    /// Check if a worker's heartbeat is fresh enough to schedule work
    ///
    /// A worker is considered fresh if its last heartbeat is within
    /// HEARTBEAT_STALENESS_MULTIPLIER * HEARTBEAT_INTERVAL seconds.
    fn is_worker_heartbeat_fresh(worker: &attune_common::models::Worker) -> bool {
        let Some(last_heartbeat) = worker.last_heartbeat else {
            warn!(
                "Worker {} has no heartbeat recorded, considering stale",
                worker.name
            );
            return false;
        };

        let now = Utc::now();
        let age = now.signed_duration_since(last_heartbeat);
        let max_age = Duration::from_secs(DEFAULT_HEARTBEAT_INTERVAL * HEARTBEAT_STALENESS_MULTIPLIER);

        let is_fresh = age.to_std().unwrap_or(Duration::MAX) <= max_age;

        if !is_fresh {
            warn!(
                "Worker {} heartbeat is stale: last seen {} seconds ago (max: {} seconds)",
                worker.name,
                age.num_seconds(),
                max_age.as_secs()
            );
        } else {
            debug!(
                "Worker {} heartbeat is fresh: last seen {} seconds ago",
                worker.name,
                age.num_seconds()
            );
        }

        is_fresh
    }

    /// Queue execution to a specific worker
    async fn queue_to_worker(
        publisher: &Publisher,
        execution_id: &i64,
        worker_id: &i64,
        action_ref: &str,
        config: &Option<JsonValue>,
        _action: &Action,
    ) -> Result<()> {
        debug!("Queuing execution {} to worker {}", execution_id, worker_id);

        // Create payload for worker
        let payload = ExecutionScheduledPayload {
            execution_id: *execution_id,
            worker_id: *worker_id,
            action_ref: action_ref.to_string(),
            config: config.clone(),
        };

        let envelope =
            MessageEnvelope::new(MessageType::ExecutionRequested, payload).with_source("executor");

        // Publish to worker-specific queue with routing key
        let routing_key = format!("execution.dispatch.worker.{}", worker_id);
        let exchange = "attune.executions";

        publisher
            .publish_envelope_with_routing(&envelope, exchange, &routing_key)
            .await?;

        info!(
            "Published execution.scheduled message to worker {} (routing key: {})",
            worker_id, routing_key
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use attune_common::models::{Worker, WorkerRole, WorkerStatus, WorkerType};
    use chrono::{Duration as ChronoDuration, Utc};

    fn create_test_worker(name: &str, heartbeat_offset_secs: i64) -> Worker {
        let last_heartbeat = if heartbeat_offset_secs == 0 {
            None
        } else {
            Some(Utc::now() - ChronoDuration::seconds(heartbeat_offset_secs))
        };

        Worker {
            id: 1,
            name: name.to_string(),
            worker_type: WorkerType::Local,
            worker_role: WorkerRole::Action,
            runtime: None,
            host: Some("localhost".to_string()),
            port: Some(8080),
            status: Some(WorkerStatus::Active),
            capabilities: Some(serde_json::json!({
                "runtimes": ["shell", "python"]
            })),
            meta: None,
            last_heartbeat,
            created: Utc::now(),
            updated: Utc::now(),
        }
    }

    #[test]
    fn test_heartbeat_freshness_with_recent_heartbeat() {
        // Worker with heartbeat 30 seconds ago (within limit)
        let worker = create_test_worker("test-worker", 30);
        assert!(
            ExecutionScheduler::is_worker_heartbeat_fresh(&worker),
            "Worker with 30s old heartbeat should be considered fresh"
        );
    }

    #[test]
    fn test_heartbeat_freshness_with_stale_heartbeat() {
        // Worker with heartbeat 100 seconds ago (beyond 3x30s = 90s limit)
        let worker = create_test_worker("test-worker", 100);
        assert!(
            !ExecutionScheduler::is_worker_heartbeat_fresh(&worker),
            "Worker with 100s old heartbeat should be considered stale"
        );
    }

    #[test]
    fn test_heartbeat_freshness_at_boundary() {
        // Worker with heartbeat exactly at the 90 second boundary
        let worker = create_test_worker("test-worker", 90);
        assert!(
            !ExecutionScheduler::is_worker_heartbeat_fresh(&worker),
            "Worker with 90s old heartbeat should be considered stale (at boundary)"
        );
    }

    #[test]
    fn test_heartbeat_freshness_with_no_heartbeat() {
        // Worker with no heartbeat recorded
        let worker = create_test_worker("test-worker", 0);
        assert!(
            !ExecutionScheduler::is_worker_heartbeat_fresh(&worker),
            "Worker with no heartbeat should be considered stale"
        );
    }

    #[test]
    fn test_heartbeat_freshness_with_very_recent() {
        // Worker with heartbeat 5 seconds ago
        let worker = create_test_worker("test-worker", 5);
        assert!(
            ExecutionScheduler::is_worker_heartbeat_fresh(&worker),
            "Worker with 5s old heartbeat should be considered fresh"
        );
    }

    #[test]
    fn test_scheduler_creation() {
        // This is a placeholder test
        // Real tests will require database and message queue setup
        assert!(true);
    }
}
