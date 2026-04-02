//! Pack Storage Management
//!
//! This module provides utilities for managing pack storage, including:
//! - Checksum calculation (SHA256)
//! - Pack directory management
//! - Storage path resolution
//! - Pack content verification

use crate::error::{Error, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

/// Pack storage manager
pub struct PackStorage {
    base_dir: PathBuf,
}

impl PackStorage {
    /// Create a new PackStorage instance
    ///
    /// # Arguments
    ///
    /// * `base_dir` - Base directory for pack storage (e.g., /opt/attune/packs)
    pub fn new<P: Into<PathBuf>>(base_dir: P) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    /// Get the storage path for a pack
    ///
    /// # Arguments
    ///
    /// * `pack_ref` - Pack reference (e.g., "core", "my_pack")
    /// * `version` - Optional version (e.g., "1.0.0")
    ///
    /// # Returns
    ///
    /// Path where the pack should be stored
    pub fn get_pack_path(&self, pack_ref: &str, version: Option<&str>) -> PathBuf {
        if let Some(v) = version {
            self.base_dir.join(format!("{}-{}", pack_ref, v))
        } else {
            self.base_dir.join(pack_ref)
        }
    }

    /// Ensure the base directory exists
    pub fn ensure_base_dir(&self) -> Result<()> {
        if !self.base_dir.exists() {
            fs::create_dir_all(&self.base_dir).map_err(|e| {
                Error::io(format!(
                    "Failed to create pack storage directory {}: {}",
                    self.base_dir.display(),
                    e
                ))
            })?;
        }
        Ok(())
    }

    /// Move a pack from temporary location to permanent storage
    ///
    /// # Arguments
    ///
    /// * `source` - Source directory (temporary location)
    /// * `pack_ref` - Pack reference
    /// * `version` - Optional version
    ///
    /// # Returns
    ///
    /// The final storage path
    pub fn install_pack<P: AsRef<Path>>(
        &self,
        source: P,
        pack_ref: &str,
        version: Option<&str>,
    ) -> Result<PathBuf> {
        self.ensure_base_dir()?;

        let dest = self.get_pack_path(pack_ref, version);

        // Remove existing installation if present
        if dest.exists() {
            fs::remove_dir_all(&dest).map_err(|e| {
                Error::io(format!(
                    "Failed to remove existing pack at {}: {}",
                    dest.display(),
                    e
                ))
            })?;
        }

        // Copy the pack to permanent storage
        copy_dir_all(source.as_ref(), &dest)?;

        Ok(dest)
    }

    /// Remove a pack from storage
    ///
    /// # Arguments
    ///
    /// * `pack_ref` - Pack reference
    /// * `version` - Optional version
    pub fn uninstall_pack(&self, pack_ref: &str, version: Option<&str>) -> Result<()> {
        let path = self.get_pack_path(pack_ref, version);

        if path.exists() {
            fs::remove_dir_all(&path).map_err(|e| {
                Error::io(format!(
                    "Failed to remove pack at {}: {}",
                    path.display(),
                    e
                ))
            })?;
        }

        Ok(())
    }

    /// Check if a pack is installed
    pub fn is_installed(&self, pack_ref: &str, version: Option<&str>) -> bool {
        let path = self.get_pack_path(pack_ref, version);
        path.exists() && path.is_dir()
    }

    /// List all installed packs
    pub fn list_installed(&self) -> Result<Vec<String>> {
        if !self.base_dir.exists() {
            return Ok(Vec::new());
        }

        let mut packs = Vec::new();

        let entries = fs::read_dir(&self.base_dir).map_err(|e| {
            Error::io(format!(
                "Failed to read pack directory {}: {}",
                self.base_dir.display(),
                e
            ))
        })?;

        for entry in entries {
            let entry =
                entry.map_err(|e| Error::io(format!("Failed to read directory entry: {}", e)))?;
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    packs.push(name.to_string());
                }
            }
        }

        Ok(packs)
    }
}

/// Calculate SHA256 checksum of a directory
///
/// This recursively hashes all files in the directory in a deterministic order
/// (sorted by path) to produce a consistent checksum.
///
/// # Arguments
///
/// * `path` - Path to the directory
///
/// # Returns
///
/// Hex-encoded SHA256 checksum
pub fn calculate_directory_checksum<P: AsRef<Path>>(path: P) -> Result<String> {
    let path = path.as_ref();

    if !path.exists() {
        return Err(Error::io(format!(
            "Path does not exist: {}",
            path.display()
        )));
    }

    if !path.is_dir() {
        return Err(Error::validation(format!(
            "Path is not a directory: {}",
            path.display()
        )));
    }

    let mut hasher = Sha256::new();
    let mut files: Vec<PathBuf> = Vec::new();

    // Collect all files in sorted order for deterministic hashing
    for entry in WalkDir::new(path).sort_by_file_name().into_iter() {
        let entry = entry.map_err(|e| Error::io(format!("Failed to walk directory: {}", e)))?;
        if entry.file_type().is_file() {
            files.push(entry.path().to_path_buf());
        }
    }

    // Hash each file
    for file_path in files {
        // Include relative path in hash for structure integrity
        let rel_path = file_path
            .strip_prefix(path)
            .map_err(|e| Error::io(format!("Failed to strip prefix: {}", e)))?;

        hasher.update(rel_path.to_string_lossy().as_bytes());

        // Hash file contents
        let mut file = fs::File::open(&file_path).map_err(|e| {
            Error::io(format!(
                "Failed to open file {}: {}",
                file_path.display(),
                e
            ))
        })?;

        let mut buffer = [0u8; 8192];
        loop {
            let n = file.read(&mut buffer).map_err(|e| {
                Error::io(format!(
                    "Failed to read file {}: {}",
                    file_path.display(),
                    e
                ))
            })?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }
    }

    let result = hasher.finalize();
    Ok(format!("{:x}", result))
}

