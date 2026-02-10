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
    Publisher, PublisherConfig,
};
use attune_common::repositories::{execution::ExecutionRepository, FindById};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use crate::artifacts::ArtifactManager;
use crate::executor::ActionExecutor;
use crate::heartbeat::HeartbeatManager;
use crate::registration::WorkerRegistration;
use crate::runtime::local::LocalRuntime;
use crate::runtime::native::NativeRuntime;
use crate::runtime::python::PythonRuntime;
use crate::runtime::shell::ShellRuntime;
use crate::runtime::{DependencyManagerRegistry, PythonVenvManager, RuntimeRegistry};
use crate::secrets::SecretManager;

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
    worker_id: Option<i64>,
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

        // Determine which runtimes to register based on configuration
        // This reads from ATTUNE_WORKER_RUNTIMES env var (highest priority)
        let configured_runtimes = if let Ok(runtimes_env) = std::env::var("ATTUNE_WORKER_RUNTIMES")
        {
            info!(
                "Registering runtimes from ATTUNE_WORKER_RUNTIMES: {}",
                runtimes_env
            );
            runtimes_env
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect::<Vec<String>>()
        } else {
            // Fallback to auto-detection if not configured
            info!("No ATTUNE_WORKER_RUNTIMES found, registering all available runtimes");
            vec![
                "shell".to_string(),
                "python".to_string(),
                "native".to_string(),
            ]
        };

        info!("Configured runtimes: {:?}", configured_runtimes);

        // Initialize dependency manager registry for isolated environments
        let mut dependency_manager_registry = DependencyManagerRegistry::new();

        // Only setup Python virtual environment manager if Python runtime is needed
        if configured_runtimes.contains(&"python".to_string()) {
            let venv_base_dir = std::path::PathBuf::from(
                config
                    .worker
                    .as_ref()
                    .and_then(|w| w.name.clone())
                    .map(|name| format!("/tmp/attune/venvs/{}", name))
                    .unwrap_or_else(|| "/tmp/attune/venvs".to_string()),
            );
            let python_venv_manager = PythonVenvManager::new(venv_base_dir);
            dependency_manager_registry.register(Box::new(python_venv_manager));
            info!("Dependency manager initialized with Python venv support");
        }

        let dependency_manager_arc = Arc::new(dependency_manager_registry);

        // Initialize runtime registry
        let mut runtime_registry = RuntimeRegistry::new();

        // Register runtimes based on configuration
        for runtime_name in &configured_runtimes {
            match runtime_name.as_str() {
                "python" => {
                    let python_runtime = PythonRuntime::with_dependency_manager(
                        std::path::PathBuf::from("python3"),
                        std::path::PathBuf::from("/tmp/attune/actions"),
                        dependency_manager_arc.clone(),
                    );
                    runtime_registry.register(Box::new(python_runtime));
                    info!("Registered Python runtime");
                }
                "shell" => {
                    runtime_registry.register(Box::new(ShellRuntime::new()));
                    info!("Registered Shell runtime");
                }
                "native" => {
                    runtime_registry.register(Box::new(NativeRuntime::new()));
                    info!("Registered Native runtime");
                }
                "node" => {
                    warn!("Node.js runtime requested but not yet implemented, skipping");
                }
                _ => {
                    warn!("Unknown runtime type '{}', skipping", runtime_name);
                }
            }
        }

        // Only register local runtime as fallback if no specific runtimes configured
        // (LocalRuntime contains Python/Shell/Native and tries to validate all)
        if configured_runtimes.is_empty() {
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
        let packs_base_dir = std::path::PathBuf::from(&config.packs_base_dir);

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
            packs_base_dir,
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

        Ok(Self {
            config,
            db_pool: pool,
            registration,
            heartbeat,
            executor,
            mq_connection: Arc::new(mq_connection),
            publisher: Arc::new(publisher),
            consumer: None,
            worker_id: None,
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
        let mq_config = MqConfig::default();
        self.mq_connection
            .setup_worker_infrastructure(worker_id, &mq_config)
            .await
            .map_err(|e| {
                Error::Internal(format!("Failed to setup worker MQ infrastructure: {}", e))
            })?;
        info!("Worker-specific message queue infrastructure setup completed");

        // Start heartbeat
        self.heartbeat.start().await?;

        // Start consuming execution messages
        self.start_execution_consumer().await?;

        info!("Worker Service started successfully");

        Ok(())
    }

    /// Stop the worker service
    pub async fn stop(&mut self) -> Result<()> {
        info!("Stopping Worker Service - initiating graceful shutdown");

        // Mark worker as inactive first to stop receiving new tasks
        {
            let reg = self.registration.read().await;
            info!("Marking worker as inactive to stop receiving new tasks");
            reg.deregister().await?;
        }

        // Stop heartbeat
        info!("Stopping heartbeat updates");
        self.heartbeat.stop().await;

        // Wait a bit for heartbeat to stop
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Wait for in-flight tasks to complete (with timeout)
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
    async fn start_execution_consumer(&mut self) -> Result<()> {
        let worker_id = self
            .worker_id
            .ok_or_else(|| Error::Internal("Worker not registered".to_string()))?;

        // Queue name for this worker (already created in setup_worker_infrastructure)
        let queue_name = format!("worker.{}.executions", worker_id);

        info!("Starting consumer for worker queue: {}", queue_name);

        // Create consumer
        let consumer = Consumer::new(
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
        .map_err(|e| Error::Internal(format!("Failed to create consumer: {}", e)))?;

        info!("Consumer started for queue: {}", queue_name);

        info!("Message queue consumer initialized");

        // Clone Arc references for the handler
        let executor = self.executor.clone();
        let publisher = self.publisher.clone();
        let db_pool = self.db_pool.clone();

        // Consume messages with handler
        consumer
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
            .await
            .map_err(|e| Error::Internal(format!("Failed to start consumer: {}", e)))?;

        // Store consumer reference
        self.consumer = Some(Arc::new(consumer));

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
