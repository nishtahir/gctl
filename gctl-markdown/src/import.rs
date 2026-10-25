//! Reads Ghost's Lexical JSON back into the markdown AST.

use serde_json::Value;

use crate::ast::{format, Block, Inline, ListItem};
use crate::lexical::MEMBERS_ONLY;
use crate::parser::merge_text;

/// A Lexical document converted to markdown blocks.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Imported {
    pub blocks: Vec<Block>,
    /// Kinds of content that markdown can't express and were approximated
    /// (e.g. `callout`, `bookmark`). Pushing the result back would change the
    /// post, so callers should flag these.
    pub lossy: Vec<String>,
}

#[derive(Debug, thiserror::Error)]
#[error("Invalid lexical content: {0}")]
pub struct ImportError(String);

/// Converts the `lexical` string of a Ghost post.
pub fn from_lexical(lexical: &str) -> Result<Imported, ImportError> {
    let value: Value = serde_json::from_str(lexical).map_err(|e| ImportError(e.to_string()))?;
    let root = value
        .get("root")
        .filter(|root| root.is_object())
        .ok_or_else(|| ImportError("missing `root`".into()))?;

    let mut imported = Imported::default();
    for child in children(root) {
        block(child, &mut imported);
    }
    imported.lossy.sort();
    imported.lossy.dedup();
    Ok(imported)
}

fn children(node: &Value) -> &[Value] {
    node.get("children")
        .and_then(Value::as_array)
        .map_or(&[], Vec::as_slice)
}

fn kind(node: &Value) -> &str {
    node.get("type").and_then(Value::as_str).unwrap_or_default()
}

fn string<'a>(node: &'a Value, key: &str) -> &'a str {
    node.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn block(node: &Value, out: &mut Imported) {
    match kind(node) {
        "paragraph" => {
            let content = inlines(children(node));
            if !is_blank(&content) {
                out.blocks.push(Block::Paragraph(content));
            }
        }
        "heading" | "extended-heading" => {
            let level = string(node, "tag")
                .trim_start_matches('h')
                .parse()
                .unwrap_or(2);
            out.blocks.push(Block::Heading {
                level,
                content: inlines(children(node)),
            });
        }
        "quote" | "extended-quote" | "aside" => {
            out.blocks
                .push(Block::Quote(paragraph_groups(inlines(children(node)))));
        }
        "list" => out.blocks.push(list(node)),
        "codeblock" => {
            let language = string(node, "language");
            out.blocks.push(Block::CodeBlock {
                language: (!language.is_empty()).then(|| language.to_string()),
                code: string(node, "code").to_string(),
            });
        }
        "horizontalrule" => out.blocks.push(Block::Rule),
        "html" => out
            .blocks
            .push(Block::Html(string(node, "html").to_string())),
        "paywall" => out.blocks.push(Block::Html(MEMBERS_ONLY.to_string())),
        "markdown" => {
            let markdown = string(node, "markdown");
            out.lossy.extend(unsupported_markdown(markdown));
            out.blocks.push(Block::Raw(markdown.to_string()));
        }
        "image" => {
            let caption = html_text(string(node, "caption"));
            let title = if caption.is_empty() {
                string(node, "title").to_string()
            } else {
                caption
            };
            let href = string(node, "href");
            out.blocks.push(Block::Image {
                src: string(node, "src").to_string(),
                alt: string(node, "alt").to_string(),
                title: (!title.is_empty()).then_some(title),
                href: (!href.is_empty()).then(|| href.to_string()),
            });
        }
        "callout" => {
            out.lossy.push("callout".into());
            let emoji = string(node, "calloutEmoji");
            let mut content = Vec::new();
            if !emoji.is_empty() {
                content.push(plain(&format!("{emoji} ")));
            }
            content.extend(html_inlines(string(node, "calloutText")));
            out.blocks.push(Block::Quote(vec![content]));
        }
        "bookmark" => {
            out.lossy.push("bookmark".into());
            let url = string(node, "url");
            let title = node
                .pointer("/metadata/title")
                .and_then(Value::as_str)
                .filter(|t| !t.is_empty())
                .unwrap_or(url);
            out.blocks.push(link_paragraph(url, title));
        }
        "embed" => {
            out.lossy.push("embed".into());
            let url = string(node, "url");
            out.blocks.push(link_paragraph(url, url));
        }
        "file" => {
            out.lossy.push("file".into());
            let title = [string(node, "fileTitle"), string(node, "fileName")]
                .into_iter()
                .find(|t| !t.is_empty())
                .unwrap_or("File");
            out.blocks.push(link_paragraph(string(node, "src"), title));
        }
        other => {
            out.lossy.push(other.to_string());
            // Keep whatever text it holds so nothing silently disappears.
            let content = inlines(children(node));
            if !is_blank(&content) {
                out.blocks.push(Block::Paragraph(content));
            }
        }
    }
}

