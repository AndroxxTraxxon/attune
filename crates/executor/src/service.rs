//! Executor Service - Core orchestration and execution management
//!
//! The ExecutorService is the central component that:
//! - Processes enforcement messages from triggered rules
//! - Schedules executions to workers
//! - Manages execution lifecycle and state transitions
//! - Enforces execution policies (rate limiting, concurrency)
//! - Orchestrates workflows (parent-child executions)
//! - Handles human-in-the-loop inquiries

use anyhow::Result;
use attune_common::{
    config::Config,
    db::Database,
    mq::{Connection, Consumer, MessageQueueConfig, Publisher},
};
use sqlx::PgPool;
use std::sync::Arc;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

use crate::completion_listener::CompletionListener;
use crate::enforcement_processor::EnforcementProcessor;
use crate::event_processor::EventProcessor;
use crate::execution_manager::ExecutionManager;
use crate::inquiry_handler::InquiryHandler;
use crate::policy_enforcer::PolicyEnforcer;
use crate::queue_manager::{ExecutionQueueManager, QueueConfig};
use crate::scheduler::ExecutionScheduler;

/// Main executor service that orchestrates execution processing
#[derive(Clone)]
pub struct ExecutorService {
    /// Shared internal state
    inner: Arc<ExecutorServiceInner>,
}

/// Internal state for the executor service
struct ExecutorServiceInner {
    /// Database connection pool
    pool: PgPool,

    /// Configuration
    config: Arc<Config>,

    /// Message queue connection
    mq_connection: Arc<Connection>,

    /// Message queue publisher
    /// Publisher for sending messages
    publisher: Arc<Publisher>,

    /// Queue name for consumers
    #[allow(dead_code)]
    queue_name: String,

    /// Message queue configuration
    mq_config: Arc<MessageQueueConfig>,

    /// Policy enforcer for execution policies
    policy_enforcer: Arc<PolicyEnforcer>,

    /// Queue manager for FIFO execution ordering
    queue_manager: Arc<ExecutionQueueManager>,

    /// Service shutdown signal
    shutdown_tx: tokio::sync::broadcast::Sender<()>,
}

