//! Worker Health Probe
//!
//! This module provides proactive health checking for workers.
//! It tracks worker health metrics, detects degraded/unhealthy workers,
//! and provides health-aware worker selection.
//!
//! # Health States
//!
//! - **Healthy:** Worker is responsive and performing well
//! - **Degraded:** Worker is functional but showing signs of issues
//! - **Unhealthy:** Worker should not receive new executions
//!
//! # Health Metrics
//!
//! - Queue depth (from worker self-reporting)
//! - Consecutive failures
//! - Average execution time
//! - Heartbeat freshness

use attune_common::{
    error::{Error, Result},
    models::{Id, Worker, WorkerStatus},
    repositories::{FindById, List, WorkerRepository},
};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::Arc;
use tracing::{debug, info, warn};

/// Worker health state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// Worker is healthy and performing well
    Healthy,
    /// Worker is functional but showing issues
    Degraded,
    /// Worker should not receive new tasks
    Unhealthy,
}

impl HealthStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Healthy => "healthy",
            Self::Degraded => "degraded",
            Self::Unhealthy => "unhealthy",
        }
    }
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Worker health metrics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthMetrics {
    /// Current health status
    pub status: HealthStatus,
    /// Last health check time
    pub last_check: DateTime<Utc>,
    /// Consecutive failures
    pub consecutive_failures: u32,
    /// Total executions handled
    pub total_executions: u64,
    /// Failed executions
    pub failed_executions: u64,
    /// Average execution time in milliseconds
    pub average_execution_time_ms: u64,
    /// Current queue depth (estimated)
    pub queue_depth: u32,
}

impl Default for HealthMetrics {
    fn default() -> Self {
        Self {
            status: HealthStatus::Healthy,
            last_check: Utc::now(),
            consecutive_failures: 0,
            total_executions: 0,
            failed_executions: 0,
            average_execution_time_ms: 0,
            queue_depth: 0,
        }
    }
}

/// Health probe configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthProbeConfig {
    /// Enable health probing
    pub enabled: bool,
    /// Heartbeat staleness threshold in seconds
    pub heartbeat_max_age_secs: u64,
    /// Consecutive failures before marking degraded
    pub degraded_threshold: u32,
    /// Consecutive failures before marking unhealthy
    pub unhealthy_threshold: u32,
    /// Queue depth to consider degraded
    pub queue_depth_degraded: u32,
    /// Queue depth to consider unhealthy
    pub queue_depth_unhealthy: u32,
    /// Failure rate threshold for degraded (0.0 - 1.0)
    pub failure_rate_degraded: f64,
    /// Failure rate threshold for unhealthy (0.0 - 1.0)
    pub failure_rate_unhealthy: f64,
}

impl Default for HealthProbeConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            heartbeat_max_age_secs: 30,
            degraded_threshold: 3,
            unhealthy_threshold: 10,
            queue_depth_degraded: 50,
            queue_depth_unhealthy: 100,
            failure_rate_degraded: 0.3,  // 30%
            failure_rate_unhealthy: 0.7, // 70%
        }
    }
}

/// Worker health probe
pub struct WorkerHealthProbe {
    /// Database connection pool
    pool: Arc<PgPool>,
    /// Configuration
    config: HealthProbeConfig,
}

impl WorkerHealthProbe {
    /// Create a new health probe
    #[allow(dead_code)]
    pub fn new(pool: Arc<PgPool>, config: HealthProbeConfig) -> Self {
        Self { pool, config }
    }

    /// Create with default configuration
    #[allow(dead_code)]
    pub fn with_defaults(pool: Arc<PgPool>) -> Self {
        Self::new(pool, HealthProbeConfig::default())
    }

    /// Check health of a specific worker
    #[allow(dead_code)]
    pub async fn check_worker(&self, worker_id: Id) -> Result<HealthMetrics> {
        let worker = WorkerRepository::find_by_id(&*self.pool, worker_id)
            .await?
            .ok_or_else(|| Error::not_found("Worker", "id", worker_id.to_string()))?;

        self.evaluate_health(&worker)
    }

    /// Get all healthy workers
    #[allow(dead_code)]
    pub async fn get_healthy_workers(&self) -> Result<Vec<Worker>> {
        let workers = WorkerRepository::list(&*self.pool).await?;

        let mut healthy = Vec::new();
        for worker in workers {
            if self.is_worker_healthy(&worker).await {
                healthy.push(worker);
            }
        }

        Ok(healthy)
    }

