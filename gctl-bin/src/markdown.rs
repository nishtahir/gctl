use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use gctl_ghost::{Post, Tag};
use gctl_markdown::{Document, FrontMatter};

use crate::error::GctlCliError;

/// A content file ready to publish.
pub struct CompiledPost {
    pub post: Post,
    /// Whether the file is a Ghost page (`type: page`) rather than a post.
    pub page: bool,
    /// Content kinds the file was pulled with approximations for.
    pub lossy: Vec<String>,
}

/// Where a markdown file lives, so `{{< include >}}` can find other files.
pub struct Source<'a> {
    pub file: &'a Path,
    /// Includes may only read files below this directory.
    pub root: &'a Path,
}

/// Compiles a markdown file with YAML front matter into a Ghost post whose
/// body is Lexical. Includes are resolved when `source` is given.
#[cfg(test)]
pub fn compile_markdown(
    content: &str,
    source: Option<&Source>,
) -> Result<CompiledPost, GctlCliError> {
    compile_markdown_with(content, source, &HashMap::new())
}

/// Like [`compile_markdown`], with every image `src` (and the `image` front
/// matter) that is a key in `images` replaced by its value.
pub fn compile_markdown_with(
    content: &str,
    source: Option<&Source>,
    images: &HashMap<String, String>,
) -> Result<CompiledPost, GctlCliError> {
    compile_at(content, source, images, Utc::now())
}

#[cfg(test)]
fn compile_markdown_at(
    content: &str,
    source: Option<&Source>,
    now: DateTime<Utc>,
) -> Result<CompiledPost, GctlCliError> {
    compile_at(content, source, &HashMap::new(), now)
}

fn compile_at(
    content: &str,
    source: Option<&Source>,
    images: &HashMap<String, String>,
    now: DateTime<Utc>,
) -> Result<CompiledPost, GctlCliError> {
    let mut document = gctl_markdown::parse(content)?;
    // Before includes are resolved: an image's path is relative to the file
    // that wrote it, which is only known here.
    document.visit_images_mut(&mut |src| {
        if let Some(url) = images.get(src.as_str()) {
            *src = url.clone();
        }
    });
    if let Some(url) = document
        .front_matter
        .feature_image
        .as_ref()
        .and_then(|image| images.get(image))
    {
        document.front_matter.feature_image = Some(url.clone());
    }
    if let Some(source) = source {
        let file = fs::canonicalize(source.file)?;
        document.resolve_includes(
            &file.display().to_string(),
            &mut include_loader(source.root),
        )?;
    }
    let page = match document.front_matter.kind.as_deref() {
        None | Some("post") => false,
        Some("page") => true,
        Some(other) => {
            return Err(GctlCliError::Parse(format!(
                "invalid `type` '{other}': expected post or page"
            )))
        }
    };
    let post = to_post(&document, now)?;
    Ok(CompiledPost {
        post,
        page,
        lossy: document.front_matter.lossy.clone(),
    })
}

fn to_post(document: &Document, now: DateTime<Utc>) -> Result<Post, GctlCliError> {
    let fm = &document.front_matter;
    let published_at = fm.published_at()?;

    let status = resolve_status(fm, published_at, now)?;

    let tags = fm.all_tags();
    Ok(Post {
        id: fm.id.clone(),
        title: fm.title.clone(),
        slug: fm.slug.clone(),
        lexical: Some(document.to_lexical_string()),
        status: status.map(str::to_string),
        published_at: published_at.map(|d| d.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()),
        custom_excerpt: fm.description.clone(),
        feature_image: fm.feature_image.clone(),
        featured: fm.featured,
        visibility: fm.visibility.clone(),
        canonical_url: fm.canonical_url.clone(),
        codeinjection_head: fm.head_injection()?.map(str::to_string),
        codeinjection_foot: fm.foot_injection()?.map(str::to_string),
        tags: (!tags.is_empty()).then(|| {
            tags.into_iter()
                .map(|name| Tag {
                    id: None,
                    name: Some(name),
                    slug: None,
                    description: None,
                    feature_image: None,
                    visibility: None,
                    url: None,
                })
                .collect()
        }),
        ..Post::default()
    })
}

