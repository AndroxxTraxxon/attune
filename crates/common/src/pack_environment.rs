//! Pack Environment Manager
//!
//! Manages isolated runtime environments for each pack to ensure dependency isolation.
//! Each pack gets its own environment per runtime (e.g., /opt/attune/packenvs/mypack/python/).
//!
//! This prevents dependency conflicts when multiple packs use the same runtime but require
//! different versions of libraries.

use crate::config::Config;
use crate::error::{Error, Result};
use crate::models::Runtime;
use crate::repositories::action::ActionRepository;
use crate::repositories::runtime::RuntimeRepository;
use crate::repositories::FindById as _;
use serde_json::Value as JsonValue;
use sqlx::{PgPool, Row};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::process::Command;
use tokio::fs;
use tracing::{debug, error, info, warn};

/// Status of a pack environment
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackEnvironmentStatus {
    Pending,
    Installing,
    Ready,
    Failed,
    Outdated,
}

impl PackEnvironmentStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Installing => "installing",
            Self::Ready => "ready",
            Self::Failed => "failed",
            Self::Outdated => "outdated",
        }
    }

    pub fn parse_status(s: &str) -> Option<Self> {
        match s {
            "pending" => Some(Self::Pending),
            "installing" => Some(Self::Installing),
            "ready" => Some(Self::Ready),
            "failed" => Some(Self::Failed),
            "outdated" => Some(Self::Outdated),
            _ => None,
        }
    }
}

/// Pack environment record
#[derive(Debug, Clone)]
pub struct PackEnvironment {
    pub id: i64,
    pub pack: i64,
    pub pack_ref: String,
    pub runtime: i64,
    pub runtime_ref: String,
    pub env_path: String,
    pub status: PackEnvironmentStatus,
    pub installed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub last_verified: Option<chrono::DateTime<chrono::Utc>>,
    pub install_log: Option<String>,
    pub install_error: Option<String>,
    pub metadata: JsonValue,
}

/// Installer action definition
#[derive(Debug, Clone)]
pub struct InstallerAction {
    pub name: String,
    pub description: Option<String>,
    pub command: String,
    pub args: Vec<String>,
    pub cwd: Option<String>,
    pub env: HashMap<String, String>,
    pub order: i32,
    pub optional: bool,
    pub condition: Option<JsonValue>,
}

/// Pack environment manager
pub struct PackEnvironmentManager {
    pool: PgPool,
    #[allow(dead_code)] // Used for future path operations
    base_path: PathBuf,
}

impl PackEnvironmentManager {
    /// Create a new pack environment manager
    pub fn new(pool: PgPool, config: &Config) -> Self {
        let base_path = PathBuf::from(&config.packs_base_dir)
            .parent()
            .map(|p| p.join("packenvs"))
            .unwrap_or_else(|| PathBuf::from("/opt/attune/packenvs"));

        Self { pool, base_path }
    }

    /// Create a new pack environment manager with custom base path
    pub fn with_base_path(pool: PgPool, base_path: PathBuf) -> Self {
        Self { pool, base_path }
    }

    /// Create or update a pack environment
    pub async fn ensure_environment(
        &self,
        pack_id: i64,
        pack_ref: &str,
        runtime_id: i64,
        runtime_ref: &str,
        pack_path: &Path,
    ) -> Result<PackEnvironment> {
        info!(
            "Ensuring environment for pack '{}' with runtime '{}'",
            pack_ref, runtime_ref
        );

        // Check if environment already exists
        let existing = self.get_environment(pack_id, runtime_id).await?;

        if let Some(env) = existing {
            if env.status == PackEnvironmentStatus::Ready {
                info!("Environment already exists and is ready: {}", env.env_path);
                return Ok(env);
            } else if env.status == PackEnvironmentStatus::Installing {
                warn!(
                    "Environment is currently installing, returning existing record: {}",
                    env.env_path
                );
                return Ok(env);
            }
            // If failed or outdated, we'll recreate
            info!("Existing environment status: {:?}, recreating", env.status);
        }

        // Get runtime metadata
        let runtime = self.get_runtime(runtime_id).await?;

        // Check if this runtime requires an environment
        if !self.runtime_requires_environment(&runtime)? {
            info!(
                "Runtime '{}' does not require a pack-specific environment",
                runtime_ref
            );
            return self
                .create_no_op_environment(pack_id, pack_ref, runtime_id, runtime_ref)
                .await;
        }

        // Calculate environment path
        let env_path = self.calculate_env_path(pack_ref, &runtime)?;

        // Create or update database record
        let pack_env = self
            .upsert_environment_record(pack_id, pack_ref, runtime_id, runtime_ref, &env_path)
            .await?;

        // Install the environment
        self.install_environment(&pack_env, &runtime, pack_path)
            .await?;

        // Fetch updated record
        self.get_environment(pack_id, runtime_id)
            .await?
            .ok_or_else(|| {
                Error::Internal("Environment record not found after installation".to_string())
            })
    }

