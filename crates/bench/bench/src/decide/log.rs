//! Clef's decision log as Bench keeps it, in the user scope.
//!
//! It is `redkiln_calibrate`'s rotating log in Bench's folder of the user scope,
//! `~/.redkiln/calibrate/bench/` (`$REDKILN_HOME`), with anneal's old `~/.anneal/decisions/`
//! (`$ANNEAL_HOME`) read alongside and never written or deleted (Ryan's D1, option A).
//!
//! The user scope is outside the plugin's directories, so the log survives a reinstall
//! (`CLAUDE_PLUGIN_DATA` is deleted on uninstall). Rows are appended only to the new folder;
//! [`DecisionLog::logs`] lists the old log first, so a label in either matches a decision in
//! either.
//!
//! How many answered decisions each point has had is kept in `counts.json` beside the log
//! (`decide::counts`), so the 50-decision shadow period survives rotation. It is read from the
//! new folder, else from the old one, and written only to the new one, so a point keeps its
//! count across the move. Two decisions at once can lose one increment, which only lengthens
//! the shadow period.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use redkiln_calibrate::log as calibrate_log;
use redkiln_calibrate::scope::{DecisionDirs, ScopeError, UserScope};
use serde::Serialize;

use crate::baseline::BaselineError;
use crate::decide::counts::{self, COUNTS_KIND, COUNTS_VERSION};
use crate::decide::point::Point;

pub use redkiln_calibrate::log::{LOG_FILE, MAX_FILE_BYTES, MAX_ROW_BYTES, ROTATED_FILES};

/// Bench's producer name in the user scope: its rows live in `calibrate/bench/`.
pub const PRODUCER: &str = "bench";

/// Why a row could not be appended or the counts kept. The first four are
/// [`calibrate_log::LogError`]'s, with its messages.
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
    /// The counts file could not be read or written.
    #[error("decision counts")]
    Counts(#[source] BaselineError),
    /// The counts file is not JSON of the expected shape.
    #[error("{path} is not a decision counts file", path = path.display())]
    CountsFormat {
        /// The file.
        path: PathBuf,
        /// What did not parse.
        #[source]
        source: serde_json::Error,
    },
    /// The counts file belongs to something else, or to another version.
    #[error("{path} is not {COUNTS_KIND} version {COUNTS_VERSION}", path = path.display())]
    CountsForeign {
        /// The file.
        path: PathBuf,
    },
}

impl From<calibrate_log::LogError> for LogError {
    fn from(error: calibrate_log::LogError) -> Self {
        match error {
            calibrate_log::LogError::Io { op, path, source } => Self::Io { op, path, source },
            calibrate_log::LogError::NotAFile { path } => Self::NotAFile { path },
            calibrate_log::LogError::RowTooLarge { bytes } => Self::RowTooLarge { bytes },
            calibrate_log::LogError::Encode(source) => Self::Encode(source),
        }
    }
}

/// Bench's decision log: the one it writes, and anneal's old one read alongside.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionLog {
    log: calibrate_log::DecisionLog,
    legacy: Option<calibrate_log::DecisionLog>,
}

/// Bench's decision log in the user scope `lookup` names.
///
/// # Errors
/// [`ScopeError`] when no home directory is set, or the one chosen (or `$ANNEAL_HOME`) is
/// relative.
pub fn open(lookup: impl Fn(&str) -> Option<OsString>) -> Result<DecisionLog, ScopeError> {
    let dirs = UserScope::from_lookup(lookup)?.decisions(PRODUCER)?;
    Ok(DecisionLog::from_dirs(dirs))
}

impl DecisionLog {
    /// The log in `dirs.current`, reading `dirs.legacy` alongside.
    #[must_use]
    pub fn from_dirs(dirs: DecisionDirs) -> Self {
        Self {
            log: calibrate_log::DecisionLog::new(dirs.current),
            legacy: dirs.legacy.map(calibrate_log::DecisionLog::new),
        }
    }

    /// The log in the anneal home `home` (`<home>/decisions`), with no old log beside it.
    #[cfg(test)]
    pub(crate) fn in_home(home: &Path) -> Self {
        Self {
            log: calibrate_log::DecisionLog::in_home(home),
            legacy: None,
        }
    }

