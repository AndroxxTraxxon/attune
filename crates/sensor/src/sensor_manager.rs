//! Sensor Manager
//!
//! Manages the lifecycle of standalone sensor processes including loading,
//! starting, stopping, and monitoring sensor instances.
//!
//! All sensors are independent processes that communicate with the API
//! to create events. The sensor manager is responsible for:
//! - Starting sensor processes when rules become active
//! - Stopping sensor processes when no rules need them
//! - Provisioning authentication tokens for sensor processes
//! - Monitoring sensor health and restarting failed sensors

use anyhow::{anyhow, Result};
use attune_common::models::{runtime::RuntimeExecutionConfig, Id, Sensor, Trigger};
use attune_common::repositories::{FindById, List, RuntimeRepository};

use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::io;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::RwLock;
use tokio::task::JoinHandle;
use tokio::time::{interval, timeout, Duration};
use tracing::{debug, error, info, warn};

use crate::api_client::ApiClient;

fn existing_command_env(cmd: &Command, key: &str) -> Option<String> {
    cmd.as_std()
        .get_envs()
        .find_map(|(env_key, value)| {
            if env_key == key {
                value.map(|value| value.to_string_lossy().into_owned())
            } else {
                None
            }
        })
        .or_else(|| std::env::var(key).ok())
}

fn apply_runtime_env_vars(
    cmd: &mut Command,
    exec_config: &RuntimeExecutionConfig,
    pack_dir: &std::path::Path,
    env_dir: Option<&std::path::Path>,
) {
    if exec_config.env_vars.is_empty() {
        return;
    }

    let vars = exec_config.build_template_vars_with_env(pack_dir, env_dir);
    for (key, env_var_config) in &exec_config.env_vars {
        let resolved = env_var_config.resolve(&vars, existing_command_env(cmd, key).as_deref());
        debug!("Setting sensor runtime env var: {}={}", key, resolved);
        cmd.env(key, resolved);
    }
}

fn configure_sensor_process(cmd: &mut Command) -> io::Result<()> {
    #[cfg(unix)]
    {
        // Run each sensor in its own process group so shutdown can terminate
        // the interpreter wrapper and any children it spawned.
        unsafe {
            cmd.pre_exec(|| {
                if libc::setpgid(0, 0) == -1 {
                    Err(io::Error::last_os_error())
                } else {
                    Ok(())
                }
            });
        }
    }

    Ok(())
}

fn signal_process_group_or_process(pid: u32, signal: i32) {
    #[cfg(unix)]
    {
        let pgid = -(pid as i32);
        let rc = unsafe { libc::kill(pgid, signal) };
        if rc == 0 {
            return;
        }

        let err = io::Error::last_os_error();
        warn!(
            "Failed to signal sensor process group {} with signal {}: {}. Falling back to PID {}",
            pid, signal, err, pid
        );
    }

    unsafe {
        libc::kill(pid as i32, signal);
    }
}

async fn terminate_sensor_child(child: &mut Child, sensor_label: &str) {
    let Some(pid) = child.id() else {
        if let Err(e) = child.start_kill() {
            error!("Failed to kill sensor process {}: {}", sensor_label, e);
            return;
        }
        if let Err(e) = child.wait().await {
            error!("Failed to reap sensor process {}: {}", sensor_label, e);
        }
        return;
    };

    info!(
        "Sending SIGTERM to sensor {} process group {}",
        sensor_label, pid
    );
    signal_process_group_or_process(pid, libc::SIGTERM);

    match timeout(Duration::from_secs(5), child.wait()).await {
        Ok(Ok(_)) => {}
        Ok(Err(e)) => error!("Failed waiting for sensor process {}: {}", sensor_label, e),
        Err(_) => {
            warn!(
                "Sensor {} did not exit after SIGTERM + 5s, sending SIGKILL",
                sensor_label
            );
            signal_process_group_or_process(pid, libc::SIGKILL);
            if let Err(e) = child.wait().await {
                error!(
                    "Failed to reap sensor process {} after SIGKILL: {}",
                    sensor_label, e
                );
            }
        }
    }
}

/// Sensor manager that coordinates all sensor instances
#[derive(Clone)]
pub struct SensorManager {
    inner: Arc<SensorManagerInner>,
}

