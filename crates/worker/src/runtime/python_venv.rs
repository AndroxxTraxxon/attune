//! Python Virtual Environment Manager
//!
//! Manages isolated Python virtual environments for packs with Python dependencies.
//! Each pack gets its own venv to prevent dependency conflicts.

use super::dependency::{
    DependencyError, DependencyManager, DependencyResult, DependencySpec, EnvironmentInfo,
};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tracing::{debug, info, warn};

/// Python virtual environment manager
pub struct PythonVenvManager {
    /// Base directory for all virtual environments
    base_dir: PathBuf,

    /// Python interpreter to use for creating venvs
    python_path: PathBuf,

    /// Cache of environment info
    env_cache: tokio::sync::RwLock<HashMap<String, EnvironmentInfo>>,
}

/// Metadata stored with each environment
#[derive(Debug, Clone, Serialize, Deserialize)]
struct VenvMetadata {
    pack_ref: String,
    dependencies: Vec<String>,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
    python_version: String,
    dependency_hash: String,
}

impl PythonVenvManager {
    /// Create a new Python venv manager
    pub fn new(base_dir: PathBuf) -> Self {
        Self {
            base_dir,
            python_path: PathBuf::from("python3"),
            env_cache: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Create a new Python venv manager with custom Python path
    pub fn with_python_path(base_dir: PathBuf, python_path: PathBuf) -> Self {
        Self {
            base_dir,
            python_path,
            env_cache: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Get the directory path for a pack's venv
    fn get_venv_path(&self, pack_ref: &str) -> PathBuf {
        // Sanitize pack_ref to create a valid directory name
        let safe_name = pack_ref.replace(['/', '\\', '.'], "_");
        self.base_dir.join(safe_name)
    }

    /// Get the Python executable path within a venv
    fn get_venv_python(&self, venv_path: &Path) -> PathBuf {
        if cfg!(windows) {
            venv_path.join("Scripts").join("python.exe")
        } else {
            venv_path.join("bin").join("python")
        }
    }

    /// Get the pip executable path within a venv
    fn get_venv_pip(&self, venv_path: &Path) -> PathBuf {
        if cfg!(windows) {
            venv_path.join("Scripts").join("pip.exe")
        } else {
            venv_path.join("bin").join("pip")
        }
    }

    /// Get the metadata file path for a venv
    fn get_metadata_path(&self, venv_path: &Path) -> PathBuf {
        venv_path.join("attune_metadata.json")
    }

    /// Calculate a hash of dependencies for change detection
    fn calculate_dependency_hash(&self, spec: &DependencySpec) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();

        // Sort dependencies for consistent hashing
        let mut deps = spec.dependencies.clone();
        deps.sort();

        for dep in &deps {
            dep.hash(&mut hasher);
        }

        if let Some(ref content) = spec.requirements_file_content {
            content.hash(&mut hasher);
        }

        format!("{:x}", hasher.finish())
    }

    /// Create a new virtual environment
    async fn create_venv(&self, venv_path: &Path) -> DependencyResult<()> {
        info!(
            "Creating Python virtual environment at: {}",
            venv_path.display()
        );

        // Ensure base directory exists
        if let Some(parent) = venv_path.parent() {
            fs::create_dir_all(parent).await?;
        }

        // Create venv using python -m venv
        let output = Command::new(&self.python_path)
            .arg("-m")
            .arg("venv")
            .arg(venv_path)
            .arg("--clear") // Clear if exists
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| {
                DependencyError::CreateEnvironmentFailed(format!(
                    "Failed to spawn venv command: {}",
                    e
                ))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(DependencyError::CreateEnvironmentFailed(format!(
                "venv creation failed: {}",
                stderr
            )));
        }

        // Upgrade pip to latest version
        let pip_path = self.get_venv_pip(venv_path);
        let output = Command::new(&pip_path)
            .arg("install")
            .arg("--upgrade")
            .arg("pip")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| DependencyError::InstallFailed(format!("Failed to upgrade pip: {}", e)))?;

        if !output.status.success() {
            warn!("Failed to upgrade pip, continuing anyway");
        }

        info!("Virtual environment created successfully");
        Ok(())
    }

    /// Install dependencies in a venv
    async fn install_dependencies(
        &self,
        venv_path: &Path,
        spec: &DependencySpec,
    ) -> DependencyResult<()> {
        if !spec.has_dependencies() {
            debug!("No dependencies to install");
            return Ok(());
        }

        info!("Installing dependencies in venv: {}", venv_path.display());

        let pip_path = self.get_venv_pip(venv_path);

        // Install from requirements file content if provided
        if let Some(ref requirements_content) = spec.requirements_file_content {
            let req_file = venv_path.join("requirements.txt");
            fs::write(&req_file, requirements_content).await?;

            let output = Command::new(&pip_path)
                .arg("install")
                .arg("-r")
                .arg(&req_file)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
                .map_err(|e| {
                    DependencyError::InstallFailed(format!(
                        "Failed to install from requirements.txt: {}",
                        e
                    ))
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(DependencyError::InstallFailed(format!(
                    "pip install failed: {}",
                    stderr
                )));
            }

            info!("Dependencies installed from requirements.txt");
        } else if !spec.dependencies.is_empty() {
            // Install individual dependencies
            let output = Command::new(&pip_path)
                .arg("install")
                .args(&spec.dependencies)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .output()
                .await
                .map_err(|e| {
                    DependencyError::InstallFailed(format!("Failed to install dependencies: {}", e))
                })?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(DependencyError::InstallFailed(format!(
                    "pip install failed: {}",
                    stderr
                )));
            }

            info!("Installed {} dependencies", spec.dependencies.len());
        }

        Ok(())
    }

    /// Get Python version from a venv
    async fn get_python_version(&self, venv_path: &Path) -> DependencyResult<String> {
        let python_path = self.get_venv_python(venv_path);

        let output = Command::new(&python_path)
            .arg("--version")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| {
                DependencyError::ProcessError(format!("Failed to get Python version: {}", e))
            })?;

        if !output.status.success() {
            return Err(DependencyError::ProcessError(
                "Failed to get Python version".to_string(),
            ));
        }

        let version = String::from_utf8_lossy(&output.stdout);
        Ok(version.trim().to_string())
    }

    /// List installed packages in a venv
    async fn list_installed_packages(&self, venv_path: &Path) -> DependencyResult<Vec<String>> {
        let pip_path = self.get_venv_pip(venv_path);

        let output = Command::new(&pip_path)
            .arg("list")
            .arg("--format=freeze")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .await
            .map_err(|e| {
                DependencyError::ProcessError(format!("Failed to list packages: {}", e))
            })?;

        if !output.status.success() {
            return Ok(Vec::new());
        }

        let packages = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(|s| s.to_string())
            .collect();

        Ok(packages)
    }

    /// Save metadata for a venv
    async fn save_metadata(
        &self,
        venv_path: &Path,
        metadata: &VenvMetadata,
    ) -> DependencyResult<()> {
        let metadata_path = self.get_metadata_path(venv_path);
        let json = serde_json::to_string_pretty(metadata).map_err(|e| {
            DependencyError::LockFileError(format!("Failed to serialize metadata: {}", e))
        })?;

        let mut file = fs::File::create(&metadata_path).await.map_err(|e| {
            DependencyError::LockFileError(format!("Failed to create metadata file: {}", e))
        })?;

        file.write_all(json.as_bytes()).await.map_err(|e| {
            DependencyError::LockFileError(format!("Failed to write metadata: {}", e))
        })?;

        Ok(())
    }

    /// Load metadata for a venv
    async fn load_metadata(&self, venv_path: &Path) -> DependencyResult<Option<VenvMetadata>> {
        let metadata_path = self.get_metadata_path(venv_path);

        if !metadata_path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&metadata_path).await.map_err(|e| {
            DependencyError::LockFileError(format!("Failed to read metadata: {}", e))
        })?;

        let metadata: VenvMetadata = serde_json::from_str(&content).map_err(|e| {
            DependencyError::LockFileError(format!("Failed to parse metadata: {}", e))
        })?;

        Ok(Some(metadata))
    }

