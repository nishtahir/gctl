use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::GctlCliError;

/// Credentials for one Ghost site.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct SiteConfig {
    pub ghost_url: Option<String>,
    pub ghost_admin_key: Option<String>,
}

/// Credentials persisted by `gctl auth login`. The top-level credentials are
/// the unnamed site; `sites` holds the ones saved with `--site <name>`.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct StoredConfig {
    pub ghost_url: Option<String>,
    pub ghost_admin_key: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub sites: BTreeMap<String, SiteConfig>,
    /// The named site used when `--site` isn't given.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default_site: Option<String>,
}

impl StoredConfig {
    pub fn path() -> Result<PathBuf, GctlCliError> {
        let dir = dirs::config_dir()
            .ok_or_else(|| GctlCliError::Config("Could not locate config directory".into()))?;
        Ok(dir.join("gctl").join("config.json"))
    }

    /// Returns an empty config when nothing has been stored yet.
    pub fn load() -> Result<Self, GctlCliError> {
        match fs::read_to_string(Self::path()?) {
            Ok(contents) => serde_json::from_str(&contents)
                .map_err(|e| GctlCliError::Config(format!("Invalid config file: {e}"))),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    /// The credentials for `site`, else for the default site, else the unnamed ones.
    pub fn site(&self, site: Option<&str>) -> Result<SiteConfig, GctlCliError> {
        match site.or(self.default_site.as_deref()) {
            None => Ok(SiteConfig {
                ghost_url: self.ghost_url.clone(),
                ghost_admin_key: self.ghost_admin_key.clone(),
            }),
            Some(name) => self.sites.get(name).cloned().ok_or_else(|| {
                let known: Vec<_> = self.sites.keys().map(String::as_str).collect();
                GctlCliError::Config(if known.is_empty() {
                    format!("No site named `{name}`. Add it with `gctl auth login --site {name}`.")
                } else {
                    format!(
                        "No site named `{name}`. Known sites: {}. Add one with `gctl auth login --site {name}`.",
                        known.join(", ")
                    )
                })
            }),
        }
    }

    pub fn save(&self) -> Result<PathBuf, GctlCliError> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let contents = serde_json::to_string_pretty(self)?;
        write_private(&path, &contents)?;
        Ok(path)
    }

    /// Returns whether a config file existed.
    pub fn delete() -> Result<bool, GctlCliError> {
        match fs::remove_file(Self::path()?) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
            Err(e) => Err(e.into()),
        }
    }
}

#[cfg(unix)]
fn write_private(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

    let mut file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    // `mode` only applies on creation; tighten a pre-existing file too.
    file.set_permissions(fs::Permissions::from_mode(0o600))?;
    file.write_all(contents.as_bytes())
}

#[cfg(not(unix))]
fn write_private(path: &std::path::Path, contents: &str) -> std::io::Result<()> {
    fs::write(path, contents)
}

#[cfg(test)]
mod test {
    use super::*;

    fn site(url: &str) -> SiteConfig {
        SiteConfig {
            ghost_url: Some(url.into()),
            ghost_admin_key: Some("id:secret".into()),
        }
    }

    fn config() -> StoredConfig {
        StoredConfig {
            ghost_url: Some("https://plain.test".into()),
            ghost_admin_key: Some("id:secret".into()),
            sites: BTreeMap::from([
                ("prod".into(), site("https://prod.test")),
                ("staging".into(), site("https://staging.test")),
            ]),
            default_site: None,
        }
    }

    fn url(config: &StoredConfig, site_name: Option<&str>) -> String {
        match config.site(site_name) {
            Ok(site) => site.ghost_url.unwrap_or_default(),
            Err(e) => e.to_string(),
        }
    }

    #[test]
    fn picks_the_site() {
        let mut with_default = config();
        with_default.default_site = Some("prod".into());
        let results = [
            ("unnamed", url(&config(), None)),
            ("named", url(&config(), Some("staging"))),
            ("default", url(&with_default, None)),
            ("flag beats default", url(&with_default, Some("staging"))),
            ("unknown", url(&config(), Some("nope"))),
            (
                "unknown, none saved",
                url(&StoredConfig::default(), Some("nope")),
            ),
        ];
        insta::assert_debug_snapshot!(results);
    }

    #[test]
    fn old_config_files_still_load() {
        let old = r#"{"ghost_url":"https://a.test","ghost_admin_key":"id:secret"}"#;
        let config: StoredConfig = serde_json::from_str(old).unwrap();
        assert_eq!(url(&config, None), "https://a.test");
        let written = serde_json::to_string(&config).unwrap();
        assert!(!written.contains("sites") && !written.contains("default_site"));
    }
}
