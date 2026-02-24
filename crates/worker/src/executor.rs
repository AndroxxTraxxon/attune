//! Action Executor Module
//!
//! Coordinates the execution of actions by managing the runtime,
//! loading action data, preparing execution context, and collecting results.

use attune_common::error::{Error, Result};
use attune_common::models::{runtime::Runtime as RuntimeModel, Action, Execution, ExecutionStatus};
use attune_common::repositories::execution::{ExecutionRepository, UpdateExecutionInput};
use attune_common::repositories::{FindById, Update};
use std::path::PathBuf as StdPathBuf;

use serde_json::Value as JsonValue;
use sqlx::PgPool;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, error, info, warn};

use crate::artifacts::ArtifactManager;
use crate::runtime::{ExecutionContext, ExecutionResult, RuntimeRegistry};
use crate::secrets::SecretManager;

/// Action executor that orchestrates execution flow
pub struct ActionExecutor {
    pool: PgPool,
    runtime_registry: RuntimeRegistry,
    artifact_manager: ArtifactManager,
    secret_manager: SecretManager,
    max_stdout_bytes: usize,
    max_stderr_bytes: usize,
    packs_base_dir: PathBuf,
    api_url: String,
}

impl ActionExecutor {
    /// Create a new action executor
    pub fn new(
        pool: PgPool,
        runtime_registry: RuntimeRegistry,
        artifact_manager: ArtifactManager,
        secret_manager: SecretManager,
        max_stdout_bytes: usize,
        max_stderr_bytes: usize,
        packs_base_dir: PathBuf,
        api_url: String,
    ) -> Self {
        Self {
            pool,
            runtime_registry,
            artifact_manager,
            secret_manager,
            max_stdout_bytes,
            max_stderr_bytes,
            packs_base_dir,
            api_url,
        }
    }

    /// Execute an action for the given execution
    pub async fn execute(&self, execution_id: i64) -> Result<ExecutionResult> {
        info!("Starting execution: {}", execution_id);

        // Update execution status to running
        if let Err(e) = self
            .update_execution_status(execution_id, ExecutionStatus::Running)
            .await
        {
            error!("Failed to update execution status to running: {}", e);
            return Err(e);
        }

        // Load execution from database
        let execution = self.load_execution(execution_id).await?;

        // Load action from database
        let action = self.load_action(&execution).await?;

        // Prepare execution context
        let context = match self.prepare_execution_context(&execution, &action).await {
            Ok(ctx) => ctx,
            Err(e) => {
                error!("Failed to prepare execution context: {}", e);
                self.handle_execution_failure(
                    execution_id,
                    None,
                    Some(&format!("Failed to prepare execution context: {}", e)),
                )
                .await?;
                return Err(e);
            }
        };

        // Execute the action
        // Note: execute_action should rarely return Err - most failures should be
        // captured in ExecutionResult with non-zero exit codes
        let result = match self.execute_action(context).await {
            Ok(result) => result,
            Err(e) => {
                error!("Action execution failed catastrophically: {}", e);
                // This should only happen for unrecoverable errors like runtime not found
                self.handle_execution_failure(
                    execution_id,
                    None,
                    Some(&format!("Action execution failed: {}", e)),
                )
                .await?;
                return Err(e);
            }
        };

        // Store artifacts
        if let Err(e) = self.store_execution_artifacts(execution_id, &result).await {
            warn!("Failed to store artifacts: {}", e);
            // Don't fail the execution just because artifact storage failed
        }

        // Update execution with result
        let is_success = result.is_success();
        debug!(
            "Execution {} result: exit_code={}, error={:?}, is_success={}",
            execution_id, result.exit_code, result.error, is_success
        );

        if is_success {
            self.handle_execution_success(execution_id, &result).await?;
        } else {
            self.handle_execution_failure(execution_id, Some(&result), None)
                .await?;
        }

        info!(
            "Execution {} completed: {}",
            execution_id,
            if result.is_success() {
                "success"
            } else {
                "failed"
            }
        );

        Ok(result)
    }

