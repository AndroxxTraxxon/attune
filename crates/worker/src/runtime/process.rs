//! Process Runtime Implementation
//!
//! A generic, configuration-driven runtime that executes actions as subprocesses.
//! Instead of having separate Rust implementations for each language (Python,
//! Node.js, etc.), this runtime reads its behavior from the database
//! `runtime.execution_config` JSONB column.
//!
//! The execution config describes:
//! - **Interpreter**: which binary to invoke and with what arguments
//! - **Environment**: how to create isolated environments (virtualenv, node_modules)
//! - **Dependencies**: how to detect and install pack dependencies
//!
//! At pack install time, the config drives environment creation and dependency
//! installation. At action execution time, it drives interpreter selection,
//! working directory, and process invocation.

use super::{
    parameter_passing::{self, ParameterDeliveryConfig},
    process_executor, ExecutionContext, ExecutionResult, Runtime, RuntimeError, RuntimeResult,
};
use async_trait::async_trait;
use attune_common::models::runtime::{EnvironmentConfig, RuntimeExecutionConfig};
use std::path::{Path, PathBuf};
use tokio::process::Command;
use tracing::{debug, error, info, warn};

/// A generic runtime driven by `RuntimeExecutionConfig` from the database.
///
/// Each `ProcessRuntime` instance corresponds to a row in the `runtime` table.
/// The worker creates one per registered runtime at startup (loaded from DB).
pub struct ProcessRuntime {
    /// Runtime name (lowercase, used for matching in RuntimeRegistry).
    /// Corresponds to `runtime.name` lowercased (e.g., "python", "shell").
    runtime_name: String,

    /// Execution configuration parsed from `runtime.execution_config` JSONB.
    config: RuntimeExecutionConfig,

    /// Base directory where all packs are stored.
    /// Action file paths are resolved relative to this.
    packs_base_dir: PathBuf,

    /// Base directory for isolated runtime environments (virtualenvs, etc.).
    /// Environments are stored at `{runtime_envs_dir}/{pack_ref}/{runtime_name}`.
    /// This keeps the pack directory clean and read-only.
    runtime_envs_dir: PathBuf,
}

impl ProcessRuntime {
    /// Create a new ProcessRuntime from database configuration.
    ///
    /// # Arguments
    /// * `runtime_name` - Lowercase runtime name (e.g., "python", "shell", "node")
    /// * `config` - Parsed `RuntimeExecutionConfig` from the runtime table
    /// * `packs_base_dir` - Base directory for pack storage
    /// * `runtime_envs_dir` - Base directory for isolated runtime environments
    pub fn new(
        runtime_name: String,
        config: RuntimeExecutionConfig,
        packs_base_dir: PathBuf,
        runtime_envs_dir: PathBuf,
    ) -> Self {
        Self {
            runtime_name,
            config,
            packs_base_dir,
            runtime_envs_dir,
        }
    }

    /// Resolve the pack directory from an action reference.
    ///
    /// Action refs are formatted as `pack_ref.action_name`, so the pack_ref
    /// is everything before the first dot.
    #[allow(dead_code)] // Completes logical API surface; exercised in unit tests
    fn resolve_pack_dir(&self, action_ref: &str) -> PathBuf {
        let pack_ref = action_ref.split('.').next().unwrap_or(action_ref);
        self.packs_base_dir.join(pack_ref)
    }

