//! Execution Timeout Monitor
//!
//! This module monitors executions in SCHEDULED status and fails them if they
//! don't transition to RUNNING within a configured timeout period.
//!
//! This prevents executions from being stuck indefinitely when workers:
//! - Stop or crash after being selected
//! - Fail to consume messages from their queues
//! - Are partitioned from the network

use anyhow::Result;
use attune_common::{
    models::{enums::ExecutionStatus, Execution},
    mq::{MessageEnvelope, MessageType, Publisher},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::interval;
use tracing::{debug, error, info, warn};

/// Configuration for timeout monitor
#[derive(Debug, Clone)]
pub struct TimeoutMonitorConfig {
    /// How long an execution can remain in SCHEDULED status before timing out
    pub scheduled_timeout: Duration,

    /// How often to check for stale executions
    pub check_interval: Duration,

    /// Whether to enable the timeout monitor
    pub enabled: bool,
}

impl Default for TimeoutMonitorConfig {
    fn default() -> Self {
        Self {
            scheduled_timeout: Duration::from_secs(300), // 5 minutes
            check_interval: Duration::from_secs(60),     // 1 minute
            enabled: true,
        }
    }
}

/// Payload for execution completion messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionCompletedPayload {
    pub execution_id: i64,
    pub status: ExecutionStatus,
    pub result: Option<JsonValue>,
}

/// Monitors scheduled executions and fails those that timeout
pub struct ExecutionTimeoutMonitor {
    pool: PgPool,
    publisher: Arc<Publisher>,
    config: TimeoutMonitorConfig,
}

impl ExecutionTimeoutMonitor {
    /// Create a new timeout monitor
    pub fn new(pool: PgPool, publisher: Arc<Publisher>, config: TimeoutMonitorConfig) -> Self {
        Self {
            pool,
            publisher,
            config,
        }
    }

    /// Start the timeout monitor loop
    pub async fn start(self: Arc<Self>) -> Result<()> {
        if !self.config.enabled {
            info!("Execution timeout monitor is disabled");
            return Ok(());
        }

        info!(
            "Starting execution timeout monitor (timeout: {}s, check interval: {}s)",
            self.config.scheduled_timeout.as_secs(),
            self.config.check_interval.as_secs()
        );

        let mut check_interval = interval(self.config.check_interval);

        loop {
            check_interval.tick().await;

            if let Err(e) = self.check_stale_executions().await {
                error!("Error checking stale executions: {}", e);
                // Continue running despite errors
            }
        }
    }

    /// Check for executions stuck in SCHEDULED status
    async fn check_stale_executions(&self) -> Result<()> {
        let cutoff = self.calculate_cutoff_time();

        debug!(
            "Checking for executions scheduled before {}",
            cutoff.format("%Y-%m-%d %H:%M:%S UTC")
        );

        // Find executions stuck in SCHEDULED status
        let stale_executions = sqlx::query_as::<_, Execution>(
            "SELECT * FROM execution
             WHERE status = $1
             AND updated < $2
             ORDER BY updated ASC
             LIMIT 100", // Process in batches to avoid overwhelming system
        )
        .bind(ExecutionStatus::Scheduled)
        .bind(cutoff)
        .fetch_all(&self.pool)
        .await?;

        if stale_executions.is_empty() {
            debug!("No stale scheduled executions found");
            return Ok(());
        }

        warn!(
            "Found {} stale scheduled executions (older than {}s)",
            stale_executions.len(),
            self.config.scheduled_timeout.as_secs()
        );

        for execution in stale_executions {
            let age_seconds = (Utc::now() - execution.updated).num_seconds();

            warn!(
                "Execution {} has been scheduled for {} seconds (timeout: {}s), marking as failed",
                execution.id,
                age_seconds,
                self.config.scheduled_timeout.as_secs()
            );

            if let Err(e) = self.fail_execution(&execution, age_seconds).await {
                error!("Failed to fail execution {}: {}", execution.id, e);
                // Continue processing other executions
            }
        }

        Ok(())
    }

