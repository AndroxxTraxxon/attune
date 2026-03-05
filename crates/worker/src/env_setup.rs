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

use attune_common::models::RuntimeVersion;
use attune_common::mq::PackRegisteredPayload;
use attune_common::repositories::action::ActionRepository;
use attune_common::repositories::pack::PackRepository;
use attune_common::repositories::runtime::RuntimeRepository;
use attune_common::repositories::runtime_version::RuntimeVersionRepository;
use attune_common::repositories::{FindById, List};
use attune_common::runtime_detection::runtime_in_filter;

// Re-export the utility that the API also uses so callers can reach it from
// either crate without adding a direct common dependency for this one function.
pub use attune_common::pack_environment::collect_runtime_names_for_pack;

use crate::runtime::process::ProcessRuntime;

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

    // Filter to runtimes this worker supports (alias-aware matching)
    let target_runtimes: Vec<&String> = event
        .runtime_names
        .iter()
        .filter(|name| {
            if let Some(filter) = runtime_filter {
                runtime_in_filter(name, filter)
            } else {
                true
            }
        })
        .collect();

    if target_runtimes.is_empty() {
        debug!(
            "No matching runtimes for pack '{}' on this worker (event runtimes: {:?}, worker filter: {:?})",
            event.pack_ref, event.runtime_names, runtime_filter,
        );
        return pack_result;
    }

    // Load runtime configs from DB by name
    let all_runtimes = match RuntimeRepository::list(db_pool).await {
        Ok(rts) => rts,
        Err(e) => {
            let msg = format!("Failed to load runtimes from database: {}", e);
            error!("{}", msg);
            pack_result.errors.push(msg);
            return pack_result;
        }
    };

    for rt_name in target_runtimes {
        // Find the runtime in DB (match by lowercase name)
        let rt = match all_runtimes
            .iter()
            .find(|r| r.name.to_lowercase() == *rt_name)
        {
            Some(r) => r,
            None => {
                debug!("Runtime '{}' not found in database, skipping", rt_name);
                continue;
            }
        };

        let exec_config = rt.parsed_execution_config();
        if exec_config.environment.is_none() && !exec_config.has_dependencies(&pack_dir) {
            debug!(
                "Runtime '{}' has no environment config, skipping for pack '{}'",
                rt_name, event.pack_ref,
            );
            pack_result.environments_skipped.push(rt_name.clone());
            continue;
        }

        // Set up base (unversioned) environment
        let env_dir = runtime_envs_dir.join(&event.pack_ref).join(rt_name);

        let process_runtime = ProcessRuntime::new(
            rt_name.clone(),
            exec_config,
            packs_base_dir.to_path_buf(),
            runtime_envs_dir.to_path_buf(),
        );

        match process_runtime
            .setup_pack_environment(&pack_dir, &env_dir)
            .await
        {
            Ok(()) => {
                info!(
                    "Environment for runtime '{}' ready for pack '{}'",
                    rt_name, event.pack_ref,
                );
                pack_result.environments_created.push(rt_name.clone());
            }
            Err(e) => {
                let msg = format!(
                    "Failed to set up '{}' environment for pack '{}': {}",
                    rt_name, event.pack_ref, e,
                );
                warn!("{}", msg);
                pack_result.errors.push(msg);
            }
        }

        // Set up per-version environments for available runtime versions
        setup_version_environments(
            db_pool,
            rt.id,
            rt_name,
            &event.pack_ref,
            &pack_dir,
            packs_base_dir,
            runtime_envs_dir,
            &mut pack_result,
        )
        .await;
    }

    pack_result
}

/// Internal helper: set up environments for a single pack during the startup scan.
///
/// Discovers which runtimes the pack's actions use, filters by this worker's
/// capabilities, and creates any missing environments. Also creates per-version
/// environments for runtimes that have registered versions.
#[allow(clippy::too_many_arguments)]
async fn setup_environments_for_pack(
    db_pool: &PgPool,
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

    // Collect unique runtime IDs referenced by actions in this pack
    let mut seen_runtime_ids = HashSet::new();
    for action in &actions {
        if let Some(runtime_id) = action.runtime {
            seen_runtime_ids.insert(runtime_id);
        }
    }

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
                            RuntimeVersionRepository::find_available_by_runtime(
                                db_pool, runtime_id,
                            )
                            .await
                            .unwrap_or_default();
                        setup_version_environments_from_list(
                            &versions,
                            &rt_name,
                            pack_ref,
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
            let available_versions: Vec<RuntimeVersion> =
                versions.iter().filter(|v| v.available).cloned().collect();
            setup_version_environments_from_list(
                &available_versions,
                &rt_name,
                pack_ref,
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
    // Apply worker runtime filter (alias-aware matching)
    if let Some(filter) = runtime_filter {
        if !runtime_in_filter(rt_name, filter) {
            debug!(
                "Runtime '{}' not in worker filter, skipping for pack '{}'",
                rt_name, pack_ref,
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
    rt_name: &str,
    pack_ref: &str,
    pack_dir: &Path,
    packs_base_dir: &Path,
    runtime_envs_dir: &Path,
    pack_result: &mut PackEnvSetupResult,
) {
    if versions.is_empty() {
        return;
    }

    for version in versions {
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

/// Set up per-version environments for a runtime by querying the database.
///
/// This is a convenience wrapper around `setup_version_environments_from_list`
/// that queries available versions from the database first. Used in the
/// pack.registered event handler where we don't have a pre-loaded version map.
#[allow(clippy::too_many_arguments)]
async fn setup_version_environments(
    db_pool: &PgPool,
    runtime_id: i64,
    rt_name: &str,
    pack_ref: &str,
    pack_dir: &Path,
    packs_base_dir: &Path,
    runtime_envs_dir: &Path,
    pack_result: &mut PackEnvSetupResult,
) {
    let versions =
        match RuntimeVersionRepository::find_available_by_runtime(db_pool, runtime_id).await {
            Ok(v) => v,
            Err(e) => {
                debug!(
                    "Failed to load versions for runtime '{}' (id {}): {}. \
                 Skipping version-specific environments.",
                    rt_name, runtime_id, e,
                );
                return;
            }
        };

    setup_version_environments_from_list(
        &versions,
        rt_name,
        pack_ref,
        pack_dir,
        packs_base_dir,
        runtime_envs_dir,
        pack_result,
    )
    .await;
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
}
