//! Proactive Runtime Environment Setup
//!
//! This module provides functions for setting up runtime environments (Python
//! virtualenvs, Node.js node_modules, etc.) proactively — either at worker
//! startup (scanning all registered packs) or in response to a `pack.registered`
//! MQ event.
//!
//! The goal is to ensure environments are ready *before* the first execution,
//! eliminating the first-run penalty and potential permission errors that occur
//! when setup is deferred to execution time.
//!
//! ## Version-Aware Environments
//!
//! When runtime versions are registered (e.g., Python 3.11, 3.12, 3.13), this
//! module creates per-version environments at:
//!   `{runtime_envs_dir}/{pack_ref}/{runtime_name}-{version}`
//!
//! For example: `/opt/attune/runtime_envs/my_pack/python-3.12`
//!
//! This ensures that different versions maintain isolated environments with
//! their own interpreter binaries and installed dependencies. A base (unversioned)
//! environment is also created for backward compatibility with actions that don't
//! declare a version constraint.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use sqlx::PgPool;
use tracing::{debug, error, info, warn};

use attune_common::models::{Runtime, RuntimeVersion, Worker};
use attune_common::mq::PackRegisteredPayload;
use attune_common::repositories::action::ActionRepository;
use attune_common::repositories::pack::PackRepository;
use attune_common::repositories::runtime::RuntimeRepository;
use attune_common::repositories::runtime_version::RuntimeVersionRepository;
use attune_common::repositories::{FindById, FindByRef, List, WorkerRepository};
use attune_common::runtime_detection::{normalize_runtime_name, runtime_aliases_match_filter};
use attune_common::version_matching::matches_constraint;

// Re-export the utility that the API also uses so callers can reach it from
// either crate without adding a direct common dependency for this one function.
pub use attune_common::pack_environment::collect_runtime_names_for_pack;

use crate::runtime::process::ProcessRuntime;

#[derive(Debug, Clone, Default)]
struct RuntimeRequirementProfile {
    any_version: bool,
    constraints: Vec<String>,
}

/// Result of setting up environments for a single pack.
#[derive(Debug)]
pub struct PackEnvSetupResult {
    pub pack_ref: String,
    pub environments_created: Vec<String>,
    pub environments_skipped: Vec<String>,
    pub errors: Vec<String>,
}

/// Result of the full startup scan across all packs.
#[derive(Debug)]
pub struct StartupScanResult {
    pub packs_scanned: usize,
    pub environments_created: usize,
    pub environments_skipped: usize,
    pub errors: Vec<String>,
}

