//! Workflow Loader
//!
//! This module handles loading workflow definitions from YAML files in pack directories.
//! It scans pack directories, parses workflow YAML files, validates them, and prepares
//! them for registration in the database.

use crate::error::{Error, Result};

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
    ///
    /// Scans two directories in order:
    /// 1. `{pack_dir}/workflows/` — legacy/standalone workflow files
    /// 2. `{pack_dir}/actions/workflows/` — visual-builder and action-linked workflow files
    ///
    /// If the same workflow ref appears in both directories, the version from
    /// `actions/workflows/` wins (it is scanned second and overwrites the map entry).
    pub async fn load_pack_workflows(
        &self,
        pack_name: &str,
        pack_dir: &Path,
    ) -> Result<HashMap<String, LoadedWorkflow>> {
        let mut workflows = HashMap::new();

        // Scan both workflow directories
        let scan_dirs: Vec<std::path::PathBuf> = vec![
            pack_dir.join("workflows"),
            pack_dir.join("actions").join("workflows"),
        ];

        for workflows_dir in &scan_dirs {
            if !workflows_dir.exists() {
                continue;
            }

            let workflow_files = self.scan_workflow_files(workflows_dir, pack_name).await?;

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
        }

        if workflows.is_empty() {
            debug!("No workflows found in pack '{}'", pack_name);
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

        if let Some(ref err) = validation_error {
            if !self.config.skip_validation {
                return Err(Error::validation(format!(
                    "Workflow validation failed: {}",
                    err
                )));
            }
        }

        Ok(LoadedWorkflow {
            file: file.clone(),
            workflow,
            validation_error,
        })
    }

    /// Reload a specific workflow by reference
    ///
    /// Searches for the workflow file in both `workflows/` and
    /// `actions/workflows/` directories, trying `.yaml`, `.yml`, and
    /// `.workflow.yaml` extensions.
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

        // Candidate directories and filename patterns to search
        let dirs = [
            pack_dir.join("actions").join("workflows"),
            pack_dir.join("workflows"),
        ];
        let extensions = [
            format!("{}.workflow.yaml", workflow_name),
            format!("{}.yaml", workflow_name),
            format!("{}.workflow.yml", workflow_name),
            format!("{}.yml", workflow_name),
        ];

        for dir in &dirs {
            for filename in &extensions {
                let candidate = dir.join(filename);
                if candidate.exists() {
                    let file = WorkflowFile {
                        path: candidate,
                        pack: pack_name.to_string(),
                        name: workflow_name.to_string(),
                        ref_name: ref_name.to_string(),
                    };
                    return self.load_workflow_file(&file).await;
                }
            }
        }

        Err(Error::not_found("workflow", "ref", ref_name))
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
    ///
    /// Handles both `{name}.yaml` and `{name}.workflow.yaml` naming
    /// conventions. For files with a `.workflow.yaml` suffix (produced by
    /// the visual workflow builder), the `.workflow` portion is stripped
    /// when deriving the workflow name and ref.
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
                        if let Some(raw_stem) = path.file_stem().and_then(|n| n.to_str()) {
                            // Strip `.workflow` suffix if present:
                            //   "deploy.workflow.yaml" -> stem "deploy.workflow" -> name "deploy"
                            //   "deploy.yaml"          -> stem "deploy"          -> name "deploy"
                            let name = raw_stem.strip_suffix(".workflow").unwrap_or(raw_stem);

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

    /// Verify that `scan_workflow_files` strips the `.workflow` suffix from
    /// filenames like `deploy.workflow.yaml`, yielding name `deploy` and
    /// ref `pack.deploy` instead of `pack.deploy.workflow`.
    #[tokio::test]
    async fn test_scan_workflow_files_strips_workflow_suffix() {
        let temp_dir = TempDir::new().unwrap();
        let packs_dir = temp_dir.path().to_path_buf();
        let pack_dir = packs_dir.join("my_pack");
        let workflows_dir = pack_dir.join("actions").join("workflows");
        fs::create_dir_all(&workflows_dir).await.unwrap();

        let workflow_yaml = r#"
ref: my_pack.deploy
label: Deploy
version: "1.0.0"
tasks:
  - name: step1
    action: core.noop
"#;
        fs::write(workflows_dir.join("deploy.workflow.yaml"), workflow_yaml)
            .await
            .unwrap();

        let config = LoaderConfig {
            packs_base_dir: packs_dir,
            skip_validation: true,
            max_file_size: 1024 * 1024,
        };

        let loader = WorkflowLoader::new(config);
        let files = loader
            .scan_workflow_files(&workflows_dir, "my_pack")
            .await
            .unwrap();

        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "deploy");
        assert_eq!(files[0].ref_name, "my_pack.deploy");
    }

    /// Verify that `load_pack_workflows` discovers workflow files in both
    /// `workflows/` (legacy) and `actions/workflows/` (visual builder)
    /// directories, and that `actions/workflows/` wins on ref collision.
    #[tokio::test]
    async fn test_load_pack_workflows_scans_both_directories() {
        let temp_dir = TempDir::new().unwrap();
        let packs_dir = temp_dir.path().to_path_buf();
        let pack_dir = packs_dir.join("dual_pack");

        // Legacy directory: workflows/
        let legacy_dir = pack_dir.join("workflows");
        fs::create_dir_all(&legacy_dir).await.unwrap();

        let legacy_yaml = r#"
ref: dual_pack.alpha
label: Alpha (legacy)
version: "1.0.0"
tasks:
  - name: t1
    action: core.noop
"#;
        fs::write(legacy_dir.join("alpha.yaml"), legacy_yaml)
            .await
            .unwrap();

        // Also put a workflow that only exists in the legacy dir
        let beta_yaml = r#"
ref: dual_pack.beta
label: Beta
version: "1.0.0"
tasks:
  - name: t1
    action: core.noop
"#;
        fs::write(legacy_dir.join("beta.yaml"), beta_yaml)
            .await
            .unwrap();

        // Visual builder directory: actions/workflows/
        let builder_dir = pack_dir.join("actions").join("workflows");
        fs::create_dir_all(&builder_dir).await.unwrap();

        let builder_yaml = r#"
ref: dual_pack.alpha
label: Alpha (builder)
version: "2.0.0"
tasks:
  - name: t1
    action: core.noop
"#;
        fs::write(builder_dir.join("alpha.workflow.yaml"), builder_yaml)
            .await
            .unwrap();

        let config = LoaderConfig {
            packs_base_dir: packs_dir,
            skip_validation: true,
            max_file_size: 1024 * 1024,
        };

        let loader = WorkflowLoader::new(config);
        let workflows = loader
            .load_pack_workflows("dual_pack", &pack_dir)
            .await
            .unwrap();

        // Both alpha and beta should be present
        assert_eq!(workflows.len(), 2);
        assert!(workflows.contains_key("dual_pack.alpha"));
        assert!(workflows.contains_key("dual_pack.beta"));

        // Alpha should come from actions/workflows/ (scanned second, overwrites)
        let alpha = &workflows["dual_pack.alpha"];
        assert_eq!(alpha.workflow.label, "Alpha (builder)");
        assert_eq!(alpha.workflow.version, "2.0.0");

        // Beta only exists in legacy dir
        let beta = &workflows["dual_pack.beta"];
        assert_eq!(beta.workflow.label, "Beta");
    }

    /// Verify that `reload_workflow` finds files in `actions/workflows/`
    /// with the `.workflow.yaml` extension.
    #[tokio::test]
    async fn test_reload_workflow_finds_actions_workflows_dir() {
        let temp_dir = TempDir::new().unwrap();
        let packs_dir = temp_dir.path().to_path_buf();
        let pack_dir = packs_dir.join("rp");
        let builder_dir = pack_dir.join("actions").join("workflows");
        fs::create_dir_all(&builder_dir).await.unwrap();

        let yaml = r#"
ref: rp.deploy
label: Deploy
version: "1.0.0"
tasks:
  - name: step1
    action: core.noop
"#;
        fs::write(builder_dir.join("deploy.workflow.yaml"), yaml)
            .await
            .unwrap();

        let config = LoaderConfig {
            packs_base_dir: packs_dir,
            skip_validation: true,
            max_file_size: 1024 * 1024,
        };

        let loader = WorkflowLoader::new(config);
        let loaded = loader.reload_workflow("rp.deploy").await.unwrap();

        assert_eq!(loaded.workflow.r#ref, "rp.deploy");
        assert_eq!(loaded.file.name, "deploy");
        assert_eq!(loaded.file.ref_name, "rp.deploy");
    }
}