#[derive(Debug, Clone, Default)]
pub struct SensorActivityMetrics {
    pub monitored_sensors: u64,
    pub running_sensors: u64,
    pub active_rules: u64,
}

struct SensorManagerInner {
    db: PgPool,
    sensors: Arc<RwLock<HashMap<Id, SensorInstance>>>,
    running: Arc<RwLock<bool>>,
    packs_base_dir: String,
    runtime_envs_dir: String,
    api_client: ApiClient,
    api_url: String,
    mq_url: String,
}

impl SensorManager {
    /// Create a new sensor manager
    pub fn new(db: PgPool) -> Self {
        // Get packs base directory from config or default
        let packs_base_dir =
            std::env::var("ATTUNE_PACKS_BASE_DIR").unwrap_or_else(|_| "./packs".to_string());

        // Get API URL from config or default
        let api_url =
            std::env::var("ATTUNE_API_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());

        // Get MQ URL from config or default
        let mq_url = std::env::var("ATTUNE_MQ_URL")
            .unwrap_or_else(|_| "amqp://guest:guest@localhost:5672".to_string());

        let runtime_envs_dir = std::env::var("ATTUNE_RUNTIME_ENVS_DIR")
            .or_else(|_| std::env::var("ATTUNE__RUNTIME_ENVS_DIR"))
            .unwrap_or_else(|_| "/opt/attune/runtime_envs".to_string());

        // Create API client for token provisioning (no admin token - uses internal endpoint)
        let api_client = ApiClient::new(api_url.clone(), None);

        Self {
            inner: Arc::new(SensorManagerInner {
                db,
                sensors: Arc::new(RwLock::new(HashMap::new())),
                running: Arc::new(RwLock::new(false)),
                packs_base_dir,
                runtime_envs_dir,
                api_client,
                api_url,
                mq_url,
            }),
        }
    }

    /// Start the sensor manager
    pub async fn start(&self) -> Result<()> {
        info!("Starting sensor manager");

        // Mark as running
        *self.inner.running.write().await = true;

        // Load and start all enabled sensors with active rules
        let sensors = self.load_enabled_sensors().await?;
        info!("Loaded {} enabled sensor(s)", sensors.len());

        for sensor in sensors {
            // Only start sensors that have active rules
            match self.has_active_rules(sensor.trigger).await {
                Ok(true) => {
                    let count = self
                        .get_active_rule_count(sensor.trigger)
                        .await
                        .unwrap_or(0);
                    info!(
                        "Starting sensor {} - has {} active rule(s)",
                        sensor.r#ref, count
                    );
                    if let Err(e) = self.start_sensor(sensor).await {
                        error!("Failed to start sensor: {}", e);
                    }
                }
                Ok(false) => {
                    info!("Skipping sensor {} - no active rules", sensor.r#ref);
                }
                Err(e) => {
                    error!(
                        "Failed to check active rules for sensor {}: {}",
                        sensor.r#ref, e
                    );
                }
            }
        }

        // Start monitoring loop
        let manager = self.clone();
        tokio::spawn(async move {
            manager.monitoring_loop().await;
        });

        info!("Sensor manager started");

        Ok(())
    }

    /// Stop the sensor manager
    pub async fn stop(&self) -> Result<()> {
        info!("Stopping sensor manager");

        // Mark as not running
        *self.inner.running.write().await = false;

        // Collect sensor IDs to stop
        let sensor_ids: Vec<Id> = self.inner.sensors.read().await.keys().copied().collect();

        // Stop all sensors
        for sensor_id in sensor_ids {
            info!("Stopping sensor {}", sensor_id);
            if let Err(e) = self.stop_sensor(sensor_id).await {
                error!("Failed to stop sensor {}: {}", sensor_id, e);
            }
        }

        info!("Sensor manager stopped");

        Ok(())
    }

    /// Load all enabled sensors from the database
    async fn load_enabled_sensors(&self) -> Result<Vec<Sensor>> {
        use attune_common::repositories::SensorRepository;

        let all_sensors = SensorRepository::list(&self.inner.db).await?;
        let enabled_sensors: Vec<Sensor> = all_sensors.into_iter().filter(|s| s.enabled).collect();
        Ok(enabled_sensors)
    }

    async fn ensure_runtime_environment(
        &self,
        exec_config: &RuntimeExecutionConfig,
        pack_dir: &std::path::Path,
        env_dir: &std::path::Path,
    ) -> Result<()> {
        let env_cfg = match &exec_config.environment {
            Some(cfg) if cfg.env_type != "none" => cfg,
            _ => return Ok(()),
        };

        let vars = exec_config.build_template_vars_with_env(pack_dir, Some(env_dir));

        if !env_dir.exists() {
            if env_cfg.create_command.is_empty() {
                return Err(anyhow!(
                    "Runtime environment '{}' requires create_command but none is configured",
                    env_cfg.env_type
                ));
            }

            if let Some(parent) = env_dir.parent() {
                tokio::fs::create_dir_all(parent).await.map_err(|e| {
                    anyhow!(
                        "Failed to create runtime environment parent directory {}: {}",
                        parent.display(),
                        e
                    )
                })?;
            }

            let resolved_cmd =
                RuntimeExecutionConfig::resolve_command(&env_cfg.create_command, &vars);
            let (program, args) = resolved_cmd
                .split_first()
                .ok_or_else(|| anyhow!("Empty create_command for runtime environment"))?;

            info!(
                "Creating sensor runtime environment at {}: {:?}",
                env_dir.display(),
                resolved_cmd
            );

            let output = Command::new(program)
                .args(args)
                .current_dir(pack_dir)
                .output()
                .await
                .map_err(|e| anyhow!("Failed to run create command '{}': {}", program, e))?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow!(
                    "Runtime environment creation failed (exit {}): {}",
                    output.status.code().unwrap_or(-1),
                    stderr.trim()
                ));
            }
        }

        let dep_cfg = match &exec_config.dependencies {
            Some(cfg) => cfg,
            None => return Ok(()),
        };

        let manifest_path = pack_dir.join(&dep_cfg.manifest_file);
        if !manifest_path.exists() || dep_cfg.install_command.is_empty() {
            return Ok(());
        }

        let install_marker = env_dir.join(".attune_sensor_deps_installed");
        if install_marker.exists() {
            return Ok(());
        }

        let resolved_cmd = RuntimeExecutionConfig::resolve_command(&dep_cfg.install_command, &vars);
        let (program, args) = resolved_cmd
            .split_first()
            .ok_or_else(|| anyhow!("Empty install_command for runtime dependencies"))?;

        info!(
            "Installing sensor runtime dependencies for {} using {:?}",
            pack_dir.display(),
            resolved_cmd
        );

        let output = Command::new(program)
            .args(args)
            .current_dir(pack_dir)
            .output()
            .await
            .map_err(|e| {
                anyhow!(
                    "Failed to run dependency install command '{}': {}",
                    program,
                    e
                )
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!(
                "Runtime dependency installation failed (exit {}): {}",
                output.status.code().unwrap_or(-1),
                stderr.trim()
            ));
        }

        tokio::fs::write(&install_marker, b"ok")
            .await
            .map_err(|e| {
                anyhow!(
                    "Failed to write dependency install marker {}: {}",
                    install_marker.display(),
                    e
                )
            })?;

        Ok(())
    }