fn link_paragraph(url: &str, title: &str) -> Block {
    Block::Paragraph(vec![Inline::Link {
        url: url.trim().to_string(),
        title: None,
        children: vec![plain(title)],
    }])
}

fn plain(text: &str) -> Inline {
    Inline::Text {
        text: text.to_string(),
        format: 0,
    }
}

fn is_blank(content: &[Inline]) -> bool {
    content.iter().all(|i| match i {
        Inline::Text { text, .. } => text.trim().is_empty(),
        Inline::LineBreak => true,
        _ => false,
    })
}

/// Quotes hold one run of inline nodes; two line breaks in a row separate paragraphs.
fn paragraph_groups(content: Vec<Inline>) -> Vec<Vec<Inline>> {
    let mut groups = vec![Vec::new()];
    let mut iter = content.into_iter().peekable();
    while let Some(inline) = iter.next() {
        if inline == Inline::LineBreak && iter.peek() == Some(&Inline::LineBreak) {
            iter.next();
            groups.push(Vec::new());
        } else {
            groups.last_mut().unwrap().push(inline);
        }
    }
    groups.retain(|g| !is_blank(g));
    groups
}

fn list(node: &Value) -> Block {
    let ordered = string(node, "listType") == "number";
    let start = node.get("start").and_then(Value::as_u64).unwrap_or(1) as u32;
    let mut items: Vec<ListItem> = Vec::new();

    for item in children(node) {
        let (nested, content): (Vec<&Value>, Vec<&Value>) =
            children(item).iter().partition(|c| kind(c) == "list");
        let text = inlines(&content.into_iter().cloned().collect::<Vec<_>>());
        let nested_lists: Vec<Block> = nested.into_iter().map(list).collect();

        if is_blank(&text) && !nested_lists.is_empty() {
            // Lexical's wrapper item: a nested list belongs to the previous item.
            match items.last_mut() {
                Some(previous) => previous.children.extend(nested_lists),
                None => items.push(ListItem {
                    content: Vec::new(),
                    children: nested_lists,
                }),
            }
        } else {
            items.push(ListItem {
                content: text,
                children: nested_lists,
            });
        }
    }
    Block::List {
        ordered,
        start,
        items,
    }
}

fn inlines(nodes: &[Value]) -> Vec<Inline> {
    let mut out = Vec::new();
    for node in nodes {
        match kind(node) {
            "text" | "extended-text" | "code-highlight" => {
                let fmt = node.get("format").and_then(Value::as_u64).unwrap_or(0) as u32;
                let fmt =
                    fmt & (format::BOLD | format::ITALIC | format::STRIKETHROUGH | format::CODE);
                // Some editors store line breaks as newlines inside the text.
                for (n, line) in string(node, "text").split('\n').enumerate() {
                    if n > 0 {
                        out.push(Inline::LineBreak);
                    }
                    out.push(Inline::Text {
                        text: line.to_string(),
                        format: fmt,
                    });
                }
            }
            "linebreak" => out.push(Inline::LineBreak),
            "link" | "autolink" => {
                let title = string(node, "title");
                out.push(Inline::Link {
                    url: string(node, "url").trim().to_string(),
                    title: (!title.is_empty()).then(|| title.to_string()),
                    children: inlines(children(node)),
                });
            }
            _ => out.extend(inlines(children(node))),
        }
    }
    merge_text(out)
}

/// Syntax that Ghost's markdown card renders but this tool's parser doesn't.
fn unsupported_markdown(markdown: &str) -> Vec<String> {
    let mut found = Vec::new();
    let lines: Vec<&str> = markdown.lines().collect();
    let separator = |l: &str| {
        let l = l.trim();
        l.contains('-') && l.contains('|') && l.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '))
    };
    if lines
        .windows(2)
        .any(|w| w[0].contains('|') && separator(w[1]))
    {
        found.push("table".to_string());
    }
    if markdown.contains("[^") || markdown.contains("^[") {
        found.push("footnote".to_string());
    }
    if lines.iter().any(|l| {
        let l = l.trim_start();
        ["- [ ]", "- [x]", "* [ ]", "* [x]"]
            .iter()
            .any(|p| l.starts_with(p))
    }) {
        found.push("task list".to_string());
    }
    found
}

