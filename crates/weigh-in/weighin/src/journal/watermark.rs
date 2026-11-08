//! How far the store's events are exported: the store position up to which a checkpoint
//! settled the store (Core's `ScopedCheckpoint::through`), kept in the state dir (D4) as
//! `journal-watermark`, so the Stop hook examines only the journals with a later event instead
//! of every journal the store ever held. Every journal whose last event is at or before it was
//! in its journal ref or inbox, as far as this clone could tell then. It only moves forward:
//! what a sweep finds below it is reported by the sweep, not handed back to the Stop.
//!
//! Each repository sharing a store keeps its own file, `journal-watermark-<hex>`, named by the
//! sha256 of its owner (the repository and the store), so no repository's checkpoint resets
//! another's. The file holds the position, the repository and the store it was written for, one
//! per line. It is overwritten in place, never deleted (ST-05, B-01), as `hook-errors.seen` is.
//! A missing or torn file, or one written for another owner (another store at this path, one
//! recreated since), reads as no watermark: the next checkpoint examines every journal. A file
//! that exists but cannot be read is an error, and is not written over.

use std::fmt::Display;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use redkiln_core::ids::Digest;

use crate::env::Env;

/// The start of the file's name in the state dir; the owner's digest follows.
const FILE_PREFIX: &str = "journal-watermark-";

/// The watermark of one store for one repository.
#[derive(Debug)]
pub(crate) struct Watermark {
    path: PathBuf,
    /// Whose it is, as the file's last two lines hold it: the repository's git common dir (its
    /// worktrees share refs), else `repo` as given, then the store's id.
    owner: String,
}

impl Watermark {
    /// The watermark of `env`'s store, whose id is `store`, for the repository at `repo`.
    ///
    /// # Errors
    /// The store could not be located, so there is no state dir.
    pub(crate) fn of(env: &Env, repo: &Path, store: impl Display) -> Result<Self> {
        let repo = env.common_dir.as_deref().unwrap_or(repo);
        let owner = format!("{}\n{store}", repo.display());
        let file = format!("{FILE_PREFIX}{}", Digest::sha256(owner.as_bytes()).hex());
        Ok(Self {
            path: env.state_dir()?.join(file),
            owner,
        })
    }

    /// The position recorded for this repository and store: 0, which examines every journal,
    /// when there is no file, or it is torn or another's (module doc).
    ///
    /// # Errors
    /// The file exists but cannot be read; the caller then examines every journal and must not
    /// write, since it cannot tell whether that would move the watermark back.
    pub(crate) fn read(&self) -> Result<u64> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => Ok(position(&text, &self.owner).unwrap_or(0)),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(0),
            Err(e) => Err(anyhow::Error::new(e).context(format!(
                "reading the export watermark in {}, so every Stop examines every journal until \
                 it can be read",
                self.path.display()
            ))),
        }
    }

    /// Records `through` for this repository and store.
    ///
    /// # Errors
    /// The state dir or the file cannot be written.
    pub(crate) fn write(&self, through: u64) -> Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        std::fs::write(&self.path, format!("{through}\n{}\n", self.owner))
            .with_context(|| format!("recording the export watermark in {}", self.path.display()))
    }
}

/// The position `text` records for `owner`, or `None` when it is torn or another's.
fn position(text: &str, owner: &str) -> Option<u64> {
    let (position, rest) = text.split_once('\n')?;
    if rest.strip_suffix('\n')? != owner {
        return None;
    }
    position.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::position;

    const OWNER: &str = "/r/.git\nstore-a";

    #[test]
    fn a_watermark_reads_only_whole_and_for_its_own_repository_and_store() {
        assert_eq!(position("42\n/r/.git\nstore-a\n", OWNER), Some(42));
        assert_eq!(
            position("42\n/other/.git\nstore-a\n", OWNER),
            None,
            "another repository's"
        );
        assert_eq!(
            position("42\n/r/.git\nstore-b\n", OWNER),
            None,
            "another store's"
        );
        assert_eq!(position("42\n/r/.git\n", OWNER), None, "no store");
        assert_eq!(position("42\n/r/.git\nstore-a", OWNER), None, "torn");
        assert_eq!(
            position("4x\n/r/.git\nstore-a\n", OWNER),
            None,
            "not a position"
        );
        assert_eq!(position("", OWNER), None, "empty");
    }
}