    /// Start a sensor instance
    async fn start_sensor(&self, sensor: Sensor) -> Result<()> {
        info!("Starting sensor {} ({})", sensor.r#ref, sensor.id);

        // Load trigger information
        let trigger = self.load_trigger(sensor.trigger).await?;

        // All sensors are now standalone processes
        let instance = self
            .start_standalone_sensor(sensor.clone(), trigger)
            .await?;

        // Store instance
        self.inner.sensors.write().await.insert(sensor.id, instance);

        info!("Sensor {} started successfully", sensor.r#ref);

        Ok(())
    }

    /// Start a standalone sensor with token provisioning
    async fn start_standalone_sensor(
        &self,
        sensor: Sensor,
        trigger: Trigger,
    ) -> Result<SensorInstance> {
        info!("Starting standalone sensor: {}", sensor.r#ref);

        // Get trigger types
        let trigger_types = vec![trigger.r#ref.clone()];

        // Provision sensor token via API
        info!("Provisioning token for sensor: {}", sensor.r#ref);
        let token_response = self
            .inner
            .api_client
            .create_sensor_token(&sensor.r#ref, trigger_types, Some(86400))
            .await
            .map_err(|e| anyhow!("Failed to provision sensor token: {}", e))?;

        info!(
            "Token provisioned for sensor {} (expires: {})",
            sensor.r#ref, token_response.expires_at
        );

        // Build sensor script path
        let pack_ref = sensor
            .pack_ref
            .as_ref()
            .ok_or_else(|| anyhow!("Sensor {} has no pack_ref", sensor.r#ref))?;

        let sensor_script = format!(
            "{}/{}/sensors/{}",
            self.inner.packs_base_dir, pack_ref, sensor.entrypoint
        );

        // Load the runtime to determine how to execute the sensor
        let runtime = RuntimeRepository::find_by_id(&self.inner.db, sensor.runtime)
            .await?
            .ok_or_else(|| {
                anyhow!(
                    "Runtime {} not found for sensor {}",
                    sensor.runtime,
                    sensor.r#ref
                )
            })?;

        let exec_config = runtime.parsed_execution_config();
        let rt_name = runtime.name.to_lowercase();
        let runtime_env_suffix = runtime
            .r#ref
            .rsplit('.')
            .next()
            .filter(|suffix| !suffix.is_empty())
            .unwrap_or(&rt_name);

        info!(
            "Sensor {} runtime details: id={}, ref='{}', name='{}', execution_config={}",
            sensor.r#ref, runtime.id, runtime.r#ref, runtime.name, runtime.execution_config
        );

        // Resolve the interpreter: check for a virtualenv/node_modules first,
        // then fall back to the system interpreter.
        let pack_dir = std::path::PathBuf::from(&self.inner.packs_base_dir).join(pack_ref);
        let env_dir = std::path::PathBuf::from(&self.inner.runtime_envs_dir)
            .join(pack_ref)
            .join(runtime_env_suffix);
        if let Err(e) = self
            .ensure_runtime_environment(&exec_config, &pack_dir, &env_dir)
            .await
        {
            warn!(
                "Failed to ensure sensor runtime environment for {} at {}: {}",
                sensor.r#ref,
                env_dir.display(),
                e
            );
        }

        let env_dir_opt = if env_dir.exists() {
            Some(env_dir.as_path())
        } else {
            None
        };

        // Determine whether we need an interpreter or can execute directly.
        // Determine native vs interpreted purely from the runtime's execution_config.
        // A native runtime (e.g., core.native) has no interpreter configured —
        // its binary field is empty. Interpreted runtimes (Python, Node, etc.)
        // declare their interpreter binary explicitly in execution_config.
        let interpreter_binary = &exec_config.interpreter.binary;
        let is_native = interpreter_binary.is_empty()
            || interpreter_binary == "native"
            || interpreter_binary == "none";

        info!(
            "Sensor {} runtime={} (ref={}) interpreter='{}' native={} env_dir_exists={}",
            sensor.r#ref,
            rt_name,
            runtime.r#ref,
            interpreter_binary,
            is_native,
            env_dir.exists()
        );
        info!("Starting standalone sensor process: {}", sensor_script);

        // Fetch trigger instances (enabled rules with their trigger params)
        info!(
            "About to fetch trigger instances for sensor {} (trigger_id: {})",
            sensor.r#ref, sensor.trigger
        );
        let trigger_instances = match self.fetch_trigger_instances(sensor.trigger).await {
            Ok(instances) => {
                info!(
                    "Fetched {} trigger instance(s) for sensor {}",
                    instances.len(),
                    sensor.r#ref
                );
                instances
            }
            Err(e) => {
                error!(
                    "Failed to fetch trigger instances for sensor {}: {}",
                    sensor.r#ref, e
                );
                return Err(e);
            }
        };

        let trigger_instances_json = serde_json::to_string(&trigger_instances)
            .map_err(|e| anyhow!("Failed to serialize trigger instances: {}", e))?;
        info!("Trigger instances JSON: {}", trigger_instances_json);

        // Build the command: use the interpreter for non-native runtimes,
        // execute the script directly for native binaries.
        let (spawn_binary, mut cmd) = if is_native {
            (sensor_script.clone(), Command::new(&sensor_script))
        } else {
            let resolved_interpreter =
                exec_config.resolve_interpreter_with_env(&pack_dir, env_dir_opt);
            info!(
                "Using interpreter {} for sensor {}",
                resolved_interpreter.display(),
                sensor.r#ref
            );
            let binary_str = resolved_interpreter.display().to_string();
            let mut c = Command::new(&resolved_interpreter);
            // Pass any extra interpreter args (e.g., -u for unbuffered Python)
            for arg in &exec_config.interpreter.args {
                c.arg(arg);
            }
            c.arg(&sensor_script);
            (binary_str, c)
        };

        // Log the full command for diagnostics
        info!(
            "Spawning sensor {}: binary='{}' is_native={} script='{}'",
            sensor.r#ref, spawn_binary, is_native, sensor_script
        );

        // Pre-flight check: verify the binary exists and is accessible
        let spawn_path = std::path::Path::new(&spawn_binary);
        if spawn_path.is_absolute() || spawn_path.components().count() > 1 {
            // Absolute or relative path with directory component — check it directly
            match std::fs::metadata(spawn_path) {
                Ok(meta) => {
                    use std::os::unix::fs::PermissionsExt;
                    let mode = meta.permissions().mode();
                    let is_exec = mode & 0o111 != 0;
                    if !is_exec {
                        error!(
                            "Binary '{}' exists but is not executable (mode: {:o}). \
                             Sensor runtime ref='{}', execution_config interpreter='{}'.",
                            spawn_binary, mode, runtime.r#ref, interpreter_binary
                        );
                    }
                }
                Err(e) => {
                    error!(
                        "Cannot access binary '{}': {}. \
                         Sensor runtime ref='{}', execution_config interpreter='{}'.",
                        spawn_binary, e, runtime.r#ref, interpreter_binary
                    );
                }
            }
        }

        // Start the standalone sensor with token and configuration
        // Pass sensor ref (e.g., "core.interval_timer_sensor") for proper identification
        cmd.env("ATTUNE_API_URL", &self.inner.api_url)
            .env("ATTUNE_API_TOKEN", &token_response.token)
            .env("ATTUNE_SENSOR_ID", sensor.id.to_string())
            .env("ATTUNE_SENSOR_REF", &sensor.r#ref)
            .env("ATTUNE_SENSOR_TRIGGERS", &trigger_instances_json)
            .env("ATTUNE_MQ_URL", &self.inner.mq_url)
            .env("ATTUNE_MQ_EXCHANGE", "attune.events")
            .env("ATTUNE_LOG_LEVEL", "info");

        apply_runtime_env_vars(&mut cmd, &exec_config, &pack_dir, env_dir_opt);
        configure_sensor_process(&mut cmd)
            .map_err(|e| anyhow!("Failed to configure sensor process: {}", e))?;

        let mut child = cmd
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                anyhow!(
                    "Failed to start sensor process for '{}': {} \
                     (binary='{}', is_native={}, runtime_ref='{}', \
                     interpreter_config='{}', env_dir='{}')",
                    sensor.r#ref,
                    e,
                    spawn_binary,
                    is_native,
                    runtime.r#ref,
                    interpreter_binary,
                    env_dir.display()
                )
            })?;

        // Get stdout and stderr for logging (standalone sensors output JSON logs to stdout)
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("Failed to capture sensor stdout"))?;

        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| anyhow!("Failed to capture sensor stderr"))?;

        // Spawn task to log stdout
        let sensor_ref_stdout = sensor.r#ref.clone();
        let stdout_handle = tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();

            while let Ok(Some(line)) = reader.next_line().await {
                info!("Sensor {} stdout: {}", sensor_ref_stdout, line);
            }

            info!("Sensor {} stdout stream closed", sensor_ref_stdout);
        });

        // Spawn task to log stderr
        let sensor_ref_stderr = sensor.r#ref.clone();
        let stderr_handle = tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();

            while let Ok(Some(line)) = reader.next_line().await {
                warn!("Sensor {} stderr: {}", sensor_ref_stderr, line);
            }

            info!("Sensor {} stderr stream closed", sensor_ref_stderr);
        });

        Ok(SensorInstance::new_standalone(
            sensor.r#ref.clone(),
            child,
            stdout_handle,
            stderr_handle,
        ))
    }

    /// Load trigger information
    async fn load_trigger(&self, trigger_id: Id) -> Result<Trigger> {
        use attune_common::repositories::TriggerRepository;

        TriggerRepository::find_by_id(&self.inner.db, trigger_id)
            .await?
            .ok_or_else(|| anyhow!("Trigger {} not found", trigger_id))
    }

    /// Check if a trigger has any active/enabled rules
    async fn has_active_rules(&self, trigger_id: Id) -> Result<bool> {
        let count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM rule
            WHERE trigger = $1
              AND enabled = TRUE
            "#,
        )
        .bind(trigger_id)
        .fetch_one(&self.inner.db)
        .await?;

        Ok(count > 0)
    }