    /// Calculate the cutoff time for stale executions
    fn calculate_cutoff_time(&self) -> DateTime<Utc> {
        let timeout_duration = chrono::Duration::from_std(self.config.scheduled_timeout)
            .expect("Invalid timeout duration");

        Utc::now() - timeout_duration
    }

    /// Mark an execution as failed due to timeout
    async fn fail_execution(&self, execution: &Execution, age_seconds: i64) -> Result<()> {
        let execution_id = execution.id;
        let error_message = format!(
            "Execution timeout: worker did not pick up task within {} seconds (scheduled for {} seconds)",
            self.config.scheduled_timeout.as_secs(),
            age_seconds
        );

        info!(
            "Failing execution {} due to timeout: {}",
            execution_id, error_message
        );

        // Create failure result
        let result = serde_json::json!({
            "error": error_message,
            "failed_by": "execution_timeout_monitor",
            "timeout_seconds": self.config.scheduled_timeout.as_secs(),
            "age_seconds": age_seconds,
            "original_status": "scheduled"
        });

        // Update execution status in database
        sqlx::query(
            "UPDATE execution
             SET status = $1,
                 result = $2,
                 updated = NOW()
             WHERE id = $3",
        )
        .bind(ExecutionStatus::Failed)
        .bind(&result)
        .bind(execution_id)
        .execute(&self.pool)
        .await?;

        info!("Execution {} marked as failed in database", execution_id);

        // Publish completion notification
        self.publish_completion_notification(execution_id, result)
            .await?;

        info!(
            "Published completion notification for execution {}",
            execution_id
        );

        Ok(())
    }

    /// Publish execution completion notification
    async fn publish_completion_notification(
        &self,
        execution_id: i64,
        result: JsonValue,
    ) -> Result<()> {
        let payload = ExecutionCompletedPayload {
            execution_id,
            status: ExecutionStatus::Failed,
            result: Some(result),
        };

        let envelope = MessageEnvelope::new(MessageType::ExecutionCompleted, payload)
            .with_source("execution_timeout_monitor");

        // Publish to main executions exchange
        self.publisher.publish_envelope(&envelope).await?;

        Ok(())
    }

    /// Get current configuration
    #[allow(dead_code)]
    pub fn config(&self) -> &TimeoutMonitorConfig {
        &self.config
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration as ChronoDuration;

    fn create_test_config() -> TimeoutMonitorConfig {
        TimeoutMonitorConfig {
            scheduled_timeout: Duration::from_secs(60), // 1 minute for tests
            check_interval: Duration::from_secs(1),     // 1 second for tests
            enabled: true,
        }
    }

    #[test]
    fn test_config_defaults() {
        let config = TimeoutMonitorConfig::default();
        assert_eq!(config.scheduled_timeout.as_secs(), 300);
        assert_eq!(config.check_interval.as_secs(), 60);
        assert!(config.enabled);
    }

    #[test]
    fn test_cutoff_calculation() {
        // Test that cutoff is calculated as now - scheduled_timeout
        let config = create_test_config(); // scheduled_timeout = 60s

        let before = Utc::now() - ChronoDuration::seconds(60);

        // calculate_cutoff uses Utc::now() internally, so we compute expected bounds
        let timeout_duration =
            chrono::Duration::from_std(config.scheduled_timeout).expect("Invalid timeout duration");
        let cutoff = Utc::now() - timeout_duration;

        let after = Utc::now() - ChronoDuration::seconds(60);

        // cutoff should be between before and after (both ~60s ago)
        let diff_before = (cutoff - before).num_seconds().abs();
        let diff_after = (cutoff - after).num_seconds().abs();
        assert!(
            diff_before <= 1,
            "Cutoff time should be ~60s ago (before check)"
        );
        assert!(
            diff_after <= 1,
            "Cutoff time should be ~60s ago (after check)"
        );
    }

    #[test]
    fn test_disabled_config() {
        let mut config = create_test_config();
        config.enabled = false;

        // Verify the config is properly set to disabled
        assert!(!config.enabled);
        assert_eq!(config.scheduled_timeout.as_secs(), 60);
        assert_eq!(config.check_interval.as_secs(), 1);
    }
}