    /// The folder written: the log, `counts.json`, `proposals.json` and hook state.
    #[must_use]
    pub fn dir(&self) -> &Path {
        self.log.dir()
    }

    /// The old folder read alongside, when there is one.
    #[must_use]
    pub fn legacy_dir(&self) -> Option<&Path> {
        self.legacy.as_ref().map(calibrate_log::DecisionLog::dir)
    }

    /// The live log file.
    #[must_use]
    pub fn path(&self) -> PathBuf {
        self.log.path()
    }

    /// The logs to read rows from, oldest first: the old one, then the one written.
    pub fn logs(&self) -> impl Iterator<Item = &calibrate_log::DecisionLog> + '_ {
        self.legacy.iter().chain(std::iter::once(&self.log))
    }

    /// Where to read the state file `name` from: the folder written when anything is at
    /// `<dir>/<name>` (or it cannot be told), else the old folder when the file is there, else
    /// `<dir>/<name>`, whose absence the reader reports.
    #[must_use]
    pub fn file_to_read(&self, name: &str) -> PathBuf {
        let current = self.dir().join(name);
        let absent = |path: &Path| {
            fs::symlink_metadata(path).is_err_and(|error| error.kind() == io::ErrorKind::NotFound)
        };
        if !absent(&current) {
            return current;
        }
        self.legacy_dir()
            .map(|legacy| legacy.join(name))
            .filter(|old| !absent(old))
            .unwrap_or(current)
    }

    /// The `bench: warning:` message for a run that reads the old folder: `Some` when it exists
    /// as a directory, `None` otherwise.
    #[must_use]
    pub fn legacy_warning(&self) -> Option<String> {
        let legacy = self.legacy_dir()?;
        fs::metadata(legacy)
            .is_ok_and(|metadata| metadata.is_dir())
            .then(|| {
                format!(
                    "reading the old decision log at {} too; new rows go to {}, and the old \
                     folder is never written or deleted",
                    legacy.display(),
                    self.dir().display()
                )
            })
    }

    /// Appends `row` as one JSON line to the folder written, rotating first when the file
    /// would pass its cap.
    ///
    /// # Errors
    /// [`LogError::RowTooLarge`] for a row over [`MAX_ROW_BYTES`] (nothing is written);
    /// [`LogError::NotAFile`] when something other than a file is at the log's path;
    /// [`LogError::Io`] when the directory or file cannot be written.
    pub fn append<T: Serialize>(&self, row: &T) -> Result<(), LogError> {
        Ok(self.log.append(row)?)
    }

    /// How many answered decisions `point` has had.
    ///
    /// # Errors
    /// [`LogError::Counts`] when the counts file cannot be read or is not a regular file;
    /// [`LogError::CountsFormat`] or [`LogError::CountsForeign`] when it is not ours.
    pub fn answered(&self, point: Point) -> Result<u64, LogError> {
        Ok(self.load_counts()?.get(&point).copied().unwrap_or(0))
    }

    /// How many answered decisions each point has had, read once; a point with none is absent.
    ///
    /// # Errors
    /// As [`DecisionLog::answered`].
    pub(crate) fn answered_by_point(&self) -> Result<BTreeMap<Point, u64>, LogError> {
        self.load_counts()
    }

    /// Counts one more answered decision of `point` in the folder written, and returns the new
    /// count.
    ///
    /// # Errors
    /// As [`DecisionLog::answered`], and [`LogError::Counts`] when the file cannot be written.
    pub fn count_answered(&self, point: Point) -> Result<u64, LogError> {
        let mut answered = self.load_counts()?;
        let count = answered.entry(point).or_insert(0);
        *count = count.saturating_add(1);
        let total = *count;
        counts::write(&self.counts_path(), answered)?;
        Ok(total)
    }

    /// The counts file in the folder written.
    fn counts_path(&self) -> PathBuf {
        counts::path(self.dir())
    }

    /// The counts of the folder written, else of the old one, else none.
    fn load_counts(&self) -> Result<BTreeMap<Point, u64>, LogError> {
        if let Some(answered) = counts::read(&self.counts_path())? {
            return Ok(answered);
        }
        match self.legacy_dir() {
            Some(legacy) => Ok(counts::read(&counts::path(legacy))?.unwrap_or_default()),
            None => Ok(BTreeMap::new()),
        }
    }
}

#[cfg(test)]
mod tests;