/// Resolves `{{< include >}}` directives and injection files in `document`.
pub fn resolve_includes(
    document: &mut gctl_markdown::Document,
    source: &Source,
) -> Result<(), GctlCliError> {
    let file = fs::canonicalize(source.file)?;
    document.resolve_includes(
        &file.display().to_string(),
        &mut include_loader(source.root),
    )?;
    Ok(())
}

/// Reads included files from disk, relative to the including file. A file
/// outside `root` is refused, including through `..` or a symlink, so a shared
/// markdown file can't pull arbitrary files from the machine into a post.
fn include_loader(root: &Path) -> impl FnMut(&str, &str) -> Result<(String, String), String> {
    let root: PathBuf = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    move |from, path| {
        let base = Path::new(from).parent().unwrap_or(Path::new("."));
        let wanted = base.join(path);
        let file = fs::canonicalize(&wanted)
            .map_err(|e| format!("cannot read {}: {e}", wanted.display()))?;
        if !file.starts_with(&root) {
            return Err(format!(
                "{} is outside {}; includes must stay inside the content directory (see --include-root)",
                file.display(),
                root.display()
            ));
        }
        if !file.is_file() {
            return Err(format!("{} is not a file", file.display()));
        }
        let content = fs::read_to_string(&file)
            .map_err(|e| format!("cannot read {}: {e}", file.display()))?;
        Ok((file.display().to_string(), content))
    }
}

/// The post's status comes from its front matter: `status:` if given, else
/// `draft: true|false`. A published post dated in the future is scheduled
/// instead. `None` means the file doesn't say; the caller decides (a new post
/// becomes a draft, an existing one keeps the status it has).
fn resolve_status(
    fm: &FrontMatter,
    published_at: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
) -> Result<Option<&'static str>, GctlCliError> {
    if fm.status.is_none() && fm.draft.is_none() {
        return Ok(None);
    }
    let wants_published = match (fm.status.as_deref(), fm.draft) {
        (Some("draft"), Some(false)) | (Some("published" | "scheduled"), Some(true)) => {
            return Err(GctlCliError::Parse(
                "front matter `status` and `draft` contradict each other".into(),
            ))
        }
        (Some("draft"), _) | (None, Some(true) | None) => false,
        (Some("published" | "scheduled"), _) | (None, Some(false)) => true,
        (Some(other), _) => {
            return Err(GctlCliError::Parse(format!(
                "invalid `status` '{other}': expected draft, published or scheduled"
            )))
        }
    };

    let in_future = published_at.is_some_and(|date| date > now);
    match (wants_published, fm.status.as_deref()) {
        (false, _) => Ok(Some("draft")),
        (true, _) if in_future => Ok(Some("scheduled")),
        (true, Some("scheduled")) => Err(GctlCliError::Parse(
            "`status: scheduled` needs a `date` in the future".into(),
        )),
        (true, _) => Ok(Some("published")),
    }
}

#[cfg(test)]
mod test {
    use super::*;

    fn at(date: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(date)
            .unwrap()
            .with_timezone(&Utc)
    }

    fn compile(content: &str) -> CompiledPost {
        compile_markdown_at(content, None, at("2024-06-01T00:00:00Z")).unwrap()
    }

    #[test]
    fn maps_front_matter_to_post() {
        let compiled = compile(
            "---\ntitle: Hello World\nslug: hello-world\nid: foo\ndate: 2024-01-31\ntags: [a, b]\ndescription: Short\n---\n\nSome *content*\n",
        );
        let mut post = compiled.post;
        // The lexical body is covered by the markdown crate's snapshot.
        assert!(post.lexical.take().unwrap().contains("Some "));
        insta::assert_json_snapshot!(post);
    }

