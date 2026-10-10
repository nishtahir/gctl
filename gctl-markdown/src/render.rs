use crate::ast::{format, Block, Inline, ListItem};
use crate::parser::merge_text;

/// Renders blocks back to markdown that [`parse`](crate::parse) reads as the
/// same document.
pub fn to_markdown(blocks: &[Block]) -> String {
    let rendered: Vec<String> = blocks
        .iter()
        .map(render_block)
        .filter(|b| !b.trim().is_empty())
        .collect();
    if rendered.is_empty() {
        return String::new();
    }
    let mut out = rendered.join("\n\n");
    out.push('\n');
    out
}

fn render_block(block: &Block) -> String {
    match block {
        Block::Heading { level, content } => {
            format!("{} {}", "#".repeat(*level as usize), inline_line(content))
        }
        Block::Paragraph(content) => paragraph(content),
        Block::CodeBlock { language, code } => {
            // The fence must be longer than any run of backticks in the code.
            let longest = code.split(|c| c != '`').map(str::len).max().unwrap_or(0);
            let fence = "`".repeat((longest + 1).max(3));
            format!(
                "{fence}{}\n{code}\n{fence}",
                language.as_deref().unwrap_or("")
            )
        }
        Block::Quote(groups) => groups
            .iter()
            .map(|group| prefix_lines(&paragraph(group), "> "))
            .collect::<Vec<_>>()
            .join("\n>\n"),
        Block::List {
            ordered,
            start,
            items,
        } => list(*ordered, *start, items),
        Block::Rule => "---".to_string(),
        Block::Include { path, lang } => crate::parser::directive(path, lang.as_deref()),
        Block::Html(html) => html_block(html),
        Block::Raw(markdown) => markdown.trim().to_string(),
        Block::Image {
            src,
            alt,
            title,
            href,
        } => {
            let image = image(src, alt, title.as_deref());
            match href {
                Some(href) => format!("[{image}]({})", url(href)),
                None => image,
            }
        }
    }
}

