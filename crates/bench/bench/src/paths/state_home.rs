//! Where Bench keeps its per-user state (context snapshots, the monitor's levels, the
//! status-line hint marker, the handoff note records and the Stop-block counters).
//!
//! The folder is `<user scope>/bench/` (`$REDKILN_HOME/bench`, else `~/.redkiln/bench`), PR 5.6.
//! anneal kept these files in its home (`$ANNEAL_HOME`, else `~/.anneal`), which is read as a
//! fallback, file by file, and never written or deleted (D1 style).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use redkiln_calibrate::scope::{ScopeError, UserScope};

use crate::repo_file::is_present;

/// Bench's folder in the user scope's root.
pub(crate) const BENCH_DIR: &str = "bench";

/// A folder of state files: where a file is read from, and where it is written.
pub(crate) trait StateHome {
    /// The file `name` to read.
    fn read(&self, name: &str) -> PathBuf;
    /// The file `name` to write.
    fn written(&self, name: &str) -> PathBuf;
}

/// A plain folder reads and writes its files in itself.
impl StateHome for Path {
    fn read(&self, name: &str) -> PathBuf {
        self.join(name)
    }

    fn written(&self, name: &str) -> PathBuf {
        self.join(name)
    }
}

/// Bench's state folder in the user scope, with anneal's home read below it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BenchHome {
    current: PathBuf,
    legacy: Option<PathBuf>,
}

impl BenchHome {
    /// The folder the user scope `lookup` names; `lookup` reads one environment variable.
    ///
    /// # Errors
    /// [`ScopeError::NoHome`] when neither `REDKILN_HOME`, `HOME` nor `USERPROFILE` is set;
    /// [`ScopeError::RelativeHome`] when the root, or anneal's home, would be relative.
    pub(crate) fn from_lookup(
        lookup: impl Fn(&str) -> Option<OsString>,
    ) -> Result<Self, ScopeError> {
        let scope = UserScope::from_lookup(lookup)?;
        let current = scope.root().join(BENCH_DIR);
        let legacy = scope
            .legacy_home()
            .filter(|legacy| *legacy != current)
            .map(Path::to_path_buf);
        Ok(Self { current, legacy })
    }

    /// `<root>/bench`: the only folder written.
    #[cfg(test)]
    fn current(&self) -> &Path {
        &self.current
    }

    /// anneal's home, read when a file is absent from the current folder; `None` without one,
    /// or when it is the current folder itself.
    #[cfg(test)]
    fn legacy(&self) -> Option<&Path> {
        self.legacy.as_deref()
    }
}

impl StateHome for BenchHome {
    /// The current folder's file when anything is there, else anneal's when anything is there,
    /// else the current folder's (absent: read as empty).
    fn read(&self, name: &str) -> PathBuf {
        let current = self.current.join(name);
        if is_present(&current) {
            return current;
        }
        self.legacy
            .as_ref()
            .map(|legacy| legacy.join(name))
            .filter(|legacy| is_present(legacy))
            .unwrap_or(current)
    }

    fn written(&self, name: &str) -> PathBuf {
        self.current.join(name)
    }
}

#[cfg(test)]
mod tests;
