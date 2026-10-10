use gctl_markdown::{parse, set_front_matter_id};

const ID: &str = "6891829ae31775000106141a";

/// Each input with the result of setting the id, plus the id read back.
fn cases(inputs: &[&str]) -> Vec<(String, String, Option<String>)> {
    inputs
        .iter()
        .map(|input| {
            let output = set_front_matter_id(input, ID);
            let id = parse(&output).unwrap().front_matter.id;
            (input.to_string(), output, id)
        })
        .collect()
}

#[test]
fn inserts_into_existing_front_matter() {
    insta::assert_debug_snapshot!(cases(&[
        "---\ntitle: Hello\n---\n\nBody\n",
        "---\n# keep me\ntitle: Hello\ntags: [a, b]\n---\nBody\n",
        "---\n---\nBody\n",
    ]));
}

#[test]
fn replaces_an_existing_id() {
    insta::assert_debug_snapshot!(cases(&[
        "---\ntitle: Hello\nid: old\n---\nBody\n",
        "---\nid:\ntitle: Hello\n---\nBody\n",
        "---\nid: \"old\" # comment\ntitle: Hello\n---\nBody\n",
    ]));
}

#[test]
fn only_a_top_level_id_key_counts() {
    insta::assert_debug_snapshot!(cases(&[
        "---\nidentity: x\nidx: 1\nauthor:\n  id: nested\n---\nBody\n",
    ]));
}

#[test]
fn adds_front_matter_when_there_is_none() {
    insta::assert_debug_snapshot!(cases(&[
        "Just a body\n",
        "",
        "---\nlooks like front matter but never closes\n",
        "Text\n\n---\n\nmore text\n",
    ]));
}

#[test]
fn preserves_line_endings_and_bom() {
    insta::assert_debug_snapshot!(cases(&[
        "---\r\ntitle: Hello\r\n---\r\nBody\r\n",
        "\u{feff}---\ntitle: Hello\n---\nBody\n",
        "Body\r\n",
    ]));
}

#[test]
fn body_is_never_modified() {
    let body = "# Heading\n\n---\nid: not front matter\n---\n\n```\n---\n```\n";
    let source = format!("---\ntitle: T\n---\n{body}");
    let output = set_front_matter_id(&source, ID);
    assert!(output.ends_with(body), "{output}");
}

#[test]
fn setting_twice_is_stable() {
    let once = set_front_matter_id("---\ntitle: T\n---\nBody\n", ID);
    assert_eq!(set_front_matter_id(&once, ID), once);
}
