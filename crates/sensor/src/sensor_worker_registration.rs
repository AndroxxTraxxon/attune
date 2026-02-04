//! Sensor Worker Registration Module
//!
//! Handles sensor worker registration, discovery, and status management in the database.
//! Similar to action worker registration but tailored for sensor service instances.
//!
//! Runtime detection uses the unified RuntimeDetector from common crate.

use attune_common::config::Config;
use attune_common::error::Result;
use attune_common::models::{Worker, WorkerRole, WorkerStatus, WorkerType};
use attune_common::runtime_detection::RuntimeDetector;
use chrono::Utc;
use serde_json::json;
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use tracing::{debug, info};

/// Sensor worker registration manager
pub struct SensorWorkerRegistration {
    pool: PgPool,
    worker_id: Option<i64>,
    worker_name: String,
    host: Option<String>,
    capabilities: HashMap<String, serde_json::Value>,
}

impl SensorWorkerRegistration {
    /// Create a new sensor worker registration manager
    pub fn new(pool: PgPool, config: &Config) -> Self {
        let worker_name = config
            .sensor
            .as_ref()
            .and_then(|s| s.worker_name.clone())
            .unwrap_or_else(|| {
                format!(
                    "sensor-{}",
                    hostname::get()
                        .unwrap_or_else(|_| "unknown".into())
                        .to_string_lossy()
                )
            });

        let host = config
            .sensor
            .as_ref()
            .and_then(|s| s.host.clone())
            .or_else(|| {
                hostname::get()
                    .ok()
                    .map(|h| h.to_string_lossy().to_string())
            });

        // Initial capabilities (will be populated asynchronously)
        let mut capabilities = HashMap::new();

        // Set max_concurrent_sensors from config
        let max_concurrent = config
            .sensor
            .as_ref()
            .and_then(|s| s.max_concurrent_sensors)
            .unwrap_or(10);
        capabilities.insert("max_concurrent_sensors".to_string(), json!(max_concurrent));

        // Add sensor worker version metadata
        capabilities.insert(
            "sensor_version".to_string(),
            json!(env!("CARGO_PKG_VERSION")),
        );

        // Placeholder for runtimes (will be detected asynchronously)
        capabilities.insert("runtimes".to_string(), json!(Vec::<String>::new()));

        Self {
            pool,
            worker_id: None,
            worker_name,
            host,
            capabilities,
        }
    }

    /// Register the sensor worker in the database
    pub async fn register(&mut self, config: &Config) -> Result<i64> {
        // Detect runtimes from database if not already configured
        self.detect_capabilities_async(config).await?;

        info!("Registering sensor worker: {}", self.worker_name);

        // Check if sensor worker with this name already exists
        let existing = sqlx::query_as::<_, Worker>(
            "SELECT * FROM worker WHERE name = $1 AND worker_role = 'sensor' ORDER BY created DESC LIMIT 1",
        )
        .bind(&self.worker_name)
        .fetch_optional(&self.pool)
        .await?;

        let worker_id = if let Some(existing_worker) = existing {
            info!(
                "Sensor worker '{}' already exists (ID: {}), updating status",
                self.worker_name, existing_worker.id
            );

            // Update existing sensor worker to active status with new heartbeat
            sqlx::query(
                r#"
                UPDATE worker
                SET status = $1,
                    capabilities = $2,
                    last_heartbeat = $3,
                    updated = $4,
                    host = $5
                WHERE id = $6
                "#,
            )
            .bind(WorkerStatus::Active)
            .bind(serde_json::to_value(&self.capabilities)?)
            .bind(Utc::now())
            .bind(Utc::now())
            .bind(&self.host)
            .bind(existing_worker.id)
            .execute(&self.pool)
            .await?;

            existing_worker.id
        } else {
            // Insert new sensor worker
            let row = sqlx::query(
                r#"
                INSERT INTO worker (name, worker_type, worker_role, host, status, capabilities, last_heartbeat)
                VALUES ($1, $2, $3, $4, $5, $6, $7)
                RETURNING id
                "#,
            )
            .bind(&self.worker_name)
            .bind(WorkerType::Local) // Sensor workers are always local
            .bind(WorkerRole::Sensor)
            .bind(&self.host)
            .bind(WorkerStatus::Active)
            .bind(serde_json::to_value(&self.capabilities)?)
            .bind(Utc::now())
            .fetch_one(&self.pool)
            .await?;

            let worker_id: i64 = row.get("id");
            info!("Sensor worker registered with ID: {}", worker_id);
            worker_id
        };

        self.worker_id = Some(worker_id);
        Ok(worker_id)
    }

    /// Send heartbeat to update last_heartbeat timestamp
    pub async fn heartbeat(&self) -> Result<()> {
        if let Some(worker_id) = self.worker_id {
            sqlx::query(
                r#"
                UPDATE worker
                SET last_heartbeat = $1,
                    status = $2,
                    updated = $3
                WHERE id = $4
                "#,
            )
            .bind(Utc::now())
            .bind(WorkerStatus::Active)
            .bind(Utc::now())
            .bind(worker_id)
            .execute(&self.pool)
            .await?;

            debug!("Sensor worker heartbeat sent");
        }

        Ok(())
    }