    /// Extract the pack_ref from an action reference.
    fn extract_pack_ref<'a>(&self, action_ref: &'a str) -> &'a str {
        action_ref.split('.').next().unwrap_or(action_ref)
    }

    /// Compute the external environment directory for a pack.
    ///
    /// Returns `{runtime_envs_dir}/{pack_ref}/{runtime_name}`,
    /// e.g., `/opt/attune/runtime_envs/python_example/python`.
    fn env_dir_for_pack(&self, pack_ref: &str) -> PathBuf {
        self.runtime_envs_dir
            .join(pack_ref)
            .join(&self.runtime_name)
    }

    /// Get the interpreter path, checking for an external pack environment first.
    #[cfg(test)]
    fn resolve_interpreter(&self, pack_dir: &Path, env_dir: Option<&Path>) -> PathBuf {
        self.config.resolve_interpreter_with_env(pack_dir, env_dir)
    }

    /// Set up the runtime environment for a pack at an external location.
    ///
    /// Environments are created at `{runtime_envs_dir}/{pack_ref}/{runtime_name}`
    /// to keep the pack directory clean and read-only.
    ///
    /// # Arguments
    /// * `pack_dir` - Absolute path to the pack directory (for manifest files)
    /// * `env_dir` - Absolute path to the environment directory to create
    pub async fn setup_pack_environment(
        &self,
        pack_dir: &Path,
        env_dir: &Path,
    ) -> RuntimeResult<()> {
        let env_cfg = match &self.config.environment {
            Some(cfg) if cfg.env_type != "none" => cfg,
            _ => {
                debug!(
                    "No environment configuration for runtime '{}', skipping setup",
                    self.runtime_name
                );
                return Ok(());
            }
        };

        let vars = self
            .config
            .build_template_vars_with_env(pack_dir, Some(env_dir));

        if !env_dir.exists() {
            // Environment does not exist yet — create it.
            self.create_environment(env_cfg, pack_dir, env_dir, &vars)
                .await?;
        } else {
            // Environment directory exists — verify the interpreter is usable.
            // A venv created by a different container may contain broken symlinks
            // (e.g. python3 -> /usr/bin/python3 when this container has it at
            // /usr/local/bin/python3).
            if self.env_needs_recreate(env_cfg, pack_dir, env_dir) {
                if let Err(e) = std::fs::remove_dir_all(env_dir) {
                    warn!(
                        "Failed to remove broken environment at {}: {}. Skipping recreate.",
                        env_dir.display(),
                        e,
                    );
                    // Still try to install dependencies even if we couldn't recreate
                    self.install_dependencies(pack_dir, env_dir).await?;
                    return Ok(());
                }

                self.create_environment(env_cfg, pack_dir, env_dir, &vars)
                    .await?;
            }
        }

        // Install dependencies if configured and manifest file exists
        self.install_dependencies(pack_dir, env_dir).await?;

        Ok(())
    }

    /// Check whether an existing environment directory has a broken or missing
    /// interpreter and needs to be recreated.
    ///
    /// Returns `true` if the environment should be deleted and recreated.
    fn env_needs_recreate(
        &self,
        env_cfg: &EnvironmentConfig,
        pack_dir: &Path,
        env_dir: &Path,
    ) -> bool {
        let interp_template = match env_cfg.interpreter_path {
            Some(ref t) => t,
            None => {
                debug!(
                    "Environment already exists at {}, skipping creation \
                     (no interpreter_path to verify)",
                    env_dir.display()
                );
                return false;
            }
        };

        let mut check_vars = std::collections::HashMap::new();
        check_vars.insert("env_dir", env_dir.to_string_lossy().to_string());
        check_vars.insert("pack_dir", pack_dir.to_string_lossy().to_string());
        let resolved = RuntimeExecutionConfig::resolve_template(interp_template, &check_vars);
        let resolved_path = std::path::PathBuf::from(&resolved);

        if resolved_path.exists() {
            debug!(
                "Environment already exists at {} with valid interpreter at {}",
                env_dir.display(),
                resolved_path.display(),
            );
            return false;
        }

        // Interpreter not reachable — distinguish broken symlinks for diagnostics
        let is_broken_symlink = std::fs::symlink_metadata(&resolved_path)
            .map(|m| m.file_type().is_symlink())
            .unwrap_or(false);

        if is_broken_symlink {
            let target = std::fs::read_link(&resolved_path)
                .map(|t| t.display().to_string())
                .unwrap_or_else(|_| "<unreadable>".to_string());
            warn!(
                "Environment at {} has broken interpreter symlink: '{}' -> '{}'. \
                 Removing and recreating...",
                env_dir.display(),
                resolved_path.display(),
                target,
            );
        } else {
            warn!(
                "Environment at {} exists but interpreter not found at '{}'. \
                 Removing and recreating...",
                env_dir.display(),
                resolved_path.display(),
            );
        }
        true
    }

    /// Run the environment create_command to produce a new environment at `env_dir`.
    ///
    /// Ensures parent directories exist, resolves the create command template,
    /// executes it, and logs the result.
    async fn create_environment(
        &self,
        env_cfg: &EnvironmentConfig,
        pack_dir: &Path,
        env_dir: &Path,
        vars: &std::collections::HashMap<&str, String>,
    ) -> RuntimeResult<()> {
        if env_cfg.create_command.is_empty() {
            return Err(RuntimeError::SetupError(format!(
                "Environment type '{}' requires a create_command but none configured",
                env_cfg.env_type
            )));
        }

        // Ensure parent directories exist
        if let Some(parent) = env_dir.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|e| {
                RuntimeError::SetupError(format!(
                    "Failed to create environment parent directory {}: {}",
                    parent.display(),
                    e
                ))
            })?;
        }

        let resolved_cmd = RuntimeExecutionConfig::resolve_command(&env_cfg.create_command, vars);
        info!(
            "Creating {} environment at {}: {:?}",
            env_cfg.env_type,
            env_dir.display(),
            resolved_cmd
        );

        let (program, args) = resolved_cmd
            .split_first()
            .ok_or_else(|| RuntimeError::SetupError("Empty create_command".to_string()))?;

        let output = Command::new(program)
            .args(args)
            .current_dir(pack_dir)
            .output()
            .await
            .map_err(|e| {
                RuntimeError::SetupError(format!(
                    "Failed to run environment create command '{}': {}",
                    program, e
                ))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(RuntimeError::SetupError(format!(
                "Environment creation failed (exit {}): {}",
                output.status.code().unwrap_or(-1),
                stderr.trim()
            )));
        }

        info!(
            "Created {} environment at {}",
            env_cfg.env_type,
            env_dir.display()
        );

        Ok(())
    }

    /// Install dependencies for a pack if a manifest file is present.
    ///
    /// Reads the dependency configuration from `execution_config.dependencies`
    /// and runs the install command if the manifest file (e.g., requirements.txt)
    /// exists in the pack directory.
    ///
    /// # Arguments
    /// * `pack_dir` - Absolute path to the pack directory (for manifest files)
    /// * `env_dir` - Absolute path to the environment directory
    pub async fn install_dependencies(&self, pack_dir: &Path, env_dir: &Path) -> RuntimeResult<()> {
        let dep_cfg = match &self.config.dependencies {
            Some(cfg) => cfg,
            None => {
                debug!(
                    "No dependency configuration for runtime '{}', skipping",
                    self.runtime_name
                );
                return Ok(());
            }
        };

        let manifest_path = pack_dir.join(&dep_cfg.manifest_file);
        if !manifest_path.exists() {
            debug!(
                "No dependency manifest '{}' found in {}, skipping installation",
                dep_cfg.manifest_file,
                pack_dir.display()
            );
            return Ok(());
        }

        if dep_cfg.install_command.is_empty() {
            warn!(
                "Dependency manifest '{}' found but no install_command configured for runtime '{}'",
                dep_cfg.manifest_file, self.runtime_name
            );
            return Ok(());
        }

        // Check whether dependencies have already been installed for the current
        // manifest content. We store a SHA-256 checksum of the manifest file in a
        // marker file inside env_dir. If the checksum matches, we skip the
        // (potentially expensive) install command.
        let marker_path = env_dir.join(".attune_deps_installed");
        let current_checksum = Self::file_checksum(&manifest_path).await;

        if let Some(ref checksum) = current_checksum {
            if let Ok(stored) = tokio::fs::read_to_string(&marker_path).await {
                if stored.trim() == checksum.as_str() {
                    debug!(
                        "Dependencies already installed for runtime '{}' in {} (manifest unchanged)",
                        self.runtime_name,
                        env_dir.display(),
                    );
                    return Ok(());
                }
            }
        }

        // Build template vars with the external env_dir
        let vars = self
            .config
            .build_template_vars_with_env(pack_dir, Some(env_dir));
        let resolved_cmd = RuntimeExecutionConfig::resolve_command(&dep_cfg.install_command, &vars);

        info!(
            "Installing dependencies for pack at {} using: {:?}",
            pack_dir.display(),
            resolved_cmd
        );

        let (program, args) = resolved_cmd
            .split_first()
            .ok_or_else(|| RuntimeError::SetupError("Empty install_command".to_string()))?;

        let output = Command::new(program)
            .args(args)
            .current_dir(pack_dir)
            .output()
            .await
            .map_err(|e| {
                RuntimeError::SetupError(format!(
                    "Failed to run dependency install command '{}': {}",
                    program, e
                ))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(RuntimeError::SetupError(format!(
                "Dependency installation failed (exit {}): {}",
                output.status.code().unwrap_or(-1),
                stderr.trim()
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        info!(
            "Dependencies installed successfully for runtime '{}' in {}",
            self.runtime_name,
            env_dir.display()
        );
        debug!("Install output: {}", stdout.trim());

        // Write the checksum marker so subsequent calls skip the install.
        if let Some(checksum) = current_checksum {
            if let Err(e) = tokio::fs::write(&marker_path, checksum.as_bytes()).await {
                warn!(
                    "Failed to write dependency marker file {}: {}",
                    marker_path.display(),
                    e
                );
            }
        }

        Ok(())
    }

    /// Compute a hex-encoded SHA-256 checksum of a file's contents.
    /// Returns `None` if the file cannot be read.
    async fn file_checksum(path: &Path) -> Option<String> {
        use sha2::{Digest, Sha256};
        let data = tokio::fs::read(path).await.ok()?;
        let hash = Sha256::digest(&data);
        Some(format!("{:x}", hash))
    }

    /// Check whether a pack has dependencies that need to be installed.
    pub fn pack_has_dependencies(&self, pack_dir: &Path) -> bool {
        self.config.has_dependencies(pack_dir)
    }

    /// Check whether the environment for a pack exists at the external location.
    pub fn environment_exists(&self, pack_ref: &str) -> bool {
        let env_dir = self.env_dir_for_pack(pack_ref);
        env_dir.exists()
    }

    /// Get a reference to the execution config.
    pub fn config(&self) -> &RuntimeExecutionConfig {
        &self.config
    }
}

#[async_trait]
impl Runtime for ProcessRuntime {
    fn name(&self) -> &str {
        &self.runtime_name
    }

    fn can_execute(&self, context: &ExecutionContext) -> bool {
        // Match by runtime_name if specified in the context.
        // When an explicit runtime_name is provided, it is authoritative —
        // we only match if the name matches; we do NOT fall through to
        // extension-based matching because the caller has already decided
        // which runtime should handle this action.
        if let Some(ref name) = context.runtime_name {
            return name.eq_ignore_ascii_case(&self.runtime_name);
        }

        // No runtime_name specified — fall back to file extension matching
        if let Some(ref code_path) = context.code_path {
            if self.config.matches_file_extension(code_path) {
                return true;
            }
        }

        // Check entry_point extension
        if self
            .config
            .matches_file_extension(Path::new(&context.entry_point))
        {
            return true;
        }

        false
    }

    async fn execute(&self, context: ExecutionContext) -> RuntimeResult<ExecutionResult> {
        // Determine the effective execution config: use the version-specific
        // override if the executor resolved a specific runtime version for this
        // action, otherwise fall back to this ProcessRuntime's built-in config.
        let effective_config: &RuntimeExecutionConfig = context
            .runtime_config_override
            .as_ref()
            .unwrap_or(&self.config);

        if let Some(ref ver) = context.selected_runtime_version {
            info!(
                "Executing action '{}' (execution_id: {}) with runtime '{}' version {}, \
                 parameter delivery: {:?}, format: {:?}, output format: {:?}",
                context.action_ref,
                context.execution_id,
                self.runtime_name,
                ver,
                context.parameter_delivery,
                context.parameter_format,
                context.output_format,
            );
        } else {
            info!(
                "Executing action '{}' (execution_id: {}) with runtime '{}', \
                 parameter delivery: {:?}, format: {:?}, output format: {:?}",
                context.action_ref,
                context.execution_id,
                self.runtime_name,
                context.parameter_delivery,
                context.parameter_format,
                context.output_format,
            );
        }

        let pack_ref = self.extract_pack_ref(&context.action_ref);
        let pack_dir = self.packs_base_dir.join(pack_ref);

        // Compute external env_dir for this pack/runtime combination.
        // When a specific runtime version is selected, the env dir includes a
        // version suffix (e.g., "python-3.12") for per-version isolation.
        // Pattern: {runtime_envs_dir}/{pack_ref}/{runtime_name[-version]}
        let env_dir = if let Some(ref suffix) = context.runtime_env_dir_suffix {
            self.runtime_envs_dir.join(pack_ref).join(suffix)
        } else {
            self.env_dir_for_pack(pack_ref)
        };
        let env_dir_opt = if effective_config.environment.is_some() {
            Some(env_dir.as_path())
        } else {
            None
        };

        // Runtime environments are set up proactively — either at worker startup
        // (scanning all registered packs) or via pack.registered MQ events when a
        // new pack is installed. We only log a warning here if the expected
        // environment directory is missing so operators can investigate.
        if effective_config.environment.is_some() && pack_dir.exists() && !env_dir.exists() {
            warn!(
                "Runtime environment for pack '{}' not found at {}. \
                 The environment should have been created at startup or on pack registration. \
                 Proceeding with system interpreter as fallback.",
                context.action_ref,
                env_dir.display(),
            );
        }

        // If the environment directory exists but contains a broken interpreter
        // (e.g. broken symlinks from a venv created in a different container),
        // attempt to recreate it before resolving the interpreter.
        if effective_config.environment.is_some() && env_dir.exists() && pack_dir.exists() {
            if let Some(ref env_cfg) = effective_config.environment {
                if let Some(ref interp_template) = env_cfg.interpreter_path {
                    let mut vars = std::collections::HashMap::new();
                    vars.insert("env_dir", env_dir.to_string_lossy().to_string());
                    vars.insert("pack_dir", pack_dir.to_string_lossy().to_string());
                    let resolved = RuntimeExecutionConfig::resolve_template(interp_template, &vars);
                    let resolved_path = std::path::PathBuf::from(&resolved);

                    // Check for a broken symlink: symlink_metadata succeeds for
                    // the link itself even when its target is missing, while
                    // exists() (which follows symlinks) returns false.
                    let is_broken_symlink = !resolved_path.exists()
                        && std::fs::symlink_metadata(&resolved_path)
                            .map(|m| m.file_type().is_symlink())
                            .unwrap_or(false);

                    if is_broken_symlink {
                        let target = std::fs::read_link(&resolved_path)
                            .map(|t| t.display().to_string())
                            .unwrap_or_else(|_| "<unreadable>".to_string());
                        warn!(
                            "Detected broken symlink at '{}' -> '{}' in venv for pack '{}'. \
                             Removing broken environment and recreating...",
                            resolved_path.display(),
                            target,
                            context.action_ref,
                        );

                        // Remove the broken environment directory
                        if let Err(e) = std::fs::remove_dir_all(&env_dir) {
                            warn!(
                                "Failed to remove broken environment at {}: {}. \
                                 Will proceed with system interpreter.",
                                env_dir.display(),
                                e,
                            );
                        } else {
                            // Recreate the environment using a temporary ProcessRuntime
                            // with the effective (possibly version-specific) config.
                            let setup_runtime = ProcessRuntime::new(
                                self.runtime_name.clone(),
                                effective_config.clone(),
                                self.packs_base_dir.clone(),
                                self.runtime_envs_dir.clone(),
                            );
                            match setup_runtime
                                .setup_pack_environment(&pack_dir, &env_dir)
                                .await
                            {
                                Ok(()) => {
                                    info!(
                                        "Successfully recreated environment for pack '{}' at {}",
                                        context.action_ref,
                                        env_dir.display(),
                                    );
                                }
                                Err(e) => {
                                    warn!(
                                        "Failed to recreate environment for pack '{}' at {}: {}. \
                                         Will proceed with system interpreter.",
                                        context.action_ref,
                                        env_dir.display(),
                                        e,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }

        let interpreter = effective_config.resolve_interpreter_with_env(&pack_dir, env_dir_opt);

        info!(
            "Resolved interpreter: {} (env_dir: {}, env_exists: {}, pack_dir: {}, version: {})",
            interpreter.display(),
            env_dir.display(),
            env_dir.exists(),
            pack_dir.display(),
            context
                .selected_runtime_version
                .as_deref()
                .unwrap_or("default"),
        );

        // Prepare environment and parameters according to delivery method
        let mut env = context.env.clone();

        // Inject runtime-specific environment variables from execution_config.
        // These are template-based (e.g., NODE_PATH={env_dir}/node_modules) and
        // resolved against the current pack/env directories.
        if !effective_config.env_vars.is_empty() {
            let vars = effective_config.build_template_vars_with_env(&pack_dir, env_dir_opt);
            for (key, value_template) in &effective_config.env_vars {
                let resolved = RuntimeExecutionConfig::resolve_template(value_template, &vars);
                debug!(
                    "Setting runtime env var: {}={} (template: {})",
                    key, resolved, value_template
                );
                env.insert(key.clone(), resolved);
            }
        }
        let param_config = ParameterDeliveryConfig {
            delivery: context.parameter_delivery,
            format: context.parameter_format,
        };
        let prepared_params =
            parameter_passing::prepare_parameters(&context.parameters, &mut env, param_config)?;
        let parameters_stdin = prepared_params.stdin_content();

        // Determine working directory: use context override, or pack dir
        let working_dir = context
            .working_dir
            .as_deref()
            .filter(|p| p.exists())
            .or_else(|| {
                if pack_dir.exists() {
                    Some(pack_dir.as_path())
                } else {
                    None
                }
            });

        // Build the command based on whether we have a file or inline code
        let cmd = if let Some(ref code_path) = context.code_path {
            // File-based execution: interpreter [args] <action_file>
            debug!("Executing file: {}", code_path.display());
            process_executor::build_action_command(
                &interpreter,
                &effective_config.interpreter.args,
                code_path,
                working_dir,
                &env,
            )
        } else if let Some(ref code) = context.code {
            // Inline code execution: interpreter -c <code>
            debug!("Executing inline code ({} bytes)", code.len());
            let mut cmd = process_executor::build_inline_command(&interpreter, code, &env);
            if let Some(dir) = working_dir {
                cmd.current_dir(dir);
            }
            cmd
        } else {
            // No code_path and no inline code — try treating entry_point as a file
            // relative to the pack's actions directory
            let action_file = pack_dir.join("actions").join(&context.entry_point);
            if action_file.exists() {
                debug!("Executing action file: {}", action_file.display());
                process_executor::build_action_command(
                    &interpreter,
                    &effective_config.interpreter.args,
                    &action_file,
                    working_dir,
                    &env,
                )
            } else {
                error!(
                    "No code, code_path, or action file found for action '{}'. \
                     Tried: {}",
                    context.action_ref,
                    action_file.display()
                );
                return Err(RuntimeError::InvalidAction(format!(
                    "No executable content found for action '{}'. \
                     Expected file at: {}",
                    context.action_ref,
                    action_file.display()
                )));
            }
        };

        // Log the full command about to be executed
        info!(
            "Running command: {:?} (action: '{}', execution_id: {}, working_dir: {:?})",
            cmd,
            context.action_ref,
            context.execution_id,
            working_dir
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| "<none>".to_string()),
        );

        // Execute with streaming output capture
        process_executor::execute_streaming(
            cmd,
            &context.secrets,
            parameters_stdin,
            context.timeout,
            context.max_stdout_bytes,
            context.max_stderr_bytes,
            context.output_format,
        )
        .await
    }

    async fn setup(&self) -> RuntimeResult<()> {
        info!("Setting up ProcessRuntime '{}'", self.runtime_name);

        let binary = &self.config.interpreter.binary;

        // Verify the interpreter is available on the system
        let result = Command::new(binary).arg("--version").output().await;

        match result {
            Ok(output) => {
                if output.status.success() {
                    let version = String::from_utf8_lossy(&output.stdout);
                    let stderr_version = String::from_utf8_lossy(&output.stderr);
                    // Some interpreters print version to stderr (e.g., Python on some systems)
                    let version_str = if version.trim().is_empty() {
                        stderr_version.trim().to_string()
                    } else {
                        version.trim().to_string()
                    };
                    info!(
                        "ProcessRuntime '{}' ready: {} ({})",
                        self.runtime_name, binary, version_str
                    );
                } else {
                    warn!(
                        "Interpreter '{}' for runtime '{}' returned non-zero exit code \
                         on --version check (may still work for execution)",
                        binary, self.runtime_name
                    );
                }
            }
            Err(e) => {
                // The interpreter isn't available — this is a warning, not a hard failure,
                // because the runtime might only be used in containers where the interpreter
                // is available at execution time.
                warn!(
                    "Interpreter '{}' for runtime '{}' not found: {}. \
                     Actions using this runtime may fail.",
                    binary, self.runtime_name, e
                );
            }
        }

        Ok(())
    }

    async fn cleanup(&self) -> RuntimeResult<()> {
        info!("Cleaning up ProcessRuntime '{}'", self.runtime_name);
        Ok(())
    }

    async fn validate(&self) -> RuntimeResult<()> {
        debug!("Validating ProcessRuntime '{}'", self.runtime_name);

        let binary = &self.config.interpreter.binary;

        // Check if interpreter is available
        let output = Command::new(binary).arg("--version").output().await;

        match output {
            Ok(output) if output.status.success() => Ok(()),
            Ok(output) => {
                // Non-zero exit but binary exists — warn but don't fail
                warn!(
                    "Interpreter '{}' returned exit code {} on validation",
                    binary,
                    output.status.code().unwrap_or(-1)
                );
                Ok(())
            }
            Err(e) => {
                warn!(
                    "Interpreter '{}' for runtime '{}' not available: {}",
                    binary, self.runtime_name, e
                );
                // Don't fail validation — the interpreter might be available in containers
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use attune_common::models::runtime::{
        DependencyConfig, EnvironmentConfig, InterpreterConfig, RuntimeExecutionConfig,
    };
    use attune_common::models::{OutputFormat, ParameterDelivery, ParameterFormat};
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn make_shell_config() -> RuntimeExecutionConfig {
        RuntimeExecutionConfig {
            interpreter: InterpreterConfig {
                binary: "/bin/bash".to_string(),
                args: vec![],
                file_extension: Some(".sh".to_string()),
            },
            environment: None,
            dependencies: None,
            env_vars: HashMap::new(),
        }
    }

    fn make_python_config() -> RuntimeExecutionConfig {
        RuntimeExecutionConfig {
            interpreter: InterpreterConfig {
                binary: "python3".to_string(),
                args: vec!["-u".to_string()],
                file_extension: Some(".py".to_string()),
            },
            environment: Some(EnvironmentConfig {
                env_type: "virtualenv".to_string(),
                dir_name: ".venv".to_string(),
                create_command: vec![
                    "python3".to_string(),
                    "-m".to_string(),
                    "venv".to_string(),
                    "{env_dir}".to_string(),
                ],
                interpreter_path: Some("{env_dir}/bin/python3".to_string()),
            }),
            dependencies: Some(DependencyConfig {
                manifest_file: "requirements.txt".to_string(),
                install_command: vec![
                    "{interpreter}".to_string(),
                    "-m".to_string(),
                    "pip".to_string(),
                    "install".to_string(),
                    "-r".to_string(),
                    "{manifest_path}".to_string(),
                ],
            }),
            env_vars: HashMap::new(),
        }
    }

    #[test]
    fn test_can_execute_by_runtime_name() {
        let runtime = ProcessRuntime::new(
            "python".to_string(),
            make_python_config(),
            PathBuf::from("/tmp/packs"),
            PathBuf::from("/tmp/runtime_envs"),
        );

        let context = ExecutionContext {
            execution_id: 1,
            action_ref: "mypack.hello".to_string(),
            parameters: HashMap::new(),
            env: HashMap::new(),
            secrets: HashMap::new(),
            timeout: Some(10),
            working_dir: None,
            entry_point: "hello.py".to_string(),
            code: None,
            code_path: None,
            runtime_name: Some("python".to_string()),
            runtime_config_override: None,
            runtime_env_dir_suffix: None,
            selected_runtime_version: None,
            max_stdout_bytes: 1024,
            max_stderr_bytes: 1024,
            parameter_delivery: ParameterDelivery::default(),
            parameter_format: ParameterFormat::default(),
            output_format: OutputFormat::default(),
        };

        assert!(runtime.can_execute(&context));
    }

    #[test]
    fn test_can_execute_by_file_extension() {
        let runtime = ProcessRuntime::new(
            "python".to_string(),
            make_python_config(),
            PathBuf::from("/tmp/packs"),
            PathBuf::from("/tmp/runtime_envs"),
        );

        let context = ExecutionContext {
            execution_id: 1,
            action_ref: "mypack.hello".to_string(),
            parameters: HashMap::new(),
            env: HashMap::new(),
            secrets: HashMap::new(),
            timeout: Some(10),
            working_dir: None,
            entry_point: "hello.py".to_string(),
            code: None,
            code_path: Some(PathBuf::from("/tmp/packs/mypack/actions/hello.py")),
            runtime_name: None,
            runtime_config_override: None,
            runtime_env_dir_suffix: None,
            selected_runtime_version: None,
            max_stdout_bytes: 1024,
            max_stderr_bytes: 1024,
            parameter_delivery: ParameterDelivery::default(),
            parameter_format: ParameterFormat::default(),
            output_format: OutputFormat::default(),
        };

        assert!(runtime.can_execute(&context));
    }

    #[test]
    fn test_cannot_execute_wrong_extension() {
        let runtime = ProcessRuntime::new(
            "python".to_string(),
            make_python_config(),
            PathBuf::from("/tmp/packs"),
            PathBuf::from("/tmp/runtime_envs"),
        );

        let context = ExecutionContext {
            execution_id: 1,
            action_ref: "mypack.hello".to_string(),
            parameters: HashMap::new(),
            env: HashMap::new(),
            secrets: HashMap::new(),
            timeout: Some(10),
            working_dir: None,
            entry_point: "hello.sh".to_string(),
            code: None,
            code_path: Some(PathBuf::from("/tmp/packs/mypack/actions/hello.sh")),
            runtime_name: None,
            runtime_config_override: None,
            runtime_env_dir_suffix: None,
            selected_runtime_version: None,
            max_stdout_bytes: 1024,
            max_stderr_bytes: 1024,
            parameter_delivery: ParameterDelivery::default(),
            parameter_format: ParameterFormat::default(),
            output_format: OutputFormat::default(),
        };

        assert!(!runtime.can_execute(&context));
    }

    #[test]
    fn test_resolve_pack_dir() {
        let runtime = ProcessRuntime::new(
            "shell".to_string(),
            make_shell_config(),
            PathBuf::from("/opt/attune/packs"),
            PathBuf::from("/opt/attune/runtime_envs"),
        );

        let pack_dir = runtime.resolve_pack_dir("mypack.echo");
        assert_eq!(pack_dir, PathBuf::from("/opt/attune/packs/mypack"));
    }

    #[test]
    fn test_resolve_interpreter_no_env() {
        let runtime = ProcessRuntime::new(
            "shell".to_string(),
            make_shell_config(),
            PathBuf::from("/tmp/packs"),
            PathBuf::from("/tmp/runtime_envs"),
        );

        let interpreter = runtime.resolve_interpreter(Path::new("/tmp/packs/mypack"), None);
        assert_eq!(interpreter, PathBuf::from("/bin/bash"));
    }

    #[test]
    fn test_env_dir_for_pack() {
        let runtime = ProcessRuntime::new(
            "python".to_string(),
            make_python_config(),
            PathBuf::from("/opt/attune/packs"),
            PathBuf::from("/opt/attune/runtime_envs"),
        );

        let env_dir = runtime.env_dir_for_pack("python_example");
        assert_eq!(
            env_dir,
            PathBuf::from("/opt/attune/runtime_envs/python_example/python")
        );
    }

    #[tokio::test]
    async fn test_execute_shell_file() {
        let temp_dir = TempDir::new().unwrap();
        let packs_dir = temp_dir.path().join("packs");
        let pack_dir = packs_dir.join("testpack");
        let actions_dir = pack_dir.join("actions");
        std::fs::create_dir_all(&actions_dir).unwrap();

        // Write a simple shell script
        let script_path = actions_dir.join("hello.sh");
        std::fs::write(
            &script_path,
            "#!/bin/bash\necho 'hello from process runtime'",
        )
        .unwrap();

        let runtime = ProcessRuntime::new(
            "shell".to_string(),
            make_shell_config(),
            packs_dir,
            temp_dir.path().join("runtime_envs"),
        );

        let context = ExecutionContext {
            execution_id: 1,
            action_ref: "testpack.hello".to_string(),
            parameters: HashMap::new(),
            env: HashMap::new(),
            secrets: HashMap::new(),
            timeout: Some(10),
            working_dir: None,
            entry_point: "hello.sh".to_string(),
            code: None,
            code_path: Some(script_path),
            runtime_name: Some("shell".to_string()),
            runtime_config_override: None,
            runtime_env_dir_suffix: None,
            selected_runtime_version: None,
            max_stdout_bytes: 1024 * 1024,
            max_stderr_bytes: 1024 * 1024,
            parameter_delivery: ParameterDelivery::default(),
            parameter_format: ParameterFormat::default(),
            output_format: OutputFormat::default(),
        };

        let result = runtime.execute(context).await.unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("hello from process runtime"));
    }

    #[tokio::test]
    async fn test_execute_python_file() {
        let temp_dir = TempDir::new().unwrap();
        let packs_dir = temp_dir.path().join("packs");
        let pack_dir = packs_dir.join("testpack");
        let actions_dir = pack_dir.join("actions");
        std::fs::create_dir_all(&actions_dir).unwrap();

        // Write a simple Python script
        let script_path = actions_dir.join("hello.py");
        std::fs::write(&script_path, "print('hello from python process runtime')").unwrap();

        let config = RuntimeExecutionConfig {
            interpreter: InterpreterConfig {
                binary: "python3".to_string(),
                args: vec![],
                file_extension: Some(".py".to_string()),
            },
            environment: None,
            dependencies: None,
            env_vars: HashMap::new(),
        };

        let runtime = ProcessRuntime::new(
            "python".to_string(),
            config,
            packs_dir,
            temp_dir.path().join("runtime_envs"),
        );

        let context = ExecutionContext {
            execution_id: 2,
            action_ref: "testpack.hello".to_string(),
            parameters: HashMap::new(),
            env: HashMap::new(),
            secrets: HashMap::new(),
            timeout: Some(10),
            working_dir: None,
            entry_point: "hello.py".to_string(),
            code: None,
            code_path: Some(script_path),
            runtime_name: Some("python".to_string()),
            runtime_config_override: None,
            runtime_env_dir_suffix: None,
            selected_runtime_version: None,
            max_stdout_bytes: 1024 * 1024,
            max_stderr_bytes: 1024 * 1024,
            parameter_delivery: ParameterDelivery::default(),
            parameter_format: ParameterFormat::default(),
            output_format: OutputFormat::default(),
        };

        let result = runtime.execute(context).await.unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("hello from python process runtime"));
    }

    #[tokio::test]
    async fn test_execute_inline_code() {
        let temp_dir = TempDir::new().unwrap();

        let runtime = ProcessRuntime::new(
            "shell".to_string(),
            make_shell_config(),
            temp_dir.path().to_path_buf(),
            temp_dir.path().join("runtime_envs"),
        );

        let context = ExecutionContext {
            execution_id: 3,
            action_ref: "adhoc.test".to_string(),
            parameters: HashMap::new(),
            env: HashMap::new(),
            secrets: HashMap::new(),
            timeout: Some(10),
            working_dir: None,
            entry_point: "inline".to_string(),
            code: Some("echo 'inline shell code'".to_string()),
            code_path: None,
            runtime_name: Some("shell".to_string()),
            runtime_config_override: None,
            runtime_env_dir_suffix: None,
            selected_runtime_version: None,
            max_stdout_bytes: 1024 * 1024,
            max_stderr_bytes: 1024 * 1024,
            parameter_delivery: ParameterDelivery::default(),
            parameter_format: ParameterFormat::default(),
            output_format: OutputFormat::default(),
        };

        let result = runtime.execute(context).await.unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("inline shell code"));
    }

    #[tokio::test]
    async fn test_execute_entry_point_fallback() {
        let temp_dir = TempDir::new().unwrap();
        let packs_dir = temp_dir.path().join("packs");
        let pack_dir = packs_dir.join("testpack");
        let actions_dir = pack_dir.join("actions");
        std::fs::create_dir_all(&actions_dir).unwrap();

        // Write a script at the expected path
        std::fs::write(
            actions_dir.join("greet.sh"),
            "#!/bin/bash\necho 'found via entry_point'",
        )
        .unwrap();

        let runtime = ProcessRuntime::new(
            "shell".to_string(),
            make_shell_config(),
            packs_dir,
            temp_dir.path().join("runtime_envs"),
        );

        // No code_path, no code — should resolve via entry_point
        let context = ExecutionContext {
            execution_id: 4,
            action_ref: "testpack.greet".to_string(),
            parameters: HashMap::new(),
            env: HashMap::new(),
            secrets: HashMap::new(),
            timeout: Some(10),
            working_dir: None,
            entry_point: "greet.sh".to_string(),
            code: None,
            code_path: None,
            runtime_name: Some("shell".to_string()),
            runtime_config_override: None,
            runtime_env_dir_suffix: None,
            selected_runtime_version: None,
            max_stdout_bytes: 1024 * 1024,
            max_stderr_bytes: 1024 * 1024,
            parameter_delivery: ParameterDelivery::default(),
            parameter_format: ParameterFormat::default(),
            output_format: OutputFormat::default(),
        };

        let result = runtime.execute(context).await.unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(result.stdout.contains("found via entry_point"));
    }

    #[tokio::test]
    async fn test_setup_pack_environment_no_config() {
        let temp_dir = TempDir::new().unwrap();
        let pack_dir = temp_dir.path().join("testpack");
        let env_dir = temp_dir
            .path()
            .join("runtime_envs")
            .join("testpack")
            .join("shell");
        std::fs::create_dir_all(&pack_dir).unwrap();

        let runtime = ProcessRuntime::new(
            "shell".to_string(),
            make_shell_config(),
            temp_dir.path().to_path_buf(),
            temp_dir.path().join("runtime_envs"),
        );

        // Should succeed immediately (no environment to create)
        runtime
            .setup_pack_environment(&pack_dir, &env_dir)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_pack_has_dependencies() {
        let temp_dir = TempDir::new().unwrap();
        let pack_dir = temp_dir.path().join("testpack");
        std::fs::create_dir_all(&pack_dir).unwrap();

        let runtime = ProcessRuntime::new(
            "python".to_string(),
            make_python_config(),
            temp_dir.path().to_path_buf(),
            temp_dir.path().join("runtime_envs"),
        );

        // No requirements.txt yet
        assert!(!runtime.pack_has_dependencies(&pack_dir));

        // Create requirements.txt
        std::fs::write(pack_dir.join("requirements.txt"), "requests>=2.28.0\n").unwrap();
        assert!(runtime.pack_has_dependencies(&pack_dir));
    }

    #[tokio::test]
    async fn test_setup_and_validate() {
        let temp_dir = TempDir::new().unwrap();

        let runtime = ProcessRuntime::new(
            "shell".to_string(),
            make_shell_config(),
            temp_dir.path().to_path_buf(),
            temp_dir.path().join("runtime_envs"),
        );

        // Setup and validate should succeed for shell (bash is always available)
        runtime.setup().await.unwrap();
        runtime.validate().await.unwrap();
    }

    #[tokio::test]
    async fn test_working_dir_set_to_pack_dir() {
        let temp_dir = TempDir::new().unwrap();
        let packs_dir = temp_dir.path().join("packs");
        let pack_dir = packs_dir.join("testpack");
        let actions_dir = pack_dir.join("actions");
        std::fs::create_dir_all(&actions_dir).unwrap();

        // Write a script that prints the working directory
        let script_path = actions_dir.join("pwd.sh");
        std::fs::write(&script_path, "#!/bin/bash\npwd").unwrap();

        let runtime = ProcessRuntime::new(
            "shell".to_string(),
            make_shell_config(),
            packs_dir,
            temp_dir.path().join("runtime_envs"),
        );

        let context = ExecutionContext {
            execution_id: 5,
            action_ref: "testpack.pwd".to_string(),
            parameters: HashMap::new(),
            env: HashMap::new(),
            secrets: HashMap::new(),
            timeout: Some(10),
            working_dir: None,
            entry_point: "pwd.sh".to_string(),
            code: None,
            code_path: Some(script_path),
            runtime_name: Some("shell".to_string()),
            runtime_config_override: None,
            runtime_env_dir_suffix: None,
            selected_runtime_version: None,
            max_stdout_bytes: 1024 * 1024,
            max_stderr_bytes: 1024 * 1024,
            parameter_delivery: ParameterDelivery::default(),
            parameter_format: ParameterFormat::default(),
            output_format: OutputFormat::default(),
        };

        let result = runtime.execute(context).await.unwrap();
        assert_eq!(result.exit_code, 0);
        // Working dir should be the pack dir
        let output_path = result.stdout.trim();
        assert_eq!(
            output_path,
            pack_dir.to_string_lossy().as_ref(),
            "Working directory should be set to the pack directory"
        );
    }
}
