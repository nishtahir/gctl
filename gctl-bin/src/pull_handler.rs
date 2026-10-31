use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use bon::Builder;
use console::style;
use gctl_ghost::{GhostContentApi, Post, PostFilter};
use gctl_markdown::FrontMatter;
use serde::Serialize;
use tracing::{debug, info};

use crate::cli::PullArgs;
use crate::error::GctlCliError;
use crate::markdown::{resolve_includes, Source};
use crate::output::{format_sync_results, SyncRow};
use crate::push_handler::{display_name, find_content_files};

#[derive(Builder)]
pub struct PullCommandHandler<'a> {
    api: &'a GhostContentApi<'a>,
}

/// A post rendered as the text of a markdown file.
#[derive(Debug)]
pub struct RenderedPost {
    pub file: String,
    /// Kinds of content that were approximated in markdown.
    pub lossy: Vec<String>,
}

/// Front matter in the order it is written. The id is written separately so it
/// is always quoted, matching what `gctl push` writes back.
#[derive(Serialize)]
struct PulledFrontMatter<'a> {
    title: &'a str,
    slug: &'a str,
    status: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    date: Option<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    tags: Vec<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    image: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    featured: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    visibility: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    canonical: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    codeinjection_head: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    codeinjection_foot: Option<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    lossy: Vec<&'a str>,
}

fn non_empty(value: &Option<String>) -> Option<&str> {
    value.as_deref().filter(|v| !v.is_empty())
}

/// Renders a post as a markdown file. `None` when it has no Lexical content
/// to convert (e.g. a post still stored as mobiledoc).
pub fn render_post(post: &Post) -> Result<Option<RenderedPost>, GctlCliError> {
    let (Some(id), Some(lexical)) = (post.id.as_deref(), post.lexical.as_deref()) else {
        return Ok(None);
    };
    let imported = gctl_markdown::from_lexical(lexical)
        .map_err(|e| GctlCliError::Parse(e.to_string()))?;
    let body = gctl_markdown::to_markdown(&imported.blocks);

    let tags: Vec<&str> = post
        .tags
        .iter()
        .flatten()
        .filter_map(|tag| tag.name.as_deref())
        .collect();
    let front_matter = PulledFrontMatter {
        title: post.title.as_deref().unwrap_or_default(),
        slug: post.slug.as_deref().unwrap_or_default(),
        // Emailed posts are published posts.
        status: match post.status.as_deref() {
            Some("sent") | None => "published",
            Some(status) => status,
        },
        date: non_empty(&post.published_at),
        tags,
        description: non_empty(&post.custom_excerpt),
        image: non_empty(&post.feature_image),
        featured: post.featured.filter(|&featured| featured),
        visibility: non_empty(&post.visibility).filter(|v| *v != "public"),
        canonical: non_empty(&post.canonical_url),
        codeinjection_head: non_empty(&post.codeinjection_head),
        codeinjection_foot: non_empty(&post.codeinjection_foot),
        lossy: imported.lossy.iter().map(String::as_str).collect(),
    };

    let yaml = serde_yaml::to_string(&front_matter)
        .map_err(|e| GctlCliError::Parse(e.to_string()))?;
    let mut file = format!("---\nid: \"{id}\"\n{yaml}---\n");
    if !body.trim().is_empty() {
        file.push('\n');
        file.push_str(&body);
    }
    Ok(Some(RenderedPost {
        file,
        lossy: imported.lossy,
    }))
}

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Create,
    Update,
    Unchanged,
    /// A different file is already there and `--force` wasn't given.
    Conflict,
}

/// What to do with `rendered` given the file currently at its path. `source`
/// lets the file's includes and injection files be read before comparing.
pub fn decide(
    existing: Option<&str>,
    source: Option<&Source>,
    rendered: &str,
    force: bool,
) -> Action {
    match existing {
        None => Action::Create,
        Some(existing) if in_sync(existing, source, rendered) => Action::Unchanged,
        Some(_) if force => Action::Update,
        Some(_) => Action::Conflict,
    }
}

