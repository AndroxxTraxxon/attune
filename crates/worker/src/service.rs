//! Worker Service Module
//!
//! Main service orchestration for the Attune Worker Service.
//! Manages worker registration, heartbeat, message consumption, and action execution.

use attune_common::config::Config;
use attune_common::db::Database;
use attune_common::error::{Error, Result};
use attune_common::models::ExecutionStatus;
use attune_common::mq::{
    config::MessageQueueConfig as MqConfig, Connection, Consumer, ConsumerConfig,
    ExecutionCompletedPayload, ExecutionStatusChangedPayload, MessageEnvelope, MessageType,
    PackRegisteredPayload, Publisher, PublisherConfig,
};
use attune_common::repositories::{execution::ExecutionRepository, FindById};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tracing::{debug, error, info, warn};

use crate::artifacts::ArtifactManager;
use crate::env_setup;
use crate::executor::ActionExecutor;
use crate::heartbeat::HeartbeatManager;
use crate::registration::WorkerRegistration;
use crate::runtime::local::LocalRuntime;
use crate::runtime::native::NativeRuntime;
use crate::runtime::process::ProcessRuntime;
use crate::runtime::shell::ShellRuntime;
use crate::runtime::RuntimeRegistry;
use crate::secrets::SecretManager;

use attune_common::repositories::runtime::RuntimeRepository;
use attune_common::repositories::List;

/// Message payload for execution.scheduled events
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionScheduledPayload {
    pub execution_id: i64,
    pub action_ref: String,
    pub worker_id: i64,
}

/// Worker service that manages execution lifecycle
pub struct WorkerService {
    #[allow(dead_code)]
    config: Config,
    db_pool: PgPool,
    registration: Arc<RwLock<WorkerRegistration>>,
    heartbeat: Arc<HeartbeatManager>,
    executor: Arc<ActionExecutor>,
    mq_connection: Arc<Connection>,
    publisher: Arc<Publisher>,
    consumer: Option<Arc<Consumer>>,
    consumer_handle: Option<JoinHandle<()>>,
    pack_consumer: Option<Arc<Consumer>>,
    pack_consumer_handle: Option<JoinHandle<()>>,
    worker_id: Option<i64>,
    /// Runtime filter derived from ATTUNE_WORKER_RUNTIMES
    runtime_filter: Option<Vec<String>>,
    /// Base directory for pack files
    packs_base_dir: PathBuf,
    /// Base directory for isolated runtime environments
    runtime_envs_dir: PathBuf,
}