    /// Check if a venv exists and is valid
    async fn is_valid_venv(&self, venv_path: &Path) -> bool {
        if !venv_path.exists() {
            return false;
        }

        let python_path = self.get_venv_python(venv_path);
        if !python_path.exists() {
            return false;
        }

        // Try to run python --version to verify it works
        let result = Command::new(&python_path)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await;

        matches!(result, Ok(status) if status.success())
    }

    /// Build environment info from a venv
    async fn build_env_info(
        &self,
        pack_ref: &str,
        venv_path: &Path,
    ) -> DependencyResult<EnvironmentInfo> {
        let is_valid = self.is_valid_venv(venv_path).await;
        let python_path = self.get_venv_python(venv_path);

        let (python_version, installed_deps, created_at, updated_at) = if is_valid {
            let version = self
                .get_python_version(venv_path)
                .await
                .unwrap_or_else(|_| "Unknown".to_string());
            let deps = self
                .list_installed_packages(venv_path)
                .await
                .unwrap_or_default();

            let metadata = self.load_metadata(venv_path).await.ok().flatten();
            let created = metadata
                .as_ref()
                .map(|m| m.created_at)
                .unwrap_or_else(chrono::Utc::now);
            let updated = metadata
                .as_ref()
                .map(|m| m.updated_at)
                .unwrap_or_else(chrono::Utc::now);

            (version, deps, created, updated)
        } else {
            (
                "Unknown".to_string(),
                Vec::new(),
                chrono::Utc::now(),
                chrono::Utc::now(),
            )
        };

        Ok(EnvironmentInfo {
            id: pack_ref.to_string(),
            path: venv_path.to_path_buf(),
            runtime: "python".to_string(),
            runtime_version: python_version,
            installed_dependencies: installed_deps,
            created_at,
            updated_at,
            is_valid,
            executable_path: python_path,
        })
    }
}

