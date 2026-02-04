//! Runtime Detection Module
//!
//! Provides unified runtime capability detection for both sensor and worker services.
//! Supports three-tier configuration:
//! 1. Environment variable override (highest priority)
//! 2. Config file specification (medium priority)
//! 3. Database-driven detection with verification (lowest priority)

use crate::config::Config;
use crate::error::Result;
use crate::models::Runtime;
use serde_json::json;
use sqlx::PgPool;
use std::collections::HashMap;
use std::process::Command;
use tracing::{debug, info, warn};

/// Runtime detection service
pub struct RuntimeDetector {
    pool: PgPool,
}

impl RuntimeDetector {
    /// Create a new runtime detector
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Detect available runtimes using three-tier priority:
    /// 1. Environment variable (ATTUNE_WORKER_RUNTIMES or ATTUNE_SENSOR_RUNTIMES)
    /// 2. Config file capabilities
    /// 3. Database-driven detection with verification
    ///
    /// Returns a HashMap of capabilities including the "runtimes" key with detected runtime names
    pub async fn detect_capabilities(
        &self,
        _config: &Config,
        env_var_name: &str,
        config_capabilities: Option<&HashMap<String, serde_json::Value>>,
    ) -> Result<HashMap<String, serde_json::Value>> {
        let mut capabilities = HashMap::new();

        // Check environment variable override first (highest priority)
        if let Ok(runtimes_env) = std::env::var(env_var_name) {
            info!(
                "Using runtimes from {} (override): {}",
                env_var_name, runtimes_env
            );
            let runtime_list: Vec<String> = runtimes_env
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            capabilities.insert("runtimes".to_string(), json!(runtime_list));

            // Copy any other capabilities from config
            if let Some(config_caps) = config_capabilities {
                for (key, value) in config_caps.iter() {
                    if key != "runtimes" {
                        capabilities.insert(key.clone(), value.clone());
                    }
                }
            }

            return Ok(capabilities);
        }

        // Check config file (medium priority)
        if let Some(config_caps) = config_capabilities {
            if let Some(config_runtimes) = config_caps.get("runtimes") {
                if let Some(runtime_array) = config_runtimes.as_array() {
                    if !runtime_array.is_empty() {
                        info!("Using runtimes from config file");
                        let runtime_list: Vec<String> = runtime_array
                            .iter()
                            .filter_map(|v| v.as_str().map(|s| s.to_lowercase()))
                            .collect();
                        capabilities.insert("runtimes".to_string(), json!(runtime_list));

                        // Copy other capabilities from config
                        for (key, value) in config_caps.iter() {
                            if key != "runtimes" {
                                capabilities.insert(key.clone(), value.clone());
                            }
                        }

                        return Ok(capabilities);
                    }
                }
            }

            // Copy non-runtime capabilities from config
            for (key, value) in config_caps.iter() {
                if key != "runtimes" {
                    capabilities.insert(key.clone(), value.clone());
                }
            }
        }

        // Database-driven detection (lowest priority)
        info!("No runtime override found, detecting from database...");
        let detected_runtimes = self.detect_from_database().await?;
        capabilities.insert("runtimes".to_string(), json!(detected_runtimes));

        Ok(capabilities)
    }

