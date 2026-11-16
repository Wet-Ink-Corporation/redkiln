//! Bench's folder in a worktree, `.redkiln/bench/` (blueprint, Repo config: `.redkiln/<plugin>/`),
//! and anneal's `.anneal/` before it, which Bench reads as a 0.x fallback and never writes or
//! deletes (Move 2 step 5, PR 5.6).
//!
//! Each file Bench reads from the folder is a [`RepoFile`]: its path in `.redkiln/bench/` and
//! in `.anneal/`. [`RepoFile::choose`] picks the one to read: the new one when anything is at
//! its path (so a reader refuses a directory or symlink there rather than falling back past
//! it), else the old one when anything is at that path, else the new one (absent). Reading the
//! old one, or finding both, is a [`Notice`]: a command writes it as one `bench: warning:` line
//! ([`warn`]); hooks stay silent.

use std::fmt;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// anneal's folder, read as a 0.x fallback.
pub(crate) const LEGACY_DIR: &str = ".anneal";

/// A file of Bench's folder: where Bench reads it, and where anneal kept it, both relative to
/// the worktree root and `/`-separated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RepoFile {
    /// In `.redkiln/bench/`.
    pub(crate) path: &'static str,
    /// In `.anneal/`.
    pub(crate) legacy: &'static str,
}

/// The repository's settings: the merge gate's, `check_fast`, the orchestrate leads.
pub(crate) const CONFIG: RepoFile = RepoFile {
    path: ".redkiln/bench/config.json",
    legacy: ".anneal/config.json",
};

/// `floor-guard`'s ratchets.
pub(crate) const RATCHETS: RepoFile = RepoFile {
    path: ".redkiln/bench/ratchets.json",
    legacy: ".anneal/ratchets.json",
};

/// The standing orders every subagent gets.
pub(crate) const STANDING_ORDERS: RepoFile = RepoFile {
    path: ".redkiln/bench/standing-orders.md",
    legacy: ".anneal/standing-orders.md",
};

/// Which of a file's two paths to read, and what to warn about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Chosen {
    /// The path to read, relative to the worktree root.
    pub(crate) path: &'static str,
    /// The old file read, or found beside the new one.
    pub(crate) notice: Option<Notice>,
}

/// What a command warns about a file of anneal's old folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Notice {
    /// Only the old file exists, and is read.
    Fallback(RepoFile),
    /// Both exist: the new one is read and the old one ignored.
    Ignored(RepoFile),
}

impl fmt::Display for Notice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fallback(file) => write!(
                f,
                "{} is read because {} does not exist; bench reads anneal's {LEGACY_DIR}/ only \
                 as a 0.x fallback, so move the file there",
                file.legacy, file.path
            ),
            Self::Ignored(file) => {
                write!(f, "{} is ignored: bench reads {}", file.legacy, file.path)
            }
        }
    }
}

impl RepoFile {
    /// Which of the two paths to read in the worktree `root` (see the module docs).
    pub(crate) fn choose(self, root: &Path) -> Chosen {
        let current = is_present(&under(root, self.path));
        let legacy = is_present(&under(root, self.legacy));
        match (current, legacy) {
            (true, true) => Chosen {
                path: self.path,
                notice: Some(Notice::Ignored(self)),
            },
            (false, true) => Chosen {
                path: self.legacy,
                notice: Some(Notice::Fallback(self)),
            },
            (true | false, false) => Chosen {
                path: self.path,
                notice: None,
            },
        }
    }
}

/// The notices for `files` in the worktree `root`, in order.
pub(crate) fn notices(root: &Path, files: &[RepoFile]) -> Vec<Notice> {
    files
        .iter()
        .filter_map(|file| file.choose(root).notice)
        .collect()
}

/// Writes each notice for `files` in the worktree `root` to `err` as one `bench: warning:`
/// line (SPEC CLI-08).
///
/// # Errors
/// The I/O error when `err` cannot be written.
pub(crate) fn warn<W: Write>(root: &Path, files: &[RepoFile], mut err: W) -> io::Result<()> {
    for notice in notices(root, files) {
        crate::diagnostic::warning(&mut err, &notice)?;
    }
    Ok(())
}

/// Whether anything is at `path`, a symlink included (not followed). A path whose metadata
/// cannot be read counts as present, so its reader reports why rather than reading past it.
fn is_present(path: &Path) -> bool {
    match fs::symlink_metadata(path) {
        Ok(_) => true,
        Err(error) => error.kind() != io::ErrorKind::NotFound,
    }
}

/// `relative` (`/`-separated) under `root`.
pub(crate) fn under(root: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(root.to_path_buf(), |path, part| path.join(part))
}

#[cfg(test)]
mod tests;
