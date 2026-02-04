//! Runtime Dependency Management
//!
//! Provides generic abstractions for managing runtime dependencies across
//! different languages (Python, Node.js, Java, etc.).

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use thiserror::Error;

/// Dependency manager result type
pub type DependencyResult<T> = std::result::Result<T, DependencyError>;

/// Dependency manager errors
#[derive(Debug, Error)]
pub enum DependencyError {
    #[error("Failed to create environment: {0}")]
    CreateEnvironmentFailed(String),

    #[error("Failed to install dependencies: {0}")]
    InstallFailed(String),

    #[error("Environment not found: {0}")]
    EnvironmentNotFound(String),

    #[error("Invalid dependency specification: {0}")]
    InvalidDependencySpec(String),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Process execution error: {0}")]
    ProcessError(String),

    #[error("Lock file error: {0}")]
    LockFileError(String),

    #[error("Environment validation failed: {0}")]
    ValidationFailed(String),
}

/// Dependency specification for a pack
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencySpec {
    /// Runtime type (python, nodejs, java, etc.)
    pub runtime: String,

    /// List of dependencies (e.g., ["requests==2.28.0", "flask>=2.0.0"])
    pub dependencies: Vec<String>,

    /// Requirements file content (alternative to dependencies list)
    pub requirements_file_content: Option<String>,

    /// Minimum runtime version required
    pub min_version: Option<String>,

    /// Maximum runtime version required
    pub max_version: Option<String>,

    /// Additional metadata
    pub metadata: HashMap<String, serde_json::Value>,
}

impl DependencySpec {
    /// Create a new dependency specification
    pub fn new(runtime: impl Into<String>) -> Self {
        Self {
            runtime: runtime.into(),
            dependencies: Vec::new(),
            requirements_file_content: None,
            min_version: None,
            max_version: None,
            metadata: HashMap::new(),
        }
    }

    /// Add a dependency
    pub fn with_dependency(mut self, dep: impl Into<String>) -> Self {
        self.dependencies.push(dep.into());
        self
    }

    /// Add multiple dependencies
    pub fn with_dependencies(mut self, deps: Vec<String>) -> Self {
        self.dependencies.extend(deps);
        self
    }

    /// Set requirements file content
    pub fn with_requirements_file(mut self, content: String) -> Self {
        self.requirements_file_content = Some(content);
        self
    }

    /// Set version constraints
    pub fn with_version_range(
        mut self,
        min_version: Option<String>,
        max_version: Option<String>,
    ) -> Self {
        self.min_version = min_version;
        self.max_version = max_version;
        self
    }

    /// Check if this spec has any dependencies
    pub fn has_dependencies(&self) -> bool {
        !self.dependencies.is_empty() || self.requirements_file_content.is_some()
    }
}

/// Information about an isolated environment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvironmentInfo {
    /// Unique environment identifier (typically pack_ref)
    pub id: String,

    /// Path to the environment directory
    pub path: PathBuf,

    /// Runtime type
    pub runtime: String,

    /// Runtime version in the environment
    pub runtime_version: String,

    /// List of installed dependencies
    pub installed_dependencies: Vec<String>,

    /// Timestamp when environment was created
    pub created_at: chrono::DateTime<chrono::Utc>,

    /// Timestamp when environment was last updated
    pub updated_at: chrono::DateTime<chrono::Utc>,

    /// Whether the environment is valid and ready to use
    pub is_valid: bool,

    /// Environment-specific executable path (e.g., venv/bin/python)
    pub executable_path: PathBuf,
}

/// Trait for managing isolated runtime environments
#[async_trait]
pub trait DependencyManager: Send + Sync {
    /// Get the runtime type this manager handles (e.g., "python", "nodejs")
    fn runtime_type(&self) -> &str;

    /// Create or update an isolated environment for a pack
    ///
    /// # Arguments
    /// * `pack_ref` - Unique identifier for the pack (e.g., "core.http")
    /// * `spec` - Dependency specification
    ///
    /// # Returns
    /// Information about the created/updated environment
    async fn ensure_environment(
        &self,
        pack_ref: &str,
        spec: &DependencySpec,
    ) -> DependencyResult<EnvironmentInfo>;

    /// Get information about an existing environment
    async fn get_environment(&self, pack_ref: &str) -> DependencyResult<Option<EnvironmentInfo>>;

    /// Remove an environment
    async fn remove_environment(&self, pack_ref: &str) -> DependencyResult<()>;

