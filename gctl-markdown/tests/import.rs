//! Lexical -> markdown, one snapshot per node type. Inputs use the node shapes
//! Ghost's editor writes.

use gctl_markdown::{from_lexical, to_markdown};
use serde_json::{json, Value};

fn text_fmt(text: &str, format: u32) -> Value {
    json!({ "detail": 0, "format": format, "mode": "normal", "style": "",
            "text": text, "type": "extended-text", "version": 1 })
}
fn text(t: &str) -> Value {
    text_fmt(t, 0)
}
fn linebreak() -> Value {
    json!({ "type": "linebreak", "version": 1 })
}
fn element(kind: &str, children: Vec<Value>) -> Value {
    json!({ "children": children, "direction": "ltr", "format": "", "indent": 0,
            "type": kind, "version": 1 })
}
fn with(mut node: Value, extra: Value) -> Value {
    node.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    node
}
fn paragraph(children: Vec<Value>) -> Value {
    element("paragraph", children)
}
fn list(kind: &str, start: u32, items: Vec<Value>) -> Value {
    with(
        element("list", items),
        json!({ "listType": kind, "start": start, "tag": if kind == "number" { "ol" } else { "ul" } }),
    )
}
fn item(children: Vec<Value>) -> Value {
    with(element("listitem", children), json!({ "value": 1 }))
}

/// Imports a document made of `nodes`; returns the markdown and lossy kinds.
fn import(nodes: Vec<Value>) -> (String, Vec<String>) {
    let lexical = json!({ "root": element("root", nodes) }).to_string();
    let imported = from_lexical(&lexical).unwrap();
    (to_markdown(&imported.blocks), imported.lossy)
}

/// Several documents, each paired with what it imports to.
macro_rules! cases {
    ($name:ident, $($nodes:expr),+ $(,)?) => {
        #[test]
        fn $name() {
            let results: Vec<(String, Vec<String>)> = vec![$(import($nodes)),+];
            insta::assert_debug_snapshot!(results);
        }
    };
}

cases!(
    paragraphs,
    vec![paragraph(vec![text("Hello world")])],
    vec![paragraph(vec![text("one")]), paragraph(vec![text("two")])],
    // Empty paragraphs are spacing, not content.
    vec![
        paragraph(vec![]),
        paragraph(vec![text("  ")]),
        paragraph(vec![text("kept")])
    ],
    // Newlines inside a text node are line breaks.
    vec![paragraph(vec![text("a\nb")])],
    vec![paragraph(vec![text("a"), linebreak(), text("b")])],
    // Plain `text` nodes work like `extended-text`.
    vec![paragraph(vec![
        json!({ "type": "text", "format": 1, "text": "bold", "detail": 0,
                                "mode": "normal", "style": "", "version": 1 })
    ])]
);

cases!(
    text_formats,
    vec![paragraph(vec![
        text_fmt("bold", 1),
        text(" "),
        text_fmt("italic", 2),
        text(" "),
        text_fmt("strike", 4),
        text(" "),
        text_fmt("code", 16),
        text(" "),
        text_fmt("both", 3),
    ])],
    // Underline/sub/superscript have no markdown form and are dropped.
    vec![paragraph(vec![
        text_fmt("underlined", 8),
        text(" "),
        text_fmt("sup", 64)
    ])],
    // Whitespace stays outside the markers.
    vec![paragraph(vec![
        text("a"),
        text_fmt(" padded ", 1),
        text("b")
    ])],
    // Runs split by the editor are merged.
    vec![paragraph(vec![
        text_fmt("one", 1),
        text_fmt(" ", 1),
        text_fmt("two", 1)
    ])]
);

cases!(
    text_escaping,
    vec![paragraph(vec![text(
        "2 * 3 * 4 and snake_case and _lead and a [bracket and `tick`"
    )])],
    vec![paragraph(vec![text("# not a heading")])],
    vec![paragraph(vec![text("> not a quote")])],
    vec![paragraph(vec![text("- not a list")])],
    vec![paragraph(vec![text("1. not a list")])],
    vec![paragraph(vec![text("<div> and <b>tags</b> but 1 < 2")])],
    vec![paragraph(vec![text("back\\slash")])]
);

