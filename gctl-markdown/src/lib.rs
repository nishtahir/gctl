//! Compiles Hugo-style markdown files (YAML front matter + markdown body)
//! into Ghost Lexical documents.

mod ast;
mod edit;
mod error;
mod front_matter;
mod import;
mod include;
mod lexical;
mod parser;
mod render;

pub use ast::*;
pub use edit::set_front_matter_id;
pub use error::MarkdownError;
pub use front_matter::{FrontMatter, Injection};
pub use import::{from_lexical, ImportError, Imported};
pub use include::Loader;
pub use parser::parse;
pub use render::to_markdown;