/// Whether a local file already says what the post says. Formatting that
/// markdown ignores (spacing, `_` vs `*`) doesn't count as a difference.
fn in_sync(existing: &str, source: Option<&Source>, rendered: &str) -> bool {
    if existing == rendered {
        return true;
    }
    let (Ok(mut local), Ok(remote)) = (
        gctl_markdown::parse(existing),
        gctl_markdown::parse(rendered),
    ) else {
        return false;
    };
    // The post holds the flattened content, so compare against the same.
    if let Some(source) = source {
        if resolve_includes(&mut local, source).is_err() {
            return false;
        }
    }
    let normalized = |doc: &gctl_markdown::Document| -> Option<String> {
        let imported = gctl_markdown::from_lexical(&doc.to_lexical_string()).ok()?;
        Some(gctl_markdown::to_markdown(&imported.blocks))
    };
    front_matter_agrees(&local.front_matter, &remote.front_matter)
        && normalized(&local) == normalized(&remote)
}

/// Whether every front matter field the local file sets matches the post. A
/// field the file leaves out (a hand-written file rarely sets `status` or a
/// full timestamp) isn't a difference.
fn front_matter_agrees(local: &FrontMatter, remote: &FrontMatter) -> bool {
    fn same<T: PartialEq>(local: &Option<T>, remote: &Option<T>) -> bool {
        local.is_none() || local == remote
    }
    let status_agrees = match (&local.status, local.draft) {
        (Some(status), _) => remote.status.as_ref() == Some(status),
        (None, Some(draft)) => draft == (remote.status.as_deref() == Some("draft")),
        (None, None) => true,
    };
    // `2024-01-31` and `2024-01-31T00:00:00.000Z` are the same moment.
    let date_agrees = match (local.published_at(), remote.published_at()) {
        (Ok(None), _) => true,
        (Ok(local), Ok(remote)) => local == remote,
        _ => false,
    };
    same(&local.id, &remote.id)
        && same(&local.title, &remote.title)
        && same(&local.slug, &remote.slug)
        && same(&local.description, &remote.description)
        && same(&local.feature_image, &remote.feature_image)
        && same(&local.featured, &remote.featured)
        && same(&local.visibility, &remote.visibility)
        && same(&local.canonical_url, &remote.canonical_url)
        && same(&local.codeinjection_head, &remote.codeinjection_head)
        && same(&local.codeinjection_foot, &remote.codeinjection_foot)
        && (local.all_tags().is_empty() || local.all_tags() == remote.all_tags())
        && status_agrees
        && date_agrees
}

/// `<dir>/<slug>.md`, kept visible to `gctl push` and safe as a file name.
fn path_for(dir: &Path, post: &Post) -> PathBuf {
    let slug = crate::push_handler::slugify(post.slug.as_deref().unwrap_or_default());
    let name = if slug.is_empty() {
        post.id.clone().unwrap_or_else(|| "post".into())
    } else {
        slug
    };
    dir.join(format!("{name}.md"))
}

/// Files under `dir` that already carry a Ghost id, by id.
fn files_by_id(dir: &Path) -> HashMap<String, PathBuf> {
    let Ok(files) = find_content_files(dir) else {
        return HashMap::new();
    };
    files
        .into_iter()
        .filter_map(|path| {
            let content = fs::read_to_string(&path).ok()?;
            let id = gctl_markdown::parse(&content).ok()?.front_matter.id?;
            Some((id, path))
        })
        .collect()
}

