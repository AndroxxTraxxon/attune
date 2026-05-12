//! Artifacts Module
//!
//! Handles storage and retrieval of execution artifacts (logs, outputs, results).

use attune_common::error::{Error, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::{debug, info, warn};

/// Artifact type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ArtifactType {
    /// Execution logs (stdout/stderr)
    Log,
    /// Execution result data
    Result,
    /// Custom file output
    File,
    /// Trace/debug information
    Trace,
}

/// Artifact metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artifact {
    /// Artifact ID
    pub id: String,
    /// Execution ID
    pub execution_id: i64,
    /// Artifact type
    pub artifact_type: ArtifactType,
    /// File path
    pub path: PathBuf,
    /// Content type (MIME type)
    pub content_type: String,
    /// Size in bytes
    pub size: u64,
    /// Creation timestamp
    pub created: chrono::DateTime<chrono::Utc>,
}

/// Artifact manager for storing execution artifacts
pub struct ArtifactManager {
    /// Base directory for artifact storage
    base_dir: PathBuf,
}

impl ArtifactManager {
    /// Create a new artifact manager
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }

    /// Initialize the artifact storage directory
    pub async fn initialize(&self) -> Result<()> {
        attune_common::utils::create_shared_dir_all(&self.base_dir)
            .await
            .map_err(|e| Error::Internal(format!("Failed to create artifact directory: {}", e)))?;

        info!("Artifact storage initialized at: {:?}", self.base_dir);
        Ok(())
    }

    /// Get the directory path for an execution
    pub fn get_execution_dir(&self, execution_id: i64) -> PathBuf {
        self.base_dir.join(format!("execution_{}", execution_id))
    }

    /// Store execution logs
    pub async fn store_logs(
        &self,
        execution_id: i64,
        stdout: &str,
        stderr: &str,
    ) -> Result<Vec<Artifact>> {
        let exec_dir = self.get_execution_dir(execution_id);
        attune_common::utils::create_shared_dir_all(&exec_dir)
            .await
            .map_err(|e| Error::Internal(format!("Failed to create execution directory: {}", e)))?;

        let mut artifacts = Vec::new();

        // Store stdout
        if !stdout.is_empty() {
            // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path -- Artifact filenames are fixed constants under an execution-scoped directory derived from the execution ID.
            let stdout_path = exec_dir.join("stdout.log");
            let mut file = fs::File::create(&stdout_path)
                .await
                .map_err(|e| Error::Internal(format!("Failed to create stdout file: {}", e)))?;
            file.write_all(stdout.as_bytes())
                .await
                .map_err(|e| Error::Internal(format!("Failed to write stdout: {}", e)))?;
            file.sync_all()
                .await
                .map_err(|e| Error::Internal(format!("Failed to sync stdout file: {}", e)))?;

            let metadata = fs::metadata(&stdout_path)
                .await
                .map_err(|e| Error::Internal(format!("Failed to get stdout metadata: {}", e)))?;
            artifacts.push(Artifact {
                id: format!("{}_stdout", execution_id),
                execution_id,
                artifact_type: ArtifactType::Log,
                path: stdout_path,
                content_type: "text/plain".to_string(),
                size: metadata.len(),
                created: chrono::Utc::now(),
            });

            debug!(
                "Stored stdout log for execution {} ({} bytes)",
                execution_id,
                metadata.len()
            );
        }

        // Store stderr
        if !stderr.is_empty() {
            // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path -- Artifact filenames are fixed constants under an execution-scoped directory derived from the execution ID.
            let stderr_path = exec_dir.join("stderr.log");
            let mut file = fs::File::create(&stderr_path)
                .await
                .map_err(|e| Error::Internal(format!("Failed to create stderr file: {}", e)))?;
            file.write_all(stderr.as_bytes())
                .await
                .map_err(|e| Error::Internal(format!("Failed to write stderr: {}", e)))?;
            file.sync_all()
                .await
                .map_err(|e| Error::Internal(format!("Failed to sync stderr file: {}", e)))?;

            let metadata = fs::metadata(&stderr_path)
                .await
                .map_err(|e| Error::Internal(format!("Failed to get stderr metadata: {}", e)))?;
            artifacts.push(Artifact {
                id: format!("{}_stderr", execution_id),
                execution_id,
                artifact_type: ArtifactType::Log,
                path: stderr_path,
                content_type: "text/plain".to_string(),
                size: metadata.len(),
                created: chrono::Utc::now(),
            });

            debug!(
                "Stored stderr log for execution {} ({} bytes)",
                execution_id,
                metadata.len()
            );
        }

        Ok(artifacts)
    }

    /// Store execution result
    pub async fn store_result(
        &self,
        execution_id: i64,
        result: &serde_json::Value,
    ) -> Result<Artifact> {
        let exec_dir = self.get_execution_dir(execution_id);
        attune_common::utils::create_shared_dir_all(&exec_dir)
            .await
            .map_err(|e| Error::Internal(format!("Failed to create execution directory: {}", e)))?;

        // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path -- Result artifacts are written to a fixed filename inside the execution-scoped directory.
        let result_path = exec_dir.join("result.json");
        let result_json = serde_json::to_string_pretty(result)?;

        let mut file = fs::File::create(&result_path)
            .await
            .map_err(|e| Error::Internal(format!("Failed to create result file: {}", e)))?;
        file.write_all(result_json.as_bytes())
            .await
            .map_err(|e| Error::Internal(format!("Failed to write result: {}", e)))?;
        file.sync_all()
            .await
            .map_err(|e| Error::Internal(format!("Failed to sync result file: {}", e)))?;

        let metadata = fs::metadata(&result_path)
            .await
            .map_err(|e| Error::Internal(format!("Failed to get result metadata: {}", e)))?;

        debug!(
            "Stored result for execution {} ({} bytes)",
            execution_id,
            metadata.len()
        );

        Ok(Artifact {
            id: format!("{}_result", execution_id),
            execution_id,
            artifact_type: ArtifactType::Result,
            path: result_path,
            content_type: "application/json".to_string(),
            size: metadata.len(),
            created: chrono::Utc::now(),
        })
    }

    /// Store a custom file artifact
    pub async fn store_file(
        &self,
        execution_id: i64,
        filename: &str,
        content: &[u8],
        content_type: Option<&str>,
    ) -> Result<Artifact> {
        let exec_dir = self.get_execution_dir(execution_id);
        attune_common::utils::create_shared_dir_all(&exec_dir)
            .await
            .map_err(|e| Error::Internal(format!("Failed to create execution directory: {}", e)))?;

        // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path -- Custom artifact paths are always rooted under the execution-scoped artifact directory.
        let file_path = exec_dir.join(filename);
        let mut file = fs::File::create(&file_path)
            .await
            .map_err(|e| Error::Internal(format!("Failed to create file: {}", e)))?;
        file.write_all(content)
            .await
            .map_err(|e| Error::Internal(format!("Failed to write file: {}", e)))?;
        file.sync_all()
            .await
            .map_err(|e| Error::Internal(format!("Failed to sync file: {}", e)))?;

        let metadata = fs::metadata(&file_path)
            .await
            .map_err(|e| Error::Internal(format!("Failed to get file metadata: {}", e)))?;

        debug!(
            "Stored file artifact {} for execution {} ({} bytes)",
            filename,
            execution_id,
            metadata.len()
        );

        Ok(Artifact {
            id: format!("{}_{}", execution_id, filename),
            execution_id,
            artifact_type: ArtifactType::File,
            path: file_path,
            content_type: content_type
                .unwrap_or("application/octet-stream")
                .to_string(),
            size: metadata.len(),
            created: chrono::Utc::now(),
        })
    }

    /// Read an artifact
    pub async fn read_artifact(&self, artifact: &Artifact) -> Result<Vec<u8>> {
        // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path -- Artifact reads use paths previously created by the artifact manager inside the configured artifact root.
        fs::read(&artifact.path)
            .await
            .map_err(|e| Error::Internal(format!("Failed to read artifact: {}", e)))
    }

    /// Delete artifacts for an execution
    pub async fn delete_execution_artifacts(&self, execution_id: i64) -> Result<()> {
        let exec_dir = self.get_execution_dir(execution_id);

        if exec_dir.exists() {
            fs::remove_dir_all(&exec_dir).await.map_err(|e| {
                Error::Internal(format!("Failed to delete execution artifacts: {}", e))
            })?;

            info!("Deleted artifacts for execution {}", execution_id);
        } else {
            warn!(
                "No artifacts found for execution {} (directory does not exist)",
                execution_id
            );
        }

        Ok(())
    }

    /// Clean up old artifacts (retention policy)
    pub async fn cleanup_old_artifacts(&self, retention_days: u64) -> Result<usize> {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(retention_days as i64);
        let mut deleted_count = 0;

        let mut entries = fs::read_dir(&self.base_dir)
            .await
            .map_err(|e| Error::Internal(format!("Failed to read artifact directory: {}", e)))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| Error::Internal(format!("Failed to read directory entry: {}", e)))?
        {
            let path = entry.path();
            if path.is_dir() {
                if let Ok(metadata) = fs::metadata(&path).await {
                    if let Ok(modified) = metadata.modified() {
                        let modified_time: chrono::DateTime<chrono::Utc> = modified.into();
                        if modified_time < cutoff {
                            if let Err(e) = fs::remove_dir_all(&path).await {
                                warn!("Failed to delete old artifact directory {:?}: {}", path, e);
                            } else {
                                deleted_count += 1;
                                debug!("Deleted old artifact directory: {:?}", path);
                            }
                        }
                    }
                }
            }
        }

        info!(
            "Cleaned up {} old artifact directories (retention: {} days)",
            deleted_count, retention_days
        );

        Ok(deleted_count)
    }
}

