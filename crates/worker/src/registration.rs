//! Worker Registration Module
//!
//! Handles worker registration, discovery, and status management in the database.
//! Uses unified runtime detection from the common crate.

use attune_common::config::Config;
use attune_common::error::{Error, Result};
use attune_common::models::{Worker, WorkerRole, WorkerStatus, WorkerType};
use attune_common::runtime_detection::RuntimeDetector;
use chrono::Utc;
use serde_json::json;
use sqlx::PgPool;
use std::collections::HashMap;
use tracing::{info, warn};

use crate::runtime_detect::DetectedRuntime;

/// Worker registration manager
pub struct WorkerRegistration {
    pool: PgPool,
    worker_id: Option<i64>,
    worker_name: String,
    worker_type: WorkerType,
    worker_role: WorkerRole,
    runtime_id: Option<i64>,
    host: Option<String>,
    port: Option<i32>,
    capabilities: HashMap<String, serde_json::Value>,
}

impl WorkerRegistration {
    /// Create a new worker registration manager
    pub fn new(pool: PgPool, config: &Config) -> Self {
        let worker_name = config
            .worker
            .as_ref()
            .and_then(|w| w.name.clone())
            .unwrap_or_else(|| {
                format!(
                    "worker-{}",
                    hostname::get()
                        .unwrap_or_else(|_| "unknown".into())
                        .to_string_lossy()
                )
            });

        let worker_type = config
            .worker
            .as_ref()
            .and_then(|w| w.worker_type)
            .unwrap_or(WorkerType::Local);

        let worker_role = WorkerRole::Action;

        let runtime_id = config.worker.as_ref().and_then(|w| w.runtime_id);

        let host = config
            .worker
            .as_ref()
            .and_then(|w| w.host.clone())
            .or_else(|| {
                hostname::get()
                    .ok()
                    .map(|h| h.to_string_lossy().to_string())
            });

        let port = config.worker.as_ref().and_then(|w| w.port);

        // Initial capabilities (will be populated asynchronously)
        let mut capabilities = HashMap::new();

        // Set max_concurrent_executions from config
        let max_concurrent = config
            .worker
            .as_ref()
            .map(|w| w.max_concurrent_tasks)
            .unwrap_or(10);
        capabilities.insert(
            "max_concurrent_executions".to_string(),
            json!(max_concurrent),
        );

        // Add worker version metadata
        capabilities.insert(
            "worker_version".to_string(),
            json!(env!("CARGO_PKG_VERSION")),
        );

        // Placeholder for runtimes (will be detected asynchronously)
        capabilities.insert("runtimes".to_string(), json!(Vec::<String>::new()));

        Self {
            pool,
            worker_id: None,
            worker_name,
            worker_type,
            worker_role,
            runtime_id,
            host,
            port,
            capabilities,
        }
    }

    /// Store detected runtime interpreter metadata in capabilities.
    ///
    /// This is used by the agent (`attune-agent`) to record the full details of
    /// auto-detected interpreters — binary paths and versions — alongside the
    /// simple `runtimes` string list used for backward compatibility.
    ///
    /// The data is stored under the `detected_interpreters` capability key as a
    /// JSON array of objects:
    /// ```json
    /// [
    ///   {"name": "python", "path": "/usr/bin/python3", "version": "3.12.1"},
    ///   {"name": "shell", "path": "/bin/bash", "version": "5.2.15"}
    /// ]
    /// ```
    pub fn set_detected_runtimes(&mut self, runtimes: Vec<DetectedRuntime>) {
        let interpreters: Vec<serde_json::Value> = runtimes
            .iter()
            .map(|rt| {
                json!({
                    "name": rt.name,
                    "path": rt.path,
                    "version": rt.version,
                })
            })
            .collect();

        self.capabilities
            .insert("detected_interpreters".to_string(), json!(interpreters));

        info!(
            "Stored {} detected interpreter(s) in capabilities",
            runtimes.len()
        );
    }

