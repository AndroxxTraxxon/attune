//! Volume-based artifact file transport.
//!
//! Reads and writes files directly on a shared filesystem. This is the
//! fast path used when the worker/sensor and API share a mounted volume.

use async_trait::async_trait;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncSeekExt;

use super::{ArtifactFileTransport, BoxAsyncReader, BoxAsyncWriter};
use crate::error::{Error, Result};

/// Direct filesystem transport backed by a shared volume directory.
#[derive(Debug, Clone)]
pub struct VolumeTransport {
    base_dir: PathBuf,
}

impl VolumeTransport {
    pub fn new(base_dir: &str) -> Self {
        Self {
            base_dir: PathBuf::from(base_dir),
        }
    }

    fn resolve(&self, file_path: &str) -> PathBuf {
        self.base_dir.join(file_path)
    }

    /// Ensure parent directories exist for the given path.
    async fn ensure_parent(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.map_err(|e| {
                Error::Io(format!(
                    "Failed to create directory {}: {e}",
                    parent.display()
                ))
            })?;
        }
        Ok(())
    }
}

#[async_trait]
impl ArtifactFileTransport for VolumeTransport {
    async fn write_file(
        &self,
        file_path: &str,
        content: &[u8],
        _content_type: Option<&str>,
    ) -> Result<()> {
        let path = self.resolve(file_path);
        self.ensure_parent(&path).await?;
        fs::write(&path, content)
            .await
            .map_err(|e| Error::Io(format!("Failed to write {}: {e}", path.display())))
    }

    async fn read_file(&self, file_path: &str) -> Result<Vec<u8>> {
        let path = self.resolve(file_path);
        fs::read(&path)
            .await
            .map_err(|e| Error::Io(format!("Failed to read {}: {e}", path.display())))
    }

    async fn append_file(&self, file_path: &str, content: &[u8]) -> Result<()> {
        let path = self.resolve(file_path);
        self.ensure_parent(&path).await?;

        use tokio::io::AsyncWriteExt;
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
            .map_err(|e| Error::Io(format!("Failed to open for append {}: {e}", path.display())))?;
        file.write_all(content)
            .await
            .map_err(|e| Error::Io(format!("Failed to append to {}: {e}", path.display())))
    }

    async fn file_exists(&self, file_path: &str) -> Result<bool> {
        let path = self.resolve(file_path);
        Ok(path.exists())
    }

    async fn file_size(&self, file_path: &str) -> Result<Option<u64>> {
        let path = self.resolve(file_path);
        match fs::metadata(&path).await {
            Ok(meta) => Ok(Some(meta.len())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(Error::Io(format!("Failed to stat {}: {e}", path.display()))),
        }
    }

    async fn delete_file(&self, file_path: &str) -> Result<()> {
        let path = self.resolve(file_path);
        match fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(Error::Io(format!(
                "Failed to delete {}: {e}",
                path.display()
            ))),
        }
    }

    async fn rename_file(&self, from: &str, to: &str) -> Result<()> {
        let src = self.resolve(from);
        let dst = self.resolve(to);
        self.ensure_parent(&dst).await?;
        fs::rename(&src, &dst).await.map_err(|e| {
            Error::Io(format!(
                "Failed to rename {} → {}: {e}",
                src.display(),
                dst.display()
            ))
        })
    }

    async fn create_writer(&self, file_path: &str) -> Result<BoxAsyncWriter> {
        let path = self.resolve(file_path);
        self.ensure_parent(&path).await?;
        let file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .await
            .map_err(|e| {
                Error::Io(format!(
                    "Failed to create writer for {}: {e}",
                    path.display()
                ))
            })?;
        Ok(Box::pin(file))
    }

    async fn open_reader(&self, file_path: &str, offset: u64) -> Result<BoxAsyncReader> {
        let path = self.resolve(file_path);
        let mut file = fs::File::open(&path)
            .await
            .map_err(|e| Error::Io(format!("Failed to open reader for {}: {e}", path.display())))?;
        if offset > 0 {
            file.seek(std::io::SeekFrom::Start(offset))
                .await
                .map_err(|e| Error::Io(format!("Failed to seek in {}: {e}", path.display())))?;
        }
        Ok(Box::pin(file))
    }

    fn transport_mode(&self) -> &'static str {
        "volume"
    }

    fn base_dir(&self) -> &str {
        self.base_dir.to_str().unwrap_or("/opt/attune/artifacts")
    }

    async fn ensure_parent_dirs(&self, file_path: &str) -> Result<()> {
        let path = self.resolve(file_path);
        self.ensure_parent(&path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_write_read_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let transport = VolumeTransport::new(tmp.path().to_str().unwrap());

        transport
            .write_file("test/hello.txt", b"Hello, world!", None)
            .await
            .unwrap();
        let content = transport.read_file("test/hello.txt").await.unwrap();
        assert_eq!(content, b"Hello, world!");
    }

    #[tokio::test]
    async fn test_file_exists() {
        let tmp = TempDir::new().unwrap();
        let transport = VolumeTransport::new(tmp.path().to_str().unwrap());

        assert!(!transport.file_exists("nope.txt").await.unwrap());
        transport.write_file("yes.txt", b"ok", None).await.unwrap();
        assert!(transport.file_exists("yes.txt").await.unwrap());
    }

    #[tokio::test]
    async fn test_append_file() {
        let tmp = TempDir::new().unwrap();
        let transport = VolumeTransport::new(tmp.path().to_str().unwrap());

        transport.append_file("log.txt", b"line1\n").await.unwrap();
        transport.append_file("log.txt", b"line2\n").await.unwrap();
        let content = transport.read_file("log.txt").await.unwrap();
        assert_eq!(content, b"line1\nline2\n");
    }

    #[tokio::test]
    async fn test_file_size() {
        let tmp = TempDir::new().unwrap();
        let transport = VolumeTransport::new(tmp.path().to_str().unwrap());

        assert_eq!(transport.file_size("nope").await.unwrap(), None);
        transport
            .write_file("f.bin", &[0u8; 42], None)
            .await
            .unwrap();
        assert_eq!(transport.file_size("f.bin").await.unwrap(), Some(42));
    }

    #[tokio::test]
    async fn test_delete_file() {
        let tmp = TempDir::new().unwrap();
        let transport = VolumeTransport::new(tmp.path().to_str().unwrap());

        transport.write_file("rm.txt", b"bye", None).await.unwrap();
        transport.delete_file("rm.txt").await.unwrap();
        assert!(!transport.file_exists("rm.txt").await.unwrap());
        // Deleting again is OK
        transport.delete_file("rm.txt").await.unwrap();
    }

    #[tokio::test]
    async fn test_rename_file() {
        let tmp = TempDir::new().unwrap();
        let transport = VolumeTransport::new(tmp.path().to_str().unwrap());

        transport.write_file("a.txt", b"data", None).await.unwrap();
        transport.rename_file("a.txt", "sub/b.txt").await.unwrap();
        assert!(!transport.file_exists("a.txt").await.unwrap());
        let content = transport.read_file("sub/b.txt").await.unwrap();
        assert_eq!(content, b"data");
    }
}
