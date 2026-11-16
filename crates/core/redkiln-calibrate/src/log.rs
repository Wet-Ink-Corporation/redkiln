//! The decision log: `<dir>/decisions.jsonl`, one JSON row per decision or label,
//! append-only, in the directory its producer's [`crate::scope::UserScope`] names.
//!
//! Bounds (H-13): a row is at most [`MAX_ROW_BYTES`]; a file that would pass
//! [`MAX_FILE_BYTES`] is renamed to `decisions.1.jsonl` (the older ones moving up to
//! `decisions.4.jsonl`, and the fifth dropped), so the log holds at most 80 MiB. Each row is one
//! `write` of the whole line to a file opened for appending, so concurrent writers do not
//! interleave rows. The rows themselves are the producer's; [`crate::rows`] reads anneal's v1
//! decision and label rows.

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;

/// The log's directory inside an anneal home (the v1 layout, `~/.anneal/decisions`).
pub(crate) const LOG_DIR: &str = "decisions";

/// The live log file.
pub const LOG_FILE: &str = "decisions.jsonl";

/// Most bytes in one row, newline included.
pub const MAX_ROW_BYTES: usize = 8 * 1024;

/// The size past which the live file is rotated.
pub const MAX_FILE_BYTES: u64 = 16 * 1024 * 1024;

/// How many rotated files are kept.
pub const ROTATED_FILES: u32 = 4;

/// Why a row could not be appended.
#[derive(Debug, thiserror::Error)]
pub enum LogError {
    /// A file or directory could not be read, created, renamed or written.
    #[error("{op} {path}", path = path.display())]
    Io {
        /// What was being done.
        op: &'static str,
        /// The path.
        path: PathBuf,
        /// The failure.
        #[source]
        source: io::Error,
    },
    /// Something other than a regular file is at the log's path.
    #[error("{path} is not a regular file", path = path.display())]
    NotAFile {
        /// The path.
        path: PathBuf,
    },
    /// The row is over [`MAX_ROW_BYTES`].
    #[error("a decision row of {bytes} bytes is over {MAX_ROW_BYTES}")]
    RowTooLarge {
        /// The row's size.
        bytes: usize,
    },
    /// The row could not be encoded.
    #[error("encoding a decision row")]
    Encode(#[source] serde_json::Error),
}

/// The decision log in one directory.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionLog {
    dir: PathBuf,
    max_file_bytes: u64,
}

impl DecisionLog {
    /// The log in `dir`.
    #[must_use]
    pub const fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            max_file_bytes: MAX_FILE_BYTES,
        }
    }

    /// The log in the anneal home `home`: `<home>/decisions`, anneal's v1 layout, which a
    /// producer reads alongside its own (D1) and never writes. Tests only: a producer gets its
    /// folders from [`crate::scope::UserScope::decisions`].
    #[cfg(test)]
    pub(crate) fn in_home(home: &Path) -> Self {
        Self::new(home.join(LOG_DIR))
    }

    /// The log's directory.
    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The live log file.
    #[must_use]
    pub fn path(&self) -> PathBuf {
        self.dir.join(LOG_FILE)
    }

    /// Appends `row` as one JSON line, rotating the file first when it would pass its cap.
    ///
    /// # Errors
    /// [`LogError::RowTooLarge`] for a row over [`MAX_ROW_BYTES`] (nothing is written);
    /// [`LogError::NotAFile`] when something other than a file is at the log's path;
    /// [`LogError::Io`] when the directory or file cannot be written.
    pub fn append<T: Serialize>(&self, row: &T) -> Result<(), LogError> {
        let mut line = serde_json::to_vec(row).map_err(LogError::Encode)?;
        line.push(b'\n');
        if line.len() > MAX_ROW_BYTES {
            return Err(LogError::RowTooLarge { bytes: line.len() });
        }
        fs::create_dir_all(&self.dir).map_err(|source| io_error("creating", &self.dir, source))?;
        let path = self.path();
        let row_bytes = u64::try_from(line.len()).unwrap_or(u64::MAX);
        match fs::symlink_metadata(&path) {
            Ok(metadata) if !metadata.is_file() => return Err(LogError::NotAFile { path }),
            Ok(metadata) if metadata.len().saturating_add(row_bytes) > self.max_file_bytes => {
                self.rotate()?;
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(source) => return Err(io_error("reading", &path, source)),
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .map_err(|source| io_error("opening", &path, source))?;
        file.write_all(&line)
            .map_err(|source| io_error("appending to", &path, source))
    }

    /// Every file the log can have, oldest first: `decisions.4.jsonl` down to
    /// `decisions.1.jsonl`, then the live file. Some may not exist.
    pub fn files(&self) -> impl Iterator<Item = PathBuf> + '_ {
        (1..=ROTATED_FILES)
            .rev()
            .map(|generation| self.rotated(generation))
            .chain(std::iter::once(self.path()))
    }

    /// The `generation`th rotated file: `decisions.<generation>.jsonl`.
    fn rotated(&self, generation: u32) -> PathBuf {
        self.dir.join(format!("decisions.{generation}.jsonl"))
    }

    /// Moves each rotated file up one generation (the oldest is overwritten), then the live
    /// file to generation 1.
    fn rotate(&self) -> Result<(), LogError> {
        for generation in (1..ROTATED_FILES).rev() {
            let from = self.rotated(generation);
            match fs::rename(&from, self.rotated(generation.saturating_add(1))) {
                Ok(()) => {}
                // A generation not written yet: nothing to move.
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(source) => return Err(io_error("rotating", &from, source)),
            }
        }
        let path = self.path();
        match fs::rename(&path, self.rotated(1)) {
            Ok(()) => Ok(()),
            // Another call rotated the live file first; the append creates a new one.
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(io_error("rotating", &path, source)),
        }
    }
}

fn io_error(op: &'static str, path: &Path, source: io::Error) -> LogError {
    LogError::Io {
        op,
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests;
