use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use console::style;
use gctl_ghost::GhostContentApi;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::info;

use crate::error::GctlCliError;

/// File in the pushed directory that remembers where images were uploaded.
/// Ghost stores a new copy on every upload, so without it each push of an
/// unchanged post would add duplicates. Hidden, so `push` doesn't read it as a
/// post; safe to commit or delete.
pub const CACHE_FILE: &str = ".gctl-images.json";

#[derive(Default, Serialize, Deserialize)]
struct CacheFile {
    /// The site the URLs belong to; a cache from another site is ignored.
    site: String,
    /// SHA-256 of an image's bytes -> its URL on Ghost.
    images: HashMap<String, String>,
}

/// Local images and where they live on Ghost.
pub struct ImageStore {
    path: PathBuf,
    cache: CacheFile,
    dirty: bool,
}

impl ImageStore {
    pub fn load(dir: &Path, site: &str) -> ImageStore {
        let path = dir.join(CACHE_FILE);
        let cache = fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<CacheFile>(&text).ok())
            .filter(|cache| cache.site == site)
            .unwrap_or_else(|| CacheFile {
                site: site.to_string(),
                images: HashMap::new(),
            });
        ImageStore {
            path,
            cache,
            dirty: false,
        }
    }

    pub fn save(&self) -> Result<(), GctlCliError> {
        if self.dirty {
            fs::write(
                &self.path,
                serde_json::to_string_pretty(&self.cache)? + "\n",
            )?;
        }
        Ok(())
    }

    /// Uploads the local images `content` refers to (in the body and as
    /// `image:`), unless Ghost already has them. Returns the `src` as written
    /// -> URL on Ghost, for [`crate::markdown::compile_markdown_with`]. With
    /// `dry_run` nothing is uploaded: images that aren't on Ghost yet are left
    /// out, but missing files are still an error.
    pub async fn upload_for(
        &mut self,
        api: &GhostContentApi<'_>,
        file: &Path,
        content: &str,
        dry_run: bool,
    ) -> Result<HashMap<String, String>, GctlCliError> {
        let mut document = gctl_markdown::parse(content)?;
        let mut srcs = Vec::new();
        document.visit_images_mut(&mut |src| srcs.push(src.clone()));
        srcs.extend(document.front_matter.feature_image.clone());

        let dir = file.parent().unwrap_or(Path::new("."));
        let mut urls = HashMap::new();
        for src in srcs {
            if urls.contains_key(&src) || !is_local(&src) {
                continue;
            }
            let path = dir.join(path_part(&src));
            let bytes = fs::read(&path).map_err(|e| {
                GctlCliError::Parse(format!(
                    "image `{src}`: cannot read {}: {e}",
                    path.display()
                ))
            })?;
            let hash = format!("{:x}", Sha256::digest(&bytes));
            if let Some(url) = self.cache.images.get(&hash) {
                urls.insert(src, url.clone());
            } else if !dry_run {
                info!("{} {}", style("Uploading").green(), path.display());
                let uploaded = api
                    .images()
                    .upload(&path, Some("image".into()), None)
                    .await?;
                let url = uploaded.into_iter().next().map(|i| i.url).ok_or_else(|| {
                    GctlCliError::Parse(format!("Ghost returned no URL for `{src}`"))
                })?;
                self.cache.images.insert(hash, url.clone());
                self.dirty = true;
                urls.insert(src, url);
            }
        }
        Ok(urls)
    }
}

/// Whether an image `src` names a file next to the post, rather than a URL or
/// a path on the Ghost site.
fn is_local(src: &str) -> bool {
    let has_scheme = src.split_once(':').is_some_and(|(scheme, _)| {
        scheme
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
    });
    !src.is_empty() && !has_scheme && !src.starts_with('/') && !src.starts_with('#')
}

/// `img/a.png?raw=1#x` -> `img/a.png`
fn path_part(src: &str) -> &str {
    src.split(['?', '#']).next().unwrap_or(src)
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn tells_local_images_from_urls() {
        let results: Vec<_> = [
            "img/a.png",
            "./a.png",
            "../shared/a.png",
            "a b.png",
            "https://example.com/a.png",
            "//cdn.example.com/a.png",
            "data:image/png;base64,AAAA",
            "/content/images/a.png",
            "C:/a.png",
            "",
        ]
        .iter()
        .map(|src| (*src, is_local(src)))
        .collect();
        insta::assert_debug_snapshot!(results);
    }

    #[test]
    fn cache_is_tied_to_the_site() {
        let dir = std::env::temp_dir().join(format!("gctl-images-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let mut store = ImageStore::load(&dir, "https://a.test");
        store
            .cache
            .images
            .insert("abc".into(), "https://a.test/x.png".into());
        store.dirty = true;
        store.save().unwrap();

        let same = ImageStore::load(&dir, "https://a.test").cache.images.len();
        let other = ImageStore::load(&dir, "https://b.test").cache.images.len();
        fs::remove_dir_all(&dir).unwrap();
        assert_eq!((same, other), (1, 0));
    }
}