/// Scan all registered packs and create missing runtime environments.
///
/// This is called at worker startup, before the worker begins consuming
/// execution messages. It ensures that environments for all known packs
/// are ready to go.
///
/// # Arguments
/// * `db_pool` - Database connection pool
/// * `runtime_filter` - Optional list of runtime names this worker supports
///   (from `ATTUNE_WORKER_RUNTIMES`). If `None`, all runtimes are considered.
/// * `packs_base_dir` - Base directory where pack files are stored
/// * `runtime_envs_dir` - Base directory for isolated runtime environments
pub async fn scan_and_setup_all_environments(
    db_pool: &PgPool,
    worker_id: i64,
    runtime_filter: Option<&[String]>,
    packs_base_dir: &Path,
    runtime_envs_dir: &Path,
) -> StartupScanResult {
    info!("Starting runtime environment scan for all registered packs");

    let mut result = StartupScanResult {
        packs_scanned: 0,
        environments_created: 0,
        environments_skipped: 0,
        errors: Vec::new(),
    };

    // Load all runtimes from DB, indexed by ID for quick lookup
    let runtimes = match RuntimeRepository::list(db_pool).await {
        Ok(rts) => rts,
        Err(e) => {
            let msg = format!("Failed to load runtimes from database: {}", e);
            error!("{}", msg);
            result.errors.push(msg);
            return result;
        }
    };

    let runtime_map: HashMap<i64, _> = runtimes.into_iter().map(|r| (r.id, r)).collect();
    let worker = load_worker_for_env_setup(db_pool, worker_id).await;

    // Load all packs
    let packs = match PackRepository::list(db_pool).await {
        Ok(p) => p,
        Err(e) => {
            let msg = format!("Failed to load packs from database: {}", e);
            error!("{}", msg);
            result.errors.push(msg);
            return result;
        }
    };

    // Load all runtime versions, indexed by runtime ID
    let version_map: HashMap<i64, Vec<RuntimeVersion>> =
        match RuntimeVersionRepository::list(db_pool).await {
            Ok(versions) => {
                let mut map: HashMap<i64, Vec<RuntimeVersion>> = HashMap::new();
                for v in versions {
                    map.entry(v.runtime).or_default().push(v);
                }
                map
            }
            Err(e) => {
                warn!(
                    "Failed to load runtime versions from database: {}. \
                     Version-specific environments will not be created.",
                    e
                );
                HashMap::new()
            }
        };

    info!("Found {} registered pack(s) to scan", packs.len());

    for pack in &packs {
        result.packs_scanned += 1;

        let pack_result = setup_environments_for_pack(
            db_pool,
            worker.as_ref(),
            &pack.r#ref,
            pack.id,
            runtime_filter,
            packs_base_dir,
            runtime_envs_dir,
            &runtime_map,
            &version_map,
        )
        .await;

        result.environments_created += pack_result.environments_created.len();
        result.environments_skipped += pack_result.environments_skipped.len();
        result.errors.extend(pack_result.errors);
    }

    info!(
        "Environment scan complete: {} pack(s) scanned, {} environment(s) created, \
         {} skipped, {} error(s)",
        result.packs_scanned,
        result.environments_created,
        result.environments_skipped,
        result.errors.len(),
    );

    result
}

/// Set up environments for a single pack, triggered by a `pack.registered` MQ event.
///
/// This is called when the worker receives a `PackRegistered` message. It only
/// sets up environments for the runtimes listed in the event payload (intersection
/// with this worker's supported runtimes).
pub async fn setup_environments_for_registered_pack(
    db_pool: &PgPool,
    worker_id: i64,
    event: &PackRegisteredPayload,
    runtime_filter: Option<&[String]>,
    packs_base_dir: &Path,
    runtime_envs_dir: &Path,
) -> PackEnvSetupResult {
    info!(
        "Setting up environments for newly registered pack '{}' (version {})",
        event.pack_ref, event.version
    );

    let mut pack_result = PackEnvSetupResult {
        pack_ref: event.pack_ref.clone(),
        environments_created: Vec::new(),
        environments_skipped: Vec::new(),
        errors: Vec::new(),
    };
    let worker = load_worker_for_env_setup(db_pool, worker_id).await;

    let pack_dir = packs_base_dir.join(&event.pack_ref);
    if !pack_dir.exists() {
        let msg = format!(
            "Pack directory does not exist: {}. Skipping environment setup.",
            pack_dir.display()
        );
        warn!("{}", msg);
        pack_result.errors.push(msg);
        return pack_result;
    }

    let pack = match PackRepository::find_by_ref(db_pool, &event.pack_ref).await {
        Ok(Some(pack)) => pack,
        Ok(None) => {
            let msg = format!(
                "Pack '{}' not found in database during environment setup",
                event.pack_ref
            );
            warn!("{}", msg);
            pack_result.errors.push(msg);
            return pack_result;
        }
        Err(e) => {
            let msg = format!(
                "Failed to load pack '{}' during environment setup: {}",
                event.pack_ref, e
            );
            warn!("{}", msg);
            pack_result.errors.push(msg);
            return pack_result;
        }
    };

    let runtimes = match RuntimeRepository::list(db_pool).await {
        Ok(rts) => rts,
        Err(e) => {
            let msg = format!("Failed to load runtimes from database: {}", e);
            error!("{}", msg);
            pack_result.errors.push(msg);
            return pack_result;
        }
    };
    let runtime_map: HashMap<i64, _> = runtimes.into_iter().map(|r| (r.id, r)).collect();

    let version_map: HashMap<i64, Vec<RuntimeVersion>> = match RuntimeVersionRepository::list(
        db_pool,
    )
    .await
    {
        Ok(versions) => {
            let mut map: HashMap<i64, Vec<RuntimeVersion>> = HashMap::new();
            for v in versions {
                map.entry(v.runtime).or_default().push(v);
            }
            map
        }
        Err(e) => {
            warn!(
                    "Failed to load runtime versions from database: {}. Version-specific environments will not be created.",
                    e
                );
            HashMap::new()
        }
    };

    setup_environments_for_pack(
        db_pool,
        worker.as_ref(),
        &pack.r#ref,
        pack.id,
        runtime_filter,
        packs_base_dir,
        runtime_envs_dir,
        &runtime_map,
        &version_map,
    )
    .await
}

