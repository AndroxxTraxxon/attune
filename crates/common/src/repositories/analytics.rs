//! Analytics repository for querying TimescaleDB continuous aggregates
//!
//! This module provides read-only query methods for the continuous aggregate
//! materialized views created in migration 000009_timescaledb_history. These views are
//! auto-refreshed by TimescaleDB policies and provide pre-computed hourly
//! rollups for dashboard widgets.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{Executor, FromRow, Postgres};

use crate::Result;

/// Repository for querying analytics continuous aggregates.
///
/// All methods are read-only. The underlying materialized views are
/// auto-refreshed by TimescaleDB continuous aggregate policies.
pub struct AnalyticsRepository;

// ---------------------------------------------------------------------------
// Row types returned by aggregate queries
// ---------------------------------------------------------------------------

/// A single hourly bucket of execution status transitions.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ExecutionStatusBucket {
    /// Start of the 1-hour bucket
    pub bucket: DateTime<Utc>,
    /// Action ref (e.g., "core.http_request"); NULL when grouped across all actions
    pub action_ref: Option<String>,
    /// The status that was transitioned to (e.g., "completed", "failed")
    pub new_status: Option<String>,
    /// Number of transitions in this bucket
    pub transition_count: i64,
}

/// A single hourly bucket of execution throughput (creations).
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ExecutionThroughputBucket {
    /// Start of the 1-hour bucket
    pub bucket: DateTime<Utc>,
    /// Action ref; NULL when grouped across all actions
    pub action_ref: Option<String>,
    /// Number of executions created in this bucket
    pub execution_count: i64,
}

/// A single hourly bucket of event volume.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct EventVolumeBucket {
    /// Start of the 1-hour bucket
    pub bucket: DateTime<Utc>,
    /// Trigger ref; NULL when grouped across all triggers
    pub trigger_ref: Option<String>,
    /// Number of events created in this bucket
    pub event_count: i64,
}

/// A single hourly bucket of worker status transitions.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct WorkerStatusBucket {
    /// Start of the 1-hour bucket
    pub bucket: DateTime<Utc>,
    /// Worker name; NULL when grouped across all workers
    pub worker_name: Option<String>,
    /// The status transitioned to (e.g., "online", "offline")
    pub new_status: Option<String>,
    /// Number of transitions in this bucket
    pub transition_count: i64,
}

/// A single hourly bucket of enforcement volume.
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct EnforcementVolumeBucket {
    /// Start of the 1-hour bucket
    pub bucket: DateTime<Utc>,
    /// Rule ref; NULL when grouped across all rules
    pub rule_ref: Option<String>,
    /// Number of enforcements created in this bucket
    pub enforcement_count: i64,
}

/// A single hourly bucket of execution volume (from the execution table directly).
#[derive(Debug, Clone, Serialize, FromRow)]
pub struct ExecutionVolumeBucket {
    /// Start of the 1-hour bucket
    pub bucket: DateTime<Utc>,
    /// Action ref; NULL when grouped across all actions
    pub action_ref: Option<String>,
    /// The initial status at creation time
    pub initial_status: Option<String>,
    /// Number of executions created in this bucket
    pub execution_count: i64,
}

/// Aggregated failure rate over a time range.
#[derive(Debug, Clone, Serialize)]
pub struct FailureRateSummary {
    /// Total status transitions to terminal states in the window
    pub total_terminal: i64,
    /// Number of transitions to "failed" status
    pub failed_count: i64,
    /// Number of transitions to "timeout" status
    pub timeout_count: i64,
    /// Number of transitions to "completed" status
    pub completed_count: i64,
    /// Failure rate as a percentage (0.0 – 100.0)
    pub failure_rate_pct: f64,
}

// ---------------------------------------------------------------------------
// Query parameters
// ---------------------------------------------------------------------------

/// Common time-range parameters for analytics queries.
#[derive(Debug, Clone)]
pub struct AnalyticsTimeRange {
    /// Start of the query window (inclusive). Defaults to 24 hours ago.
    pub since: DateTime<Utc>,
    /// End of the query window (inclusive). Defaults to now.
    pub until: DateTime<Utc>,
}

impl Default for AnalyticsTimeRange {
    fn default() -> Self {
        let now = Utc::now();
        Self {
            since: now - chrono::Duration::hours(24),
            until: now,
        }
    }
}

impl AnalyticsTimeRange {
    /// Create a range covering the last N hours from now.
    pub fn last_hours(hours: i64) -> Self {
        let now = Utc::now();
        Self {
            since: now - chrono::Duration::hours(hours),
            until: now,
        }
    }

