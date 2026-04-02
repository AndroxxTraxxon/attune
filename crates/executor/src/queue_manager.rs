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
#[derive(Debug, Clone)]
struct QueueEntry {
    /// Execution or enforcement ID being queued
    execution_id: Id,
    /// When this entry was added to the queue
    enqueued_at: DateTime<Utc>,
    /// Notifier to wake up this specific waiter
    notifier: Arc<Notify>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct QueueKey {
    action_id: Id,
    group_key: Option<String>,
}

/// Queue state for a single action/group pair
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlotAcquireOutcome {
    pub acquired: bool,
    pub current_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SlotEnqueueOutcome {
    Acquired,
    Enqueued,
}

#[derive(Debug, Clone)]
pub struct SlotReleaseOutcome {
    pub next_execution_id: Option<Id>,
    queue_key: QueueKey,
}

#[derive(Debug, Clone)]
pub struct QueuedRemovalOutcome {
    pub next_execution_id: Option<Id>,
    queue_key: QueueKey,
    removed_entry: QueueEntry,
    removed_index: usize,
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
    /// Per-action/per-group queues.
    queues: DashMap<QueueKey, Arc<Mutex<ActionQueue>>>,
    /// Tracks which queue key currently owns an active execution slot.
    active_execution_keys: DashMap<Id, QueueKey>,
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
            active_execution_keys: DashMap::new(),
            config,
            db_pool: None,
        }
    }

    /// Create a new execution queue manager with database persistence
    pub fn with_db_pool(config: QueueConfig, db_pool: PgPool) -> Self {
        Self {
            queues: DashMap::new(),
            active_execution_keys: DashMap::new(),
            config,
            db_pool: Some(db_pool),
        }
    }

    /// Create with default configuration
    #[allow(dead_code)]
    pub fn with_defaults() -> Self {
        Self::new(QueueConfig::default())
    }

    fn queue_key(&self, action_id: Id, group_key: Option<String>) -> QueueKey {
        QueueKey {
            action_id,
            group_key,
        }
    }