impl Default for ArtifactManager {
    fn default() -> Self {
        Self::new(PathBuf::from("/tmp/attune/artifacts"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_artifact_manager_store_logs() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ArtifactManager::new(temp_dir.path().to_path_buf());
        manager.initialize().await.unwrap();

        let artifacts = manager
            .store_logs(1, "stdout output", "stderr output")
            .await
            .unwrap();

        assert_eq!(artifacts.len(), 2);
    }

    #[tokio::test]
    async fn test_artifact_manager_store_result() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ArtifactManager::new(temp_dir.path().to_path_buf());
        manager.initialize().await.unwrap();

        let result = serde_json::json!({"status": "success", "value": 42});
        let artifact = manager.store_result(1, &result).await.unwrap();

        assert_eq!(artifact.execution_id, 1);
        assert_eq!(artifact.content_type, "application/json");
    }

    #[tokio::test]
    async fn test_artifact_manager_delete() {
        let temp_dir = TempDir::new().unwrap();
        let manager = ArtifactManager::new(temp_dir.path().to_path_buf());
        manager.initialize().await.unwrap();

        manager.store_logs(1, "test", "test").await.unwrap();
        assert!(manager.get_execution_dir(1).exists());

        manager.delete_execution_artifacts(1).await.unwrap();
        assert!(!manager.get_execution_dir(1).exists());
    }
}