    /// Load execution from database
    async fn load_execution(&self, execution_id: i64) -> Result<Execution> {
        debug!("Loading execution: {}", execution_id);

        ExecutionRepository::find_by_id(&self.pool, execution_id)
            .await?
            .ok_or_else(|| Error::not_found("Execution", "id", execution_id.to_string()))
    }

    /// Load action from database using execution data
    async fn load_action(&self, execution: &Execution) -> Result<Action> {
        debug!("Loading action: {}", execution.action_ref);

        // Try to load by action ID if available
        if let Some(action_id) = execution.action {
            let action = sqlx::query_as::<_, Action>("SELECT * FROM action WHERE id = $1")
                .bind(action_id)
                .fetch_optional(&self.pool)
                .await?;

            if let Some(action) = action {
                return Ok(action);
            }
        }

        // Fallback: look up by the full qualified action ref directly
        let action = sqlx::query_as::<_, Action>("SELECT * FROM action WHERE ref = $1")
            .bind(&execution.action_ref)
            .fetch_optional(&self.pool)
            .await?;

        if let Some(action) = action {
            return Ok(action);
        }

        // Final fallback: parse action_ref as "pack.action" and query by pack ref
        let parts: Vec<&str> = execution.action_ref.split('.').collect();
        if parts.len() != 2 {
            return Err(Error::validation(format!(
                "Invalid action reference format: {}. Expected format: pack.action",
                execution.action_ref
            )));
        }

        let pack_ref = parts[0];

        // Query action by pack ref and full action ref
        let action = sqlx::query_as::<_, Action>(
            r#"
            SELECT a.*
            FROM action a
            JOIN pack p ON a.pack = p.id
            WHERE p.ref = $1 AND a.ref = $2
            "#,
        )
        .bind(pack_ref)
        .bind(&execution.action_ref)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| Error::not_found("Action", "ref", execution.action_ref.clone()))?;