    /// Detect available runtimes by querying database and verifying each runtime
    pub async fn detect_from_database(&self) -> Result<Vec<String>> {
        info!("Querying database for runtime definitions...");

        // Query all runtimes from database (no longer filtered by type)
        let runtimes = sqlx::query_as::<_, Runtime>(
            r#"
            SELECT id, ref, pack, pack_ref, description, name,
                   distributions, installation, installers, created, updated
            FROM runtime
            WHERE ref NOT LIKE '%.sensor.builtin'
            ORDER BY ref
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        info!("Found {} runtime(s) in database", runtimes.len());

        let mut available_runtimes = Vec::new();

        // Verify each runtime
        for runtime in runtimes {
            if Self::verify_runtime_available(&runtime).await {
                info!("✓ Runtime available: {} ({})", runtime.name, runtime.r#ref);
                available_runtimes.push(runtime.name.to_lowercase());
            } else {
                debug!(
                    "✗ Runtime not available: {} ({})",
                    runtime.name, runtime.r#ref
                );
            }
        }

        info!("Detected available runtimes: {:?}", available_runtimes);

        Ok(available_runtimes)
    }

    /// Verify if a runtime is available on this system
    pub async fn verify_runtime_available(runtime: &Runtime) -> bool {
        // Check if runtime is always available (e.g., shell, native, builtin)
        if let Some(verification) = runtime.distributions.get("verification") {
            if let Some(always_available) = verification.get("always_available") {
                if always_available.as_bool() == Some(true) {
                    debug!("Runtime {} is marked as always available", runtime.name);
                    return true;
                }
            }

            if let Some(check_required) = verification.get("check_required") {
                if check_required.as_bool() == Some(false) {
                    debug!(
                        "Runtime {} does not require verification check",
                        runtime.name
                    );
                    return true;
                }
            }

            // Get verification commands
            if let Some(commands) = verification.get("commands") {
                if let Some(commands_array) = commands.as_array() {
                    // Try each command in priority order
                    let mut sorted_commands = commands_array.clone();
                    sorted_commands.sort_by_key(|cmd| {
                        cmd.get("priority").and_then(|p| p.as_i64()).unwrap_or(999)
                    });

                    for cmd in sorted_commands {
                        if Self::try_verification_command(&cmd, &runtime.name).await {
                            return true;
                        }
                    }
                }
            }
        }

        // No verification metadata or all checks failed
        false
    }

    /// Try executing a verification command to check if runtime is available
    async fn try_verification_command(cmd: &serde_json::Value, runtime_name: &str) -> bool {
        let binary = match cmd.get("binary").and_then(|b| b.as_str()) {
            Some(b) => b,
            None => {
                warn!(
                    "Verification command missing 'binary' field for {}",
                    runtime_name
                );
                return false;
            }
        };

        let args = cmd
            .get("args")
            .and_then(|a| a.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect::<Vec<String>>()
            })
            .unwrap_or_default();

        let expected_exit_code = cmd.get("exit_code").and_then(|e| e.as_i64()).unwrap_or(0);

        let pattern = cmd.get("pattern").and_then(|p| p.as_str());

        let optional = cmd
            .get("optional")
            .and_then(|o| o.as_bool())
            .unwrap_or(false);

        debug!(
            "Trying verification: {} {:?} (expecting exit code {})",
            binary, args, expected_exit_code
        );

        // Execute command
        let output = match Command::new(binary).args(&args).output() {
            Ok(output) => output,
            Err(e) => {
                if !optional {
                    debug!("Failed to execute {}: {}", binary, e);
                }
                return false;
            }
        };

        // Check exit code
        let exit_code = output.status.code().unwrap_or(-1);
        if exit_code != expected_exit_code as i32 {
            if !optional {
                debug!(
                    "Command {} exited with {} (expected {})",
                    binary, exit_code, expected_exit_code
                );
            }
            return false;
        }

        // Check pattern if specified
        if let Some(pattern_str) = pattern {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let combined_output = format!("{}{}", stdout, stderr);

            match regex::Regex::new(pattern_str) {
                Ok(re) => {
                    if re.is_match(&combined_output) {
                        debug!(
                            "✓ Runtime verified: {} (matched pattern: {})",
                            runtime_name, pattern_str
                        );
                        return true;
                    } else {
                        if !optional {
                            debug!(
                                "Command {} output did not match pattern: {}",
                                binary, pattern_str
                            );
                        }
                        return false;
                    }
                }
                Err(e) => {
                    warn!("Invalid regex pattern '{}': {}", pattern_str, e);
                    return false;
                }
            }
        }

        // No pattern specified, just check exit code (already verified above)
        debug!("✓ Runtime verified: {} (exit code match)", runtime_name);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_verification_command_structure() {
        let cmd = json!({
            "binary": "python3",
            "args": ["--version"],
            "exit_code": 0,
            "pattern": "Python 3\\.",
            "priority": 1
        });

        assert_eq!(cmd.get("binary").unwrap().as_str().unwrap(), "python3");
        assert!(cmd.get("args").unwrap().is_array());
        assert_eq!(cmd.get("exit_code").unwrap().as_i64().unwrap(), 0);
    }

    #[test]
    fn test_always_available_flag() {
        let verification = json!({
            "always_available": true
        });

        assert_eq!(
            verification
                .get("always_available")
                .unwrap()
                .as_bool()
                .unwrap(),
            true
        );
    }

    #[tokio::test]
    async fn test_verify_command_with_pattern() {
        // Test shell verification (should always work)
        let cmd = json!({
            "binary": "sh",
            "args": ["--version"],
            "exit_code": 0,
            "optional": true,
            "priority": 1
        });

        // This might fail on some systems, but should not panic
        let _ = RuntimeDetector::try_verification_command(&cmd, "Shell").await;
    }
}