cases!(
    headings,
    vec![element("extended-heading", vec![text("One")]).tap("tag", "h1")],
    vec![element("extended-heading", vec![text("Two *em*")]).tap("tag", "h2")],
    // Older posts use the plain node types.
    vec![element("heading", vec![text("Plain")]).tap("tag", "h3")],
    vec![element(
        "extended-heading",
        vec![text("Six"), linebreak(), text("lines")]
    )
    .tap("tag", "h6")]
);

cases!(
    quotes,
    vec![element("extended-quote", vec![text("wise words")])],
    vec![element("quote", vec![text("old style")])],
    vec![element(
        "extended-quote",
        vec![text("first"), linebreak(), linebreak(), text("second")]
    )],
    vec![element(
        "extended-quote",
        vec![text("one"), linebreak(), text("two")]
    )],
    vec![element("aside", vec![text("pulled")])]
);

cases!(
    links,
    vec![paragraph(vec![
        text("see "),
        with(
            element("link", vec![text_fmt("this", 1)]),
            json!({ "url": "https://example.com", "title": null, "rel": null, "target": null })
        ),
        text(" and "),
        with(
            element("link", vec![text("titled")]),
            json!({ "url": "/x", "title": "A title" })
        ),
    ])],
    // Ghost sometimes stores URLs with stray spaces; markdown can't.
    vec![paragraph(vec![with(
        element("link", vec![text("padded")]),
        json!({ "url": " https://rustup.rs/ " })
    )])],
    vec![paragraph(vec![with(
        element("link", vec![text("parens")]),
        json!({ "url": "https://en.wikipedia.org/wiki/Rust_(programming_language)" })
    )])]
);

cases!(
    lists,
    vec![list(
        "bullet",
        1,
        vec![item(vec![text("a")]), item(vec![text("b")])]
    )],
    vec![list(
        "number",
        1,
        vec![item(vec![text("a")]), item(vec![text("b")])]
    )],
    vec![list(
        "number",
        5,
        vec![item(vec![text("five")]), item(vec![text("six")])]
    )],
    // Nested lists live in a wrapper item holding only the list.
    vec![list(
        "bullet",
        1,
        vec![
            item(vec![text("a")]),
            item(vec![list(
                "bullet",
                1,
                vec![item(vec![text("b")]), item(vec![text("c")])]
            )]),
            item(vec![text("d")]),
        ]
    )],
    vec![list(
        "number",
        1,
        vec![
            item(vec![text("a")]),
            item(vec![list("bullet", 1, vec![item(vec![text("mixed")])])]),
        ]
    )],
    // Paragraphs inside an item are separated by two line breaks.
    vec![list(
        "bullet",
        1,
        vec![
            item(vec![
                text("first"),
                linebreak(),
                linebreak(),
                text("second"),
                linebreak(),
                linebreak()
            ]),
            item(vec![text("next")]),
        ]
    )],
    // Formatting and links in items.
    vec![list(
        "bullet",
        1,
        vec![item(vec![text_fmt("bold", 1), text(" tail")])]
    )]
);

cases!(
    code_blocks,
    vec![json!({ "type": "codeblock", "code": "fn main() {}", "language": "rust" })],
    vec![
        json!({ "type": "codeblock", "version": 1, "code": "plain", "language": "", "caption": "" })
    ],
    vec![json!({ "type": "codeblock", "code": "no language key" })],
    // Code containing backticks gets a longer fence.
    vec![json!({ "type": "codeblock", "code": "a\n```\nb", "language": "md" })]
);

cases!(
    rules_html_and_paywall,
    vec![json!({ "type": "horizontalrule", "version": 1 })],
    vec![json!({ "type": "html", "html": "<div class=\"note\">\n  <p>hi</p>\n</div>" })],
    // Blank lines inside <pre> are kept; elsewhere they are dropped.
    vec![json!({ "type": "html", "html": "<pre>\na\n\nb\n</pre>" })],
    vec![json!({ "type": "html", "html": "<div>\na\n\nb\n</div>" })],
    vec![json!({ "type": "paywall", "version": 1 })]
);