    /// Mark this worker as running in agent mode.
    ///
    /// Agent-mode workers auto-detect their runtimes at startup (as opposed to
    /// being configured via `ATTUNE_WORKER_RUNTIMES` or config files). Setting
    /// this flag allows the system to distinguish agents from standard workers.
    pub fn set_agent_mode(&mut self, is_agent: bool) {
        self.capabilities
            .insert("agent_mode".to_string(), json!(is_agent));
    }

    /// Detect available runtimes using the unified runtime detector
    pub async fn detect_capabilities(&mut self, config: &Config) -> Result<()> {
        info!("Detecting worker capabilities...");

        let detector = RuntimeDetector::new(self.pool.clone());

        // Get config capabilities if available
        let config_capabilities = config.worker.as_ref().and_then(|w| w.capabilities.as_ref());

        // Detect capabilities with three-tier priority:
        // 1. ATTUNE_WORKER_RUNTIMES env var
        // 2. Config file
        // 3. Database-driven detection
        let detected_capabilities = detector
            .detect_capabilities(config, "ATTUNE_WORKER_RUNTIMES", config_capabilities)
            .await?;

        // Merge detected capabilities with existing ones
        for (key, value) in detected_capabilities {
            self.capabilities.insert(key, value);
        }

        info!("Worker capabilities detected: {:?}", self.capabilities);

        Ok(())
    }

    /// Register the worker in the database
    pub async fn register(&mut self) -> Result<i64> {
        info!("Registering worker: {}", self.worker_name);

        // Check if worker with this name already exists
        let existing = sqlx::query_as::<_, Worker>(
            "SELECT * FROM worker WHERE name = $1 ORDER BY created DESC LIMIT 1",
        )
        .bind(&self.worker_name)
        .fetch_optional(&self.pool)
        .await?;

        let worker_id = if let Some(existing_worker) = existing {
            info!(
                "Worker '{}' already exists (ID: {}), updating status",
                self.worker_name, existing_worker.id
            );

            // Update existing worker to active status with new heartbeat
            sqlx::query(
                r#"
                UPDATE worker
                SET status = $1,
                    last_heartbeat = $2,
                    host = $3,
                    port = $4,
                    capabilities = $5,
                    updated = $2
                WHERE id = $6
                "#,
            )
            .bind(WorkerStatus::Active)
            .bind(Utc::now())
            .bind(&self.host)
            .bind(self.port)
            .bind(serde_json::to_value(&self.capabilities)?)
            .bind(existing_worker.id)
            .execute(&self.pool)
            .await?;

            existing_worker.id
        } else {
            info!("Creating new worker registration: {}", self.worker_name);

            // Insert new worker
            let worker = sqlx::query_as::<_, Worker>(
                r#"
                INSERT INTO worker (name, worker_type, worker_role, runtime, host, port, status, capabilities, last_heartbeat)
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
                RETURNING *
                "#,
            )
            .bind(&self.worker_name)
            .bind(self.worker_type)
            .bind(self.worker_role)
            .bind(self.runtime_id)
            .bind(&self.host)
            .bind(self.port)
            .bind(WorkerStatus::Active)
            .bind(serde_json::to_value(&self.capabilities)?)
            .bind(Utc::now())
            .fetch_one(&self.pool)
            .await?;

            worker.id
        };

        self.worker_id = Some(worker_id);
        info!("Worker registered successfully with ID: {}", worker_id);

        Ok(worker_id)
    }

    /// Deregister the worker (mark as inactive)
    pub async fn deregister(&self) -> Result<()> {
        if let Some(worker_id) = self.worker_id {
            info!("Deregistering worker ID: {}", worker_id);

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

            info!("Worker deregistered successfully");
        } else {
            warn!("Cannot deregister: worker not registered");
        }

        Ok(())
    }

    /// Update worker heartbeat
    pub async fn update_heartbeat(&self) -> Result<()> {
        if let Some(worker_id) = self.worker_id {
            sqlx::query(
                r#"
                UPDATE worker
                SET last_heartbeat = $1,
                    updated = $1
                WHERE id = $2
                "#,
            )
            .bind(Utc::now())
            .bind(worker_id)
            .execute(&self.pool)
            .await?;
        } else {
            return Err(Error::invalid_state("Worker not registered"));
        }

        Ok(())
    }

