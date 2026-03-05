use anyhow::{Context, Result};
use clap::Subcommand;
use colored::Colorize;

use crate::config::CliConfig;
use crate::output::{self, OutputFormat};

#[derive(Subcommand)]
pub enum ConfigCommands {
    /// List all configuration values
    List,
    /// Get a configuration value
    Get {
        /// Configuration key
        key: String,
    },
    /// Set a configuration value
    Set {
        /// Configuration key
        key: String,
        /// Configuration value
        value: String,
    },
    /// Show the configuration file path
    Path,
    /// List all profiles
    Profiles,
    /// Show current profile
    Current,
    /// Switch to a different profile
    Use {
        /// Profile name
        name: String,
    },
    /// Add or update a profile
    AddProfile {
        /// Profile name
        name: String,
        /// API URL
        #[arg(short, long)]
        api_url: String,
        /// Description
        #[arg(short, long)]
        description: Option<String>,
    },
    /// Remove a profile
    RemoveProfile {
        /// Profile name
        name: String,
    },
    /// Show profile details
    ShowProfile {
        /// Profile name
        name: String,
    },
}

pub async fn handle_config_command(
    _profile: &Option<String>,
    command: ConfigCommands,
    output_format: OutputFormat,
) -> Result<()> {
    match command {
        ConfigCommands::List => handle_list(output_format).await,
        ConfigCommands::Get { key } => handle_get(key, output_format).await,
        ConfigCommands::Set { key, value } => handle_set(key, value, output_format).await,
        ConfigCommands::Path => handle_path(output_format).await,
        ConfigCommands::Profiles => handle_profiles(output_format).await,
        ConfigCommands::Current => handle_current(output_format).await,
        ConfigCommands::Use { name } => handle_use(name, output_format).await,
        ConfigCommands::AddProfile {
            name,
            api_url,
            description,
        } => handle_add_profile(name, api_url, description, output_format).await,
        ConfigCommands::RemoveProfile { name } => handle_remove_profile(name, output_format).await,
        ConfigCommands::ShowProfile { name } => handle_show_profile(name, output_format).await,
    }
}

async fn handle_list(output_format: OutputFormat) -> Result<()> {
    let config = CliConfig::load()?; // Config commands always use default profile
    let all_config = config.list_all();

    match output_format {
        OutputFormat::Json => {
            let map: std::collections::HashMap<String, String> = all_config.into_iter().collect();
            output::print_output(&map, output_format)?;
        }
        OutputFormat::Yaml => {
            let map: std::collections::HashMap<String, String> = all_config.into_iter().collect();
            output::print_output(&map, output_format)?;
        }
        OutputFormat::Table => {
            output::print_section("Configuration");
            let pairs: Vec<(&str, String)> = all_config
                .iter()
                .map(|(k, v)| (k.as_str(), v.clone()))
                .collect();
            output::print_key_value_table(pairs);
        }
    }

    Ok(())
}

async fn handle_get(key: String, output_format: OutputFormat) -> Result<()> {
    let config = CliConfig::load()?; // Config commands always use default profile
    let value = config.get_value(&key)?;

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            let result = serde_json::json!({
                "key": key,
                "value": value
            });
            output::print_output(&result, output_format)?;
        }
        OutputFormat::Table => {
            println!("{}", value);
        }
    }

    Ok(())
}

async fn handle_profiles(output_format: OutputFormat) -> Result<()> {
    let config = CliConfig::load()?; // Config commands always use default profile
    let profiles = config.list_profiles();
    let current = &config.current_profile;

    match output_format {
        OutputFormat::Json => {
            let data: Vec<_> = profiles
                .iter()
                .map(|name| {
                    serde_json::json!({
                        "name": name,
                        "current": name == current
                    })
                })
                .collect();
            output::print_output(&data, output_format)?;
        }
        OutputFormat::Yaml => {
            let data: Vec<_> = profiles
                .iter()
                .map(|name| {
                    serde_json::json!({
                        "name": name,
                        "current": name == current
                    })
                })
                .collect();
            output::print_output(&data, output_format)?;
        }
        OutputFormat::Table => {
            output::print_section("Profiles");
            for name in profiles {
                if name == *current {
                    println!("  • {} (active)", name.bright_green().bold());
                } else {
                    println!("  • {}", name);
                }
            }
        }
    }

    Ok(())
}