    /// Get workers sorted by health (healthiest first)
    #[allow(dead_code)]
    pub async fn get_workers_by_health(&self) -> Result<Vec<(Worker, HealthMetrics)>> {
        let workers = WorkerRepository::list(&*self.pool).await?;

        let mut worker_health = Vec::new();
        for worker in workers {
            match self.evaluate_health(&worker) {
                Ok(metrics) => worker_health.push((worker, metrics)),
                Err(e) => warn!("Failed to evaluate health for worker {}: {}", worker.id, e),
            }
        }

        // Sort by health status (healthy first), then by queue depth
        worker_health.sort_by(|a, b| match (a.1.status, b.1.status) {
            (HealthStatus::Healthy, HealthStatus::Healthy) => a.1.queue_depth.cmp(&b.1.queue_depth),
            (HealthStatus::Healthy, _) => std::cmp::Ordering::Less,
            (_, HealthStatus::Healthy) => std::cmp::Ordering::Greater,
            (HealthStatus::Degraded, HealthStatus::Degraded) => {
                a.1.queue_depth.cmp(&b.1.queue_depth)
            }
            (HealthStatus::Degraded, HealthStatus::Unhealthy) => std::cmp::Ordering::Less,
            (HealthStatus::Unhealthy, HealthStatus::Degraded) => std::cmp::Ordering::Greater,
            (HealthStatus::Unhealthy, HealthStatus::Unhealthy) => {
                a.1.queue_depth.cmp(&b.1.queue_depth)
            }
        });

        Ok(worker_health)
    }

    /// Check if worker is healthy (simple boolean check)
    #[allow(dead_code)]
    pub async fn is_worker_healthy(&self, worker: &Worker) -> bool {
        // Check basic status
        if worker.status != Some(WorkerStatus::Active) {
            return false;
        }

        // Check heartbeat freshness
        if !self.is_heartbeat_fresh(worker) {
            return false;
        }

        // Evaluate detailed health
        match self.evaluate_health(worker) {
            Ok(metrics) => matches!(
                metrics.status,
                HealthStatus::Healthy | HealthStatus::Degraded
            ),
            Err(_) => false,
        }
    }

    /// Evaluate worker health based on metrics
    fn evaluate_health(&self, worker: &Worker) -> Result<HealthMetrics> {
        // Extract health metrics from capabilities
        let metrics = self.extract_health_metrics(worker);

        // Check heartbeat
        if !self.is_heartbeat_fresh(worker) {
            return Ok(HealthMetrics {
                status: HealthStatus::Unhealthy,
                ..metrics
            });
        }

        // Calculate failure rate
        let failure_rate = if metrics.total_executions > 0 {
            metrics.failed_executions as f64 / metrics.total_executions as f64
        } else {
            0.0
        };

        // Determine health status based on thresholds
        let status = if metrics.consecutive_failures >= self.config.unhealthy_threshold
            || metrics.queue_depth >= self.config.queue_depth_unhealthy
            || failure_rate >= self.config.failure_rate_unhealthy
        {
            HealthStatus::Unhealthy
        } else if metrics.consecutive_failures >= self.config.degraded_threshold
            || metrics.queue_depth >= self.config.queue_depth_degraded
            || failure_rate >= self.config.failure_rate_degraded
        {
            HealthStatus::Degraded
        } else {
            HealthStatus::Healthy
        };

        debug!(
            "Worker {} health: {:?} (failures: {}, queue: {}, failure_rate: {:.2}%)",
            worker.name,
            status,
            metrics.consecutive_failures,
            metrics.queue_depth,
            failure_rate * 100.0
        );

        Ok(HealthMetrics { status, ..metrics })
    }

    /// Check if worker heartbeat is fresh
    fn is_heartbeat_fresh(&self, worker: &Worker) -> bool {
        let Some(last_heartbeat) = worker.last_heartbeat else {
            warn!("Worker {} has no heartbeat", worker.name);
            return false;
        };

        let age = Utc::now() - last_heartbeat;
        let max_age = Duration::seconds(self.config.heartbeat_max_age_secs as i64);

        if age > max_age {
            warn!(
                "Worker {} heartbeat stale: {} seconds old (max: {})",
                worker.name,
                age.num_seconds(),
                max_age.num_seconds()
            );
            return false;
        }

        true
    }