    /// Validate an environment is still functional
    async fn validate_environment(&self, pack_ref: &str) -> DependencyResult<bool>;

    /// Get the executable path for running actions in this environment
    ///
    /// # Arguments
    /// * `pack_ref` - Pack identifier
    ///
    /// # Returns
    /// Path to the runtime executable within the isolated environment
    async fn get_executable_path(&self, pack_ref: &str) -> DependencyResult<PathBuf>;

    /// List all managed environments
    async fn list_environments(&self) -> DependencyResult<Vec<EnvironmentInfo>>;

    /// Clean up invalid or unused environments
    async fn cleanup(&self, keep_recent: usize) -> DependencyResult<Vec<String>>;

    /// Check if dependencies have changed and environment needs updating
    async fn needs_update(&self, pack_ref: &str, _spec: &DependencySpec) -> DependencyResult<bool> {
        // Default implementation: check if environment exists and validate it
        match self.get_environment(pack_ref).await? {
            None => Ok(true), // Doesn't exist, needs creation
            Some(env_info) => {
                // Check if environment is valid
                if !env_info.is_valid {
                    return Ok(true);
                }

                // Could add more sophisticated checks here (dependency hash comparison, etc.)
                Ok(false)
            }
        }
    }
}

/// Registry for managing multiple dependency managers
pub struct DependencyManagerRegistry {
    managers: HashMap<String, Box<dyn DependencyManager>>,
}

impl DependencyManagerRegistry {
    /// Create a new registry
    pub fn new() -> Self {
        Self {
            managers: HashMap::new(),
        }
    }

    /// Register a dependency manager
    pub fn register(&mut self, manager: Box<dyn DependencyManager>) {
        let runtime_type = manager.runtime_type().to_string();
        self.managers.insert(runtime_type, manager);
    }

    /// Get a dependency manager by runtime type
    pub fn get(&self, runtime_type: &str) -> Option<&dyn DependencyManager> {
        self.managers.get(runtime_type).map(|m| m.as_ref())
    }

    /// Check if a runtime type is supported
    pub fn supports(&self, runtime_type: &str) -> bool {
        self.managers.contains_key(runtime_type)
    }

    /// List all supported runtime types
    pub fn supported_runtimes(&self) -> Vec<String> {
        self.managers.keys().cloned().collect()
    }

    /// Ensure environment for a pack with given spec
    pub async fn ensure_environment(
        &self,
        pack_ref: &str,
        spec: &DependencySpec,
    ) -> DependencyResult<EnvironmentInfo> {
        let manager = self.get(&spec.runtime).ok_or_else(|| {
            DependencyError::InvalidDependencySpec(format!(
                "No dependency manager found for runtime: {}",
                spec.runtime
            ))
        })?;

        manager.ensure_environment(pack_ref, spec).await
    }

    /// Get executable path for a pack
    pub async fn get_executable_path(
        &self,
        pack_ref: &str,
        runtime_type: &str,
    ) -> DependencyResult<PathBuf> {
        let manager = self.get(runtime_type).ok_or_else(|| {
            DependencyError::InvalidDependencySpec(format!(
                "No dependency manager found for runtime: {}",
                runtime_type
            ))
        })?;

        manager.get_executable_path(pack_ref).await
    }

    /// Cleanup all managers
    pub async fn cleanup_all(&self, keep_recent: usize) -> DependencyResult<Vec<String>> {
        let mut removed = Vec::new();

        for manager in self.managers.values() {
            let mut cleaned = manager.cleanup(keep_recent).await?;
            removed.append(&mut cleaned);
        }

        Ok(removed)
    }
}

impl Default for DependencyManagerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dependency_spec_builder() {
        let spec = DependencySpec::new("python")
            .with_dependency("requests==2.28.0")
            .with_dependency("flask>=2.0.0")
            .with_version_range(Some("3.8".to_string()), Some("3.11".to_string()));

        assert_eq!(spec.runtime, "python");
        assert_eq!(spec.dependencies.len(), 2);
        assert!(spec.has_dependencies());
        assert_eq!(spec.min_version, Some("3.8".to_string()));
    }

    #[test]
    fn test_dependency_spec_empty() {
        let spec = DependencySpec::new("nodejs");
        assert!(!spec.has_dependencies());
    }

    #[test]
    fn test_dependency_manager_registry() {
        let registry = DependencyManagerRegistry::new();
        assert_eq!(registry.supported_runtimes().len(), 0);
        assert!(!registry.supports("python"));
    }
}
