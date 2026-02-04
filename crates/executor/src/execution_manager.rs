//! Execution Manager - Manages execution lifecycle and status transitions
//!
//! This module is responsible for:
//! - Listening for ExecutionStatusChanged messages
//! - Updating execution records in the database
//! - Managing workflow executions (parent-child relationships)
//! - Triggering child executions when parent completes
//! - Handling execution failures and retries
//! - Publishing status change notifications

use anyhow::Result;
use attune_common::{
    models::{enums::ExecutionStatus, Execution},
    mq::{
        Consumer, ExecutionCompletedPayload, ExecutionRequestedPayload,
        ExecutionStatusChangedPayload, MessageEnvelope, MessageType, Publisher,
    },
    repositories::{
        execution::{CreateExecutionInput, ExecutionRepository},
        Create, FindById, Update,
    },
};

use sqlx::PgPool;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

/// Execution manager that handles lifecycle and status updates
pub struct ExecutionManager {
    pool: PgPool,
    publisher: Arc<Publisher>,
    consumer: Arc<Consumer>,
}

impl ExecutionManager {
    /// Create a new execution manager
    pub fn new(pool: PgPool, publisher: Arc<Publisher>, consumer: Arc<Consumer>) -> Self {
        Self {
            pool,
            publisher,
            consumer,
        }
    }

    /// Start processing execution status messages
    pub async fn start(&self) -> Result<()> {
        info!("Starting execution manager");

        let pool = self.pool.clone();
        let publisher = self.publisher.clone();

        // Use the handler pattern to consume messages
        self.consumer
            .consume_with_handler(
                move |envelope: MessageEnvelope<ExecutionStatusChangedPayload>| {
                    let pool = pool.clone();
                    let publisher = publisher.clone();

                    async move {
                        if let Err(e) =
                            Self::process_status_change(&pool, &publisher, &envelope).await
                        {
                            error!("Error processing status change: {}", e);
                            // Return error to trigger nack with requeue
                            return Err(format!("Failed to process status change: {}", e).into());
                        }
                        Ok(())
                    }
                },
            )
            .await?;

        Ok(())
    }

    /// Process an execution status change message
    async fn process_status_change(
        pool: &PgPool,
        publisher: &Publisher,
        envelope: &MessageEnvelope<ExecutionStatusChangedPayload>,
    ) -> Result<()> {
        debug!("Processing execution status change: {:?}", envelope);

        let execution_id = envelope.payload.execution_id;
        let status_str = &envelope.payload.new_status;
        let status = Self::parse_execution_status(status_str)?;

        info!(
            "Processing status change for execution {}: {:?}",
            execution_id, status
        );

        // Fetch execution from database
        let mut execution = ExecutionRepository::find_by_id(pool, execution_id)
            .await?
            .ok_or_else(|| anyhow::anyhow!("Execution not found: {}", execution_id))?;

        // Update status
        let old_status = execution.status.clone();
        execution.status = status;

        // Note: ExecutionStatusChangedPayload doesn't contain result data
        // Results are only in ExecutionCompletedPayload

        // Update execution in database
        ExecutionRepository::update(pool, execution.id, execution.clone().into()).await?;

        info!(
            "Updated execution {} status: {:?} -> {:?}",
            execution_id, old_status, status
        );

        // Handle status-specific logic
        match status {
            ExecutionStatus::Completed | ExecutionStatus::Failed | ExecutionStatus::Cancelled => {
                Self::handle_completion(pool, publisher, &execution).await?;
            }
            _ => {}
        }

        Ok(())
    }

    /// Parse execution status from string
    fn parse_execution_status(status: &str) -> Result<ExecutionStatus> {
        match status.to_lowercase().as_str() {
            "requested" => Ok(ExecutionStatus::Requested),
            "scheduling" => Ok(ExecutionStatus::Scheduling),
            "scheduled" => Ok(ExecutionStatus::Scheduled),
            "running" => Ok(ExecutionStatus::Running),
            "completed" => Ok(ExecutionStatus::Completed),
            "failed" => Ok(ExecutionStatus::Failed),
            "cancelled" | "canceled" => Ok(ExecutionStatus::Cancelled),
            "canceling" => Ok(ExecutionStatus::Canceling),
            "abandoned" => Ok(ExecutionStatus::Abandoned),
            "timeout" => Ok(ExecutionStatus::Timeout),
            _ => Err(anyhow::anyhow!("Invalid execution status: {}", status)),
        }
    }