    #[test]
    fn rewrites_image_sources() {
        let images = HashMap::from([
            (
                "img/a.png".to_string(),
                "https://cdn.test/a.png".to_string(),
            ),
            (
                "cover.png".to_string(),
                "https://cdn.test/cover.png".to_string(),
            ),
        ]);
        let compiled = compile_at(
            "---\ntitle: T\nimage: cover.png\n---\n\n![a](img/a.png)\n\n![b](https://x.test/b.png)\n",
            None,
            &images,
            at("2024-06-01T00:00:00Z"),
        )
        .unwrap();
        let lexical = compiled.post.lexical.unwrap();
        let srcs: Vec<_> = [
            "https://cdn.test/a.png",
            "https://x.test/b.png",
            "img/a.png",
        ]
        .iter()
        .map(|src| (*src, lexical.contains(src)))
        .collect();
        insta::assert_debug_snapshot!((compiled.post.feature_image, srcs));
    }

    #[test]
    fn type_chooses_between_posts_and_pages() {
        let kinds: Vec<_> = ["", "type: post", "type: page", "type: article"]
            .iter()
            .map(|fm| {
                let result = compile_markdown(&format!("---\ntitle: T\n{fm}\n---\n"), None)
                    .map(|compiled| compiled.page)
                    .map_err(|e| e.to_string());
                (*fm, result)
            })
            .collect();
        insta::assert_debug_snapshot!(kinds);
    }

    #[test]
    fn maps_code_injection() {
        let post = |fm: &str| {
            let post = compile(&format!("---\ntitle: T\n{fm}\n---\n")).post;
            (
                fm.to_string(),
                post.codeinjection_head,
                post.codeinjection_foot,
            )
        };
        insta::assert_debug_snapshot!([
            post("codeinjection_head: |\n  <script>\n    a();\n  </script>\ncodeinjection_foot: <b>foot</b>"),
            // Left out: not sent, so the post keeps what it has.
            post("slug: x"),
            // Empty: sent, so the post's injection is cleared.
            post("codeinjection_head: \"\"\ncodeinjection_foot: \"\""),
        ]);
    }

    #[test]
    fn status_comes_from_front_matter() {
        let status = |fm: &str| {
            let result = compile_markdown_at(
                &format!("---\n{fm}\n---\n"),
                None,
                at("2024-06-01T00:00:00Z"),
            )
            .map(|compiled| compiled.post.status)
            .map_err(|e| e.to_string());
            (fm.to_string(), result)
        };
        insta::assert_debug_snapshot!([
            // Nothing specified: left to the caller (draft on create, unchanged on update).
            status("title: a"),
            status("draft: true"),
            status("draft: false"),
            status("status: draft"),
            status("status: published"),
            // Future dates schedule instead of publishing.
            status("draft: false\ndate: 2024-01-01"),
            status("draft: false\ndate: 2025-01-01"),
            status("status: published\ndate: 2025-01-01"),
            status("status: scheduled\ndate: 2025-01-01"),
            status("status: draft\ndate: 2025-01-01"),
            status("date: 2025-01-01"),
            // Problems.
            status("status: scheduled"),
            status("status: scheduled\ndate: 2024-01-01"),
            status("status: live"),
            status("status: draft\ndraft: false"),
            status("status: published\ndraft: true"),
        ]);
    }

    #[test]
    fn rejects_invalid_front_matter() {
        let errors: Vec<_> = ["---\ndate: nope\n---\n", "---\n: [\n---\n"]
            .iter()
            .map(|input| compile_markdown(input, None).err().map(|e| e.to_string()))
            .collect();
        insta::assert_debug_snapshot!(errors);
    }

    /// A throwaway directory tree; returns its canonical root.
    struct Tree(PathBuf);