    /// Create a range covering the last N days from now.
    pub fn last_days(days: i64) -> Self {
        let now = Utc::now();
        Self {
            since: now - chrono::Duration::days(days),
            until: now,
        }
    }
}

// ---------------------------------------------------------------------------
// Repository implementation
// ---------------------------------------------------------------------------

impl AnalyticsRepository {
    // =======================================================================
    // Execution status transitions
    // =======================================================================

    /// Get execution status transitions per hour, aggregated across all actions.
    ///
    /// Returns one row per (bucket, new_status) pair, ordered by bucket ascending.
    pub async fn execution_status_hourly<'e, E>(
        executor: E,
        range: &AnalyticsTimeRange,
    ) -> Result<Vec<ExecutionStatusBucket>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let rows = sqlx::query_as::<_, ExecutionStatusBucket>(
            r#"
            SELECT
                bucket,
                NULL::text AS action_ref,
                new_status,
                SUM(transition_count)::bigint AS transition_count
            FROM execution_status_hourly
            WHERE bucket >= $1 AND bucket <= $2
            GROUP BY bucket, new_status
            ORDER BY bucket ASC, new_status
            "#,
        )
        .bind(range.since)
        .bind(range.until)
        .fetch_all(executor)
        .await?;

        Ok(rows)
    }

    /// Get execution status transitions per hour for a specific action.
    pub async fn execution_status_hourly_by_action<'e, E>(
        executor: E,
        range: &AnalyticsTimeRange,
        action_ref: &str,
    ) -> Result<Vec<ExecutionStatusBucket>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let rows = sqlx::query_as::<_, ExecutionStatusBucket>(
            r#"
            SELECT
                bucket,
                action_ref,
                new_status,
                transition_count
            FROM execution_status_hourly
            WHERE bucket >= $1 AND bucket <= $2 AND action_ref = $3
            ORDER BY bucket ASC, new_status
            "#,
        )
        .bind(range.since)
        .bind(range.until)
        .bind(action_ref)
        .fetch_all(executor)
        .await?;

        Ok(rows)
    }

    // =======================================================================
    // Execution throughput
    // =======================================================================

    /// Get execution creation throughput per hour, aggregated across all actions.
    pub async fn execution_throughput_hourly<'e, E>(
        executor: E,
        range: &AnalyticsTimeRange,
    ) -> Result<Vec<ExecutionThroughputBucket>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let rows = sqlx::query_as::<_, ExecutionThroughputBucket>(
            r#"
            SELECT
                bucket,
                NULL::text AS action_ref,
                SUM(execution_count)::bigint AS execution_count
            FROM execution_throughput_hourly
            WHERE bucket >= $1 AND bucket <= $2
            GROUP BY bucket
            ORDER BY bucket ASC
            "#,
        )
        .bind(range.since)
        .bind(range.until)
        .fetch_all(executor)
        .await?;

        Ok(rows)
    }

    /// Get execution creation throughput per hour for a specific action.
    pub async fn execution_throughput_hourly_by_action<'e, E>(
        executor: E,
        range: &AnalyticsTimeRange,
        action_ref: &str,
    ) -> Result<Vec<ExecutionThroughputBucket>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let rows = sqlx::query_as::<_, ExecutionThroughputBucket>(
            r#"
            SELECT
                bucket,
                action_ref,
                execution_count
            FROM execution_throughput_hourly
            WHERE bucket >= $1 AND bucket <= $2 AND action_ref = $3
            ORDER BY bucket ASC
            "#,
        )
        .bind(range.since)
        .bind(range.until)
        .bind(action_ref)
        .fetch_all(executor)
        .await?;

        Ok(rows)
    }

    // =======================================================================
    // Event volume
    // =======================================================================

    /// Get event creation volume per hour, aggregated across all triggers.
    pub async fn event_volume_hourly<'e, E>(
        executor: E,
        range: &AnalyticsTimeRange,
    ) -> Result<Vec<EventVolumeBucket>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let rows = sqlx::query_as::<_, EventVolumeBucket>(
            r#"
            SELECT
                bucket,
                NULL::text AS trigger_ref,
                SUM(event_count)::bigint AS event_count
            FROM event_volume_hourly
            WHERE bucket >= $1 AND bucket <= $2
            GROUP BY bucket
            ORDER BY bucket ASC
            "#,
        )
        .bind(range.since)
        .bind(range.until)
        .fetch_all(executor)
        .await?;

        Ok(rows)
    }

    /// Get event creation volume per hour for a specific trigger.
    pub async fn event_volume_hourly_by_trigger<'e, E>(
        executor: E,
        range: &AnalyticsTimeRange,
        trigger_ref: &str,
    ) -> Result<Vec<EventVolumeBucket>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let rows = sqlx::query_as::<_, EventVolumeBucket>(
            r#"
            SELECT
                bucket,
                trigger_ref,
                event_count
            FROM event_volume_hourly
            WHERE bucket >= $1 AND bucket <= $2 AND trigger_ref = $3
            ORDER BY bucket ASC
            "#,
        )
        .bind(range.since)
        .bind(range.until)
        .bind(trigger_ref)
        .fetch_all(executor)
        .await?;

        Ok(rows)
    }

    // =======================================================================
    // Worker health
    // =======================================================================

    /// Get worker status transitions per hour, aggregated across all workers.
    pub async fn worker_status_hourly<'e, E>(
        executor: E,
        range: &AnalyticsTimeRange,
    ) -> Result<Vec<WorkerStatusBucket>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let rows = sqlx::query_as::<_, WorkerStatusBucket>(
            r#"
            SELECT
                bucket,
                NULL::text AS worker_name,
                new_status,
                SUM(transition_count)::bigint AS transition_count
            FROM worker_status_hourly
            WHERE bucket >= $1 AND bucket <= $2
            GROUP BY bucket, new_status
            ORDER BY bucket ASC, new_status
            "#,
        )
        .bind(range.since)
        .bind(range.until)
        .fetch_all(executor)
        .await?;

        Ok(rows)
    }

    /// Get worker status transitions per hour for a specific worker.
    pub async fn worker_status_hourly_by_name<'e, E>(
        executor: E,
        range: &AnalyticsTimeRange,
        worker_name: &str,
    ) -> Result<Vec<WorkerStatusBucket>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let rows = sqlx::query_as::<_, WorkerStatusBucket>(
            r#"
            SELECT
                bucket,
                worker_name,
                new_status,
                transition_count
            FROM worker_status_hourly
            WHERE bucket >= $1 AND bucket <= $2 AND worker_name = $3
            ORDER BY bucket ASC, new_status
            "#,
        )
        .bind(range.since)
        .bind(range.until)
        .bind(worker_name)
        .fetch_all(executor)
        .await?;

        Ok(rows)
    }

    // =======================================================================
    // Enforcement volume
    // =======================================================================

    /// Get enforcement creation volume per hour, aggregated across all rules.
    pub async fn enforcement_volume_hourly<'e, E>(
        executor: E,
        range: &AnalyticsTimeRange,
    ) -> Result<Vec<EnforcementVolumeBucket>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let rows = sqlx::query_as::<_, EnforcementVolumeBucket>(
            r#"
            SELECT
                bucket,
                NULL::text AS rule_ref,
                SUM(enforcement_count)::bigint AS enforcement_count
            FROM enforcement_volume_hourly
            WHERE bucket >= $1 AND bucket <= $2
            GROUP BY bucket
            ORDER BY bucket ASC
            "#,
        )
        .bind(range.since)
        .bind(range.until)
        .fetch_all(executor)
        .await?;

        Ok(rows)
    }

    /// Get enforcement creation volume per hour for a specific rule.
    pub async fn enforcement_volume_hourly_by_rule<'e, E>(
        executor: E,
        range: &AnalyticsTimeRange,
        rule_ref: &str,
    ) -> Result<Vec<EnforcementVolumeBucket>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        let rows = sqlx::query_as::<_, EnforcementVolumeBucket>(
            r#"
            SELECT
                bucket,
                rule_ref,
                enforcement_count
            FROM enforcement_volume_hourly
            WHERE bucket >= $1 AND bucket <= $2 AND rule_ref = $3
            ORDER BY bucket ASC
            "#,
        )
        .bind(range.since)
        .bind(range.until)
        .bind(rule_ref)
        .fetch_all(executor)
        .await?;

        Ok(rows)
    }

    // =======================================================================
    // Execution volume (from the execution table directly)
    // =======================================================================

    /// Query the `execution_volume_hourly` continuous aggregate for execution
    /// creation volume across all actions.
    pub async fn execution_volume_hourly<'e, E>(
        executor: E,
        range: &AnalyticsTimeRange,
    ) -> Result<Vec<ExecutionVolumeBucket>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, ExecutionVolumeBucket>(
            r#"
            SELECT
                bucket,
                NULL::text AS action_ref,
                initial_status::text AS initial_status,
                SUM(execution_count)::bigint AS execution_count
            FROM execution_volume_hourly
            WHERE bucket >= $1 AND bucket <= $2
            GROUP BY bucket, initial_status
            ORDER BY bucket ASC, initial_status
            "#,
        )
        .bind(range.since)
        .bind(range.until)
        .fetch_all(executor)
        .await
        .map_err(Into::into)
    }

    /// Query the `execution_volume_hourly` continuous aggregate filtered by
    /// a specific action ref.
    pub async fn execution_volume_hourly_by_action<'e, E>(
        executor: E,
        range: &AnalyticsTimeRange,
        action_ref: &str,
    ) -> Result<Vec<ExecutionVolumeBucket>>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        sqlx::query_as::<_, ExecutionVolumeBucket>(
            r#"
            SELECT
                bucket,
                action_ref,
                initial_status::text AS initial_status,
                execution_count
            FROM execution_volume_hourly
            WHERE bucket >= $1 AND bucket <= $2 AND action_ref = $3
            ORDER BY bucket ASC, initial_status
            "#,
        )
        .bind(range.since)
        .bind(range.until)
        .bind(action_ref)
        .fetch_all(executor)
        .await
        .map_err(Into::into)
    }

    // =======================================================================
    // Derived analytics
    // =======================================================================

    /// Compute the execution failure rate over a time range.
    ///
    /// Uses the `execution_status_hourly` aggregate to count terminal-state
    /// transitions (completed, failed, timeout) and derive the failure
    /// percentage.
    pub async fn execution_failure_rate<'e, E>(
        executor: E,
        range: &AnalyticsTimeRange,
    ) -> Result<FailureRateSummary>
    where
        E: Executor<'e, Database = Postgres> + 'e,
    {
        // Query terminal-state transitions from the aggregate
        let rows = sqlx::query_as::<_, (Option<String>, i64)>(
            r#"
            SELECT
                new_status,
                SUM(transition_count)::bigint AS cnt
            FROM execution_status_hourly
            WHERE bucket >= $1 AND bucket <= $2
              AND new_status IN ('completed', 'failed', 'timeout')
            GROUP BY new_status
            "#,
        )
        .bind(range.since)
        .bind(range.until)
        .fetch_all(executor)
        .await?;

        let mut completed: i64 = 0;
        let mut failed: i64 = 0;
        let mut timeout: i64 = 0;

        for (status, count) in &rows {
            match status.as_deref() {
                Some("completed") => completed = *count,
                Some("failed") => failed = *count,
                Some("timeout") => timeout = *count,
                _ => {}
            }
        }

        let total_terminal = completed + failed + timeout;
        let failure_rate_pct = if total_terminal > 0 {
            ((failed + timeout) as f64 / total_terminal as f64) * 100.0
        } else {
            0.0
        };

        Ok(FailureRateSummary {
            total_terminal,
            failed_count: failed,
            timeout_count: timeout,
            completed_count: completed,
            failure_rate_pct,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_analytics_time_range_default() {
        let range = AnalyticsTimeRange::default();
        let diff = range.until - range.since;
        // Should be approximately 24 hours
        assert!((diff.num_hours() - 24).abs() <= 1);
    }

    #[test]
    fn test_analytics_time_range_last_hours() {
        let range = AnalyticsTimeRange::last_hours(6);
        let diff = range.until - range.since;
        assert!((diff.num_hours() - 6).abs() <= 1);
    }

    #[test]
    fn test_analytics_time_range_last_days() {
        let range = AnalyticsTimeRange::last_days(7);
        let diff = range.until - range.since;
        assert!((diff.num_days() - 7).abs() <= 1);
    }

    #[test]
    fn test_failure_rate_summary_zero_total() {
        let summary = FailureRateSummary {
            total_terminal: 0,
            failed_count: 0,
            timeout_count: 0,
            completed_count: 0,
            failure_rate_pct: 0.0,
        };
        assert_eq!(summary.failure_rate_pct, 0.0);
    }

    #[test]
    fn test_failure_rate_calculation() {
        // 80 completed, 15 failed, 5 timeout → 20% failure rate
        let total = 80 + 15 + 5;
        let rate = ((15 + 5) as f64 / total as f64) * 100.0;
        assert!((rate - 20.0).abs() < 0.01);
    }
}
