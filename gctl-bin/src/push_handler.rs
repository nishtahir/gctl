use std::fs;
use std::path::{Path, PathBuf};

use bon::Builder;
use console::style;
use gctl_ghost::{GhostContentApi, Post, PostFilter};
use tracing::{debug, info};

use crate::cli::PushArgs;
use crate::error::GctlCliError;
use crate::markdown::{compile_markdown, Source};
use crate::output::{format_sync_results, SyncRow};

#[derive(Builder)]
pub struct PushCommandHandler<'a> {
    api: &'a GhostContentApi<'a>,
}

enum Outcome {
    Created,
    Updated,
}

struct Pushed {
    outcome: Outcome,
    post: Post,
    /// Something worth telling the user that didn't stop the push.
    warning: Option<String>,
}

impl<'a> PushCommandHandler<'a> {
    pub async fn handle_command(&self, args: PushArgs) -> Result<(), GctlCliError> {
        let root = PathBuf::from(&args.path);
        let files = find_content_files(&root)?;
        if files.is_empty() {
            return Err(GctlCliError::Push(format!(
                "No markdown files found in {}",
                root.display()
            )));
        }
        info!("Pushing {} file(s)...", files.len());

        // Includes may read inside the pushed directory (or the folder of a
        // single pushed file) unless a wider root is given explicitly.
        let include_root = match &args.include_root {
            Some(dir) => PathBuf::from(dir),
            None if root.is_dir() => root.clone(),
            None => root
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .map_or_else(|| PathBuf::from("."), Path::to_path_buf),
        };

        let mut rows = Vec::new();
        let mut failures = 0;
        for file in &files {
            let name = display_name(&root, file);
            debug!("Pushing {}", name);
            match self.push_file(file, &include_root, &args).await {
                Ok(Pushed {
                    outcome,
                    post,
                    warning,
                }) => {
                    let url = post.url.unwrap_or_default();
                    rows.push(SyncRow {
                        file: name,
                        result: outcome_label(&outcome, args.dry_run).to_string(),
                        status: post.status.unwrap_or_else(|| "(unchanged)".into()),
                        detail: match warning {
                            Some(warning) => format!("{url} (warning: {warning})"),
                            None => url,
                        },
                    })
                }
                Err(err) => {
                    failures += 1;
                    rows.push(SyncRow {
                        file: name,
                        result: "failed".into(),
                        status: String::new(),
                        detail: err.to_string(),
                    });
                }
            }
        }

        format_sync_results(rows, args.json)?;

        if failures > 0 {
            return Err(GctlCliError::Push(format!(
                "{failures} of {} file(s) failed to push",
                files.len()
            )));
        }
        Ok(())
    }

    async fn push_file(
        &self,
        file: &Path,
        include_root: &Path,
        args: &PushArgs,
    ) -> Result<Pushed, GctlCliError> {
        let content = fs::read_to_string(file)?;
        let source = Source {
            file,
            root: include_root,
        };
        let compiled = compile_markdown(&content, Some(&source))?;
        if !compiled.lossy.is_empty() && !args.force {
            return Err(GctlCliError::Parse(format!(
                "pulled with approximations ({}); pushing would change the post. Review the file, then delete its `lossy:` line, or push with --force",
                compiled.lossy.join(", ")
            )));
        }
        let mut post = compiled.post;

        if post.title.as_deref().is_none_or(|t| t.trim().is_empty()) {
            return Err(GctlCliError::Parse(
                "front matter is missing a `title`".into(),
            ));
        }
        let slug = post.slug.clone().unwrap_or_else(|| slug_from_path(file));
        post.slug = Some(slug.clone());

        // The id in the file identifies the post, so edits to the slug or the
        // file name update it instead of creating another. Without one, fall
        // back to the slug.
        let recorded_id = post.id.take();
        let existing_id = match &recorded_id {
            Some(id) => Some(id.clone()),
            None => self.find_by_slug(&slug).await?,
        };

        // A file that doesn't say is a draft when it creates a post, so
        // nothing goes live by accident. When it updates one, the post keeps
        // its current status, so the first push of an existing file can't
        // unpublish it.
        if existing_id.is_none() && post.status.is_none() {
            post.status = Some("draft".into());
        }

        let (outcome, post) = match (existing_id, args.dry_run) {
            (Some(id), false) => (Outcome::Updated, self.api.posts().edit(&id, &post).await?),
            (None, false) => (Outcome::Created, self.api.posts().add(&post).await?),
            (Some(_), true) => (Outcome::Updated, post),
            (None, true) => (Outcome::Created, post),
        };
        info!("{} {}", style("Pushed").green(), style(&slug).yellow());

        let warning = match post.id.as_deref() {
            Some(id) if !args.dry_run && recorded_id.as_deref() != Some(id) => {
                save_id(file, &content, id)
                    .err()
                    .map(|e| format!("could not save the post id to the file: {e}"))
            }
            _ => None,
        };

        Ok(Pushed {
            outcome,
            post,
            warning,
        })
    }

