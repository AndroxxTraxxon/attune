//! Dead Letter Handler
//!
//! This module handles messages that expire from worker queues and are routed to the
//! dead letter queue (DLQ). When a worker fails to process an execution request within
//! the configured TTL (default 5 minutes), the message is moved to the DLQ.
//!
//! The dead letter handler:
//! - Consumes messages from the dead letter queue
//! - Identifies the execution that expired
//! - Marks it as FAILED with appropriate error information
//! - Logs the failure for operational visibility

use attune_common::{
    error::Error,
    models::ExecutionStatus,
    mq::{Consumer, ConsumerConfig, MessageEnvelope, MessageType, MqResult},
    repositories::{execution::UpdateExecutionInput, ExecutionRepository, FindById},
};
use chrono::Utc;
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

/// Dead letter handler for processing expired messages
pub struct DeadLetterHandler {
    /// Database connection pool
    pool: Arc<PgPool>,
    /// Message consumer
    consumer: Consumer,
    /// Running state
    running: Arc<Mutex<bool>>,
}

impl DeadLetterHandler {
    /// Create a new dead letter handler
    pub async fn new(pool: Arc<PgPool>, consumer: Consumer) -> Result<Self, Error> {
        Ok(Self {
            pool,
            consumer,
            running: Arc::new(Mutex::new(false)),
        })
    }

    /// Start the dead letter handler
    pub async fn start(&self) -> Result<(), Error> {
        info!(
            "Starting dead letter handler for queue '{}'",
            self.consumer.queue()
        );

        {
            let mut running = self.running.lock().await;
            if *running {
                warn!("Dead letter handler already running");
                return Ok(());
            }
            *running = true;
        }

        let pool = Arc::clone(&self.pool);
        let running = Arc::clone(&self.running);

        // Start consuming messages
        let consumer_result = self
            .consumer
            .consume_with_handler(move |envelope: MessageEnvelope<serde_json::Value>| {
                let pool = Arc::clone(&pool);
                let running = Arc::clone(&running);

                async move {
                    // Check if we should continue processing
                    {
                        let is_running = running.lock().await;
                        if !*is_running {
                            info!("Dead letter handler stopping, rejecting message");
                            return Err(attune_common::mq::MqError::Consume(
                                "Handler is shutting down".to_string(),
                            ));
                        }
                    }

                    info!(
                        "Processing dead letter message {} of type {:?}",
                        envelope.message_id, envelope.message_type
                    );

                    match envelope.message_type {
                        MessageType::ExecutionRequested => {
                            handle_execution_requested(&pool, &envelope).await
                        }
                        _ => {
                            warn!(
                                "Received unexpected message type {:?} in DLQ: {}",
                                envelope.message_type, envelope.message_id
                            );
                            // Acknowledge unexpected messages to remove them from queue
                            Ok(())
                        }
                    }
                }
            })
            .await;

        {
            let mut running = self.running.lock().await;
            *running = false;
        }

        consumer_result.map_err(|e| {
            error!("Dead letter handler error: {}", e);
            Error::Internal(format!("Dead letter handler failed: {}", e))
        })
    }

    /// Stop the dead letter handler
    #[allow(dead_code)]
    pub async fn stop(&self) {
        info!("Stopping dead letter handler");
        let mut running = self.running.lock().await;
        *running = false;
    }

    /// Check if the handler is running
    #[allow(dead_code)]
    pub async fn is_running(&self) -> bool {
        *self.running.lock().await
    }
}

