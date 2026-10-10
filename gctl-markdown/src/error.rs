use thiserror::Error;

use crate::parser::Rule;

#[derive(Error, Debug)]
pub enum MarkdownError {
    #[error("Failed to parse markdown: {0}")]
    Parse(#[from] Box<pest::error::Error<Rule>>),
    #[error("Invalid front matter: {0}")]
    FrontMatter(#[from] serde_yaml::Error),
    #[error("Include error: {0}")]
    Include(String),
    #[error("Invalid date '{0}': expected e.g. 2024-01-31 or 2024-01-31T10:00:00Z")]
    Date(String),
}