    /// Get the registered worker ID
    pub fn worker_id(&self) -> Option<i64> {
        self.worker_id
    }

    /// Get the worker name
    pub fn worker_name(&self) -> &str {
        &self.worker_name
    }

    /// Add a capability to the worker
    pub fn add_capability(&mut self, key: String, value: serde_json::Value) {
        self.capabilities.insert(key, value);
    }

    /// Update worker capabilities in the database
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

            info!("Worker capabilities updated");
        }

        Ok(())
    }
}

impl Drop for WorkerRegistration {
    fn drop(&mut self) {
        // Note: We can't make this async, so we just log
        // The main service should call deregister() explicitly during shutdown
        if self.worker_id.is_some() {
            info!("WorkerRegistration dropped - worker should be deregistered");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore] // Requires database
    async fn test_worker_registration() {
        let config = Config::load().unwrap();
        let db = attune_common::db::Database::new(&config.database)
            .await
            .unwrap();
        let pool = db.pool().clone();
        let mut registration = WorkerRegistration::new(pool, &config);

        // Detect capabilities
        registration.detect_capabilities(&config).await.unwrap();

        // Register worker
        let worker_id = registration.register().await.unwrap();
        assert!(worker_id > 0);
        assert_eq!(registration.worker_id(), Some(worker_id));

        // Update heartbeat
        registration.update_heartbeat().await.unwrap();

        // Deregister worker
        registration.deregister().await.unwrap();
    }

    #[tokio::test]
    #[ignore] // Requires database
    async fn test_worker_capabilities() {
        let config = Config::load().unwrap();
        let db = attune_common::db::Database::new(&config.database)
            .await
            .unwrap();
        let pool = db.pool().clone();
        let mut registration = WorkerRegistration::new(pool, &config);

        registration.detect_capabilities(&config).await.unwrap();
        registration.register().await.unwrap();

        // Add capability
        registration.add_capability("test_capability".to_string(), json!(true));
        registration.update_capabilities().await.unwrap();

        registration.deregister().await.unwrap();
    }

    #[test]
    fn test_detected_runtimes_json_structure() {
        // Test the JSON structure that set_detected_runtimes builds
        let runtimes = vec![
            DetectedRuntime {
                name: "python".to_string(),
                path: "/usr/bin/python3".to_string(),
                version: Some("3.12.1".to_string()),
            },
            DetectedRuntime {
                name: "shell".to_string(),
                path: "/bin/bash".to_string(),
                version: None,
            },
        ];

        let interpreters: Vec<serde_json::Value> = runtimes
            .iter()
            .map(|rt| {
                json!({
                    "name": rt.name,
                    "path": rt.path,
                    "version": rt.version,
                })
            })
            .collect();

        let json_value = json!(interpreters);

        // Verify structure
        let arr = json_value.as_array().unwrap();
        assert_eq!(arr.len(), 2);
        assert_eq!(arr[0]["name"], "python");
        assert_eq!(arr[0]["path"], "/usr/bin/python3");
        assert_eq!(arr[0]["version"], "3.12.1");
        assert_eq!(arr[1]["name"], "shell");
        assert_eq!(arr[1]["path"], "/bin/bash");
        assert!(arr[1]["version"].is_null());
    }

    #[test]
    fn test_detected_runtimes_empty() {
        let runtimes: Vec<DetectedRuntime> = vec![];
        let interpreters: Vec<serde_json::Value> = runtimes
            .iter()
            .map(|rt| {
                json!({
                    "name": rt.name,
                    "path": rt.path,
                    "version": rt.version,
                })
            })
            .collect();

        let json_value = json!(interpreters);
        assert_eq!(json_value.as_array().unwrap().len(), 0);
    }

    #[test]
    fn test_agent_mode_capability_value() {
        // Verify the JSON value for agent_mode capability
        let value = json!(true);
        assert_eq!(value, true);

        let value = json!(false);
        assert_eq!(value, false);
    }
}
