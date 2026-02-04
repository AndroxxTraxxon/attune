//! Execution Queue Manager - Manages FIFO queues for execution ordering
//!
//! This module provides guaranteed FIFO ordering for executions when policies
//! (concurrency limits, delays) are enforced. Each action has its own queue,
//! ensuring fair ordering and deterministic behavior.
//!
//! Key features:
//! - One FIFO queue per action_id
//! - Tokio Notify for efficient async waiting
//! - Thread-safe with DashMap
//! - Queue statistics for monitoring
//! - Configurable queue limits and timeouts

use anyhow::Result;
use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::{Mutex, Notify};
use tokio::time::{timeout, Duration};
use tracing::{debug, info, warn};

use attune_common::models::Id;
use attune_common::repositories::queue_stats::{QueueStatsRepository, UpsertQueueStatsInput};

/// Configuration for the queue manager
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueConfig {
    /// Maximum number of executions that can be queued per action
    pub max_queue_length: usize,
    /// Maximum time an execution can wait in queue (seconds)
    pub queue_timeout_seconds: u64,
    /// Whether to collect and expose queue metrics
    pub enable_metrics: bool,
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            max_queue_length: 10000,
            queue_timeout_seconds: 3600, // 1 hour
            enable_metrics: true,
        }
    }
}

/// Entry in the execution queue
#[derive(Debug)]
struct QueueEntry {
    /// Execution or enforcement ID being queued
    execution_id: Id,
    /// When this entry was added to the queue
    enqueued_at: DateTime<Utc>,
    /// Notifier to wake up this specific waiter
    notifier: Arc<Notify>,
}

/// Queue state for a single action
struct ActionQueue {
    /// FIFO queue of waiting executions
    queue: VecDeque<QueueEntry>,
    /// Number of currently active (running) executions
    active_count: u32,
    /// Maximum number of concurrent executions allowed
    max_concurrent: u32,
    /// Total number of executions that have been enqueued
    total_enqueued: u64,
    /// Total number of executions that have completed
    total_completed: u64,
}

impl ActionQueue {
    fn new(max_concurrent: u32) -> Self {
        Self {
            queue: VecDeque::new(),
            active_count: 0,
            max_concurrent,
            total_enqueued: 0,
            total_completed: 0,
        }
    }

    /// Check if there's capacity to run another execution
    fn has_capacity(&self) -> bool {
        self.active_count < self.max_concurrent
    }

    /// Check if queue is at capacity
    fn is_full(&self, max_queue_length: usize) -> bool {
        self.queue.len() >= max_queue_length
    }
}

/// Statistics about a queue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueStats {
    /// Action ID
    pub action_id: Id,
    /// Number of executions waiting in queue
    pub queue_length: usize,
    /// Number of currently running executions
    pub active_count: u32,
    /// Maximum concurrent executions allowed
    pub max_concurrent: u32,
    /// Timestamp of oldest queued execution (if any)
    pub oldest_enqueued_at: Option<DateTime<Utc>>,
    /// Total enqueued since queue creation
    pub total_enqueued: u64,
    /// Total completed since queue creation
    pub total_completed: u64,
}

/// Manages execution queues with FIFO ordering guarantees
pub struct ExecutionQueueManager {
    /// Per-action queues (key: action_id)
    queues: DashMap<Id, Arc<Mutex<ActionQueue>>>,
    /// Configuration
    config: QueueConfig,
    /// Database connection pool (optional for stats persistence)
    db_pool: Option<PgPool>,
}

impl ExecutionQueueManager {
    /// Create a new execution queue manager
    #[allow(dead_code)]
    pub fn new(config: QueueConfig) -> Self {
        Self {
            queues: DashMap::new(),
            config,
            db_pool: None,
        }
    }

    /// Create a new execution queue manager with database persistence
    pub fn with_db_pool(config: QueueConfig, db_pool: PgPool) -> Self {
        Self {
            queues: DashMap::new(),
            config,
            db_pool: Some(db_pool),
        }
    }

    /// Create with default configuration
    #[allow(dead_code)]
    pub fn with_defaults() -> Self {
        Self::new(QueueConfig::default())
    }