    /// Mark sensor worker as inactive
    pub async fn deregister(&self) -> Result<()> {
        if let Some(worker_id) = self.worker_id {
            info!("Deregistering sensor worker: {}", self.worker_name);

            sqlx::query(
                r#"
                UPDATE worker
                SET status = $1,
                    updated = $2
                WHERE id = $3
                "#,
            )
            .bind(WorkerStatus::Inactive)
            .bind(Utc::now())
            .bind(worker_id)
            .execute(&self.pool)
            .await?;

            info!("Sensor worker deregistered");
        }

        Ok(())
    }

    /// Get the registered sensor worker ID
    pub fn worker_id(&self) -> Option<i64> {
        self.worker_id
    }

    /// Get the sensor worker name
    pub fn worker_name(&self) -> &str {
        &self.worker_name
    }

    /// Add a capability to the sensor worker
    pub fn add_capability(&mut self, key: String, value: serde_json::Value) {
        self.capabilities.insert(key, value);
    }

    /// Update sensor worker capabilities in the database
    pub async fn update_capabilities(&self) -> Result<()> {
        if let Some(worker_id) = self.worker_id {
            sqlx::query(
                r#"
                UPDATE worker
                SET capabilities = $1,
                    updated = $2
                WHERE id = $3
                "#,
            )
            .bind(serde_json::to_value(&self.capabilities)?)
            .bind(Utc::now())
            .bind(worker_id)
            .execute(&self.pool)
            .await?;

            info!("Sensor worker capabilities updated");
        }

        Ok(())
    }

    /// Detect sensor worker capabilities based on database-driven runtime verification
    ///
    /// This is a synchronous wrapper that should be called after pool is available.
    /// The actual detection happens in `detect_capabilities_async`.
    /// Detect available runtimes using the unified runtime detector
    pub async fn detect_capabilities_async(&mut self, config: &Config) -> Result<()> {
        info!("Detecting sensor worker capabilities...");

        let detector = RuntimeDetector::new(self.pool.clone());

        // Get config capabilities if available
        let config_capabilities = config.sensor.as_ref().and_then(|s| s.capabilities.as_ref());

        // Detect capabilities with three-tier priority:
        // 1. ATTUNE_SENSOR_RUNTIMES env var
        // 2. Config file
        // 3. Database-driven detection
        let detected_capabilities = detector
            .detect_capabilities(config, "ATTUNE_SENSOR_RUNTIMES", config_capabilities)
            .await?;

        // Merge detected capabilities with existing ones
        for (key, value) in detected_capabilities {
            self.capabilities.insert(key, value);
        }

        info!(
            "Sensor worker capabilities detected: {:?}",
            self.capabilities
        );

        Ok(())
    }
}

impl Drop for SensorWorkerRegistration {
    fn drop(&mut self) {
        // Note: We can't make this async, so we just log
        // The main service should call deregister() explicitly during shutdown
        if self.worker_id.is_some() {
            info!("SensorWorkerRegistration dropped - sensor worker should be deregistered");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires database
    async fn test_database_driven_detection() {
        let config = Config::load().unwrap();
        let db = attune_common::db::Database::new(&config.database)
            .await
            .unwrap();
        let pool = db.pool().clone();
        let mut registration = SensorWorkerRegistration::new(pool, &config);

        // Detect runtimes from database
        registration
            .detect_capabilities_async(&config)
            .await
            .unwrap();

        // Should have detected some runtimes
        let runtimes = registration.capabilities.get("runtimes").unwrap();
        let runtime_array = runtimes.as_array().unwrap();
        assert!(!runtime_array.is_empty());

        println!("Detected runtimes: {:?}", runtime_array);
    }

    #[tokio::test]
    #[ignore] // Requires database
    async fn test_sensor_worker_registration() {
        let config = Config::load().unwrap();
        let db = attune_common::db::Database::new(&config.database)
            .await
            .unwrap();
        let pool = db.pool().clone();
        let mut registration = SensorWorkerRegistration::new(pool, &config);

        // Test registration
        let worker_id = registration.register(&config).await.unwrap();
        assert!(worker_id > 0);
        assert_eq!(registration.worker_id(), Some(worker_id));

        // Test heartbeat
        registration.heartbeat().await.unwrap();

        // Test deregistration
        registration.deregister().await.unwrap();
    }

    #[tokio::test]
    #[ignore] // Requires database
    async fn test_sensor_worker_capabilities() {
        let config = Config::load().unwrap();
        let db = attune_common::db::Database::new(&config.database)
            .await
            .unwrap();
        let pool = db.pool().clone();
        let mut registration = SensorWorkerRegistration::new(pool, &config);

        registration.register(&config).await.unwrap();

        // Add custom capability
        registration.add_capability("custom_feature".to_string(), json!(true));
        registration.update_capabilities().await.unwrap();

        registration.deregister().await.unwrap();
    }
}
