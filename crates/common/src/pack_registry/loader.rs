//! Pack Component Loader
//!
//! Reads runtime, action, trigger, and sensor YAML definitions from a pack directory
//! and registers them in the database. This is the Rust-native equivalent of
//! the Python `load_core_pack.py` script used during init-packs.
//!
//! Components are loaded in dependency order:
//! 1. Runtimes (no dependencies)
//! 2. Triggers (no dependencies)
//! 3. Actions (depend on runtime; workflow actions also create workflow_definition records)
//! 4. Sensors (depend on triggers and runtime)
//!
//! All loaders use **upsert** semantics: if an entity with the same ref already
//! exists it is updated in place (preserving its database ID); otherwise a new
//! row is created. After loading, entities that belong to the pack but whose
//! refs are no longer present in the YAML files are deleted.
//!
//! ## Workflow Actions
//!
//! An action YAML may include a `workflow_file` field pointing to a workflow
//! definition file relative to the `actions/` directory (e.g.,
//! `workflow_file: workflows/deploy.workflow.yaml`). When present the loader:
//!
//! 1. Reads and parses the referenced workflow YAML file.
//! 2. Creates or updates a `workflow_definition` record in the database.
//! 3. Creates the action record with `workflow_def` linked to the definition.
//!
//! This allows the action YAML to control action-level metadata (ref, label,
//! parameters, policies) independently of the workflow graph. Multiple actions
//! can reference the same workflow file with different configurations.

use std::collections::HashMap;
use std::path::Path;

use sqlx::PgPool;
use tracing::{debug, info, warn};

use crate::error::{Error, Result};
use crate::models::Id;
use crate::repositories::action::{ActionRepository, UpdateActionInput};
use crate::repositories::runtime::{CreateRuntimeInput, RuntimeRepository, UpdateRuntimeInput};
use crate::repositories::runtime_version::{
    CreateRuntimeVersionInput, RuntimeVersionRepository, UpdateRuntimeVersionInput,
};
use crate::repositories::trigger::{
    CreateSensorInput, CreateTriggerInput, SensorRepository, TriggerRepository, UpdateSensorInput,
    UpdateTriggerInput,
};
use crate::repositories::workflow::{
    CreateWorkflowDefinitionInput, UpdateWorkflowDefinitionInput, WorkflowDefinitionRepository,
};
use crate::repositories::{Create, Delete, FindById, FindByRef, Update};
use crate::version_matching::extract_version_components;
use crate::workflow::parser::parse_workflow_yaml;

/// Result of loading pack components into the database.
#[derive(Debug, Default)]
pub struct PackLoadResult {
    /// Number of runtimes created
    pub runtimes_loaded: usize,
    /// Number of runtimes updated (already existed)
    pub runtimes_updated: usize,
    /// Number of runtimes skipped due to errors
    pub runtimes_skipped: usize,
    /// Number of triggers created
    pub triggers_loaded: usize,
    /// Number of triggers updated
    pub triggers_updated: usize,
    /// Number of triggers skipped
    pub triggers_skipped: usize,
    /// Number of actions created
    pub actions_loaded: usize,
    /// Number of actions updated
    pub actions_updated: usize,
    /// Number of actions skipped
    pub actions_skipped: usize,
    /// Number of sensors created
    pub sensors_loaded: usize,
    /// Number of sensors updated
    pub sensors_updated: usize,
    /// Number of sensors skipped
    pub sensors_skipped: usize,
    /// Number of stale entities removed
    pub removed: usize,
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