/// Blank lines end an HTML block in markdown, except inside `<pre>`-style
/// blocks and comments, which run to their closing tag. Elsewhere they are
/// only formatting, so they are dropped to keep the block in one piece.
fn html_block(html: &str) -> String {
    let html = html.trim_end();
    let lower = html.trim_start().to_ascii_lowercase();
    let self_delimiting = ["<pre", "<script", "<style", "<textarea", "<!--"]
        .iter()
        .any(|tag| lower.starts_with(tag));
    if self_delimiting {
        return html.to_string();
    }
    html.lines()
        .filter(|line| !line.trim().is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

fn prefix_lines(text: &str, prefix: &str) -> String {
    text.lines()
        .map(|line| format!("{prefix}{line}").trim_end().to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

fn paragraph(content: &[Inline]) -> String {
    let rendered = inlines(content);
    let lines: Vec<String> = rendered
        .trim()
        .split('\n')
        .map(|line| escape_line_start(line.trim()))
        .collect();
    lines.join("\n")
}

/// Inline content on a single line (headings can't contain breaks).
fn inline_line(content: &[Inline]) -> String {
    let flat: Vec<Inline> = content
        .iter()
        .map(|inline| match inline {
            Inline::LineBreak => Inline::Text {
                text: " ".into(),
                format: 0,
            },
            other => other.clone(),
        })
        .collect();
    inlines(&flat).replace('\n', " ").trim().to_string()
}

fn list(ordered: bool, start: u32, items: &[ListItem]) -> String {
    let mut lines = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let marker = if ordered {
            format!("{}. ", start + i as u32)
        } else {
            "- ".to_string()
        };
        let indent = " ".repeat(marker.len());
        // Two line breaks in a row are a paragraph break inside the item.
        for (n, group) in paragraph_groups(&item.content).iter().enumerate() {
            let text = inlines(group);
            let mut text_lines = text.trim().split('\n').map(str::trim);
            if n == 0 {
                lines.push(
                    format!("{marker}{}", text_lines.next().unwrap_or(""))
                        .trim_end()
                        .to_string(),
                );
            } else {
                lines.push(String::new());
                lines.push(
                    format!("{indent}{}", text_lines.next().unwrap_or(""))
                        .trim_end()
                        .to_string(),
                );
            }
            for line in text_lines {
                lines.push(format!("{indent}{line}").trim_end().to_string());
            }
        }
        for nested in &item.children {
            for line in render_block(nested).lines() {
                lines.push(format!("{indent}{line}"));
            }
        }
    }
    lines.join("\n")
}

/// Splits inline content at double line breaks.
fn paragraph_groups(content: &[Inline]) -> Vec<Vec<Inline>> {
    let mut groups = vec![Vec::new()];
    let mut i = 0;
    while i < content.len() {
        if content[i] == Inline::LineBreak && content.get(i + 1) == Some(&Inline::LineBreak) {
            groups.push(Vec::new());
            i += 2;
        } else {
            groups.last_mut().unwrap().push(content[i].clone());
            i += 1;
        }
    }
    groups.retain(|g| !g.is_empty());
    if groups.is_empty() {
        groups.push(Vec::new());
    }
    groups
}

fn image(src: &str, alt: &str, title: Option<&str>) -> String {
    let alt = alt.replace('\\', "\\\\").replace(']', "\\]");
    match title.filter(|t| !t.is_empty()) {
        Some(title) => format!("![{alt}]({} \"{}\")", url(src), title.replace('"', "'")),
        None => format!("![{alt}]({})", url(src)),
    }
}

/// Links can't contain spaces or unbalanced parentheses.
fn url(url: &str) -> String {
    url.trim()
        .replace(' ', "%20")
        .replace('(', "%28")
        .replace(')', "%29")
}

fn inlines(content: &[Inline]) -> String {
    let mut content = merge_text(content.to_vec());
    // Breaks at either end mean nothing and would render as a stray `\`.
    while content.first() == Some(&Inline::LineBreak) {
        content.remove(0);
    }
    while content.last() == Some(&Inline::LineBreak) {
        content.pop();
    }
    let content = content;
    let mut out = String::new();
    for inline in &content {
        match inline {
            Inline::Text { text, format } => out.push_str(&styled(text, *format)),
            Inline::Link {
                url: href,
                title,
                children,
            } => {
                let text = inlines(children);
                match title.as_deref().filter(|t| !t.is_empty()) {
                    Some(title) => out.push_str(&format!(
                        "[{text}]({} \"{}\")",
                        url(href),
                        title.replace('"', "'")
                    )),
                    None => out.push_str(&format!("[{text}]({})", url(href))),
                }
            }
            Inline::Image { src, alt, title } => out.push_str(&image(src, alt, title.as_deref())),
            Inline::LineBreak => out.push_str("\\\n"),
        }
    }
    out
}

/// Wraps text in the markers for its format, keeping surrounding whitespace
/// outside them (`** bold **` isn't emphasis).
fn styled(text: &str, fmt: u32) -> String {
    let core = text.trim();
    if core.is_empty() || fmt == 0 {
        return escape(text);
    }
    let start = &text[..text.len() - text.trim_start().len()];
    let end = &text[text.trim_end().len()..];

    let mut body = if fmt & format::CODE != 0 && !core.contains(['`', '\n']) {
        format!("`{core}`")
    } else {
        escape(core)
    };
    match (fmt & format::BOLD != 0, fmt & format::ITALIC != 0) {
        (true, true) => body = format!("***{body}***"),
        (true, false) => body = format!("**{body}**"),
        (false, true) => body = format!("*{body}*"),
        _ => {}
    }
    if fmt & format::STRIKETHROUGH != 0 {
        body = format!("~~{body}~~");
    }
    format!("{}{body}{}", escape(start), escape(end))
}

/// Escapes characters the parser would read as markup.
fn escape(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    for (i, &c) in chars.iter().enumerate() {
        let prev = i.checked_sub(1).map(|j| chars[j]);
        let next = chars.get(i + 1).copied();
        let escape = match c {
            '\\' | '*' | '`' | '[' => true,
            // Underscores inside words are literal.
            '_' => {
                !(prev.is_some_and(|p| p.is_ascii_alphanumeric())
                    && next.is_some_and(|n| n.is_ascii_alphanumeric()))
            }
            '~' => prev == Some('~') || next == Some('~'),
            // Only tag-like text can start an HTML block.
            '<' => next.is_some_and(|n| n.is_ascii_alphabetic() || n == '/' || n == '!'),
            _ => false,
        };
        if escape {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Escapes text at the start of a line that would otherwise open a block.
fn escape_line_start(line: &str) -> String {
    let trimmed = line.trim_start();
    let indent = &line[..line.len() - trimmed.len()];
    let hashes = trimmed.chars().take_while(|&c| c == '#').count();
    let digits = trimmed.chars().take_while(char::is_ascii_digit).count();
    let after_digits = &trimmed[digits..];

    let block_start = (1..=6).contains(&hashes)
        && trimmed[hashes..]
            .chars()
            .next()
            .map_or(true, char::is_whitespace)
        || trimmed.starts_with('>')
        || trimmed.starts_with("- ")
        || trimmed.starts_with("+ ")
        || trimmed == "-"
        || trimmed == "+"
        || trimmed.starts_with("```")
        || (trimmed.len() >= 3
            && ["-", "_", "*"]
                .iter()
                .any(|m| trimmed.chars().all(|c| m.starts_with(c))));

    if digits > 0 && (after_digits.starts_with(". ") || after_digits.starts_with(") ")) {
        // `1. ` -> `1\. `
        return format!("{indent}{}\\{}", &trimmed[..digits], &trimmed[digits..]);
    }
    if block_start {
        return format!("{indent}\\{trimmed}");
    }
    line.to_string()
}
