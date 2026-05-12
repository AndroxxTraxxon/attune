//! Per-sensor rotating log files.
//!
//! Each sensor instance gets its own stdout and stderr log files with
//! size-based rotation. Log output is both written to files and forwarded
//! to tracing for centralized observability.
//!
//! Log file layout:
//!   {artifacts_dir}/sensors/{sensor_ref}/stdout.log   (current)
//!   {artifacts_dir}/sensors/{sensor_ref}/stdout.log.1 (previous)
//!   {artifacts_dir}/sensors/{sensor_ref}/stdout.log.2 (older)
//!   ...
//!   {artifacts_dir}/sensors/{sensor_ref}/stderr.log
//!   {artifacts_dir}/sensors/{sensor_ref}/stderr.log.1
//!   ...

use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::ChildStdout;
use tracing::{info, warn};

use attune_common::artifact_transport::ArtifactFileTransport;

/// Configuration for sensor log rotation.
#[derive(Debug, Clone)]
pub struct SensorLogConfig {
    /// Maximum bytes per log file before rotation.
    pub max_bytes: u64,
    /// Number of rotated files to keep (e.g., 5 → stdout.log.1 through .5).
    pub max_files: u32,
}

impl Default for SensorLogConfig {
    fn default() -> Self {
        Self {
            max_bytes: 10 * 1024 * 1024, // 10 MB
            max_files: 5,
        }
    }
}

/// A rotating log file writer for a single sensor stream (stdout or stderr).
///
/// Writes to a local file and rotates when the current file exceeds
/// `max_bytes`. Rotation renames `log → log.1 → log.2 → ...`, deleting
/// the oldest file when `max_files` is exceeded.
pub struct RotatingLogWriter {
    /// Relative path within artifacts_dir (e.g., `sensors/core.timer/stdout.log`).
    relative_path: String,
    /// Absolute path for direct file I/O (volume transport) or staging.
    abs_path: PathBuf,
    /// Current file handle (lazy open on first write).
    file: Option<tokio::fs::File>,
    /// Current file size in bytes.
    current_size: u64,
    /// Rotation config.
    config: SensorLogConfig,
}

impl RotatingLogWriter {
    pub fn new(
        artifacts_dir: &Path,
        sensor_ref: &str,
        stream: &str,
        config: SensorLogConfig,
    ) -> Self {
        let relative_path = format!("sensors/{}/{}.log", sensor_ref, stream);
        let abs_path = artifacts_dir.join(&relative_path);
        Self {
            relative_path,
            abs_path,
            file: None,
            current_size: 0,
            config,
        }
    }

    /// Relative path within the artifacts directory.
    pub fn relative_path(&self) -> &str {
        &self.relative_path
    }

    async fn ensure_open(&mut self) -> std::io::Result<&mut tokio::fs::File> {
        if self.file.is_none() {
            if let Some(parent) = self.abs_path.parent() {
                attune_common::utils::create_shared_dir_all(parent).await?;
            }
            // Check existing file size on re-open (sensor restart)
            match tokio::fs::metadata(&self.abs_path).await {
                Ok(meta) => {
                    self.current_size = meta.len();
                    let file = tokio::fs::OpenOptions::new()
                        .create(true)
                        .append(true)
                        .open(&self.abs_path)
                        .await?;
                    self.file = Some(file);
                }
                Err(_) => {
                    self.current_size = 0;
                    let file = tokio::fs::OpenOptions::new()
                        .create(true)
                        .write(true)
                        .truncate(true)
                        .open(&self.abs_path)
                        .await?;
                    self.file = Some(file);
                }
            }
        }
        Ok(self.file.as_mut().unwrap())
    }

    /// Write a line to the log file, rotating if needed.
    pub async fn write_line(&mut self, line: &[u8]) -> std::io::Result<()> {
        if self.current_size + line.len() as u64 > self.config.max_bytes {
            self.rotate().await?;
        }

        {
            let file = self.ensure_open().await?;
            file.write_all(line).await?;
            if !line.ends_with(b"\n") {
                file.write_all(b"\n").await?;
            }
            file.flush().await?;
        }
        self.current_size += line.len() as u64;
        if !line.ends_with(b"\n") {
            self.current_size += 1;
        }
        Ok(())
    }

    async fn rotate(&mut self) -> std::io::Result<()> {
        // Close current file
        self.file = None;
        self.current_size = 0;

        // Shift existing rotated files: .N → .N+1
        // Delete the oldest if it exceeds max_files
        for i in (1..self.config.max_files).rev() {
            let from = format!("{}.{}", self.abs_path.display(), i);
            let to = format!("{}.{}", self.abs_path.display(), i + 1);
            let _ = tokio::fs::rename(&from, &to).await;
        }

        // Delete the file that would exceed max_files
        let overflow = format!("{}.{}", self.abs_path.display(), self.config.max_files + 1);
        let _ = tokio::fs::remove_file(&overflow).await;

        // Current → .1
        let first_rotated = format!("{}.1", self.abs_path.display());
        let _ = tokio::fs::rename(&self.abs_path, &first_rotated).await;

        Ok(())
    }

    /// Flush and close the underlying file.
    pub async fn close(&mut self) {
        if let Some(mut f) = self.file.take() {
            let _ = f.flush().await;
        }
    }
}