    /// Extract health metrics from worker capabilities
    fn extract_health_metrics(&self, worker: &Worker) -> HealthMetrics {
        let mut metrics = HealthMetrics {
            last_check: Utc::now(),
            ..Default::default()
        };

        let Some(capabilities) = &worker.capabilities else {
            return metrics;
        };

        let Some(health_obj) = capabilities.get("health") else {
            return metrics;
        };

        // Extract metrics from health object
        if let Some(status_str) = health_obj.get("status").and_then(|v| v.as_str()) {
            metrics.status = match status_str {
                "healthy" => HealthStatus::Healthy,
                "degraded" => HealthStatus::Degraded,
                "unhealthy" => HealthStatus::Unhealthy,
                _ => HealthStatus::Healthy,
            };
        }

        if let Some(last_check_str) = health_obj.get("last_check").and_then(|v| v.as_str()) {
            if let Ok(last_check) = DateTime::parse_from_rfc3339(last_check_str) {
                metrics.last_check = last_check.with_timezone(&Utc);
            }
        }

        if let Some(failures) = health_obj
            .get("consecutive_failures")
            .and_then(|v| v.as_u64())
        {
            metrics.consecutive_failures = failures as u32;
        }

        if let Some(total) = health_obj.get("total_executions").and_then(|v| v.as_u64()) {
            metrics.total_executions = total;
        }

        if let Some(failed) = health_obj.get("failed_executions").and_then(|v| v.as_u64()) {
            metrics.failed_executions = failed;
        }

        if let Some(avg_time) = health_obj
            .get("average_execution_time_ms")
            .and_then(|v| v.as_u64())
        {
            metrics.average_execution_time_ms = avg_time;
        }

        if let Some(depth) = health_obj.get("queue_depth").and_then(|v| v.as_u64()) {
            metrics.queue_depth = depth as u32;
        }

        metrics
    }

    /// Get recommended worker for execution based on health
    #[allow(dead_code)]
    pub async fn get_best_worker(&self, runtime_name: &str) -> Result<Option<Worker>> {
        let workers_by_health = self.get_workers_by_health().await?;

        // Filter by runtime and health
        for (worker, metrics) in workers_by_health {
            // Skip unhealthy workers
            if metrics.status == HealthStatus::Unhealthy {
                continue;
            }

            // Check runtime support
            if self.worker_supports_runtime(&worker, runtime_name) {
                info!(
                    "Selected worker {} (health: {:?}, queue: {}) for runtime '{}'",
                    worker.name, metrics.status, metrics.queue_depth, runtime_name
                );
                return Ok(Some(worker));
            }
        }

        warn!("No healthy worker found for runtime '{}'", runtime_name);
        Ok(None)
    }

    /// Check if worker supports a runtime
    fn worker_supports_runtime(&self, worker: &Worker, runtime_name: &str) -> bool {
        let Some(capabilities) = &worker.capabilities else {
            return false;
        };

        let Some(runtimes) = capabilities.get("runtimes") else {
            return false;
        };

        let Some(runtime_array) = runtimes.as_array() else {
            return false;
        };

        runtime_array.iter().any(|v| {
            v.as_str()
                .map_or(false, |s| s.eq_ignore_ascii_case(runtime_name))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_health_status_display() {
        assert_eq!(HealthStatus::Healthy.to_string(), "healthy");
        assert_eq!(HealthStatus::Degraded.to_string(), "degraded");
        assert_eq!(HealthStatus::Unhealthy.to_string(), "unhealthy");
    }

    #[test]
    fn test_default_health_metrics() {
        let metrics = HealthMetrics::default();
        assert_eq!(metrics.status, HealthStatus::Healthy);
        assert_eq!(metrics.consecutive_failures, 0);
        assert_eq!(metrics.queue_depth, 0);
    }

    #[test]
    fn test_health_probe_config_defaults() {
        let config = HealthProbeConfig::default();
        assert!(config.enabled);
        assert_eq!(config.heartbeat_max_age_secs, 30);
        assert_eq!(config.degraded_threshold, 3);
        assert_eq!(config.unhealthy_threshold, 10);
        assert_eq!(config.queue_depth_degraded, 50);
        assert_eq!(config.queue_depth_unhealthy, 100);
    }

    #[test]
    fn test_extract_health_metrics() {
        let probe = WorkerHealthProbe::with_defaults(Arc::new(unsafe { std::mem::zeroed() }));

        let worker = Worker {
            id: 1,
            name: "test-worker".to_string(),
            worker_type: attune_common::models::WorkerType::Container,
            worker_role: attune_common::models::WorkerRole::Action,
            runtime: None,
            host: None,
            port: None,
            status: Some(WorkerStatus::Active),
            capabilities: Some(json!({
                "health": {
                    "status": "degraded",
                    "consecutive_failures": 5,
                    "queue_depth": 25,
                    "total_executions": 100,
                    "failed_executions": 10
                }
            })),
            meta: None,
            last_heartbeat: Some(Utc::now()),
            created: Utc::now(),
            updated: Utc::now(),
        };

        let metrics = probe.extract_health_metrics(&worker);
        assert_eq!(metrics.status, HealthStatus::Degraded);
        assert_eq!(metrics.consecutive_failures, 5);
        assert_eq!(metrics.queue_depth, 25);
        assert_eq!(metrics.total_executions, 100);
        assert_eq!(metrics.failed_executions, 10);
    }
}
