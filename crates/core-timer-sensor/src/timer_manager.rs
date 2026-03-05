//! Timer Manager
//!
//! Manages individual timer tasks for each rule, with support for:
//! - Interval-based timers (fires every N seconds/minutes/hours/days)
//! - Cron-based timers (fires based on cron expressions)
//! - DateTime-based timers (fires once at a specific time)

use crate::api_client::{ApiClient, CreateEventRequest};
use crate::types::{TimeUnit, TimerConfig};
use anyhow::Result;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{Mutex, RwLock};
use tokio_cron_scheduler::{Job, JobScheduler};
use tracing::{debug, error, info, warn};
use uuid::Uuid;

/// Timer manager for handling per-rule timers
#[derive(Clone)]
pub struct TimerManager {
    inner: Arc<TimerManagerInner>,
}

struct TimerManagerInner {
    /// Map of rule_id -> job UUID in the scheduler
    active_jobs: RwLock<HashMap<i64, Uuid>>,
    /// Shared cron scheduler for all timer types (wrapped in Mutex for shutdown)
    scheduler: Mutex<JobScheduler>,
    /// API client for creating events
    api_client: ApiClient,
}

impl TimerManager {
    /// Create a new timer manager
    pub async fn new(api_client: ApiClient) -> Result<Self> {
        let scheduler = JobScheduler::new().await?;

        // Start the scheduler
        scheduler.start().await?;

        Ok(Self {
            inner: Arc::new(TimerManagerInner {
                active_jobs: RwLock::new(HashMap::new()),
                scheduler: Mutex::new(scheduler),
                api_client,
            }),
        })
    }

    /// Start a timer for a rule
    pub async fn start_timer(&self, rule_id: i64, config: TimerConfig) -> Result<()> {
        // Stop existing timer if any
        self.stop_timer(rule_id).await;

        info!("Starting timer for rule {}: {:?}", rule_id, config);

        // Create appropriate job type
        let job = match &config {
            TimerConfig::Interval { interval, unit } => {
                self.create_interval_job(rule_id, *interval, *unit).await?
            }
            TimerConfig::Cron { expression } => {
                self.create_cron_job(rule_id, expression.clone()).await?
            }
            TimerConfig::DateTime { fire_at } => {
                self.create_datetime_job(rule_id, *fire_at).await?
            }
        };

        // Add job to scheduler and store UUID
        let job_uuid = self.inner.scheduler.lock().await.add(job).await?;
        self.inner
            .active_jobs
            .write()
            .await
            .insert(rule_id, job_uuid);

        info!(
            "Timer started for rule {} with job UUID {}",
            rule_id, job_uuid
        );

        Ok(())
    }

    /// Stop a timer for a rule
    pub async fn stop_timer(&self, rule_id: i64) {
        let mut active_jobs = self.inner.active_jobs.write().await;

        if let Some(job_uuid) = active_jobs.remove(&rule_id) {
            if let Err(e) = self.inner.scheduler.lock().await.remove(&job_uuid).await {
                warn!(
                    "Failed to remove job {} for rule {}: {}",
                    job_uuid, rule_id, e
                );
            } else {
                info!("Stopped timer for rule {}", rule_id);
            }
        } else {
            debug!("No timer found for rule {}", rule_id);
        }
    }

    /// Stop all timers
    pub async fn stop_all(&self) {
        let mut active_jobs = self.inner.active_jobs.write().await;

        let count = active_jobs.len();
        for (rule_id, job_uuid) in active_jobs.drain() {
            if let Err(e) = self.inner.scheduler.lock().await.remove(&job_uuid).await {
                warn!(
                    "Failed to remove job {} for rule {}: {}",
                    job_uuid, rule_id, e
                );
            } else {
                debug!("Stopped timer for rule {}", rule_id);
            }
        }

        info!("Stopped {} timers", count);
    }

    /// Get count of active timers
    #[allow(dead_code)]
    pub async fn timer_count(&self) -> usize {
        self.inner.active_jobs.read().await.len()
    }

    /// Shutdown the scheduler
    pub async fn shutdown(&self) -> Result<()> {
        info!("Shutting down timer manager");
        self.stop_all().await;
        self.inner.scheduler.lock().await.shutdown().await?;
        Ok(())
    }

