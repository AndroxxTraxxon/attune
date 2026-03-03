use anyhow::Result;
use clap::Subcommand;
use serde::{Deserialize, Serialize};

use crate::client::ApiClient;
use crate::config::CliConfig;
use crate::output::{self, OutputFormat};

#[derive(Subcommand)]
pub enum AuthCommands {
    /// Log in to Attune API
    Login {
        /// Username or email
        #[arg(short, long)]
        username: String,

        /// Password (will prompt if not provided)
        #[arg(long)]
        password: Option<String>,

        /// API URL to log in to (saved into the profile for future use)
        #[arg(long)]
        url: Option<String>,

        /// Save credentials into a named profile (creates it if it doesn't exist)
        #[arg(long)]
        save_profile: Option<String>,
    },
    /// Log out and clear authentication tokens
    Logout,
    /// Show current authentication status
    Whoami,
    /// Refresh authentication token
    Refresh,
}

#[derive(Debug, Serialize, Deserialize)]
struct LoginRequest {
    login: String,
    password: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct LoginResponse {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
}

#[derive(Debug, Serialize, Deserialize)]
struct Identity {
    id: i64,
    login: String,
    display_name: Option<String>,
}

pub async fn handle_auth_command(
    profile: &Option<String>,
    command: AuthCommands,
    api_url: &Option<String>,
    output_format: OutputFormat,
) -> Result<()> {
    match command {
        AuthCommands::Login {
            username,
            password,
            url,
            save_profile,
        } => {
            // --url is a convenient alias for --api-url at login time
            let effective_api_url = url.or_else(|| api_url.clone());
            handle_login(
                username,
                password,
                save_profile.as_ref().or(profile.as_ref()),
                &effective_api_url,
                output_format,
            )
            .await
        }
        AuthCommands::Logout => handle_logout(profile, output_format).await,
        AuthCommands::Whoami => handle_whoami(profile, api_url, output_format).await,
        AuthCommands::Refresh => handle_refresh(profile, api_url, output_format).await,
    }
}

async fn handle_login(
    username: String,
    password: Option<String>,
    profile: Option<&String>,
    api_url: &Option<String>,
    output_format: OutputFormat,
) -> Result<()> {
    // Determine which profile name will own these credentials.
    // If --save-profile / --profile was given, use that; otherwise use the
    // currently-active profile.
    let mut config = CliConfig::load()?;
    let target_profile_name = profile
        .cloned()
        .unwrap_or_else(|| config.current_profile.clone());

    // If a URL was provided and the target profile doesn't exist yet, create it.
    if !config.profiles.contains_key(&target_profile_name) {
        let url = api_url.clone().unwrap_or_else(|| "http://localhost:8080".to_string());
        use crate::config::Profile;
        config.set_profile(
            target_profile_name.clone(),
            Profile {
                api_url: url,
                auth_token: None,
                refresh_token: None,
                output_format: None,
                description: None,
            },
        )?;
    } else if let Some(url) = api_url {
        // Profile exists — update its api_url if an explicit URL was provided.
        if let Some(p) = config.profiles.get_mut(&target_profile_name) {
            p.api_url = url.clone();
        }
        config.save()?;
    }

    // Build a temporary config view that points at the target profile so
    // ApiClient uses the right base URL.
    let mut login_config = CliConfig::load()?;
    login_config.current_profile = target_profile_name.clone();

    // Prompt for password if not provided
    let password = match password {
        Some(p) => p,
        None => {
            let pw = dialoguer::Password::new()
                .with_prompt("Password")
                .interact()?;
            pw
        }
    };

    let mut client = ApiClient::from_config(&login_config, api_url);

    let login_req = LoginRequest {
        login: username,
        password,
    };

    let response: LoginResponse = client.post("/auth/login", &login_req).await?;

    // Persist tokens into the target profile.
    let mut config = CliConfig::load()?;
    // Ensure the profile exists (it may have just been created above and saved).
    if let Some(p) = config.profiles.get_mut(&target_profile_name) {
        p.auth_token = Some(response.access_token.clone());
        p.refresh_token = Some(response.refresh_token.clone());
        config.save()?;
    } else {
        // Fallback: set_auth writes to the current profile.
        config.set_auth(response.access_token.clone(), response.refresh_token.clone())?;
    }

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            output::print_output(&response, output_format)?;
        }
        OutputFormat::Table => {
            output::print_success("Successfully logged in");
            output::print_info(&format!("Token expires in {} seconds", response.expires_in));
            if target_profile_name != config.current_profile {
                output::print_info(&format!(
                    "Credentials saved to profile '{}'",
                    target_profile_name
                ));
            }
        }
    }

    Ok(())
}

async fn handle_logout(profile: &Option<String>, output_format: OutputFormat) -> Result<()> {
    let mut config = CliConfig::load_with_profile(profile.as_deref())?;
    config.clear_auth()?;

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            let msg = serde_json::json!({"message": "Successfully logged out"});
            output::print_output(&msg, output_format)?;
        }
        OutputFormat::Table => {
            output::print_success("Successfully logged out");
        }
    }

    Ok(())
}

async fn handle_whoami(
    profile: &Option<String>,
    api_url: &Option<String>,
    output_format: OutputFormat,
) -> Result<()> {
    let config = CliConfig::load_with_profile(profile.as_deref())?;

    if config.auth_token().ok().flatten().is_none() {
        anyhow::bail!("Not logged in. Use 'attune auth login' to authenticate.");
    }

    let mut client = ApiClient::from_config(&config, api_url);

    let identity: Identity = client.get("/auth/me").await?;

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            output::print_output(&identity, output_format)?;
        }
        OutputFormat::Table => {
            output::print_section("Current Identity");
            output::print_key_value_table(vec![
                ("ID", identity.id.to_string()),
                ("Login", identity.login),
                (
                    "Display Name",
                    identity.display_name.unwrap_or_else(|| "-".to_string()),
                ),
            ]);
        }
    }

    Ok(())
}

async fn handle_refresh(
    profile: &Option<String>,
    api_url: &Option<String>,
    output_format: OutputFormat,
) -> Result<()> {
    let config = CliConfig::load_with_profile(profile.as_deref())?;

    // Check if we have a refresh token
    let refresh_token = config
        .refresh_token()
        .ok()
        .flatten()
        .ok_or_else(|| anyhow::anyhow!("No refresh token found. Please log in again."))?;

    let mut client = ApiClient::from_config(&config, api_url);

    #[derive(Serialize)]
    struct RefreshRequest {
        refresh_token: String,
    }

    // Call the refresh endpoint
    let response: LoginResponse = client
        .post("/auth/refresh", &RefreshRequest { refresh_token })
        .await?;

    // Save new tokens to config
    let mut config = CliConfig::load()?;
    config.set_auth(
        response.access_token.clone(),
        response.refresh_token.clone(),
    )?;

    match output_format {
        OutputFormat::Json | OutputFormat::Yaml => {
            output::print_output(&response, output_format)?;
        }
        OutputFormat::Table => {
            output::print_success("Token refreshed successfully");
            output::print_info(&format!(
                "New token expires in {} seconds",
                response.expires_in
            ));
        }
    }

    Ok(())
}