    /// Get an existing pack environment
    pub async fn get_environment(
        &self,
        pack_id: i64,
        runtime_id: i64,
    ) -> Result<Option<PackEnvironment>> {
        let row = sqlx::query(
            r#"
            SELECT id, pack, pack_ref, runtime, runtime_ref, env_path, status,
                   installed_at, last_verified, install_log, install_error, metadata
            FROM pack_environment
            WHERE pack = $1 AND runtime = $2
            "#,
        )
        .bind(pack_id)
        .bind(runtime_id)
        .fetch_optional(&self.pool)
        .await?;

        if let Some(row) = row {
            let status_str: String = row.try_get("status")?;
            let status = PackEnvironmentStatus::parse_status(&status_str)
                .unwrap_or(PackEnvironmentStatus::Failed);

            Ok(Some(PackEnvironment {
                id: row.try_get("id")?,
                pack: row.try_get("pack")?,
                pack_ref: row.try_get("pack_ref")?,
                runtime: row.try_get("runtime")?,
                runtime_ref: row.try_get("runtime_ref")?,
                env_path: row.try_get("env_path")?,
                status,
                installed_at: row.try_get("installed_at")?,
                last_verified: row.try_get("last_verified")?,
                install_log: row.try_get("install_log")?,
                install_error: row.try_get("install_error")?,
                metadata: row.try_get("metadata")?,
            }))
        } else {
            Ok(None)
        }
    }

    /// Get the executable path for a pack environment
    pub async fn get_executable_path(
        &self,
        pack_id: i64,
        runtime_id: i64,
        executable_name: &str,
    ) -> Result<Option<String>> {
        let env = match self.get_environment(pack_id, runtime_id).await? {
            Some(e) => e,
            None => return Ok(None),
        };

        if env.status != PackEnvironmentStatus::Ready {
            return Ok(None);
        }

        // Get runtime to check executable templates
        let runtime = self.get_runtime(runtime_id).await?;

        let executable_path =
            if let Some(templates) = runtime.installers.get("executable_templates") {
                if let Some(template) = templates.get(executable_name) {
                    if let Some(template_str) = template.as_str() {
                        self.resolve_template(
                            template_str,
                            &env.pack_ref,
                            &env.runtime_ref,
                            &env.env_path,
                            "",
                        )?
                    } else {
                        return Ok(None);
                    }
                } else {
                    return Ok(None);
                }
            } else {
                return Ok(None);
            };

        Ok(Some(executable_path))
    }

    /// Delete a pack environment
    pub async fn delete_environment(&self, pack_id: i64, runtime_id: i64) -> Result<()> {
        let env = match self.get_environment(pack_id, runtime_id).await? {
            Some(e) => e,
            None => {
                debug!(
                    "No environment to delete for pack {} runtime {}",
                    pack_id, runtime_id
                );
                return Ok(());
            }
        };

        info!("Deleting environment: {}", env.env_path);

        // Delete filesystem directory
        let env_path = PathBuf::from(&env.env_path);
        if env_path.exists() {
            fs::remove_dir_all(&env_path).await.map_err(|e| {
                Error::Internal(format!("Failed to delete environment directory: {}", e))
            })?;
            info!("Deleted environment directory: {}", env.env_path);
        }

        // Delete database record
        sqlx::query("DELETE FROM pack_environment WHERE id = $1")
            .bind(env.id)
            .execute(&self.pool)
            .await?;

        info!(
            "Deleted environment record for pack {} runtime {}",
            pack_id, runtime_id
        );

        Ok(())
    }

