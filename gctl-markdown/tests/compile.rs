//! Parsing behaviour that isn't tied to one Lexical node: front matter, the
//! block structure of a whole post, and how malformed input is handled.

use gctl_markdown::{parse, Document};

const POST: &str = r#"---
title: "Hello, World"
slug: hello-world
date: 2024-01-31
draft: false
tags: [rust, wasm]
categories: Programming
description: A short summary
---

# Heading *one*

Some **bold**, *italic*, ~~gone~~, `code`, snake_case_word and a [link](https://example.com "Title").
A second line in the same paragraph.  
A hard break above.

## Lists

- one
- two
  - nested a
  - nested b
- three

3. third
4. fourth

> quoted **text**
> more

```rust
fn main() {}
```

---

![An image](https://example.com/a.png)

Text before ![inline](https://example.com/b.png) text after

<div class="note">
raw html
</div>
"#;

/// Parse results for several inputs, as `(input, result)` pairs.
fn parsed(inputs: &[&str]) -> Vec<(String, Result<Document, String>)> {
    inputs
        .iter()
        .map(|input| (input.to_string(), parse(input).map_err(|e| e.to_string())))
        .collect()
}

#[test]
fn front_matter() {
    let fm = parse(POST).unwrap().front_matter;
    insta::assert_debug_snapshot!((&fm, fm.all_tags(), fm.published_at()));
}

#[test]
fn front_matter_aliases_and_scalars() {
    insta::assert_debug_snapshot!(parsed(&[
        "---\ntags: single\n---\n",
        "---\npublishDate: 2024-01-31T10:30:00Z\nsummary: s\nimage: /i.png\ncanonical: https://x\n---\n",
        "---\ncategories: [a, b]\ntags: [b, c]\n---\n",
        "---\nunknown_key: ignored\ntitle: t\n---\n",
        "---\n---\n",
    ]));
}

#[test]
fn front_matter_code_injection() {
    insta::assert_debug_snapshot!(parsed(&[
        "---\ncodeinjection_head: |\n  <script>\n    run();\n  </script>\ncodeinjection_foot: |-\n  <div id=\"x\"></div>\n---\nBody\n",
        "---\ncodeinjection_head: '<meta name=\"robots\" content=\"noindex\">'\n---\n",
        // An empty value is meaningful: it clears the injection on push.
        "---\ncodeinjection_head: \"\"\ncodeinjection_foot:\n---\n",
        "---\ncodeinjection_head: [not, a, string]\n---\n",
    ]));
}

#[test]
fn front_matter_dates() {
    let dates = [
        "2024-01-31",
        "2024-01-31T10:30:00Z",
        "2024-01-31T10:30:00+02:00",
        "2024-01-31 10:30:00",
        "2024-01-31 10:30",
        "not-a-date",
    ];
    let results: Vec<_> = dates
        .iter()
        .map(|date| {
            let doc = parse(&format!("---\ndate: \"{date}\"\n---\n")).unwrap();
            (
                *date,
                doc.front_matter
                    .published_at()
                    .map(|d| d.map(|d| d.to_rfc3339()))
                    .map_err(|e| e.to_string()),
            )
        })
        .collect();
    insta::assert_debug_snapshot!(results);
}

#[test]
fn front_matter_errors() {
    insta::assert_debug_snapshot!(["---\ntitle: [unclosed\n---\n", "---\ndraft: maybe\n---\n"]
        .iter()
        .map(|input| parse(input).map(|_| ()).map_err(|e| e.to_string()))
        .collect::<Vec<_>>());
}

#[test]
fn document_blocks() {
    insta::assert_debug_snapshot!(parse(POST).unwrap().body);
}

#[test]
fn document_lexical() {
    insta::assert_json_snapshot!(parse(POST).unwrap().to_lexical());
}

#[test]
fn without_front_matter() {
    insta::assert_debug_snapshot!(parsed(&["Just text\n"]));
}

#[test]
fn empty_and_crlf_input() {
    insta::assert_debug_snapshot!(parsed(&[
        "",
        "---\n---\n",
        "---\r\ntitle: x\r\n---\r\n\r\nhi\r\n",
        "\u{feff}---\ntitle: bom\n---\n",
    ]));
}

#[test]
fn malformed_syntax_degrades_to_plain_text() {
    insta::assert_debug_snapshot!(parsed(&[
        "2 * 3 * 4",
        "unclosed **bold",
        "unclosed *emph",
        "a lone ` backtick",
        "[not a link] (nope)",
        "[unclosed](http://x",
        "![unclosed",
        "~~ spaced ~~",
        "trailing backslash\\",
        "snake_case and __init__ and a_b_c",
        "100% sure! really?",
        "#hashtag is not a heading",
        "-no space is not a list",
        "1.5 is not a list",
        "<b>inline html</b> stays a paragraph",
    ]));
}

#[test]
fn unterminated_blocks_do_not_hang_or_panic() {
    insta::assert_debug_snapshot!(parsed(&[
        "```rust\nfn main() {",
        "```",
        "> ",
        ">",
        "- ",
        "-\n",
        "1. ",
        "<div>",
        "---",
        "---\ntitle: x",
        "#",
        "# ",
        "\t\n  \n",
        "- a\n\n\n- b\n\n  \n- c",
        "  - indented first\n  - second",
        "text\n- list interrupts\n# heading interrupts\n> quote interrupts",
    ]));
}
