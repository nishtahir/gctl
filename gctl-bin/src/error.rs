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
    /// Local and remote content disagree and nothing was overwritten.
    #[error("{0}")]
    Conflict(String),
    #[error("Parse error: {0}")]
    Parse(String),
}

impl GctlCliError {
    /// The process exit code: 1 for failures, 3 when the only problem is
    /// content that differs and was left alone. (2 is clap's, for bad usage.)
    pub fn exit_code(&self) -> i32 {
        match self {
            GctlCliError::Conflict(_) => 3,
            _ => 1,
        }
    }
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

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn conflicts_have_their_own_exit_code() {
        let codes = [
            GctlCliError::Conflict("x".into()).exit_code(),
            GctlCliError::Push("x".into()).exit_code(),
            GctlCliError::Config("x".into()).exit_code(),
        ];
        assert_eq!(codes, [3, 1, 1]);
    }
}