    /// Enqueue an execution and wait until it can proceed
    ///
    /// This method will:
    /// 1. Check if there's capacity to run immediately
    /// 2. If not, add to FIFO queue and wait for notification
    /// 3. Return when execution can proceed
    /// 4. Increment active count
    ///
    /// # Arguments
    /// * `action_id` - The action being executed
    /// * `execution_id` - The execution/enforcement ID
    /// * `max_concurrent` - Maximum concurrent executions for this action
    ///
    /// # Returns
    /// * `Ok(())` - Execution can proceed
    /// * `Err(_)` - Queue full or timeout
    pub async fn enqueue_and_wait(
        &self,
        action_id: Id,
        execution_id: Id,
        max_concurrent: u32,
    ) -> Result<()> {
        debug!(
            "Enqueuing execution {} for action {} (max_concurrent: {})",
            execution_id, action_id, max_concurrent
        );

        // Get or create queue for this action
        let queue_arc = self
            .queues
            .entry(action_id)
            .or_insert_with(|| Arc::new(Mutex::new(ActionQueue::new(max_concurrent))))
            .clone();

        // Create notifier for this execution
        let notifier = Arc::new(Notify::new());

        // Try to enqueue
        {
            let mut queue = queue_arc.lock().await;

            // Update max_concurrent if it changed
            queue.max_concurrent = max_concurrent;

            // Check if we can run immediately
            if queue.has_capacity() {
                debug!(
                    "Execution {} can run immediately (active: {}/{})",
                    execution_id, queue.active_count, queue.max_concurrent
                );
                queue.active_count += 1;
                queue.total_enqueued += 1;

                // Persist stats to database if available
                drop(queue);
                self.persist_queue_stats(action_id).await;

                return Ok(());
            }

            // Check if queue is full
            if queue.is_full(self.config.max_queue_length) {
                warn!(
                    "Queue full for action {}: {} entries (limit: {})",
                    action_id,
                    queue.queue.len(),
                    self.config.max_queue_length
                );
                return Err(anyhow::anyhow!(
                    "Queue full for action {}: maximum {} entries",
                    action_id,
                    self.config.max_queue_length
                ));
            }

            // Add to queue
            let entry = QueueEntry {
                execution_id,
                enqueued_at: Utc::now(),
                notifier: notifier.clone(),
            };

            queue.queue.push_back(entry);
            queue.total_enqueued += 1;

            info!(
                "Execution {} queued for action {} at position {} (active: {}/{})",
                execution_id,
                action_id,
                queue.queue.len() - 1,
                queue.active_count,
                queue.max_concurrent
            );
        }

        // Persist stats to database if available
        self.persist_queue_stats(action_id).await;

        // Wait for notification with timeout
        let wait_duration = Duration::from_secs(self.config.queue_timeout_seconds);

        match timeout(wait_duration, notifier.notified()).await {
            Ok(_) => {
                debug!("Execution {} notified, can proceed", execution_id);
                Ok(())
            }
            Err(_) => {
                // Timeout - remove from queue
                let mut queue = queue_arc.lock().await;
                queue.queue.retain(|e| e.execution_id != execution_id);

                warn!(
                    "Execution {} timed out after {} seconds in queue",
                    execution_id, self.config.queue_timeout_seconds
                );

                Err(anyhow::anyhow!(
                    "Queue timeout for execution {}: waited {} seconds",
                    execution_id,
                    self.config.queue_timeout_seconds
                ))
            }
        }
    }

