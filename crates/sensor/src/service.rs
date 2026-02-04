//! Sensor Service
//!
//! Main service orchestrator that coordinates sensor management
//! and rule lifecycle listening.

use crate::rule_lifecycle_listener::RuleLifecycleListener;
use crate::sensor_manager::SensorManager;
use crate::sensor_worker_registration::SensorWorkerRegistration;
use anyhow::Result;
use attune_common::config::Config;
use attune_common::db::Database;
use attune_common::mq::MessageQueue;
use sqlx::PgPool;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// Sensor Service state
#[derive(Clone)]
pub struct SensorService {
    inner: Arc<SensorServiceInner>,
}

struct SensorServiceInner {
    config: Config,
    db: PgPool,
    mq: MessageQueue,
    sensor_manager: Arc<SensorManager>,
    rule_lifecycle_listener: Arc<RuleLifecycleListener>,
    sensor_worker_registration: Arc<RwLock<SensorWorkerRegistration>>,
    heartbeat_interval: u64,
    running: Arc<RwLock<bool>>,
}

impl SensorService {
    /// Create a new sensor service
    pub async fn new(config: Config) -> Result<Self> {
        info!("Initializing Sensor Service");

        // Connect to database
        info!("Connecting to database...");
        let database = Database::new(&config.database).await?;
        let db = database.pool().clone();
        info!("Database connection established");

        // Connect to message queue
        info!("Connecting to message queue...");
        let mq_config = config
            .message_queue
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Message queue configuration is required"))?;
        let mq = MessageQueue::connect(&mq_config.url).await?;
        info!("Message queue connection established");

        // Create service components
        info!("Creating service components...");

        let sensor_manager = Arc::new(SensorManager::new(db.clone()));

        // Create rule lifecycle listener
        let rule_lifecycle_listener = Arc::new(RuleLifecycleListener::new(
            db.clone(),
            mq.get_connection().clone(),
            sensor_manager.clone(),
        ));

        // Create sensor worker registration
        let sensor_worker_registration = SensorWorkerRegistration::new(db.clone(), &config);
        let heartbeat_interval = config
            .sensor
            .as_ref()
            .map(|s| s.heartbeat_interval)
            .unwrap_or(30);

        Ok(Self {
            inner: Arc::new(SensorServiceInner {
                config,
                db,
                mq,
                sensor_manager,
                rule_lifecycle_listener,
                sensor_worker_registration: Arc::new(RwLock::new(sensor_worker_registration)),
                heartbeat_interval,
                running: Arc::new(RwLock::new(false)),
            }),
        })
    }

    /// Start the sensor service
    pub async fn start(&self) -> Result<()> {
        info!("Starting Sensor Service");

        // Mark as running
        *self.inner.running.write().await = true;

        // Register sensor worker
        info!("Registering sensor worker...");
        let worker_id = self
            .inner
            .sensor_worker_registration
            .write()
            .await
            .register(&self.inner.config)
            .await?;
        info!("Sensor worker registered with ID: {}", worker_id);

        // Start rule lifecycle listener
        info!("Starting rule lifecycle listener...");
        if let Err(e) = self.inner.rule_lifecycle_listener.start().await {
            error!("Failed to start rule lifecycle listener: {}", e);
            return Err(e);
        }
        info!("Rule lifecycle listener started");

        // Start sensor manager
        info!("Starting sensor manager...");
        if let Err(e) = self.inner.sensor_manager.start().await {
            error!("Failed to start sensor manager: {}", e);
            return Err(e);
        }
        info!("Sensor manager started");

        // Start heartbeat loop
        let registration = self.inner.sensor_worker_registration.clone();
        let heartbeat_interval = self.inner.heartbeat_interval;
        let running = self.inner.running.clone();
        tokio::spawn(async move {
            while *running.read().await {
                tokio::time::sleep(tokio::time::Duration::from_secs(heartbeat_interval)).await;
                if let Err(e) = registration.read().await.heartbeat().await {
                    error!("Failed to send sensor worker heartbeat: {}", e);
                }
            }
        });

        // Wait until stopped
        while *self.inner.running.read().await {
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }

        info!("Sensor Service stopped");

        Ok(())
    }

    /// Stop the sensor service
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping Sensor Service");

        // Mark as not running
        *self.inner.running.write().await = false;

        // Deregister sensor worker
        info!("Deregistering sensor worker...");
        if let Err(e) = self
            .inner
            .sensor_worker_registration
            .read()
            .await
            .deregister()
            .await
        {
            error!("Failed to deregister sensor worker: {}", e);
        }

        // Stop rule lifecycle listener
        info!("Stopping rule lifecycle listener...");
        if let Err(e) = self.inner.rule_lifecycle_listener.stop().await {
            error!("Failed to stop rule lifecycle listener: {}", e);
        }

        // Stop sensor manager
        info!("Stopping sensor manager...");
        if let Err(e) = self.inner.sensor_manager.stop().await {
            error!("Failed to stop sensor manager: {}", e);
        }

        // Close message queue connection
        info!("Closing message queue connection...");
        if let Err(e) = self.inner.mq.close().await {
            warn!("Error closing message queue: {}", e);
        }

        // Close database connection
        info!("Closing database connection...");
        self.inner.db.close().await;

        info!("Sensor Service stopped successfully");

        Ok(())
    }

    /// Check if service is running
    pub async fn is_running(&self) -> bool {
        *self.inner.running.read().await
    }

    /// Get database pool
    pub fn db(&self) -> &PgPool {
        &self.inner.db
    }

    /// Get message queue
    pub fn mq(&self) -> &MessageQueue {
        &self.inner.mq
    }

    /// Get sensor manager
    pub fn sensor_manager(&self) -> Arc<SensorManager> {
        self.inner.sensor_manager.clone()
    }

    /// Get health status
    pub async fn health_check(&self) -> HealthStatus {
        // Check if service is running
        if !*self.inner.running.read().await {
            return HealthStatus::Unhealthy("Service not running".to_string());
        }

        // Check database connection
        if let Err(e) = sqlx::query("SELECT 1").execute(&self.inner.db).await {
            return HealthStatus::Unhealthy(format!("Database connection failed: {}", e));
        }

        // Check sensor manager health
        let active_sensors = self.inner.sensor_manager.active_count().await;
        let failed_sensors = self.inner.sensor_manager.failed_count().await;

        if active_sensors == 0 {
            return HealthStatus::Degraded("No active sensors".to_string());
        }

        if failed_sensors > 10 {
            return HealthStatus::Degraded(format!("{} sensors have failed", failed_sensors));
        }

        HealthStatus::Healthy
    }
}

/// Health status enumeration
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    /// Service is healthy
    Healthy,
    /// Service is degraded but operational
    Degraded(String),
    /// Service is unhealthy
    Unhealthy(String),
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Healthy => write!(f, "healthy"),
            HealthStatus::Degraded(msg) => write!(f, "degraded: {}", msg),
            HealthStatus::Unhealthy(msg) => write!(f, "unhealthy: {}", msg),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_health_status_display() {
        assert_eq!(HealthStatus::Healthy.to_string(), "healthy");
        assert_eq!(
            HealthStatus::Degraded("test".to_string()).to_string(),
            "degraded: test"
        );
        assert_eq!(
            HealthStatus::Unhealthy("error".to_string()).to_string(),
            "unhealthy: error"
        );
    }
}
