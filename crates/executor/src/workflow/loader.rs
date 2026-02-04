//! Workflow Loader
//!
//! This module handles loading workflow definitions from YAML files in pack directories.
//! It scans pack directories, parses workflow YAML files, validates them, and prepares
//! them for registration in the database.

use attune_common::error::{Error, Result};

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tracing::{debug, info, warn};

use super::parser::{parse_workflow_yaml, WorkflowDefinition};
use super::validator::WorkflowValidator;

/// Workflow file metadata
#[derive(Debug, Clone)]
pub struct WorkflowFile {
    /// Full path to the workflow YAML file
    pub path: PathBuf,
    /// Pack name
    pub pack: String,
    /// Workflow name (from filename)
    pub name: String,
    /// Workflow reference (pack.name)
    pub ref_name: String,
}

/// Loaded workflow ready for registration
#[derive(Debug, Clone)]
pub struct LoadedWorkflow {
    /// File metadata
    pub file: WorkflowFile,
    /// Parsed workflow definition
    pub workflow: WorkflowDefinition,
    /// Validation error (if any)
    pub validation_error: Option<String>,
}

/// Workflow loader configuration
#[derive(Debug, Clone)]
pub struct LoaderConfig {
    /// Base directory containing pack directories
    pub packs_base_dir: PathBuf,
    /// Whether to skip validation errors
    pub skip_validation: bool,
    /// Maximum workflow file size in bytes (default: 1MB)
    pub max_file_size: usize,
}

impl Default for LoaderConfig {
    fn default() -> Self {
        Self {
            packs_base_dir: PathBuf::from("/opt/attune/packs"),
            skip_validation: false,
            max_file_size: 1024 * 1024, // 1MB
        }
    }
}

/// Workflow loader for scanning and loading workflow files
pub struct WorkflowLoader {
    config: LoaderConfig,
}

impl WorkflowLoader {
    /// Create a new workflow loader
    pub fn new(config: LoaderConfig) -> Self {
        Self { config }
    }

    /// Scan all packs and load all workflows
    ///
    /// Returns a map of workflow reference names to loaded workflows
    pub async fn load_all_workflows(&self) -> Result<HashMap<String, LoadedWorkflow>> {
        info!(
            "Scanning for workflows in: {}",
            self.config.packs_base_dir.display()
        );

        let mut workflows = HashMap::new();
        let pack_dirs = self.scan_pack_directories().await?;

        for pack_dir in pack_dirs {
            let pack_name = pack_dir
                .file_name()
                .and_then(|n| n.to_str())
                .ok_or_else(|| Error::validation("Invalid pack directory name"))?
                .to_string();

            match self.load_pack_workflows(&pack_name, &pack_dir).await {
                Ok(pack_workflows) => {
                    info!(
                        "Loaded {} workflows from pack '{}'",
                        pack_workflows.len(),
                        pack_name
                    );
                    workflows.extend(pack_workflows);
                }
                Err(e) => {
                    warn!("Failed to load workflows from pack '{}': {}", pack_name, e);
                }
            }
        }

        info!("Total workflows loaded: {}", workflows.len());
        Ok(workflows)
    }

    /// Load all workflows from a specific pack
    pub async fn load_pack_workflows(
        &self,
        pack_name: &str,
        pack_dir: &Path,
    ) -> Result<HashMap<String, LoadedWorkflow>> {
        let workflows_dir = pack_dir.join("workflows");

        if !workflows_dir.exists() {
            debug!("No workflows directory in pack '{}'", pack_name);
            return Ok(HashMap::new());
        }

        let workflow_files = self.scan_workflow_files(&workflows_dir, pack_name).await?;
        let mut workflows = HashMap::new();

        for file in workflow_files {
            match self.load_workflow_file(&file).await {
                Ok(loaded) => {
                    workflows.insert(loaded.file.ref_name.clone(), loaded);
                }
                Err(e) => {
                    warn!("Failed to load workflow '{}': {}", file.path.display(), e);
                }
            }
        }

        Ok(workflows)
    }