    async fn get_or_create_queue(
        &self,
        queue_key: QueueKey,
        max_concurrent: u32,
    ) -> Arc<Mutex<ActionQueue>> {
        self.queues
            .entry(queue_key)
            .or_insert_with(|| Arc::new(Mutex::new(ActionQueue::new(max_concurrent))))
            .clone()
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
    #[allow(dead_code)]
    pub async fn enqueue_and_wait(
        &self,
        action_id: Id,
        execution_id: Id,
        max_concurrent: u32,
        group_key: Option<String>,
    ) -> Result<()> {
        if self.active_execution_keys.contains_key(&execution_id) {
            debug!(
                "Execution {} already owns an active slot, skipping queue wait",
                execution_id
            );
            return Ok(());
        }

        debug!(
            "Enqueuing execution {} for action {} (max_concurrent: {}, group: {:?})",
            execution_id, action_id, max_concurrent, group_key
        );

        let queue_key = self.queue_key(action_id, group_key);
        let queue_arc = self
            .get_or_create_queue(queue_key.clone(), max_concurrent)
            .await;

        // Create notifier for this execution
        let notifier = Arc::new(Notify::new());

        // Try to enqueue
        {
            let mut queue = queue_arc.lock().await;

            // Update max_concurrent if it changed
            queue.max_concurrent = max_concurrent;

            let queued_index = queue
                .queue
                .iter()
                .position(|entry| entry.execution_id == execution_id);
            if let Some(queued_index) = queued_index {
                if queued_index == 0 && queue.has_capacity() {
                    let entry = queue.queue.pop_front().expect("front entry just checked");
                    queue.active_count += 1;
                    self.active_execution_keys
                        .insert(entry.execution_id, queue_key.clone());
                    drop(queue);
                    self.persist_queue_stats(action_id).await;
                    return Ok(());
                }
                debug!(
                    "Execution {} is already queued for action {} (group: {:?})",
                    execution_id, action_id, queue_key.group_key
                );
                return Ok(());
            }

            // Check if we can run immediately
            if queue.has_capacity() {
                debug!(
                    "Execution {} can run immediately for action {} (active: {}/{}, group: {:?})",
                    execution_id,
                    action_id,
                    queue.active_count,
                    queue.max_concurrent,
                    queue_key.group_key
                );
                queue.active_count += 1;
                queue.total_enqueued += 1;
                self.active_execution_keys
                    .insert(execution_id, queue_key.clone());

                // Persist stats to database if available
                drop(queue);
                self.persist_queue_stats(action_id).await;

                return Ok(());
            }

            // Check if queue is full
            if queue.is_full(self.config.max_queue_length) {
                warn!(
                    "Queue full for action {} group {:?}: {} entries (limit: {})",
                    action_id,
                    queue_key.group_key,
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
                "Execution {} queued for action {} at position {} (active: {}/{}, group: {:?})",
                execution_id,
                action_id,
                queue.queue.len() - 1,
                queue.active_count,
                queue.max_concurrent,
                queue_key.group_key
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

    /// Acquire a slot immediately or enqueue without blocking the caller.
    pub async fn enqueue(
        &self,
        action_id: Id,
        execution_id: Id,
        max_concurrent: u32,
        group_key: Option<String>,
    ) -> Result<SlotEnqueueOutcome> {
        if self.active_execution_keys.contains_key(&execution_id) {
            debug!(
                "Execution {} already owns an active slot, treating as acquired",
                execution_id
            );
            return Ok(SlotEnqueueOutcome::Acquired);
        }

        debug!(
            "Enqueuing execution {} for action {} without waiting (max_concurrent: {}, group: {:?})",
            execution_id, action_id, max_concurrent, group_key
        );

        let queue_key = self.queue_key(action_id, group_key);
        let queue_arc = self
            .get_or_create_queue(queue_key.clone(), max_concurrent)
            .await;

        {
            let mut queue = queue_arc.lock().await;
            queue.max_concurrent = max_concurrent;

            let queued_index = queue
                .queue
                .iter()
                .position(|entry| entry.execution_id == execution_id);
            if let Some(queued_index) = queued_index {
                if queued_index == 0 && queue.has_capacity() {
                    let entry = queue.queue.pop_front().expect("front entry just checked");
                    queue.active_count += 1;
                    self.active_execution_keys
                        .insert(entry.execution_id, queue_key.clone());
                    drop(queue);
                    self.persist_queue_stats(action_id).await;
                    return Ok(SlotEnqueueOutcome::Acquired);
                }
                debug!(
                    "Execution {} is already queued for action {} (group: {:?})",
                    execution_id, action_id, queue_key.group_key
                );
                return Ok(SlotEnqueueOutcome::Enqueued);
            }

            if queue.has_capacity() {
                queue.active_count += 1;
                queue.total_enqueued += 1;
                self.active_execution_keys
                    .insert(execution_id, queue_key.clone());

                drop(queue);
                self.persist_queue_stats(action_id).await;
                return Ok(SlotEnqueueOutcome::Acquired);
            }

            if queue.is_full(self.config.max_queue_length) {
                warn!(
                    "Queue full for action {} group {:?}: {} entries (limit: {})",
                    action_id,
                    queue_key.group_key,
                    queue.queue.len(),
                    self.config.max_queue_length
                );
                return Err(anyhow::anyhow!(
                    "Queue full for action {}: maximum {} entries",
                    action_id,
                    self.config.max_queue_length
                ));
            }

            queue.queue.push_back(QueueEntry {
                execution_id,
                enqueued_at: Utc::now(),
                notifier: Arc::new(Notify::new()),
            });
            queue.total_enqueued += 1;
        }

        self.persist_queue_stats(action_id).await;
        Ok(SlotEnqueueOutcome::Enqueued)
    }

    /// Try to acquire a slot immediately without queueing.
    pub async fn try_acquire(
        &self,
        action_id: Id,
        execution_id: Id,
        max_concurrent: u32,
        group_key: Option<String>,
    ) -> Result<SlotAcquireOutcome> {
        let queue_key = self.queue_key(action_id, group_key);
        let queue_arc = self
            .get_or_create_queue(queue_key.clone(), max_concurrent)
            .await;
        let mut queue = queue_arc.lock().await;

        queue.max_concurrent = max_concurrent;
        let current_count = queue.active_count;

        if self.active_execution_keys.contains_key(&execution_id) {
            debug!(
                "Execution {} already owns a slot for action {} (group: {:?})",
                execution_id, action_id, queue_key.group_key
            );
            return Ok(SlotAcquireOutcome {
                acquired: true,
                current_count,
            });
        }

        if queue.has_capacity() {
            queue.active_count += 1;
            queue.total_enqueued += 1;
            self.active_execution_keys
                .insert(execution_id, queue_key.clone());
            drop(queue);
            self.persist_queue_stats(action_id).await;
            return Ok(SlotAcquireOutcome {
                acquired: true,
                current_count,
            });
        }

        Ok(SlotAcquireOutcome {
            acquired: false,
            current_count,
        })
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
    /// * `execution_id` - The execution that completed
    ///
    /// # Returns
    /// * `Ok(true)` - A queued execution was notified
    /// * `Ok(false)` - No executions were waiting
    /// * `Err(_)` - Error accessing queue
    pub async fn notify_completion(&self, execution_id: Id) -> Result<bool> {
        Ok(self
            .notify_completion_with_next(execution_id)
            .await?
            .is_some())
    }

    pub async fn notify_completion_with_next(&self, execution_id: Id) -> Result<Option<Id>> {
        let release = match self.release_active_slot(execution_id).await? {
            Some(release) => release,
            None => return Ok(None),
        };

        let Some(next_execution_id) = release.next_execution_id else {
            return Ok(None);
        };

        if self.activate_queued_execution(next_execution_id).await? {
            Ok(Some(next_execution_id))
        } else {
            self.restore_active_slot(execution_id, &release).await?;
            Ok(None)
        }
    }

    pub async fn release_active_slot(
        &self,
        execution_id: Id,
    ) -> Result<Option<SlotReleaseOutcome>> {
        let Some((_, queue_key)) = self.active_execution_keys.remove(&execution_id) else {
            debug!(
                "No active queue slot found for execution {} (queue may have been cleared)",
                execution_id
            );
            return Ok(None);
        };
        let action_id = queue_key.action_id;

        debug!(
            "Processing completion notification for execution {} on action {} (group: {:?})",
            execution_id, action_id, queue_key.group_key
        );

        // Get queue for this action/group
        let queue_arc = match self.queues.get(&queue_key) {
            Some(q) => q.clone(),
            None => {
                debug!(
                    "No queue found for action {} group {:?}",
                    action_id, queue_key.group_key
                );
                return Ok(None);
            }
        };

        let mut queue = queue_arc.lock().await;

        // Decrement active count
        if queue.active_count > 0 {
            queue.active_count -= 1;
            queue.total_completed += 1;
            debug!(
                "Decremented active count for action {} group {:?} to {}",
                action_id, queue_key.group_key, queue.active_count
            );
        } else {
            warn!(
                "Completion notification for action {} group {:?} but active_count is 0",
                action_id, queue_key.group_key
            );
        }

        // Check if there are queued executions
        if queue.queue.is_empty() {
            debug!(
                "No executions queued for action {} group {:?} after completion",
                action_id, queue_key.group_key
            );
            drop(queue);
            self.persist_queue_stats(action_id).await;
            return Ok(Some(SlotReleaseOutcome {
                next_execution_id: None,
                queue_key,
            }));
        }

        let next_execution_id = queue.queue.front().map(|entry| entry.execution_id);
        if let Some(next_execution_id) = next_execution_id {
            info!(
                "Execution {} is next for action {} group {:?}",
                next_execution_id, action_id, queue_key.group_key
            );
        }

        drop(queue);
        self.persist_queue_stats(action_id).await;

        Ok(Some(SlotReleaseOutcome {
            next_execution_id,
            queue_key,
        }))
    }

    pub async fn restore_active_slot(
        &self,
        execution_id: Id,
        outcome: &SlotReleaseOutcome,
    ) -> Result<()> {
        let action_id = outcome.queue_key.action_id;
        let queue_arc = self.get_or_create_queue(outcome.queue_key.clone(), 1).await;
        let mut queue = queue_arc.lock().await;

        queue.active_count += 1;
        if queue.total_completed > 0 {
            queue.total_completed -= 1;
        }
        self.active_execution_keys
            .insert(execution_id, outcome.queue_key.clone());

        drop(queue);
        self.persist_queue_stats(action_id).await;
        Ok(())
    }

    pub async fn activate_queued_execution(&self, execution_id: Id) -> Result<bool> {
        for entry in self.queues.iter() {
            let queue_key = entry.key().clone();
            let queue_arc = entry.value().clone();
            let mut queue = queue_arc.lock().await;

            let Some(front) = queue.queue.front() else {
                continue;
            };

            if front.execution_id != execution_id {
                continue;
            }

            if !queue.has_capacity() {
                return Ok(false);
            }

            let entry = queue.queue.pop_front().expect("front entry just checked");
            info!(
                "Activating queued execution {} for action {} group {:?} (queued for {:?})",
                entry.execution_id,
                queue_key.action_id,
                queue_key.group_key,
                Utc::now() - entry.enqueued_at
            );
            queue.active_count += 1;
            self.active_execution_keys
                .insert(entry.execution_id, queue_key.clone());

            drop(queue);
            entry.notifier.notify_one();
            self.persist_queue_stats(queue_key.action_id).await;
            return Ok(true);
        }

        Ok(false)
    }

    pub async fn remove_queued_execution(
        &self,
        execution_id: Id,
    ) -> Result<Option<QueuedRemovalOutcome>> {
        for entry in self.queues.iter() {
            let queue_key = entry.key().clone();
            let queue_arc = entry.value().clone();
            let mut queue = queue_arc.lock().await;

            let Some(index) = queue
                .queue
                .iter()
                .position(|queued| queued.execution_id == execution_id)
            else {
                continue;
            };

            let removed_entry = queue.queue.remove(index).expect("queue index just checked");
            let next_execution_id = if index == 0 {
                queue.queue.front().map(|queued| queued.execution_id)
            } else {
                None
            };
            let action_id = queue_key.action_id;

            drop(queue);
            self.persist_queue_stats(action_id).await;

            return Ok(Some(QueuedRemovalOutcome {
                next_execution_id,
                queue_key,
                removed_entry,
                removed_index: index,
            }));
        }

        Ok(None)
    }

    pub async fn restore_queued_execution(&self, outcome: &QueuedRemovalOutcome) -> Result<()> {
        let action_id = outcome.queue_key.action_id;
        let queue_arc = self.get_or_create_queue(outcome.queue_key.clone(), 1).await;
        let mut queue = queue_arc.lock().await;

        if outcome.removed_index <= queue.queue.len() {
            queue
                .queue
                .insert(outcome.removed_index, outcome.removed_entry.clone());
        } else {
            queue.queue.push_back(outcome.removed_entry.clone());
        }

        drop(queue);
        self.persist_queue_stats(action_id).await;
        Ok(())
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
        let queue_arcs: Vec<Arc<Mutex<ActionQueue>>> = self
            .queues
            .iter()
            .filter(|entry| entry.key().action_id == action_id)
            .map(|entry| entry.value().clone())
            .collect();

        if queue_arcs.is_empty() {
            return None;
        }

        let mut queue_length = 0usize;
        let mut active_count = 0u32;
        let mut max_concurrent = 0u32;
        let mut oldest_enqueued_at: Option<DateTime<Utc>> = None;
        let mut total_enqueued = 0u64;
        let mut total_completed = 0u64;

        for queue_arc in queue_arcs {
            let queue = queue_arc.lock().await;
            queue_length += queue.queue.len();
            active_count += queue.active_count;
            max_concurrent += queue.max_concurrent;
            total_enqueued += queue.total_enqueued;
            total_completed += queue.total_completed;

            if let Some(candidate) = queue.queue.front().map(|e| e.enqueued_at) {
                oldest_enqueued_at = Some(match oldest_enqueued_at {
                    Some(current) => current.min(candidate),
                    None => candidate,
                });
            }
        }

        Some(QueueStats {
            action_id,
            queue_length,
            active_count,
            max_concurrent,
            oldest_enqueued_at,
            total_enqueued,
            total_completed,
        })
    }

    /// Get statistics for all queues
    #[allow(dead_code)]
    pub async fn get_all_queue_stats(&self) -> Vec<QueueStats> {
        let mut stats = Vec::new();

        let mut action_ids = std::collections::BTreeSet::new();
        for entry in self.queues.iter() {
            action_ids.insert(entry.key().action_id);
        }

        for action_id in action_ids {
            if let Some(action_stats) = self.get_queue_stats(action_id).await {
                stats.push(action_stats);
            }
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

        let queue_arcs: Vec<Arc<Mutex<ActionQueue>>> = self
            .queues
            .iter()
            .filter(|entry| entry.key().action_id == action_id)
            .map(|entry| entry.value().clone())
            .collect();

        for queue_arc in queue_arcs {
            let mut queue = queue_arc.lock().await;
            let initial_len = queue.queue.len();
            queue.queue.retain(|e| e.execution_id != execution_id);
            if initial_len != queue.queue.len() {
                info!("Cancelled execution {} from queue", execution_id);
                return Ok(true);
            }
        }

        debug!(
            "Execution {} not found in queue (may be running)",
            execution_id
        );

        Ok(false)
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
        self.active_execution_keys.clear();
    }

    /// Get the number of actions with active queues
    #[allow(dead_code)]
    pub fn active_queue_count(&self) -> usize {
        self.queues
            .iter()
            .map(|entry| entry.key().action_id)
            .collect::<std::collections::BTreeSet<_>>()
            .len()
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
        let result = manager.enqueue_and_wait(1, 100, 2, None).await;
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
            .enqueue_and_wait(action_id, 100, max_concurrent, None)
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
                    .enqueue_and_wait(action_id, exec_id, max_concurrent, None)
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
        for execution_id in 100..103 {
            sleep(Duration::from_millis(50)).await;
            manager.notify_completion(execution_id).await.unwrap();
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
        manager
            .enqueue_and_wait(action_id, 100, 1, None)
            .await
            .unwrap();

        // Queue second execution
        let manager_clone = Arc::new(manager);
        let manager_ref = manager_clone.clone();

        let handle = tokio::spawn(async move {
            manager_ref
                .enqueue_and_wait(action_id, 101, 1, None)
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
        let notified = manager_clone.notify_completion(100).await.unwrap();
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
        manager.enqueue_and_wait(1, 100, 1, None).await.unwrap();
        manager.enqueue_and_wait(2, 200, 1, None).await.unwrap();

        // Both should be active
        let stats1 = manager.get_queue_stats(1).await.unwrap();
        let stats2 = manager.get_queue_stats(2).await.unwrap();

        assert_eq!(stats1.active_count, 1);
        assert_eq!(stats2.active_count, 1);

        // Completion on action 1 shouldn't affect action 2
        manager.notify_completion(100).await.unwrap();

        let stats1 = manager.get_queue_stats(1).await.unwrap();
        let stats2 = manager.get_queue_stats(2).await.unwrap();

        assert_eq!(stats1.active_count, 0);
        assert_eq!(stats2.active_count, 1);
    }

    #[tokio::test]
    async fn test_grouped_queues_are_independent() {
        let manager = ExecutionQueueManager::with_defaults();
        let action_id = 1;

        manager
            .enqueue_and_wait(action_id, 100, 1, Some("prod".to_string()))
            .await
            .unwrap();
        manager
            .enqueue_and_wait(action_id, 200, 1, Some("staging".to_string()))
            .await
            .unwrap();

        let stats = manager.get_queue_stats(action_id).await.unwrap();
        assert_eq!(stats.active_count, 2);
        assert_eq!(stats.queue_length, 0);
        assert_eq!(stats.max_concurrent, 2);
    }

    #[tokio::test]
    async fn test_cancel_execution() {
        let manager = ExecutionQueueManager::with_defaults();
        let action_id = 1;

        // Fill capacity
        manager
            .enqueue_and_wait(action_id, 100, 1, None)
            .await
            .unwrap();

        // Queue more executions
        let manager_arc = Arc::new(manager);
        let manager_ref = manager_arc.clone();

        let handle = tokio::spawn(async move {
            let result = manager_ref.enqueue_and_wait(action_id, 101, 1, None).await;
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
        manager
            .enqueue_and_wait(action_id, 100, 2, None)
            .await
            .unwrap();

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
        manager
            .enqueue_and_wait(action_id, 100, 1, None)
            .await
            .unwrap();

        // Queue 2 more (should reach limit)
        let manager_ref = manager.clone();
        tokio::spawn(async move {
            manager_ref
                .enqueue_and_wait(action_id, 101, 1, None)
                .await
                .unwrap();
        });

        let manager_ref = manager.clone();
        tokio::spawn(async move {
            manager_ref
                .enqueue_and_wait(action_id, 102, 1, None)
                .await
                .unwrap();
        });

        sleep(Duration::from_millis(100)).await;

        // Next one should fail
        let result = manager.enqueue_and_wait(action_id, 103, 1, None).await;
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
            .enqueue_and_wait(action_id, 0, max_concurrent, None)
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
                    .enqueue_and_wait(action_id, i, max_concurrent, None)
                    .await
                    .unwrap();
                order.lock().await.push(i);
            });

            handles.push(handle);
        }

        // Give time to queue
        sleep(Duration::from_millis(200)).await;

        // Release them all
        for execution_id in 0..num_executions {
            sleep(Duration::from_millis(10)).await;
            manager.notify_completion(execution_id).await.unwrap();
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