    /// Verify an environment is still functional
    pub async fn verify_environment(&self, pack_id: i64, runtime_id: i64) -> Result<bool> {
        let env = match self.get_environment(pack_id, runtime_id).await? {
            Some(e) => e,
            None => return Ok(false),
        };

        if env.status != PackEnvironmentStatus::Ready {
            return Ok(false);
        }

        // Check if directory exists
        let env_path = PathBuf::from(&env.env_path);
        if !env_path.exists() {
            warn!("Environment path does not exist: {}", env.env_path);
            self.mark_environment_outdated(env.id).await?;
            return Ok(false);
        }

        // Update last_verified timestamp
        sqlx::query("UPDATE pack_environment SET last_verified = NOW() WHERE id = $1")
            .bind(env.id)
            .execute(&self.pool)
            .await?;

        Ok(true)
    }

    /// List all environments for a pack
    pub async fn list_pack_environments(&self, pack_id: i64) -> Result<Vec<PackEnvironment>> {
        let rows = sqlx::query(
            r#"
            SELECT id, pack, pack_ref, runtime, runtime_ref, env_path, status,
                   installed_at, last_verified, install_log, install_error, metadata
            FROM pack_environment
            WHERE pack = $1
            ORDER BY runtime_ref
            "#,
        )
        .bind(pack_id)
        .fetch_all(&self.pool)
        .await?;

        let mut environments = Vec::new();
        for row in rows {
            let status_str: String = row.try_get("status")?;
            let status = PackEnvironmentStatus::parse_status(&status_str)
                .unwrap_or(PackEnvironmentStatus::Failed);

            environments.push(PackEnvironment {
                id: row.try_get("id")?,
                pack: row.try_get("pack")?,
                pack_ref: row.try_get("pack_ref")?,
                runtime: row.try_get("runtime")?,
                runtime_ref: row.try_get("runtime_ref")?,
                env_path: row.try_get("env_path")?,
                status,
                installed_at: row.try_get("installed_at")?,
                last_verified: row.try_get("last_verified")?,
                install_log: row.try_get("install_log")?,
                install_error: row.try_get("install_error")?,
                metadata: row.try_get("metadata")?,
            });
        }

        Ok(environments)
    }

    // ========================================================================
    // Private helper methods
    // ========================================================================

    async fn get_runtime(&self, runtime_id: i64) -> Result<Runtime> {
        sqlx::query_as::<_, Runtime>(
            r#"
            SELECT id, ref, pack, pack_ref, description, name,
                   distributions, installation, installers, execution_config,
                   created, updated
            FROM runtime
            WHERE id = $1
            "#,
        )
        .bind(runtime_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Error::Internal(format!("Failed to fetch runtime: {}", e)))
    }

    fn runtime_requires_environment(&self, runtime: &Runtime) -> Result<bool> {
        if let Some(requires) = runtime.installers.get("requires_environment") {
            Ok(requires.as_bool().unwrap_or(true))
        } else {
            // Default: if there are installers, environment is required
            if let Some(installers) = runtime.installers.get("installers") {
                if let Some(arr) = installers.as_array() {
                    Ok(!arr.is_empty())
                } else {
                    Ok(false)
                }
            } else {
                Ok(false)
            }
        }
    }