    /// Load a single workflow file
    pub async fn load_workflow_file(&self, file: &WorkflowFile) -> Result<LoadedWorkflow> {
        debug!("Loading workflow from: {}", file.path.display());

        // Check file size
        let metadata = fs::metadata(&file.path).await.map_err(|e| {
            Error::validation(format!("Failed to read workflow file metadata: {}", e))
        })?;

        if metadata.len() > self.config.max_file_size as u64 {
            return Err(Error::validation(format!(
                "Workflow file exceeds maximum size of {} bytes",
                self.config.max_file_size
            )));
        }

        // Read and parse YAML
        let content = fs::read_to_string(&file.path)
            .await
            .map_err(|e| Error::validation(format!("Failed to read workflow file: {}", e)))?;

        let workflow = parse_workflow_yaml(&content)?;

        // Validate workflow
        let validation_error = if self.config.skip_validation {
            None
        } else {
            WorkflowValidator::validate(&workflow)
                .err()
                .map(|e| e.to_string())
        };

        if validation_error.is_some() && !self.config.skip_validation {
            return Err(Error::validation(format!(
                "Workflow validation failed: {}",
                validation_error.as_ref().unwrap()
            )));
        }

        Ok(LoadedWorkflow {
            file: file.clone(),
            workflow,
            validation_error,
        })
    }

    /// Reload a specific workflow by reference
    pub async fn reload_workflow(&self, ref_name: &str) -> Result<LoadedWorkflow> {
        let parts: Vec<&str> = ref_name.split('.').collect();
        if parts.len() != 2 {
            return Err(Error::validation(format!(
                "Invalid workflow reference: {}",
                ref_name
            )));
        }

        let pack_name = parts[0];
        let workflow_name = parts[1];

        let pack_dir = self.config.packs_base_dir.join(pack_name);
        let workflow_path = pack_dir
            .join("workflows")
            .join(format!("{}.yaml", workflow_name));

        if !workflow_path.exists() {
            // Try .yml extension
            let workflow_path_yml = pack_dir
                .join("workflows")
                .join(format!("{}.yml", workflow_name));
            if workflow_path_yml.exists() {
                let file = WorkflowFile {
                    path: workflow_path_yml,
                    pack: pack_name.to_string(),
                    name: workflow_name.to_string(),
                    ref_name: ref_name.to_string(),
                };
                return self.load_workflow_file(&file).await;
            }

            return Err(Error::not_found("workflow", "ref", ref_name));
        }

        let file = WorkflowFile {
            path: workflow_path,
            pack: pack_name.to_string(),
            name: workflow_name.to_string(),
            ref_name: ref_name.to_string(),
        };

        self.load_workflow_file(&file).await
    }

    /// Scan pack directories
    async fn scan_pack_directories(&self) -> Result<Vec<PathBuf>> {
        if !self.config.packs_base_dir.exists() {
            return Err(Error::validation(format!(
                "Packs base directory does not exist: {}",
                self.config.packs_base_dir.display()
            )));
        }

        let mut pack_dirs = Vec::new();
        let mut entries = fs::read_dir(&self.config.packs_base_dir)
            .await
            .map_err(|e| Error::validation(format!("Failed to read packs directory: {}", e)))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| Error::validation(format!("Failed to read directory entry: {}", e)))?
        {
            let path = entry.path();
            if path.is_dir() {
                pack_dirs.push(path);
            }
        }