#[async_trait]
impl DependencyManager for PythonVenvManager {
    fn runtime_type(&self) -> &str {
        "python"
    }

    async fn ensure_environment(
        &self,
        pack_ref: &str,
        spec: &DependencySpec,
    ) -> DependencyResult<EnvironmentInfo> {
        info!("Ensuring Python environment for pack: {}", pack_ref);

        let venv_path = self.get_venv_path(pack_ref);
        let dependency_hash = self.calculate_dependency_hash(spec);

        // Check if environment exists and is up to date
        if venv_path.exists() {
            if let Some(metadata) = self.load_metadata(&venv_path).await? {
                if metadata.dependency_hash == dependency_hash
                    && self.is_valid_venv(&venv_path).await
                {
                    debug!("Using existing venv (dependencies unchanged)");
                    let env_info = self.build_env_info(pack_ref, &venv_path).await?;

                    // Update cache
                    let mut cache = self.env_cache.write().await;
                    cache.insert(pack_ref.to_string(), env_info.clone());

                    return Ok(env_info);
                }
                info!("Dependencies changed or venv invalid, recreating environment");
            }
        }

        // Create or recreate the venv
        self.create_venv(&venv_path).await?;

        // Install dependencies
        self.install_dependencies(&venv_path, spec).await?;

        // Get Python version
        let python_version = self.get_python_version(&venv_path).await?;

        // Save metadata
        let metadata = VenvMetadata {
            pack_ref: pack_ref.to_string(),
            dependencies: spec.dependencies.clone(),
            created_at: chrono::Utc::now(),
            updated_at: chrono::Utc::now(),
            python_version: python_version.clone(),
            dependency_hash,
        };
        self.save_metadata(&venv_path, &metadata).await?;

        // Build environment info
        let env_info = self.build_env_info(pack_ref, &venv_path).await?;

        // Update cache
        let mut cache = self.env_cache.write().await;
        cache.insert(pack_ref.to_string(), env_info.clone());

        info!("Python environment ready for pack: {}", pack_ref);
        Ok(env_info)
    }

    async fn get_environment(&self, pack_ref: &str) -> DependencyResult<Option<EnvironmentInfo>> {
        // Check cache first
        {
            let cache = self.env_cache.read().await;
            if let Some(env_info) = cache.get(pack_ref) {
                return Ok(Some(env_info.clone()));
            }
        }

        let venv_path = self.get_venv_path(pack_ref);
        if !venv_path.exists() {
            return Ok(None);
        }

        let env_info = self.build_env_info(pack_ref, &venv_path).await?;

        // Update cache
        let mut cache = self.env_cache.write().await;
        cache.insert(pack_ref.to_string(), env_info.clone());

        Ok(Some(env_info))
    }

    async fn remove_environment(&self, pack_ref: &str) -> DependencyResult<()> {
        info!("Removing Python environment for pack: {}", pack_ref);

        let venv_path = self.get_venv_path(pack_ref);
        if venv_path.exists() {
            fs::remove_dir_all(&venv_path).await?;
        }

        // Remove from cache
        let mut cache = self.env_cache.write().await;
        cache.remove(pack_ref);

        info!("Environment removed");
        Ok(())
    }