    fn calculate_env_path(&self, pack_ref: &str, runtime: &Runtime) -> Result<PathBuf> {
        let template = runtime
            .installers
            .get("base_path_template")
            .and_then(|v| v.as_str())
            .unwrap_or("/opt/attune/packenvs/{pack_ref}/{runtime_name_lower}");

        let runtime_name_lower = runtime.name.to_lowercase();
        let path_str = template
            .replace("{pack_ref}", pack_ref)
            .replace("{runtime_ref}", &runtime.r#ref)
            .replace("{runtime_name_lower}", &runtime_name_lower);

        Ok(PathBuf::from(path_str))
    }

    async fn upsert_environment_record(
        &self,
        pack_id: i64,
        pack_ref: &str,
        runtime_id: i64,
        runtime_ref: &str,
        env_path: &Path,
    ) -> Result<PackEnvironment> {
        let env_path_str = env_path.to_string_lossy().to_string();

        let row = sqlx::query(
            r#"
            INSERT INTO pack_environment (pack, pack_ref, runtime, runtime_ref, env_path, status)
            VALUES ($1, $2, $3, $4, $5, 'pending')
            ON CONFLICT (pack, runtime)
            DO UPDATE SET
                env_path = EXCLUDED.env_path,
                status = 'pending',
                install_log = NULL,
                install_error = NULL,
                updated = NOW()
            RETURNING id, pack, pack_ref, runtime, runtime_ref, env_path, status,
                      installed_at, last_verified, install_log, install_error, metadata
            "#,
        )
        .bind(pack_id)
        .bind(pack_ref)
        .bind(runtime_id)
        .bind(runtime_ref)
        .bind(&env_path_str)
        .fetch_one(&self.pool)
        .await?;

        let status_str: String = row.try_get("status")?;
        let status = PackEnvironmentStatus::parse_status(&status_str)
            .unwrap_or(PackEnvironmentStatus::Pending);

        Ok(PackEnvironment {
            id: row.try_get("id")?,
            pack: row.try_get("pack")?,
            pack_ref: row.try_get("pack_ref")?,
            runtime: row.try_get("runtime")?,
            runtime_ref: row.try_get("runtime_ref")?,
            env_path: row.try_get("env_path")?,
            status,
            installed_at: row.try_get("installed_at")?,
            last_verified: row.try_get("last_verified")?,
            install_log: row.try_get("install_log")?,
            install_error: row.try_get("install_error")?,
            metadata: row.try_get("metadata")?,
        })
    }

    async fn create_no_op_environment(
        &self,
        pack_id: i64,
        pack_ref: &str,
        runtime_id: i64,
        runtime_ref: &str,
    ) -> Result<PackEnvironment> {
        let row = sqlx::query(
            r#"
            INSERT INTO pack_environment (pack, pack_ref, runtime, runtime_ref, env_path, status, installed_at)
            VALUES ($1, $2, $3, $4, '', 'ready', NOW())
            ON CONFLICT (pack, runtime)
            DO UPDATE SET status = 'ready', installed_at = NOW(), updated = NOW()
            RETURNING id, pack, pack_ref, runtime, runtime_ref, env_path, status,
                      installed_at, last_verified, install_log, install_error, metadata
            "#,
        )
        .bind(pack_id)
        .bind(pack_ref)
        .bind(runtime_id)
        .bind(runtime_ref)
        .fetch_one(&self.pool)
        .await?;

        let status_str: String = row.try_get("status")?;
        let status = PackEnvironmentStatus::parse_status(&status_str)
            .unwrap_or(PackEnvironmentStatus::Ready);

        Ok(PackEnvironment {
            id: row.try_get("id")?,
            pack: row.try_get("pack")?,
            pack_ref: row.try_get("pack_ref")?,
            runtime: row.try_get("runtime")?,
            runtime_ref: row.try_get("runtime_ref")?,
            env_path: row.try_get("env_path")?,
            status,
            installed_at: row.try_get("installed_at")?,
            last_verified: row.try_get("last_verified")?,
            install_log: row.try_get("install_log")?,
            install_error: row.try_get("install_error")?,
            metadata: row.try_get("metadata")?,
        })
    }

    async fn install_environment(
        &self,
        pack_env: &PackEnvironment,
        runtime: &Runtime,
        pack_path: &Path,
    ) -> Result<()> {
        info!("Installing environment: {}", pack_env.env_path);

        // Update status to installing
        sqlx::query("UPDATE pack_environment SET status = 'installing' WHERE id = $1")
            .bind(pack_env.id)
            .execute(&self.pool)
            .await?;

        let mut install_log = String::new();

        // Create environment directory
        let env_path = PathBuf::from(&pack_env.env_path);
        if env_path.exists() {
            warn!(
                "Environment directory already exists, removing: {}",
                pack_env.env_path
            );
            fs::remove_dir_all(&env_path).await.map_err(|e| {
                Error::Internal(format!("Failed to remove existing environment: {}", e))
            })?;
        }

        fs::create_dir_all(&env_path).await.map_err(|e| {
            Error::Internal(format!("Failed to create environment directory: {}", e))
        })?;

        install_log.push_str(&format!("Created directory: {}\n", pack_env.env_path));

        // Get installer actions
        let installer_actions = self.parse_installer_actions(
            runtime,
            &pack_env.pack_ref,
            &pack_env.runtime_ref,
            &pack_env.env_path,
            pack_path,
        )?;

        // Execute each installer action in order
        for action in installer_actions {
            info!(
                "Executing installer: {} - {}",
                action.name,
                action.description.as_deref().unwrap_or("")
            );

            // Check condition if present
            if let Some(condition) = &action.condition {
                if !self.evaluate_condition(condition, pack_path)? {
                    info!("Skipping installer '{}': condition not met", action.name);
                    install_log
                        .push_str(&format!("Skipped: {} (condition not met)\n", action.name));
                    continue;
                }
            }

            match self.execute_installer_action(&action).await {
                Ok(output) => {
                    install_log.push_str(&format!("\n=== {} ===\n", action.name));
                    install_log.push_str(&output);
                    install_log.push('\n');
                }
                Err(e) => {
                    let error_msg = format!("Installer '{}' failed: {}", action.name, e);
                    error!("{}", error_msg);
                    install_log.push_str(&format!("\nERROR: {}\n", error_msg));

                    if !action.optional {
                        // Mark as failed
                        sqlx::query(
                            "UPDATE pack_environment SET status = 'failed', install_log = $1, install_error = $2 WHERE id = $3"
                        )
                        .bind(&install_log)
                        .bind(&error_msg)
                        .bind(pack_env.id)
                        .execute(&self.pool)
                        .await?;

                        return Err(Error::Internal(error_msg));
                    } else {
                        warn!("Optional installer '{}' failed, continuing", action.name);
                    }
                }
            }
        }

        // Mark as ready
        sqlx::query(
            "UPDATE pack_environment SET status = 'ready', installed_at = NOW(), last_verified = NOW(), install_log = $1 WHERE id = $2"
        )
        .bind(&install_log)
        .bind(pack_env.id)
        .execute(&self.pool)
        .await?;

        info!("Environment installation complete: {}", pack_env.env_path);

        Ok(())
    }

    fn parse_installer_actions(
        &self,
        runtime: &Runtime,
        pack_ref: &str,
        runtime_ref: &str,
        env_path: &str,
        pack_path: &Path,
    ) -> Result<Vec<InstallerAction>> {
        let installers = runtime
            .installers
            .get("installers")
            .and_then(|v| v.as_array())
            .ok_or_else(|| Error::Internal("No installers found for runtime".to_string()))?;

        let pack_path_str = pack_path.to_string_lossy().to_string();
        let mut actions = Vec::new();

        for installer in installers {
            let name = installer
                .get("name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| Error::Internal("Installer missing 'name' field".to_string()))?
                .to_string();

            let description = installer
                .get("description")
                .and_then(|v| v.as_str())
                .map(String::from);

            let command_template = installer
                .get("command")
                .and_then(|v| v.as_str())
                .ok_or_else(|| {
                    Error::Internal(format!("Installer '{}' missing 'command' field", name))
                })?;

            let command = self.resolve_template(
                command_template,
                pack_ref,
                runtime_ref,
                env_path,
                &pack_path_str,
            )?;

            let args_template = installer
                .get("args")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(String::from)
                        .collect::<Vec<String>>()
                })
                .unwrap_or_default();

            let args = args_template
                .iter()
                .map(|arg| {
                    self.resolve_template(arg, pack_ref, runtime_ref, env_path, &pack_path_str)
                })
                .collect::<Result<Vec<String>>>()?;

            let cwd_template = installer.get("cwd").and_then(|v| v.as_str());
            let cwd = if let Some(cwd_t) = cwd_template {
                Some(self.resolve_template(
                    cwd_t,
                    pack_ref,
                    runtime_ref,
                    env_path,
                    &pack_path_str,
                )?)
            } else {
                None
            };

            let env_map = installer
                .get("env")
                .and_then(|v| v.as_object())
                .map(|obj| {
                    obj.iter()
                        .filter_map(|(k, v)| {
                            v.as_str().and_then(|s| {
                                let resolved = self
                                    .resolve_template(
                                        s,
                                        pack_ref,
                                        runtime_ref,
                                        env_path,
                                        &pack_path_str,
                                    )
                                    .ok()?;
                                Some((k.clone(), resolved))
                            })
                        })
                        .collect::<HashMap<String, String>>()
                })
                .unwrap_or_default();

            let order = installer
                .get("order")
                .and_then(|v| v.as_i64())
                .unwrap_or(999) as i32;
            let optional = installer
                .get("optional")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let condition = installer.get("condition").cloned();

            actions.push(InstallerAction {
                name,
                description,
                command,
                args,
                cwd,
                env: env_map,
                order,
                optional,
                condition,
            });
        }

        // Sort by order
        actions.sort_by_key(|a| a.order);

        Ok(actions)
    }

