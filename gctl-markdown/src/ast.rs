use crate::front_matter::FrontMatter;

/// A parsed content file.
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub front_matter: FrontMatter,
    pub body: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Heading {
        level: u8,
        content: Vec<Inline>,
    },
    Paragraph(Vec<Inline>),
    CodeBlock {
        language: Option<String>,
        code: String,
    },
    /// Each entry is one line group of inline content (Lexical quotes hold
    /// inline nodes directly).
    Quote(Vec<Vec<Inline>>),
    List {
        ordered: bool,
        start: u32,
        items: Vec<ListItem>,
    },
    Rule,
    Html(String),
    /// `{{< include "path" >}}`: content from another file. Replaced by
    /// [`Document::resolve_includes`]; with `lang`, the file becomes a code block.
    Include {
        path: String,
        lang: Option<String>,
    },
    /// Markdown carried over verbatim, e.g. from a Ghost markdown card.
    Raw(String),
    Image {
        src: String,
        alt: String,
        title: Option<String>,
        href: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListItem {
    pub content: Vec<Inline>,
    /// Nested lists.
    pub children: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Text {
        text: String,
        format: u32,
    },
    Link {
        url: String,
        title: Option<String>,
        children: Vec<Inline>,
    },
    Image {
        src: String,
        alt: String,
        title: Option<String>,
    },
    LineBreak,
}

/// Lexical text format bit flags.
pub mod format {
    pub const BOLD: u32 = 1;
    pub const ITALIC: u32 = 2;
    pub const STRIKETHROUGH: u32 = 4;
    pub const CODE: u32 = 16;
}