impl WorkerService {
    /// Create a new worker service
    pub async fn new(config: Config) -> Result<Self> {
        info!("Initializing Worker Service");

        // Initialize database
        let db = Database::new(&config.database).await?;
        let pool = db.pool().clone();
        info!("Database connection established");

        // Initialize message queue connection
        let mq_url = config
            .message_queue
            .as_ref()
            .ok_or_else(|| Error::Internal("Message queue configuration is required".to_string()))?
            .url
            .as_str();

        let mq_connection = Connection::connect(mq_url)
            .await
            .map_err(|e| Error::Internal(format!("Failed to connect to message queue: {}", e)))?;
        info!("Message queue connection established");

        // Setup common message queue infrastructure (exchanges and DLX)
        let mq_config = MqConfig::default();
        match mq_connection.setup_common_infrastructure(&mq_config).await {
            Ok(_) => info!("Common message queue infrastructure setup completed"),
            Err(e) => {
                warn!(
                    "Failed to setup common MQ infrastructure (may already exist): {}",
                    e
                );
            }
        }

        // Initialize message queue publisher
        let publisher = Publisher::new(
            &mq_connection,
            PublisherConfig {
                confirm_publish: true,
                timeout_secs: 30,
                exchange: "attune.executions".to_string(),
            },
        )
        .await
        .map_err(|e| Error::Internal(format!("Failed to create publisher: {}", e)))?;
        info!("Message queue publisher initialized");

        // Initialize worker registration
        let registration = Arc::new(RwLock::new(WorkerRegistration::new(pool.clone(), &config)));

        // Initialize artifact manager
        let artifact_base_dir = std::path::PathBuf::from(
            config
                .worker
                .as_ref()
                .and_then(|w| w.name.clone())
                .map(|name| format!("/tmp/attune/artifacts/{}", name))
                .unwrap_or_else(|| "/tmp/attune/artifacts".to_string()),
        );
        let artifact_manager = ArtifactManager::new(artifact_base_dir);
        artifact_manager.initialize().await?;

        let packs_base_dir = std::path::PathBuf::from(&config.packs_base_dir);
        let runtime_envs_dir = std::path::PathBuf::from(&config.runtime_envs_dir);

        // Determine which runtimes to register based on configuration
        // ATTUNE_WORKER_RUNTIMES env var filters which runtimes this worker handles.
        // If not set, all action runtimes from the database are loaded.
        let runtime_filter: Option<Vec<String>> =
            std::env::var("ATTUNE_WORKER_RUNTIMES").ok().map(|env_val| {
                info!(
                    "Filtering runtimes from ATTUNE_WORKER_RUNTIMES: {}",
                    env_val
                );
                env_val
                    .split(',')
                    .map(|s| s.trim().to_lowercase())
                    .filter(|s| !s.is_empty())
                    .collect()
            });

        // Initialize runtime registry
        let mut runtime_registry = RuntimeRegistry::new();

        // Load runtimes from the database and create ProcessRuntime instances.
        // Each runtime row's `execution_config` JSONB drives how the ProcessRuntime
        // invokes interpreters, manages environments, and installs dependencies.
        // We skip runtimes with empty execution_config (e.g., the built-in sensor
        // runtime) since they have no interpreter and cannot execute as a process.
        match RuntimeRepository::list(&pool).await {
            Ok(db_runtimes) => {
                let executable_runtimes: Vec<_> = db_runtimes
                    .into_iter()
                    .filter(|r| {
                        let config = r.parsed_execution_config();
                        // A runtime is executable if it has a non-default interpreter
                        // (the default is "/bin/sh" from InterpreterConfig::default,
                        // but runtimes with no execution_config at all will have an
                        // empty JSON object that deserializes to defaults with no
                        // file_extension — those are not real process runtimes).
                        config.interpreter.file_extension.is_some()
                            || r.execution_config != serde_json::json!({})
                    })
                    .collect();

                info!(
                    "Found {} executable runtime(s) in database",
                    executable_runtimes.len()
                );

                for rt in executable_runtimes {
                    let rt_name = rt.name.to_lowercase();

                    // Apply filter if ATTUNE_WORKER_RUNTIMES is set
                    if let Some(ref filter) = runtime_filter {
                        if !filter.contains(&rt_name) {
                            debug!(
                                "Skipping runtime '{}' (not in ATTUNE_WORKER_RUNTIMES filter)",
                                rt_name
                            );
                            continue;
                        }
                    }

                    let exec_config = rt.parsed_execution_config();
                    let process_runtime = ProcessRuntime::new(
                        rt_name.clone(),
                        exec_config,
                        packs_base_dir.clone(),
                        runtime_envs_dir.clone(),
                    );
                    runtime_registry.register(Box::new(process_runtime));
                    info!(
                        "Registered ProcessRuntime '{}' from database (ref: {})",
                        rt_name, rt.r#ref
                    );
                }
            }
            Err(e) => {
                warn!(
                    "Failed to load runtimes from database: {}. \
                     Falling back to built-in defaults.",
                    e
                );
            }
        }

        // If no runtimes were loaded from the DB, register built-in defaults
        if runtime_registry.list_runtimes().is_empty() {
            info!("No runtimes loaded from database, registering built-in defaults");

            // Shell runtime (always available)
            runtime_registry.register(Box::new(ShellRuntime::new()));
            info!("Registered built-in Shell runtime");

            // Native runtime (for compiled binaries)
            runtime_registry.register(Box::new(NativeRuntime::new()));
            info!("Registered built-in Native runtime");

            // Local runtime as catch-all fallback
            let local_runtime = LocalRuntime::new();
            runtime_registry.register(Box::new(local_runtime));
            info!("Registered Local runtime (fallback)");
        }

        // Validate all registered runtimes
        runtime_registry
            .validate_all()
            .await
            .map_err(|e| Error::Internal(format!("Failed to validate runtimes: {}", e)))?;

        info!(
            "Successfully validated runtimes: {:?}",
            runtime_registry.list_runtimes()
        );

        // Initialize secret manager
        let encryption_key = config.security.encryption_key.clone();
        let secret_manager = SecretManager::new(pool.clone(), encryption_key)?;
        info!("Secret manager initialized");

        // Initialize action executor
        let max_stdout_bytes = config
            .worker
            .as_ref()
            .map(|w| w.max_stdout_bytes)
            .unwrap_or(10 * 1024 * 1024);
        let max_stderr_bytes = config
            .worker
            .as_ref()
            .map(|w| w.max_stderr_bytes)
            .unwrap_or(10 * 1024 * 1024);

        // Get API URL from environment or construct from server config
        let api_url = std::env::var("ATTUNE_API_URL")
            .unwrap_or_else(|_| format!("http://{}:{}", config.server.host, config.server.port));

        let executor = Arc::new(ActionExecutor::new(
            pool.clone(),
            runtime_registry,
            artifact_manager,
            secret_manager,
            max_stdout_bytes,
            max_stderr_bytes,
            packs_base_dir.clone(),
            api_url,
        ));

        // Initialize heartbeat manager
        let heartbeat_interval = config
            .worker
            .as_ref()
            .map(|w| w.heartbeat_interval)
            .unwrap_or(30);
        let heartbeat = Arc::new(HeartbeatManager::new(
            registration.clone(),
            heartbeat_interval,
        ));

        // Capture the runtime filter for use in env setup
        let runtime_filter_for_service = runtime_filter.clone();

        Ok(Self {
            config,
            db_pool: pool,
            registration,
            heartbeat,
            executor,
            mq_connection: Arc::new(mq_connection),
            publisher: Arc::new(publisher),
            consumer: None,
            consumer_handle: None,
            pack_consumer: None,
            pack_consumer_handle: None,
            worker_id: None,
            runtime_filter: runtime_filter_for_service,
            packs_base_dir,
            runtime_envs_dir,
        })
    }