    fn resolve_template(
        &self,
        template: &str,
        pack_ref: &str,
        runtime_ref: &str,
        env_path: &str,
        pack_path: &str,
    ) -> Result<String> {
        let result = template
            .replace("{env_path}", env_path)
            .replace("{pack_path}", pack_path)
            .replace("{pack_ref}", pack_ref)
            .replace("{runtime_ref}", runtime_ref);

        Ok(result)
    }

    async fn execute_installer_action(&self, action: &InstallerAction) -> Result<String> {
        debug!("Executing: {} {:?}", action.command, action.args);

        let mut cmd = Command::new(&action.command);
        cmd.args(&action.args);

        if let Some(cwd) = &action.cwd {
            cmd.current_dir(cwd);
        }

        for (key, value) in &action.env {
            cmd.env(key, value);
        }

        let output = cmd.output().map_err(|e| {
            Error::Internal(format!(
                "Failed to execute command '{}': {}",
                action.command, e
            ))
        })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let combined = format!("STDOUT:\n{}\nSTDERR:\n{}\n", stdout, stderr);

        if !output.status.success() {
            return Err(Error::Internal(format!(
                "Command failed with exit code {:?}\n{}",
                output.status.code(),
                combined
            )));
        }

        Ok(combined)
    }

    fn evaluate_condition(&self, condition: &JsonValue, pack_path: &Path) -> Result<bool> {
        // Check file_exists condition
        if let Some(file_path_template) = condition.get("file_exists").and_then(|v| v.as_str()) {
            let file_path = file_path_template.replace("{pack_path}", &pack_path.to_string_lossy());
            return Ok(PathBuf::from(file_path).exists());
        }

        // Default: condition is true
        Ok(true)
    }

