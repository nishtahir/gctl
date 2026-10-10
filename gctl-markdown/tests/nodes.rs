//! One snapshot per Lexical node type (with its variants). Each snapshot lists
//! the markdown input next to the exact JSON nodes Ghost receives, so any
//! change to the emitted shape shows up as a snapshot diff.

use gctl_markdown::parse;
use serde_json::{json, Value};

/// The root's children for a markdown source.
fn nodes(markdown: &str) -> Value {
    parse(markdown).unwrap().to_lexical()["root"]["children"].clone()
}

fn cases(inputs: &[&str]) -> Vec<Value> {
    inputs
        .iter()
        .map(|markdown| json!({ "markdown": markdown, "nodes": nodes(markdown) }))
        .collect()
}

/// Declares a test that snapshots the nodes produced for each input.
macro_rules! snapshot {
    ($name:ident, $($markdown:expr),+ $(,)?) => {
        #[test]
        fn $name() {
            insta::assert_json_snapshot!(cases(&[$($markdown),+]));
        }
    };
}

// ---- root ----

#[test]
fn root_wraps_children() {
    insta::assert_json_snapshot!(parse("Hi").unwrap().to_lexical());
}

snapshot!(
    root_empty_documents,
    "",
    "\n\n  \n",
    "---\ntitle: only front matter\n---\n"
);

#[test]
fn root_lexical_string_is_serialized_json() {
    insta::assert_snapshot!(parse("Hi **there**").unwrap().to_lexical_string());
}

// ---- paragraph ----

snapshot!(
    paragraph,
    "Hello world",
    "one\n\n\ntwo",
    "one\ntwo",
    "one\n   two"
);

// ---- extended-text ----

snapshot!(text_bold, "**bold**", "__bold__");
snapshot!(text_italic, "*it*", "_it_");
snapshot!(text_strikethrough, "~~gone~~");
snapshot!(text_code, "`x < y`", "`*not bold*`");
snapshot!(
    text_formats_combine,
    "***both***",
    "**bold *and italic* bold**",
    "~~**x**~~"
);
snapshot!(text_runs_are_merged, "Hello, world! (really)");
snapshot!(text_escapes, r"\*literal\* \_x\_ \# \[a\]");
snapshot!(
    text_underscores_inside_words,
    "snake_case_name and __init__"
);
snapshot!(text_unbalanced_markers, "2 * 3 * 4", "**open", "a ` b");
snapshot!(text_non_ascii, "naïve **café** 日本語 🎉");

// ---- linebreak ----

snapshot!(linebreak, "one  \ntwo", "one\\\ntwo");

// ---- link ----

snapshot!(
    link,
    "[Rust](https://rust-lang.org)",
    r#"[Rust](https://rust-lang.org "The language")"#,
    "[**bold** and `code`](/x)",
    "see [a](/a) and *[b](/b)*",
    "[a] (b)",
    "[a](http://x"
);

// ---- extended-heading ----

snapshot!(
    heading_levels,
    "# Title",
    "## Title",
    "### Title",
    "#### Title",
    "##### Title",
    "###### Title"
);
snapshot!(heading_inline_formatting, "## A *very* `big` [one](/x)");
snapshot!(heading_closing_hashes, "## Title ##", "# C#");
snapshot!(heading_not_a_heading, "#hashtag", "####### seven");
snapshot!(heading_interrupts_paragraph, "text\n# Title\nmore");

// ---- extended-quote ----

snapshot!(
    quote,
    "> wise words",
    "> one\n> two",
    "> one\n>\n> two",
    "> **bold** [link](/x)",
    ">tight"
);

// ---- list / listitem ----

snapshot!(list_bullet_markers, "- a\n- b", "* a\n* b", "+ a\n+ b");
snapshot!(list_ordered, "1. a\n2. b", "1) a\n2) b");
snapshot!(list_ordered_start_number, "5. five\n6. six");
snapshot!(list_item_inline_formatting, "- **a** [b](/b)");
snapshot!(
    list_nested,
    "- a\n  - b\n  - c\n- d",
    "1. a\n   - b",
    "- a\n  - b\n    - c"
);
snapshot!(list_continuation_line, "- one\n  still one\n- two");
snapshot!(
    list_item_with_several_paragraphs,
    "- first\n\n  second\n- next",
    "1. one\n\n   more\n\n   even more\n2. two",
    "- tight\n  lazy continuation\n- next"
);
snapshot!(list_blank_lines_between_items, "- a\n\n- b");
snapshot!(list_marker_type_change, "- a\n\n1. b");
snapshot!(list_interrupts_paragraph, "intro\n- a");
snapshot!(list_marker_needs_space, "-no", "1.5 apples");

