//! Pack installer module for downloading and extracting packs from various sources
//!
//! This module provides functionality for:
//! - Cloning git repositories
//! - Downloading and extracting archives (zip, tar.gz)
//! - Copying local directories
//! - Verifying checksums
//! - Resolving registry references to install sources
//! - Progress reporting during installation

use super::{Checksum, InstallSource, PackIndexEntry, RegistryClient};
use crate::config::PackRegistryConfig;
use crate::error::{Error, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs;
use tokio::process::Command;

/// Progress callback type
pub type ProgressCallback = Arc<dyn Fn(ProgressEvent) + Send + Sync>;

/// Progress event during pack installation
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    /// Started a new step
    StepStarted {
        step: String,
        message: String,
    },
    /// Step completed
    StepCompleted {
        step: String,
        message: String,
    },
    /// Download progress
    Downloading {
        url: String,
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
    },
    /// Extraction progress
    Extracting {
        file: String,
    },
    /// Verification progress
    Verifying {
        message: String,
    },
    /// Warning message
    Warning {
        message: String,
    },
    /// Info message
    Info {
        message: String,
    },
}

/// Pack installer for handling various installation sources
pub struct PackInstaller {
    /// Temporary directory for downloads
    temp_dir: PathBuf,

    /// Registry client for resolving pack references
    registry_client: Option<RegistryClient>,

    /// Whether to verify checksums
    verify_checksums: bool,

    /// Progress callback (optional)
    progress_callback: Option<ProgressCallback>,
}

/// Information about an installed pack
#[derive(Debug, Clone)]
pub struct InstalledPack {
    /// Path to the pack directory
    pub path: PathBuf,

    /// Installation source
    pub source: PackSource,

    /// Checksum (if available and verified)
    pub checksum: Option<String>,
}

/// Pack installation source type
#[derive(Debug, Clone)]
pub enum PackSource {
    /// Git repository
    Git {
        url: String,
        git_ref: Option<String>,
    },

    /// Archive URL (zip, tar.gz, tgz)
    Archive { url: String },

    /// Local directory
    LocalDirectory { path: PathBuf },

    /// Local archive file
    LocalArchive { path: PathBuf },

    /// Registry reference
    Registry {
        pack_ref: String,
        version: Option<String>,
    },
}

impl PackInstaller {
    /// Create a new pack installer
    pub async fn new(
        temp_base_dir: impl AsRef<Path>,
        registry_config: Option<PackRegistryConfig>,
    ) -> Result<Self> {
        let temp_dir = temp_base_dir.as_ref().join("pack-installs");
        fs::create_dir_all(&temp_dir)
            .await
            .map_err(|e| Error::internal(format!("Failed to create temp directory: {}", e)))?;

        let (registry_client, verify_checksums) = if let Some(config) = registry_config {
            let verify_checksums = config.verify_checksums;
            (Some(RegistryClient::new(config)?), verify_checksums)
        } else {
            (None, false)
        };

        Ok(Self {
            temp_dir,
            registry_client,
            verify_checksums,
            progress_callback: None,
        })
    }

    /// Set progress callback
    pub fn with_progress_callback(mut self, callback: ProgressCallback) -> Self {
        self.progress_callback = Some(callback);
        self
    }

    /// Report progress event
    fn report_progress(&self, event: ProgressEvent) {
        if let Some(ref callback) = self.progress_callback {
            callback(event);
        }
    }

    /// Install a pack from the given source
    pub async fn install(&self, source: PackSource) -> Result<InstalledPack> {
        match source {
            PackSource::Git { url, git_ref } => self.install_from_git(&url, git_ref.as_deref()).await,
            PackSource::Archive { url } => self.install_from_archive_url(&url, None).await,
            PackSource::LocalDirectory { path } => self.install_from_local_directory(&path).await,
            PackSource::LocalArchive { path } => self.install_from_local_archive(&path).await,
            PackSource::Registry { pack_ref, version } => {
                self.install_from_registry(&pack_ref, version.as_deref()).await
            }
        }
    }

