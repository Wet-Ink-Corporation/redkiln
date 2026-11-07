//! How far the store's events are exported: the last store position read by a checkpoint that
//! deferred and broke nothing, kept in the state dir (D4) as `journal-watermark`, so the Stop
//! hook examines only the journals with a later event instead of every journal the store ever
//! held (PR B review F1). Every event at or before it is in its journal ref or inbox, as far as
//! this clone could tell when it was written.
//!
//! The file holds the position and the repository it was written for, one per line. It is
//! overwritten in place, never deleted (ST-05, B-01), as `hook-errors.seen` is. A missing,
//! unreadable or torn file, or one written for another repository sharing the store, reads as
//! no watermark: the next checkpoint examines every journal.

use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};

use crate::env::Env;

/// The file's name in the state dir.
const FILE: &str = "journal-watermark";

/// The watermark of one store for one repository.
#[derive(Debug)]
pub(crate) struct Watermark {
    path: PathBuf,
    /// The repository's git common dir (its worktrees share refs), else `repo` as given.
    repo: String,
}

impl Watermark {
    /// The watermark of `env`'s store for the repository at `repo`.
    ///
    /// # Errors
    /// The store could not be located, so there is no state dir.
    pub(crate) fn of(env: &Env, repo: &Path) -> Result<Self> {
        let repo = env.common_dir.as_deref().unwrap_or(repo);
        Ok(Self {
            path: env.state_dir()?.join(FILE),
            repo: repo.display().to_string(),
        })
    }

    /// The position recorded for this repository; 0, which examines every journal, when none
    /// can be read (module doc).
    pub(crate) fn read(&self) -> u64 {
        std::fs::read_to_string(&self.path)
            .ok()
            .and_then(|text| position(&text, &self.repo))
            .unwrap_or(0)
    }

    /// Records `through` for this repository.
    ///
    /// # Errors
    /// The state dir or the file cannot be written.
    pub(crate) fn write(&self, through: u64) -> Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        std::fs::write(&self.path, format!("{through}\n{}\n", self.repo))
            .with_context(|| format!("recording the export watermark in {}", self.path.display()))
    }
}

/// The position `text` records for `repo`, or `None` when it is torn or another repository's.
fn position(text: &str, repo: &str) -> Option<u64> {
    let (position, rest) = text.split_once('\n')?;
    if rest.strip_suffix('\n')? != repo {
        return None;
    }
    position.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::position;

    #[test]
    fn a_watermark_reads_only_whole_and_for_its_own_repository() {
        assert_eq!(position("42\n/r/.git\n", "/r/.git"), Some(42));
        assert_eq!(
            position("42\n/other/.git\n", "/r/.git"),
            None,
            "another repository's"
        );
        assert_eq!(position("42\n/r/.g", "/r/.git"), None, "torn");
        assert_eq!(position("4x\n/r/.git\n", "/r/.git"), None, "not a position");
        assert_eq!(position("", "/r/.git"), None, "empty");
    }
}
