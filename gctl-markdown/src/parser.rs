use pest::iterators::Pair;
use pest::Parser;

use crate::ast::{format, Block, Document, Inline, ListItem};
use crate::error::MarkdownError;
use crate::front_matter::FrontMatter;

#[derive(pest_derive::Parser)]
#[grammar = "markdown.pest"]
struct MarkdownParser;

/// Parses a content file: optional YAML front matter followed by markdown.
pub fn parse(input: &str) -> Result<Document, MarkdownError> {
    let input = input.strip_prefix('\u{feff}').unwrap_or(input);
    let input = input.replace("\r\n", "\n").replace('\r', "\n");

    let document = MarkdownParser::parse(Rule::document, &input)
        .map_err(Box::new)?
        .next()
        .expect("document rule always produces a pair");

    let mut front_matter = FrontMatter::default();
    let mut body = Vec::new();
    for pair in document.into_inner() {
        match pair.as_rule() {
            Rule::frontmatter => {
                let yaml = pair.into_inner().next().map_or("", |p| p.as_str());
                if !yaml.trim().is_empty() {
                    front_matter = serde_yaml::from_str(yaml)?;
                }
            }
            Rule::body => body = parse_body(pair)?,
            _ => {}
        }
    }
    Ok(Document { front_matter, body })
}

fn parse_blocks(input: &str) -> Result<Vec<Block>, MarkdownError> {
    let pair = MarkdownParser::parse(Rule::blocks_only, input)
        .map_err(Box::new)?
        .next()
        .and_then(|p| p.into_inner().next())
        .expect("blocks_only always contains a body");
    parse_body(pair)
}

fn parse_body(body: Pair<Rule>) -> Result<Vec<Block>, MarkdownError> {
    let mut blocks = Vec::new();
    for pair in body.into_inner() {
        match pair.as_rule() {
            Rule::heading => {
                let mut inner = pair.into_inner();
                let level = inner.next().unwrap().as_str().len() as u8;
                let text = strip_closing_hashes(inner.next().unwrap().as_str().trim());
                blocks.push(Block::Heading {
                    level,
                    content: parse_inlines(text)?,
                });
            }
            Rule::fenced_code => {
                let mut inner = pair.into_inner();
                let language = inner
                    .next()
                    .and_then(|info| info.as_str().split_whitespace().next().map(str::to_string));
                let code = inner.next().map_or("", |p| p.as_str());
                blocks.push(Block::CodeBlock {
                    language,
                    code: code.strip_suffix('\n').unwrap_or(code).to_string(),
                });
            }
            Rule::hr => blocks.push(Block::Rule),
            Rule::blockquote => {
                let text = pair
                    .into_inner()
                    .filter_map(|line| line.into_inner().next())
                    .map(|text| text.as_str())
                    .collect::<Vec<_>>()
                    .join("\n");
                blocks.push(quote(parse_blocks(&text)?));
            }
            Rule::ulist => blocks.push(parse_list(pair, false)?),
            Rule::olist => blocks.push(parse_list(pair, true)?),
            Rule::html_block => blocks.push(Block::Html(pair.as_str().trim_end().to_string())),
            Rule::paragraph => blocks.extend(paragraph(pair.as_str())?),
            _ => {}
        }
    }
    Ok(blocks)
}

/// `# Title ##` -> `Title`
fn strip_closing_hashes(text: &str) -> &str {
    let stripped = text.trim_end_matches('#');
    if stripped.len() != text.len() && stripped.ends_with(' ') {
        stripped.trim_end()
    } else {
        text
    }
}

