use crate::parser::Rule;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum GhostLiquidError {
    #[error("Parser error: {0}")]
    Parser(#[from] pest::error::Error<Rule>),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid block structure: {0}")]
    Structure(String),

    #[error("Missing required parameter: {0}")]
    MissingParam(String),

    #[error("Invalid block nesting: {0}")]
    InvalidNesting(String),
}
