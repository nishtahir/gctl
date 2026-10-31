use console::style;
use dialoguer::{Input, Password};
use gctl_ghost::GhostAdminToken;
use url::Url;

use crate::cli::AuthCommands;
use crate::config::{SiteConfig, StoredConfig};
use crate::error::GctlCliError;

pub struct AuthCommandHandler {
    /// The `--site` the user passed, if any.
    pub site: Option<String>,
}

impl AuthCommandHandler {
    pub fn handle_command(&self, command: AuthCommands) -> Result<(), GctlCliError> {
        match command {
            AuthCommands::Login => self.login(),
            AuthCommands::Logout => self.logout(),
            AuthCommands::Status => self.status(),
            AuthCommands::Use { name } => self.use_site(&name),
        }
    }

    fn login(&self) -> Result<(), GctlCliError> {
        let mut config = StoredConfig::load()?;
        // Without --site, log in to the site that would be used.
        let site = self.site.clone().or_else(|| config.default_site.clone());
        // Offer what is saved for this site as the default answer.
        let existing = match &site {
            Some(name) => config.sites.get(name).cloned().unwrap_or_default(),
            None => SiteConfig {
                ghost_url: config.ghost_url.clone(),
                ghost_admin_key: None,
            },
        };

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

        let credentials = SiteConfig {
            ghost_url: Some(ghost_url),
            ghost_admin_key: Some(ghost_admin_key),
        };
        match &site {
            Some(name) => {
                config.sites.insert(name.clone(), credentials);
            }
            None => {
                config.ghost_url = credentials.ghost_url;
                config.ghost_admin_key = credentials.ghost_admin_key;
            }
        }
        let path = config.save()?;

        println!("Saved credentials to {}", style(path.display()).green());
        Ok(())
    }

    fn logout(&self) -> Result<(), GctlCliError> {
        let Some(name) = &self.site else {
            if StoredConfig::delete()? {
                println!("Removed stored credentials");
            } else {
                println!("No stored credentials found");
            }
            return Ok(());
        };
        let mut config = StoredConfig::load()?;
        if config.sites.remove(name).is_none() {
            println!("No stored credentials for `{name}`");
            return Ok(());
        }
        if config.default_site.as_ref() == Some(name) {
            config.default_site = None;
        }
        config.save()?;
        println!("Removed stored credentials for `{name}`");
        Ok(())
    }

    fn use_site(&self, name: &str) -> Result<(), GctlCliError> {
        let mut config = StoredConfig::load()?;
        // Fails with the list of known sites when there is no such site.
        config.site(Some(name))?;
        config.default_site = Some(name.to_string());
        config.save()?;
        println!("Using `{name}` when --site isn't given");
        Ok(())
    }

    fn status(&self) -> Result<(), GctlCliError> {
        let config = StoredConfig::load()?;
        let describe = |site: &SiteConfig| match (&site.ghost_url, &site.ghost_admin_key) {
            (Some(url), Some(key)) => {
                let id = key.split(':').next().unwrap_or_default();
                Some(format!("{} (key id {})", style(url).cyan(), id))
            }
            _ => None,
        };

        let unnamed = SiteConfig {
            ghost_url: config.ghost_url.clone(),
            ghost_admin_key: config.ghost_admin_key.clone(),
        };
        let mut any = false;
        if let Some(text) = describe(&unnamed) {
            any = true;
            let marker = if config.default_site.is_none() {
                "*"
            } else {
                " "
            };
            println!("{marker} {:<12} {text}", "(default)");
        }
        for (name, site) in &config.sites {
            if let Some(text) = describe(site) {
                any = true;
                let marker = if config.default_site.as_ref() == Some(name) {
                    "*"
                } else {
                    " "
                };
                println!("{marker} {name:<12} {text}");
            }
        }
        if any {
            println!("\n* used when --site isn't given");
        } else {
            println!("Not logged in. Run `gctl auth login`.");
        }
        Ok(())
    }
}