/// Splits paragraph text so images become their own blocks; Lexical images
/// are cards, not inline nodes.
fn paragraph(raw: &str) -> Result<Vec<Block>, MarkdownError> {
    let text = raw
        .lines()
        .map(str::trim_start)
        .collect::<Vec<_>>()
        .join("\n");
    let mut blocks = Vec::new();
    let mut current: Vec<Inline> = Vec::new();

    let flush = |current: &mut Vec<Inline>, blocks: &mut Vec<Block>| {
        let inlines = std::mem::take(current);
        if !is_blank(&inlines) {
            blocks.push(Block::Paragraph(trim_inlines(inlines)));
        }
    };

    for inline in parse_inlines(text.trim_end())? {
        match inline {
            Inline::Image { src, alt, title } => {
                flush(&mut current, &mut blocks);
                blocks.push(Block::Image {
                    src,
                    alt,
                    title,
                    href: None,
                });
            }
            // [![alt](src)](href) -> a linked image card
            Inline::Link {
                url,
                title: _,
                children,
            } if matches!(children.as_slice(), [Inline::Image { .. }]) => {
                flush(&mut current, &mut blocks);
                if let Some(Inline::Image { src, alt, title }) = children.into_iter().next() {
                    blocks.push(Block::Image {
                        src,
                        alt,
                        title,
                        href: Some(url),
                    });
                }
            }
            other => current.push(other),
        }
    }
    flush(&mut current, &mut blocks);
    Ok(blocks)
}

fn is_blank(inlines: &[Inline]) -> bool {
    inlines.iter().all(|i| match i {
        Inline::Text { text, .. } => text.trim().is_empty(),
        Inline::LineBreak => true,
        _ => false,
    })
}

/// Drops whitespace and line breaks at both ends (left behind when an image is split out).
fn trim_inlines(mut inlines: Vec<Inline>) -> Vec<Inline> {
    while matches!(inlines.first(), Some(Inline::LineBreak)) {
        inlines.remove(0);
    }
    while matches!(inlines.last(), Some(Inline::LineBreak)) {
        inlines.pop();
    }
    if let Some(Inline::Text { text, .. }) = inlines.first_mut() {
        *text = text.trim_start().to_string();
    }
    if let Some(Inline::Text { text, .. }) = inlines.last_mut() {
        *text = text.trim_end().to_string();
    }
    inlines.retain(|i| !matches!(i, Inline::Text { text, .. } if text.is_empty()));
    inlines
}

/// Lexical quotes hold inline nodes, so inner blocks are flattened into line groups.
fn quote(blocks: Vec<Block>) -> Block {
    let mut groups = Vec::new();
    for block in blocks {
        match block {
            Block::Paragraph(inlines)
            | Block::Heading {
                content: inlines, ..
            } => groups.push(inlines),
            Block::List { items, .. } => {
                for item in items {
                    let mut line = vec![Inline::Text {
                        text: "- ".into(),
                        format: 0,
                    }];
                    line.extend(item.content);
                    groups.push(line);
                }
            }
            Block::CodeBlock { code, .. } => groups.push(vec![Inline::Text {
                text: code,
                format: format::CODE,
            }]),
            _ => {}
        }
    }
    Block::Quote(groups)
}

fn parse_list(pair: Pair<Rule>, ordered: bool) -> Result<Block, MarkdownError> {
    let mut start = 1;
    let mut items = Vec::new();

    for (index, item) in pair.into_inner().enumerate() {
        let mut marker = "";
        let mut text = "";
        let mut continuation: Vec<&str> = Vec::new();
        for part in item.into_inner() {
            match part.as_rule() {
                Rule::ol_marker | Rule::ul_marker => marker = part.as_str(),
                Rule::item_text => text = part.as_str(),
                Rule::item_cont => continuation.push(part.as_str().trim_end_matches('\n')),
                _ => {}
            }
        }
        if ordered && index == 0 {
            start = marker.trim_end_matches(['.', ')']).parse().unwrap_or(1);
        }

        let mut content = parse_inlines(text.trim())?;
        let mut children = Vec::new();

        if !continuation.is_empty() {
            let indent = continuation
                .iter()
                .map(|l| l.len() - l.trim_start().len())
                .min()
                .unwrap_or(0);
            let dedented = continuation
                .iter()
                .map(|l| &l[indent..])
                .collect::<Vec<_>>()
                .join("\n");
            for block in parse_blocks(&dedented)? {
                match block {
                    Block::List { .. } => children.push(block),
                    // Lazy continuation text belongs to the item itself.
                    Block::Paragraph(inlines) => {
                        content.push(Inline::Text {
                            text: " ".into(),
                            format: 0,
                        });
                        content.extend(inlines);
                    }
                    _ => {}
                }
            }
        }
        items.push(ListItem {
            content: merge_text(content),
            children,
        });
    }

    Ok(Block::List {
        ordered,
        start,
        items,
    })
}

