use gctl_ghost::GhostError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GctlCliError {
    #[error("Failed to parse URL: {0}")]
    UrlParseError(#[from] url::ParseError),
    #[error("Ghost API error: {0}")]
    GhostError(#[from] GhostError),
    #[error("Configuration error: {0}")]
    ConfigError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
}

impl From<std::io::Error> for GctlCliError {
    fn from(value: std::io::Error) -> Self {
        GctlCliError::ConfigError(value.to_string())
    }
}

impl From<serde_json::Error> for GctlCliError {
    fn from(err: serde_json::Error) -> Self {
        GctlCliError::ParseError(err.to_string())
    }
}