// ---- codeblock ----

snapshot!(
    codeblock,
    "```rust\nfn main() {}\n```",
    "```\nplain\n```",
    "```rust ignore\nx\n```",
    "```\n```"
);
snapshot!(
    codeblock_is_verbatim,
    "```md\n# not a heading\n\n  - not a list\n*not italic*\n```"
);
snapshot!(
    codeblock_longer_fences,
    "````md\n```\ninner\n```\n````",
    "``````\n`````\nstill code\n``````"
);
snapshot!(codeblock_unterminated, "```sh\necho hi");
snapshot!(
    codeblock_interrupts_paragraph,
    "text\n```\ncode\n```\nafter"
);

// ---- horizontalrule ----

snapshot!(
    horizontalrule,
    "---",
    "***",
    "___",
    "-----",
    "*****",
    "---  ",
    "a\n\n---\n\nb"
);
snapshot!(horizontalrule_not_bold_or_list, "**bold**", "* a");

// ---- html ----

snapshot!(
    html_block,
    "<div class=\"note\">\n  <p>hi</p>\n</div>",
    "<table>\n<tr></tr>\n</table>\n\nafter",
    "<!-- note -->"
);
snapshot!(
    html_inline_tags_stay_in_paragraph,
    "<b>bold</b> text",
    "<a href=\"/x\">link</a> text",
    "<kbd>x</kbd> y"
);
snapshot!(html_is_not_markdown, "<div>\n*not italic*\n</div>");

snapshot!(
    html_raw_blocks_run_to_their_closing_tag,
    "<pre>\nline one\n\nline three\n</pre>\n\nafter",
    "<script>\nlet a;\n\nlet b;\n</script>",
    "<!-- a comment\n\nspanning blank lines -->\n\nafter",
    // Unterminated: ends at the first blank line like any other HTML.
    "<pre>\nnever closed\n\nafter"
);
snapshot!(
    html_members_only_comment_is_a_paywall,
    "free\n\n<!--members-only-->\n\npaid",
    "<!-- members-only -->"
);

// ---- image ----

snapshot!(
    image,
    "![A cat](https://example.com/cat.png)",
    r#"![A cat](/cat.png "Fluffy")"#,
    "![](/x.png)"
);
snapshot!(image_linked, "[![A cat](/cat.png)](https://example.com)");
snapshot!(
    image_splits_paragraph,
    "before ![x](/x.png) after",
    "![a](/a.png)\n![b](/b.png)",
    "![a](/a.png)  \n"
);

// ---- whole documents ----

snapshot!(
    document_block_order,
    "# T\n\npara\n\n- a\n\n> q\n\n```\nc\n```\n\n---\n\n![i](/i.png)\n\n<hr>\n"
);
snapshot!(
    document_crlf_line_endings,
    "# T\r\n\r\n- a\r\n- b\r\n\r\n```rs\r\nx\r\n```\r\n"
);
snapshot!(
    document_front_matter_is_not_body,
    "---\ntitle: T\ntags: [a]\n---\n\nBody"
);

// ---- indented blocks ----

snapshot!(
    html_blocks_may_be_indented_up_to_three_spaces,
    "  <div class=\"a\">\n    <p>one</p>\n  </div>",
    "   <section>\n   text\n   </section>",
    // Four spaces is indented text, not a block start.
    "    <div>not html</div>",
    // Each chunk of a demo split by blank lines is its own HTML card.
    "<div class=\"demo\">\n  <h1></h1>\n\n  <div class=\"controls\">\n    <button>Step</button>\n  </div>\n\n  <section>\n    <h2>x</h2>\n  </section>\n</div>"
);
snapshot!(
    other_blocks_may_be_indented_up_to_three_spaces,
    "  # Heading",
    "   > quote",
    "  ---",
    "  ```rust\n  fn main() {}\n  ```",
    "  {{< include \"a.md\" >}}",
    "    # four spaces is text"
);
