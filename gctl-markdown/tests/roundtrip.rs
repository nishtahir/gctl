//! Rendering a parsed document back to markdown must parse to the same
//! document, so pulled files mean what Ghost has.

use gctl_markdown::{parse, to_markdown};

/// Each input, its rendered markdown, and whether it parses back identically.
fn roundtrip(inputs: &[&str]) -> Vec<(String, String, bool)> {
    inputs
        .iter()
        .map(|input| {
            let first = parse(input).unwrap().body;
            let rendered = to_markdown(&first);
            let second = parse(&rendered).unwrap().body;
            (input.to_string(), rendered, first == second)
        })
        .collect()
}

#[test]
fn inline_formatting() {
    insta::assert_debug_snapshot!(roundtrip(&[
        "**bold** *italic* ~~strike~~ `code` ***both***",
        "**bold *and italic* bold**",
        "~~**x**~~",
        "snake_case_name and __init__ and a_b",
        "2 * 3 * 4 and a ` tick and ~tilde and a [bracket",
        "**open and ~~ and \\*literal\\* and \\_x\\_",
        "[link](https://example.com \"Title\") and [**bold** link](/x)",
        "one  \ntwo\\\nthree",
        "x < y and <b>tag</b> and a <div> in text",
        "naïve **café** 日本語 🎉",
    ]));
}

#[test]
fn blocks() {
    insta::assert_debug_snapshot!(roundtrip(&[
        "# One\n\n## Two *em*\n\n###### Six",
        "> quote **bold**\n>\n> second paragraph",
        "- a\n- b\n  - nested\n  - nested two\n- c",
        "1. one\n2. two\n   - mixed\n3. three",
        "5. five\n6. six",
        "```rust\nfn main() {}\n```\n\n```\nplain\n```",
        "text\n\n---\n\n***",
        "<div>\nhtml\n</div>\n\n<!--members-only-->",
        "![alt](/a.png \"Title\")\n\n[![linked](/b.png)](https://example.com)",
        "text before ![x](/x.png) text after",
    ]));
}

#[test]
fn text_that_looks_like_blocks() {
    insta::assert_debug_snapshot!(roundtrip(&[
        "\\# not a heading",
        "\\> not a quote",
        "\\- not a list",
        "1\\. not a list",
        "\\--- not a rule",
        "\\``` not a fence",
        "#hashtag and 1.5 and -no and +no",
    ]));
}

#[test]
fn documents_are_stable() {
    // Rendering twice gives the same text (a fixed point).
    let input = "# T\n\nSome **bold** text.\n\n- a\n  - b\n\n> q\n\n```rs\nx\n```\n";
    let once = to_markdown(&parse(input).unwrap().body);
    let twice = to_markdown(&parse(&once).unwrap().body);
    insta::assert_snapshot!(format!("{once}\n=== same: {}", once == twice));
}

#[test]
fn tables() {
    insta::assert_debug_snapshot!(roundtrip(&[
        "| a | b |\n| --- | --- |\n| 1 | 2 |",
        "a | b\n:-- | --:\nx | y",
        "| L | C | R |\n|:--|:-:|--:|\n| 1 | 2 | 3 |",
        "| `a\\|b` | x \\| y | **b** *i* [l](/u) |\n|---|---|---|\n| | z | |",
        "| only |\n| - |",
        "| a | b |\n| - | - |\n| short |",
    ]));
}