    /// Start the worker service
    pub async fn start(&mut self) -> Result<()> {
        info!("Starting Worker Service");

        // Detect runtime capabilities and register worker
        let worker_id = {
            let mut reg = self.registration.write().await;
            reg.detect_capabilities(&self.config).await?;
            reg.register().await?
        };
        self.worker_id = Some(worker_id);

        info!("Worker registered with ID: {}", worker_id);

        // Setup worker-specific message queue infrastructure
        // (includes per-worker execution queue AND pack registration queue)
        let mq_config = MqConfig::default();
        self.mq_connection
            .setup_worker_infrastructure(worker_id, &mq_config)
            .await
            .map_err(|e| {
                Error::Internal(format!("Failed to setup worker MQ infrastructure: {}", e))
            })?;
        info!("Worker-specific message queue infrastructure setup completed");

        // Proactively set up runtime environments for all registered packs.
        // This runs before we start consuming execution messages so that
        // environments are ready by the time the first execution arrives.
        self.scan_and_setup_environments().await;

        // Start heartbeat
        self.heartbeat.start().await?;

        // Start consuming execution messages
        self.start_execution_consumer().await?;

        // Start consuming pack registration events
        self.start_pack_consumer().await?;

        info!("Worker Service started successfully");

        Ok(())
    }

