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

impl From<ureq::Error> for GhostError {
    fn from(err: ureq::Error) -> GhostError {
        match err {
            ureq::Error::Status(401, response) => {
                let status = response.status_text().to_string();
                let body = response.into_string().unwrap_or_default();
                eprintln!("Auth Error Response: {}", body);
                GhostError::AuthError(format!("Invalid credentials: {}", status))
            }
            ureq::Error::Status(code, response) => {
                let status = response.status_text().to_string();
                let body = response.into_string().unwrap_or_default();
                eprintln!("HTTP Error Response: {}", body);
                GhostError::HttpError(format!("Status {}: {}", code, status))
            }
            ureq::Error::Transport(e) => GhostError::HttpError(format!("Transport error: {}", e)),
        }
    }
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