/// Internal helper: set up environments for a single pack during the startup scan.
///
/// Discovers which runtimes the pack's actions use, filters by this worker's
/// capabilities, and creates any missing environments. Also creates per-version
/// environments for runtimes that have registered versions.
#[allow(clippy::too_many_arguments)]
async fn setup_environments_for_pack(
    db_pool: &PgPool,
    worker: Option<&Worker>,
    pack_ref: &str,
    pack_id: i64,
    runtime_filter: Option<&[String]>,
    packs_base_dir: &Path,
    runtime_envs_dir: &Path,
    runtime_map: &HashMap<i64, attune_common::models::Runtime>,
    version_map: &HashMap<i64, Vec<RuntimeVersion>>,
) -> PackEnvSetupResult {
    let mut pack_result = PackEnvSetupResult {
        pack_ref: pack_ref.to_string(),
        environments_created: Vec::new(),
        environments_skipped: Vec::new(),
        errors: Vec::new(),
    };

    let pack_dir = packs_base_dir.join(pack_ref);
    if !pack_dir.exists() {
        debug!(
            "Pack directory '{}' does not exist on disk, skipping",
            pack_dir.display()
        );
        return pack_result;
    }

    // Get all actions for this pack
    let actions = match ActionRepository::find_by_pack(db_pool, pack_id).await {
        Ok(a) => a,
        Err(e) => {
            let msg = format!("Failed to load actions for pack '{}': {}", pack_ref, e);
            warn!("{}", msg);
            pack_result.errors.push(msg);
            return pack_result;
        }
    };

    let runtime_requirements = collect_runtime_requirements(&actions);
    let seen_runtime_ids: HashSet<i64> = runtime_requirements.keys().copied().collect();

    if seen_runtime_ids.is_empty() {
        debug!("Pack '{}' has no actions with runtimes, skipping", pack_ref);
        return pack_result;
    }

    for runtime_id in seen_runtime_ids {
        let rt = match runtime_map.get(&runtime_id) {
            Some(r) => r,
            None => {
                // Try fetching from DB directly (might be a newly added runtime)
                match RuntimeRepository::find_by_id(db_pool, runtime_id).await {
                    Ok(Some(r)) => {
                        // Can't insert into the borrowed map, so just use it inline
                        let rt_name = r.name.to_lowercase();
                        process_runtime_for_pack(
                            &r,
                            &rt_name,
                            pack_ref,
                            runtime_filter,
                            &pack_dir,
                            packs_base_dir,
                            runtime_envs_dir,
                            &mut pack_result,
                        )
                        .await;
                        // Also set up version-specific environments
                        let versions: Vec<attune_common::models::RuntimeVersion> =
                            RuntimeVersionRepository::find_by_runtime(db_pool, runtime_id)
                                .await
                                .unwrap_or_default();
                        setup_version_environments_from_list(
                            &versions,
                            &r,
                            &rt_name,
                            pack_ref,
                            worker,
                            runtime_requirements.get(&runtime_id),
                            &pack_dir,
                            packs_base_dir,
                            runtime_envs_dir,
                            &mut pack_result,
                        )
                        .await;
                        continue;
                    }
                    Ok(None) => {
                        debug!("Runtime ID {} not found in database, skipping", runtime_id);
                        continue;
                    }
                    Err(e) => {
                        warn!("Failed to load runtime {}: {}", runtime_id, e);
                        continue;
                    }
                }
            }
        };

        let rt_name = rt.name.to_lowercase();
        process_runtime_for_pack(
            rt,
            &rt_name,
            pack_ref,
            runtime_filter,
            &pack_dir,
            packs_base_dir,
            runtime_envs_dir,
            &mut pack_result,
        )
        .await;

        // Set up per-version environments for available versions of this runtime
        if let Some(versions) = version_map.get(&runtime_id) {
            setup_version_environments_from_list(
                versions,
                rt,
                &rt_name,
                pack_ref,
                worker,
                runtime_requirements.get(&runtime_id),
                &pack_dir,
                packs_base_dir,
                runtime_envs_dir,
                &mut pack_result,
            )
            .await;
        }
    }

    if !pack_result.environments_created.is_empty() {
        info!(
            "Pack '{}': created environments for {:?}",
            pack_ref, pack_result.environments_created,
        );
    }

    pack_result
}