/// Calculate SHA256 checksum of a single file
///
/// # Arguments
///
/// * `path` - Path to the file
///
/// # Returns
///
/// Hex-encoded SHA256 checksum
pub fn calculate_file_checksum<P: AsRef<Path>>(path: P) -> Result<String> {
    let path = path.as_ref();

    if !path.exists() {
        return Err(Error::io(format!(
            "File does not exist: {}",
            path.display()
        )));
    }

    if !path.is_file() {
        return Err(Error::validation(format!(
            "Path is not a file: {}",
            path.display()
        )));
    }

    let mut hasher = Sha256::new();
    let mut file = fs::File::open(path)
        .map_err(|e| Error::io(format!("Failed to open file {}: {}", path.display(), e)))?;

    let mut buffer = [0u8; 8192];
    loop {
        let n = file
            .read(&mut buffer)
            .map_err(|e| Error::io(format!("Failed to read file {}: {}", path.display(), e)))?;
        if n == 0 {
            break;
        }
        hasher.update(&buffer[..n]);
    }

    let result = hasher.finalize();
    Ok(format!("{:x}", result))
}

/// Copy a directory recursively
fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst).map_err(|e| {
        Error::io(format!(
            "Failed to create destination directory {}: {}",
            dst.display(),
            e
        ))
    })?;

    // nosemgrep: rust.actix.path-traversal.tainted-path.tainted-path -- Pack storage copy recursively processes validated local directories under the configured pack store.
    for entry in fs::read_dir(src).map_err(|e| {
        Error::io(format!(
            "Failed to read source directory {}: {}",
            src.display(),
            e
        ))
    })? {
        let entry =
            entry.map_err(|e| Error::io(format!("Failed to read directory entry: {}", e)))?;
        let path = entry.path();
        let file_name = entry.file_name();
        let dest_path = dst.join(&file_name);

        if path.is_dir() {
            copy_dir_all(&path, &dest_path)?;
        } else {
            fs::copy(&path, &dest_path).map_err(|e| {
                Error::io(format!(
                    "Failed to copy file {} to {}: {}",
                    path.display(),
                    dest_path.display(),
                    e
                ))
            })?;
        }
    }

    Ok(())
}

/// Verify a pack's checksum matches the expected value
///
/// # Arguments
///
/// * `pack_path` - Path to the pack directory
/// * `expected_checksum` - Expected SHA256 checksum (hex-encoded)
///
/// # Returns
///
/// `Ok(true)` if checksums match, `Ok(false)` if they don't match,
/// or `Err` on I/O errors
pub fn verify_checksum<P: AsRef<Path>>(pack_path: P, expected_checksum: &str) -> Result<bool> {
    let actual = calculate_directory_checksum(pack_path)?;
    Ok(actual.eq_ignore_ascii_case(expected_checksum))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::TempDir;

    #[test]
    fn test_pack_storage_paths() {
        let storage = PackStorage::new("/opt/attune/packs");

        let path1 = storage.get_pack_path("core", None);
        assert_eq!(path1, PathBuf::from("/opt/attune/packs/core"));

        let path2 = storage.get_pack_path("core", Some("1.0.0"));
        assert_eq!(path2, PathBuf::from("/opt/attune/packs/core-1.0.0"));
    }

    #[test]
    fn test_calculate_file_checksum() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");

        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"Hello, world!").unwrap();
        drop(file);

        let checksum = calculate_file_checksum(&file_path).unwrap();

        // Known SHA256 of "Hello, world!"
        assert_eq!(
            checksum,
            "315f5bdb76d078c43b8ac0064e4a0164612b1fce77c869345bfc94c75894edd3"
        );
    }

    #[test]
    fn test_calculate_directory_checksum() {
        let temp_dir = TempDir::new().unwrap();

        // Create a simple directory structure
        let subdir = temp_dir.path().join("subdir");
        fs::create_dir(&subdir).unwrap();

        let file1 = temp_dir.path().join("file1.txt");
        let mut f = File::create(&file1).unwrap();
        f.write_all(b"content1").unwrap();
        drop(f);

        let file2 = subdir.join("file2.txt");
        let mut f = File::create(&file2).unwrap();
        f.write_all(b"content2").unwrap();
        drop(f);

        let checksum1 = calculate_directory_checksum(temp_dir.path()).unwrap();

        // Calculate again - should be deterministic
        let checksum2 = calculate_directory_checksum(temp_dir.path()).unwrap();

        assert_eq!(checksum1, checksum2);
        assert_eq!(checksum1.len(), 64); // SHA256 is 64 hex characters
    }
}
