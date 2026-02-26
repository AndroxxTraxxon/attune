//! Pack Component Loader
//!
//! Reads runtime, action, trigger, and sensor YAML definitions from a pack directory
//! and registers them in the database. This is the Rust-native equivalent of
//! the Python `load_core_pack.py` script used during init-packs.
//!
//! Components are loaded in dependency order:
//! 1. Runtimes (no dependencies)
//! 2. Triggers (no dependencies)
//! 3. Actions (depend on runtime)
//! 4. Sensors (depend on triggers and runtime)

use std::collections::HashMap;
use std::path::Path;

use sqlx::PgPool;
use tracing::{info, warn};

use crate::error::{Error, Result};
use crate::models::Id;
use crate::repositories::action::ActionRepository;
use crate::repositories::runtime::{CreateRuntimeInput, RuntimeRepository};
use crate::repositories::runtime_version::{CreateRuntimeVersionInput, RuntimeVersionRepository};
use crate::repositories::trigger::{
    CreateSensorInput, CreateTriggerInput, SensorRepository, TriggerRepository,
};
use crate::repositories::{Create, FindById, FindByRef, Update};
use crate::version_matching::extract_version_components;

/// Result of loading pack components into the database.
#[derive(Debug, Default)]
pub struct PackLoadResult {
    /// Number of runtimes loaded
    pub runtimes_loaded: usize,
    /// Number of runtimes skipped (already exist)
    pub runtimes_skipped: usize,
    /// Number of triggers loaded
    pub triggers_loaded: usize,
    /// Number of triggers skipped (already exist)
    pub triggers_skipped: usize,
    /// Number of actions loaded
    pub actions_loaded: usize,
    /// Number of actions skipped (already exist)
    pub actions_skipped: usize,
    /// Number of sensors loaded
    pub sensors_loaded: usize,
    /// Number of sensors skipped (already exist)
    pub sensors_skipped: usize,
    /// Warnings encountered during loading
    pub warnings: Vec<String>,
}

impl PackLoadResult {
    pub fn total_loaded(&self) -> usize {
        self.runtimes_loaded + self.triggers_loaded + self.actions_loaded + self.sensors_loaded
    }

    pub fn total_skipped(&self) -> usize {
        self.runtimes_skipped + self.triggers_skipped + self.actions_skipped + self.sensors_skipped
    }
}

/// Loads pack components (triggers, actions, sensors) from YAML files on disk
/// into the database.
pub struct PackComponentLoader<'a> {
    pool: &'a PgPool,
    pack_id: Id,
    pack_ref: String,
}

impl<'a> PackComponentLoader<'a> {
    pub fn new(pool: &'a PgPool, pack_id: Id, pack_ref: &str) -> Self {
        Self {
            pool,
            pack_id,
            pack_ref: pack_ref.to_string(),
        }
    }

    /// Load all components from the pack directory.
    ///
    /// Reads triggers, actions, and sensors from their respective subdirectories
    /// and registers them in the database. Components that already exist (by ref)
    /// are skipped.
    pub async fn load_all(&self, pack_dir: &Path) -> Result<PackLoadResult> {
        let mut result = PackLoadResult::default();

        info!(
            "Loading components for pack '{}' from {}",
            self.pack_ref,
            pack_dir.display()
        );

        // 1. Load runtimes first (no dependencies)
        self.load_runtimes(pack_dir, &mut result).await?;

        // 2. Load triggers (no dependencies)
        let trigger_ids = self.load_triggers(pack_dir, &mut result).await?;

        // 3. Load actions (depend on runtime)
        self.load_actions(pack_dir, &mut result).await?;

        // 4. Load sensors (depend on triggers and runtime)
        self.load_sensors(pack_dir, &trigger_ids, &mut result)
            .await?;

        info!(
            "Pack '{}' component loading complete: {} loaded, {} skipped, {} warnings",
            self.pack_ref,
            result.total_loaded(),
            result.total_skipped(),
            result.warnings.len()
        );

        Ok(result)
    }