    pub fn total_updated(&self) -> usize {
        self.runtimes_updated + self.triggers_updated + self.actions_updated + self.sensors_updated
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
    /// Uses upsert semantics: entities that already exist (by ref) are updated
    /// in place, preserving their database IDs. New entities are created.
    /// After loading, entities that belong to the pack but are no longer
    /// present in the YAML files are removed.
    pub async fn load_all(&self, pack_dir: &Path) -> Result<PackLoadResult> {
        let mut result = PackLoadResult::default();

        info!(
            "Loading components for pack '{}' from {}",
            self.pack_ref,
            pack_dir.display()
        );

        // 1. Load runtimes first (no dependencies)
        let runtime_refs = self.load_runtimes(pack_dir, &mut result).await?;

        // 2. Load triggers (no dependencies)
        let (trigger_ids, trigger_refs) = self.load_triggers(pack_dir, &mut result).await?;

        // 3. Load actions (depend on runtime)
        let action_refs = self.load_actions(pack_dir, &mut result).await?;

        // 4. Load sensors (depend on triggers and runtime)
        let sensor_refs = self
            .load_sensors(pack_dir, &trigger_ids, &mut result)
            .await?;

        // 5. Clean up entities that are no longer in the pack's YAML files
        self.cleanup_removed_entities(
            &runtime_refs,
            &trigger_refs,
            &action_refs,
            &sensor_refs,
            &mut result,
        )
        .await;

        info!(
            "Pack '{}' component loading complete: {} created, {} updated, {} skipped, {} removed, {} warnings",
            self.pack_ref,
            result.total_loaded(),
            result.total_updated(),
            result.total_skipped(),
            result.removed,
            result.warnings.len()
        );

        Ok(result)
    }

    /// Load runtime definitions from `pack_dir/runtimes/*.yaml`.
    ///
    /// Runtimes define how actions and sensors are executed (interpreter,
    /// environment setup, dependency management). They are loaded first
    /// since actions reference them.
    ///
    /// Returns the set of runtime refs that were loaded (for cleanup).
    async fn load_runtimes(
        &self,
        pack_dir: &Path,
        result: &mut PackLoadResult,
    ) -> Result<Vec<String>> {
        let runtimes_dir = pack_dir.join("runtimes");
        let mut loaded_refs = Vec::new();

        if !runtimes_dir.exists() {
            info!("No runtimes directory found for pack '{}'", self.pack_ref);
            return Ok(loaded_refs);
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

            // Check if runtime already exists — update in place if so
            if let Some(existing) = RuntimeRepository::find_by_ref(self.pool, &runtime_ref).await? {
                let update_input = UpdateRuntimeInput {
                    description,
                    name: Some(name),
                    distributions: Some(distributions),
                    installation,
                    execution_config: Some(execution_config),
                };

                match RuntimeRepository::update(self.pool, existing.id, update_input).await {
                    Ok(_) => {
                        info!("Updated runtime '{}' (ID: {})", runtime_ref, existing.id);
                        result.runtimes_updated += 1;

                        // Also upsert version entries
                        self.load_runtime_versions(&data, existing.id, &runtime_ref, result)
                            .await;
                    }
                    Err(e) => {
                        let msg = format!("Failed to update runtime '{}': {}", runtime_ref, e);
                        warn!("{}", msg);
                        result.warnings.push(msg);
                    }
                }
                loaded_refs.push(runtime_ref);
                continue;
            }

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
                    loaded_refs.push(runtime_ref.clone());

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
                                    "Runtime '{}' already exists (concurrent creation), treating as update",
                                    runtime_ref
                                );
                                loaded_refs.push(runtime_ref);
                                result.runtimes_updated += 1;
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

        Ok(loaded_refs)
    }

    /// Load runtime version entries from a runtime's YAML `versions` array.
    ///
    /// Uses upsert: existing versions (by runtime + version string) are updated,
    /// new versions are created.
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

        // Collect version strings we loaded so we can clean up removed versions
        let mut loaded_versions = Vec::new();

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

            // Check if this version already exists — update in place if so
            if let Ok(Some(existing)) = RuntimeVersionRepository::find_by_runtime_and_version(
                self.pool,
                runtime_id,
                &version_str,
            )
            .await
            {
                let update_input = UpdateRuntimeVersionInput {
                    version: None, // version string doesn't change
                    version_major: Some(version_major),
                    version_minor: Some(version_minor),
                    version_patch: Some(version_patch),
                    execution_config: Some(execution_config),
                    distributions: Some(distributions),
                    is_default: Some(is_default),
                    available: None, // preserve current availability — verification sets this
                    verified_at: None,
                    meta: Some(meta),
                };

                match RuntimeVersionRepository::update(self.pool, existing.id, update_input).await {
                    Ok(_) => {
                        info!(
                            "Updated version '{}' for runtime '{}' (ID: {})",
                            version_str, runtime_ref, existing.id
                        );
                    }
                    Err(e) => {
                        let msg = format!(
                            "Failed to update version '{}' for runtime '{}': {}",
                            version_str, runtime_ref, e
                        );
                        warn!("{}", msg);
                        result.warnings.push(msg);
                    }
                }
                loaded_versions.push(version_str);
                continue;
            }

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
                    loaded_versions.push(version_str);
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
                                loaded_versions.push(version_str);
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

        // Clean up versions that are no longer in the YAML
        if let Ok(existing_versions) =
            RuntimeVersionRepository::find_by_runtime(self.pool, runtime_id).await
        {
            for existing in existing_versions {
                if !loaded_versions.contains(&existing.version) {
                    info!(
                        "Removing stale version '{}' for runtime '{}'",
                        existing.version, runtime_ref
                    );
                    if let Err(e) = RuntimeVersionRepository::delete(self.pool, existing.id).await {
                        warn!(
                            "Failed to delete stale version '{}' for runtime '{}': {}",
                            existing.version, runtime_ref, e
                        );
                    }
                }
            }
        }
    }

