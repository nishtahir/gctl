use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use bon::Builder;
use console::style;
use gctl_ghost::{GhostContentApi, Post, PostFilter};
use tracing::{debug, info};

use crate::cli::PushArgs;
use crate::error::GctlCliError;
use crate::images::ImageStore;
use crate::markdown::{compile_markdown_with, Source};
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
            None => include_root_parent(&root),
        };

        // Where uploaded images are remembered: next to the posts.
        let cache_dir = if root.is_dir() {
            root.clone()
        } else {
            include_root_parent(&root)
        };
        let mut images = ImageStore::load(&cache_dir, self.api.site_url().as_str());

        if !args.watch {
            return self
                .push_files(&files, &root, &include_root, &mut images, &args)
                .await;
        }

        // Rerun the push whenever something in the directory changes.
        eprintln!(
            "Watching {} for changes (Ctrl-C to stop)...",
            root.display()
        );
        if let Err(err) = self
            .push_files(&files, &root, &include_root, &mut images, &args)
            .await
        {
            eprintln!("{}", style(err).red());
        }
        let mut seen = snapshot(&root);
        loop {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            let now = snapshot(&root);
            let changed = changed_files(&seen, &now, &root);
            if changed.is_empty() {
                continue;
            }
            // Give an editor time to finish writing before reading the file.
            tokio::time::sleep(std::time::Duration::from_millis(200)).await;
            if let Err(err) = self
                .push_files(&changed, &root, &include_root, &mut images, &args)
                .await
            {
                eprintln!("{}", style(err).red());
            }
            // Pushing writes post ids back into files; don't push those again.
            seen = snapshot(&root);
        }
    }

    /// Pushes `files` and prints what happened to each.
    async fn push_files(
        &self,
        files: &[PathBuf],
        root: &Path,
        include_root: &Path,
        images: &mut ImageStore,
        args: &PushArgs,
    ) -> Result<(), GctlCliError> {
        let mut rows = Vec::new();
        let mut failures = 0;
        for file in files {
            let name = display_name(root, file);
            debug!("Pushing {}", name);
            match self.push_file(file, include_root, images, args).await {
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

        // Uploads already happened, so keep track of them even if some files failed.
        if let Err(err) = images.save() {
            tracing::warn!("could not save {}: {err}", crate::images::CACHE_FILE);
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
        images: &mut ImageStore,
        args: &PushArgs,
    ) -> Result<Pushed, GctlCliError> {
        let content = fs::read_to_string(file)?;
        let source = Source {
            file,
            root: include_root,
        };
        let uploaded = images
            .upload_for(self.api, file, &content, args.dry_run)
            .await?;
        let compiled = compile_markdown_with(&content, Some(&source), &uploaded)?;
        if !compiled.lossy.is_empty() && !args.force {
            return Err(GctlCliError::Parse(format!(
                "pulled with approximations ({}); pushing would change the post. Review the file, then delete its `lossy:` line, or push with --force",
                compiled.lossy.join(", ")
            )));
        }
        let page = compiled.page;
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
            None => self.find_by_slug(&slug, page).await?,
        };

        // A file that doesn't say is a draft when it creates a post, so
        // nothing goes live by accident. When it updates one, the post keeps
        // its current status, so the first push of an existing file can't
        // unpublish it.
        if existing_id.is_none() && post.status.is_none() {
            post.status = Some("draft".into());
        }

        let (outcome, post) = match (existing_id, args.dry_run) {
            (Some(id), false) => (
                Outcome::Updated,
                self.api.collection(page).edit(&id, &post).await?,
            ),
            (None, false) => (
                Outcome::Created,
                self.api.collection(page).add(&post).await?,
            ),
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

    async fn find_by_slug(&self, slug: &str, page: bool) -> Result<Option<String>, GctlCliError> {
        let filter = PostFilter {
            slug: Some(slug.to_string()),
            limit: Some(1),
            ..PostFilter::default()
        };
        let posts = self.api.collection(page).browse(&filter).await?;
        Ok(posts.into_iter().next().and_then(|p| p.id))
    }
}

/// Modification times of everything under `root` that isn't hidden. Includes
/// and injection files count, so editing a snippet is noticed too.
fn snapshot(root: &Path) -> HashMap<PathBuf, SystemTime> {
    fn walk(path: &Path, out: &mut HashMap<PathBuf, SystemTime>) {
        let hidden = path
            .file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.starts_with('.') && n != "." && n != "..");
        if hidden {
            return;
        }
        if path.is_dir() {
            if let Ok(entries) = fs::read_dir(path) {
                for entry in entries.flatten() {
                    walk(&entry.path(), out);
                }
            }
        } else if let Some(modified) = fs::metadata(path).ok().and_then(|m| m.modified().ok()) {
            out.insert(path.to_path_buf(), modified);
        }
    }
    let mut out = HashMap::new();
    walk(root, &mut out);
    out
}

/// The posts to push after `before` became `after`: the posts that changed,
/// or all of them when something else (a snippet) did.
fn changed_files(
    before: &HashMap<PathBuf, SystemTime>,
    after: &HashMap<PathBuf, SystemTime>,
    root: &Path,
) -> Vec<PathBuf> {
    let changed: Vec<&PathBuf> = after
        .iter()
        .filter(|(path, time)| before.get(*path) != Some(time))
        .map(|(path, _)| path)
        .collect();
    if changed.is_empty() {
        return Vec::new();
    }
    let posts = find_content_files(root).unwrap_or_default();
    if changed.iter().all(|path| posts.contains(path)) {
        let mut changed: Vec<PathBuf> = changed.into_iter().cloned().collect();
        changed.sort();
        changed
    } else {
        posts
    }
}

/// The folder a single pushed file is in.
fn include_root_parent(file: &Path) -> PathBuf {
    file.parent()
        .filter(|p| !p.as_os_str().is_empty())
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
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
    fn watching_pushes_changed_posts_or_everything_for_a_snippet() {
        let dir = std::env::temp_dir().join(format!("gctl-watch-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join(".git")).unwrap();
        for f in ["a.md", "b.md", "_snip.md", ".git/x.md"] {
            fs::write(dir.join(f), "").unwrap();
        }
        let at = |secs| SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(secs);
        let times = |a, b, snip| {
            HashMap::from([
                (dir.join("a.md"), at(a)),
                (dir.join("b.md"), at(b)),
                (dir.join("_snip.md"), at(snip)),
            ])
        };
        let names = |files: Vec<PathBuf>| -> Vec<String> {
            files.iter().map(|p| display_name(&dir, p)).collect()
        };
        let results = [
            (
                "nothing",
                changed_files(&times(1, 1, 1), &times(1, 1, 1), &dir),
            ),
            (
                "one post",
                changed_files(&times(1, 1, 1), &times(2, 1, 1), &dir),
            ),
            (
                "two posts",
                changed_files(&times(1, 1, 1), &times(2, 2, 1), &dir),
            ),
            (
                "a snippet",
                changed_files(&times(1, 1, 1), &times(1, 1, 2), &dir),
            ),
            (
                "both",
                changed_files(&times(1, 1, 1), &times(2, 1, 2), &dir),
            ),
        ]
        .map(|(name, files)| (name, names(files)));
        let seen = snapshot(&dir).len();
        fs::remove_dir_all(&dir).unwrap();
        insta::assert_debug_snapshot!((results, seen));
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