impl<'a> PullCommandHandler<'a> {
    pub async fn handle_command(&self, args: PullArgs) -> Result<(), GctlCliError> {
        let dir = PathBuf::from(&args.path);
        info!("Fetching posts...");
        let posts = if let Some(id) = &args.id {
            vec![self.api.posts().read(id).await?]
        } else {
            let filter = PostFilter {
                status: args.status,
                tag: args.tag,
                author: args.author,
                raw: args.filter,
                limit: args.limit,
                ..PostFilter::default()
            };
            self.api.posts().browse(&filter).await?
        };
        debug!("Found {} posts", posts.len());

        let by_id = if dir.is_dir() {
            files_by_id(&dir)
        } else {
            HashMap::new()
        };
        let mut rows = Vec::new();
        let mut conflicts = 0;

        for post in &posts {
            let title = post.title.clone().unwrap_or_default();
            let status = post.status.clone().unwrap_or_default();
            let path = post
                .id
                .as_ref()
                .and_then(|id| by_id.get(id).cloned())
                .unwrap_or_else(|| path_for(&dir, post));
            let name = display_name(&dir, &path);

            let rendered = match render_post(post) {
                Ok(Some(rendered)) => rendered,
                Ok(None) => {
                    rows.push(row(name, "skipped", status, "no Lexical content".into()));
                    continue;
                }
                Err(err) => {
                    rows.push(row(name, "failed", status, format!("{title}: {err}")));
                    continue;
                }
            };

            let existing = fs::read_to_string(&path).ok();
            let source = Source {
                file: &path,
                root: &dir,
            };
            let action = decide(
                existing.as_deref(),
                Some(&source),
                &rendered.file,
                args.force,
            );
            let label = match (&action, args.dry_run) {
                (Action::Create, false) => "created",
                (Action::Create, true) => "would create",
                (Action::Update, false) => "updated",
                (Action::Update, true) => "would update",
                (Action::Unchanged, _) => "unchanged",
                (Action::Conflict, _) => "conflict",
            };

            let mut detail = match action {
                Action::Conflict => {
                    conflicts += 1;
                    "differs from Ghost; use --force to overwrite".to_string()
                }
                _ => String::new(),
            };
            if !rendered.lossy.is_empty() {
                if !detail.is_empty() {
                    detail.push_str("; ");
                }
                detail.push_str(&format!("lossy: {}", rendered.lossy.join(", ")));
            }

            if !args.dry_run && matches!(action, Action::Create | Action::Update) {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::write(&path, &rendered.file)?;
                info!("{} {}", style("Wrote").green(), path.display());
            }
            rows.push(row(name, label, status, detail));
        }

        format_sync_results(rows);

        if conflicts > 0 {
            return Err(GctlCliError::Push(format!(
                "{conflicts} file(s) differ from Ghost and were left alone"
            )));
        }
        Ok(())
    }
}

