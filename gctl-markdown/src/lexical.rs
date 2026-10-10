use serde_json::{json, Value};

use crate::ast::{Block, Document, Inline, ListItem};

impl Document {
    /// The Lexical editor state Ghost stores in a post's `lexical` field.
    pub fn to_lexical(&self) -> Value {
        json!({
            "root": {
                "children": blocks(&self.body),
                "direction": "ltr",
                "format": "",
                "indent": 0,
                "type": "root",
                "version": 1
            }
        })
    }

    /// [`to_lexical`](Self::to_lexical) serialized, as the Admin API expects.
    pub fn to_lexical_string(&self) -> String {
        self.to_lexical().to_string()
    }
}

fn element(kind: &str, children: Vec<Value>) -> Value {
    json!({
        "children": children,
        "direction": "ltr",
        "format": "",
        "indent": 0,
        "type": kind,
        "version": 1
    })
}

fn with(mut node: Value, extra: Value) -> Value {
    if let (Some(node), Some(extra)) = (node.as_object_mut(), extra.as_object()) {
        node.extend(extra.clone());
    }
    node
}

/// Raw markdown is compiled like any other, so it can span several nodes.
fn blocks(list: &[Block]) -> Vec<Value> {
    let mut out = Vec::new();
    for b in list {
        match b {
            Block::Raw(markdown) => {
                let parsed = crate::parser::parse_blocks(markdown).unwrap_or_default();
                out.extend(blocks(&parsed));
            }
            other => out.push(block(other)),
        }
    }
    out
}

fn block(b: &Block) -> Value {
    match b {
        Block::Heading { level, content } => with(
            element("extended-heading", inlines(content)),
            json!({ "tag": format!("h{level}") }),
        ),
        Block::Paragraph(content) => element("paragraph", inlines(content)),
        Block::CodeBlock { language, code } => json!({
            "type": "codeblock",
            "version": 1,
            "code": code,
            "language": language.clone().unwrap_or_default(),
            "caption": ""
        }),
        Block::Quote(groups) => {
            let mut children = Vec::new();
            for (i, group) in groups.iter().enumerate() {
                if i > 0 {
                    // Paragraph breaks inside a quote.
                    children.push(linebreak());
                    children.push(linebreak());
                }
                children.extend(inlines(group));
            }
            element("extended-quote", children)
        }
        Block::List {
            ordered,
            start,
            items,
        } => list(*ordered, *start, items, 0),
        Block::Rule => json!({ "type": "horizontalrule", "version": 1 }),
        Block::Html(html) if html.trim() == MEMBERS_ONLY => {
            json!({ "type": "paywall", "version": 1 })
        }
        Block::Html(html) => json!({ "type": "html", "version": 1, "html": html }),
        Block::Raw(_) => unreachable!("raw markdown is expanded by `blocks`"),
        // Unresolved: show the directive instead of silently dropping content.
        Block::Include { path, lang } => element(
            "paragraph",
            vec![inline(&Inline::Text {
                text: crate::parser::directive(path, lang.as_deref()),
                format: 0,
            })],
        ),
        Block::Image {
            src,
            alt,
            title,
            href,
        } => json!({
            "type": "image",
            "version": 1,
            "src": src,
            "width": null,
            "height": null,
            "title": "",
            "alt": alt,
            // A markdown image title becomes the visible caption.
            "caption": escape_html(title.as_deref().unwrap_or_default()),
            "cardWidth": "regular",
            "href": href.clone().unwrap_or_default()
        }),
    }
}

/// The comment Ghost's HTML uses for the members-only divider.
pub(crate) const MEMBERS_ONLY: &str = "<!--members-only-->";

fn escape_html(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

fn list(ordered: bool, start: u32, items: &[ListItem], indent: u32) -> Value {
    let mut children = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let value = start + i as u32;
        children.push(with(
            element("listitem", inlines(&item.content)),
            json!({ "value": value, "indent": indent }),
        ));
        // Lexical nests a list inside a wrapper item that holds only that list.
        for nested in &item.children {
            if let Block::List {
                ordered,
                start,
                items,
            } = nested
            {
                children.push(with(
                    element("listitem", vec![list(*ordered, *start, items, indent + 1)]),
                    json!({ "value": value, "indent": indent }),
                ));
            }
        }
    }
    with(
        element("list", children),
        json!({
            "listType": if ordered { "number" } else { "bullet" },
            "start": start,
            "tag": if ordered { "ol" } else { "ul" }
        }),
    )
}

fn linebreak() -> Value {
    json!({ "type": "linebreak", "version": 1 })
}

fn inlines(content: &[Inline]) -> Vec<Value> {
    content.iter().map(inline).collect()
}

fn inline(i: &Inline) -> Value {
    match i {
        Inline::Text { text, format } => json!({
            "detail": 0,
            "format": format,
            "mode": "normal",
            "style": "",
            "text": text,
            "type": "extended-text",
            "version": 1
        }),
        Inline::Link {
            url,
            title,
            children,
        } => with(
            element("link", inlines(children)),
            json!({ "rel": null, "target": null, "title": title, "url": url }),
        ),
        // Images are hoisted into blocks while parsing paragraphs; this only
        // keeps the alt text if one ends up somewhere unexpected.
        Inline::Image { alt, .. } => inline(&Inline::Text {
            text: alt.clone(),
            format: 0,
        }),
        Inline::LineBreak => linebreak(),
    }
}