async fn handle_current(output_format: OutputFormat) -> Result<()> {
    let config = CliConfig::load()?; // Config commands always use default profile

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            let result = serde_json::json!({
                "current_profile": config.current_profile
            });
            output::print_output(&result, output_format)?;
        }
        OutputFormat::Table => {
            println!("{}", config.current_profile);
        }
    }

    Ok(())
}

async fn handle_use(name: String, output_format: OutputFormat) -> Result<()> {
    let mut config = CliConfig::load()?;
    config.switch_profile(name.clone())?;

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            let result = serde_json::json!({
                "current_profile": name,
                "message": "Switched profile"
            });
            output::print_output(&result, output_format)?;
        }
        OutputFormat::Table => {
            output::print_success(&format!("Switched to profile '{}'", name));
        }
    }

    Ok(())
}

async fn handle_add_profile(
    name: String,
    api_url: String,
    description: Option<String>,
    output_format: OutputFormat,
) -> Result<()> {
    use crate::config::Profile;

    let mut config = CliConfig::load()?;

    let profile = Profile {
        api_url: api_url.clone(),
        auth_token: None,
        refresh_token: None,
        output_format: None,
        description,
    };

    config.set_profile(name.clone(), profile)?;

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            let result = serde_json::json!({
                "profile": name,
                "api_url": api_url,
                "message": "Profile added"
            });
            output::print_output(&result, output_format)?;
        }
        OutputFormat::Table => {
            output::print_success(&format!("Profile '{}' added", name));
            output::print_info(&format!("API URL: {}", api_url));
        }
    }

    Ok(())
}

async fn handle_remove_profile(name: String, output_format: OutputFormat) -> Result<()> {
    let mut config = CliConfig::load()?;
    config.remove_profile(&name)?;

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            let result = serde_json::json!({
                "profile": name,
                "message": "Profile removed"
            });
            output::print_output(&result, output_format)?;
        }
        OutputFormat::Table => {
            output::print_success(&format!("Profile '{}' removed", name));
        }
    }

    Ok(())
}

async fn handle_show_profile(name: String, output_format: OutputFormat) -> Result<()> {
    let config = CliConfig::load()?; // Config commands always use default profile
    let profile = config
        .get_profile(&name)
        .context(format!("Profile '{}' not found", name))?;

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            output::print_output(&profile, output_format)?;
        }
        OutputFormat::Table => {
            output::print_section(&format!("Profile: {}", name));
            let mut pairs = vec![
                ("API URL", profile.api_url.clone()),
                (
                    "Auth Token",
                    profile
                        .auth_token
                        .as_ref()
                        .map(|_| "***")
                        .unwrap_or("(not set)")
                        .to_string(),
                ),
                (
                    "Refresh Token",
                    profile
                        .refresh_token
                        .as_ref()
                        .map(|_| "***")
                        .unwrap_or("(not set)")
                        .to_string(),
                ),
            ];

            if let Some(output_format) = &profile.output_format {
                pairs.push(("Output Format", output_format.clone()));
            }

            if let Some(description) = &profile.description {
                pairs.push(("Description", description.clone()));
            }

            output::print_key_value_table(pairs);
        }
    }

    Ok(())
}

async fn handle_set(key: String, value: String, output_format: OutputFormat) -> Result<()> {
    let mut config = CliConfig::load()?;
    config.set_value(&key, value.clone())?;

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            let result = serde_json::json!({
                "key": key,
                "value": value,
                "message": "Configuration updated"
            });
            output::print_output(&result, output_format)?;
        }
        OutputFormat::Table => {
            println!("Configuration updated: {} = {}", key, value);
        }
    }

    Ok(())
}

async fn handle_path(output_format: OutputFormat) -> Result<()> {
    let path = CliConfig::config_path()?;

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            let result = serde_json::json!({
                "path": path.to_string_lossy()
            });
            output::print_output(&result, output_format)?;
        }
        OutputFormat::Table => {
            println!("{}", path.display());
        }
    }

    Ok(())
}
