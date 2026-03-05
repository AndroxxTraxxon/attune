//! Completion Listener - Handles execution completion notifications
//!
//! This module is responsible for:
//! - Listening for ExecutionCompleted messages from workers
//! - Releasing queue slots via QueueManager
//! - Updating execution status in database (if needed)
//! - Detecting inquiry requests in execution results
//! - Creating inquiries for human-in-the-loop workflows
//! - Enabling FIFO execution ordering by notifying waiting executions
//! - Advancing workflow orchestration when child task executions complete

use anyhow::Result;
use attune_common::{
    mq::{Consumer, ExecutionCompletedPayload, MessageEnvelope, Publisher},
    repositories::{execution::ExecutionRepository, FindById},
};
use sqlx::PgPool;
use std::sync::atomic::AtomicUsize;
use std::sync::Arc;
use tracing::{debug, error, info, warn};

use crate::{
    inquiry_handler::InquiryHandler, queue_manager::ExecutionQueueManager,
    scheduler::ExecutionScheduler,
};

/// Completion listener that handles execution completion messages
pub struct CompletionListener {
    pool: PgPool,
    consumer: Arc<Consumer>,
    publisher: Arc<Publisher>,
    queue_manager: Arc<ExecutionQueueManager>,
    /// Round-robin counter shared with the scheduler for dispatching workflow
    /// successor tasks to workers.
    round_robin_counter: Arc<AtomicUsize>,
}