    /// Handle execution completion (success, failure, or cancellation)
    async fn handle_completion(
        pool: &PgPool,
        publisher: &Publisher,
        execution: &Execution,
    ) -> Result<()> {
        info!("Handling completion for execution: {}", execution.id);

        // Check if this execution has child executions to trigger
        if let Some(child_actions) = Self::get_child_actions(execution).await? {
            // Only trigger children on completion
            if execution.status == ExecutionStatus::Completed {
                Self::trigger_child_executions(pool, publisher, execution, &child_actions).await?;
            } else {
                warn!(
                    "Execution {} failed/canceled, skipping child executions",
                    execution.id
                );
            }
        }

        // Publish completion notification
        Self::publish_completion_notification(pool, publisher, execution).await?;

        Ok(())
    }

    /// Get child actions from execution result (for workflow orchestration)
    async fn get_child_actions(_execution: &Execution) -> Result<Option<Vec<String>>> {
        // TODO: Implement workflow logic
        // - Check if action has defined workflow
        // - Extract next actions from execution result
        // - Parse workflow definition

        // For now, return None (no child executions)
        Ok(None)
    }

    /// Trigger child executions for a completed parent
    async fn trigger_child_executions(
        pool: &PgPool,
        publisher: &Publisher,
        parent: &Execution,
        child_actions: &[String],
    ) -> Result<()> {
        info!(
            "Triggering {} child executions for parent: {}",
            child_actions.len(),
            parent.id
        );

        for action_ref in child_actions {
            let child_input = CreateExecutionInput {
                action: None,
                action_ref: action_ref.clone(),
                config: parent.config.clone(), // Pass parent config to child
                parent: Some(parent.id),       // Link to parent execution
                enforcement: parent.enforcement,
                executor: None, // Will be assigned during scheduling
                status: ExecutionStatus::Requested,
                result: None,
                workflow_task: None, // Non-workflow execution
            };

            let child_execution = ExecutionRepository::create(pool, child_input).await?;

            info!(
                "Created child execution {} for parent {}",
                child_execution.id, parent.id
            );

            // Publish ExecutionRequested message for child
            let payload = ExecutionRequestedPayload {
                execution_id: child_execution.id,
                action_id: None, // Child executions typically don't have action_id set yet
                action_ref: action_ref.clone(),
                parent_id: Some(parent.id),
                enforcement_id: None,
                config: None,
            };

            let envelope = MessageEnvelope::new(MessageType::ExecutionRequested, payload)
                .with_source("executor");

            publisher.publish_envelope(&envelope).await?;
        }

        Ok(())
    }

    /// Publish execution completion notification
    async fn publish_completion_notification(
        _pool: &PgPool,
        publisher: &Publisher,
        execution: &Execution,
    ) -> Result<()> {
        // Get action_id (required field)
        let action_id = execution
            .action
            .ok_or_else(|| anyhow::anyhow!("Execution {} has no action_id", execution.id))?;

        let payload = ExecutionCompletedPayload {
            execution_id: execution.id,
            action_id,
            action_ref: execution.action_ref.clone(),
            status: format!("{:?}", execution.status),
            result: execution.result.clone(),
            completed_at: chrono::Utc::now(),
        };

        let envelope =
            MessageEnvelope::new(MessageType::ExecutionCompleted, payload).with_source("executor");

        publisher.publish_envelope(&envelope).await?;

        info!(
            "Published execution.completed notification for execution: {}",
            execution.id
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_execution_manager_creation() {
        // This is a placeholder test
        // Real tests will require database and message queue setup
        assert!(true);
    }

    #[test]
    fn test_parse_execution_status() {
        // Mock pool, publisher, consumer for testing
        // In real tests, these would be properly initialized
    }
}