/// Spawn a task that reads a sensor's stdout, writes to a rotating log file,
/// and also forwards each line to tracing.
pub fn spawn_stdout_log_task(
    stdout: ChildStdout,
    sensor_ref: String,
    artifacts_dir: PathBuf,
    log_config: SensorLogConfig,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut writer = RotatingLogWriter::new(&artifacts_dir, &sensor_ref, "stdout", log_config);
        let mut reader = BufReader::new(stdout).lines();

        while let Ok(Some(line)) = reader.next_line().await {
            info!("Sensor {} stdout: {}", sensor_ref, line);
            if let Err(e) = writer.write_line(line.as_bytes()).await {
                warn!("Failed to write sensor {} stdout log: {}", sensor_ref, e);
            }
        }

        writer.close().await;
        info!("Sensor {} stdout stream closed", sensor_ref);
    })
}

/// Spawn a task that reads a sensor's stderr, writes to a rotating log file,
/// and also forwards each line to tracing.
pub fn spawn_stderr_log_task(
    stderr: tokio::process::ChildStderr,
    sensor_ref: String,
    artifacts_dir: PathBuf,
    log_config: SensorLogConfig,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        let mut writer = RotatingLogWriter::new(&artifacts_dir, &sensor_ref, "stderr", log_config);
        let mut reader = BufReader::new(stderr).lines();

        while let Ok(Some(line)) = reader.next_line().await {
            warn!("Sensor {} stderr: {}", sensor_ref, line);
            if let Err(e) = writer.write_line(line.as_bytes()).await {
                warn!("Failed to write sensor {} stderr log: {}", sensor_ref, e);
            }
        }

        writer.close().await;
        info!("Sensor {} stderr stream closed", sensor_ref);
    })
}

/// Register sensor log artifacts in the database so they are discoverable
/// via the standard artifact API.
pub async fn register_sensor_log_artifacts(
    pool: &sqlx::PgPool,
    sensor_ref: &str,
    _transport: &dyn ArtifactFileTransport,
) -> anyhow::Result<()> {
    use attune_common::models::enums::{
        ArtifactType, ArtifactVisibility, OwnerType, RetentionPolicyType,
    };
    use attune_common::repositories::artifact::{ArtifactRepository, CreateArtifactInput};
    use attune_common::repositories::{Create, FindByRef};

    for stream in &["stdout", "stderr"] {
        let artifact_ref = format!("sensor.{}.{}", sensor_ref, stream);
        // Only create if it doesn't already exist
        if attune_common::repositories::artifact::ArtifactRepository::find_by_ref(
            pool,
            &artifact_ref,
        )
        .await?
        .is_some()
        {
            continue;
        }

        ArtifactRepository::create(
            pool,
            CreateArtifactInput {
                r#ref: artifact_ref,
                scope: OwnerType::Sensor,
                owner: sensor_ref.to_string(),
                r#type: ArtifactType::FileText,
                visibility: ArtifactVisibility::Private,
                retention_policy: RetentionPolicyType::Days,
                retention_limit: 90, // days
                name: Some(format!("{} sensor {} log", sensor_ref, stream)),
                description: Some(format!(
                    "Rotating {} log for sensor '{}'",
                    stream, sensor_ref
                )),
                content_type: Some("text/plain".to_string()),
                data: None,
            },
        )
        .await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_rotating_log_writer_basic_write() {
        let tmp = TempDir::new().unwrap();
        let config = SensorLogConfig {
            max_bytes: 1024,
            max_files: 3,
        };
        let mut writer = RotatingLogWriter::new(tmp.path(), "test.sensor", "stdout", config);

        writer.write_line(b"hello world").await.unwrap();
        writer.close().await;

        let content = tokio::fs::read_to_string(tmp.path().join("sensors/test.sensor/stdout.log"))
            .await
            .unwrap();
        assert!(content.contains("hello world"));
    }

    #[tokio::test]
    async fn test_rotating_log_writer_rotation() {
        let tmp = TempDir::new().unwrap();
        let config = SensorLogConfig {
            max_bytes: 50, // Very small to trigger rotation
            max_files: 3,
        };
        let mut writer = RotatingLogWriter::new(tmp.path(), "test.sensor", "stdout", config);

        // Write enough to trigger rotation
        for i in 0..10 {
            writer
                .write_line(format!("line number {}", i).as_bytes())
                .await
                .unwrap();
        }
        writer.close().await;

        // Check that rotated files exist
        let base = tmp.path().join("sensors/test.sensor/stdout.log");
        assert!(base.exists());
        let rotated_1 = PathBuf::from(format!("{}.1", base.display()));
        assert!(rotated_1.exists());
    }

    #[tokio::test]
    async fn test_rotating_log_writer_max_files_enforced() {
        let tmp = TempDir::new().unwrap();
        let config = SensorLogConfig {
            max_bytes: 30,
            max_files: 2,
        };
        let mut writer = RotatingLogWriter::new(tmp.path(), "test.sensor", "stderr", config);

        // Write enough lines to rotate multiple times
        for i in 0..20 {
            writer
                .write_line(format!("error line {}", i).as_bytes())
                .await
                .unwrap();
        }
        writer.close().await;

        let base = tmp.path().join("sensors/test.sensor/stderr.log");
        assert!(base.exists());

        // .1 and .2 should exist, .3 should not (max_files=2)
        let rotated_1 = PathBuf::from(format!("{}.1", base.display()));
        let rotated_2 = PathBuf::from(format!("{}.2", base.display()));
        let rotated_3 = PathBuf::from(format!("{}.3", base.display()));
        assert!(rotated_1.exists());
        assert!(rotated_2.exists());
        assert!(!rotated_3.exists());
    }
}