impl CompletionListener {
    /// Create a new completion listener
    pub fn new(
        pool: PgPool,
        consumer: Arc<Consumer>,
        publisher: Arc<Publisher>,
        queue_manager: Arc<ExecutionQueueManager>,
    ) -> Self {
        Self {
            pool,
            consumer,
            publisher,
            queue_manager,
            round_robin_counter: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Start processing execution completed messages
    pub async fn start(&self) -> Result<()> {
        info!("Starting completion listener");

        let pool = self.pool.clone();
        let publisher = self.publisher.clone();
        let queue_manager = self.queue_manager.clone();
        let round_robin_counter = self.round_robin_counter.clone();

        // Use the handler pattern to consume messages
        self.consumer
            .consume_with_handler(
                move |envelope: MessageEnvelope<ExecutionCompletedPayload>| {
                    let pool = pool.clone();
                    let publisher = publisher.clone();
                    let queue_manager = queue_manager.clone();
                    let round_robin_counter = round_robin_counter.clone();

                    async move {
                        if let Err(e) = Self::process_execution_completed(
                            &pool,
                            &publisher,
                            &queue_manager,
                            &round_robin_counter,
                            &envelope,
                        )
                        .await
                        {
                            error!("Error processing execution completion: {}", e);
                            // Return error to trigger nack with requeue
                            return Err(
                                format!("Failed to process execution completion: {}", e).into()
                            );
                        }
                        Ok(())
                    }
                },
            )
            .await?;

        Ok(())
    }

    /// Process an execution completed message
    async fn process_execution_completed(
        pool: &PgPool,
        publisher: &Publisher,
        queue_manager: &ExecutionQueueManager,
        round_robin_counter: &AtomicUsize,
        envelope: &MessageEnvelope<ExecutionCompletedPayload>,
    ) -> Result<()> {
        debug!("Processing execution completed message: {:?}", envelope);

        let execution_id = envelope.payload.execution_id;
        let action_id = envelope.payload.action_id;

        info!(
            "Processing completion for execution: {} (action: {})",
            execution_id, action_id
        );

        // Verify execution exists in database
        let execution = ExecutionRepository::find_by_id(pool, execution_id).await?;

        if let Some(ref exec) = execution {
            debug!(
                "Execution {} found with status: {:?}",
                execution_id, exec.status
            );

            // Check if this execution is a workflow child task and advance the
            // workflow orchestration (schedule successor tasks or complete the
            // workflow).
            if exec.workflow_task.is_some() {
                info!(
                    "Execution {} is a workflow task, advancing workflow",
                    execution_id
                );
                if let Err(e) =
                    ExecutionScheduler::advance_workflow(pool, publisher, round_robin_counter, exec)
                        .await
                {
                    error!(
                        "Failed to advance workflow for execution {}: {}",
                        execution_id, e
                    );
                    // Continue processing — don't fail the entire completion
                }
            }

            // Check if execution result contains an inquiry request
            if let Some(result) = &exec.result {
                if InquiryHandler::has_inquiry_request(result) {
                    info!(
                        "Execution {} result contains inquiry request, creating inquiry",
                        execution_id
                    );

                    match InquiryHandler::create_inquiry_from_result(
                        pool,
                        publisher,
                        execution_id,
                        result,
                    )
                    .await
                    {
                        Ok(inquiry) => {
                            info!(
                                "Created inquiry {} for execution {}, execution paused for response",
                                inquiry.id, execution_id
                            );
                        }
                        Err(e) => {
                            error!(
                                "Failed to create inquiry for execution {}: {}",
                                execution_id, e
                            );
                            // Continue processing - don't fail the entire completion
                        }
                    }
                }
            }
        } else {
            warn!(
                "Execution {} not found in database, but still releasing queue slot",
                execution_id
            );
        }

        // Release queue slot for this action
        info!(
            "Releasing queue slot for action {} (execution {} completed)",
            action_id, execution_id
        );

        match queue_manager.notify_completion(action_id).await {
            Ok(notified) => {
                if notified {
                    info!(
                        "Queue slot released for action {}, next execution notified",
                        action_id
                    );
                } else {
                    debug!(
                        "Queue slot released for action {}, no executions waiting",
                        action_id
                    );
                }
            }
            Err(e) => {
                error!(
                    "Failed to release queue slot for action {}: {}",
                    action_id, e
                );
                return Err(e);
            }
        }

        // Get queue statistics for logging
        if let Some(stats) = queue_manager.get_queue_stats(action_id).await {
            debug!(
                "Queue stats for action {}: {} active, {} queued, {} total completed",
                action_id, stats.active_count, stats.queue_length, stats.total_completed
            );
        }

        info!(
            "Successfully processed completion for execution: {} (action: {})",
            execution_id, action_id
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue_manager::ExecutionQueueManager;

    #[tokio::test]
    async fn test_notify_completion_releases_slot() {
        let queue_manager = Arc::new(ExecutionQueueManager::with_defaults());
        let action_id = 1;

        // Simulate acquiring a slot
        queue_manager
            .enqueue_and_wait(action_id, 100, 1)
            .await
            .unwrap();

        // Verify slot is active
        let stats = queue_manager.get_queue_stats(action_id).await.unwrap();
        assert_eq!(stats.active_count, 1);
        assert_eq!(stats.queue_length, 0);

        // Simulate completion notification
        let notified = queue_manager.notify_completion(action_id).await.unwrap();
        assert!(!notified); // No one waiting

        // Verify slot is released
        let stats = queue_manager.get_queue_stats(action_id).await.unwrap();
        assert_eq!(stats.active_count, 0);
    }

    #[tokio::test]
    async fn test_notify_completion_wakes_waiting() {
        let queue_manager = Arc::new(ExecutionQueueManager::with_defaults());
        let action_id = 1;

        // Fill capacity
        queue_manager
            .enqueue_and_wait(action_id, 100, 1)
            .await
            .unwrap();

        // Queue another execution
        let queue_manager_clone = queue_manager.clone();
        let handle = tokio::spawn(async move {
            queue_manager_clone
                .enqueue_and_wait(action_id, 101, 1)
                .await
                .unwrap();
        });

        // Give it time to queue
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Verify one is queued
        let stats = queue_manager.get_queue_stats(action_id).await.unwrap();
        assert_eq!(stats.active_count, 1);
        assert_eq!(stats.queue_length, 1);

        // Notify completion
        let notified = queue_manager.notify_completion(action_id).await.unwrap();
        assert!(notified); // Should wake the waiting execution

        // Wait for queued execution to proceed
        handle.await.unwrap();

        // Verify stats
        let stats = queue_manager.get_queue_stats(action_id).await.unwrap();
        assert_eq!(stats.active_count, 1); // Second execution now active
        assert_eq!(stats.queue_length, 0);
        assert_eq!(stats.total_completed, 1);
    }

    #[tokio::test]
    async fn test_multiple_completions_fifo_order() {
        let queue_manager = Arc::new(ExecutionQueueManager::with_defaults());
        let action_id = 1;

        // Fill capacity
        queue_manager
            .enqueue_and_wait(action_id, 100, 1)
            .await
            .unwrap();

        // Queue multiple executions
        let execution_order = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let mut handles = vec![];

        for exec_id in 101..=103 {
            let queue_manager = queue_manager.clone();
            let order = execution_order.clone();

            let handle = tokio::spawn(async move {
                queue_manager
                    .enqueue_and_wait(action_id, exec_id, 1)
                    .await
                    .unwrap();
                order.lock().await.push(exec_id);
            });

            handles.push(handle);
        }

        // Give time to queue
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        // Release them one by one
        for _ in 0..3 {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            queue_manager.notify_completion(action_id).await.unwrap();
        }

        // Wait for all to complete
        for handle in handles {
            handle.await.unwrap();
        }

        // Verify FIFO order
        let order = execution_order.lock().await;
        assert_eq!(*order, vec![101, 102, 103]);
    }

    #[tokio::test]
    async fn test_completion_with_no_queue() {
        let queue_manager = Arc::new(ExecutionQueueManager::with_defaults());
        let action_id = 999; // Non-existent action

        // Should succeed but not notify anyone
        let result = queue_manager.notify_completion(action_id).await;
        assert!(result.is_ok());
        assert!(!result.unwrap());
    }
}