    async fn validate_environment(&self, pack_ref: &str) -> DependencyResult<bool> {
        let venv_path = self.get_venv_path(pack_ref);
        Ok(self.is_valid_venv(&venv_path).await)
    }

    async fn get_executable_path(&self, pack_ref: &str) -> DependencyResult<PathBuf> {
        let venv_path = self.get_venv_path(pack_ref);
        let python_path = self.get_venv_python(&venv_path);

        if !python_path.exists() {
            return Err(DependencyError::EnvironmentNotFound(format!(
                "Python executable not found for pack: {}",
                pack_ref
            )));
        }

        Ok(python_path)
    }

    async fn list_environments(&self) -> DependencyResult<Vec<EnvironmentInfo>> {
        let mut environments = Vec::new();

        let mut entries = fs::read_dir(&self.base_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            if entry.file_type().await?.is_dir() {
                let venv_path = entry.path();
                if self.is_valid_venv(&venv_path).await {
                    // Extract pack_ref from directory name
                    if let Some(dir_name) = venv_path.file_name().and_then(|n| n.to_str()) {
                        if let Ok(env_info) = self.build_env_info(dir_name, &venv_path).await {
                            environments.push(env_info);
                        }
                    }
                }
            }
        }

        Ok(environments)
    }

    async fn cleanup(&self, keep_recent: usize) -> DependencyResult<Vec<String>> {
        info!(
            "Cleaning up Python virtual environments (keeping {} most recent)",
            keep_recent
        );

        let mut environments = self.list_environments().await?;

        // Sort by updated_at, newest first
        environments.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        let mut removed = Vec::new();

        // Remove environments beyond keep_recent threshold
        for env in environments.iter().skip(keep_recent) {
            // Also skip if environment is invalid
            if !env.is_valid {
                if let Err(e) = self.remove_environment(&env.id).await {
                    warn!("Failed to remove environment {}: {}", env.id, e);
                } else {
                    removed.push(env.id.clone());
                }
            }
        }

        info!("Cleaned up {} environments", removed.len());
        Ok(removed)
    }

    async fn needs_update(&self, pack_ref: &str, spec: &DependencySpec) -> DependencyResult<bool> {
        let venv_path = self.get_venv_path(pack_ref);

        if !venv_path.exists() {
            return Ok(true);
        }

        if !self.is_valid_venv(&venv_path).await {
            return Ok(true);
        }

        // Check if dependency hash matches
        if let Some(metadata) = self.load_metadata(&venv_path).await? {
            let current_hash = self.calculate_dependency_hash(spec);
            Ok(metadata.dependency_hash != current_hash)
        } else {
            // No metadata, assume needs update
            Ok(true)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_venv_path_sanitization() {
        let temp_dir = TempDir::new().unwrap();
        let manager = PythonVenvManager::new(temp_dir.path().to_path_buf());

        let path = manager.get_venv_path("core.http");
        assert!(path.to_string_lossy().contains("core_http"));

        let path = manager.get_venv_path("my/pack");
        assert!(path.to_string_lossy().contains("my_pack"));
    }

    #[test]
    fn test_dependency_hash_consistency() {
        let temp_dir = TempDir::new().unwrap();
        let manager = PythonVenvManager::new(temp_dir.path().to_path_buf());

        let spec1 = DependencySpec::new("python")
            .with_dependency("requests==2.28.0")
            .with_dependency("flask==2.0.0");

        let spec2 = DependencySpec::new("python")
            .with_dependency("flask==2.0.0")
            .with_dependency("requests==2.28.0");

        // Hashes should be the same regardless of order (we sort)
        let hash1 = manager.calculate_dependency_hash(&spec1);
        let hash2 = manager.calculate_dependency_hash(&spec2);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_dependency_hash_different() {
        let temp_dir = TempDir::new().unwrap();
        let manager = PythonVenvManager::new(temp_dir.path().to_path_buf());

        let spec1 = DependencySpec::new("python").with_dependency("requests==2.28.0");

        let spec2 = DependencySpec::new("python").with_dependency("requests==2.29.0");

        let hash1 = manager.calculate_dependency_hash(&spec1);
        let hash2 = manager.calculate_dependency_hash(&spec2);
        assert_ne!(hash1, hash2);
    }
}
