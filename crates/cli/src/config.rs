use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

/// CLI configuration stored in user's home directory
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliConfig {
    /// Current active profile name
    #[serde(default = "default_profile_name")]
    pub current_profile: String,
    /// Named profiles (like SSH hosts)
    #[serde(default)]
    pub profiles: HashMap<String, Profile>,
    /// Default output format (can be overridden per-profile)
    #[serde(default = "default_output_format")]
    pub default_output_format: String,
}

fn default_profile_name() -> String {
    "default".to_string()
}

fn default_output_format() -> String {
    "table".to_string()
}

/// A named profile for connecting to an Attune server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Profile {
    /// API endpoint URL
    pub api_url: String,
    /// Authentication token
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auth_token: Option<String>,
    /// Refresh token
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    /// Output format override for this profile
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_format: Option<String>,
    /// Optional description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl Default for CliConfig {
    fn default() -> Self {
        let mut profiles = HashMap::new();
        profiles.insert(
            "default".to_string(),
            Profile {
                api_url: "http://localhost:8080".to_string(),
                auth_token: None,
                refresh_token: None,
                output_format: None,
                description: Some("Default local server".to_string()),
            },
        );

        Self {
            current_profile: "default".to_string(),
            profiles,
            default_output_format: default_output_format(),
        }
    }
}

impl CliConfig {
    /// Get the configuration file path
    pub fn config_path() -> Result<PathBuf> {
        // Respect XDG_CONFIG_HOME environment variable (for tests and user overrides)
        let config_dir = if let Ok(xdg_config) = env::var("XDG_CONFIG_HOME") {
            PathBuf::from(xdg_config)
        } else {
            dirs::config_dir().context("Failed to determine config directory")?
        };

        let attune_config_dir = config_dir.join("attune");
        fs::create_dir_all(&attune_config_dir).context("Failed to create config directory")?;

        Ok(attune_config_dir.join("config.yaml"))
    }

    /// Load configuration from file, or create default if not exists
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;

        if !path.exists() {
            let config = Self::default();
            config.save()?;
            return Ok(config);
        }

        let content = fs::read_to_string(&path).context("Failed to read config file")?;

        let config: Self =
            serde_yaml_ng::from_str(&content).context("Failed to parse config file")?;