/// Process a single runtime for a pack: check filters, check if env exists, create if needed.
#[allow(clippy::too_many_arguments)]
async fn process_runtime_for_pack(
    rt: &attune_common::models::Runtime,
    rt_name: &str,
    pack_ref: &str,
    runtime_filter: Option<&[String]>,
    pack_dir: &Path,
    packs_base_dir: &Path,
    runtime_envs_dir: &Path,
    pack_result: &mut PackEnvSetupResult,
) {
    // Apply worker runtime filter (alias-aware matching via declared aliases)
    if let Some(filter) = runtime_filter {
        if !runtime_aliases_match_filter(&rt.aliases, filter) {
            debug!(
                "Runtime '{}' not in worker filter (aliases: {:?}), skipping for pack '{}'",
                rt_name, rt.aliases, pack_ref,
            );
            return;
        }
    }

    let exec_config = rt.parsed_execution_config();

    // Check if this runtime actually needs an environment
    if exec_config.environment.is_none() && !exec_config.has_dependencies(pack_dir) {
        debug!(
            "Runtime '{}' has no environment config, skipping for pack '{}'",
            rt_name, pack_ref,
        );
        pack_result.environments_skipped.push(rt_name.to_string());
        return;
    }

    let env_dir = runtime_envs_dir.join(pack_ref).join(rt_name);

    // Create a temporary ProcessRuntime to perform the setup
    let process_runtime = ProcessRuntime::new(
        rt_name.to_string(),
        exec_config,
        packs_base_dir.to_path_buf(),
        runtime_envs_dir.to_path_buf(),
    );

    match process_runtime
        .setup_pack_environment(pack_dir, &env_dir)
        .await
    {
        Ok(()) => {
            // setup_pack_environment is idempotent — it logs whether it created
            // the env or found it already existing.
            pack_result.environments_created.push(rt_name.to_string());
        }
        Err(e) => {
            let msg = format!(
                "Failed to set up '{}' environment for pack '{}': {}",
                rt_name, pack_ref, e,
            );
            warn!("{}", msg);
            pack_result.errors.push(msg);
        }
    }
}

