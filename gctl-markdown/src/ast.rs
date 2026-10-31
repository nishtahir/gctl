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
    /// A GFM pipe table. Every row has one cell per column.
    Table {
        align: Vec<Align>,
        header: Vec<Vec<Inline>>,
        rows: Vec<Vec<Vec<Inline>>>,
    },
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    None,
    Left,
    Center,
    Right,
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

impl Document {
    /// Calls `f` with the `src` of every image in the body (not the front
    /// matter), wherever it appears.
    pub fn visit_images_mut(&mut self, f: &mut dyn FnMut(&mut String)) {
        blocks_images(&mut self.body, f);
    }
}

fn blocks_images(blocks: &mut [Block], f: &mut dyn FnMut(&mut String)) {
    for block in blocks {
        match block {
            Block::Image { src, .. } => f(src),
            Block::Heading { content, .. } | Block::Paragraph(content) => {
                inlines_images(content, f)
            }
            Block::Quote(lines) => lines.iter_mut().for_each(|l| inlines_images(l, f)),
            Block::List { items, .. } => {
                for item in items {
                    inlines_images(&mut item.content, f);
                    blocks_images(&mut item.children, f);
                }
            }
            Block::Table { header, rows, .. } => {
                let cells = header.iter_mut().chain(rows.iter_mut().flatten());
                cells.for_each(|cell| inlines_images(cell, f));
            }
            Block::CodeBlock { .. }
            | Block::Rule
            | Block::Html(_)
            | Block::Include { .. }
            | Block::Raw(_) => {}
        }
    }
}

fn inlines_images(inlines: &mut [Inline], f: &mut dyn FnMut(&mut String)) {
    for inline in inlines {
        match inline {
            Inline::Image { src, .. } => f(src),
            Inline::Link { children, .. } => inlines_images(children, f),
            Inline::Text { .. } | Inline::LineBreak => {}
        }
    }
}

#[cfg(test)]
mod test {
    use crate::parse;

    #[test]
    fn visits_images_wherever_they_are() {
        let mut doc = parse(
            "![a](one.png)\n\n- item ![b](two.png)\n\n> quote\n\n| h |\n|---|\n| ![d](four.png) |\n\n[![e](five.png)](https://x.test)\n\n```\n![f](six.png)\n```\n",
        )
        .unwrap();
        let mut seen = Vec::new();
        doc.visit_images_mut(&mut |src| {
            seen.push(src.clone());
            src.insert_str(0, "u/");
        });
        let mut again = Vec::new();
        doc.visit_images_mut(&mut |src| again.push(src.clone()));
        insta::assert_debug_snapshot!((seen, again));
    }
}