    /// Stop the worker service gracefully
    ///
    /// Shutdown order (mirrors sensor service pattern):
    /// 1. Deregister worker (mark inactive to stop receiving new work)
    /// 2. Stop heartbeat
    /// 3. Wait for in-flight tasks with timeout
    /// 4. Close MQ connection
    /// 5. Close DB connection
    /// Scan all registered packs and create missing runtime environments.
    async fn scan_and_setup_environments(&self) {
        let filter_refs: Option<Vec<String>> = self.runtime_filter.clone();
        let filter_slice: Option<&[String]> = filter_refs.as_deref();

        let result = env_setup::scan_and_setup_all_environments(
            &self.db_pool,
            filter_slice,
            &self.packs_base_dir,
            &self.runtime_envs_dir,
        )
        .await;

        if !result.errors.is_empty() {
            warn!(
                "Environment startup scan completed with {} error(s): {:?}",
                result.errors.len(),
                result.errors,
            );
        } else {
            info!(
                "Environment startup scan completed: {} pack(s) scanned, \
                 {} environment(s) ensured, {} skipped",
                result.packs_scanned, result.environments_created, result.environments_skipped,
            );
        }
    }

    /// Start consuming pack.registered events from the per-worker packs queue.
    async fn start_pack_consumer(&mut self) -> Result<()> {
        let worker_id = self
            .worker_id
            .ok_or_else(|| Error::Internal("Worker not registered".to_string()))?;

        let queue_name = format!("worker.{}.packs", worker_id);
        info!(
            "Starting pack registration consumer for queue: {}",
            queue_name
        );

        let consumer = Arc::new(
            Consumer::new(
                &self.mq_connection,
                ConsumerConfig {
                    queue: queue_name.clone(),
                    tag: format!("worker-{}-packs", worker_id),
                    prefetch_count: 5,
                    auto_ack: false,
                    exclusive: false,
                },
            )
            .await
            .map_err(|e| Error::Internal(format!("Failed to create pack consumer: {}", e)))?,
        );

        let db_pool = self.db_pool.clone();
        let consumer_for_task = consumer.clone();
        let queue_name_for_log = queue_name.clone();
        let runtime_filter = self.runtime_filter.clone();
        let packs_base_dir = self.packs_base_dir.clone();
        let runtime_envs_dir = self.runtime_envs_dir.clone();

        let handle = tokio::spawn(async move {
            info!(
                "Pack consumer loop started for queue '{}'",
                queue_name_for_log
            );
            let result = consumer_for_task
                .consume_with_handler(move |envelope: MessageEnvelope<PackRegisteredPayload>| {
                    let db_pool = db_pool.clone();
                    let runtime_filter = runtime_filter.clone();
                    let packs_base_dir = packs_base_dir.clone();
                    let runtime_envs_dir = runtime_envs_dir.clone();

                    async move {
                        info!(
                            "Received pack.registered event for pack '{}' (version {})",
                            envelope.payload.pack_ref, envelope.payload.version,
                        );

                        let filter_slice: Option<Vec<String>> = runtime_filter;
                        let filter_ref: Option<&[String]> = filter_slice.as_deref();

                        let pack_result = env_setup::setup_environments_for_registered_pack(
                            &db_pool,
                            &envelope.payload,
                            filter_ref,
                            &packs_base_dir,
                            &runtime_envs_dir,
                        )
                        .await;

                        if !pack_result.errors.is_empty() {
                            warn!(
                                "Pack '{}' environment setup had {} error(s): {:?}",
                                pack_result.pack_ref,
                                pack_result.errors.len(),
                                pack_result.errors,
                            );
                        } else if !pack_result.environments_created.is_empty() {
                            info!(
                                "Pack '{}' environments set up: {:?}",
                                pack_result.pack_ref, pack_result.environments_created,
                            );
                        }

                        Ok(())
                    }
                })
                .await;

            match result {
                Ok(()) => info!(
                    "Pack consumer loop for queue '{}' ended",
                    queue_name_for_log
                ),
                Err(e) => error!(
                    "Pack consumer loop for queue '{}' failed: {}",
                    queue_name_for_log, e
                ),
            }
        });

        self.pack_consumer = Some(consumer);
        self.pack_consumer_handle = Some(handle);

        info!("Pack registration consumer initialized");

        Ok(())
    }

    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping Worker Service - initiating graceful shutdown");

        // 1. Mark worker as inactive first to stop receiving new tasks
        // Use if-let instead of ? so shutdown continues even if DB call fails
        {
            let reg = self.registration.read().await;
            info!("Marking worker as inactive to stop receiving new tasks");
            if let Err(e) = reg.deregister().await {
                error!("Failed to deregister worker: {}", e);
            }
        }