/// Handle an execution request that expired in a worker queue
async fn handle_execution_requested(
    pool: &PgPool,
    envelope: &MessageEnvelope<serde_json::Value>,
) -> MqResult<()> {
    debug!(
        "Handling expired ExecutionRequested message: {}",
        envelope.message_id
    );

    // Extract execution ID from payload
    let execution_id = match envelope.payload.get("execution_id") {
        Some(id) => match id.as_i64() {
            Some(id) => id,
            None => {
                error!("Invalid execution_id in payload: not an i64");
                return Ok(()); // Acknowledge to remove from queue
            }
        },
        None => {
            error!("Missing execution_id in ExecutionRequested payload");
            return Ok(()); // Acknowledge to remove from queue
        }
    };

    info!(
        "Failing execution {} due to worker queue expiration",
        execution_id
    );

    // Fetch current execution state
    let execution = match ExecutionRepository::find_by_id(pool, execution_id).await {
        Ok(Some(exec)) => exec,
        Ok(None) => {
            warn!(
                "Execution {} not found in database, may have been already processed",
                execution_id
            );
            return Ok(()); // Acknowledge to remove from queue
        }
        Err(e) => {
            error!("Failed to fetch execution {}: {}", execution_id, e);
            // Return error to nack and potentially retry
            return Err(attune_common::mq::MqError::Consume(format!(
                "Database error: {}",
                e
            )));
        }
    };

    // Only scheduled executions are still legitimately owned by the scheduler.
    // If the execution already moved to running or a terminal state, this DLQ
    // delivery is stale and must not overwrite newer state.
    if execution.status != ExecutionStatus::Scheduled {
        info!(
            "Execution {} already left Scheduled state ({:?}), skipping stale DLQ handling",
            execution_id, execution.status
        );
        return Ok(()); // Acknowledge to remove from queue
    }

    // Get worker info from payload for better error message
    let worker_id = envelope.payload.get("worker_id").and_then(|v| v.as_i64());
    let scheduled_attempt_updated_at = envelope
        .payload
        .get("scheduled_attempt_updated_at")
        .and_then(|v| v.as_str())
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    let error_message = if let Some(wid) = worker_id {
        format!(
            "Execution expired in worker queue (worker_id: {}). Worker did not process the execution within the configured TTL. This typically indicates the worker is unavailable or overloaded.",
            wid
        )
    } else {
        "Execution expired in worker queue. Worker did not process the execution within the configured TTL.".to_string()
    };

    // Update execution to failed
    let update_input = UpdateExecutionInput {
        status: Some(ExecutionStatus::Failed),
        result: Some(json!({
            "error": "Worker queue TTL expired",
            "message": error_message,
            "expired_at": Utc::now().to_rfc3339(),
        })),
        ..Default::default()
    };

    if let Some(timestamp) = scheduled_attempt_updated_at {
        // Guard on both status and the exact updated_at from when the execution was
        // scheduled — prevents overwriting state that changed after this DLQ message
        // was enqueued.
        match ExecutionRepository::update_if_status_and_updated_at(
            pool,
            execution_id,
            ExecutionStatus::Scheduled,
            timestamp,
            update_input,
        )
        .await
        {
            Ok(Some(_)) => {
                info!(
                    "Successfully failed execution {} due to worker queue expiration",
                    execution_id
                );
                Ok(())
            }
            Ok(None) => {
                info!(
                    "Skipping DLQ failure for execution {} because it already left Scheduled state",
                    execution_id
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    "Failed to update execution {} to failed state: {}",
                    execution_id, e
                );
                Err(attune_common::mq::MqError::Consume(format!(
                    "Failed to update execution: {}",
                    e
                )))
            }
        }
    } else {
        // Fallback for DLQ messages that predate the scheduled_attempt_updated_at
        // field. Use a status-only guard — same safety guarantee as the original code
        // (never overwrites terminal or running state).
        warn!(
            "DLQ message for execution {} lacks scheduled_attempt_updated_at; \
             falling back to status-only guard",
            execution_id
        );
        match ExecutionRepository::update_if_status(
            pool,
            execution_id,
            ExecutionStatus::Scheduled,
            update_input,
        )
        .await
        {
            Ok(Some(_)) => {
                info!(
                    "Successfully failed execution {} due to worker queue expiration (status-only guard)",
                    execution_id
                );
                Ok(())
            }
            Ok(None) => {
                info!(
                    "Skipping DLQ failure for execution {} because it already left Scheduled state",
                    execution_id
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    "Failed to update execution {} to failed state: {}",
                    execution_id, e
                );
                Err(attune_common::mq::MqError::Consume(format!(
                    "Failed to update execution: {}",
                    e
                )))
            }
        }
    }
}

/// Create a dead letter consumer configuration
pub fn create_dlq_consumer_config(dlq_name: &str, consumer_tag: &str) -> ConsumerConfig {
    ConsumerConfig {
        queue: dlq_name.to_string(),
        tag: consumer_tag.to_string(),
        prefetch_count: 10,
        auto_ack: false, // Manual ack for reliability
        exclusive: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_dlq_consumer_config() {
        let config = create_dlq_consumer_config("attune.dlx.queue", "dlq-handler");
        assert_eq!(config.queue, "attune.dlx.queue");
        assert_eq!(config.tag, "dlq-handler");
        assert_eq!(config.prefetch_count, 10);
        assert!(!config.auto_ack);
        assert!(!config.exclusive);
    }
}
