use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::GctlCliError;

/// Credentials persisted by `gctl auth login`.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct StoredConfig {
    pub ghost_url: Option<String>,
    pub ghost_admin_key: Option<String>,
}

impl StoredConfig {
    pub fn path() -> Result<PathBuf, GctlCliError> {
        let dir = dirs::config_dir()
            .ok_or_else(|| GctlCliError::ConfigError("Could not locate config directory".into()))?;
        Ok(dir.join("gctl").join("config.json"))
    }

    /// Returns an empty config when nothing has been stored yet.
    pub fn load() -> Result<Self, GctlCliError> {
        match fs::read_to_string(Self::path()?) {
            Ok(contents) => serde_json::from_str(&contents)
                .map_err(|e| GctlCliError::ConfigError(format!("Invalid config file: {e}"))),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
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