        Ok(action)
    }

    /// Prepare execution context from execution and action data
    async fn prepare_execution_context(
        &self,
        execution: &Execution,
        action: &Action,
    ) -> Result<ExecutionContext> {
        debug!(
            "Preparing execution context for execution: {}",
            execution.id
        );

        // Extract parameters from execution config
        let mut parameters = HashMap::new();

        if let Some(config) = &execution.config {
            info!("Execution config present: {:?}", config);

            // Try to get parameters from config.parameters first
            if let Some(params) = config.get("parameters") {
                info!("Found config.parameters key");
                if let JsonValue::Object(map) = params {
                    for (key, value) in map {
                        parameters.insert(key.clone(), value.clone());
                    }
                }
            } else if let JsonValue::Object(map) = config {
                info!("No config.parameters key, treating entire config as parameters");
                // If no parameters key, treat entire config as parameters
                // (this handles rule action_params being placed at root level)
                for (key, value) in map {
                    // Skip special keys that aren't action parameters
                    if key != "context" && key != "env" {
                        info!("Adding parameter: {} = {:?}", key, value);
                        parameters.insert(key.clone(), value.clone());
                    } else {
                        info!("Skipping special key: {}", key);
                    }
                }
            } else {
                info!("Config is not an Object, cannot extract parameters");
            }
        } else {
            info!("No execution config present");
        }

        info!(
            "Extracted {} parameters: {:?}",
            parameters.len(),
            parameters
        );

        // Prepare standard environment variables
        let mut env = HashMap::new();

        // Standard execution context variables (see docs/QUICKREF-execution-environment.md)
        env.insert("ATTUNE_EXEC_ID".to_string(), execution.id.to_string());
        env.insert("ATTUNE_ACTION".to_string(), execution.action_ref.clone());
        env.insert("ATTUNE_API_URL".to_string(), self.api_url.clone());

        // TODO: Generate execution-scoped API token
        // For now, set placeholder to maintain interface compatibility
        env.insert("ATTUNE_API_TOKEN".to_string(), "".to_string());

        // Add rule and trigger context if execution was triggered by enforcement
        if let Some(enforcement_id) = execution.enforcement {
            if let Ok(Some(enforcement)) = sqlx::query_as::<
                _,
                attune_common::models::event::Enforcement,
            >("SELECT * FROM enforcement WHERE id = $1")
            .bind(enforcement_id)
            .fetch_optional(&self.pool)
            .await
            {
                env.insert("ATTUNE_RULE".to_string(), enforcement.rule_ref);
                env.insert("ATTUNE_TRIGGER".to_string(), enforcement.trigger_ref);
            }
        }

        // Add context data as environment variables from config
        if let Some(config) = &execution.config {
            if let Some(context) = config.get("context") {
                if let JsonValue::Object(map) = context {
                    for (key, value) in map {
                        let env_key = format!("ATTUNE_CONTEXT_{}", key.to_uppercase());
                        let env_value = match value {
                            JsonValue::String(s) => s.clone(),
                            JsonValue::Number(n) => n.to_string(),
                            JsonValue::Bool(b) => b.to_string(),
                            _ => serde_json::to_string(value)?,
                        };
                        env.insert(env_key, env_value);
                    }
                }
            }
        }

        // Fetch secrets (passed securely via stdin, not environment variables)
        let secrets = match self.secret_manager.fetch_secrets_for_action(action).await {
            Ok(secrets) => {
                debug!(
                    "Fetched {} secrets for action {} (will be passed via stdin)",
                    secrets.len(),
                    action.r#ref
                );
                secrets
            }
            Err(e) => {
                warn!("Failed to fetch secrets for action {}: {}", action.r#ref, e);
                // Don't fail the execution if secrets can't be fetched
                // Some actions may not require secrets
                HashMap::new()
            }
        };

        // Determine entry point from action
        let entry_point = action.entrypoint.clone();

        // Default timeout: 5 minutes (300 seconds)
        // In the future, this could come from action metadata or execution config
        let timeout = Some(300_u64);

        // Load runtime information if specified
        let runtime_record = if let Some(runtime_id) = action.runtime {
            match sqlx::query_as::<_, RuntimeModel>(
                r#"SELECT id, ref, pack, pack_ref, description, name,
                          distributions, installation, installers, execution_config,
                          created, updated
                   FROM runtime WHERE id = $1"#,
            )
            .bind(runtime_id)
            .fetch_optional(&self.pool)
            .await
            {
                Ok(Some(runtime)) => {
                    debug!(
                        "Loaded runtime '{}' (ref: {}) for action '{}'",
                        runtime.name, runtime.r#ref, action.r#ref
                    );
                    Some(runtime)
                }
                Ok(None) => {
                    warn!(
                        "Runtime ID {} not found for action '{}'",
                        runtime_id, action.r#ref
                    );
                    None
                }
                Err(e) => {
                    warn!(
                        "Failed to load runtime {} for action '{}': {}",
                        runtime_id, action.r#ref, e
                    );
                    None
                }
            }
        } else {
            None
        };

        let runtime_name = runtime_record.as_ref().map(|r| r.name.to_lowercase());

        // Determine the pack directory for this action
        let pack_dir = self.packs_base_dir.join(&action.pack_ref);

        // Construct code_path for pack actions
        // Pack actions have their script files in packs/{pack_ref}/actions/{entrypoint}
        let code_path = if action.pack_ref.starts_with("core") || !action.is_adhoc {
            // This is a pack action, construct the file path
            let action_file_path = pack_dir.join("actions").join(&entry_point);

            if action_file_path.exists() {
                Some(action_file_path)
            } else {
                // Detailed diagnostics to help track down missing action files
                let pack_dir_exists = pack_dir.exists();
                let actions_dir = pack_dir.join("actions");
                let actions_dir_exists = actions_dir.exists();
                let actions_dir_contents: Vec<String> = if actions_dir_exists {
                    std::fs::read_dir(&actions_dir)
                        .map(|entries| {
                            entries
                                .filter_map(|e| e.ok())
                                .map(|e| e.file_name().to_string_lossy().to_string())
                                .collect()
                        })
                        .unwrap_or_default()
                } else {
                    vec![]
                };

                warn!(
                    "Action file not found for action '{}': \
                     expected_path={}, \
                     packs_base_dir={}, \
                     pack_ref={}, \
                     entrypoint={}, \
                     pack_dir_exists={}, \
                     actions_dir_exists={}, \
                     actions_dir_contents={:?}",
                    action.r#ref,
                    action_file_path.display(),
                    self.packs_base_dir.display(),
                    action.pack_ref,
                    entry_point,
                    pack_dir_exists,
                    actions_dir_exists,
                    actions_dir_contents,
                );
                None
            }
        } else {
            None // Ad-hoc actions don't have files
        };

        // For shell actions without a file, use the entrypoint as inline code
        let code = if runtime_name.as_deref() == Some("shell") && code_path.is_none() {
            Some(entry_point.clone())
        } else {
            None
        };

        // Resolve the working directory from the runtime's execution_config.
        // The ProcessRuntime also does this internally, but setting it in the
        // context allows the executor to override if needed.
        let working_dir: Option<StdPathBuf> = if pack_dir.exists() {
            Some(pack_dir)
        } else {
            None
        };

        let context = ExecutionContext {
            execution_id: execution.id,
            action_ref: execution.action_ref.clone(),
            parameters,
            env,
            secrets, // Passed securely via stdin
            timeout,
            working_dir,
            entry_point,
            code,
            code_path,
            runtime_name,
            max_stdout_bytes: self.max_stdout_bytes,
            max_stderr_bytes: self.max_stderr_bytes,
            parameter_delivery: action.parameter_delivery,
            parameter_format: action.parameter_format,
            output_format: action.output_format,
        };

        Ok(context)
    }

    /// Execute the action using the runtime registry
    async fn execute_action(&self, context: ExecutionContext) -> Result<ExecutionResult> {
        debug!("Executing action: {}", context.action_ref);

        let runtime = self
            .runtime_registry
            .get_runtime(&context)
            .map_err(|e| Error::Internal(e.to_string()))?;

        let result = runtime
            .execute(context)
            .await
            .map_err(|e| Error::Internal(e.to_string()))?;

        Ok(result)
    }

    /// Store execution artifacts (logs, results)
    async fn store_execution_artifacts(
        &self,
        execution_id: i64,
        result: &ExecutionResult,
    ) -> Result<()> {
        debug!("Storing artifacts for execution: {}", execution_id);

        // Store logs
        self.artifact_manager
            .store_logs(execution_id, &result.stdout, &result.stderr)
            .await?;

        // Store result if available
        if let Some(result_data) = &result.result {
            self.artifact_manager
                .store_result(execution_id, result_data)
                .await?;
        }

        Ok(())
    }

    /// Handle successful execution
    async fn handle_execution_success(
        &self,
        execution_id: i64,
        result: &ExecutionResult,
    ) -> Result<()> {
        info!(
            "Execution {} succeeded (exit_code={}, duration={}ms)",
            execution_id, result.exit_code, result.duration_ms
        );

        // Build comprehensive result with execution metadata
        let exec_dir = self.artifact_manager.get_execution_dir(execution_id);
        let mut result_data = serde_json::json!({
            "exit_code": result.exit_code,
            "duration_ms": result.duration_ms,
            "succeeded": true,
        });

        // Include stdout content directly in result
        if !result.stdout.is_empty() {
            result_data["stdout"] = serde_json::json!(result.stdout);
        }

        // Include stderr log path only if stderr is non-empty and non-whitespace
        if !result.stderr.trim().is_empty() {
            let stderr_path = exec_dir.join("stderr.log");
            result_data["stderr_log"] = serde_json::json!(stderr_path.to_string_lossy());
        }

        // Include parsed result if available
        if let Some(parsed_result) = &result.result {
            result_data["data"] = parsed_result.clone();
        }

        let input = UpdateExecutionInput {
            status: Some(ExecutionStatus::Completed),
            result: Some(result_data),
            executor: None,
            workflow_task: None, // Not updating workflow metadata
        };

        ExecutionRepository::update(&self.pool, execution_id, input).await?;

        Ok(())
    }

    /// Handle failed execution
    async fn handle_execution_failure(
        &self,
        execution_id: i64,
        result: Option<&ExecutionResult>,
        error_message: Option<&str>,
    ) -> Result<()> {
        if let Some(r) = result {
            error!(
                "Execution {} failed (exit_code={}, error={:?}, duration={}ms)",
                execution_id, r.exit_code, r.error, r.duration_ms
            );
        } else {
            error!(
                "Execution {} failed during preparation: {}",
                execution_id,
                error_message.unwrap_or("unknown error")
            );
        }

        let exec_dir = self.artifact_manager.get_execution_dir(execution_id);
        let mut result_data = serde_json::json!({
            "succeeded": false,
        });

        // If we have execution result, include detailed information
        if let Some(exec_result) = result {
            result_data["exit_code"] = serde_json::json!(exec_result.exit_code);
            result_data["duration_ms"] = serde_json::json!(exec_result.duration_ms);

            if let Some(ref error) = exec_result.error {
                result_data["error"] = serde_json::json!(error);
            }

            // Include stdout content directly in result
            if !exec_result.stdout.is_empty() {
                result_data["stdout"] = serde_json::json!(exec_result.stdout);
            }

            // Include stderr log path only if stderr is non-empty and non-whitespace
            if !exec_result.stderr.trim().is_empty() {
                let stderr_path = exec_dir.join("stderr.log");
                result_data["stderr_log"] = serde_json::json!(stderr_path.to_string_lossy());
            }

            // Add truncation warnings if applicable
            if exec_result.stdout_truncated {
                result_data["stdout_truncated"] = serde_json::json!(true);
                result_data["stdout_bytes_truncated"] =
                    serde_json::json!(exec_result.stdout_bytes_truncated);
            }
            if exec_result.stderr_truncated {
                result_data["stderr_truncated"] = serde_json::json!(true);
                result_data["stderr_bytes_truncated"] =
                    serde_json::json!(exec_result.stderr_bytes_truncated);
            }
        } else {
            // No execution result available (early failure during setup/preparation)
            // This should be rare - most errors should be captured in ExecutionResult
            let err_msg = error_message.unwrap_or("Execution failed during preparation");
            result_data["error"] = serde_json::json!(err_msg);

            warn!(
                "Execution {} failed without ExecutionResult - {}: {}",
                execution_id, "early/catastrophic failure", err_msg
            );

            // Check if stderr log exists and is non-empty from artifact storage
            let stderr_path = exec_dir.join("stderr.log");
            if stderr_path.exists() {
                if let Ok(contents) = tokio::fs::read_to_string(&stderr_path).await {
                    if !contents.trim().is_empty() {
                        result_data["stderr_log"] =
                            serde_json::json!(stderr_path.to_string_lossy());
                    }
                }
            }

            // Check if stdout log exists from artifact storage
            let stdout_path = exec_dir.join("stdout.log");
            if stdout_path.exists() {
                if let Ok(contents) = tokio::fs::read_to_string(&stdout_path).await {
                    if !contents.is_empty() {
                        result_data["stdout"] = serde_json::json!(contents);
                    }
                }
            }
        }

        let input = UpdateExecutionInput {
            status: Some(ExecutionStatus::Failed),
            result: Some(result_data),
            executor: None,
            workflow_task: None, // Not updating workflow metadata
        };

        ExecutionRepository::update(&self.pool, execution_id, input).await?;

        Ok(())
    }

    /// Update execution status
    async fn update_execution_status(
        &self,
        execution_id: i64,
        status: ExecutionStatus,
    ) -> Result<()> {
        debug!(
            "Updating execution {} status to: {:?}",
            execution_id, status
        );

        let input = UpdateExecutionInput {
            status: Some(status),
            result: None,
            executor: None,
            workflow_task: None, // Not updating workflow metadata
        };

        ExecutionRepository::update(&self.pool, execution_id, input).await?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_parse_action_reference() {
        let action_ref = "mypack.myaction";
        let parts: Vec<&str> = action_ref.split('.').collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0], "mypack");
        assert_eq!(parts[1], "myaction");
    }

    #[test]
    fn test_invalid_action_reference() {
        let action_ref = "invalid";
        let parts: Vec<&str> = action_ref.split('.').collect();
        assert_eq!(parts.len(), 1);
    }
}
