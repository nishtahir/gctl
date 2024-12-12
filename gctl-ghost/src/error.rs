use thiserror::Error;

#[derive(Debug, Error)]
pub enum GhostError {
    #[error("HTTP error: {0}")]
    HttpError(String),
    #[error("Authentication error: {0}")]
    AuthError(String),
    #[error("Parse error: {0}")]
    ParseError(String),
    #[error("API error: {0}")]
    ApiError(String),
}

impl From<url::ParseError> for GhostError {
    fn from(err: url::ParseError) -> GhostError {
        GhostError::ParseError(err.to_string())
    }
}

impl From<jsonwebtoken::errors::Error> for GhostError {
    fn from(err: jsonwebtoken::errors::Error) -> GhostError {
        GhostError::AuthError(err.to_string())
    }
}

impl From<std::io::Error> for GhostError {
    fn from(err: std::io::Error) -> GhostError {
        GhostError::HttpError(err.to_string())
    }
}

impl From<serde_json::Error> for GhostError {
    fn from(err: serde_json::Error) -> GhostError {
        GhostError::ParseError(err.to_string())
    }
}

impl From<reqwest::Error> for GhostError {
    fn from(err: reqwest::Error) -> Self {
        GhostError::HttpError(err.to_string())
    }
}
