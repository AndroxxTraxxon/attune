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
        AuthCommands::Login { username, password } => {
            handle_login(username, password, profile, api_url, output_format).await
        }
        AuthCommands::Logout => handle_logout(profile, output_format).await,
        AuthCommands::Whoami => handle_whoami(profile, api_url, output_format).await,
        AuthCommands::Refresh => handle_refresh(profile, api_url, output_format).await,
    }
}

async fn handle_login(
    username: String,
    password: Option<String>,
    profile: &Option<String>,
    api_url: &Option<String>,
    output_format: OutputFormat,
) -> Result<()> {
    let config = CliConfig::load_with_profile(profile.as_deref())?;

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

    let mut client = ApiClient::from_config(&config, api_url);

    let login_req = LoginRequest {
        login: username,
        password,
    };

    let response: LoginResponse = client.post("/auth/login", &login_req).await?;

    // Save tokens to config
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
            output::print_success("Successfully logged in");
            output::print_info(&format!("Token expires in {} seconds", response.expires_in));
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