    /// Get count of active rules for a trigger
    async fn get_active_rule_count(&self, trigger_id: Id) -> Result<i64> {
        let count = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(*)
            FROM rule
            WHERE trigger = $1
              AND enabled = TRUE
            "#,
        )
        .bind(trigger_id)
        .fetch_one(&self.inner.db)
        .await?;

        Ok(count)
    }

    /// Fetch trigger instances (enabled rules with their trigger params) for a trigger
    async fn fetch_trigger_instances(&self, trigger_id: Id) -> Result<Vec<serde_json::Value>> {
        let rows = sqlx::query(
            r#"
            SELECT *
            FROM rule
            WHERE trigger = $1
              AND enabled = TRUE
            "#,
        )
        .bind(trigger_id)
        .fetch_all(&self.inner.db)
        .await?;

        info!("Fetched {} rows from rule table", rows.len());

        // Convert to the format expected by timer sensor
        let trigger_instances: Vec<serde_json::Value> = rows
            .into_iter()
            .map(|row| {
                let id: i64 = row.try_get("id").unwrap_or(0);
                let ref_str: String = row.try_get("ref").unwrap_or_default();
                let trigger_params: serde_json::Value = row
                    .try_get("trigger_params")
                    .unwrap_or(serde_json::json!({}));

                info!(
                    "Rule ID: {}, Ref: {}, Params: {}",
                    id, ref_str, trigger_params
                );

                serde_json::json!({
                    "id": id,
                    "ref": ref_str,
                    "config": trigger_params
                })
            })
            .collect();

        Ok(trigger_instances)
    }

    /// Stop a sensor
    pub async fn stop_sensor(&self, sensor_id: Id) -> Result<()> {
        info!("Stopping sensor {}", sensor_id);

        let mut sensors = self.inner.sensors.write().await;

        if let Some(mut instance) = sensors.remove(&sensor_id) {
            instance.stop().await;
            info!("Sensor {} stopped", sensor_id);
        } else {
            warn!("Sensor {} not found in running instances", sensor_id);
        }

        Ok(())
    }

    /// Handle rule changes (created, enabled, disabled)
    pub async fn handle_rule_change(&self, trigger_id: Id) -> Result<()> {
        info!("Handling rule change for trigger {}", trigger_id);

        // Find sensors for this trigger
        let sensors = sqlx::query_as::<_, Sensor>(
            r#"
            SELECT
                id,
                ref,
                pack,
                pack_ref,
                label,
                description,
                entrypoint,
                runtime,
                runtime_ref,
                runtime_version_constraint,
                trigger,
                trigger_ref,
                enabled,
                param_schema,
                config,
                created,
                updated
            FROM sensor
            WHERE trigger = $1
              AND enabled = TRUE
            "#,
        )
        .bind(trigger_id)
        .fetch_all(&self.inner.db)
        .await?;

        for sensor in sensors {
            // Check if sensor is running
            let is_running = self.inner.sensors.read().await.contains_key(&sensor.id);

            // Check if sensor should be running (has active rules)
            let should_run = self.has_active_rules(trigger_id).await?;

            match (is_running, should_run) {
                (false, true) => {
                    // Start sensor
                    info!("Starting sensor {} due to rule change", sensor.r#ref);
                    if let Err(e) = self.start_sensor(sensor).await {
                        error!("Failed to start sensor: {}", e);
                    }
                }
                (true, false) => {
                    // Stop sensor
                    info!("Stopping sensor {} - no active rules", sensor.r#ref);
                    if let Err(e) = self.stop_sensor(sensor.id).await {
                        error!("Failed to stop sensor: {}", e);
                    }
                }
                (true, true) => {
                    // Restart sensor to pick up new trigger instances
                    info!(
                        "Restarting sensor {} to update trigger instances",
                        sensor.r#ref
                    );
                    if let Err(e) = self.stop_sensor(sensor.id).await {
                        error!("Failed to stop sensor: {}", e);
                    }
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    if let Err(e) = self.start_sensor(sensor).await {
                        error!("Failed to restart sensor: {}", e);
                    }
                }
                (false, false) => {
                    // No action needed
                    debug!("Sensor {} - no action needed", sensor.r#ref);
                }
            }
        }

        Ok(())
    }

    /// Monitoring loop to check sensor health
    async fn monitoring_loop(&self) {
        let mut interval = interval(Duration::from_secs(60));

        while *self.inner.running.read().await {
            interval.tick().await;

            debug!("Sensor manager monitoring check");

            let sensors = self.inner.sensors.read().await;
            for (sensor_id, instance) in sensors.iter() {
                let status = instance.status().await;

                if status.failed {
                    warn!(
                        "Sensor {} has failed (failure_count: {})",
                        sensor_id, status.failure_count
                    );
                }

                // Check if long-running process has died
                if let Some(ref _child) = instance.child_process {
                    // Note: We can't easily check if child is still running without blocking
                    // This would need enhancement with a better process management approach
                }
            }
        }

        info!("Sensor manager monitoring loop stopped");
    }

    /// Get count of active sensors
    pub async fn active_count(&self) -> usize {
        let sensors = self.inner.sensors.read().await;
        let mut active = 0;

        for instance in sensors.values() {
            let status = instance.status().await;
            if status.running && !status.failed {
                active += 1;
            }
        }

        active
    }

    /// Get count of failed sensors
    pub async fn failed_count(&self) -> usize {
        let sensors = self.inner.sensors.read().await;
        let mut failed = 0;

        for instance in sensors.values() {
            let status = instance.status().await;
            if status.failed {
                failed += 1;
            }
        }

        failed
    }

    /// Get total count of sensors
    pub async fn total_count(&self) -> usize {
        self.inner.sensors.read().await.len()
    }

    /// Get sensor activity metrics suitable for worker heartbeat/capability reporting.
    pub async fn activity_metrics(&self) -> Result<SensorActivityMetrics> {
        let running_sensor_ids: Vec<Id> = self.inner.sensors.read().await.keys().copied().collect();
        let running_sensors = self.active_count().await as u64;

        if running_sensor_ids.is_empty() {
            return Ok(SensorActivityMetrics {
                monitored_sensors: 0,
                running_sensors,
                active_rules: 0,
            });
        }

        let active_rules = sqlx::query_scalar::<_, i64>(
            r#"
            SELECT COUNT(DISTINCT rule.id)
            FROM rule
            JOIN sensor ON sensor.trigger = rule.trigger
            WHERE sensor.id = ANY($1)
              AND sensor.enabled = TRUE
              AND rule.enabled = TRUE
            "#,
        )
        .bind(&running_sensor_ids)
        .fetch_one(&self.inner.db)
        .await?;

        Ok(SensorActivityMetrics {
            monitored_sensors: running_sensor_ids.len() as u64,
            running_sensors,
            active_rules: active_rules.max(0) as u64,
        })
    }
}

/// Sensor instance managing a running sensor
struct SensorInstance {
    sensor_ref: String,
    status: Arc<RwLock<SensorStatus>>,
    child_process: Option<Child>,
    stderr_handle: Option<JoinHandle<()>>,
    stdout_handle: Option<JoinHandle<()>>,
}

impl SensorInstance {
    /// Create a new standalone sensor instance
    fn new_standalone(
        sensor_ref: String,
        child_process: Child,
        stdout_handle: JoinHandle<()>,
        stderr_handle: JoinHandle<()>,
    ) -> Self {
        Self {
            sensor_ref,
            status: Arc::new(RwLock::new(SensorStatus {
                running: true,
                failed: false,
                failure_count: 0,
                last_poll: Some(chrono::Utc::now()),
            })),
            child_process: Some(child_process),
            stderr_handle: Some(stderr_handle),
            stdout_handle: Some(stdout_handle),
        }
    }

    /// Stop the sensor
    async fn stop(&mut self) {
        {
            let mut status = self.status.write().await;
            status.running = false;
        }

        // Kill child process if exists
        if let Some(ref mut child) = self.child_process {
            terminate_sensor_child(child, &self.sensor_ref).await;
        }

        // Abort task handles
        if let Some(ref handle) = self.stdout_handle {
            handle.abort();
        }

        if let Some(ref handle) = self.stderr_handle {
            handle.abort();
        }
    }

    /// Get sensor status
    async fn status(&self) -> SensorStatus {
        self.status.read().await.clone()
    }
}

/// Sensor status information
#[derive(Clone, Debug, Default)]
pub struct SensorStatus {
    /// Whether the sensor is running
    pub running: bool,

    /// Whether the sensor has failed
    pub failed: bool,

    /// Number of consecutive failures
    pub failure_count: u32,

    /// Last successful poll time
    pub last_poll: Option<chrono::DateTime<chrono::Utc>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use attune_common::models::runtime::{
        RuntimeEnvVarConfig, RuntimeEnvVarOperation, RuntimeEnvVarSpec,
    };
    use std::collections::HashMap;
    use tempfile::NamedTempFile;
    use tokio::fs;

    #[test]
    fn test_sensor_status_default() {
        let status = SensorStatus::default();
        assert!(!status.running);
        assert!(!status.failed);
        assert_eq!(status.failure_count, 0);
        assert!(status.last_poll.is_none());
    }

    #[test]
    fn test_apply_runtime_env_vars_prepends_to_existing_command_env() {
        let mut env_vars = HashMap::new();
        env_vars.insert(
            "PYTHONPATH".to_string(),
            RuntimeEnvVarConfig::Spec(RuntimeEnvVarSpec {
                value: "{pack_dir}/lib".to_string(),
                operation: RuntimeEnvVarOperation::Prepend,
                separator: ":".to_string(),
            }),
        );

        let exec_config = RuntimeExecutionConfig {
            env_vars,
            ..RuntimeExecutionConfig::default()
        };

        let mut cmd = Command::new("python3");
        cmd.env("PYTHONPATH", "/existing/pythonpath");

        apply_runtime_env_vars(
            &mut cmd,
            &exec_config,
            std::path::Path::new("/packs/testpack"),
            None,
        );

        let resolved = cmd
            .as_std()
            .get_envs()
            .find_map(|(key, value)| {
                if key == "PYTHONPATH" {
                    value.map(|value| value.to_string_lossy().into_owned())
                } else {
                    None
                }
            })
            .expect("PYTHONPATH should be set");

        assert_eq!(resolved, "/packs/testpack/lib:/existing/pythonpath");
    }

    #[tokio::test]
    async fn test_sensor_instance_stop_reaps_child_process() {
        let script = NamedTempFile::new().unwrap();
        fs::write(script.path(), "#!/bin/sh\ntrap '' TERM\nsleep 30 &\nwait\n")
            .await
            .unwrap();

        let mut perms = fs::metadata(script.path()).await.unwrap().permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            perms.set_mode(0o755);
        }
        fs::set_permissions(script.path(), perms).await.unwrap();

        let mut cmd = Command::new("/bin/sh");
        cmd.arg(script.path())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        configure_sensor_process(&mut cmd).unwrap();

        let mut child = cmd.spawn().unwrap();
        let stdout = child.stdout.take().unwrap();
        let stderr = child.stderr.take().unwrap();
        let stdout_handle = tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(_)) = reader.next_line().await {}
        });
        let stderr_handle = tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(_)) = reader.next_line().await {}
        });

        let mut instance = SensorInstance::new_standalone(
            "test.sensor".to_string(),
            child,
            stdout_handle,
            stderr_handle,
        );
        instance.stop().await;

        let status = instance.child_process.as_mut().unwrap().try_wait().unwrap();
        assert!(
            status.is_some(),
            "child process should be reaped after stop()"
        );
    }
}
