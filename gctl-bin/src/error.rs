use gctl_ghost::GhostError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GctlCliError {
    #[error("Failed to parse URL: {0}")]
    UrlParse(#[from] url::ParseError),
    #[error("Ghost API error: {0}")]
    Ghost(#[from] GhostError),
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("Markdown error: {0}")]
    Markdown(#[from] gctl_markdown::MarkdownError),
    #[error("{0}")]
    Push(String),
    #[error("Parse error: {0}")]
    Parse(String),
}

impl From<std::io::Error> for GctlCliError {
    fn from(value: std::io::Error) -> Self {
        GctlCliError::Config(value.to_string())
    }
}

impl From<serde_json::Error> for GctlCliError {
    fn from(err: serde_json::Error) -> Self {
        GctlCliError::Parse(err.to_string())
    }
}
