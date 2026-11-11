//! Where Bench's store is: the suite store, located by Core (ST-01), with anneal's
//! `$ANNEAL_STORE` read below `$REDKILN_STORE`, and the anneal ledgers this build never opens
//! but imports once from a copy (Move 2 step 3, `legacy_import`).
//!
//! The chain: `--store`; `$REDKILN_STORE`; `$ANNEAL_STORE`, which names an anneal ledger, so the
//! store goes beside it as `<parent>/<stem>.events.sqlite3` (D5, weighin's D2 B); `[suite]
//! store`; `<git-common-dir>/redkiln/events.sqlite3`; outside git
//! `<folder>/.redkiln/events.sqlite3`. Old ledgers are only looked for (existence, never opened):
//! `<git-common-dir>/anneal/ledger.sqlite3` (outside git `<folder>/.anneal/ledger.sqlite3`) when
//! git was asked, and `$ANNEAL_STORE`'s file.

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use redkiln_core::location::{self, GitError, LocateError, STORE_ENV, Search, Source, Workspace};

use super::{ANNEAL_STORE_VAR, PathError, StoreLocation, StoreSource};

/// anneal's ledger directory under the git common dir, and outside git `.anneal`.
const ANNEAL_DIR: &str = "anneal";

/// anneal's ledger file's name in its directory.
const ANNEAL_LEDGER: &str = "ledger.sqlite3";

/// Something about where the store is that a command reports as a warning and `SessionStart`
/// once in its context; other hooks stay silent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    /// `$ANNEAL_STORE` chose the store: it names an anneal ledger, and the store is beside it.
    AnnealStore {
        /// The ledger `$ANNEAL_STORE` names.
        old: PathBuf,
        /// The store beside it.
        store: PathBuf,
    },
    /// An anneal ledger exists, which this build leaves as it is and imports once
    /// (`legacy_import`, which reports it in place of this notice).
    OldLedger {
        /// The ledger.
        path: PathBuf,
    },
    /// `.redkiln/suite.toml` could not be used as written (Core's warning).
    Suite(String),
}

impl fmt::Display for Notice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AnnealStore { old, store } => write!(
                f,
                "$ANNEAL_STORE names the anneal ledger {}; bench keeps its events in the store \
                 beside it, {} (set $REDKILN_STORE to choose the store)",
                old.display(),
                store.display()
            ),
            Self::OldLedger { path } => write!(
                f,
                "the anneal ledger {} is left as it is: bench imports its events into the \
                 suite store once",
                path.display()
            ),
            Self::Suite(warning) => f.write_str(warning),
        }
    }
}

/// The store for a command or hook run in `cwd`, git given `git_timeout`.
///
/// # Errors
/// [`PathError::GitFailed`], [`PathError::GitTimeout`], [`PathError::Git`] or
/// [`PathError::NonUtf8Output`] when git cannot answer (only git saying `cwd` is in no
/// repository is the folder step); [`PathError::Locate`] for the rest of Core's refusals (a
/// relative `cwd`, a `~/` path with no home).
pub(super) fn locate(
    flag: Option<&Path>,
    lookup: impl Fn(&str) -> Option<OsString>,
    cwd: &Path,
    git_timeout: Duration,
) -> Result<StoreLocation, PathError> {
    let redkiln = lookup(STORE_ENV);
    let anneal = lookup(ANNEAL_STORE_VAR).filter(|value| !blank(value));
    let home = lookup("HOME")
        .filter(|value| !value.is_empty())
        .or_else(|| lookup("USERPROFILE").filter(|value| !value.is_empty()))
        .map(PathBuf::from);
    let redkiln_chose = redkiln.as_deref().is_some_and(|value| !blank(value));
    // `$ANNEAL_STORE` only when nothing above it chose: its store is passed as the flag.
    let beside = match (&anneal, flag, redkiln_chose) {
        (Some(value), None, false) => Some(cwd.join(value)),
        _ => None,
    };
    let beside_store = beside.as_deref().map(store_beside);
    let search = Search {
        cwd,
        flag: flag.or(beside_store.as_deref()),
        env: redkiln.as_deref(),
        home: home.as_deref(),
        git_timeout,
    };
    let located = location::locate(&search).map_err(from_locate)?;
    let mut notices = Vec::new();
    let mut old_ledgers = Vec::new();
    let source = match beside {
        Some(old) => {
            // The notice names both paths for the warning; the location keeps its own.
            notices.push(Notice::AnnealStore {
                old: old.clone(),
                store: located.store.clone(),
            });
            old_ledgers.push(old);
            StoreSource::Env
        }
        None => StoreSource::from(located.source),
    };
    if let Some(workspace) = &located.workspace {
        old_ledgers.push(anneal_ledger(workspace));
    }
    if let Some(suite) = &located.suite {
        notices.extend(
            suite
                .warnings()
                .iter()
                .map(|w| Notice::Suite(w.to_string())),
        );
    }
    // Existence only: an old ledger is never opened here (ST-15, ST-16). One whose existence
    // cannot be checked is not reported: the notice is a warning, and the import (`legacy_import`) reads
    // each source itself and reports what it cannot read.
    old_ledgers.retain(|path| path.try_exists().is_ok_and(|exists| exists));
    // Each notice names its ledger for the warning; `old_ledgers` keeps them for the import.
    notices.extend(
        old_ledgers
            .iter()
            .map(|path| Notice::OldLedger { path: path.clone() }),
    );
    Ok(StoreLocation {
        path: located.store,
        source,
        old_ledgers,
        from: cwd.to_path_buf(),
        notices,
    })
}

/// The store beside the anneal ledger `old`: `<parent>/<stem>.events.sqlite3`.
pub(super) fn store_beside(old: &Path) -> PathBuf {
    let stem = old.file_stem().unwrap_or_else(|| OsStr::new("ledger"));
    let mut name = stem.to_os_string();
    name.push(".events.sqlite3");
    old.with_file_name(name)
}

/// anneal's ledger for `workspace`: in the git common dir, or `.anneal` in the folder.
fn anneal_ledger(workspace: &Workspace) -> PathBuf {
    match workspace {
        Workspace::Git { common_dir, .. } => common_dir.join(ANNEAL_DIR).join(ANNEAL_LEDGER),
        Workspace::Folder(folder) => folder.join(".anneal").join(ANNEAL_LEDGER),
    }
}

/// Empty, or only white space.
fn blank(value: &OsStr) -> bool {
    value.to_str().is_some_and(|s| s.trim().is_empty()) || value.is_empty()
}

impl From<Source> for StoreSource {
    fn from(source: Source) -> Self {
        match source {
            Source::Flag => Self::Flag,
            Source::Env => Self::Env,
            Source::Config => Self::Config,
            Source::GitCommonDir => Self::GitCommonDir,
            Source::Folder => Self::Folder,
        }
    }
}

/// Core's refusal as Bench's path error: git's failures keep the variants Bench's callers
/// already name.
fn from_locate(error: LocateError) -> PathError {
    match error {
        LocateError::Git(GitError::Failed { dir, stderr }) => PathError::GitFailed { dir, stderr },
        LocateError::Git(GitError::Timeout { timeout }) => PathError::GitTimeout { timeout },
        LocateError::Git(GitError::Spawn(source) | GitError::Io(source)) => PathError::Git(source),
        LocateError::Git(GitError::NotUtf8(source)) => PathError::NonUtf8Output(source),
        other => PathError::Locate(other),
    }
}

#[cfg(test)]
mod tests;
