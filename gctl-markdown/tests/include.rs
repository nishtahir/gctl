//! `{{< include "..." >}}`: parsing, resolution and the ways it can fail.

use std::collections::BTreeMap;

use gctl_markdown::{parse, to_markdown, Document};

/// An in-memory file system. Paths are resolved relative to the including
/// file's directory, with `..` handled, like the real loader.
struct Files(BTreeMap<&'static str, &'static str>);

fn join(from: &str, path: &str) -> String {
    let mut parts: Vec<&str> = match from.rsplit_once('/') {
        Some((dir, _)) => dir.split('/').collect(),
        None => vec![],
    };
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
    fn resolve(&self, main: &str, source: &str) -> Result<Document, String> {
        let mut doc = parse(main).map_err(|e| e.to_string())?;
        let mut loader = |from: &str, path: &str| {
            let id = join(from, path);
            self.0
                .get(id.as_str())
                .map(|content| (id.clone(), content.to_string()))
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

/// Resolved markdown, or the error message.
fn resolved(files: &Files, main: &str) -> String {
    match files.resolve(main, "content/post.md") {
        Ok(doc) => to_markdown(&doc.body),
        Err(e) => format!("ERROR: {e}"),
    }
}

#[test]
fn directive_syntax() {
    let inputs = [
        "{{< include \"a.md\" >}}",
        "{{<include \"a.md\">}}",
        "{{<   include   'a.md'   >}}",
        "{{< include \"dir/b.rs\" lang=\"rust\" >}}",
        "{{< include \"c.txt\" lang=\"\" >}}",
        "{{< include \"has space.md\" >}}  ",
        // Not directives:
        "text {{< include \"a.md\" >}} inside a paragraph",
        "{{< include a.md >}}",
        "{{< include \"a.md\" ",
        "{{< partial \"a.md\" >}}",
        "`{{< include \"a.md\" >}}`",
    ];
    let parsed: Vec<_> = inputs
        .iter()
        .map(|i| (i.to_string(), parse(i).unwrap().body))
        .collect();
    insta::assert_debug_snapshot!(parsed);
}

#[test]
fn directive_is_literal_in_code() {
    insta::assert_debug_snapshot!(
        parse("```\n{{< include \"a.md\" >}}\n```\n\n    {{< include \"b.md\" >}}\n")
            .unwrap()
            .body
    );
}

#[test]
fn directive_interrupts_a_paragraph() {
    insta::assert_debug_snapshot!(parse("text\n{{< include \"a.md\" >}}\nmore").unwrap().body);
}

#[test]
fn includes_markdown_blocks_at_the_directive() {
    let fs = files(&[(
        "content/footer.md",
        "---\nThanks for reading!\n\n- one\n- two\n",
    )]);
    // A leading `---` in the included file is a rule, not front matter,
    // because it has no closing fence.
    insta::assert_snapshot!(resolved(
        &fs,
        "Intro\n\n{{< include \"footer.md\" >}}\n\nOutro\n"
    ));
}

#[test]
fn included_front_matter_is_ignored() {
    let fs = files(&[(
        "content/snippet.md",
        "---\ntitle: Ignored\ntags: [nope]\n---\n\nSnippet body.\n",
    )]);
    insta::assert_snapshot!(resolved(&fs, "{{< include \"snippet.md\" >}}"));
}

#[test]
fn the_same_file_can_be_included_twice() {
    let fs = files(&[("content/note.md", "> A note\n")]);
    insta::assert_snapshot!(resolved(
        &fs,
        "One\n\n{{< include \"note.md\" >}}\n\nTwo\n\n{{< include \"note.md\" >}}\n"
    ));
}

#[test]
fn paths_are_relative_to_the_including_file() {
    let fs = files(&[
        (
            "content/_parts/outer.md",
            "Outer\n\n{{< include \"inner/leaf.md\" >}}\n\n{{< include \"../shared.md\" >}}\n",
        ),
        ("content/_parts/inner/leaf.md", "Leaf\n"),
        ("content/shared.md", "Shared\n"),
    ]);
    insta::assert_snapshot!(resolved(&fs, "{{< include \"_parts/outer.md\" >}}"));
}

#[test]
fn includes_resolve_recursively_in_order() {
    let fs = files(&[
        (
            "content/a.md",
            "A start\n\n{{< include \"b.md\" >}}\n\nA end\n",
        ),
        (
            "content/b.md",
            "B start\n\n{{< include \"c.md\" >}}\n\nB end\n",
        ),
        ("content/c.md", "C\n"),
    ]);
    insta::assert_snapshot!(resolved(&fs, "Top\n\n{{< include \"a.md\" >}}\n\nBottom\n"));
}

#[test]
fn lang_includes_a_file_as_a_code_block() {
    let fs = files(&[
        (
            "content/code/main.rs",
            "fn main() {\n    println!(\"hi\");\n}\n\n",
        ),
        ("content/code/notes.txt", "plain\n"),
        ("content/code/tricky.md", "```\ninner fence\n```\n"),
        (
            "content/code/jumbled.md",
            "# Not a heading\n{{< include \"main.rs\" >}}\n",
        ),
    ]);
    insta::assert_snapshot!([
        resolved(&fs, "{{< include \"code/main.rs\" lang=\"rust\" >}}"),
        resolved(&fs, "{{< include \"code/notes.txt\" lang=\"\" >}}"),
        // Markdown included as code is shown verbatim, never resolved.
        resolved(&fs, "{{< include \"code/jumbled.md\" lang=\"md\" >}}"),
        resolved(&fs, "{{< include \"code/tricky.md\" lang=\"md\" >}}"),
    ]
    .join("\n=====\n"));
}

#[test]
fn includes_compile_to_lexical() {
    let fs = files(&[("content/part.md", "## From the part\n\nSome **text**.\n")]);
    let doc = fs
        .resolve(
            "Before\n\n{{< include \"part.md\" >}}\n\nAfter\n",
            "content/post.md",
        )
        .unwrap();
    insta::assert_json_snapshot!(doc.to_lexical());
}

#[test]
fn unresolved_includes_stay_visible() {
    // Compiling without resolving shows the directive instead of dropping it.
    let doc = parse("Before\n\n{{< include \"part.md\" lang=\"rust\" >}}\n").unwrap();
    insta::assert_snapshot!(format!(
        "{}\n=====\n{}",
        to_markdown(&doc.body),
        doc.to_lexical()["root"]["children"][1]
    ));
}

#[test]
fn directives_inside_quotes_are_kept_as_text() {
    let doc = parse("> {{< include \"a.md\" >}}\n").unwrap();
    insta::assert_snapshot!(to_markdown(&doc.body));
}

#[test]
fn failures_are_reported_with_the_offending_path() {
    let fs = files(&[
        ("content/self.md", "{{< include \"self.md\" >}}\n"),
        ("content/ping.md", "{{< include \"pong.md\" >}}\n"),
        ("content/pong.md", "{{< include \"ping.md\" >}}\n"),
        ("content/outer.md", "{{< include \"broken.md\" >}}\n"),
        ("content/broken.md", "{{< include \"missing.md\" >}}\n"),
    ]);
    let errors = [
        "{{< include \"missing.md\" >}}",
        "{{< include \"self.md\" >}}",
        "{{< include \"ping.md\" >}}",
        "{{< include \"outer.md\" >}}",
        "ok\n\n{{< include \"nope/x.md\" lang=\"rs\" >}}",
    ]
    .map(|input| resolved(&fs, input));
    insta::assert_debug_snapshot!(errors);
}

#[test]
fn nesting_has_a_depth_limit() {
    // 13 files, each including the next.
    let contents: Vec<&'static str> = (0..13)
        .map(|i| {
            &*Box::leak(
                format!("level {i}\n\n{{{{< include \"f{}.md\" >}}}}\n", i + 1).into_boxed_str(),
            )
        })
        .collect();
    let names: Vec<&'static str> = (0..13)
        .map(|i| &*Box::leak(format!("content/f{i}.md").into_boxed_str()))
        .collect();
    let fs = Files(names.into_iter().zip(contents).collect());
    insta::assert_snapshot!(resolved(&fs, "{{< include \"f0.md\" >}}"));
}

#[test]
fn includes_survive_a_markdown_round_trip() {
    let source =
        "Before\n\n{{< include \"a.md\" >}}\n\n{{< include \"b.rs\" lang=\"rust\" >}}\n\nAfter\n";
    let once = to_markdown(&parse(source).unwrap().body);
    let twice = to_markdown(&parse(&once).unwrap().body);
    insta::assert_snapshot!(format!("{once}\n=== stable: {}", once == twice));
}

#[test]
fn html_files_are_included_as_one_verbatim_block() {
    let demo = "<div class=\"demo\">\n  <h1></h1>\n\n  <div class=\"controls\">\n    <button>Step</button>\n  </div>\n\n  <template id=\"t\">\n    <p>hi</p>\n  </template>\n</div>\n";
    let fs = files(&[
        ("content/demo.html", demo),
        ("content/DEMO.HTM", "<p>upper case extension</p>\n"),
        ("content/notes.md", "# Heading\n\ntext\n"),
    ]);
    let lexical = |main: &str| {
        let doc = fs.resolve(main, "content/post.md").unwrap();
        doc.to_lexical()["root"]["children"].clone()
    };
    insta::assert_json_snapshot!([
        lexical("Before\n\n{{< include \"demo.html\" >}}\n\nAfter\n"),
        lexical("{{<include \"DEMO.HTM\">}}"),
        // `lang` still wins: shown as code, not rendered.
        lexical("{{< include \"demo.html\" lang=\"html\" >}}"),
        // Other extensions are markdown.
        lexical("{{< include \"notes.md\" >}}"),
    ]);
}