    async fn find_by_slug(&self, slug: &str) -> Result<Option<String>, GctlCliError> {
        let filter = PostFilter {
            slug: Some(slug.to_string()),
            limit: Some(1),
            ..PostFilter::default()
        };
        let posts = self.api.posts().browse(&filter).await?;
        Ok(posts.into_iter().next().and_then(|p| p.id))
    }
}

fn outcome_label(outcome: &Outcome, dry_run: bool) -> &'static str {
    match (outcome, dry_run) {
        (Outcome::Created, false) => "created",
        (Outcome::Updated, false) => "updated",
        (Outcome::Created, true) => "would create",
        (Outcome::Updated, true) => "would update",
    }
}

/// Records the Ghost id in the file so later pushes find the same post.
fn save_id(file: &Path, content: &str, id: &str) -> std::io::Result<()> {
    fs::write(file, gctl_markdown::set_front_matter_id(content, id))
}

/// `content/posts/hello.md` -> `hello`; `content/hello/index.md` -> `hello`.
pub(crate) fn slug_from_path(file: &Path) -> String {
    let stem = file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    let name = if stem == "index" {
        file.parent()
            .and_then(|p| p.file_name())
            .and_then(|s| s.to_str())
            .unwrap_or(stem)
    } else {
        stem
    };
    slugify(name)
}

/// Lowercase ASCII, with runs of anything but letters and digits collapsed to
/// `-`. Staying within ASCII keeps the slug stable if Ghost normalizes it, so
/// the next push still finds the post.
pub(crate) fn slugify(name: &str) -> String {
    let mut slug = String::new();
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.trim_matches('-').to_string()
}

pub(crate) fn display_name(root: &Path, file: &Path) -> String {
    if root.is_file() {
        return file.file_name().map_or_else(
            || file.display().to_string(),
            |name| name.to_string_lossy().into_owned(),
        );
    }
    file.strip_prefix(root)
        .ok()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(file)
        .display()
        .to_string()
}

fn is_markdown(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("md" | "markdown")
    )
}

fn is_hidden(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.starts_with('.') || n.starts_with('_'))
}

/// A single file, or every markdown file below a directory, in a stable order.
pub(crate) fn find_content_files(root: &Path) -> Result<Vec<PathBuf>, GctlCliError> {
    if root.is_file() {
        return Ok(vec![root.to_path_buf()]);
    }
    if !root.is_dir() {
        return Err(GctlCliError::Push(format!(
            "{} does not exist",
            root.display()
        )));
    }
    let mut files = Vec::new();
    collect(root, &mut files)?;
    files.sort();
    Ok(files)
}

fn collect(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), GctlCliError> {
    for entry in fs::read_dir(dir)? {
        let path = entry?.path();
        if is_hidden(&path) {
            continue;
        }
        if path.is_dir() {
            collect(&path, files)?;
        } else if is_markdown(&path) {
            files.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn slugs_come_from_file_or_bundle_names() {
        let slugs: Vec<_> = [
            "content/Hello World.md",
            "content/posts/my_post.md",
            "content/my-bundle/index.md",
            "content/Ünï Cödé -- Post!.md",
        ]
        .iter()
        .map(|path| (*path, slug_from_path(Path::new(path))))
        .collect();
        insta::assert_debug_snapshot!(slugs);
    }

    #[test]
    fn finds_markdown_files_and_skips_hidden_and_underscored() {
        let dir = std::env::temp_dir().join(format!("gctl-push-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("nested")).unwrap();
        fs::create_dir_all(dir.join(".git")).unwrap();
        for f in [
            "b.md",
            "a.markdown",
            "nested/c.md",
            "_index.md",
            "notes.txt",
            ".git/x.md",
        ] {
            fs::write(dir.join(f), "").unwrap();
        }
        let found: Vec<String> = find_content_files(&dir)
            .unwrap()
            .iter()
            .map(|p| display_name(&dir, p))
            .collect();
        fs::remove_dir_all(&dir).unwrap();
        insta::assert_debug_snapshot!(found);
    }

    #[test]
    fn save_id_rewrites_only_the_front_matter() {
        let dir = std::env::temp_dir().join(format!("gctl-save-id-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        let file = dir.join("post.md");

        let original = "---\ntitle: Hello\n---\n\nBody with `---` and id: x\n";
        fs::write(&file, original).unwrap();
        save_id(&file, original, "abc123").unwrap();
        let first = fs::read_to_string(&file).unwrap();

        // Saving again with a different id replaces rather than duplicates.
        save_id(&file, &first, "def456").unwrap();
        let second = fs::read_to_string(&file).unwrap();
        fs::remove_dir_all(&dir).unwrap();

        insta::assert_debug_snapshot!((first, second));
    }
}