        // 2. Stop heartbeat
        info!("Stopping heartbeat updates");
        self.heartbeat.stop().await;

        // Wait a bit for heartbeat loop to notice the flag
        tokio::time::sleep(Duration::from_millis(100)).await;

        // 3. Wait for in-flight tasks to complete (with timeout)
        let shutdown_timeout = self
            .config
            .worker
            .as_ref()
            .and_then(|w| w.shutdown_timeout)
            .unwrap_or(30); // Default: 30 seconds

        info!(
            "Waiting up to {} seconds for in-flight tasks to complete",
            shutdown_timeout
        );

        let timeout_duration = Duration::from_secs(shutdown_timeout as u64);
        match tokio::time::timeout(timeout_duration, self.wait_for_in_flight_tasks()).await {
            Ok(_) => info!("All in-flight tasks completed"),
            Err(_) => warn!("Shutdown timeout reached - some tasks may have been interrupted"),
        }

        // 4. Abort consumer tasks and close message queue connection
        if let Some(handle) = self.consumer_handle.take() {
            info!("Stopping execution consumer task...");
            handle.abort();
            // Wait briefly for the task to finish
            let _ = handle.await;
        }

        if let Some(handle) = self.pack_consumer_handle.take() {
            info!("Stopping pack consumer task...");
            handle.abort();
            let _ = handle.await;
        }

        info!("Closing message queue connection...");
        if let Err(e) = self.mq_connection.close().await {
            warn!("Error closing message queue: {}", e);
        }

        // 5. Close database connection
        info!("Closing database connection...");
        self.db_pool.close().await;

        info!("Worker Service stopped");