// ---- minimal HTML for callouts and captions ----

/// An open tag (name and optional `href`) with the inlines collected inside it.
type Frame = (Option<(String, Option<String>)>, Vec<Inline>);

/// Converts the inline HTML Ghost stores in callouts and captions.
fn html_inlines(html: &str) -> Vec<Inline> {
    let mut stack: Vec<Frame> = vec![(None, Vec::new())];
    let mut fmt = 0u32;
    let mut rest = html;

    while !rest.is_empty() {
        let Some(open) = rest.find('<') else {
            push_text(&mut stack, &decode(rest), fmt);
            break;
        };
        push_text(&mut stack, &decode(&rest[..open]), fmt);
        let Some(close) = rest[open..].find('>') else {
            push_text(&mut stack, &decode(&rest[open..]), fmt);
            break;
        };
        let tag = &rest[open + 1..open + close];
        rest = &rest[open + close + 1..];

        let closing = tag.starts_with('/');
        let name = tag
            .trim_start_matches('/')
            .split(|c: char| c.is_whitespace() || c == '/')
            .next()
            .unwrap_or_default()
            .to_ascii_lowercase();
        let bit = match name.as_str() {
            "strong" | "b" => format::BOLD,
            "em" | "i" => format::ITALIC,
            "s" | "del" | "strike" => format::STRIKETHROUGH,
            "code" => format::CODE,
            _ => 0,
        };
        match (name.as_str(), closing) {
            ("br", _) => stack.last_mut().unwrap().1.push(Inline::LineBreak),
            ("a", false) => {
                let href = attribute(tag, "href").unwrap_or_default();
                stack.push((Some((href, attribute(tag, "title"))), Vec::new()));
            }
            ("a", true) if stack.len() > 1 => {
                let (link, children) = stack.pop().unwrap();
                if let Some((url, title)) = link {
                    stack.last_mut().unwrap().1.push(Inline::Link {
                        url,
                        title,
                        children: merge_text(children),
                    });
                }
            }
            _ if bit != 0 => fmt = if closing { fmt & !bit } else { fmt | bit },
            _ => {}
        }
    }
    while stack.len() > 1 {
        let (_, children) = stack.pop().unwrap();
        stack.last_mut().unwrap().1.extend(children);
    }
    merge_text(stack.pop().unwrap().1)
}

fn push_text(
    stack: &mut [Frame],
    text: &str,
    format: u32,
) {
    if !text.is_empty() {
        stack.last_mut().unwrap().1.push(Inline::Text {
            text: text.to_string(),
            format,
        });
    }
}

fn attribute(tag: &str, name: &str) -> Option<String> {
    let start = tag.find(&format!("{name}=\""))? + name.len() + 2;
    let end = tag[start..].find('"')?;
    Some(decode(&tag[start..start + end]))
}

/// Plain text of an HTML fragment.
fn html_text(html: &str) -> String {
    html_inlines(html)
        .iter()
        .map(|inline| match inline {
            Inline::Text { text, .. } => text.clone(),
            Inline::Link { children, .. } => children
                .iter()
                .filter_map(|c| match c {
                    Inline::Text { text, .. } => Some(text.as_str()),
                    _ => None,
                })
                .collect(),
            _ => " ".to_string(),
        })
        .collect::<String>()
        .trim()
        .to_string()
}

fn decode(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        rest = &rest[amp..];
        let decoded = rest.find(';').filter(|&end| end <= 10).and_then(|end| {
            let entity = &rest[1..end];
            let c = match entity {
                "amp" => Some('&'),
                "lt" => Some('<'),
                "gt" => Some('>'),
                "quot" => Some('"'),
                "apos" => Some('\''),
                "nbsp" => Some(' '),
                _ => entity
                    .strip_prefix("#x")
                    .and_then(|h| u32::from_str_radix(h, 16).ok())
                    .or_else(|| entity.strip_prefix('#').and_then(|d| d.parse().ok()))
                    .and_then(char::from_u32),
            };
            c.map(|c| (c, end))
        });
        match decoded {
            Some((c, end)) => {
                out.push(c);
                rest = &rest[end + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}