impl ExecutorService {
    /// Create a new executor service
    pub async fn new(config: Config) -> Result<Self> {
        info!("Initializing Executor Service");

        // Initialize database
        let db = Database::new(&config.database).await?;
        let pool = db.pool().clone();
        info!("Database connection established");

        // Get message queue URL
        let mq_url = config
            .message_queue
            .as_ref()
            .map(|mq| mq.url.as_str())
            .ok_or_else(|| anyhow::anyhow!("Message queue configuration is required"))?;

        // Initialize message queue connection
        let mq_connection = Connection::connect(mq_url).await?;
        info!("Message queue connection established");

        // Setup common message queue infrastructure (exchanges and DLX)
        let mq_config = MessageQueueConfig::default();
        match mq_connection.setup_common_infrastructure(&mq_config).await {
            Ok(_) => info!("Common message queue infrastructure setup completed"),
            Err(e) => {
                warn!(
                    "Failed to setup common MQ infrastructure (may already exist): {}",
                    e
                );
            }
        }

        // Setup executor-specific queues and bindings
        match mq_connection
            .setup_executor_infrastructure(&mq_config)
            .await
        {
            Ok(_) => info!("Executor message queue infrastructure setup completed"),
            Err(e) => {
                warn!(
                    "Failed to setup executor MQ infrastructure (may already exist): {}",
                    e
                );
            }
        }

        // Get queue names from MqConfig
        let enforcements_queue = mq_config.rabbitmq.queues.enforcements.name.clone();
        let execution_requests_queue = mq_config.rabbitmq.queues.execution_requests.name.clone();
        let execution_status_queue = mq_config.rabbitmq.queues.execution_status.name.clone();
        let exchange_name = mq_config.rabbitmq.exchanges.executions.name.clone();

        // Initialize message queue publisher
        let publisher = Publisher::new(
            &mq_connection,
            attune_common::mq::PublisherConfig {
                confirm_publish: true,
                timeout_secs: 30,
                exchange: exchange_name,
            },
        )
        .await?;
        info!("Message queue publisher initialized");

        info!(
            "Queue names - Enforcements: {}, Execution Requests: {}, Execution Status: {}",
            enforcements_queue, execution_requests_queue, execution_status_queue
        );

        // Create shutdown channel
        let (shutdown_tx, _) = tokio::sync::broadcast::channel(1);

        // Initialize queue manager with default configuration and database pool
        let queue_config = QueueConfig::default();
        let queue_manager = Arc::new(ExecutionQueueManager::with_db_pool(
            queue_config,
            pool.clone(),
        ));
        info!("Queue manager initialized with database persistence");

        // Initialize policy enforcer with queue manager
        let policy_enforcer = Arc::new(PolicyEnforcer::with_queue_manager(
            pool.clone(),
            queue_manager.clone(),
        ));
        info!("Policy enforcer initialized with queue manager");

        let inner = ExecutorServiceInner {
            pool,
            config: Arc::new(config),
            mq_connection: Arc::new(mq_connection),
            publisher: Arc::new(publisher),
            queue_name: execution_requests_queue.clone(), // Keep for backward compatibility
            policy_enforcer,
            queue_manager,
            shutdown_tx,
            mq_config: Arc::new(mq_config),
        };

        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Start the executor service
    pub async fn start(&self) -> Result<()> {
        info!("Starting Executor Service");

        // Spawn message consumers
        let mut handles: Vec<JoinHandle<Result<()>>> = Vec::new();

        // Start event processor with its own consumer
        info!("Starting event processor...");
        let events_queue = self.inner.mq_config.rabbitmq.queues.events.name.clone();
        let event_consumer = Consumer::new(
            &self.inner.mq_connection,
            attune_common::mq::ConsumerConfig {
                queue: events_queue,
                tag: "executor.event".to_string(),
                prefetch_count: 10,
                auto_ack: false,
                exclusive: false,
            },
        )
        .await?;
        let event_processor = EventProcessor::new(
            self.inner.pool.clone(),
            self.inner.publisher.clone(),
            Arc::new(event_consumer),
        );
        handles.push(tokio::spawn(async move { event_processor.start().await }));

        // Start completion listener with its own consumer
        info!("Starting completion listener...");
        let execution_completed_queue = self
            .inner
            .mq_config
            .rabbitmq
            .queues
            .execution_completed
            .name
            .clone();
        let completion_consumer = Consumer::new(
            &self.inner.mq_connection,
            attune_common::mq::ConsumerConfig {
                queue: execution_completed_queue,
                tag: "executor.completion".to_string(),
                prefetch_count: 10,
                auto_ack: false,
                exclusive: false,
            },
        )
        .await?;
        let completion_listener = CompletionListener::new(
            self.inner.pool.clone(),
            Arc::new(completion_consumer),
            self.inner.publisher.clone(),
            self.inner.queue_manager.clone(),
        );
        handles.push(tokio::spawn(
            async move { completion_listener.start().await },
        ));

        // Start enforcement processor with its own consumer
        info!("Starting enforcement processor...");
        let enforcements_queue = self
            .inner
            .mq_config
            .rabbitmq
            .queues
            .enforcements
            .name
            .clone();
        let enforcement_consumer = Consumer::new(
            &self.inner.mq_connection,
            attune_common::mq::ConsumerConfig {
                queue: enforcements_queue,
                tag: "executor.enforcement".to_string(),
                prefetch_count: 10,
                auto_ack: false,
                exclusive: false,
            },
        )
        .await?;
        let enforcement_processor = EnforcementProcessor::new(
            self.inner.pool.clone(),
            self.inner.publisher.clone(),
            Arc::new(enforcement_consumer),
            self.inner.policy_enforcer.clone(),
            self.inner.queue_manager.clone(),
        );
        handles.push(tokio::spawn(
            async move { enforcement_processor.start().await },
        ));

        // Start execution scheduler with its own consumer
        info!("Starting execution scheduler...");
        let execution_requests_queue = self
            .inner
            .mq_config
            .rabbitmq
            .queues
            .execution_requests
            .name
            .clone();
        let scheduler_consumer = Consumer::new(
            &self.inner.mq_connection,
            attune_common::mq::ConsumerConfig {
                queue: execution_requests_queue,
                tag: "executor.scheduler".to_string(),
                prefetch_count: 10,
                auto_ack: false,
                exclusive: false,
            },
        )
        .await?;
        let scheduler = ExecutionScheduler::new(
            self.inner.pool.clone(),
            self.inner.publisher.clone(),
            Arc::new(scheduler_consumer),
        );
        handles.push(tokio::spawn(async move { scheduler.start().await }));

        // Start execution manager with its own consumer
        info!("Starting execution manager...");
        let execution_status_queue = self
            .inner
            .mq_config
            .rabbitmq
            .queues
            .execution_status
            .name
            .clone();
        let manager_consumer = Consumer::new(
            &self.inner.mq_connection,
            attune_common::mq::ConsumerConfig {
                queue: execution_status_queue,
                tag: "executor.manager".to_string(),
                prefetch_count: 10,
                auto_ack: false,
                exclusive: false,
            },
        )
        .await?;
        let execution_manager = ExecutionManager::new(
            self.inner.pool.clone(),
            self.inner.publisher.clone(),
            Arc::new(manager_consumer),
        );
        handles.push(tokio::spawn(async move { execution_manager.start().await }));

        // Start inquiry handler with its own consumer
        info!("Starting inquiry handler...");
        let inquiry_response_queue = self
            .inner
            .mq_config
            .rabbitmq
            .queues
            .inquiry_responses
            .name
            .clone();
        let inquiry_consumer = Consumer::new(
            &self.inner.mq_connection,
            attune_common::mq::ConsumerConfig {
                queue: inquiry_response_queue,
                tag: "executor.inquiry".to_string(),
                prefetch_count: 10,
                auto_ack: false,
                exclusive: false,
            },
        )
        .await?;
        let inquiry_handler = InquiryHandler::new(
            self.inner.pool.clone(),
            self.inner.publisher.clone(),
            Arc::new(inquiry_consumer),
        );
        handles.push(tokio::spawn(async move { inquiry_handler.start().await }));

        // Start inquiry timeout checker
        info!("Starting inquiry timeout checker...");
        let timeout_pool = self.inner.pool.clone();
        handles.push(tokio::spawn(async move {
            InquiryHandler::timeout_check_loop(timeout_pool, 60).await;
            Ok(())
        }));

        info!("Executor Service started successfully");
        info!("All processors are listening for messages...");

        // Wait for shutdown signal
        let mut shutdown_rx = self.inner.shutdown_tx.subscribe();
        tokio::select! {
            _ = shutdown_rx.recv() => {
                info!("Shutdown signal received");
            }
            result = Self::wait_for_tasks(handles) => {
                match result {
                    Ok(_) => info!("All tasks completed"),
                    Err(e) => error!("Task error: {}", e),
                }
            }
        }

        Ok(())
    }

    /// Stop the executor service
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping Executor Service");

        // Send shutdown signal
        let _ = self.inner.shutdown_tx.send(());

        // Close message queue connection (will close publisher and consumer)
        self.inner.mq_connection.close().await?;

        // Close database connections
        self.inner.pool.close().await;

        info!("Executor Service stopped");

        Ok(())
    }

    /// Wait for all tasks to complete
    async fn wait_for_tasks(handles: Vec<JoinHandle<Result<()>>>) -> Result<()> {
        for handle in handles {
            if let Err(e) = handle.await {
                error!("Task panicked: {}", e);
            }
        }
        Ok(())
    }

    /// Get database pool reference
    #[allow(dead_code)]
    pub fn pool(&self) -> &PgPool {
        &self.inner.pool
    }

    /// Get config reference
    #[allow(dead_code)]
    pub fn config(&self) -> &Config {
        &self.inner.config
    }

    /// Get publisher reference
    #[allow(dead_code)]
    pub fn publisher(&self) -> &Publisher {
        &self.inner.publisher
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires database and RabbitMQ
    async fn test_service_creation() {
        let config = Config::load().expect("Failed to load config");
        let service = ExecutorService::new(config).await;
        assert!(service.is_ok());
    }
}