    /// Notify that an execution has completed, releasing a queue slot
    ///
    /// This method will:
    /// 1. Decrement active count for the action
    /// 2. Check if there are queued executions
    /// 3. Notify the first (oldest) queued execution
    /// 4. Increment active count for the notified execution
    ///
    /// # Arguments
    /// * `action_id` - The action that completed
    ///
    /// # Returns
    /// * `Ok(true)` - A queued execution was notified
    /// * `Ok(false)` - No executions were waiting
    /// * `Err(_)` - Error accessing queue
    pub async fn notify_completion(&self, action_id: Id) -> Result<bool> {
        debug!(
            "Processing completion notification for action {}",
            action_id
        );

        // Get queue for this action
        let queue_arc = match self.queues.get(&action_id) {
            Some(q) => q.clone(),
            None => {
                debug!(
                    "No queue found for action {} (no executions queued)",
                    action_id
                );
                return Ok(false);
            }
        };

        let mut queue = queue_arc.lock().await;

        // Decrement active count
        if queue.active_count > 0 {
            queue.active_count -= 1;
            queue.total_completed += 1;
            debug!(
                "Decremented active count for action {} to {}",
                action_id, queue.active_count
            );
        } else {
            warn!(
                "Completion notification for action {} but active_count is 0",
                action_id
            );
        }

        // Check if there are queued executions
        if queue.queue.is_empty() {
            debug!(
                "No executions queued for action {} after completion",
                action_id
            );
            return Ok(false);
        }

        // Pop the first (oldest) entry from queue
        if let Some(entry) = queue.queue.pop_front() {
            info!(
                "Notifying execution {} for action {} (was queued for {:?})",
                entry.execution_id,
                action_id,
                Utc::now() - entry.enqueued_at
            );

            // Increment active count for the execution we're about to notify
            queue.active_count += 1;

            // Notify the waiter (after releasing lock)
            drop(queue);
            entry.notifier.notify_one();

            // Persist stats to database if available
            self.persist_queue_stats(action_id).await;

            Ok(true)
        } else {
            // Race condition check - queue was empty after all
            Ok(false)
        }
    }

    /// Persist queue statistics to database (if database pool is available)
    async fn persist_queue_stats(&self, action_id: Id) {
        if let Some(ref pool) = self.db_pool {
            if let Some(stats) = self.get_queue_stats(action_id).await {
                let input = UpsertQueueStatsInput {
                    action_id: stats.action_id,
                    queue_length: stats.queue_length as i32,
                    active_count: stats.active_count as i32,
                    max_concurrent: stats.max_concurrent as i32,
                    oldest_enqueued_at: stats.oldest_enqueued_at,
                    total_enqueued: stats.total_enqueued as i64,
                    total_completed: stats.total_completed as i64,
                };

                if let Err(e) = QueueStatsRepository::upsert(pool, input).await {
                    warn!(
                        "Failed to persist queue stats for action {}: {}",
                        action_id, e
                    );
                }
            }
        }
    }

    /// Get statistics for a specific action's queue
    pub async fn get_queue_stats(&self, action_id: Id) -> Option<QueueStats> {
        let queue_arc = self.queues.get(&action_id)?.clone();
        let queue = queue_arc.lock().await;

        let oldest_enqueued_at = queue.queue.front().map(|e| e.enqueued_at);

        Some(QueueStats {
            action_id,
            queue_length: queue.queue.len(),
            active_count: queue.active_count,
            max_concurrent: queue.max_concurrent,
            oldest_enqueued_at,
            total_enqueued: queue.total_enqueued,
            total_completed: queue.total_completed,
        })
    }

    /// Get statistics for all queues
    #[allow(dead_code)]
    pub async fn get_all_queue_stats(&self) -> Vec<QueueStats> {
        let mut stats = Vec::new();

        for entry in self.queues.iter() {
            let action_id = *entry.key();
            let queue_arc = entry.value().clone();
            let queue = queue_arc.lock().await;

            let oldest_enqueued_at = queue.queue.front().map(|e| e.enqueued_at);

            stats.push(QueueStats {
                action_id,
                queue_length: queue.queue.len(),
                active_count: queue.active_count,
                max_concurrent: queue.max_concurrent,
                oldest_enqueued_at,
                total_enqueued: queue.total_enqueued,
                total_completed: queue.total_completed,
            });
        }

        stats
    }

    /// Cancel a queued execution
    ///
    /// Removes the execution from the queue if it's waiting.
    /// Does nothing if the execution is already running or not found.
    ///
    /// # Arguments
    /// * `action_id` - The action the execution belongs to
    /// * `execution_id` - The execution to cancel
    ///
    /// # Returns
    /// * `Ok(true)` - Execution was found and removed from queue
    /// * `Ok(false)` - Execution not found in queue
    #[allow(dead_code)]
    pub async fn cancel_execution(&self, action_id: Id, execution_id: Id) -> Result<bool> {
        debug!(
            "Attempting to cancel execution {} for action {}",
            execution_id, action_id
        );

        let queue_arc = match self.queues.get(&action_id) {
            Some(q) => q.clone(),
            None => return Ok(false),
        };

        let mut queue = queue_arc.lock().await;

        let initial_len = queue.queue.len();
        queue.queue.retain(|e| e.execution_id != execution_id);
        let removed = initial_len != queue.queue.len();

        if removed {
            info!("Cancelled execution {} from queue", execution_id);
        } else {
            debug!(
                "Execution {} not found in queue (may be running)",
                execution_id
            );
        }

        Ok(removed)
    }

