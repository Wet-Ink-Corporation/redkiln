//! Reading the files `lint skills` and the evals are given: the read cap, bounded reads, the
//! usage checks on command-line paths, and [`LintError`], why a lint could not run. A copy of
//! Bench's `lint::read`, which Bench's other lints keep.

use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::baseline::BaselineError;
use crate::exit_codes::{self, Code};

/// Files read whole (`SKILL.md`, a `--hooks` file, the index, fixtures and suites) larger than
/// this are not read past it (H-13). For a skill, anything near it already breaks the 8 KB
/// budget.
pub(crate) const MAX_READ_BYTES: u64 = 1024 * 1024;

/// Why `lint skills` (or an eval reading skills) could not run.
#[derive(Debug, thiserror::Error)]
pub enum LintError {
    /// A path given on the command line does not exist.
    #[error("no such path: {path}", path = path.display())]
    MissingPath {
        /// The path as given.
        path: PathBuf,
    },
    /// A path given on the command line is the wrong kind (a file for a directory, or the
    /// reverse).
    #[error("{path} is not {expected}", path = path.display())]
    WrongKind {
        /// The path as given.
        path: PathBuf,
        /// What it should be.
        expected: &'static str,
    },
    /// A file or directory could not be read.
    #[error("reading {path}", path = path.display())]
    Io {
        /// The path being read.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: io::Error,
    },
    /// The content-hash snapshot could not be read or written.
    #[error("lint snapshot")]
    Cache(#[from] BaselineError),
}

impl LintError {
    pub(crate) fn io(path: &Path, source: io::Error) -> Self {
        Self::Io {
            path: path.to_path_buf(),
            source,
        }
    }

    /// The documented exit code: a missing path is a usage error, the rest internal.
    #[must_use]
    pub const fn code(&self) -> Code {
        match self {
            Self::MissingPath { .. } | Self::WrongKind { .. } => exit_codes::USAGE,
            Self::Io { .. } | Self::Cache(_) => exit_codes::INTERNAL,
        }
    }
}

/// A file named on the command line must exist and must not be a directory; either mistake is
/// a usage error.
fn require_file(path: &Path) -> Result<(), LintError> {
    if !path.exists() {
        return Err(LintError::MissingPath {
            path: path.to_path_buf(),
        });
    }
    if path.is_dir() {
        return Err(LintError::WrongKind {
            path: path.to_path_buf(),
            expected: "a file",
        });
    }
    Ok(())
}

/// Reads a text file given on the command line: its text, or `Err(size)` when it is larger
/// than the read cap. A missing file is a usage error. Invalid UTF-8 is replaced, which only
/// matters for word counts and JSON parse errors.
pub(crate) fn read_text(path: &Path) -> Result<Result<String, u64>, LintError> {
    require_file(path)?;
    let capped = read_capped(path)?;
    Ok(capped
        .bytes
        .map(|bytes| String::from_utf8_lossy(&bytes).into_owned())
        .ok_or(capped.size))
}

/// A file read up to [`MAX_READ_BYTES`]: its size and, when within the cap, its bytes.
#[derive(Debug)]
pub(crate) struct Capped {
    /// The file's size on disk; or, for a file that grew past the cap after its size was
    /// read, the bytes actually read (one past the cap).
    pub size: u64,
    /// The contents, or `None` when the file is larger than the cap.
    pub bytes: Option<Vec<u8>>,
}

/// Reads `path` up to [`MAX_READ_BYTES`].
///
/// # Errors
/// [`LintError::Io`] when the file cannot be opened or read.
pub(crate) fn read_capped(path: &Path) -> Result<Capped, LintError> {
    read_bounded(path).map_err(|source| LintError::io(path, source))
}

/// Reads `path` up to [`MAX_READ_BYTES`], for callers with their own error type.
///
/// # Errors
/// The I/O error when the file cannot be opened or read.
pub(crate) fn read_bounded(path: &Path) -> io::Result<Capped> {
    let file = File::open(path)?;
    let size = file.metadata()?.len();
    if size > MAX_READ_BYTES {
        return Ok(Capped { size, bytes: None });
    }
    let mut bytes = Vec::new();
    // One byte past the cap tells a file that grew since its size was read: it is too large,
    // never silently truncated.
    file.take(MAX_READ_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)?;
    let read = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
    if read > MAX_READ_BYTES {
        return Ok(Capped {
            size: read,
            bytes: None,
        });
    }
    Ok(Capped {
        size,
        bytes: Some(bytes),
    })
}

#[cfg(all(test, unix))]
mod tests {
    use std::path::Path;

    use super::{MAX_READ_BYTES, read_bounded};

    #[test]
    fn a_file_that_reads_past_its_reported_size_is_too_large_not_truncated() {
        // `/dev/zero` reports size 0 and never ends: the same as a file that grew after its
        // size was read.
        let capped = read_bounded(Path::new("/dev/zero")).expect("/dev/zero opens");

        assert!(capped.bytes.is_none(), "not read as content");
        assert_eq!(capped.size, MAX_READ_BYTES + 1, "the bytes actually read");
    }
}
