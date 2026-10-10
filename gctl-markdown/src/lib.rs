//! Compiles Hugo-style markdown files (YAML front matter + markdown body)
//! into Ghost Lexical documents.

mod ast;
mod edit;
mod error;
mod front_matter;
mod lexical;
mod parser;

pub use ast::*;
pub use edit::set_front_matter_id;
pub use error::MarkdownError;
pub use front_matter::FrontMatter;
pub use parser::parse;