        Ok(pack_dirs)
    }

    /// Scan workflow files in a directory
    async fn scan_workflow_files(
        &self,
        workflows_dir: &Path,
        pack_name: &str,
    ) -> Result<Vec<WorkflowFile>> {
        let mut workflow_files = Vec::new();
        let mut entries = fs::read_dir(workflows_dir)
            .await
            .map_err(|e| Error::validation(format!("Failed to read workflows directory: {}", e)))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| Error::validation(format!("Failed to read directory entry: {}", e)))?
        {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "yaml" || ext == "yml" {
                        if let Some(name) = path.file_stem().and_then(|n| n.to_str()) {
                            let ref_name = format!("{}.{}", pack_name, name);
                            workflow_files.push(WorkflowFile {
                                path: path.clone(),
                                pack: pack_name.to_string(),
                                name: name.to_string(),
                                ref_name,
                            });
                        }
                    }
                }
            }
        }

        Ok(workflow_files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    async fn create_test_pack_structure() -> (TempDir, PathBuf) {
        let temp_dir = TempDir::new().unwrap();
        let packs_dir = temp_dir.path().to_path_buf();

        // Create pack structure
        let pack_dir = packs_dir.join("test_pack");
        let workflows_dir = pack_dir.join("workflows");
        fs::create_dir_all(&workflows_dir).await.unwrap();

        // Create a simple workflow file
        let workflow_yaml = r#"
ref: test_pack.test_workflow
label: Test Workflow
description: A test workflow
version: "1.0.0"
parameters:
  param1:
    type: string
    required: true
tasks:
  - name: task1
    action: core.noop
"#;
        fs::write(workflows_dir.join("test_workflow.yaml"), workflow_yaml)
            .await
            .unwrap();

        (temp_dir, packs_dir)
    }

    #[tokio::test]
    async fn test_scan_pack_directories() {
        let (_temp_dir, packs_dir) = create_test_pack_structure().await;

        let config = LoaderConfig {
            packs_base_dir: packs_dir,
            skip_validation: false,
            max_file_size: 1024 * 1024,
        };

        let loader = WorkflowLoader::new(config);
        let pack_dirs = loader.scan_pack_directories().await.unwrap();

        assert_eq!(pack_dirs.len(), 1);
        assert!(pack_dirs[0].ends_with("test_pack"));
    }

    #[tokio::test]
    async fn test_scan_workflow_files() {
        let (_temp_dir, packs_dir) = create_test_pack_structure().await;
        let pack_dir = packs_dir.join("test_pack");
        let workflows_dir = pack_dir.join("workflows");

        let config = LoaderConfig {
            packs_base_dir: packs_dir,
            skip_validation: false,
            max_file_size: 1024 * 1024,
        };

        let loader = WorkflowLoader::new(config);
        let workflow_files = loader
            .scan_workflow_files(&workflows_dir, "test_pack")
            .await
            .unwrap();

        assert_eq!(workflow_files.len(), 1);
        assert_eq!(workflow_files[0].name, "test_workflow");
        assert_eq!(workflow_files[0].pack, "test_pack");
        assert_eq!(workflow_files[0].ref_name, "test_pack.test_workflow");
    }

    #[tokio::test]
    async fn test_load_workflow_file() {
        let (_temp_dir, packs_dir) = create_test_pack_structure().await;
        let pack_dir = packs_dir.join("test_pack");
        let workflow_path = pack_dir.join("workflows").join("test_workflow.yaml");

        let file = WorkflowFile {
            path: workflow_path,
            pack: "test_pack".to_string(),
            name: "test_workflow".to_string(),
            ref_name: "test_pack.test_workflow".to_string(),
        };

        let config = LoaderConfig {
            packs_base_dir: packs_dir,
            skip_validation: true, // Skip validation for simple test
            max_file_size: 1024 * 1024,
        };

        let loader = WorkflowLoader::new(config);
        let loaded = loader.load_workflow_file(&file).await.unwrap();

        assert_eq!(loaded.workflow.r#ref, "test_pack.test_workflow");
        assert_eq!(loaded.workflow.label, "Test Workflow");
        assert_eq!(
            loaded.workflow.description,
            Some("A test workflow".to_string())
        );
    }

    #[tokio::test]
    async fn test_load_all_workflows() {
        let (_temp_dir, packs_dir) = create_test_pack_structure().await;

        let config = LoaderConfig {
            packs_base_dir: packs_dir,
            skip_validation: true, // Skip validation for simple test
            max_file_size: 1024 * 1024,
        };

        let loader = WorkflowLoader::new(config);
        let workflows = loader.load_all_workflows().await.unwrap();

        assert_eq!(workflows.len(), 1);
        assert!(workflows.contains_key("test_pack.test_workflow"));
    }

    #[tokio::test]
    async fn test_reload_workflow() {
        let (_temp_dir, packs_dir) = create_test_pack_structure().await;

        let config = LoaderConfig {
            packs_base_dir: packs_dir,
            skip_validation: true,
            max_file_size: 1024 * 1024,
        };

        let loader = WorkflowLoader::new(config);
        let loaded = loader
            .reload_workflow("test_pack.test_workflow")
            .await
            .unwrap();

        assert_eq!(loaded.workflow.r#ref, "test_pack.test_workflow");
        assert_eq!(loaded.file.ref_name, "test_pack.test_workflow");
    }

    #[tokio::test]
    async fn test_file_size_limit() {
        let temp_dir = TempDir::new().unwrap();
        let packs_dir = temp_dir.path().to_path_buf();
        let pack_dir = packs_dir.join("test_pack");
        let workflows_dir = pack_dir.join("workflows");
        fs::create_dir_all(&workflows_dir).await.unwrap();

        // Create a large file
        let large_content = "x".repeat(2048);
        let workflow_path = workflows_dir.join("large.yaml");
        fs::write(&workflow_path, large_content).await.unwrap();

        let file = WorkflowFile {
            path: workflow_path,
            pack: "test_pack".to_string(),
            name: "large".to_string(),
            ref_name: "test_pack.large".to_string(),
        };

        let config = LoaderConfig {
            packs_base_dir: packs_dir,
            skip_validation: true,
            max_file_size: 1024, // 1KB limit
        };

        let loader = WorkflowLoader::new(config);
        let result = loader.load_workflow_file(&file).await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("exceeds maximum size"));
    }
}