    /// Create an interval-based job
    async fn create_interval_job(
        &self,
        rule_id: i64,
        interval: u64,
        unit: TimeUnit,
    ) -> Result<Job> {
        let interval_seconds = match unit {
            TimeUnit::Seconds => interval,
            TimeUnit::Minutes => interval * 60,
            TimeUnit::Hours => interval * 3600,
            TimeUnit::Days => interval * 86400,
        };

        if interval_seconds == 0 {
            return Err(anyhow::anyhow!("Interval must be greater than 0"));
        }

        let api_client = self.inner.api_client.clone();
        let duration = Duration::from_secs(interval_seconds);

        info!(
            "Creating interval job for rule {} (interval: {}s)",
            rule_id, interval_seconds
        );

        let mut execution_count = 0u64;

        let job = Job::new_repeated_async(duration, move |_uuid, _lock| {
            let api_client = api_client.clone();
            let rule_id = rule_id;
            execution_count += 1;
            let count = execution_count;
            let interval_secs = interval_seconds;

            Box::pin(async move {
                let now = Utc::now();

                // Create event payload matching intervaltimer output schema
                let payload = serde_json::json!({
                    "type": "interval",
                    "interval_seconds": interval_secs,
                    "fired_at": now.to_rfc3339(),
                    "execution_count": count,
                    "sensor_ref": "core.interval_timer_sensor",
                });

                // Create event via API
                let request = CreateEventRequest::new("core.intervaltimer".to_string(), payload)
                    .with_trigger_instance_id(format!("rule_{}", rule_id));

                match api_client.create_event_with_retry(request).await {
                    Ok(event_id) => {
                        info!(
                            "Interval timer fired for rule {} (count: {}), created event {}",
                            rule_id, count, event_id
                        );
                    }
                    Err(e) => {
                        error!(
                            "Failed to create event for rule {} interval timer: {}",
                            rule_id, e
                        );
                    }
                }
            })
        })?;

        Ok(job)
    }

    /// Create a cron-based job
    async fn create_cron_job(&self, rule_id: i64, expression: String) -> Result<Job> {
        info!(
            "Creating cron job for rule {} with expression: {}",
            rule_id, expression
        );

        let api_client = self.inner.api_client.clone();
        let expr_clone = expression.clone();

        let mut execution_count = 0u64;

        let job = Job::new_async(&expression, move |uuid, mut lock| {
            let api_client = api_client.clone();
            let rule_id = rule_id;
            let expression = expr_clone.clone();
            execution_count += 1;
            let count = execution_count;

            Box::pin(async move {
                let now = Utc::now();

                // Get next scheduled time
                let next_fire = match lock.next_tick_for_job(uuid).await {
                    Ok(Some(ts)) => ts.to_rfc3339(),
                    Ok(None) => "unknown".to_string(),
                    Err(e) => {
                        warn!("Failed to get next tick for cron job {}: {}", uuid, e);
                        "unknown".to_string()
                    }
                };

                // Create event payload matching crontimer output schema
                let payload = serde_json::json!({
                    "type": "cron",
                    "fired_at": now.to_rfc3339(),
                    "scheduled_at": now.to_rfc3339(),
                    "expression": expression,
                    "timezone": "UTC",
                    "next_fire_at": next_fire,
                    "execution_count": count,
                    "sensor_ref": "core.interval_timer_sensor",
                });

                // Create event via API
                let request = CreateEventRequest::new("core.crontimer".to_string(), payload)
                    .with_trigger_instance_id(format!("rule_{}", rule_id));

                match api_client.create_event_with_retry(request).await {
                    Ok(event_id) => {
                        info!(
                            "Cron timer fired for rule {} (count: {}), created event {}",
                            rule_id, count, event_id
                        );
                    }
                    Err(e) => {
                        error!(
                            "Failed to create event for rule {} cron timer: {}",
                            rule_id, e
                        );
                    }
                }
            })
        })?;

        Ok(job)
    }