fn parse_inlines(text: &str) -> Result<Vec<Inline>, MarkdownError> {
    let pairs = MarkdownParser::parse(Rule::inlines, text).map_err(Box::new)?;
    let mut out = Vec::new();
    for pair in pairs.flat_map(|p| p.into_inner()) {
        walk_inline(pair, 0, &mut out);
    }
    Ok(merge_text(out))
}

fn walk_inline(pair: Pair<Rule>, fmt: u32, out: &mut Vec<Inline>) {
    let text = |text: &str, format| Inline::Text {
        text: text.to_string(),
        format,
    };
    match pair.as_rule() {
        Rule::text | Rule::escaped_char => out.push(text(pair.as_str(), fmt)),
        Rule::escaped => walk_children(pair, fmt, out),
        Rule::code_span => {
            if let Some(content) = pair.into_inner().next() {
                out.push(text(content.as_str(), fmt | format::CODE));
            }
        }
        Rule::strong => walk_children_with(pair, fmt | format::BOLD, out),
        Rule::emph => walk_children_with(pair, fmt | format::ITALIC, out),
        Rule::strike => walk_children_with(pair, fmt | format::STRIKETHROUGH, out),
        Rule::soft_break => out.push(text(" ", fmt)),
        Rule::hard_break => out.push(Inline::LineBreak),
        Rule::image => {
            let (mut alt, mut src, mut title) = (String::new(), String::new(), None);
            for part in pair.into_inner() {
                match part.as_rule() {
                    Rule::alt => alt = part.as_str().to_string(),
                    Rule::url => src = part.as_str().to_string(),
                    Rule::title => title = Some(unquote(part.as_str())),
                    _ => {}
                }
            }
            out.push(Inline::Image { src, alt, title });
        }
        Rule::link => {
            let (mut url, mut title, mut children) = (String::new(), None, Vec::new());
            for part in pair.into_inner() {
                match part.as_rule() {
                    Rule::url => url = part.as_str().to_string(),
                    Rule::title => title = Some(unquote(part.as_str())),
                    _ => walk_inline(part, fmt, &mut children),
                }
            }
            out.push(Inline::Link {
                url,
                title,
                children: merge_text(children),
            });
        }
        _ => {}
    }
}

fn walk_children(pair: Pair<Rule>, fmt: u32, out: &mut Vec<Inline>) {
    walk_children_with(pair, fmt, out)
}

fn walk_children_with(pair: Pair<Rule>, fmt: u32, out: &mut Vec<Inline>) {
    for child in pair.into_inner() {
        walk_inline(child, fmt, out);
    }
}

fn unquote(title: &str) -> String {
    title.trim_matches('"').to_string()
}

/// Joins neighbouring text with identical formatting.
fn merge_text(inlines: Vec<Inline>) -> Vec<Inline> {
    let mut merged: Vec<Inline> = Vec::new();
    for inline in inlines {
        if let (
            Some(Inline::Text {
                text: previous,
                format: previous_format,
            }),
            Inline::Text { text, format },
        ) = (merged.last_mut(), &inline)
        {
            if previous_format == format {
                previous.push_str(text);
                continue;
            }
        }
        merged.push(inline);
    }
    merged
}