    /// Load trigger definitions from `pack_dir/triggers/*.yaml`.
    ///
    /// Returns a map of trigger ref -> trigger ID for use by sensor loading.
    /// Load runtime definitions from `pack_dir/runtimes/*.yaml`.
    ///
    /// Runtimes define how actions and sensors are executed (interpreter,
    /// environment setup, dependency management). They are loaded first
    /// since actions reference them.
    async fn load_runtimes(&self, pack_dir: &Path, result: &mut PackLoadResult) -> Result<()> {
        let runtimes_dir = pack_dir.join("runtimes");

        if !runtimes_dir.exists() {
            info!("No runtimes directory found for pack '{}'", self.pack_ref);
            return Ok(());
        }

        let yaml_files = read_yaml_files(&runtimes_dir)?;
        info!(
            "Found {} runtime definition(s) for pack '{}'",
            yaml_files.len(),
            self.pack_ref
        );

        for (filename, content) in &yaml_files {
            let data: serde_yaml_ng::Value = serde_yaml_ng::from_str(content).map_err(|e| {
                Error::validation(format!("Failed to parse runtime YAML {}: {}", filename, e))
            })?;

            let runtime_ref = match data.get("ref").and_then(|v| v.as_str()) {
                Some(r) => r.to_string(),
                None => {
                    let msg = format!("Runtime YAML {} missing 'ref' field, skipping", filename);
                    warn!("{}", msg);
                    result.warnings.push(msg);
                    continue;
                }
            };

            // Check if runtime already exists
            if let Some(existing) = RuntimeRepository::find_by_ref(self.pool, &runtime_ref).await? {
                info!(
                    "Runtime '{}' already exists (ID: {}), skipping",
                    runtime_ref, existing.id
                );
                result.runtimes_skipped += 1;
                continue;
            }

            let name = data
                .get("name")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| extract_name_from_ref(&runtime_ref));

            let description = data
                .get("description")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let distributions = data
                .get("distributions")
                .and_then(|v| serde_json::to_value(v).ok())
                .unwrap_or_else(|| serde_json::json!({}));

            let installation = data
                .get("installation")
                .and_then(|v| serde_json::to_value(v).ok());

            let execution_config = data
                .get("execution_config")
                .and_then(|v| serde_json::to_value(v).ok())
                .unwrap_or_else(|| serde_json::json!({}));

            let input = CreateRuntimeInput {
                r#ref: runtime_ref.clone(),
                pack: Some(self.pack_id),
                pack_ref: Some(self.pack_ref.clone()),
                description,
                name,
                distributions,
                installation,
                execution_config,
            };

            match RuntimeRepository::create(self.pool, input).await {
                Ok(rt) => {
                    info!("Created runtime '{}' (ID: {})", runtime_ref, rt.id);
                    result.runtimes_loaded += 1;

                    // Load version entries from the optional `versions` array
                    self.load_runtime_versions(&data, rt.id, &runtime_ref, result)
                        .await;
                }
                Err(e) => {
                    // Check for unique constraint violation (race condition)
                    if let Error::Database(ref db_err) = e {
                        if let sqlx::Error::Database(ref inner) = db_err {
                            if inner.is_unique_violation() {
                                info!(
                                    "Runtime '{}' already exists (concurrent creation), skipping",
                                    runtime_ref
                                );
                                result.runtimes_skipped += 1;
                                continue;
                            }
                        }
                    }
                    let msg = format!("Failed to create runtime '{}': {}", runtime_ref, e);
                    warn!("{}", msg);
                    result.warnings.push(msg);
                }
            }
        }