    impl Tree {
        fn new(name: &str, files: &[(&str, &str)]) -> Tree {
            let root = std::env::temp_dir().join(format!("gctl-md-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&root);
            for (path, content) in files {
                let full = root.join(path);
                fs::create_dir_all(full.parent().unwrap()).unwrap();
                fs::write(full, content).unwrap();
            }
            Tree(fs::canonicalize(&root).unwrap())
        }

        /// Compiles `file` (relative to the tree) with includes allowed in `root`.
        /// Returns the text of the resulting Lexical, or the error with the
        /// temp directory replaced by `<tree>`.
        fn compile(&self, file: &str, root: &str) -> String {
            let path = self.0.join(file);
            let content = fs::read_to_string(&path).unwrap();
            let source = Source {
                file: &path,
                root: &self.0.join(root),
            };
            match compile_markdown_at(&content, Some(&source), at("2024-06-01T00:00:00Z")) {
                Ok(compiled) => {
                    let lexical: serde_json::Value =
                        serde_json::from_str(&compiled.post.lexical.unwrap()).unwrap();
                    lexical["root"]["children"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .map(|node| {
                            let texts: Vec<&str> = node["children"]
                                .as_array()
                                .map(|c| c.iter().filter_map(|c| c["text"].as_str()).collect())
                                .unwrap_or_default();
                            let text = node["code"]
                                .as_str()
                                .map_or_else(|| texts.join(""), str::to_string);
                            format!("{}: {text}", node["type"].as_str().unwrap())
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                }
                Err(e) => format!(
                    "ERROR {}",
                    e.to_string().replace(self.0.to_str().unwrap(), "<tree>")
                ),
            }
        }
    }

    impl Tree {
        /// The injections `file` would send, or the error (temp dir redacted).
        fn injections(&self, file: &str, root: &str) -> String {
            let path = self.0.join(file);
            let content = fs::read_to_string(&path).unwrap();
            let source = Source {
                file: &path,
                root: &self.0.join(root),
            };
            match compile_markdown_at(&content, Some(&source), at("2024-06-01T00:00:00Z")) {
                Ok(c) => format!(
                    "head={:?} foot={:?}",
                    c.post.codeinjection_head, c.post.codeinjection_foot
                ),
                Err(e) => format!(
                    "ERROR {}",
                    e.to_string().replace(self.0.to_str().unwrap(), "<tree>")
                ),
            }
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn includes_are_read_from_disk_relative_to_the_including_file() {
        let tree = Tree::new(
            "relative",
            &[
                ("content/post.md", "---\ntitle: T\n---\n\nIntro\n\n{{< include \"_parts/footer.md\" >}}\n"),
                ("content/_parts/footer.md", "Footer\n\n{{< include \"deeper/sig.md\" >}}\n\n{{< include \"../shared.md\" >}}\n"),
                ("content/_parts/deeper/sig.md", "Signature\n"),
                ("content/shared.md", "Shared\n"),
            ],
        );
        insta::assert_snapshot!(tree.compile("content/post.md", "content"));
    }

    #[test]
    fn includes_cannot_leave_the_root() {
        let tree = Tree::new(
            "escape",
            &[
                ("content/dotdot.md", "{{< include \"../secret.md\" >}}\n"),
                ("content/absolute.md", "x\n"),
                (
                    "content/lang.md",
                    "{{< include \"../secret.md\" lang=\"txt\" >}}\n",
                ),
                ("content/nested.md", "{{< include \"_p/escape.md\" >}}\n"),
                (
                    "content/_p/escape.md",
                    "{{< include \"../../secret.md\" >}}\n",
                ),
                ("secret.md", "TOP SECRET\n"),
            ],
        );
        let mut results = vec![
            ("dotdot", tree.compile("content/dotdot.md", "content")),
            ("lang", tree.compile("content/lang.md", "content")),
            ("nested", tree.compile("content/nested.md", "content")),
            // A wider root is an explicit choice.
            ("widened root", tree.compile("content/dotdot.md", ".")),
        ];
        let absolute = format!(
            "{{{{< include \"{}\" >}}}}\n",
            tree.0.join("secret.md").display()
        );
        fs::write(tree.0.join("content/absolute.md"), absolute).unwrap();
        results.push((
            "absolute path",
            tree.compile("content/absolute.md", "content"),
        ));
        insta::assert_debug_snapshot!(results);
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_cannot_be_used_to_leave_the_root() {
        let tree = Tree::new(
            "symlink",
            &[
                ("content/post.md", "{{< include \"link.md\" >}}\n"),
                ("outside/secret.md", "TOP SECRET\n"),
            ],
        );
        std::os::unix::fs::symlink(
            tree.0.join("outside/secret.md"),
            tree.0.join("content/link.md"),
        )
        .unwrap();
        insta::assert_snapshot!(tree.compile("content/post.md", "content"));
    }

    #[test]
    fn include_failures_name_the_problem() {
        let tree = Tree::new(
            "failures",
            &[
                ("content/missing.md", "{{< include \"nope.md\" >}}\n"),
                ("content/dir.md", "{{< include \"_d\" >}}\n"),
                ("content/_d/x.md", "x\n"),
                ("content/a.md", "{{< include \"b.md\" >}}\n"),
                ("content/b.md", "{{< include \"a.md\" >}}\n"),
            ],
        );
        let results = [
            tree.compile("content/missing.md", "content"),
            tree.compile("content/dir.md", "content"),
            tree.compile("content/a.md", "content"),
        ];
        insta::assert_debug_snapshot!(results);
    }

    #[test]
    fn code_includes_read_files_verbatim() {
        let tree = Tree::new(
            "code",
            &[
                (
                    "content/post.md",
                    "{{< include \"_code/main.rs\" lang=\"rust\" >}}\n",
                ),
                (
                    "content/_code/main.rs",
                    "fn main() {\n    // {{< include \"nested.md\" >}}\n}\n",
                ),
            ],
        );
        insta::assert_snapshot!(tree.compile("content/post.md", "content"));
    }

    #[test]
    fn without_a_source_includes_are_left_visible() {
        let compiled = compile_markdown("{{< include \"x.md\" >}}\n", None).unwrap();
        insta::assert_snapshot!(compiled.post.lexical.unwrap());
    }

    #[test]
    fn injection_files_are_read_from_disk() {
        let tree = Tree::new(
            "injection",
            &[
                ("content/posts/post.md", "---\ntitle: T\ncodeinjection_head: {file: ../_snippets/head.html}\ncodeinjection_foot: {file: ../_snippets/empty.html}\n---\n\nBody\n"),
                ("content/_snippets/head.html", "<script>\n  go();\n</script>\n"),
                ("content/_snippets/empty.html", ""),
                ("content/posts/inline.md", "---\ncodeinjection_head: '<b>inline</b>'\n---\n"),
            ],
        );
        insta::assert_debug_snapshot!([
            tree.injections("content/posts/post.md", "content"),
            tree.injections("content/posts/inline.md", "content"),
        ]);
    }

    #[test]
    fn injection_files_cannot_leave_the_root() {
        let tree = Tree::new(
            "injection-escape",
            &[
                (
                    "content/dotdot.md",
                    "---\ncodeinjection_head: {file: ../secret.html}\n---\n",
                ),
                (
                    "content/missing.md",
                    "---\ncodeinjection_foot: {file: nope.html}\n---\n",
                ),
                ("secret.html", "<script>steal()</script>\n"),
            ],
        );
        insta::assert_debug_snapshot!([
            tree.injections("content/dotdot.md", "content"),
            tree.injections("content/missing.md", "content"),
            // Allowed once the root is widened on purpose.
            tree.injections("content/dotdot.md", "."),
        ]);
    }

    #[test]
    fn injection_files_without_a_source_are_an_error() {
        let result = compile_markdown("---\ncodeinjection_head: {file: head.html}\n---\n", None)
            .err()
            .map(|e| e.to_string());
        insta::assert_debug_snapshot!(result);
    }
}