    /// Clear all queues (for testing or emergency situations)
    #[allow(dead_code)]
    pub async fn clear_all_queues(&self) {
        warn!("Clearing all execution queues");

        for entry in self.queues.iter() {
            let queue_arc = entry.value().clone();
            let mut queue = queue_arc.lock().await;
            queue.queue.clear();
            queue.active_count = 0;
        }
    }

    /// Get the number of actions with active queues
    #[allow(dead_code)]
    pub fn active_queue_count(&self) -> usize {
        self.queues.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_queue_manager_creation() {
        let manager = ExecutionQueueManager::with_defaults();
        assert_eq!(manager.active_queue_count(), 0);
    }

    #[tokio::test]
    async fn test_immediate_execution_with_capacity() {
        let manager = ExecutionQueueManager::with_defaults();

        // Should execute immediately when there's capacity
        let result = manager.enqueue_and_wait(1, 100, 2).await;
        assert!(result.is_ok());

        // Check stats
        let stats = manager.get_queue_stats(1).await.unwrap();
        assert_eq!(stats.active_count, 1);
        assert_eq!(stats.queue_length, 0);
    }

    #[tokio::test]
    async fn test_fifo_ordering() {
        let manager = Arc::new(ExecutionQueueManager::with_defaults());
        let action_id = 1;
        let max_concurrent = 1;

        // First execution should run immediately
        let result = manager
            .enqueue_and_wait(action_id, 100, max_concurrent)
            .await;
        assert!(result.is_ok());

        // Spawn three more executions that should queue
        let mut handles = vec![];
        let execution_order = Arc::new(Mutex::new(Vec::new()));

        for exec_id in 101..=103 {
            let manager = manager.clone();
            let order = execution_order.clone();

            let handle = tokio::spawn(async move {
                manager
                    .enqueue_and_wait(action_id, exec_id, max_concurrent)
                    .await
                    .unwrap();
                order.lock().await.push(exec_id);
            });

            handles.push(handle);
        }

        // Give tasks time to queue
        sleep(Duration::from_millis(100)).await;

        // Verify they're queued
        let stats = manager.get_queue_stats(action_id).await.unwrap();
        assert_eq!(stats.queue_length, 3);
        assert_eq!(stats.active_count, 1);

        // Release them one by one
        for _ in 0..3 {
            sleep(Duration::from_millis(50)).await;
            manager.notify_completion(action_id).await.unwrap();
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
    async fn test_completion_notification() {
        let manager = ExecutionQueueManager::with_defaults();
        let action_id = 1;

        // Start first execution
        manager.enqueue_and_wait(action_id, 100, 1).await.unwrap();

        // Queue second execution
        let manager_clone = Arc::new(manager);
        let manager_ref = manager_clone.clone();

        let handle = tokio::spawn(async move {
            manager_ref
                .enqueue_and_wait(action_id, 101, 1)
                .await
                .unwrap();
        });

        // Give it time to queue
        sleep(Duration::from_millis(100)).await;

        // Verify it's queued
        let stats = manager_clone.get_queue_stats(action_id).await.unwrap();
        assert_eq!(stats.queue_length, 1);
        assert_eq!(stats.active_count, 1);

        // Notify completion
        let notified = manager_clone.notify_completion(action_id).await.unwrap();
        assert!(notified);

        // Wait for queued execution to proceed
        handle.await.unwrap();

        // Verify stats
        let stats = manager_clone.get_queue_stats(action_id).await.unwrap();
        assert_eq!(stats.queue_length, 0);
        assert_eq!(stats.active_count, 1);
    }

    #[tokio::test]
    async fn test_multiple_actions_independent() {
        let manager = Arc::new(ExecutionQueueManager::with_defaults());

        // Start executions on different actions
        manager.enqueue_and_wait(1, 100, 1).await.unwrap();
        manager.enqueue_and_wait(2, 200, 1).await.unwrap();

        // Both should be active
        let stats1 = manager.get_queue_stats(1).await.unwrap();
        let stats2 = manager.get_queue_stats(2).await.unwrap();

        assert_eq!(stats1.active_count, 1);
        assert_eq!(stats2.active_count, 1);

        // Completion on action 1 shouldn't affect action 2
        manager.notify_completion(1).await.unwrap();

        let stats1 = manager.get_queue_stats(1).await.unwrap();
        let stats2 = manager.get_queue_stats(2).await.unwrap();

        assert_eq!(stats1.active_count, 0);
        assert_eq!(stats2.active_count, 1);
    }

    #[tokio::test]
    async fn test_cancel_execution() {
        let manager = ExecutionQueueManager::with_defaults();
        let action_id = 1;

        // Fill capacity
        manager.enqueue_and_wait(action_id, 100, 1).await.unwrap();

        // Queue more executions
        let manager_arc = Arc::new(manager);
        let manager_ref = manager_arc.clone();

        let handle = tokio::spawn(async move {
            let result = manager_ref.enqueue_and_wait(action_id, 101, 1).await;
            result
        });

        // Give it time to queue
        sleep(Duration::from_millis(100)).await;

        // Cancel the queued execution
        let cancelled = manager_arc.cancel_execution(action_id, 101).await.unwrap();
        assert!(cancelled);

        // Verify queue is empty
        let stats = manager_arc.get_queue_stats(action_id).await.unwrap();
        assert_eq!(stats.queue_length, 0);

        // The handle should complete with an error eventually
        // (it will timeout or the task will be dropped)
        drop(handle);
    }

    #[tokio::test]
    async fn test_queue_stats() {
        let manager = ExecutionQueueManager::with_defaults();
        let action_id = 1;

        // Initially no stats
        assert!(manager.get_queue_stats(action_id).await.is_none());

        // After enqueue, stats should exist
        manager.enqueue_and_wait(action_id, 100, 2).await.unwrap();

        let stats = manager.get_queue_stats(action_id).await.unwrap();
        assert_eq!(stats.action_id, action_id);
        assert_eq!(stats.active_count, 1);
        assert_eq!(stats.max_concurrent, 2);
        assert_eq!(stats.total_enqueued, 1);
    }

    #[tokio::test]
    async fn test_queue_full() {
        let config = QueueConfig {
            max_queue_length: 2,
            queue_timeout_seconds: 60,
            enable_metrics: true,
        };

        let manager = Arc::new(ExecutionQueueManager::new(config));
        let action_id = 1;

        // Fill capacity
        manager.enqueue_and_wait(action_id, 100, 1).await.unwrap();

        // Queue 2 more (should reach limit)
        let manager_ref = manager.clone();
        tokio::spawn(async move {
            manager_ref
                .enqueue_and_wait(action_id, 101, 1)
                .await
                .unwrap();
        });

        let manager_ref = manager.clone();
        tokio::spawn(async move {
            manager_ref
                .enqueue_and_wait(action_id, 102, 1)
                .await
                .unwrap();
        });

        sleep(Duration::from_millis(100)).await;

        // Next one should fail
        let result = manager.enqueue_and_wait(action_id, 103, 1).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Queue full"));
    }

    #[tokio::test]
    async fn test_high_concurrency_ordering() {
        let manager = Arc::new(ExecutionQueueManager::with_defaults());
        let action_id = 1;
        let num_executions = 100;
        let max_concurrent = 1;

        // Start first execution
        manager
            .enqueue_and_wait(action_id, 0, max_concurrent)
            .await
            .unwrap();

        let execution_order = Arc::new(Mutex::new(Vec::new()));
        let mut handles = vec![];

        // Spawn many concurrent enqueues
        for i in 1..num_executions {
            let manager = manager.clone();
            let order = execution_order.clone();

            let handle = tokio::spawn(async move {
                manager
                    .enqueue_and_wait(action_id, i, max_concurrent)
                    .await
                    .unwrap();
                order.lock().await.push(i);
            });

            handles.push(handle);
        }

        // Give time to queue
        sleep(Duration::from_millis(200)).await;

        // Release them all
        for _ in 0..num_executions {
            sleep(Duration::from_millis(10)).await;
            manager.notify_completion(action_id).await.unwrap();
        }

        // Wait for completion
        for handle in handles {
            handle.await.unwrap();
        }

        // Verify FIFO order
        let order = execution_order.lock().await;
        let expected: Vec<i64> = (1..num_executions).collect();
        assert_eq!(*order, expected);
    }
}