        Ok(())
    }

    /// Load version entries from the `versions` array in a runtime YAML.
    ///
    /// Each entry in the array describes a specific version of the runtime
    /// with its own `execution_config` and `distributions`. Example:
    ///
    /// ```yaml
    /// versions:
    ///   - version: "3.12"
    ///     is_default: true
    ///     execution_config:
    ///       interpreter:
    ///         binary: python3.12
    ///         ...
    ///     distributions:
    ///       verification:
    ///         commands:
    ///           - binary: python3.12
    ///             args: ["--version"]
    ///             ...
    /// ```
    async fn load_runtime_versions(
        &self,
        data: &serde_yaml_ng::Value,
        runtime_id: Id,
        runtime_ref: &str,
        result: &mut PackLoadResult,
    ) {
        let versions = match data.get("versions").and_then(|v| v.as_sequence()) {
            Some(seq) => seq,
            None => return, // No versions defined — that's fine
        };

        info!(
            "Loading {} version(s) for runtime '{}'",
            versions.len(),
            runtime_ref
        );

        for entry in versions {
            let version_str = match entry.get("version").and_then(|v| v.as_str()) {
                Some(v) => v.to_string(),
                None => {
                    let msg = format!(
                        "Runtime '{}' has a version entry without a 'version' field, skipping",
                        runtime_ref
                    );
                    warn!("{}", msg);
                    result.warnings.push(msg);
                    continue;
                }
            };

            // Check if this version already exists
            if let Ok(Some(_existing)) = RuntimeVersionRepository::find_by_runtime_and_version(
                self.pool,
                runtime_id,
                &version_str,
            )
            .await
            {
                info!(
                    "Version '{}' for runtime '{}' already exists, skipping",
                    version_str, runtime_ref
                );
                continue;
            }

            let (version_major, version_minor, version_patch) =
                extract_version_components(&version_str);

            let execution_config = entry
                .get("execution_config")
                .and_then(|v| serde_json::to_value(v).ok())
                .unwrap_or_else(|| serde_json::json!({}));

            let distributions = entry
                .get("distributions")
                .and_then(|v| serde_json::to_value(v).ok())
                .unwrap_or_else(|| serde_json::json!({}));

            let is_default = entry
                .get("is_default")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);

            let meta = entry
                .get("meta")
                .and_then(|v| serde_json::to_value(v).ok())
                .unwrap_or_else(|| serde_json::json!({}));

            let input = CreateRuntimeVersionInput {
                runtime: runtime_id,
                runtime_ref: runtime_ref.to_string(),
                version: version_str.clone(),
                version_major,
                version_minor,
                version_patch,
                execution_config,
                distributions,
                is_default,
                available: true, // Assume available until verification runs
                meta,
            };

            match RuntimeVersionRepository::create(self.pool, input).await {
                Ok(rv) => {
                    info!(
                        "Created version '{}' for runtime '{}' (ID: {})",
                        version_str, runtime_ref, rv.id
                    );
                }
                Err(e) => {
                    // Check for unique constraint violation (race condition)
                    if let Error::Database(ref db_err) = e {
                        if let sqlx::Error::Database(ref inner) = db_err {
                            if inner.is_unique_violation() {
                                info!(
                                    "Version '{}' for runtime '{}' already exists (concurrent), skipping",
                                    version_str, runtime_ref
                                );
                                continue;
                            }
                        }
                    }
                    let msg = format!(
                        "Failed to create version '{}' for runtime '{}': {}",
                        version_str, runtime_ref, e
                    );
                    warn!("{}", msg);
                    result.warnings.push(msg);
                }
            }
        }
    }

    async fn load_triggers(
        &self,
        pack_dir: &Path,
        result: &mut PackLoadResult,
    ) -> Result<HashMap<String, Id>> {
        let triggers_dir = pack_dir.join("triggers");
        let mut trigger_ids = HashMap::new();

        if !triggers_dir.exists() {
            info!("No triggers directory found for pack '{}'", self.pack_ref);
            return Ok(trigger_ids);
        }

        let yaml_files = read_yaml_files(&triggers_dir)?;
        info!(
            "Found {} trigger definition(s) for pack '{}'",
            yaml_files.len(),
            self.pack_ref
        );

        for (filename, content) in &yaml_files {
            let data: serde_yaml_ng::Value = serde_yaml_ng::from_str(content).map_err(|e| {
                Error::validation(format!("Failed to parse trigger YAML {}: {}", filename, e))
            })?;

            let trigger_ref = match data.get("ref").and_then(|v| v.as_str()) {
                Some(r) => r.to_string(),
                None => {
                    let msg = format!("Trigger YAML {} missing 'ref' field, skipping", filename);
                    warn!("{}", msg);
                    result.warnings.push(msg);
                    continue;
                }
            };

            // Check if trigger already exists
            if let Some(existing) = TriggerRepository::find_by_ref(self.pool, &trigger_ref).await? {
                info!(
                    "Trigger '{}' already exists (ID: {}), skipping",
                    trigger_ref, existing.id
                );
                trigger_ids.insert(trigger_ref, existing.id);
                result.triggers_skipped += 1;
                continue;
            }

            let name = extract_name_from_ref(&trigger_ref);
            let label = data
                .get("label")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| generate_label(&name));

            let description = data
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let enabled = data
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            let param_schema = data
                .get("parameters")
                .and_then(|v| serde_json::to_value(v).ok());

            let out_schema = data
                .get("output")
                .and_then(|v| serde_json::to_value(v).ok());

            let input = CreateTriggerInput {
                r#ref: trigger_ref.clone(),
                pack: Some(self.pack_id),
                pack_ref: Some(self.pack_ref.clone()),
                label,
                description: Some(description),
                enabled,
                param_schema,
                out_schema,
                is_adhoc: false,
            };

            match TriggerRepository::create(self.pool, input).await {
                Ok(trigger) => {
                    info!("Created trigger '{}' (ID: {})", trigger_ref, trigger.id);
                    trigger_ids.insert(trigger_ref, trigger.id);
                    result.triggers_loaded += 1;
                }
                Err(e) => {
                    let msg = format!("Failed to create trigger '{}': {}", trigger_ref, e);
                    warn!("{}", msg);
                    result.warnings.push(msg);
                }
            }
        }

        Ok(trigger_ids)
    }

    /// Load action definitions from `pack_dir/actions/*.yaml`.
    async fn load_actions(&self, pack_dir: &Path, result: &mut PackLoadResult) -> Result<()> {
        let actions_dir = pack_dir.join("actions");

        if !actions_dir.exists() {
            info!("No actions directory found for pack '{}'", self.pack_ref);
            return Ok(());
        }

        let yaml_files = read_yaml_files(&actions_dir)?;
        info!(
            "Found {} action definition(s) for pack '{}'",
            yaml_files.len(),
            self.pack_ref
        );

        for (filename, content) in &yaml_files {
            let data: serde_yaml_ng::Value = serde_yaml_ng::from_str(content).map_err(|e| {
                Error::validation(format!("Failed to parse action YAML {}: {}", filename, e))
            })?;

            let action_ref = match data.get("ref").and_then(|v| v.as_str()) {
                Some(r) => r.to_string(),
                None => {
                    let msg = format!("Action YAML {} missing 'ref' field, skipping", filename);
                    warn!("{}", msg);
                    result.warnings.push(msg);
                    continue;
                }
            };

            // Check if action already exists
            if let Some(existing) = ActionRepository::find_by_ref(self.pool, &action_ref).await? {
                info!(
                    "Action '{}' already exists (ID: {}), skipping",
                    action_ref, existing.id
                );
                result.actions_skipped += 1;
                continue;
            }

            let name = extract_name_from_ref(&action_ref);
            let label = data
                .get("label")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| generate_label(&name));

            let description = data
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let entrypoint = data
                .get("entry_point")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // Resolve runtime ID from runner_type
            let runner_type = data
                .get("runner_type")
                .and_then(|v| v.as_str())
                .unwrap_or("shell");

            let runtime_id = self.resolve_runtime_id(runner_type).await?;

            let param_schema = data
                .get("parameters")
                .and_then(|v| serde_json::to_value(v).ok());

            let out_schema = data
                .get("output")
                .and_then(|v| serde_json::to_value(v).ok());

            // Read optional fields for parameter delivery/format and output format.
            // The database has defaults (stdin, json, text), so we only set these
            // in the INSERT if the YAML specifies them.
            let parameter_delivery = data
                .get("parameter_delivery")
                .and_then(|v| v.as_str())
                .unwrap_or("stdin")
                .to_lowercase();

            let parameter_format = data
                .get("parameter_format")
                .and_then(|v| v.as_str())
                .unwrap_or("json")
                .to_lowercase();

            let output_format = data
                .get("output_format")
                .and_then(|v| v.as_str())
                .unwrap_or("text")
                .to_lowercase();

            // Optional runtime version constraint (e.g., ">=3.12", "~18.0")
            let runtime_version_constraint = data
                .get("runtime_version")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            // Use raw SQL to include parameter_delivery, parameter_format,
            // output_format which are not in CreateActionInput
            let create_result = sqlx::query_scalar::<_, i64>(
                r#"
                INSERT INTO action (
                    ref, pack, pack_ref, label, description, entrypoint,
                    runtime, runtime_version_constraint, param_schema, out_schema, is_adhoc,
                    parameter_delivery, parameter_format, output_format
                )
                VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
                RETURNING id
                "#,
            )
            .bind(&action_ref)
            .bind(self.pack_id)
            .bind(&self.pack_ref)
            .bind(&label)
            .bind(&description)
            .bind(&entrypoint)
            .bind(runtime_id)
            .bind(&runtime_version_constraint)
            .bind(&param_schema)
            .bind(&out_schema)
            .bind(false) // is_adhoc
            .bind(&parameter_delivery)
            .bind(&parameter_format)
            .bind(&output_format)
            .fetch_one(self.pool)
            .await;

            match create_result {
                Ok(id) => {
                    info!("Created action '{}' (ID: {})", action_ref, id);
                    result.actions_loaded += 1;
                }
                Err(e) => {
                    // Check for unique constraint violation (already exists race condition)
                    if let sqlx::Error::Database(ref db_err) = e {
                        if db_err.is_unique_violation() {
                            info!(
                                "Action '{}' already exists (concurrent creation), skipping",
                                action_ref
                            );
                            result.actions_skipped += 1;
                            continue;
                        }
                    }
                    let msg = format!("Failed to create action '{}': {}", action_ref, e);
                    warn!("{}", msg);
                    result.warnings.push(msg);
                }
            }
        }

        Ok(())
    }

    /// Load sensor definitions from `pack_dir/sensors/*.yaml`.
    async fn load_sensors(
        &self,
        pack_dir: &Path,
        trigger_ids: &HashMap<String, Id>,
        result: &mut PackLoadResult,
    ) -> Result<()> {
        let sensors_dir = pack_dir.join("sensors");

        if !sensors_dir.exists() {
            info!("No sensors directory found for pack '{}'", self.pack_ref);
            return Ok(());
        }

        let yaml_files = read_yaml_files(&sensors_dir)?;
        info!(
            "Found {} sensor definition(s) for pack '{}'",
            yaml_files.len(),
            self.pack_ref
        );

        for (filename, content) in &yaml_files {
            let data: serde_yaml_ng::Value = serde_yaml_ng::from_str(content).map_err(|e| {
                Error::validation(format!("Failed to parse sensor YAML {}: {}", filename, e))
            })?;

            // Resolve sensor runtime from YAML runner_type field.
            // Defaults to "native" if not specified (compiled binary, no interpreter).
            let runner_type = data
                .get("runner_type")
                .and_then(|v| v.as_str())
                .unwrap_or("native");
            let (sensor_runtime_id, sensor_runtime_ref) = self.resolve_runtime(runner_type).await?;

            // Validate: if the runner_type suggests an interpreted runtime (not native)
            // but we couldn't resolve it, or it resolved to a runtime with no
            // execution_config, warn at registration time rather than failing
            // opaquely at sensor startup with "Permission denied".
            let is_native_runner = matches!(
                runner_type.to_lowercase().as_str(),
                "native" | "builtin" | "standalone"
            );
            if sensor_runtime_id == 0 && !is_native_runner {
                let msg = format!(
                    "Sensor '{}' declares runner_type '{}' but no matching runtime \
                     was found in the database. The sensor will not be able to start. \
                     Ensure the core pack (with runtimes) is loaded before registering \
                     packs that depend on its runtimes.",
                    filename, runner_type
                );
                warn!("{}", msg);
                result.warnings.push(msg);
            } else if sensor_runtime_id != 0 && !is_native_runner {
                // Verify the resolved runtime has a non-empty execution_config
                if let Some(runtime) =
                    RuntimeRepository::find_by_id(self.pool, sensor_runtime_id).await?
                {
                    let exec_config = runtime.parsed_execution_config();
                    if exec_config.interpreter.binary.is_empty()
                        || exec_config.interpreter.binary == "native"
                        || exec_config.interpreter.binary == "none"
                    {
                        let msg = format!(
                            "Sensor '{}' declares runner_type '{}' (resolved to runtime '{}') \
                             but that runtime has no interpreter configured in its \
                             execution_config. The sensor will fail to start. \
                             Check the runtime definition for '{}'.",
                            filename, runner_type, runtime.r#ref, runtime.r#ref
                        );
                        warn!("{}", msg);
                        result.warnings.push(msg);
                    }
                }
            }

            let sensor_ref = match data.get("ref").and_then(|v| v.as_str()) {
                Some(r) => r.to_string(),
                None => {
                    let msg = format!("Sensor YAML {} missing 'ref' field, skipping", filename);
                    warn!("{}", msg);
                    result.warnings.push(msg);
                    continue;
                }
            };

            let name = extract_name_from_ref(&sensor_ref);
            let label = data
                .get("label")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| generate_label(&name));

            let description = data
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let enabled = data
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);

            let entrypoint = data
                .get("entry_point")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // Resolve trigger reference
            let (trigger_id, trigger_ref) = self.resolve_sensor_trigger(&data, trigger_ids).await;

            let param_schema = data
                .get("parameters")
                .and_then(|v| serde_json::to_value(v).ok());

            let config = data
                .get("config")
                .and_then(|v| serde_json::to_value(v).ok())
                .unwrap_or_else(|| serde_json::json!({}));

            // Optional runtime version constraint (e.g., ">=3.12", "~18.0")
            let runtime_version_constraint = data
                .get("runtime_version")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            // Upsert: update existing sensors so re-registration corrects
            // stale metadata (especially runtime assignments).
            if let Some(existing) = SensorRepository::find_by_ref(self.pool, &sensor_ref).await? {
                use crate::repositories::trigger::UpdateSensorInput;

                let update_input = UpdateSensorInput {
                    label: Some(label),
                    description: Some(description),
                    entrypoint: Some(entrypoint),
                    runtime: Some(sensor_runtime_id),
                    runtime_ref: Some(sensor_runtime_ref.clone()),
                    runtime_version_constraint: Some(runtime_version_constraint.clone()),
                    trigger: Some(trigger_id.unwrap_or(existing.trigger)),
                    trigger_ref: Some(trigger_ref.unwrap_or(existing.trigger_ref.clone())),
                    enabled: Some(enabled),
                    param_schema,
                    config: Some(config),
                };

                match SensorRepository::update(self.pool, existing.id, update_input).await {
                    Ok(_) => {
                        info!(
                            "Updated sensor '{}' (ID: {}, runtime: {} → {})",
                            sensor_ref, existing.id, existing.runtime_ref, sensor_runtime_ref
                        );
                        result.sensors_loaded += 1;
                    }
                    Err(e) => {
                        let msg = format!("Failed to update sensor '{}': {}", sensor_ref, e);
                        warn!("{}", msg);
                        result.warnings.push(msg);
                    }
                }
                continue;
            }

            let input = CreateSensorInput {
                r#ref: sensor_ref.clone(),
                pack: Some(self.pack_id),
                pack_ref: Some(self.pack_ref.clone()),
                label,
                description,
                entrypoint,
                runtime: sensor_runtime_id,
                runtime_ref: sensor_runtime_ref.clone(),
                runtime_version_constraint,
                trigger: trigger_id.unwrap_or(0),
                trigger_ref: trigger_ref.unwrap_or_default(),
                enabled,
                param_schema,
                config: Some(config),
            };

            match SensorRepository::create(self.pool, input).await {
                Ok(sensor) => {
                    info!("Created sensor '{}' (ID: {})", sensor_ref, sensor.id);
                    result.sensors_loaded += 1;
                }
                Err(e) => {
                    let msg = format!("Failed to create sensor '{}': {}", sensor_ref, e);
                    warn!("{}", msg);
                    result.warnings.push(msg);
                }
            }
        }

        Ok(())
    }

    /// Resolve a runtime ID from a runner type string (e.g., "shell", "python", "native").
    ///
    /// Looks up the runtime in the database by `core.{name}` ref pattern,
    /// then falls back to name-based lookup (case-insensitive).
    ///
    /// - "shell" -> "core.shell"
    /// - "python" -> "core.python"
    /// - "node"  -> "core.nodejs"
    /// - "native" -> "core.native"
    async fn resolve_runtime_id(&self, runner_type: &str) -> Result<Option<Id>> {
        let (id, _ref) = self.resolve_runtime(runner_type).await?;
        if id == 0 {
            Ok(None)
        } else {
            Ok(Some(id))
        }
    }

    /// Map a runner_type string to a (runtime_id, runtime_ref) pair.
    ///
    /// Returns `(0, "unknown")` when no matching runtime is found.
    async fn resolve_runtime(&self, runner_type: &str) -> Result<(Id, String)> {
        let runner_lower = runner_type.to_lowercase();

        // Runtime refs use the format `{pack_ref}.{name}` (e.g., "core.python").
        let refs_to_try = match runner_lower.as_str() {
            "shell" | "bash" | "sh" => vec!["core.shell"],
            "python" | "python3" => vec!["core.python"],
            "node" | "nodejs" | "node.js" => vec!["core.nodejs"],
            "native" | "builtin" | "standalone" => vec!["core.native"],
            other => vec![other],
        };

        for runtime_ref in &refs_to_try {
            if let Some(runtime) = RuntimeRepository::find_by_ref(self.pool, runtime_ref).await? {
                return Ok((runtime.id, runtime.r#ref));
            }
        }

        // Fall back to name-based lookup (case-insensitive)
        use crate::repositories::runtime::RuntimeRepository as RR;
        if let Some(runtime) = RR::find_by_name(self.pool, &runner_lower).await? {
            return Ok((runtime.id, runtime.r#ref));
        }

        warn!(
            "Could not find runtime for runner_type '{}', component will have no runtime",
            runner_type
        );
        Ok((0, "unknown".to_string()))
    }

    /// Resolve the trigger reference and ID for a sensor.
    ///
    /// Handles both `trigger_type` (singular) and `trigger_types` (array) fields.
    async fn resolve_sensor_trigger(
        &self,
        data: &serde_yaml_ng::Value,
        trigger_ids: &HashMap<String, Id>,
    ) -> (Option<Id>, Option<String>) {
        // Try trigger_types (array) first, then trigger_type (singular)
        let trigger_type_str = data
            .get("trigger_types")
            .and_then(|v| v.as_sequence())
            .and_then(|seq| seq.first())
            .and_then(|v| v.as_str())
            .or_else(|| data.get("trigger_type").and_then(|v| v.as_str()));

        let trigger_ref = match trigger_type_str {
            Some(t) => {
                if t.contains('.') {
                    t.to_string()
                } else {
                    format!("{}.{}", self.pack_ref, t)
                }
            }
            None => return (None, None),
        };

        // Look up trigger ID from our loaded triggers map first
        if let Some(&id) = trigger_ids.get(&trigger_ref) {
            return (Some(id), Some(trigger_ref));
        }

        // Fall back to database lookup
        match TriggerRepository::find_by_ref(self.pool, &trigger_ref).await {
            Ok(Some(trigger)) => (Some(trigger.id), Some(trigger_ref)),
            _ => {
                warn!("Could not resolve trigger ref '{}' for sensor", trigger_ref);
                (None, Some(trigger_ref))
            }
        }
    }
}

/// Read all `.yaml` and `.yml` files from a directory, sorted by filename.
///
/// Returns a Vec of (filename, content) pairs.
fn read_yaml_files(dir: &Path) -> Result<Vec<(String, String)>> {
    let mut files = Vec::new();

    let entries = std::fs::read_dir(dir)
        .map_err(|e| Error::io(format!("Failed to read directory {}: {}", dir.display(), e)))?;

    let mut paths: Vec<_> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            let path = e.path();
            path.is_file()
                && matches!(
                    path.extension().and_then(|ext| ext.to_str()),
                    Some("yaml") | Some("yml")
                )
        })
        .collect();

    // Sort by filename for deterministic ordering
    paths.sort_by_key(|e| e.file_name());

    for entry in paths {
        let path = entry.path();
        let filename = entry.file_name().to_string_lossy().to_string();

        let content = std::fs::read_to_string(&path)
            .map_err(|e| Error::io(format!("Failed to read file {}: {}", path.display(), e)))?;

        files.push((filename, content));
    }

    Ok(files)
}

/// Extract the short name from a dotted ref (e.g., "core.echo" -> "echo").
fn extract_name_from_ref(r: &str) -> String {
    r.rsplit('.').next().unwrap_or(r).to_string()
}

/// Generate a human-readable label from a snake_case name.
///
/// Examples:
/// - "echo" -> "Echo"
/// - "http_request" -> "Http Request"
/// - "datetime_timer" -> "Datetime Timer"
fn generate_label(name: &str) -> String {
    name.split('_')
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
                    format!("{}{}", upper, chars.as_str())
                }
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_name_from_ref() {
        assert_eq!(extract_name_from_ref("core.echo"), "echo");
        assert_eq!(extract_name_from_ref("python_example.greet"), "greet");
        assert_eq!(extract_name_from_ref("simple"), "simple");
        assert_eq!(extract_name_from_ref("a.b.c"), "c");
    }

    #[test]
    fn test_generate_label() {
        assert_eq!(generate_label("echo"), "Echo");
        assert_eq!(generate_label("http_request"), "Http Request");
        assert_eq!(generate_label("datetime_timer"), "Datetime Timer");
        assert_eq!(generate_label("a_b_c"), "A B C");
    }
}