    /// Load trigger definitions from `pack_dir/triggers/*.yaml`.
    ///
    /// Returns a map of trigger ref -> trigger ID for use by sensor loading,
    /// and the list of loaded trigger refs for cleanup.
    async fn load_triggers(
        &self,
        pack_dir: &Path,
        result: &mut PackLoadResult,
    ) -> Result<(HashMap<String, Id>, Vec<String>)> {
        let triggers_dir = pack_dir.join("triggers");
        let mut trigger_ids = HashMap::new();
        let mut loaded_refs = Vec::new();

        if !triggers_dir.exists() {
            info!("No triggers directory found for pack '{}'", self.pack_ref);
            return Ok((trigger_ids, loaded_refs));
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

            // Check if trigger already exists — update in place if so
            if let Some(existing) = TriggerRepository::find_by_ref(self.pool, &trigger_ref).await? {
                let update_input = UpdateTriggerInput {
                    label: Some(label),
                    description: Some(description),
                    enabled: Some(enabled),
                    param_schema,
                    out_schema,
                };

                match TriggerRepository::update(self.pool, existing.id, update_input).await {
                    Ok(_) => {
                        info!("Updated trigger '{}' (ID: {})", trigger_ref, existing.id);
                        result.triggers_updated += 1;
                    }
                    Err(e) => {
                        let msg = format!("Failed to update trigger '{}': {}", trigger_ref, e);
                        warn!("{}", msg);
                        result.warnings.push(msg);
                    }
                }
                trigger_ids.insert(trigger_ref.clone(), existing.id);
                loaded_refs.push(trigger_ref);
                continue;
            }

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
                    trigger_ids.insert(trigger_ref.clone(), trigger.id);
                    loaded_refs.push(trigger_ref);
                    result.triggers_loaded += 1;
                }
                Err(e) => {
                    let msg = format!("Failed to create trigger '{}': {}", trigger_ref, e);
                    warn!("{}", msg);
                    result.warnings.push(msg);
                }
            }
        }

        Ok((trigger_ids, loaded_refs))
    }

    /// Load action definitions from `pack_dir/actions/*.yaml`.
    ///
    /// Returns the list of loaded action refs for cleanup.
    ///
    /// When an action YAML contains a `workflow_file` field, the loader reads
    /// the referenced workflow definition, creates/updates a
    /// `workflow_definition` record, and links the action to it via the
    /// `action.workflow_def` FK. This enables the action YAML to control
    /// action-level metadata independently of the workflow graph, and allows
    /// multiple actions to share the same workflow file.
    async fn load_actions(
        &self,
        pack_dir: &Path,
        result: &mut PackLoadResult,
    ) -> Result<Vec<String>> {
        let actions_dir = pack_dir.join("actions");
        let mut loaded_refs = Vec::new();

        if !actions_dir.exists() {
            info!("No actions directory found for pack '{}'", self.pack_ref);
            return Ok(loaded_refs);
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

            // ── Workflow file handling ──────────────────────────────────
            // If the action declares `workflow_file`, load the referenced
            // workflow definition and link the action to it.
            let workflow_file_field = data
                .get("workflow_file")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            let workflow_def_id: Option<Id> = if let Some(ref wf_path) = workflow_file_field {
                match self
                    .load_workflow_for_action(
                        &actions_dir,
                        wf_path,
                        &action_ref,
                        &label,
                        &description,
                        &data,
                    )
                    .await
                {
                    Ok(id) => Some(id),
                    Err(e) => {
                        let msg = format!(
                            "Failed to load workflow file '{}' for action '{}': {}",
                            wf_path, action_ref, e
                        );
                        warn!("{}", msg);
                        result.warnings.push(msg);
                        // Continue creating the action without workflow link
                        None
                    }
                }
            } else {
                None
            };

            // For workflow actions the entrypoint is the workflow file path;
            // for regular actions it comes from entry_point in the YAML.
            let entrypoint = if let Some(ref wf_path) = workflow_file_field {
                wf_path.clone()
            } else {
                data.get("entry_point")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string()
            };

            // Resolve runtime ID from runner_type (workflow actions have no
            // runner_type and get runtime = None).
            let runtime_id = if workflow_file_field.is_some() {
                None
            } else {
                let runner_type = data
                    .get("runner_type")
                    .and_then(|v| v.as_str())
                    .unwrap_or("shell");
                self.resolve_runtime_id(runner_type).await?
            };

            let param_schema = data
                .get("parameters")
                .and_then(|v| serde_json::to_value(v).ok());

            let out_schema = data
                .get("output")
                .and_then(|v| serde_json::to_value(v).ok());

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

            // Check if action already exists — update in place if so
            if let Some(existing) = ActionRepository::find_by_ref(self.pool, &action_ref).await? {
                let update_input = UpdateActionInput {
                    label: Some(label),
                    description: Some(description),
                    entrypoint: Some(entrypoint),
                    runtime: runtime_id,
                    runtime_version_constraint: Some(runtime_version_constraint),
                    param_schema,
                    out_schema,
                    parameter_delivery: Some(parameter_delivery),
                    parameter_format: Some(parameter_format),
                    output_format: Some(output_format),
                };

                match ActionRepository::update(self.pool, existing.id, update_input).await {
                    Ok(_) => {
                        info!("Updated action '{}' (ID: {})", action_ref, existing.id);
                        result.actions_updated += 1;

                        // Re-link workflow definition if present
                        if let Some(wf_id) = workflow_def_id {
                            if let Err(e) =
                                ActionRepository::link_workflow_def(self.pool, existing.id, wf_id)
                                    .await
                            {
                                warn!(
                                    "Failed to link workflow def {} to action '{}': {}",
                                    wf_id, action_ref, e
                                );
                            }
                        }
                    }
                    Err(e) => {
                        let msg = format!("Failed to update action '{}': {}", action_ref, e);
                        warn!("{}", msg);
                        result.warnings.push(msg);
                    }
                }
                loaded_refs.push(action_ref);
                continue;
            }

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
                    loaded_refs.push(action_ref.clone());
                    result.actions_loaded += 1;

                    // Link workflow definition if present
                    if let Some(wf_id) = workflow_def_id {
                        if let Err(e) =
                            ActionRepository::link_workflow_def(self.pool, id, wf_id).await
                        {
                            warn!(
                                "Failed to link workflow def {} to new action '{}': {}",
                                wf_id, action_ref, e
                            );
                        } else {
                            info!(
                                "Linked action '{}' (ID: {}) to workflow definition (ID: {})",
                                action_ref, id, wf_id
                            );
                        }
                    }
                }
                Err(e) => {
                    // Check for unique constraint violation (already exists race condition)
                    if let sqlx::Error::Database(ref db_err) = e {
                        if db_err.is_unique_violation() {
                            info!(
                                "Action '{}' already exists (concurrent creation), treating as update",
                                action_ref
                            );
                            loaded_refs.push(action_ref);
                            result.actions_updated += 1;
                            continue;
                        }
                    }
                    let msg = format!("Failed to create action '{}': {}", action_ref, e);
                    warn!("{}", msg);
                    result.warnings.push(msg);
                }
            }
        }

        Ok(loaded_refs)
    }

    /// Load a workflow definition file referenced by an action's `workflow_file`
    /// field and create/update the corresponding `workflow_definition` record.
    ///
    /// Returns the database ID of the workflow definition.
    async fn load_workflow_for_action(
        &self,
        actions_dir: &Path,
        workflow_file_path: &str,
        action_ref: &str,
        action_label: &str,
        action_description: &str,
        action_data: &serde_yaml_ng::Value,
    ) -> Result<Id> {
        let full_path = actions_dir.join(workflow_file_path);
        if !full_path.exists() {
            return Err(Error::validation(format!(
                "Workflow file '{}' not found at '{}'",
                workflow_file_path,
                full_path.display()
            )));
        }

        let content = std::fs::read_to_string(&full_path).map_err(|e| {
            Error::io(format!(
                "Failed to read workflow file '{}': {}",
                full_path.display(),
                e
            ))
        })?;

        let mut workflow_yaml = parse_workflow_yaml(&content)?;

        // The action YAML is authoritative for action-level metadata.
        // Fill in ref/label/description/tags from the action when the
        // workflow file omits them (action-linked workflow files should
        // contain only the execution graph).
        if workflow_yaml.r#ref.is_empty() {
            workflow_yaml.r#ref = action_ref.to_string();
        }
        if workflow_yaml.label.is_empty() {
            workflow_yaml.label = action_label.to_string();
        }
        if workflow_yaml.description.is_none() {
            workflow_yaml.description = Some(action_description.to_string());
        }
        if workflow_yaml.tags.is_empty() {
            if let Some(tags_val) = action_data.get("tags") {
                if let Some(tags_seq) = tags_val.as_sequence() {
                    workflow_yaml.tags = tags_seq
                        .iter()
                        .filter_map(|v| v.as_str().map(|s| s.to_string()))
                        .collect();
                }
            }
        }

        let workflow_ref = workflow_yaml.r#ref.clone();

        // The action YAML is authoritative for param_schema / out_schema.
        // Fall back to the workflow file's own schemas only if the action
        // YAML doesn't define them.
        let param_schema = action_data
            .get("parameters")
            .and_then(|v| serde_json::to_value(v).ok())
            .or_else(|| workflow_yaml.parameters.clone());

        let out_schema = action_data
            .get("output")
            .and_then(|v| serde_json::to_value(v).ok())
            .or_else(|| workflow_yaml.output.clone());

        let definition_json = serde_json::to_value(&workflow_yaml)
            .map_err(|e| Error::validation(format!("Failed to serialize workflow: {}", e)))?;

        // Derive label/description for the DB record from the action YAML,
        // since it is authoritative. The workflow file values were already
        // used as fallback above when populating workflow_yaml.
        let label = workflow_yaml.label.clone();
        let description = workflow_yaml.description.clone();
        let tags = workflow_yaml.tags.clone();

        // Check if this workflow definition already exists
        if let Some(existing) =
            WorkflowDefinitionRepository::find_by_ref(self.pool, &workflow_ref).await?
        {
            debug!(
                "Updating existing workflow definition '{}' (ID: {})",
                workflow_ref, existing.id
            );

            let update_input = UpdateWorkflowDefinitionInput {
                label: Some(label),
                description,
                version: Some(workflow_yaml.version.clone()),
                param_schema,
                out_schema,
                definition: Some(definition_json),
                tags: Some(tags),
                enabled: Some(true),
            };

            WorkflowDefinitionRepository::update(self.pool, existing.id, update_input).await?;

            info!(
                "Updated workflow definition '{}' (ID: {}) for action '{}'",
                workflow_ref, existing.id, action_ref
            );

            Ok(existing.id)
        } else {
            debug!(
                "Creating new workflow definition '{}' for action '{}'",
                workflow_ref, action_ref
            );

            let create_input = CreateWorkflowDefinitionInput {
                r#ref: workflow_ref.clone(),
                pack: self.pack_id,
                pack_ref: self.pack_ref.clone(),
                label,
                description,
                version: workflow_yaml.version.clone(),
                param_schema,
                out_schema,
                definition: definition_json,
                tags,
                enabled: true,
            };

            let created = WorkflowDefinitionRepository::create(self.pool, create_input).await?;

            info!(
                "Created workflow definition '{}' (ID: {}) for action '{}'",
                workflow_ref, created.id, action_ref
            );

            Ok(created.id)
        }
    }

    /// Load sensor definitions from `pack_dir/sensors/*.yaml`.
    ///
    /// Returns the list of loaded sensor refs for cleanup.
    async fn load_sensors(
        &self,
        pack_dir: &Path,
        trigger_ids: &HashMap<String, Id>,
        result: &mut PackLoadResult,
    ) -> Result<Vec<String>> {
        let sensors_dir = pack_dir.join("sensors");
        let mut loaded_refs = Vec::new();

        if !sensors_dir.exists() {
            info!("No sensors directory found for pack '{}'", self.pack_ref);
            return Ok(loaded_refs);
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
                        result.sensors_updated += 1;
                    }
                    Err(e) => {
                        let msg = format!("Failed to update sensor '{}': {}", sensor_ref, e);
                        warn!("{}", msg);
                        result.warnings.push(msg);
                    }
                }
                loaded_refs.push(sensor_ref);
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
                    loaded_refs.push(sensor_ref);
                    result.sensors_loaded += 1;
                }
                Err(e) => {
                    let msg = format!("Failed to create sensor '{}': {}", sensor_ref, e);
                    warn!("{}", msg);
                    result.warnings.push(msg);
                }
            }
        }

        Ok(loaded_refs)
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

    /// Remove entities that belong to this pack but whose refs are no longer
    /// present in the pack's YAML files.
    ///
    /// This handles the case where an action/trigger/sensor/runtime was removed
    /// from the pack between versions. Ad-hoc (user-created) entities are never
    /// removed.
    async fn cleanup_removed_entities(
        &self,
        runtime_refs: &[String],
        trigger_refs: &[String],
        action_refs: &[String],
        sensor_refs: &[String],
        result: &mut PackLoadResult,
    ) {
        // Clean up sensors first (they depend on triggers/runtimes)
        match SensorRepository::delete_by_pack_excluding(self.pool, self.pack_id, sensor_refs).await
        {
            Ok(count) => {
                if count > 0 {
                    info!(
                        "Removed {} stale sensor(s) from pack '{}'",
                        count, self.pack_ref
                    );
                    result.removed += count as usize;
                }
            }
            Err(e) => {
                warn!(
                    "Failed to clean up stale sensors for pack '{}': {}",
                    self.pack_ref, e
                );
            }
        }

        // Clean up actions (ad-hoc preserved)
        match ActionRepository::delete_non_adhoc_by_pack_excluding(
            self.pool,
            self.pack_id,
            action_refs,
        )
        .await
        {
            Ok(count) => {
                if count > 0 {
                    info!(
                        "Removed {} stale action(s) from pack '{}'",
                        count, self.pack_ref
                    );
                    result.removed += count as usize;
                }
            }
            Err(e) => {
                warn!(
                    "Failed to clean up stale actions for pack '{}': {}",
                    self.pack_ref, e
                );
            }
        }

        // Clean up triggers (ad-hoc preserved)
        match TriggerRepository::delete_non_adhoc_by_pack_excluding(
            self.pool,
            self.pack_id,
            trigger_refs,
        )
        .await
        {
            Ok(count) => {
                if count > 0 {
                    info!(
                        "Removed {} stale trigger(s) from pack '{}'",
                        count, self.pack_ref
                    );
                    result.removed += count as usize;
                }
            }
            Err(e) => {
                warn!(
                    "Failed to clean up stale triggers for pack '{}': {}",
                    self.pack_ref, e
                );
            }
        }

        // Clean up runtimes last (actions/sensors may reference them)
        match RuntimeRepository::delete_by_pack_excluding(self.pool, self.pack_id, runtime_refs)
            .await
        {
            Ok(count) => {
                if count > 0 {
                    info!(
                        "Removed {} stale runtime(s) from pack '{}'",
                        count, self.pack_ref
                    );
                    result.removed += count as usize;
                }
            }
            Err(e) => {
                warn!(
                    "Failed to clean up stale runtimes for pack '{}': {}",
                    self.pack_ref, e
                );
            }
        }
    }
}

/// Read all YAML files from a directory, returning `(filename, content)` pairs
/// sorted by filename for deterministic ordering.
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