cases!(
    images,
    vec![
        json!({ "type": "image", "src": "https://example.com/a.png", "alt": "A cat",
                 "title": "", "caption": "", "href": "" })
    ],
    // The caption (HTML) becomes the markdown title.
    vec![json!({ "type": "image", "src": "/a.png", "alt": "",
                 "caption": "<span style=\"white-space: pre-wrap;\">Fig &amp; <em>one</em></span>" })],
    vec![
        json!({ "type": "image", "src": "/a.png", "alt": "x", "title": "Tooltip", "caption": "" })
    ],
    vec![json!({ "type": "image", "src": "/a.png", "alt": "x", "href": "https://example.com" })],
    // Older posts have sparse image nodes.
    vec![json!({ "type": "image", "src": "/a.png", "width": 516, "height": 294 })]
);

cases!(
    markdown_cards,
    vec![json!({ "type": "markdown", "markdown": "Plain **markdown** with `code`." })],
    vec![json!({ "type": "markdown", "markdown": "\nLeading blank line and a [link](/x).\n" })],
    vec![json!({ "type": "markdown", "markdown": "Has a footnote[^1].\n\n[^1]: The note." })],
    vec![json!({ "type": "markdown", "markdown": "| a | b |\n|---|---|\n| 1 | 2 |" })],
    vec![json!({ "type": "markdown", "markdown": "- [ ] todo\n- [x] done" })],
    vec![json!({ "type": "markdown", "markdown": "Inline^[footnote] style." })]
);

cases!(
    cards_markdown_cannot_express,
    vec![json!({ "type": "callout", "calloutEmoji": "💡",
                 "calloutText": "Try the <a href=\"https://example.com\">demo</a> &amp; <strong>read</strong> on" })],
    vec![json!({ "type": "callout", "calloutEmoji": "", "calloutText": "No emoji" })],
    vec![json!({ "type": "bookmark", "url": "https://github.com/x/y",
                 "metadata": { "title": "GitHub - x/y" } })],
    vec![json!({ "type": "bookmark", "url": "https://example.com", "metadata": {} })],
    vec![json!({ "type": "embed", "url": "https://youtu.be/abc", "html": "<iframe></iframe>" })],
    vec![
        json!({ "type": "file", "src": "/content/files/a.pdf", "fileTitle": "The PDF", "fileName": "a.pdf" })
    ],
    vec![json!({ "type": "file", "src": "/content/files/b.pdf", "fileName": "b.pdf" })]
);

cases!(
    unknown_nodes_are_flagged_not_dropped,
    // Unknown node with text keeps its text.
    vec![element("gallery-ish", vec![text("caption text")])],
    // Unknown node with nothing to keep is flagged but adds no content.
    vec![json!({ "type": "product", "version": 1 })]
);

#[test]
fn several_lossy_kinds_are_listed_once_and_sorted() {
    let (_, lossy) = import(vec![
        json!({ "type": "callout", "calloutText": "a" }),
        json!({ "type": "bookmark", "url": "https://a.example" }),
        json!({ "type": "callout", "calloutText": "b" }),
        json!({ "type": "markdown", "markdown": "x[^1]\n\n[^1]: y" }),
    ]);
    insta::assert_debug_snapshot!(lossy);
}

#[test]
fn whole_post_in_order() {
    let (markdown, lossy) = import(vec![
        element("extended-heading", vec![text("Title")]).tap("tag", "h1"),
        paragraph(vec![
            text("Intro with "),
            text_fmt("emphasis", 2),
            text("."),
        ]),
        list(
            "bullet",
            1,
            vec![item(vec![text("a")]), item(vec![text("b")])],
        ),
        json!({ "type": "codeblock", "code": "x = 1", "language": "py" }),
        json!({ "type": "paywall" }),
        paragraph(vec![text("Members only.")]),
    ]);
    insta::assert_snapshot!(format!("{markdown}\nlossy: {lossy:?}"));
}

#[test]
fn invalid_lexical_is_an_error() {
    let errors: Vec<String> = ["", "not json", "{}", "{\"root\": 1}"]
        .iter()
        .map(|input| match from_lexical(input) {
            Ok(_) => "ok".to_string(),
            Err(e) => e.to_string(),
        })
        .collect();
    insta::assert_debug_snapshot!(errors);
}

trait Tap {
    fn tap(self, key: &str, value: &str) -> Self;
}
impl Tap for Value {
    fn tap(mut self, key: &str, value: &str) -> Self {
        self[key] = json!(value);
        self
    }
}
