//! Pack registry index management utilities

use crate::output::{self, OutputFormat};
use anyhow::Result;
use attune_common::pack_registry::calculate_directory_checksum;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Update a registry index file with a new pack entry
pub async fn handle_index_update(
    index_path: String,
    pack_path: String,
    git_url: Option<String>,
    git_ref: Option<String>,
    archive_url: Option<String>,
    update: bool,
    output_format: OutputFormat,
) -> Result<()> {
    // Load existing index
    let index_file_path = Path::new(&index_path);
    if !index_file_path.exists() {
        return Err(anyhow::anyhow!("Index file not found: {}", index_path));
    }

    let index_content = fs::read_to_string(index_file_path)?;
    let mut index: JsonValue = serde_json::from_str(&index_content)?;

    // Get packs array (or create it)
    let packs = index
        .get_mut("packs")
        .and_then(|p| p.as_array_mut())
        .ok_or_else(|| anyhow::anyhow!("Invalid index format: missing 'packs' array"))?;

    // Load pack.yaml from the pack directory
    let pack_dir = Path::new(&pack_path);
    if !pack_dir.exists() || !pack_dir.is_dir() {
        return Err(anyhow::anyhow!("Pack directory not found: {}", pack_path));
    }

    let pack_yaml_path = pack_dir.join("pack.yaml");
    if !pack_yaml_path.exists() {
        return Err(anyhow::anyhow!(
            "pack.yaml not found in directory: {}",
            pack_path
        ));
    }

    let pack_yaml_content = fs::read_to_string(&pack_yaml_path)?;
    let pack_yaml: serde_yaml_ng::Value = serde_yaml_ng::from_str(&pack_yaml_content)?;

    // Extract pack metadata
    let pack_ref = pack_yaml
        .get("ref")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'ref' field in pack.yaml"))?;

    let version = pack_yaml
        .get("version")
        .and_then(|v| v.as_str())
        .ok_or_else(|| anyhow::anyhow!("Missing 'version' field in pack.yaml"))?;

    // Check if pack already exists in index
    let existing_index = packs
        .iter()
        .position(|p| p.get("ref").and_then(|r| r.as_str()) == Some(pack_ref));

    if let Some(_idx) = existing_index {
        if !update {
            return Err(anyhow::anyhow!(
                "Pack '{}' already exists in index. Use --update to replace it.",
                pack_ref
            ));
        }
        if output_format == OutputFormat::Table {
            output::print_info(&format!("Updating existing entry for '{}'", pack_ref));
        }
    } else if output_format == OutputFormat::Table {
        output::print_info(&format!("Adding new entry for '{}'", pack_ref));
    }

    // Calculate checksum
    if output_format == OutputFormat::Table {
        output::print_info("Calculating checksum...");
    }
    let checksum = calculate_directory_checksum(pack_dir)?;

    // Build install sources
    let mut install_sources = Vec::new();

    if let Some(ref git) = git_url {
        let default_ref = format!("v{}", version);
        let ref_value = git_ref.as_deref().unwrap_or(&default_ref);
        install_sources.push(serde_json::json!({
            "type": "git",
            "url": git,
            "ref": ref_value,
            "checksum": format!("sha256:{}", checksum)
        }));
    }

    if let Some(ref archive) = archive_url {
        install_sources.push(serde_json::json!({
            "type": "archive",
            "url": archive,
            "checksum": format!("sha256:{}", checksum)
        }));
    }

    // Extract other metadata
    let label = pack_yaml
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or(pack_ref);

    let description = pack_yaml
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    let author = pack_yaml
        .get("author")
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown");

    let license = pack_yaml
        .get("license")
        .and_then(|v| v.as_str())
        .unwrap_or("Apache-2.0");

    let email = pack_yaml.get("email").and_then(|v| v.as_str());
    let homepage = pack_yaml.get("homepage").and_then(|v| v.as_str());
    let repository = pack_yaml.get("repository").and_then(|v| v.as_str());

    let keywords: Vec<String> = pack_yaml
        .get("keywords")
        .and_then(|v| v.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let runtime_deps: Vec<String> = pack_yaml
        .get("runtime_deps")
        .and_then(|v| v.as_sequence())
        .map(|seq| {
            seq.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    // Count components
    let actions_count = pack_yaml["actions"]
        .as_mapping()
        .map(|m| m.len())
        .unwrap_or(0);
    let sensors_count = pack_yaml["sensors"]
        .as_mapping()
        .map(|m| m.len())
        .unwrap_or(0);
    let triggers_count = pack_yaml["triggers"]
        .as_mapping()
        .map(|m| m.len())
        .unwrap_or(0);

    // Build index entry
    let mut index_entry = serde_json::json!({
        "ref": pack_ref,
        "label": label,
        "description": description,
        "version": version,
        "author": author,
        "license": license,
        "keywords": keywords,
        "runtime_deps": runtime_deps,
        "install_sources": install_sources,
        "contents": {
            "actions": actions_count,
            "sensors": sensors_count,
            "triggers": triggers_count,
            "rules": 0,
            "workflows": 0
        }
    });

    // Add optional fields
    if let Some(e) = email {
        index_entry["email"] = JsonValue::String(e.to_string());
    }
    if let Some(h) = homepage {
        index_entry["homepage"] = JsonValue::String(h.to_string());
    }
    if let Some(r) = repository {
        index_entry["repository"] = JsonValue::String(r.to_string());
    }

    // Update or add entry
    if let Some(idx) = existing_index {
        packs[idx] = index_entry;
    } else {
        packs.push(index_entry);
    }

    // Write updated index back to file
    let updated_content = serde_json::to_string_pretty(&index)?;
    fs::write(index_file_path, updated_content)?;

    match output_format {
        OutputFormat::Table => {
            output::print_success(&format!("✓ Index updated successfully: {}", index_path));
            output::print_info(&format!("  Pack: {} v{}", pack_ref, version));
            output::print_info(&format!("  Checksum: sha256:{}", checksum));
        }
        OutputFormat::Json => {
            let response = serde_json::json!({
                "success": true,
                "index_file": index_path,
                "pack_ref": pack_ref,
                "version": version,
                "checksum": format!("sha256:{}", checksum),
                "action": if existing_index.is_some() { "updated" } else { "added" }
            });
            output::print_output(&response, OutputFormat::Json)?;
        }
        OutputFormat::Yaml => {
            let response = serde_json::json!({
                "success": true,
                "index_file": index_path,
                "pack_ref": pack_ref,
                "version": version,
                "checksum": format!("sha256:{}", checksum),
                "action": if existing_index.is_some() { "updated" } else { "added" }
            });
            output::print_output(&response, OutputFormat::Yaml)?;
        }
    }

    Ok(())
}

/// Merge multiple registry index files into one
pub async fn handle_index_merge(
    output_path: String,
    input_paths: Vec<String>,
    force: bool,
    output_format: OutputFormat,
) -> Result<()> {
    // Check if output file exists
    let output_file_path = Path::new(&output_path);
    if output_file_path.exists() && !force {
        return Err(anyhow::anyhow!(
            "Output file already exists: {}. Use --force to overwrite.",
            output_path
        ));
    }

    // Track all packs by ref (for deduplication)
    let mut packs_map: HashMap<String, JsonValue> = HashMap::new();
    let mut total_loaded = 0;
    let mut duplicates_resolved = 0;

    // Load and merge all input files
    for input_path in &input_paths {
        let input_file_path = Path::new(input_path);
        if !input_file_path.exists() {
            if output_format == OutputFormat::Table {
                output::print_warning(&format!("Skipping missing file: {}", input_path));
            }
            continue;
        }

        if output_format == OutputFormat::Table {
            output::print_info(&format!("Loading: {}", input_path));
        }

        let index_content = fs::read_to_string(input_file_path)?;
        let index: JsonValue = serde_json::from_str(&index_content)?;

        let packs = index
            .get("packs")
            .and_then(|p| p.as_array())
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Invalid index format in {}: missing 'packs' array",
                    input_path
                )
            })?;

        for pack in packs {
            let pack_ref = pack.get("ref").and_then(|r| r.as_str()).ok_or_else(|| {
                anyhow::anyhow!("Pack entry missing 'ref' field in {}", input_path)
            })?;

            if packs_map.contains_key(pack_ref) {
                // Check versions and keep the latest
                let existing_version = packs_map[pack_ref]
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("0.0.0");

                let new_version = pack
                    .get("version")
                    .and_then(|v| v.as_str())
                    .unwrap_or("0.0.0");

                // Simple string comparison (could use semver crate for proper comparison)
                if new_version > existing_version {
                    if output_format == OutputFormat::Table {
                        output::print_info(&format!(
                            "  Updating '{}' from {} to {}",
                            pack_ref, existing_version, new_version
                        ));
                    }
                    packs_map.insert(pack_ref.to_string(), pack.clone());
                } else if output_format == OutputFormat::Table {
                    output::print_info(&format!(
                        "  Keeping '{}' at {} (newer than {})",
                        pack_ref, existing_version, new_version
                    ));
                }
                duplicates_resolved += 1;
            } else {
                packs_map.insert(pack_ref.to_string(), pack.clone());
            }
            total_loaded += 1;
        }
    }

    // Build merged index
    let packs: Vec<JsonValue> = packs_map.into_values().collect();
    let merged_index = serde_json::json!({
        "version": "1.0",
        "generated_at": chrono::Utc::now().to_rfc3339(),
        "packs": packs
    });

    // Write merged index
    let merged_content = serde_json::to_string_pretty(&merged_index)?;
    fs::write(output_file_path, merged_content)?;

    match output_format {
        OutputFormat::Table => {
            output::print_success(&format!(
                "✓ Merged {} index files into {}",
                input_paths.len(),
                output_path
            ));
            output::print_info(&format!("  Total packs loaded: {}", total_loaded));
            output::print_info(&format!("  Unique packs: {}", packs.len()));
            if duplicates_resolved > 0 {
                output::print_info(&format!("  Duplicates resolved: {}", duplicates_resolved));
            }
        }
        OutputFormat::Json => {
            let response = serde_json::json!({
                "success": true,
                "output_file": output_path,
                "sources_count": input_paths.len(),
                "total_loaded": total_loaded,
                "unique_packs": packs.len(),
                "duplicates_resolved": duplicates_resolved
            });
            output::print_output(&response, OutputFormat::Json)?;
        }
        OutputFormat::Yaml => {
            let response = serde_json::json!({
                "success": true,
                "output_file": output_path,
                "sources_count": input_paths.len(),
                "total_loaded": total_loaded,
                "unique_packs": packs.len(),
                "duplicates_resolved": duplicates_resolved
            });
            output::print_output(&response, OutputFormat::Yaml)?;
        }
    }

    Ok(())
}
