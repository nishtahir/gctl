use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::GctlCliError;

/// File in the content directory that remembers what each post looked like on
/// Ghost when it was last pulled or pushed. It lets `gctl push` notice that a
/// post was edited in Ghost since, instead of overwriting that edit. Hidden, so
/// it isn't read as a post. It describes this checkout only: don't commit it.
pub const STATE_FILE: &str = ".gctl-state.json";

#[derive(Default, Serialize, Deserialize)]
struct StateFile {
    /// The site the ids belong to; state from another site is ignored.
    site: String,
    /// Post id -> its `updated_at` when last synced.
    updated_at: HashMap<String, String>,
}

pub struct SyncState {
    path: PathBuf,
    state: StateFile,
    dirty: bool,
}

impl SyncState {
    pub fn load(dir: &Path, site: &str) -> SyncState {
        let path = dir.join(STATE_FILE);
        let state = fs::read_to_string(&path)
            .ok()
            .and_then(|text| serde_json::from_str::<StateFile>(&text).ok())
            .filter(|state| state.site == site)
            .unwrap_or_else(|| StateFile {
                site: site.to_string(),
                updated_at: HashMap::new(),
            });
        SyncState {
            path,
            state,
            dirty: false,
        }
    }

    /// When the post was last edited on Ghost, as of the last sync.
    pub fn get(&self, id: &str) -> Option<&str> {
        self.state.updated_at.get(id).map(String::as_str)
    }

    /// Notes that the post is in sync as of `updated_at`.
    pub fn record(&mut self, id: Option<&str>, updated_at: Option<&str>) {
        let (Some(id), Some(updated_at)) = (id, updated_at) else {
            return;
        };
        if self.get(id) != Some(updated_at) {
            self.state
                .updated_at
                .insert(id.to_string(), updated_at.to_string());
            self.dirty = true;
        }
    }

    pub fn save(&self) -> Result<(), GctlCliError> {
        if self.dirty {
            fs::write(
                &self.path,
                serde_json::to_string_pretty(&self.state)? + "\n",
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn remembers_per_site() {
        let dir = std::env::temp_dir().join(format!("gctl-state-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        let mut state = SyncState::load(&dir, "https://a.test");
        state.record(Some("p1"), Some("2024-01-01T00:00:00.000Z"));
        state.record(None, Some("ignored"));
        state.record(Some("p2"), None);
        state.save().unwrap();

        let same = SyncState::load(&dir, "https://a.test");
        let other = SyncState::load(&dir, "https://b.test");
        let results = (
            same.get("p1").map(str::to_string),
            same.get("p2").map(str::to_string),
            other.get("p1").map(str::to_string),
        );
        fs::remove_dir_all(&dir).unwrap();
        insta::assert_debug_snapshot!(results);
    }
}