        Ok(config)
    }

    /// Save configuration to file
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;

        let content = serde_yaml_ng::to_string(self).context("Failed to serialize config")?;

        fs::write(&path, content).context("Failed to write config file")?;

        Ok(())
    }

    /// Get the current active profile
    pub fn current_profile(&self) -> Result<&Profile> {
        self.profiles
            .get(&self.current_profile)
            .context(format!("Profile '{}' not found", self.current_profile))
    }

    /// Get a mutable reference to the current profile
    pub fn current_profile_mut(&mut self) -> Result<&mut Profile> {
        let profile_name = self.current_profile.clone();
        self.profiles
            .get_mut(&profile_name)
            .context(format!("Profile '{}' not found", profile_name))
    }

    /// Get a profile by name
    pub fn get_profile(&self, name: &str) -> Option<&Profile> {
        self.profiles.get(name)
    }

    /// Switch to a different profile
    pub fn switch_profile(&mut self, name: String) -> Result<()> {
        if !self.profiles.contains_key(&name) {
            anyhow::bail!("Profile '{}' does not exist", name);
        }
        self.current_profile = name;
        self.save()
    }

    /// Add or update a profile
    pub fn set_profile(&mut self, name: String, profile: Profile) -> Result<()> {
        self.profiles.insert(name, profile);
        self.save()
    }

    /// Remove a profile
    pub fn remove_profile(&mut self, name: &str) -> Result<()> {
        if self.current_profile == name {
            anyhow::bail!("Cannot remove active profile");
        }
        if name == "default" {
            anyhow::bail!("Cannot remove the default profile");
        }
        self.profiles.remove(name);
        self.save()
    }

    /// List all profile names
    pub fn list_profiles(&self) -> Vec<String> {
        let mut names: Vec<String> = self.profiles.keys().cloned().collect();
        names.sort();
        names
    }

    /// Set the API URL for the current profile
    ///
    /// Part of configuration management API - used by `attune config set api-url` command
    #[allow(dead_code)]
    pub fn set_api_url(&mut self, url: String) -> Result<()> {
        let profile = self.current_profile_mut()?;
        profile.api_url = url;
        self.save()
    }

    /// Set authentication tokens for the current profile
    pub fn set_auth(&mut self, access_token: String, refresh_token: String) -> Result<()> {
        let profile = self.current_profile_mut()?;
        profile.auth_token = Some(access_token);
        profile.refresh_token = Some(refresh_token);
        self.save()
    }

    /// Clear authentication tokens for the current profile
    pub fn clear_auth(&mut self) -> Result<()> {
        let profile = self.current_profile_mut()?;
        profile.auth_token = None;
        profile.refresh_token = None;
        self.save()
    }

    /// Set a configuration value by key
    pub fn set_value(&mut self, key: &str, value: String) -> Result<()> {
        match key {
            "api_url" => {
                let profile = self.current_profile_mut()?;
                profile.api_url = value;
            }
            "output_format" => {
                let profile = self.current_profile_mut()?;
                profile.output_format = Some(value);
            }
            "default_output_format" => {
                self.default_output_format = value;
            }
            "current_profile" => {
                self.switch_profile(value)?;
                return Ok(());
            }
            _ => anyhow::bail!("Unknown config key: {}", key),
        }
        self.save()
    }

    /// Get a configuration value by key
    pub fn get_value(&self, key: &str) -> Result<String> {
        match key {
            "api_url" => {
                let profile = self.current_profile()?;
                Ok(profile.api_url.clone())
            }
            "output_format" => {
                let profile = self.current_profile()?;
                Ok(profile
                    .output_format
                    .clone()
                    .unwrap_or_else(|| self.default_output_format.clone()))
            }
            "default_output_format" => Ok(self.default_output_format.clone()),
            "current_profile" => Ok(self.current_profile.clone()),
            "auth_token" => {
                let profile = self.current_profile()?;
                Ok(profile
                    .auth_token
                    .as_ref()
                    .map(|_| "***")
                    .unwrap_or("(not set)")
                    .to_string())
            }
            "refresh_token" => {
                let profile = self.current_profile()?;
                Ok(profile
                    .refresh_token
                    .as_ref()
                    .map(|_| "***")
                    .unwrap_or("(not set)")
                    .to_string())
            }
            _ => anyhow::bail!("Unknown config key: {}", key),
        }
    }

    /// List all configuration keys and values for current profile
    pub fn list_all(&self) -> Vec<(String, String)> {
        let profile = match self.current_profile() {
            Ok(p) => p,
            Err(_) => return vec![],
        };

        vec![
            ("current_profile".to_string(), self.current_profile.clone()),
            ("api_url".to_string(), profile.api_url.clone()),
            (
                "output_format".to_string(),
                profile
                    .output_format
                    .clone()
                    .unwrap_or_else(|| self.default_output_format.clone()),
            ),
            (
                "default_output_format".to_string(),
                self.default_output_format.clone(),
            ),
            (
                "auth_token".to_string(),
                profile
                    .auth_token
                    .as_ref()
                    .map(|_| "***")
                    .unwrap_or("(not set)")
                    .to_string(),
            ),
            (
                "refresh_token".to_string(),
                profile
                    .refresh_token
                    .as_ref()
                    .map(|_| "***")
                    .unwrap_or("(not set)")
                    .to_string(),
            ),
        ]
    }

    /// Load configuration with optional profile override (without saving)
    ///
    /// Used by `--profile` flag to temporarily use a different profile
    pub fn load_with_profile(profile_name: Option<&str>) -> Result<Self> {
        let mut config = Self::load()?;

        if let Some(name) = profile_name {
            // Temporarily switch profile without saving
            if !config.profiles.contains_key(name) {
                anyhow::bail!("Profile '{}' does not exist", name);
            }
            config.current_profile = name.to_string();
        }

        Ok(config)
    }

    /// Get the effective API URL (from override, current profile, or default)
    pub fn effective_api_url(&self, override_url: &Option<String>) -> String {
        if let Some(url) = override_url {
            return url.clone();
        }

        if let Ok(profile) = self.current_profile() {
            profile.api_url.clone()
        } else {
            "http://localhost:8080".to_string()
        }
    }

    /// Get API URL for current profile (without override)
    #[allow(unused)]
    pub fn api_url(&self) -> Result<String> {
        let profile = self.current_profile()?;
        Ok(profile.api_url.clone())
    }

    /// Get auth token for current profile
    pub fn auth_token(&self) -> Result<Option<String>> {
        let profile = self.current_profile()?;
        Ok(profile.auth_token.clone())
    }

    /// Get refresh token for current profile
    pub fn refresh_token(&self) -> Result<Option<String>> {
        let profile = self.current_profile()?;
        Ok(profile.refresh_token.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = CliConfig::default();
        assert_eq!(config.current_profile, "default");
        assert_eq!(config.default_output_format, "table");
        assert!(config.profiles.contains_key("default"));

        let profile = config.current_profile().unwrap();
        assert_eq!(profile.api_url, "http://localhost:8080");
        assert!(profile.auth_token.is_none());
        assert!(profile.refresh_token.is_none());
    }

    #[test]
    fn test_effective_api_url() {
        let config = CliConfig::default();

        // No override
        assert_eq!(config.effective_api_url(&None), "http://localhost:8080");

        // With override
        let override_url = Some("http://example.com".to_string());
        assert_eq!(
            config.effective_api_url(&override_url),
            "http://example.com"
        );
    }

    #[test]
    fn test_profile_management() {
        let mut config = CliConfig::default();

        // Add a new profile
        let staging_profile = Profile {
            api_url: "https://staging.example.com".to_string(),
            auth_token: None,
            refresh_token: None,
            output_format: Some("json".to_string()),
            description: Some("Staging environment".to_string()),
        };
        config
            .set_profile("staging".to_string(), staging_profile)
            .unwrap();

        // List profiles
        let profiles = config.list_profiles();
        assert!(profiles.contains(&"default".to_string()));
        assert!(profiles.contains(&"staging".to_string()));

        // Switch to staging
        config.switch_profile("staging".to_string()).unwrap();
        assert_eq!(config.current_profile, "staging");

        let profile = config.current_profile().unwrap();
        assert_eq!(profile.api_url, "https://staging.example.com");
    }

    #[test]
    fn test_cannot_remove_default_profile() {
        let mut config = CliConfig::default();
        let result = config.remove_profile("default");
        assert!(result.is_err());
    }

    #[test]
    fn test_cannot_remove_active_profile() {
        let mut config = CliConfig::default();

        let test_profile = Profile {
            api_url: "http://test.com".to_string(),
            auth_token: None,
            refresh_token: None,
            output_format: None,
            description: None,
        };
        config
            .set_profile("test".to_string(), test_profile)
            .unwrap();
        config.switch_profile("test".to_string()).unwrap();

        let result = config.remove_profile("test");
        assert!(result.is_err());
    }

    #[test]
    fn test_get_set_value() {
        let mut config = CliConfig::default();

        assert_eq!(
            config.get_value("api_url").unwrap(),
            "http://localhost:8080"
        );
        assert_eq!(config.get_value("output_format").unwrap(), "table");

        // Set API URL for current profile
        config
            .set_value("api_url", "http://test.com".to_string())
            .unwrap();
        assert_eq!(config.get_value("api_url").unwrap(), "http://test.com");

        // Set output format for current profile
        config
            .set_value("output_format", "json".to_string())
            .unwrap();
        assert_eq!(config.get_value("output_format").unwrap(), "json");
    }
}