/// Set up per-version environments for a runtime, given a list of available versions.
///
/// For each available version, creates an environment at:
///   `{runtime_envs_dir}/{pack_ref}/{runtime_name}-{version}`
///
/// This uses the version's own `execution_config` (which may specify a different
/// interpreter binary, environment create command, etc.).
#[allow(clippy::too_many_arguments)]
async fn setup_version_environments_from_list(
    versions: &[RuntimeVersion],
    runtime: &Runtime,
    rt_name: &str,
    pack_ref: &str,
    worker: Option<&Worker>,
    requirements: Option<&RuntimeRequirementProfile>,
    pack_dir: &Path,
    packs_base_dir: &Path,
    runtime_envs_dir: &Path,
    pack_result: &mut PackEnvSetupResult,
) {
    let versions = filter_versions_for_worker(versions, runtime, worker, requirements);
    if versions.is_empty() {
        return;
    }

    for version in &versions {
        let version_exec_config = version.parsed_execution_config();

        // Skip versions with no environment config and no dependencies
        if version_exec_config.environment.is_none()
            && !version_exec_config.has_dependencies(pack_dir)
        {
            debug!(
                "Version '{}' {} has no environment config, skipping for pack '{}'",
                version.runtime_ref, version.version, pack_ref,
            );
            continue;
        }

        let version_env_suffix = format!("{}-{}", rt_name, version.version);
        let version_env_dir = runtime_envs_dir.join(pack_ref).join(&version_env_suffix);

        let version_runtime = ProcessRuntime::new(
            rt_name.to_string(),
            version_exec_config,
            packs_base_dir.to_path_buf(),
            runtime_envs_dir.to_path_buf(),
        );

        match version_runtime
            .setup_pack_environment(pack_dir, &version_env_dir)
            .await
        {
            Ok(()) => {
                info!(
                    "Version environment '{}' ready for pack '{}'",
                    version_env_suffix, pack_ref,
                );
                pack_result.environments_created.push(version_env_suffix);
            }
            Err(e) => {
                let msg = format!(
                    "Failed to set up version environment '{}' for pack '{}': {}",
                    version_env_suffix, pack_ref, e,
                );
                warn!("{}", msg);
                pack_result.errors.push(msg);
            }
        }
    }
}

async fn load_worker_for_env_setup(db_pool: &PgPool, worker_id: i64) -> Option<Worker> {
    match WorkerRepository::find_by_id(db_pool, worker_id).await {
        Ok(Some(worker)) => Some(worker),
        Ok(None) => {
            warn!(
                "Worker {} not found during environment setup; skipping version-specific filtering",
                worker_id
            );
            None
        }
        Err(e) => {
            warn!(
                "Failed to load worker {} during environment setup: {}. Skipping version-specific filtering",
                worker_id, e
            );
            None
        }
    }
}

fn filter_versions_for_worker(
    versions: &[RuntimeVersion],
    runtime: &Runtime,
    worker: Option<&Worker>,
    requirements: Option<&RuntimeRequirementProfile>,
) -> Vec<RuntimeVersion> {
    let Some(worker) = worker else {
        return Vec::new();
    };

    let advertised_versions = worker_runtime_versions_for_runtime(worker, runtime);
    if advertised_versions.is_empty() {
        return Vec::new();
    }

    versions
        .iter()
        .filter(|version| {
            version_matches_worker(version, &advertised_versions)
                && version_qualifies_for_pack(version, requirements)
        })
        .cloned()
        .collect()
}

fn collect_runtime_requirements(
    actions: &[attune_common::models::action::Action],
) -> HashMap<i64, RuntimeRequirementProfile> {
    let mut requirements = HashMap::new();

    for action in actions {
        let Some(runtime_id) = action.runtime else {
            continue;
        };

        let profile = requirements
            .entry(runtime_id)
            .or_insert_with(RuntimeRequirementProfile::default);

        match action
            .runtime_version_constraint
            .as_deref()
            .map(str::trim)
            .filter(|constraint| !constraint.is_empty())
        {
            Some(constraint) => {
                if !profile
                    .constraints
                    .iter()
                    .any(|existing| existing == constraint)
                {
                    profile.constraints.push(constraint.to_string());
                }
            }
            None => profile.any_version = true,
        }
    }

    requirements
}

fn version_qualifies_for_pack(
    version: &RuntimeVersion,
    requirements: Option<&RuntimeRequirementProfile>,
) -> bool {
    let Some(requirements) = requirements else {
        return false;
    };

    if requirements.any_version || requirements.constraints.is_empty() {
        return true;
    }

    requirements
        .constraints
        .iter()
        .any(|constraint| matches_constraint(&version.version, constraint).unwrap_or(false))
}