    /// Create a datetime-based (one-shot) job
    async fn create_datetime_job(
        &self,
        rule_id: i64,
        fire_at: chrono::DateTime<Utc>,
    ) -> Result<Job> {
        let now = Utc::now();

        if fire_at <= now {
            return Err(anyhow::anyhow!(
                "DateTime timer fire_at must be in the future"
            ));
        }

        let duration = (fire_at - now)
            .to_std()
            .map_err(|e| anyhow::anyhow!("Invalid duration: {}", e))?;

        info!(
            "Creating one-shot job for rule {} scheduled at {}",
            rule_id,
            fire_at.to_rfc3339()
        );

        let api_client = self.inner.api_client.clone();
        let scheduled_time = fire_at.to_rfc3339();

        let job = Job::new_one_shot_async(duration, move |_uuid, _lock| {
            let api_client = api_client.clone();
            let rule_id = rule_id;
            let scheduled_time = scheduled_time.clone();

            Box::pin(async move {
                let now = Utc::now();

                // Calculate delay between scheduled and actual fire time
                let delay_ms = (now.timestamp_millis() - fire_at.timestamp_millis()).max(0);

                // Create event payload matching datetimetimer output schema
                let payload = serde_json::json!({
                    "type": "one_shot",
                    "fire_at": scheduled_time,
                    "fired_at": now.to_rfc3339(),
                    "timezone": "UTC",
                    "delay_ms": delay_ms,
                    "sensor_ref": "core.interval_timer_sensor",
                });

                // Create event via API
                let request = CreateEventRequest::new("core.datetimetimer".to_string(), payload)
                    .with_trigger_instance_id(format!("rule_{}", rule_id));

                match api_client.create_event_with_retry(request).await {
                    Ok(event_id) => {
                        info!(
                            "DateTime timer fired for rule {}, created event {}",
                            rule_id, event_id
                        );
                    }
                    Err(e) => {
                        error!(
                            "Failed to create event for rule {} datetime timer: {}",
                            rule_id, e
                        );
                    }
                }

                info!("One-shot timer completed for rule {}", rule_id);
            })
        })?;

        Ok(job)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_timer_manager_creation() {
        let api_client = ApiClient::new("http://localhost:8080".to_string(), "token".to_string());
        let manager = TimerManager::new(api_client).await.unwrap();
        assert_eq!(manager.timer_count().await, 0);
        manager.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_timer_manager_start_stop() {
        let api_client = ApiClient::new("http://localhost:8080".to_string(), "token".to_string());
        let manager = TimerManager::new(api_client).await.unwrap();

        let config = TimerConfig::Interval {
            interval: 60,
            unit: TimeUnit::Seconds,
        };

        // Start timer
        manager.start_timer(1, config).await.unwrap();
        assert_eq!(manager.timer_count().await, 1);

        // Stop timer
        manager.stop_timer(1).await;
        assert_eq!(manager.timer_count().await, 0);

        manager.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_timer_manager_stop_all() {
        let api_client = ApiClient::new("http://localhost:8080".to_string(), "token".to_string());
        let manager = TimerManager::new(api_client).await.unwrap();

        let config = TimerConfig::Interval {
            interval: 60,
            unit: TimeUnit::Seconds,
        };

        // Start multiple timers
        manager.start_timer(1, config.clone()).await.unwrap();
        manager.start_timer(2, config.clone()).await.unwrap();
        manager.start_timer(3, config).await.unwrap();

        assert_eq!(manager.timer_count().await, 3);

        // Stop all
        manager.stop_all().await;
        assert_eq!(manager.timer_count().await, 0);

        manager.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_interval_timer_validation() {
        let api_client = ApiClient::new("http://localhost:8080".to_string(), "token".to_string());
        let manager = TimerManager::new(api_client).await.unwrap();

        let config = TimerConfig::Interval {
            interval: 0,
            unit: TimeUnit::Seconds,
        };

        // Should fail with zero interval
        let result = manager.start_timer(1, config).await;
        assert!(result.is_err());

        manager.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_datetime_timer_validation() {
        let api_client = ApiClient::new("http://localhost:8080".to_string(), "token".to_string());
        let manager = TimerManager::new(api_client).await.unwrap();

        // Create a datetime in the past
        let past = Utc::now() - chrono::Duration::seconds(60);
        let config = TimerConfig::DateTime { fire_at: past };

        // Should fail with past datetime
        let result = manager.start_timer(1, config).await;
        assert!(result.is_err());

        manager.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_cron_timer_creation() {
        let api_client = ApiClient::new("http://localhost:8080".to_string(), "token".to_string());
        let manager = TimerManager::new(api_client).await.unwrap();

        // Valid cron expression: every minute
        let config = TimerConfig::Cron {
            expression: "0 * * * * *".to_string(),
        };

        // Should succeed
        let result = manager.start_timer(1, config).await;
        assert!(result.is_ok());
        assert_eq!(manager.timer_count().await, 1);

        manager.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_cron_timer_invalid_expression() {
        let api_client = ApiClient::new("http://localhost:8080".to_string(), "token".to_string());
        let manager = TimerManager::new(api_client).await.unwrap();

        // Invalid cron expression
        let config = TimerConfig::Cron {
            expression: "invalid cron".to_string(),
        };

        // Should fail with invalid expression
        let result = manager.start_timer(1, config).await;
        assert!(result.is_err());

        manager.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_timer_restart() {
        let api_client = ApiClient::new("http://localhost:8080".to_string(), "token".to_string());
        let manager = TimerManager::new(api_client).await.unwrap();

        let config1 = TimerConfig::Interval {
            interval: 60,
            unit: TimeUnit::Seconds,
        };

        let config2 = TimerConfig::Interval {
            interval: 30,
            unit: TimeUnit::Seconds,
        };

        // Start first timer
        manager.start_timer(1, config1).await.unwrap();
        assert_eq!(manager.timer_count().await, 1);

        // Start second timer for same rule (should replace)
        manager.start_timer(1, config2).await.unwrap();
        assert_eq!(manager.timer_count().await, 1);

        manager.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_all_timer_types_comprehensive() {
        let api_client = ApiClient::new("http://localhost:8080".to_string(), "token".to_string());
        let manager = TimerManager::new(api_client).await.unwrap();

        // Test 1: Interval timer
        let interval_config = TimerConfig::Interval {
            interval: 5,
            unit: TimeUnit::Seconds,
        };
        manager.start_timer(100, interval_config).await.unwrap();

        // Test 2: Cron timer - every minute
        let cron_config = TimerConfig::Cron {
            expression: "0 * * * * *".to_string(),
        };
        manager.start_timer(200, cron_config).await.unwrap();

        // Test 3: DateTime timer - 2 seconds in the future
        let fire_time = Utc::now() + chrono::Duration::seconds(2);
        let datetime_config = TimerConfig::DateTime { fire_at: fire_time };
        manager.start_timer(300, datetime_config).await.unwrap();

        // Verify all three timers are active
        assert_eq!(manager.timer_count().await, 3);

        // Stop specific timers
        manager.stop_timer(100).await;
        assert_eq!(manager.timer_count().await, 2);

        manager.stop_timer(200).await;
        assert_eq!(manager.timer_count().await, 1);

        manager.stop_timer(300).await;
        assert_eq!(manager.timer_count().await, 0);

        manager.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_cron_various_expressions() {
        let api_client = ApiClient::new("http://localhost:8080".to_string(), "token".to_string());
        let manager = TimerManager::new(api_client).await.unwrap();

        // Test various valid cron expressions
        let expressions = [
            "0 0 * * * *",    // Every hour
            "0 */15 * * * *", // Every 15 minutes
            "0 0 0 * * *",    // Daily at midnight
            "0 0 9 * * 1-5",  // Weekdays at 9 AM
            "0 30 8 * * *",   // Every day at 8:30 AM
        ];

        for (i, expr) in expressions.iter().enumerate() {
            let config = TimerConfig::Cron {
                expression: expr.to_string(),
            };
            let result = manager.start_timer(i as i64 + 1, config).await;
            assert!(
                result.is_ok(),
                "Failed to create cron job with expression: {}",
                expr
            );
        }

        assert_eq!(manager.timer_count().await, expressions.len());

        manager.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_datetime_timer_future_validation() {
        let api_client = ApiClient::new("http://localhost:8080".to_string(), "token".to_string());
        let manager = TimerManager::new(api_client).await.unwrap();

        // Test various future times
        let one_second = Utc::now() + chrono::Duration::seconds(1);
        let one_minute = Utc::now() + chrono::Duration::minutes(1);
        let one_hour = Utc::now() + chrono::Duration::hours(1);

        let config1 = TimerConfig::DateTime {
            fire_at: one_second,
        };
        assert!(manager.start_timer(1, config1).await.is_ok());

        let config2 = TimerConfig::DateTime {
            fire_at: one_minute,
        };
        assert!(manager.start_timer(2, config2).await.is_ok());

        let config3 = TimerConfig::DateTime { fire_at: one_hour };
        assert!(manager.start_timer(3, config3).await.is_ok());

        assert_eq!(manager.timer_count().await, 3);

        manager.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_mixed_timer_replacement() {
        let api_client = ApiClient::new("http://localhost:8080".to_string(), "token".to_string());
        let manager = TimerManager::new(api_client).await.unwrap();

        let rule_id = 42;

        // Start with interval timer
        let interval_config = TimerConfig::Interval {
            interval: 60,
            unit: TimeUnit::Seconds,
        };
        manager.start_timer(rule_id, interval_config).await.unwrap();
        assert_eq!(manager.timer_count().await, 1);

        // Replace with cron timer
        let cron_config = TimerConfig::Cron {
            expression: "0 0 * * * *".to_string(),
        };
        manager.start_timer(rule_id, cron_config).await.unwrap();
        assert_eq!(manager.timer_count().await, 1);

        // Replace with datetime timer
        let datetime_config = TimerConfig::DateTime {
            fire_at: Utc::now() + chrono::Duration::hours(1),
        };
        manager.start_timer(rule_id, datetime_config).await.unwrap();
        assert_eq!(manager.timer_count().await, 1);

        manager.shutdown().await.unwrap();
    }
}
