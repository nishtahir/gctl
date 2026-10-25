use console::style;
use dialoguer::{Input, Password};
use gctl_ghost::GhostAdminToken;
use url::Url;

use crate::cli::AuthCommands;
use crate::config::StoredConfig;
use crate::error::GctlCliError;

pub struct AuthCommandHandler;

impl AuthCommandHandler {
    pub fn handle_command(&self, command: AuthCommands) -> Result<(), GctlCliError> {
        match command {
            AuthCommands::Login => self.login(),
            AuthCommands::Logout => self.logout(),
            AuthCommands::Status => self.status(),
        }
    }

    fn login(&self) -> Result<(), GctlCliError> {
        let existing = StoredConfig::load()?;

        let mut url_prompt = Input::<String>::new().with_prompt("Ghost URL");
        if let Some(url) = existing.ghost_url {
            url_prompt = url_prompt.default(url);
        }
        let ghost_url = url_prompt
            .validate_with(|input: &String| {
                Url::parse(input).map(|_| ()).map_err(|e| e.to_string())
            })
            .interact_text()
            .map_err(|e| GctlCliError::Config(e.to_string()))?;

        let ghost_admin_key = Password::new()
            .with_prompt("Admin API key (id:secret)")
            .validate_with(|input: &String| {
                GhostAdminToken::parse(input)
                    .map(|_| ())
                    .map_err(|e| e.to_string())
            })
            .interact()
            .map_err(|e| GctlCliError::Config(e.to_string()))?;

        let path = StoredConfig {
            ghost_url: Some(ghost_url),
            ghost_admin_key: Some(ghost_admin_key),
        }
        .save()?;

        println!("Saved credentials to {}", style(path.display()).green());
        Ok(())
    }

    fn logout(&self) -> Result<(), GctlCliError> {
        if StoredConfig::delete()? {
            println!("Removed stored credentials");
        } else {
            println!("No stored credentials found");
        }
        Ok(())
    }

    fn status(&self) -> Result<(), GctlCliError> {
        let config = StoredConfig::load()?;
        match (config.ghost_url, config.ghost_admin_key) {
            (Some(url), Some(key)) => {
                let id = key.split(':').next().unwrap_or_default();
                println!("Logged in to {} (key id {})", style(url).cyan(), id);
            }
            _ => println!("Not logged in. Run `gctl auth login`."),
        }
        Ok(())
    }
}
