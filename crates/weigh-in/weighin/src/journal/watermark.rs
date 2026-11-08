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
//! A missing or torn file (cut short, or not UTF-8), or one written for another owner (another
//! store at this path, one recreated since), reads as no watermark: the next checkpoint examines
//! every journal. A file that exists but cannot be read, or is not a regular file (a symlink, a
//! directory; HK-6), is an error, and is not written over.

use std::fmt::Display;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
use redkiln_core::ids::Digest;

use crate::env::Env;
use crate::state_file::{Presence, regular_or_absent};

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
        let unreadable = |e: anyhow::Error| {
            e.context(format!(
                "reading the export watermark in {}, so every Stop examines every journal until \
                 it can be read",
                self.path.display()
            ))
        };
        // A symlink or anything else that is not a regular file is not read (HK-6).
        if regular_or_absent(&self.path).map_err(unreadable)? == Presence::Absent {
            return Ok(0);
        }
        // Bytes, not text: a file that is not UTF-8 is torn, as one cut short is.
        match std::fs::read(&self.path) {
            Ok(bytes) => Ok(std::str::from_utf8(&bytes)
                .ok()
                .and_then(|text| position(text, &self.owner))
                .unwrap_or(0)),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(0),
            Err(e) => Err(unreadable(anyhow::Error::new(e))),
        }
    }

    /// Records `through` for this repository and store.
    ///
    /// # Errors
    /// The state dir or the file cannot be written, or something other than a regular file is
    /// at its path (HK-6).
    pub(crate) fn write(&self, through: u64) -> Result<()> {
        if let Some(dir) = self.path.parent() {
            std::fs::create_dir_all(dir).with_context(|| format!("creating {}", dir.display()))?;
        }
        regular_or_absent(&self.path).with_context(|| {
            format!("recording the export watermark in {}", self.path.display())
        })?;
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
    use super::{Watermark, position};

    const OWNER: &str = "/r/.git\nstore-a";

    #[test]
    fn a_watermark_file_that_is_not_utf8_reads_as_torn() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("journal-watermark-x");
        std::fs::write(&path, b"42\n/r/.git\nstore-\xff\n").expect("written");
        let watermark = Watermark {
            path,
            owner: OWNER.to_owned(),
        };

        assert_eq!(watermark.read().expect("torn, not an error"), 0);
    }

    #[cfg(unix)]
    #[test]
    #[cfg_attr(miri, ignore = "makes a symlink through the file system")]
    fn a_symlinked_watermark_reads_as_an_error_and_is_not_written() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("elsewhere");
        std::fs::write(&target, format!("42\n{OWNER}\n")).expect("target written");
        let path = dir.path().join("journal-watermark-x");
        std::os::unix::fs::symlink(&target, &path).expect("symlink");
        let watermark = Watermark {
            path,
            owner: OWNER.to_owned(),
        };

        let read = watermark.read();
        let written = watermark.write(43);

        assert!(
            read.as_ref()
                .is_err_and(|e| format!("{e:#}").contains("not a regular file")),
            "{read:?}"
        );
        assert!(
            written
                .as_ref()
                .is_err_and(|e| format!("{e:#}").contains("not a regular file")),
            "{written:?}"
        );
        assert_eq!(
            std::fs::read_to_string(&target).expect("target readable"),
            format!("42\n{OWNER}\n"),
            "the link's target is left alone"
        );
    }

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