    /// Install from git repository
    async fn install_from_git(&self, url: &str, git_ref: Option<&str>) -> Result<InstalledPack> {
        tracing::info!("Installing pack from git: {} (ref: {:?})", url, git_ref);

        self.report_progress(ProgressEvent::StepStarted {
            step: "clone".to_string(),
            message: format!("Cloning git repository: {}", url),
        });

        // Create unique temp directory for this installation
        let install_dir = self.create_temp_dir().await?;

        // Clone the repository
        let mut clone_cmd = Command::new("git");
        clone_cmd.arg("clone");

        // Add depth=1 for faster cloning if no specific ref
        if git_ref.is_none() {
            clone_cmd.arg("--depth").arg("1");
        }

        clone_cmd.arg(&url).arg(&install_dir);

        let output = clone_cmd
            .output()
            .await
            .map_err(|e| Error::internal(format!("Failed to execute git clone: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::internal(format!("Git clone failed: {}", stderr)));
        }

        // Checkout specific ref if provided
        if let Some(ref_spec) = git_ref {
            let checkout_output = Command::new("git")
                .arg("-C")
                .arg(&install_dir)
                .arg("checkout")
                .arg(ref_spec)
                .output()
                .await
                .map_err(|e| Error::internal(format!("Failed to execute git checkout: {}", e)))?;

            if !checkout_output.status.success() {
                let stderr = String::from_utf8_lossy(&checkout_output.stderr);
                return Err(Error::internal(format!("Git checkout failed: {}", stderr)));
            }
        }

        // Find pack.yaml (could be at root or in pack/ subdirectory)
        let pack_dir = self.find_pack_directory(&install_dir).await?;

        Ok(InstalledPack {
            path: pack_dir,
            source: PackSource::Git {
                url: url.to_string(),
                git_ref: git_ref.map(String::from),
            },
            checksum: None,
        })
    }

    /// Install from archive URL
    async fn install_from_archive_url(
        &self,
        url: &str,
        expected_checksum: Option<&str>,
    ) -> Result<InstalledPack> {
        tracing::info!("Installing pack from archive: {}", url);

        // Download the archive
        let archive_path = self.download_archive(url).await?;

        // Verify checksum if provided
        if let Some(checksum_str) = expected_checksum {
            if self.verify_checksums {
                self.verify_archive_checksum(&archive_path, checksum_str)
                    .await?;
            }
        }

        // Extract the archive
        let extract_dir = self.extract_archive(&archive_path).await?;

        // Find pack.yaml
        let pack_dir = self.find_pack_directory(&extract_dir).await?;

        // Clean up archive file
        let _ = fs::remove_file(&archive_path).await;

        Ok(InstalledPack {
            path: pack_dir,
            source: PackSource::Archive {
                url: url.to_string(),
            },
            checksum: expected_checksum.map(String::from),
        })
    }

    /// Install from local directory
    async fn install_from_local_directory(&self, source_path: &Path) -> Result<InstalledPack> {
        tracing::info!("Installing pack from local directory: {:?}", source_path);

        // Verify source exists and is a directory
        if !source_path.exists() {
            return Err(Error::not_found("directory", "path", source_path.display().to_string()));
        }

        if !source_path.is_dir() {
            return Err(Error::validation(format!(
                "Path is not a directory: {}",
                source_path.display()
            )));
        }

        // Create temp directory
        let install_dir = self.create_temp_dir().await?;

        // Copy directory contents
        self.copy_directory(source_path, &install_dir).await?;

        // Find pack.yaml
        let pack_dir = self.find_pack_directory(&install_dir).await?;

        Ok(InstalledPack {
            path: pack_dir,
            source: PackSource::LocalDirectory {
                path: source_path.to_path_buf(),
            },
            checksum: None,
        })
    }

    /// Install from local archive file
    async fn install_from_local_archive(&self, archive_path: &Path) -> Result<InstalledPack> {
        tracing::info!("Installing pack from local archive: {:?}", archive_path);

        // Verify file exists
        if !archive_path.exists() {
            return Err(Error::not_found("file", "path", archive_path.display().to_string()));
        }

        if !archive_path.is_file() {
            return Err(Error::validation(format!(
                "Path is not a file: {}",
                archive_path.display()
            )));
        }

        // Extract the archive
        let extract_dir = self.extract_archive(archive_path).await?;

        // Find pack.yaml
        let pack_dir = self.find_pack_directory(&extract_dir).await?;

        Ok(InstalledPack {
            path: pack_dir,
            source: PackSource::LocalArchive {
                path: archive_path.to_path_buf(),
            },
            checksum: None,
        })
    }

    /// Install from registry reference
    async fn install_from_registry(
        &self,
        pack_ref: &str,
        version: Option<&str>,
    ) -> Result<InstalledPack> {
        tracing::info!(
            "Installing pack from registry: {} (version: {:?})",
            pack_ref,
            version
        );

        let registry_client = self
            .registry_client
            .as_ref()
            .ok_or_else(|| Error::configuration("Registry client not configured"))?;

        // Search for the pack
        let (pack_entry, _registry_url) = registry_client
            .search_pack(pack_ref)
            .await?
            .ok_or_else(|| Error::not_found("pack", "ref", pack_ref))?;

        // Validate version if specified
        if let Some(requested_version) = version {
            if requested_version != "latest" && pack_entry.version != requested_version {
                return Err(Error::validation(format!(
                    "Pack {} version {} not found (available: {})",
                    pack_ref, requested_version, pack_entry.version
                )));
            }
        }

        // Get the preferred install source (try git first, then archive)
        let install_source = self.select_install_source(&pack_entry)?;

        // Install from the selected source
        match install_source {
            InstallSource::Git {
                url,
                git_ref,
                checksum,
            } => {
                let mut installed = self
                    .install_from_git(&url, git_ref.as_deref())
                    .await?;
                installed.checksum = Some(checksum);
                Ok(installed)
            }
            InstallSource::Archive { url, checksum } => {
                self.install_from_archive_url(&url, Some(&checksum)).await
            }
        }
    }

    /// Select the best install source from a pack entry
    fn select_install_source(&self, pack_entry: &PackIndexEntry) -> Result<InstallSource> {
        if pack_entry.install_sources.is_empty() {
            return Err(Error::validation(format!(
                "Pack {} has no install sources",
                pack_entry.pack_ref
            )));
        }

        // Prefer git sources for development
        for source in &pack_entry.install_sources {
            if matches!(source, InstallSource::Git { .. }) {
                return Ok(source.clone());
            }
        }

        // Fall back to first archive source
        for source in &pack_entry.install_sources {
            if matches!(source, InstallSource::Archive { .. }) {
                return Ok(source.clone());
            }
        }

        // Return first source if no preference matched
        Ok(pack_entry.install_sources[0].clone())
    }

    /// Download an archive from a URL
    async fn download_archive(&self, url: &str) -> Result<PathBuf> {
        let client = reqwest::Client::new();

        let response = client
            .get(url)
            .send()
            .await
            .map_err(|e| Error::internal(format!("Failed to download archive: {}", e)))?;

        if !response.status().is_success() {
            return Err(Error::internal(format!(
                "Failed to download archive: HTTP {}",
                response.status()
            )));
        }

        // Determine filename from URL
        let filename = url
            .split('/')
            .last()
            .unwrap_or("archive.zip")
            .to_string();

        let archive_path = self.temp_dir.join(&filename);

        // Download to file
        let bytes = response
            .bytes()
            .await
            .map_err(|e| Error::internal(format!("Failed to read archive bytes: {}", e)))?;

        fs::write(&archive_path, &bytes)
            .await
            .map_err(|e| Error::internal(format!("Failed to write archive: {}", e)))?;

        Ok(archive_path)
    }

    /// Extract an archive (zip or tar.gz)
    async fn extract_archive(&self, archive_path: &Path) -> Result<PathBuf> {
        let extract_dir = self.create_temp_dir().await?;

        let extension = archive_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        match extension {
            "zip" => self.extract_zip(archive_path, &extract_dir).await?,
            "gz" | "tgz" => self.extract_tar_gz(archive_path, &extract_dir).await?,
            _ => {
                return Err(Error::validation(format!(
                    "Unsupported archive format: {}",
                    extension
                )));
            }
        }

        Ok(extract_dir)
    }

    /// Extract a zip archive
    async fn extract_zip(&self, archive_path: &Path, extract_dir: &Path) -> Result<()> {
        let output = Command::new("unzip")
            .arg("-q") // Quiet
            .arg(archive_path)
            .arg("-d")
            .arg(extract_dir)
            .output()
            .await
            .map_err(|e| Error::internal(format!("Failed to execute unzip: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::internal(format!("Failed to extract zip: {}", stderr)));
        }

        Ok(())
    }

    /// Extract a tar.gz archive
    async fn extract_tar_gz(&self, archive_path: &Path, extract_dir: &Path) -> Result<()> {
        let output = Command::new("tar")
            .arg("xzf")
            .arg(archive_path)
            .arg("-C")
            .arg(extract_dir)
            .output()
            .await
            .map_err(|e| Error::internal(format!("Failed to execute tar: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::internal(format!("Failed to extract tar.gz: {}", stderr)));
        }

        Ok(())
    }

    /// Verify archive checksum
    async fn verify_archive_checksum(
        &self,
        archive_path: &Path,
        checksum_str: &str,
    ) -> Result<()> {
        let checksum = Checksum::parse(checksum_str)
            .map_err(|e| Error::validation(format!("Invalid checksum: {}", e)))?;

        let computed = self.compute_checksum(archive_path, &checksum.algorithm).await?;

        if computed != checksum.hash {
            return Err(Error::validation(format!(
                "Checksum mismatch: expected {}, got {}",
                checksum.hash, computed
            )));
        }

        tracing::info!("Checksum verified: {}", checksum_str);
        Ok(())
    }

    /// Compute checksum of a file
    async fn compute_checksum(&self, path: &Path, algorithm: &str) -> Result<String> {
        let command = match algorithm {
            "sha256" => "sha256sum",
            "sha512" => "sha512sum",
            "sha1" => "sha1sum",
            "md5" => "md5sum",
            _ => {
                return Err(Error::validation(format!(
                    "Unsupported hash algorithm: {}",
                    algorithm
                )));
            }
        };

        let output = Command::new(command)
            .arg(path)
            .output()
            .await
            .map_err(|e| Error::internal(format!("Failed to compute checksum: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::internal(format!("Checksum computation failed: {}", stderr)));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let hash = stdout
            .split_whitespace()
            .next()
            .ok_or_else(|| Error::internal("Failed to parse checksum output"))?;

        Ok(hash.to_lowercase())
    }

    /// Find pack directory (pack.yaml location)
    async fn find_pack_directory(&self, base_dir: &Path) -> Result<PathBuf> {
        // Check if pack.yaml exists at root
        let root_pack_yaml = base_dir.join("pack.yaml");
        if root_pack_yaml.exists() {
            return Ok(base_dir.to_path_buf());
        }

        // Check in pack/ subdirectory
        let pack_subdir = base_dir.join("pack");
        let pack_subdir_yaml = pack_subdir.join("pack.yaml");
        if pack_subdir_yaml.exists() {
            return Ok(pack_subdir);
        }

        // Check in first subdirectory (common for GitHub archives)
        let mut entries = fs::read_dir(base_dir)
            .await
            .map_err(|e| Error::internal(format!("Failed to read directory: {}", e)))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| Error::internal(format!("Failed to read directory entry: {}", e)))?
        {
            let path = entry.path();
            if path.is_dir() {
                let subdir_pack_yaml = path.join("pack.yaml");
                if subdir_pack_yaml.exists() {
                    return Ok(path);
                }
            }
        }

        Err(Error::validation(format!(
            "pack.yaml not found in {}",
            base_dir.display()
        )))
    }

    /// Copy directory recursively
    #[async_recursion::async_recursion]
    async fn copy_directory(&self, src: &Path, dst: &Path) -> Result<()> {
        use tokio::fs;

        // Create destination directory if it doesn't exist
        fs::create_dir_all(dst)
            .await
            .map_err(|e| Error::internal(format!("Failed to create destination directory: {}", e)))?;

        // Read source directory
        let mut entries = fs::read_dir(src)
            .await
            .map_err(|e| Error::internal(format!("Failed to read source directory: {}", e)))?;

        // Copy each entry
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| Error::internal(format!("Failed to read directory entry: {}", e)))?
        {
            let path = entry.path();
            let file_name = entry.file_name();
            let dest_path = dst.join(&file_name);

            let metadata = entry
                .metadata()
                .await
                .map_err(|e| Error::internal(format!("Failed to read entry metadata: {}", e)))?;

            if metadata.is_dir() {
                // Recursively copy subdirectory
                self.copy_directory(&path, &dest_path).await?;
            } else {
                // Copy file
                fs::copy(&path, &dest_path)
                    .await
                    .map_err(|e| Error::internal(format!("Failed to copy file: {}", e)))?;
            }
        }

        Ok(())
    }

    /// Create a unique temporary directory
    async fn create_temp_dir(&self) -> Result<PathBuf> {
        let uuid = uuid::Uuid::new_v4();
        let dir = self.temp_dir.join(uuid.to_string());

        fs::create_dir_all(&dir)
            .await
            .map_err(|e| Error::internal(format!("Failed to create temp directory: {}", e)))?;

        Ok(dir)
    }

    /// Clean up temporary directory
    pub async fn cleanup(&self, pack_path: &Path) -> Result<()> {
        if pack_path.starts_with(&self.temp_dir) {
            fs::remove_dir_all(pack_path)
                .await
                .map_err(|e| Error::internal(format!("Failed to cleanup temp directory: {}", e)))?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_checksum_parsing() {
        let checksum = Checksum::parse("sha256:abc123def456").unwrap();
        assert_eq!(checksum.algorithm, "sha256");
        assert_eq!(checksum.hash, "abc123def456");
    }

    #[tokio::test]
    async fn test_select_install_source_prefers_git() {
        let entry = PackIndexEntry {
            pack_ref: "test".to_string(),
            label: "Test".to_string(),
            description: "Test pack".to_string(),
            version: "1.0.0".to_string(),
            author: "Test".to_string(),
            email: None,
            homepage: None,
            repository: None,
            license: "MIT".to_string(),
            keywords: vec![],
            runtime_deps: vec![],
            install_sources: vec![
                InstallSource::Archive {
                    url: "https://example.com/archive.zip".to_string(),
                    checksum: "sha256:abc123".to_string(),
                },
                InstallSource::Git {
                    url: "https://github.com/example/pack".to_string(),
                    git_ref: Some("v1.0.0".to_string()),
                    checksum: "sha256:def456".to_string(),
                },
            ],
            contents: Default::default(),
            dependencies: None,
            meta: None,
        };

        let temp_dir = std::env::temp_dir().join("attune-test");
        let installer = PackInstaller::new(&temp_dir, None).await.unwrap();
        let source = installer.select_install_source(&entry).unwrap();

        assert!(matches!(source, InstallSource::Git { .. }));
    }
}
