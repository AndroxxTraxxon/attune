//! Policy Enforcer - Enforces execution policies
//!
//! This module is responsible for:
//! - Rate limiting: Limit executions per time window
//! - Concurrency control: Maximum concurrent executions
//! - Quota management: Resource limits per tenant/pack
//! - Policy evaluation before execution creation
//! - Policy enforcement during scheduling

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use std::sync::Arc;
use tracing::{debug, info, warn};

use attune_common::models::{enums::ExecutionStatus, Id};

use crate::queue_manager::ExecutionQueueManager;

/// Policy violation type
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[allow(clippy::enum_variant_names)]
pub enum PolicyViolation {
    /// Rate limit exceeded
    RateLimitExceeded {
        limit: u32,
        window_seconds: u32,
        current_count: u32,
    },
    /// Concurrency limit exceeded
    ConcurrencyLimitExceeded { limit: u32, current_count: u32 },
    /// Resource quota exceeded
    QuotaExceeded {
        quota_type: String,
        limit: u64,
        current_usage: u64,
    },
}

impl std::fmt::Display for PolicyViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PolicyViolation::RateLimitExceeded {
                limit,
                window_seconds,
                current_count,
            } => {
                write!(
                    f,
                    "Rate limit exceeded: {} executions in {} seconds (limit: {})",
                    current_count, window_seconds, limit
                )
            }
            PolicyViolation::ConcurrencyLimitExceeded {
                limit,
                current_count,
            } => {
                write!(
                    f,
                    "Concurrency limit exceeded: {} running executions (limit: {})",
                    current_count, limit
                )
            }
            PolicyViolation::QuotaExceeded {
                quota_type,
                limit,
                current_usage,
            } => {
                write!(
                    f,
                    "{} quota exceeded: {} (limit: {})",
                    quota_type, current_usage, limit
                )
            }
        }
    }
}

/// Execution policy configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ExecutionPolicy {
    /// Rate limit: maximum executions per time window
    pub rate_limit: Option<RateLimit>,
    /// Concurrency limit: maximum concurrent executions
    pub concurrency_limit: Option<u32>,
    /// Resource quotas
    pub quotas: Option<HashMap<String, u64>>,
}

/// Rate limit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimit {
    /// Maximum number of executions
    pub max_executions: u32,
    /// Time window in seconds
    pub window_seconds: u32,
}

/// Policy enforcement scope
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(dead_code)] // Used in tests
pub enum PolicyScope {
    /// Global policy (all executions)
    Global,
    /// Per-pack policy
    Pack(Id),
    /// Per-action policy
    Action(Id),
    /// Per-identity policy (tenant)
    Identity(Id),
}

/// Policy enforcer that validates execution policies
pub struct PolicyEnforcer {
    pool: PgPool,
    /// Global execution policy
    global_policy: ExecutionPolicy,
    /// Per-pack policies
    pack_policies: HashMap<Id, ExecutionPolicy>,
    /// Per-action policies
    action_policies: HashMap<Id, ExecutionPolicy>,
    /// Queue manager for FIFO execution ordering
    queue_manager: Option<Arc<ExecutionQueueManager>>,
}