fn worker_runtime_versions_for_runtime(worker: &Worker, runtime: &Runtime) -> Vec<String> {
    let mut versions = Vec::new();
    let candidate_runtime_names: Vec<String> = runtime
        .aliases
        .iter()
        .map(|alias| normalize_runtime_name(alias))
        .chain(std::iter::once(normalize_runtime_name(&runtime.name)))
        .collect();

    let Some(capabilities) = worker
        .capabilities
        .as_ref()
        .and_then(|value| value.as_object())
    else {
        return versions;
    };

    if let Some(runtime_versions) = capabilities.get("runtime_versions") {
        if let Some(runtime_versions_obj) = runtime_versions.as_object() {
            for runtime_name in &candidate_runtime_names {
                if let Some(version_values) = runtime_versions_obj.get(runtime_name) {
                    if let Some(version_array) = version_values.as_array() {
                        versions.extend(
                            version_array
                                .iter()
                                .filter_map(|value| value.as_str().map(ToOwned::to_owned)),
                        );
                    }
                }
            }
        }
    }

    if versions.is_empty() {
        if let Some(detected_interpreters) = capabilities.get("detected_interpreters") {
            if let Some(interpreters) = detected_interpreters.as_array() {
                for interpreter in interpreters {
                    let Some(name) = interpreter.get("name").and_then(|value| value.as_str())
                    else {
                        continue;
                    };

                    if !candidate_runtime_names
                        .iter()
                        .any(|candidate| candidate == &normalize_runtime_name(name))
                    {
                        continue;
                    }

                    if let Some(version) =
                        interpreter.get("version").and_then(|value| value.as_str())
                    {
                        versions.push(version.to_string());
                    }
                }
            }
        }
    }

    versions.sort();
    versions.dedup();
    versions
}

fn version_matches_worker(version: &RuntimeVersion, advertised_versions: &[String]) -> bool {
    advertised_versions.iter().any(|advertised_version| {
        advertised_version == &version.version
            || matches_constraint(advertised_version, &version.version).unwrap_or(false)
    })
}

/// Determine the runtime filter from the `ATTUNE_WORKER_RUNTIMES` environment variable.
///
/// Returns `None` if the variable is not set (meaning all runtimes are accepted).
pub fn runtime_filter_from_env() -> Option<Vec<String>> {
    std::env::var("ATTUNE_WORKER_RUNTIMES")
        .ok()
        .map(|val| parse_runtime_filter(&val))
}