fn row(file: String, result: &str, status: String, detail: String) -> SyncRow {
    SyncRow {
        file,
        result: result.into(),
        status,
        detail,
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use gctl_ghost::Tag;
    use serde_json::json;

    /// `super::decide` without a source file: nothing to resolve.
    fn decide(existing: Option<&str>, rendered: &str, force: bool) -> Action {
        super::decide(existing, None, rendered, force)
    }

    fn tag(name: &str) -> Tag {
        Tag {
            id: None,
            name: Some(name.into()),
            slug: None,
            description: None,
            feature_image: None,
            visibility: None,
            url: None,
        }
    }

    fn lexical(nodes: serde_json::Value) -> String {
        json!({ "root": { "children": nodes, "direction": "ltr", "format": "",
                          "indent": 0, "type": "root", "version": 1 } })
        .to_string()
    }

    fn text(t: &str, format: u32) -> serde_json::Value {
        json!({ "type": "extended-text", "text": t, "format": format, "detail": 0,
                "mode": "normal", "style": "", "version": 1 })
    }

    fn post(lexical: Option<String>) -> Post {
        Post {
            id: Some("6891829ae317750001061429".into()),
            title: Some("Hello: World".into()),
            slug: Some("hello-world".into()),
            status: Some("published".into()),
            published_at: Some("2024-03-16T18:00:00.000Z".into()),
            tags: Some(vec![tag("rust"), tag("#internal")]),
            custom_excerpt: Some("A summary".into()),
            feature_image: Some("https://example.com/cover.png".into()),
            featured: Some(true),
            visibility: Some("members".into()),
            canonical_url: Some("https://example.com/original".into()),
            codeinjection_head: Some("<script>\n  track();\n</script>\n".into()),
            codeinjection_foot: Some("<div id=\"widget\"></div>".into()),
            lexical,
            ..Post::default()
        }
    }

    fn body() -> String {
        lexical(json!([
            { "type": "paragraph", "children": [text("Some ", 0), text("bold", 1), text(" text.", 0)],
              "direction": "ltr", "format": "", "indent": 0, "version": 1 },
            { "type": "callout", "calloutEmoji": "💡", "calloutText": "Careful" },
        ]))
    }

    #[test]
    fn renders_a_full_post() {
        let rendered = render_post(&post(Some(body()))).unwrap().unwrap();
        insta::assert_snapshot!(format!("{}\nlossy: {:?}", rendered.file, rendered.lossy));
    }

    #[test]
    fn renders_a_minimal_post() {
        let minimal = Post {
            id: Some("abc".into()),
            title: Some("Bare".into()),
            slug: Some("bare".into()),
            status: Some("draft".into()),
            visibility: Some("public".into()),
            featured: Some(false),
            tags: Some(vec![]),
            custom_excerpt: Some(String::new()),
            lexical: Some(lexical(json!([]))),
            ..Post::default()
        };
        insta::assert_snapshot!(render_post(&minimal).unwrap().unwrap().file);
    }

    #[test]
    fn emailed_posts_are_published_and_titles_are_quoted_when_needed() {
        let mut sent = post(Some(lexical(json!([]))));
        sent.status = Some("sent".into());
        sent.title = Some("Yes: 123 # not a comment".into());
        insta::assert_snapshot!(render_post(&sent).unwrap().unwrap().file);
    }

    #[test]
    fn posts_without_lexical_are_skipped() {
        assert!(render_post(&post(None)).unwrap().is_none());
    }

    #[test]
    fn rendered_files_read_back_with_the_same_front_matter() {
        let rendered = render_post(&post(Some(body()))).unwrap().unwrap();
        let parsed = gctl_markdown::parse(&rendered.file).unwrap();
        insta::assert_debug_snapshot!(parsed.front_matter);
    }

    #[test]
    fn decisions() {
        let rendered = render_post(&post(Some(body()))).unwrap().unwrap().file;
        let lines = |f: &dyn Fn(&str) -> String| f(&rendered);
        let variants: Vec<(&str, Action)> = vec![
            ("missing file", decide(None, &rendered, false)),
            ("identical", decide(Some(&rendered), &rendered, false)),
            // Things markdown ignores:
            (
                "extra blank lines",
                decide(Some(&lines(&|r| r.replace("\n\n", "\n\n\n\n"))),
                    &rendered,
                    false,
                ),
            ),
            (
                "trailing spaces",
                decide(Some(&lines(&|r| r.replace("text.\n", "text.   \n"))),
                    &rendered,
                    false,
                ),
            ),
            (
                "underscore emphasis",
                decide(Some(&lines(&|r| r.replace("**bold**", "__bold__"))),
                    &rendered,
                    false,
                ),
            ),
            (
                "crlf line endings",
                decide(Some(&lines(&|r| r.replace('\n', "\r\n"))), &rendered, false),
            ),
            (
                "front matter keys reordered",
                decide(Some(&lines(&|r| {
                        r.replace(
                            "title: 'Hello: World'\nslug: hello-world\n",
                            "slug: hello-world\ntitle: 'Hello: World'\n",
                        )
                    })),
                    &rendered,
                    false,
                ),
            ),
            // A hand-written file that only sets some fields:
            (
                "sparse hand-written front matter",
                decide(Some("---\nid: \"6891829ae317750001061429\"\ntitle: 'Hello: World'\ndate: 2024-03-16T18:00:00Z\ntags: [rust, '#internal']\ndraft: false\n---\n\nSome **bold** text.\n\n> 💡 Careful\n"),
                    &rendered,
                    false,
                ),
            ),
            (
                "hand-written, date without a time is midnight",
                decide(Some("---\ndate: 2024-03-16\n---\n\nSome **bold** text.\n\n> 💡 Careful\n"),
                    &rendered,
                    false,
                ),
            ),
            (
                "hand-written, draft flag disagrees",
                decide(Some("---\nid: \"6891829ae317750001061429\"\ndraft: true\n---\n\nSome **bold** text.\n\n> 💡 Careful\n"),
                    &rendered,
                    false,
                ),
            ),
            (
                "hand-written, tags disagree",
                decide(Some("---\ntags: [other]\n---\n\nSome **bold** text.\n\n> 💡 Careful\n"),
                    &rendered,
                    false,
                ),
            ),
            (
                "hand-written, different date",
                decide(Some("---\ndate: 2024-03-17\n---\n\nSome **bold** text.\n\n> 💡 Careful\n"),
                    &rendered,
                    false,
                ),
            ),
            // Code injection is content: changing it is a real difference.
            (
                "edited head injection",
                decide(Some(&lines(&|r| r.replace("track();", "track(1);"))), &rendered, false),
            ),
            (
                "injection dropped from the file (not a difference)",
                decide(Some("---\nid: \"6891829ae317750001061429\"\n---\n\nSome **bold** text.\n\n> 💡 Careful\n"),
                    &rendered,
                    false,
                ),
            ),
            (
                "injection emptied in the file (a difference)",
                decide(Some("---\ncodeinjection_head: \"\"\n---\n\nSome **bold** text.\n\n> 💡 Careful\n"),
                    &rendered,
                    false,
                ),
            ),
            // Real differences:
            (
                "edited text",
                decide(Some(&lines(&|r| r.replace("Some", "Other"))),
                    &rendered,
                    false,
                ),
            ),
            (
                "edited text, forced",
                decide(Some(&lines(&|r| r.replace("Some", "Other"))),
                    &rendered,
                    true,
                ),
            ),
            (
                "edited title",
                decide(Some(&lines(&|r| r.replace("Hello", "Howdy"))),
                    &rendered,
                    false,
                ),
            ),
            (
                "extra space inside a sentence",
                decide(Some(&lines(&|r| r.replace("Some **bold**", "Some  **bold**"))),
                    &rendered,
                    false,
                ),
            ),
            (
                "not even markdown",
                decide(Some("---\n: [\n---\n"), &rendered, false),
            ),
        ];
        insta::assert_debug_snapshot!(variants);
    }

    #[test]
    fn paths_are_built_from_slugs() {
        let dir = Path::new("content");
        let with_slug = |slug: &str| {
            let mut p = post(None);
            p.slug = Some(slug.into());
            path_for(dir, &p).display().to_string()
        };
        insta::assert_debug_snapshot!([
            with_slug("hello-world"),
            with_slug("2024-01-01-dated"),
            with_slug("Odd Slug/with..parts"),
            with_slug("_hidden"),
            with_slug(""),
        ]);
    }

    /// Decisions for a hand-written file that uses includes and injection
    /// files, compared with the flattened post.
    #[test]
    fn local_includes_and_injection_files_are_resolved_before_comparing() {
        let dir = std::env::temp_dir().join(format!("gctl-pull-resolve-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("_snippets")).unwrap();
        fs::write(
            dir.join("_snippets/head.html"),
            "<script>\n  track();\n</script>\n",
        )
        .unwrap();
        fs::write(dir.join("_snippets/foot.html"), "<div id=\"widget\"></div>").unwrap();
        fs::write(dir.join("_snippets/note.md"), "> 💡 Careful\n").unwrap();
        let dir = fs::canonicalize(&dir).unwrap();

        let rendered = render_post(&post(Some(body()))).unwrap().unwrap().file;
        let local = |fm: &str, body: &str| {
            format!("---\nid: \"6891829ae317750001061429\"\n{fm}---\n\n{body}")
        };
        let file = dir.join("post.md");
        let decide_with = |content: &str| {
            fs::write(&file, content).unwrap();
            let source = Source {
                file: &file,
                root: &dir,
            };
            super::decide(Some(content), Some(&source), &rendered, false)
        };

        let flattened_body = "Some **bold** text.\n\n{{< include \"_snippets/note.md\" >}}\n";
        let results = vec![
            (
                "files match the post",
                decide_with(&local(
                    "codeinjection_head: {file: _snippets/head.html}\ncodeinjection_foot: {file: _snippets/foot.html}\n",
                    flattened_body,
                )),
            ),
            (
                "inline injection and include",
                decide_with(&local("codeinjection_foot: '<div id=\"widget\"></div>'\n", flattened_body)),
            ),
            (
                "injection file differs",
                decide_with(&local("codeinjection_head: {file: _snippets/foot.html}\n", flattened_body)),
            ),
            (
                "injection file missing",
                decide_with(&local("codeinjection_head: {file: _snippets/gone.html}\n", flattened_body)),
            ),
            (
                "include missing",
                decide_with(&local("", "Some **bold** text.\n\n{{< include \"_snippets/gone.md\" >}}\n")),
            ),
            (
                "include escapes the directory",
                decide_with(&local("", "Some **bold** text.\n\n{{< include \"../outside.md\" >}}\n")),
            ),
        ];
        fs::remove_dir_all(&dir).unwrap();
        insta::assert_debug_snapshot!(results);
    }
}
