//! `codeinjection_head` / `codeinjection_foot`: inline text or `{file: path}`.

use std::collections::BTreeMap;

use gctl_markdown::{parse, to_markdown, Document};

/// Parses each front matter block, showing the injection fields or the error.
fn parsed(blocks: &[&str]) -> Vec<(String, String)> {
    blocks
        .iter()
        .map(|fm| {
            let result = match parse(&format!("---\n{fm}\n---\n")) {
                Ok(doc) => format!(
                    "head={:?} foot={:?}",
                    doc.front_matter.codeinjection_head, doc.front_matter.codeinjection_foot
                ),
                Err(e) => format!("ERROR {e}"),
            };
            (fm.to_string(), result)
        })
        .collect()
}

#[test]
fn forms() {
    insta::assert_debug_snapshot!(parsed(&[
        "codeinjection_head: <b>inline</b>",
        "codeinjection_head: |\n  <script>\n    a();\n  </script>",
        "codeinjection_head: {file: _snippets/analytics.html}",
        "codeinjection_head:\n  file: _snippets/analytics.html\ncodeinjection_foot:\n  file: \"_snippets/foot.html\"",
        "codeinjection_head: \"\"",
        "codeinjection_head:",
        // Mistakes get messages that say what is expected.
        "codeinjection_head: {path: x.html}",
        "codeinjection_head: {file: x.html, extra: 1}",
        "codeinjection_head: {}",
        "codeinjection_head: {file: [a, b]}",
        "codeinjection_head: 5",
        "codeinjection_head: [a, b]",
        "codeinjection_head: true",
    ]));
}

/// Files by identity, with `/`-joined relative paths resolved like the CLI.
struct Files(BTreeMap<&'static str, &'static str>);

fn join(from: &str, path: &str) -> String {
    let mut parts: Vec<&str> = from
        .rsplit_once('/')
        .map_or(vec![], |(d, _)| d.split('/').collect());
    for part in path.split('/') {
        match part {
            ".." => {
                parts.pop();
            }
            "." | "" => {}
            other => parts.push(other),
        }
    }
    parts.join("/")
}

impl Files {
    fn resolve(&self, source: &str) -> Result<Document, String> {
        let mut doc = parse(self.0[source]).map_err(|e| e.to_string())?;
        let mut loader = |from: &str, path: &str| {
            let id = join(from, path);
            self.0
                .get(id.as_str())
                .map(|c| (id.clone(), c.to_string()))
                .ok_or_else(|| format!("no such file {id}"))
        };
        doc.resolve_includes(source, &mut loader)
            .map_err(|e| e.to_string())?;
        Ok(doc)
    }
}

fn files(entries: &[(&'static str, &'static str)]) -> Files {
    Files(entries.iter().copied().collect())
}

/// What the post would send for head and foot, or the error.
fn injections(files: &Files, source: &str) -> String {
    match files.resolve(source) {
        Ok(doc) => format!(
            "head={:?}\nfoot={:?}",
            doc.front_matter.head_injection().unwrap(),
            doc.front_matter.foot_injection().unwrap()
        ),
        Err(e) => format!("ERROR {e}"),
    }
}

#[test]
fn files_are_read_verbatim() {
    let fs = files(&[
        (
            "content/post.md",
            "---\ncodeinjection_head: {file: _snippets/head.html}\ncodeinjection_foot: {file: _snippets/foot.html}\n---\n\nBody\n",
        ),
        // Whitespace, blank lines, tabs and a trailing newline all survive.
        ("content/_snippets/head.html", "<script>\n\tlet a = 1;\n\n  go();\n</script>\n"),
        ("content/_snippets/foot.html", "<div></div>"),
    ]);
    insta::assert_snapshot!(injections(&fs, "content/post.md"));
}

#[test]
fn file_contents_are_not_processed_as_markdown_or_includes() {
    let fs = files(&[
        (
            "content/post.md",
            "---\ncodeinjection_head: {file: head.html}\n---\n",
        ),
        (
            "content/head.html",
            "# not a heading\n{{< include \"other.md\" >}}\n*not italic* ---\n---\n",
        ),
    ]);
    insta::assert_snapshot!(injections(&fs, "content/post.md"));
}

#[test]
fn inline_and_file_forms_mix() {
    let fs = files(&[
        (
            "content/post.md",
            "---\ncodeinjection_head: '<meta name=\"x\">'\ncodeinjection_foot: {file: foot.html}\n---\n",
        ),
        ("content/foot.html", "<script src=\"/foot.js\"></script>\n"),
    ]);
    insta::assert_snapshot!(injections(&fs, "content/post.md"));
}

#[test]
fn paths_are_relative_to_the_file_and_empty_files_clear() {
    let fs = files(&[
        (
            "content/posts/post.md",
            "---\ncodeinjection_head: {file: ../_snippets/head.html}\ncodeinjection_foot: {file: ../_snippets/empty.html}\n---\n",
        ),
        ("content/_snippets/head.html", "<style>a{}</style>"),
        ("content/_snippets/empty.html", ""),
    ]);
    insta::assert_snapshot!(injections(&fs, "content/posts/post.md"));
}

#[test]
fn injection_files_and_body_includes_resolve_together() {
    let fs = files(&[
        (
            "content/post.md",
            "---\ncodeinjection_head: {file: head.html}\n---\n\nBefore\n\n{{< include \"part.md\" >}}\n",
        ),
        ("content/head.html", "<link rel=\"x\">"),
        ("content/part.md", "From the part\n"),
    ]);
    let doc = fs.resolve("content/post.md").unwrap();
    insta::assert_snapshot!(format!(
        "{}\n=====\n{}",
        to_markdown(&doc.body),
        injections(&fs, "content/post.md")
    ));
}

#[test]
fn problems_name_the_key_and_the_file() {
    let fs = files(&[
        (
            "content/missing.md",
            "---\ncodeinjection_head: {file: nope.html}\n---\n",
        ),
        (
            "content/foot.md",
            "---\ncodeinjection_foot: {file: nope.html}\n---\n",
        ),
    ]);
    insta::assert_debug_snapshot!([
        injections(&fs, "content/missing.md"),
        injections(&fs, "content/foot.md"),
    ]);
}

#[test]
fn unresolved_file_references_are_an_error_not_silence() {
    // Reading the text before includes are resolved must not quietly give
    // `None`, or a push would leave the old injection in place.
    let doc =
        parse("---\ncodeinjection_head: {file: head.html}\ncodeinjection_foot: <b>ok</b>\n---\n")
            .unwrap();
    insta::assert_debug_snapshot!((
        doc.front_matter.head_injection().map_err(|e| e.to_string()),
        doc.front_matter.foot_injection().map_err(|e| e.to_string()),
    ));
}