    async fn mark_environment_outdated(&self, env_id: i64) -> Result<()> {
        sqlx::query("UPDATE pack_environment SET status = 'outdated' WHERE id = $1")
            .bind(env_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

/// Collect the lowercase runtime names that require environment setup for a pack.
///
/// This queries the pack's actions, resolves their runtimes, and returns the names
/// of any runtimes that have environment or dependency configuration. It is used by
/// the API when publishing `PackRegistered` MQ events so that workers know which
/// runtimes to set up without re-querying the database.
pub async fn collect_runtime_names_for_pack(
    db_pool: &PgPool,
    pack_id: i64,
    pack_path: &Path,
) -> Vec<String> {
    let actions = match ActionRepository::find_by_pack(db_pool, pack_id).await {
        Ok(a) => a,
        Err(e) => {
            warn!("Failed to load actions for pack ID {}: {}", pack_id, e);
            return Vec::new();
        }
    };

    let mut seen_runtime_ids = HashSet::new();
    for action in &actions {
        if let Some(runtime_id) = action.runtime {
            seen_runtime_ids.insert(runtime_id);
        }
    }

    let mut runtime_names = Vec::new();
    for runtime_id in seen_runtime_ids {
        match RuntimeRepository::find_by_id(db_pool, runtime_id).await {
            Ok(Some(rt)) => {
                let exec_config = rt.parsed_execution_config();
                if exec_config.environment.is_some() || exec_config.has_dependencies(pack_path) {
                    runtime_names.push(rt.name.to_lowercase());
                }
            }
            Ok(None) => {
                debug!("Runtime ID {} not found, skipping", runtime_id);
            }
            Err(e) => {
                warn!("Failed to load runtime {}: {}", runtime_id, e);
            }
        }
    }

    runtime_names
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_status_conversion() {
        assert_eq!(PackEnvironmentStatus::Ready.as_str(), "ready");
        assert_eq!(
            PackEnvironmentStatus::parse_status("ready"),
            Some(PackEnvironmentStatus::Ready)
        );
        assert_eq!(PackEnvironmentStatus::parse_status("invalid"), None);
    }
}