/// Parse a comma-separated runtime filter string into a list of lowercase runtime names.
/// Empty entries are filtered out.
fn parse_runtime_filter(val: &str) -> Vec<String> {
    val.split(',')
        .map(|s| s.trim().to_lowercase())
        .filter(|s| !s.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn test_parse_runtime_filter_values() {
        let filter = parse_runtime_filter("shell,Python, Node");
        assert_eq!(filter, vec!["shell", "python", "node"]);
    }

    #[test]
    fn test_parse_runtime_filter_empty() {
        let filter = parse_runtime_filter("");
        assert!(filter.is_empty());
    }

    #[test]
    fn test_parse_runtime_filter_whitespace() {
        let filter = parse_runtime_filter("  shell , , python  ");
        assert_eq!(filter, vec!["shell", "python"]);
    }

    #[test]
    fn test_pack_env_setup_result_defaults() {
        let result = PackEnvSetupResult {
            pack_ref: "test".to_string(),
            environments_created: vec![],
            environments_skipped: vec![],
            errors: vec![],
        };
        assert_eq!(result.pack_ref, "test");
        assert!(result.environments_created.is_empty());
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_startup_scan_result_defaults() {
        let result = StartupScanResult {
            packs_scanned: 0,
            environments_created: 0,
            environments_skipped: 0,
            errors: vec![],
        };
        assert_eq!(result.packs_scanned, 0);
        assert_eq!(result.environments_created, 0);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_filter_versions_for_worker_uses_runtime_versions_capability() {
        let runtime = Runtime {
            id: 1,
            r#ref: "core.python".to_string(),
            pack: None,
            pack_ref: Some("core".to_string()),
            description: None,
            name: "Python".to_string(),
            aliases: vec!["python".to_string(), "python3".to_string()],
            distributions: serde_json::json!({}),
            installation: None,
            installers: serde_json::json!({}),
            execution_config: serde_json::json!({}),
            auto_detected: false,
            detection_config: serde_json::json!({}),
            created: Utc::now(),
            updated: Utc::now(),
        };
        let worker = Worker {
            id: 1,
            name: "worker-1".to_string(),
            worker_type: attune_common::models::WorkerType::Local,
            worker_role: attune_common::models::WorkerRole::Action,
            runtime: None,
            host: None,
            port: None,
            status: Some(attune_common::models::WorkerStatus::Active),
            capabilities: Some(serde_json::json!({
                "runtime_versions": {
                    "python": ["3.12.13", "3.11.9"]
                }
            })),
            meta: None,
            last_heartbeat: None,
            created: Utc::now(),
            updated: Utc::now(),
        };
        let versions = vec![
            RuntimeVersion {
                id: 1,
                runtime: 1,
                runtime_ref: "core.python".to_string(),
                version: "3.12".to_string(),
                version_major: Some(3),
                version_minor: Some(12),
                version_patch: None,
                execution_config: serde_json::json!({}),
                distributions: serde_json::json!({}),
                is_default: false,
                available: false,
                verified_at: None,
                meta: serde_json::json!({}),
                created: Utc::now(),
                updated: Utc::now(),
            },
            RuntimeVersion {
                id: 2,
                runtime: 1,
                runtime_ref: "core.python".to_string(),
                version: "3.13".to_string(),
                version_major: Some(3),
                version_minor: Some(13),
                version_patch: None,
                execution_config: serde_json::json!({}),
                distributions: serde_json::json!({}),
                is_default: false,
                available: true,
                verified_at: None,
                meta: serde_json::json!({}),
                created: Utc::now(),
                updated: Utc::now(),
            },
        ];

        let requirements = RuntimeRequirementProfile {
            any_version: false,
            constraints: vec![">=3.12,<3.13".to_string()],
        };
        let filtered =
            filter_versions_for_worker(&versions, &runtime, Some(&worker), Some(&requirements));

        assert_eq!(
            filtered
                .iter()
                .map(|v| v.version.as_str())
                .collect::<Vec<_>>(),
            vec!["3.12"]
        );
    }

    #[test]
    fn test_filter_versions_for_worker_falls_back_to_detected_interpreters() {
        let runtime = Runtime {
            id: 1,
            r#ref: "core.python".to_string(),
            pack: None,
            pack_ref: Some("core".to_string()),
            description: None,
            name: "Python".to_string(),
            aliases: vec!["python".to_string(), "python3".to_string()],
            distributions: serde_json::json!({}),
            installation: None,
            installers: serde_json::json!({}),
            execution_config: serde_json::json!({}),
            auto_detected: false,
            detection_config: serde_json::json!({}),
            created: Utc::now(),
            updated: Utc::now(),
        };
        let worker = Worker {
            id: 1,
            name: "agent-1".to_string(),
            worker_type: attune_common::models::WorkerType::Local,
            worker_role: attune_common::models::WorkerRole::Action,
            runtime: None,
            host: None,
            port: None,
            status: Some(attune_common::models::WorkerStatus::Active),
            capabilities: Some(serde_json::json!({
                "detected_interpreters": [
                    { "name": "python", "version": "3.12.13", "path": "/usr/bin/python3" }
                ]
            })),
            meta: None,
            last_heartbeat: None,
            created: Utc::now(),
            updated: Utc::now(),
        };
        let versions = vec![RuntimeVersion {
            id: 1,
            runtime: 1,
            runtime_ref: "core.python".to_string(),
            version: "3.12".to_string(),
            version_major: Some(3),
            version_minor: Some(12),
            version_patch: None,
            execution_config: serde_json::json!({}),
            distributions: serde_json::json!({}),
            is_default: false,
            available: false,
            verified_at: None,
            meta: serde_json::json!({}),
            created: Utc::now(),
            updated: Utc::now(),
        }];

        let requirements = RuntimeRequirementProfile {
            any_version: false,
            constraints: vec![">=3.12,<3.13".to_string()],
        };
        let filtered =
            filter_versions_for_worker(&versions, &runtime, Some(&worker), Some(&requirements));

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].version, "3.12");
    }

    #[test]
    fn test_filter_versions_for_worker_returns_empty_without_worker_context() {
        let runtime = Runtime {
            id: 1,
            r#ref: "core.python".to_string(),
            pack: None,
            pack_ref: Some("core".to_string()),
            description: None,
            name: "Python".to_string(),
            aliases: vec!["python".to_string(), "python3".to_string()],
            distributions: serde_json::json!({}),
            installation: None,
            installers: serde_json::json!({}),
            execution_config: serde_json::json!({}),
            auto_detected: false,
            detection_config: serde_json::json!({}),
            created: Utc::now(),
            updated: Utc::now(),
        };
        let versions = vec![RuntimeVersion {
            id: 1,
            runtime: 1,
            runtime_ref: "core.python".to_string(),
            version: "3.12".to_string(),
            version_major: Some(3),
            version_minor: Some(12),
            version_patch: None,
            execution_config: serde_json::json!({}),
            distributions: serde_json::json!({}),
            is_default: false,
            available: true,
            verified_at: None,
            meta: serde_json::json!({}),
            created: Utc::now(),
            updated: Utc::now(),
        }];

        let requirements = RuntimeRequirementProfile {
            any_version: true,
            constraints: Vec::new(),
        };
        let filtered = filter_versions_for_worker(&versions, &runtime, None, Some(&requirements));

        assert!(filtered.is_empty());
    }

    #[test]
    fn test_filter_versions_for_worker_respects_pack_constraints() {
        let runtime = Runtime {
            id: 1,
            r#ref: "core.python".to_string(),
            pack: None,
            pack_ref: Some("core".to_string()),
            description: None,
            name: "Python".to_string(),
            aliases: vec!["python".to_string(), "python3".to_string()],
            distributions: serde_json::json!({}),
            installation: None,
            installers: serde_json::json!({}),
            execution_config: serde_json::json!({}),
            auto_detected: false,
            detection_config: serde_json::json!({}),
            created: Utc::now(),
            updated: Utc::now(),
        };
        let worker = Worker {
            id: 1,
            name: "worker-1".to_string(),
            worker_type: attune_common::models::WorkerType::Local,
            worker_role: attune_common::models::WorkerRole::Action,
            runtime: None,
            host: None,
            port: None,
            status: Some(attune_common::models::WorkerStatus::Active),
            capabilities: Some(serde_json::json!({
                "runtime_versions": {
                    "python": ["3.12.13", "3.13.2"]
                }
            })),
            meta: None,
            last_heartbeat: None,
            created: Utc::now(),
            updated: Utc::now(),
        };
        let versions = vec![
            RuntimeVersion {
                id: 1,
                runtime: 1,
                runtime_ref: "core.python".to_string(),
                version: "3.12".to_string(),
                version_major: Some(3),
                version_minor: Some(12),
                version_patch: None,
                execution_config: serde_json::json!({}),
                distributions: serde_json::json!({}),
                is_default: false,
                available: true,
                verified_at: None,
                meta: serde_json::json!({}),
                created: Utc::now(),
                updated: Utc::now(),
            },
            RuntimeVersion {
                id: 2,
                runtime: 1,
                runtime_ref: "core.python".to_string(),
                version: "3.13".to_string(),
                version_major: Some(3),
                version_minor: Some(13),
                version_patch: None,
                execution_config: serde_json::json!({}),
                distributions: serde_json::json!({}),
                is_default: false,
                available: true,
                verified_at: None,
                meta: serde_json::json!({}),
                created: Utc::now(),
                updated: Utc::now(),
            },
        ];
        let requirements = RuntimeRequirementProfile {
            any_version: false,
            constraints: vec!["~3.12".to_string()],
        };

        let filtered =
            filter_versions_for_worker(&versions, &runtime, Some(&worker), Some(&requirements));

        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].version, "3.12");
    }
}