impl PolicyEnforcer {
    /// Create a new policy enforcer
    #[allow(dead_code)]
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            global_policy: ExecutionPolicy::default(),
            pack_policies: HashMap::new(),
            action_policies: HashMap::new(),
            queue_manager: None,
        }
    }

    /// Create a new policy enforcer with queue manager
    pub fn with_queue_manager(pool: PgPool, queue_manager: Arc<ExecutionQueueManager>) -> Self {
        Self {
            pool,
            global_policy: ExecutionPolicy::default(),
            pack_policies: HashMap::new(),
            action_policies: HashMap::new(),
            queue_manager: Some(queue_manager),
        }
    }

    /// Create with global policy
    #[allow(dead_code)]
    pub fn with_global_policy(pool: PgPool, policy: ExecutionPolicy) -> Self {
        Self {
            pool,
            global_policy: policy,
            pack_policies: HashMap::new(),
            action_policies: HashMap::new(),
            queue_manager: None,
        }
    }

    /// Set the queue manager
    #[allow(dead_code)]
    pub fn set_queue_manager(&mut self, queue_manager: Arc<ExecutionQueueManager>) {
        self.queue_manager = Some(queue_manager);
    }

    /// Set global execution policy
    #[allow(dead_code)]
    pub fn set_global_policy(&mut self, policy: ExecutionPolicy) {
        self.global_policy = policy;
    }

    /// Set policy for a specific pack
    #[allow(dead_code)]
    pub fn set_pack_policy(&mut self, pack_id: Id, policy: ExecutionPolicy) {
        self.pack_policies.insert(pack_id, policy);
    }

    /// Set policy for a specific action
    #[allow(dead_code)]
    pub fn set_action_policy(&mut self, action_id: Id, policy: ExecutionPolicy) {
        self.action_policies.insert(action_id, policy);
    }

    /// Get the concurrency limit for a specific action
    ///
    /// Returns the most specific concurrency limit found:
    /// 1. Action-specific policy
    /// 2. Pack policy
    /// 3. Global policy
    /// 4. None (unlimited)
    pub fn get_concurrency_limit(&self, action_id: Id, pack_id: Option<Id>) -> Option<u32> {
        // Check action-specific policy first
        if let Some(policy) = self.action_policies.get(&action_id) {
            if let Some(limit) = policy.concurrency_limit {
                return Some(limit);
            }
        }

        // Check pack policy
        if let Some(pack_id) = pack_id {
            if let Some(policy) = self.pack_policies.get(&pack_id) {
                if let Some(limit) = policy.concurrency_limit {
                    return Some(limit);
                }
            }
        }

        // Check global policy
        self.global_policy.concurrency_limit
    }

    /// Enforce policies and wait in queue if necessary
    ///
    /// This method combines policy checking with queue management to ensure:
    /// 1. Policy violations are detected early
    /// 2. FIFO ordering is maintained when capacity is limited
    /// 3. Executions wait efficiently for available slots
    ///
    /// # Arguments
    /// * `action_id` - The action to execute
    /// * `pack_id` - The pack containing the action
    /// * `execution_id` - The execution/enforcement ID for queue tracking
    ///
    /// # Returns
    /// * `Ok(())` - Policy allows execution and queue slot obtained
    /// * `Err(PolicyViolation)` - Policy prevents execution
    /// * `Err(QueueError)` - Queue timeout or other queue error
    pub async fn enforce_and_wait(
        &self,
        action_id: Id,
        pack_id: Option<Id>,
        execution_id: Id,
    ) -> Result<()> {
        // First, check for policy violations (rate limit, quotas, etc.)
        // Note: We skip concurrency check here since queue manages that
        if let Some(violation) = self
            .check_policies_except_concurrency(action_id, pack_id)
            .await?
        {
            warn!("Policy violation for action {}: {}", action_id, violation);
            return Err(anyhow::anyhow!("Policy violation: {}", violation));
        }

        // If queue manager is available, use it for concurrency control
        if let Some(queue_manager) = &self.queue_manager {
            let concurrency_limit = self
                .get_concurrency_limit(action_id, pack_id)
                .unwrap_or(u32::MAX); // Default to unlimited if no policy

            debug!(
                "Enqueuing execution {} for action {} with concurrency limit {}",
                execution_id, action_id, concurrency_limit
            );

            queue_manager
                .enqueue_and_wait(action_id, execution_id, concurrency_limit)
                .await?;

            info!(
                "Execution {} obtained queue slot for action {}",
                execution_id, action_id
            );
        } else {
            // No queue manager - use legacy polling behavior
            debug!(
                "No queue manager configured, using legacy policy wait for action {}",
                action_id
            );

            if let Some(concurrency_limit) = self.get_concurrency_limit(action_id, pack_id) {
                // Check concurrency with old method
                let scope = PolicyScope::Action(action_id);
                if let Some(violation) = self
                    .check_concurrency_limit(concurrency_limit, &scope)
                    .await?
                {
                    return Err(anyhow::anyhow!("Policy violation: {}", violation));
                }
            }
        }

        Ok(())
    }

    /// Check policies except concurrency (which is handled by queue)
    async fn check_policies_except_concurrency(
        &self,
        action_id: Id,
        pack_id: Option<Id>,
    ) -> Result<Option<PolicyViolation>> {
        // Check action-specific policy first
        if let Some(policy) = self.action_policies.get(&action_id) {
            if let Some(violation) = self
                .evaluate_policy_except_concurrency(policy, PolicyScope::Action(action_id))
                .await?
            {
                return Ok(Some(violation));
            }
        }

        // Check pack policy
        if let Some(pack_id) = pack_id {
            if let Some(policy) = self.pack_policies.get(&pack_id) {
                if let Some(violation) = self
                    .evaluate_policy_except_concurrency(policy, PolicyScope::Pack(pack_id))
                    .await?
                {
                    return Ok(Some(violation));
                }
            }
        }

        // Check global policy
        if let Some(violation) = self
            .evaluate_policy_except_concurrency(&self.global_policy, PolicyScope::Global)
            .await?
        {
            return Ok(Some(violation));
        }

        Ok(None)
    }

    /// Evaluate a policy against current state (except concurrency)
    async fn evaluate_policy_except_concurrency(
        &self,
        policy: &ExecutionPolicy,
        scope: PolicyScope,
    ) -> Result<Option<PolicyViolation>> {
        // Check rate limit
        if let Some(rate_limit) = &policy.rate_limit {
            if let Some(violation) = self.check_rate_limit(rate_limit, &scope).await? {
                return Ok(Some(violation));
            }
        }

        // Skip concurrency check - handled by queue

        // Check quotas
        if let Some(quotas) = &policy.quotas {
            for (quota_type, limit) in quotas {
                if let Some(violation) = self.check_quota(quota_type, *limit, &scope).await? {
                    return Ok(Some(violation));
                }
            }
        }

        Ok(None)
    }

    /// Check if execution is allowed under policies
    #[allow(dead_code)]
    pub async fn check_policies(
        &self,
        action_id: Id,
        pack_id: Option<Id>,
    ) -> Result<Option<PolicyViolation>> {
        // Check action-specific policy first
        if let Some(policy) = self.action_policies.get(&action_id) {
            if let Some(violation) = self
                .evaluate_policy(policy, PolicyScope::Action(action_id))
                .await?
            {
                return Ok(Some(violation));
            }
        }

        // Check pack policy
        if let Some(pack_id) = pack_id {
            if let Some(policy) = self.pack_policies.get(&pack_id) {
                if let Some(violation) = self
                    .evaluate_policy(policy, PolicyScope::Pack(pack_id))
                    .await?
                {
                    return Ok(Some(violation));
                }
            }
        }

        // Check global policy
        if let Some(violation) = self
            .evaluate_policy(&self.global_policy, PolicyScope::Global)
            .await?
        {
            return Ok(Some(violation));
        }

        Ok(None)
    }

    /// Evaluate a policy against current state
    #[allow(dead_code)]
    async fn evaluate_policy(
        &self,
        policy: &ExecutionPolicy,
        scope: PolicyScope,
    ) -> Result<Option<PolicyViolation>> {
        // Check rate limit
        if let Some(rate_limit) = &policy.rate_limit {
            if let Some(violation) = self.check_rate_limit(rate_limit, &scope).await? {
                return Ok(Some(violation));
            }
        }

        // Check concurrency limit
        if let Some(concurrency_limit) = policy.concurrency_limit {
            if let Some(violation) = self
                .check_concurrency_limit(concurrency_limit, &scope)
                .await?
            {
                return Ok(Some(violation));
            }
        }

        // Check quotas
        if let Some(quotas) = &policy.quotas {
            for (quota_type, limit) in quotas {
                if let Some(violation) = self.check_quota(quota_type, *limit, &scope).await? {
                    return Ok(Some(violation));
                }
            }
        }

        Ok(None)
    }

    /// Check rate limit for a scope
    async fn check_rate_limit(
        &self,
        rate_limit: &RateLimit,
        scope: &PolicyScope,
    ) -> Result<Option<PolicyViolation>> {
        let window_start = Utc::now() - Duration::seconds(rate_limit.window_seconds as i64);

        let count = self.count_executions_since(scope, window_start).await?;

        if count >= rate_limit.max_executions {
            info!(
                "Rate limit exceeded for {:?}: {} executions in {} seconds (limit: {})",
                scope, count, rate_limit.window_seconds, rate_limit.max_executions
            );

            return Ok(Some(PolicyViolation::RateLimitExceeded {
                limit: rate_limit.max_executions,
                window_seconds: rate_limit.window_seconds,
                current_count: count,
            }));
        }

        debug!(
            "Rate limit check passed for {:?}: {} / {} executions in {} seconds",
            scope, count, rate_limit.max_executions, rate_limit.window_seconds
        );

        Ok(None)
    }

    /// Check concurrency limit for a scope
    async fn check_concurrency_limit(
        &self,
        limit: u32,
        scope: &PolicyScope,
    ) -> Result<Option<PolicyViolation>> {
        let count = self.count_running_executions(scope).await?;

        if count >= limit {
            info!(
                "Concurrency limit exceeded for {:?}: {} running executions (limit: {})",
                scope, count, limit
            );

            return Ok(Some(PolicyViolation::ConcurrencyLimitExceeded {
                limit,
                current_count: count,
            }));
        }

        debug!(
            "Concurrency limit check passed for {:?}: {} / {} running executions",
            scope, count, limit
        );

        Ok(None)
    }

    /// Check resource quota for a scope
    async fn check_quota(
        &self,
        quota_type: &str,
        limit: u64,
        scope: &PolicyScope,
    ) -> Result<Option<PolicyViolation>> {
        // TODO: Implement quota tracking based on quota_type
        // For now, we'll just return None (no quota enforcement)

        debug!(
            "Quota check for {:?}: {} (limit: {}, not implemented yet)",
            scope, quota_type, limit
        );

        Ok(None)
    }

    /// Count executions created since a specific time
    async fn count_executions_since(
        &self,
        scope: &PolicyScope,
        since: DateTime<Utc>,
    ) -> Result<u32> {
        let count: i64 = match scope {
            PolicyScope::Global => {
                sqlx::query_scalar("SELECT COUNT(*) FROM attune.execution WHERE created >= $1")
                    .bind(since)
                    .fetch_one(&self.pool)
                    .await?
            }
            PolicyScope::Pack(pack_id) => {
                sqlx::query_scalar(
                    r#"
                    SELECT COUNT(*)
                    FROM attune.execution e
                    JOIN attune.action a ON e.action = a.id
                    WHERE a.pack = $1 AND e.created >= $2
                    "#,
                )
                .bind(pack_id)
                .bind(since)
                .fetch_one(&self.pool)
                .await?
            }
            PolicyScope::Action(action_id) => {
                sqlx::query_scalar(
                    "SELECT COUNT(*) FROM attune.execution WHERE action = $1 AND created >= $2",
                )
                .bind(action_id)
                .bind(since)
                .fetch_one(&self.pool)
                .await?
            }
            PolicyScope::Identity(_identity_id) => {
                // TODO: Track executions by identity/tenant
                // For now, treat as global
                sqlx::query_scalar("SELECT COUNT(*) FROM attune.execution WHERE created >= $1")
                    .bind(since)
                    .fetch_one(&self.pool)
                    .await?
            }
        };

        Ok(count as u32)
    }

    /// Count currently running executions
    async fn count_running_executions(&self, scope: &PolicyScope) -> Result<u32> {
        let count: i64 = match scope {
            PolicyScope::Global => {
                sqlx::query_scalar("SELECT COUNT(*) FROM attune.execution WHERE status = $1")
                    .bind(ExecutionStatus::Running)
                    .fetch_one(&self.pool)
                    .await?
            }
            PolicyScope::Pack(pack_id) => {
                sqlx::query_scalar(
                    r#"
                    SELECT COUNT(*)
                    FROM attune.execution e
                    JOIN attune.action a ON e.action = a.id
                    WHERE a.pack = $1 AND e.status = $2
                    "#,
                )
                .bind(pack_id)
                .bind(ExecutionStatus::Running)
                .fetch_one(&self.pool)
                .await?
            }
            PolicyScope::Action(action_id) => {
                sqlx::query_scalar(
                    "SELECT COUNT(*) FROM attune.execution WHERE action = $1 AND status = $2",
                )
                .bind(action_id)
                .bind(ExecutionStatus::Running)
                .fetch_one(&self.pool)
                .await?
            }
            PolicyScope::Identity(_identity_id) => {
                // TODO: Track executions by identity/tenant
                // For now, treat as global
                sqlx::query_scalar("SELECT COUNT(*) FROM attune.execution WHERE status = $1")
                    .bind(ExecutionStatus::Running)
                    .fetch_one(&self.pool)
                    .await?
            }
        };

        Ok(count as u32)
    }

    /// Wait for policy compliance (block until policies allow execution)
    #[allow(dead_code)]
    pub async fn wait_for_policy_compliance(
        &self,
        action_id: Id,
        pack_id: Option<Id>,
        max_wait_seconds: u32,
    ) -> Result<bool> {
        let start = Utc::now();
        let max_wait = Duration::seconds(max_wait_seconds as i64);

        loop {
            // Check if policies allow execution
            if self.check_policies(action_id, pack_id).await?.is_none() {
                return Ok(true);
            }

            // Check if we've exceeded max wait time
            if Utc::now() - start > max_wait {
                warn!(
                    "Policy compliance timeout after {} seconds for action {}",
                    max_wait_seconds, action_id
                );
                return Ok(false);
            }

            // Wait a bit before checking again
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue_manager::QueueConfig;
    use tokio::time::{sleep, Duration};

    #[test]
    fn test_policy_violation_display() {
        let violation = PolicyViolation::RateLimitExceeded {
            limit: 10,
            window_seconds: 60,
            current_count: 15,
        };
        assert!(violation.to_string().contains("Rate limit exceeded"));

        let violation = PolicyViolation::ConcurrencyLimitExceeded {
            limit: 5,
            current_count: 7,
        };
        assert!(violation.to_string().contains("Concurrency limit exceeded"));

        let violation = PolicyViolation::QuotaExceeded {
            quota_type: "cpu".to_string(),
            limit: 100,
            current_usage: 150,
        };
        assert!(violation.to_string().contains("cpu quota exceeded"));
    }

    #[test]
    fn test_execution_policy_default() {
        let policy = ExecutionPolicy::default();
        assert!(policy.rate_limit.is_none());
        assert!(policy.concurrency_limit.is_none());
        assert!(policy.quotas.is_none());
    }

    #[test]
    fn test_rate_limit() {
        let rate_limit = RateLimit {
            max_executions: 10,
            window_seconds: 60,
        };
        assert_eq!(rate_limit.max_executions, 10);
        assert_eq!(rate_limit.window_seconds, 60);
    }

    #[test]
    fn test_policy_scope_equality() {
        assert_eq!(PolicyScope::Global, PolicyScope::Global);
        assert_eq!(PolicyScope::Pack(1), PolicyScope::Pack(1));
        assert_ne!(PolicyScope::Pack(1), PolicyScope::Pack(2));
        assert_eq!(PolicyScope::Action(1), PolicyScope::Action(1));
        assert_ne!(PolicyScope::Action(1), PolicyScope::Action(2));
    }

    #[tokio::test]
    async fn test_get_concurrency_limit_action_specific() {
        let pool = sqlx::PgPool::connect_lazy("postgresql://localhost/test").unwrap();
        let mut enforcer = PolicyEnforcer::new(pool);

        // Set action-specific policy
        let policy = ExecutionPolicy {
            concurrency_limit: Some(5),
            ..Default::default()
        };
        enforcer.set_action_policy(1, policy);

        assert_eq!(enforcer.get_concurrency_limit(1, None), Some(5));
        assert_eq!(enforcer.get_concurrency_limit(2, None), None);
    }

    #[tokio::test]
    async fn test_get_concurrency_limit_pack() {
        let pool = sqlx::PgPool::connect_lazy("postgresql://localhost/test").unwrap();
        let mut enforcer = PolicyEnforcer::new(pool);

        // Set pack policy
        let policy = ExecutionPolicy {
            concurrency_limit: Some(10),
            ..Default::default()
        };
        enforcer.set_pack_policy(100, policy);

        assert_eq!(enforcer.get_concurrency_limit(1, Some(100)), Some(10));
        assert_eq!(enforcer.get_concurrency_limit(1, Some(200)), None);
    }

    #[tokio::test]
    async fn test_get_concurrency_limit_global() {
        let pool = sqlx::PgPool::connect_lazy("postgresql://localhost/test").unwrap();
        let policy = ExecutionPolicy {
            concurrency_limit: Some(20),
            ..Default::default()
        };
        let enforcer = PolicyEnforcer::with_global_policy(pool, policy);

        assert_eq!(enforcer.get_concurrency_limit(1, None), Some(20));
    }

    #[tokio::test]
    async fn test_get_concurrency_limit_precedence() {
        let pool = sqlx::PgPool::connect_lazy("postgresql://localhost/test").unwrap();
        let mut enforcer = PolicyEnforcer::new(pool);

        // Set all levels
        enforcer.set_global_policy(ExecutionPolicy {
            concurrency_limit: Some(20),
            ..Default::default()
        });

        enforcer.set_pack_policy(
            100,
            ExecutionPolicy {
                concurrency_limit: Some(10),
                ..Default::default()
            },
        );

        enforcer.set_action_policy(
            1,
            ExecutionPolicy {
                concurrency_limit: Some(5),
                ..Default::default()
            },
        );

        // Action-specific should take precedence
        assert_eq!(enforcer.get_concurrency_limit(1, Some(100)), Some(5));

        // Without action policy, pack should take precedence
        assert_eq!(enforcer.get_concurrency_limit(2, Some(100)), Some(10));

        // Without action or pack policy, global should apply
        assert_eq!(enforcer.get_concurrency_limit(2, Some(200)), Some(20));
    }

    #[tokio::test]
    async fn test_enforce_and_wait_with_queue_manager() {
        let pool = sqlx::PgPool::connect_lazy("postgresql://localhost/test").unwrap();
        let queue_manager = Arc::new(ExecutionQueueManager::with_defaults());
        let mut enforcer = PolicyEnforcer::with_queue_manager(pool, queue_manager.clone());

        // Set concurrency limit
        enforcer.set_action_policy(
            1,
            ExecutionPolicy {
                concurrency_limit: Some(1),
                ..Default::default()
            },
        );

        // First execution should proceed immediately
        let result = enforcer.enforce_and_wait(1, None, 100).await;
        assert!(result.is_ok());

        // Check queue stats
        let stats = queue_manager.get_queue_stats(1).await.unwrap();
        assert_eq!(stats.active_count, 1);
        assert_eq!(stats.queue_length, 0);
    }

    #[tokio::test]
    async fn test_enforce_and_wait_fifo_ordering() {
        let pool = sqlx::PgPool::connect_lazy("postgresql://localhost/test").unwrap();
        let queue_manager = Arc::new(ExecutionQueueManager::with_defaults());
        let mut enforcer = PolicyEnforcer::with_queue_manager(pool, queue_manager.clone());

        enforcer.set_action_policy(
            1,
            ExecutionPolicy {
                concurrency_limit: Some(1),
                ..Default::default()
            },
        );
        let enforcer = Arc::new(enforcer);

        // First execution
        let result = enforcer.enforce_and_wait(1, None, 100).await;
        assert!(result.is_ok());

        // Queue multiple executions
        let execution_order = Arc::new(tokio::sync::Mutex::new(Vec::new()));
        let mut handles = vec![];

        for exec_id in 101..=103 {
            let enforcer = enforcer.clone();
            let queue_manager = queue_manager.clone();
            let order = execution_order.clone();

            let handle = tokio::spawn(async move {
                enforcer.enforce_and_wait(1, None, exec_id).await.unwrap();
                order.lock().await.push(exec_id);
                // Simulate work
                sleep(Duration::from_millis(10)).await;
                queue_manager.notify_completion(1).await.unwrap();
            });

            handles.push(handle);
        }

        // Give tasks time to queue
        sleep(Duration::from_millis(100)).await;

        // Release first execution
        queue_manager.notify_completion(1).await.unwrap();

        // Wait for all
        for handle in handles {
            handle.await.unwrap();
        }

        // Verify FIFO order
        let order = execution_order.lock().await;
        assert_eq!(*order, vec![101, 102, 103]);
    }

    #[tokio::test]
    async fn test_enforce_and_wait_without_queue_manager() {
        let pool = sqlx::PgPool::connect_lazy("postgresql://localhost/test").unwrap();
        let mut enforcer = PolicyEnforcer::new(pool);

        // Set unlimited concurrency
        enforcer.set_action_policy(
            1,
            ExecutionPolicy {
                concurrency_limit: None,
                ..Default::default()
            },
        );

        // Should work without queue manager (legacy behavior)
        let result = enforcer.enforce_and_wait(1, None, 100).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_enforce_and_wait_queue_timeout() {
        let config = QueueConfig {
            max_queue_length: 100,
            queue_timeout_seconds: 1, // Short timeout for test
            enable_metrics: true,
        };

        let pool = sqlx::PgPool::connect_lazy("postgresql://localhost/test").unwrap();
        let queue_manager = Arc::new(ExecutionQueueManager::new(config));
        let mut enforcer = PolicyEnforcer::with_queue_manager(pool, queue_manager.clone());

        // Set concurrency limit
        enforcer.set_action_policy(
            1,
            ExecutionPolicy {
                concurrency_limit: Some(1),
                ..Default::default()
            },
        );

        // First execution proceeds
        enforcer.enforce_and_wait(1, None, 100).await.unwrap();

        // Second execution should timeout
        let result = enforcer.enforce_and_wait(1, None, 101).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("timeout"));
    }

    // Integration tests would require database setup
    // Those should be in a separate integration test file
}