        Ok(())
    }

    /// Wait for in-flight tasks to complete
    async fn wait_for_in_flight_tasks(&self) {
        // Poll for active executions with short intervals
        loop {
            // Check if executor has any active tasks
            // Note: This is a simplified check. In a real implementation,
            // we would track active execution count in the executor.
            tokio::time::sleep(Duration::from_millis(500)).await;

            // TODO: Add proper tracking of active executions in ActionExecutor
            // For now, we just wait a reasonable amount of time
            // This will be improved when we add execution tracking
            break;
        }
    }

    /// Start consuming execution.scheduled messages
    ///
    /// Spawns the consumer loop as a background task so that `start()` returns
    /// immediately, allowing the caller to set up signal handlers.
    async fn start_execution_consumer(&mut self) -> Result<()> {
        let worker_id = self
            .worker_id
            .ok_or_else(|| Error::Internal("Worker not registered".to_string()))?;

        // Queue name for this worker (already created in setup_worker_infrastructure)
        let queue_name = format!("worker.{}.executions", worker_id);

        info!("Starting consumer for worker queue: {}", queue_name);

        // Create consumer
        let consumer = Arc::new(
            Consumer::new(
                &self.mq_connection,
                ConsumerConfig {
                    queue: queue_name.clone(),
                    tag: format!("worker-{}", worker_id),
                    prefetch_count: 10,
                    auto_ack: false,
                    exclusive: false,
                },
            )
            .await
            .map_err(|e| Error::Internal(format!("Failed to create consumer: {}", e)))?,
        );

        info!("Consumer created for queue: {}", queue_name);

        // Clone Arc references for the spawned task
        let executor = self.executor.clone();
        let publisher = self.publisher.clone();
        let db_pool = self.db_pool.clone();
        let consumer_for_task = consumer.clone();
        let queue_name_for_log = queue_name.clone();

        // Spawn the consumer loop as a background task so start() can return
        let handle = tokio::spawn(async move {
            info!("Consumer loop started for queue '{}'", queue_name_for_log);
            let result = consumer_for_task
                .consume_with_handler(
                    move |envelope: MessageEnvelope<ExecutionScheduledPayload>| {
                        let executor = executor.clone();
                        let publisher = publisher.clone();
                        let db_pool = db_pool.clone();

                        async move {
                            Self::handle_execution_scheduled(executor, publisher, db_pool, envelope)
                                .await
                                .map_err(|e| format!("Execution handler error: {}", e).into())
                        }
                    },
                )
                .await;

            match result {
                Ok(()) => info!("Consumer loop for queue '{}' ended", queue_name_for_log),
                Err(e) => error!(
                    "Consumer loop for queue '{}' failed: {}",
                    queue_name_for_log, e
                ),
            }
        });

        // Store consumer reference and task handle
        self.consumer = Some(consumer);
        self.consumer_handle = Some(handle);

        info!("Message queue consumer initialized");

        Ok(())
    }

    /// Handle execution.scheduled message
    async fn handle_execution_scheduled(
        executor: Arc<ActionExecutor>,
        publisher: Arc<Publisher>,
        db_pool: PgPool,
        envelope: MessageEnvelope<ExecutionScheduledPayload>,
    ) -> Result<()> {
        let execution_id = envelope.payload.execution_id;

        info!(
            "Processing execution.scheduled for execution: {}",
            execution_id
        );

        // Publish status: running
        if let Err(e) = Self::publish_status_update(
            &db_pool,
            &publisher,
            execution_id,
            ExecutionStatus::Running,
            None,
            None,
        )
        .await
        {
            error!("Failed to publish running status: {}", e);
            // Continue anyway - we'll update the database directly
        }

        // Execute the action
        match executor.execute(execution_id).await {
            Ok(result) => {
                info!(
                    "Execution {} completed successfully in {}ms",
                    execution_id, result.duration_ms
                );

                // Publish status: completed
                if let Err(e) = Self::publish_status_update(
                    &db_pool,
                    &publisher,
                    execution_id,
                    ExecutionStatus::Completed,
                    result.result.clone(),
                    None,
                )
                .await
                {
                    error!("Failed to publish success status: {}", e);
                }

                // Publish completion notification for queue management
                if let Err(e) = Self::publish_completion_notification(
                    &db_pool,
                    &publisher,
                    execution_id,
                    ExecutionStatus::Completed,
                )
                .await
                {
                    error!(
                        "Failed to publish completion notification for execution {}: {}",
                        execution_id, e
                    );
                    // Continue - this is important for queue management but not fatal
                }
            }
            Err(e) => {
                error!("Execution {} failed: {}", execution_id, e);

                // Publish status: failed
                if let Err(e) = Self::publish_status_update(
                    &db_pool,
                    &publisher,
                    execution_id,
                    ExecutionStatus::Failed,
                    None,
                    Some(e.to_string()),
                )
                .await
                {
                    error!("Failed to publish failure status: {}", e);
                }

                // Publish completion notification for queue management
                if let Err(e) = Self::publish_completion_notification(
                    &db_pool,
                    &publisher,
                    execution_id,
                    ExecutionStatus::Failed,
                )
                .await
                {
                    error!(
                        "Failed to publish completion notification for execution {}: {}",
                        execution_id, e
                    );
                    // Continue - this is important for queue management but not fatal
                }
            }
        }

        Ok(())
    }

    /// Publish execution status update
    async fn publish_status_update(
        db_pool: &PgPool,
        publisher: &Publisher,
        execution_id: i64,
        status: ExecutionStatus,
        _result: Option<serde_json::Value>,
        _error: Option<String>,
    ) -> Result<()> {
        // Fetch execution to get action_ref and previous status
        let execution = ExecutionRepository::find_by_id(db_pool, execution_id)
            .await?
            .ok_or_else(|| {
                Error::Internal(format!(
                    "Execution {} not found for status update",
                    execution_id
                ))
            })?;

        let new_status_str = match status {
            ExecutionStatus::Running => "running",
            ExecutionStatus::Completed => "completed",
            ExecutionStatus::Failed => "failed",
            ExecutionStatus::Cancelled => "cancelled",
            ExecutionStatus::Timeout => "timeout",
            _ => "unknown",
        };

        let previous_status_str = format!("{:?}", execution.status).to_lowercase();

        let payload = ExecutionStatusChangedPayload {
            execution_id,
            action_ref: execution.action_ref,
            previous_status: previous_status_str,
            new_status: new_status_str.to_string(),
            changed_at: Utc::now(),
        };

        let message_type = MessageType::ExecutionStatusChanged;

        let envelope = MessageEnvelope::new(message_type, payload).with_source("worker");

        publisher
            .publish_envelope(&envelope)
            .await
            .map_err(|e| Error::Internal(format!("Failed to publish status update: {}", e)))?;

        Ok(())
    }

    /// Publish execution completion notification for queue management
    async fn publish_completion_notification(
        db_pool: &PgPool,
        publisher: &Publisher,
        execution_id: i64,
        final_status: ExecutionStatus,
    ) -> Result<()> {
        // Fetch execution to get action_id and other required fields
        let execution = ExecutionRepository::find_by_id(db_pool, execution_id)
            .await?
            .ok_or_else(|| {
                Error::Internal(format!(
                    "Execution {} not found after completion",
                    execution_id
                ))
            })?;

        // Extract action_id - it should always be present for valid executions
        let action_id = execution.action.ok_or_else(|| {
            Error::Internal(format!(
                "Execution {} has no associated action",
                execution_id
            ))
        })?;

        info!(
            "Publishing completion notification for execution {} (action_id: {})",
            execution_id, action_id
        );

        let payload = ExecutionCompletedPayload {
            execution_id: execution.id,
            action_id,
            action_ref: execution.action_ref.clone(),
            status: format!("{:?}", final_status),
            result: execution.result.clone(),
            completed_at: Utc::now(),
        };

        let envelope =
            MessageEnvelope::new(MessageType::ExecutionCompleted, payload).with_source("worker");

        publisher.publish_envelope(&envelope).await.map_err(|e| {
            Error::Internal(format!("Failed to publish completion notification: {}", e))
        })?;

        info!(
            "Completion notification published for execution {}",
            execution_id
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_queue_name_format() {
        let worker_id = 42;
        let queue_name = format!("worker.{}.executions", worker_id);
        assert_eq!(queue_name, "worker.42.executions");
    }

    #[test]
    fn test_status_string_conversion() {
        let status = ExecutionStatus::Running;
        let status_str = match status {
            ExecutionStatus::Running => "running",
            _ => "unknown",
        };
        assert_eq!(status_str, "running");
    }

    #[test]
    fn test_execution_completed_payload_structure() {
        let payload = ExecutionCompletedPayload {
            execution_id: 123,
            action_id: 456,
            action_ref: "test.action".to_string(),
            status: "Completed".to_string(),
            result: Some(serde_json::json!({"output": "test"})),
            completed_at: Utc::now(),
        };

        assert_eq!(payload.execution_id, 123);
        assert_eq!(payload.action_id, 456);
        assert_eq!(payload.action_ref, "test.action");
        assert_eq!(payload.status, "Completed");
        assert!(payload.result.is_some());
    }

    // Test removed - ExecutionStatusPayload struct doesn't exist
    // #[test]
    // fn test_execution_status_payload_structure() {
    //     ...
    // }

    #[test]
    fn test_execution_scheduled_payload_structure() {
        let payload = ExecutionScheduledPayload {
            execution_id: 111,
            action_ref: "core.test".to_string(),
            worker_id: 222,
        };

        assert_eq!(payload.execution_id, 111);
        assert_eq!(payload.action_ref, "core.test");
        assert_eq!(payload.worker_id, 222);
    }

    #[test]
    fn test_status_format_for_completion() {
        let status = ExecutionStatus::Completed;
        let status_str = format!("{:?}", status);
        assert_eq!(status_str, "Completed");

        let status = ExecutionStatus::Failed;
        let status_str = format!("{:?}", status);
        assert_eq!(status_str, "Failed");

        let status = ExecutionStatus::Timeout;
        let status_str = format!("{:?}", status);
        assert_eq!(status_str, "Timeout");

        let status = ExecutionStatus::Cancelled;
        let status_str = format!("{:?}", status);
        assert_eq!(status_str, "Cancelled");
    }
}
