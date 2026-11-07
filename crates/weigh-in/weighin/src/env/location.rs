//! Where weighin's store is: Core's [`locate`] (ST-01), with weighin 2.x's two fallbacks for the
//! places a 1.x ledger was configured, and the old ledgers found on the way (Move 1, D1, D2).
//!
//! The chain, first match wins:
//! 1. `--store`;
//! 2. `$REDKILN_STORE`;
//! 3. `$WEIGHIN_STORE`, which names a 1.x ledger: the store goes beside it,
//!    `<parent>/<stem>.events.sqlite3`, with a warning (2.x only);
//! 4. `store` in `.redkiln/suite.toml`;
//! 5. `store` in `.kb/_ledger/config.json`, likewise naming a 1.x ledger (2.x only);
//! 6. `<git-common-dir>/redkiln/events.sqlite3`, or outside git `<folder>/.redkiln/events.sqlite3`.
//!
//! A 1.x ledger is never opened (ST-15, ST-16): each one that exists is a [`Notice`] until the
//! import (Move 1 step 8) takes it in. Process values are read once, into [`Vars`]: Core reads
//! no environment.

use std::ffi::{OsStr, OsString};
use std::fmt;
use std::path::{Path, PathBuf};

use redkiln_core::location::{GIT_TIMEOUT, LocateError, Search, Source, Workspace, locate};
use redkiln_core::suite::{Suite, Warning};

use crate::text::Report;

/// The process values store location reads, taken once.
#[derive(Debug, Default)]
pub(crate) struct Vars {
    redkiln_store: Option<OsString>,
    weighin_store: Option<OsString>,
    home: Option<PathBuf>,
}

impl Vars {
    /// `$REDKILN_STORE`, `$WEIGHIN_STORE` and the home directory (`HOME`, else `USERPROFILE`).
    pub(crate) fn from_process() -> Self {
        Self {
            redkiln_store: std::env::var_os(redkiln_core::location::STORE_ENV),
            weighin_store: std::env::var_os("WEIGHIN_STORE"),
            home: std::env::var_os("HOME")
                .or_else(|| std::env::var_os("USERPROFILE"))
                .map(PathBuf::from),
        }
    }
}

/// The store's file and the step that chose it (ST-03).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StoreAt {
    pub(crate) path: PathBuf,
    pub(crate) source: Source,
}

/// Something store location took or found that the user should hear about.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Notice {
    /// `$WEIGHIN_STORE` placed the store (2.x only, ST-01).
    WeighinStore(Fallback),
    /// `config.json`'s `store` placed the store (2.x only).
    LegacyStoreKey(Fallback),
    /// A weighin 1.x ledger is here, and is not read until it is imported.
    OldLedger { path: PathBuf },
    /// suite.toml, or one of its keys, could not be used.
    Suite(String),
}

impl fmt::Display for Notice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WeighinStore(fallback) => {
                fallback.describe(f, "$WEIGHIN_STORE", "$REDKILN_STORE")
            }
            Self::LegacyStoreKey(fallback) => fallback.describe(
                f,
                "`store` in .kb/_ledger/config.json",
                "`store` in .redkiln/suite.toml",
            ),
            Self::OldLedger { path } => write!(
                f,
                "a weighin 1.x ledger is at {}; weighin 2.0 leaves it unchanged and lists none of its items until it is imported",
                path.display()
            ),
            Self::Suite(text) => f.write_str(text),
        }
    }
}

/// What is at the path a 2.x fallback names: a 1.x ledger, or, for a setup made after 1.x
/// (`init --ledger`), nothing yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum There {
    /// A file, taken for the 1.x ledger (one whose existence cannot be checked counts).
    File,
    /// No file.
    Nothing,
}

impl There {
    /// What is at `path`: looked at, never opened.
    fn at(path: &Path) -> Self {
        match path.try_exists() {
            Ok(false) => Self::Nothing,
            Ok(true) | Err(_) => Self::File,
        }
    }
}

/// A 2.x fallback's answer: the path it named, the store beside it, and what is at that path.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Fallback {
    pub(crate) old: PathBuf,
    pub(crate) store: PathBuf,
    pub(crate) there: There,
}

impl Fallback {
    /// The fallback that named `old` and put the store at `store`, `old` looked at.
    fn new((old, store): (PathBuf, PathBuf)) -> Self {
        let there = There::at(&old);
        Self { old, store, there }
    }

    /// The notice: `setting` named the path, and `instead` is the setting to use. Only a file
    /// that is there is called a 1.x ledger.
    fn describe(&self, f: &mut fmt::Formatter<'_>, setting: &str, instead: &str) -> fmt::Result {
        let (old, store) = (self.old.display(), self.store.display());
        match self.there {
            There::File => write!(
                f,
                "{setting} names the 1.x ledger {old}; the store is beside it, at {store} (set {instead} instead)"
            ),
            There::Nothing => write!(
                f,
                "{setting} is {old} and no file is there; the store is beside that path, at {store} (set {instead} instead)"
            ),
        }
    }
}

/// Where the store is, where the working directory sits, and what to tell the user.
#[derive(Debug)]
pub(crate) struct Found {
    pub(crate) store: StoreAt,
    pub(crate) workspace: Workspace,
    pub(crate) notices: Vec<Notice>,
}

/// The store for `cwd`, first match of the chain in the module documentation. `flag` is
/// `--store`; `legacy_key` reads config.json's `store` from the worktree root it is given, and
/// is called once, when the working directory's place is known.
///
/// # Errors
/// As Core's [`locate`]: a relative `cwd`, a `~/` path with no home directory, or git failing
/// for any reason other than `cwd` being in no repository.
pub(crate) fn locate_store(
    cwd: &Path,
    flag: Option<&Path>,
    vars: &Vars,
    legacy_key: impl FnOnce(&Path) -> Option<String>,
) -> Result<Found, LocateError> {
    let home = vars.home.as_deref();
    let mut notices = Vec::new();
    // Step 3, below `$REDKILN_STORE`: the store beside the ledger `$WEIGHIN_STORE` names, given
    // to Core as the variable's value, so a later step never runs.
    let redkiln = set(vars.redkiln_store.as_deref());
    let weighin = match (flag, redkiln, set(vars.weighin_store.as_deref())) {
        (None, None, Some(named)) => Some(beside(&resolve(cwd, trimmed(named), home)?)),
        _ => None,
    };
    let search = Search {
        flag,
        env: match &weighin {
            Some((_, store)) => Some(store.as_os_str()),
            None => redkiln,
        },
        home,
        ..Search::new(cwd)
    };
    let located = locate(&search)?;
    let mut store = StoreAt {
        path: located.store,
        source: located.source,
    };
    let workspace = match located.workspace {
        Some(workspace) => workspace,
        None => Workspace::discover(cwd, GIT_TIMEOUT)?,
    };
    let key = legacy_key(workspace.root());
    // The path a fallback named is also an old ledger, so it is named twice when it is there.
    let mut named = None;
    if let Some(found) = weighin {
        let fallback = Fallback::new(found);
        named = Some(fallback.old.clone());
        notices.push(Notice::WeighinStore(fallback));
    } else if matches!(store.source, Source::GitCommonDir | Source::Folder)
        && let Some(key) = key.as_deref().filter(|k| !k.trim().is_empty())
    {
        let fallback = Fallback::new(legacy_key_store(workspace.root(), key, home)?);
        store = StoreAt {
            path: fallback.store.clone(),
            source: Source::Config,
        };
        named = Some(fallback.old.clone());
        notices.push(Notice::LegacyStoreKey(fallback));
    }
    notices.extend(old_ledgers(named, &workspace));
    notices.extend(
        located
            .suite
            .iter()
            .flat_map(Suite::warnings)
            .filter_map(suite_notice),
    );
    Ok(Found {
        store,
        workspace,
        notices,
    })
}

/// Step 5, below suite.toml's `store`: the 1.x ledger config.json's `key` names, resolved
/// against the worktree `root`, and the store beside it.
///
/// # Errors
/// A `~/` key with no home directory, reported as the config step's.
fn legacy_key_store(
    root: &Path,
    key: &str,
    home: Option<&Path>,
) -> Result<(PathBuf, PathBuf), LocateError> {
    let resolved = resolve(root, OsStr::new(key.trim()), home).map_err(|e| match e {
        LocateError::NoHome { .. } => LocateError::NoHome {
            step: Source::Config,
        },
        other => other,
    })?;
    Ok(beside(&resolved))
}

/// `value` as Core resolves a store path: `~/` expanded, then joined to `base`, normalised.
fn resolve(base: &Path, value: &OsStr, home: Option<&Path>) -> Result<PathBuf, LocateError> {
    let search = Search {
        env: Some(value),
        home,
        ..Search::new(base)
    };
    // Core's second step resolves the variable without asking git.
    locate(&search).map(|located| located.store)
}

/// The 1.x ledger at `old`, and the store beside it: `<parent>/<stem>.events.sqlite3` (D2).
fn beside(old: &Path) -> (PathBuf, PathBuf) {
    // A resolved path ends in a name unless it is a root or ends in `..`; the store is then
    // `.events.sqlite3` in it, never a failure of location.
    let mut name = old.file_stem().map(OsStr::to_os_string).unwrap_or_default();
    name.push(".events.sqlite3");
    let store = old.with_file_name(name);
    (old.to_path_buf(), store)
}

/// A notice for each old ledger that exists: the one a fallback named, then the default place
/// (`<common>/weighin/ledger.sqlite3`, outside git `<folder>/.weighin/ledger.sqlite3`). Looked at,
/// never opened; one whose existence cannot be checked is named too, as it may be there.
fn old_ledgers(named: Option<PathBuf>, workspace: &Workspace) -> Vec<Notice> {
    let default = match workspace {
        Workspace::Git { common_dir, .. } => common_dir.join("weighin"),
        Workspace::Folder(folder) => folder.join(".weighin"),
    }
    .join("ledger.sqlite3");
    let mut paths = named.into_iter().collect::<Vec<_>>();
    if !paths.contains(&default) {
        paths.push(default);
    }
    paths
        .into_iter()
        .filter(|path| There::at(path) == There::File)
        .map(|path| Notice::OldLedger { path })
        .collect()
}

/// A suite.toml warning, as weighin words it while its class still comes from config.json (#17
/// part 5b moves it onto suite.toml): `LegacyRepoClass` is not news yet, and an unusable file
/// does not make this repository `client`.
fn suite_notice(warning: &Warning) -> Option<Notice> {
    match warning {
        Warning::Unusable { path, problem } => Some(Notice::Suite(format!(
            "{} is not used: {}",
            path.display(),
            Report(problem)
        ))),
        Warning::Value { .. } => Some(Notice::Suite(warning.to_string())),
        Warning::LegacyRepoClass { .. } => None,
    }
}

/// A variable's value, unless it is unset, empty or only white space.
fn set(value: Option<&OsStr>) -> Option<&OsStr> {
    value.filter(|v| !blank(v))
}

/// `value` without surrounding white space, as 1.x read `$WEIGHIN_STORE`; a value that is not
/// UTF-8 is taken as it is.
fn trimmed(value: &OsStr) -> &OsStr {
    value.to_str().map_or(value, |s| OsStr::new(s.trim()))
}

/// Empty, or only white space.
fn blank(value: &OsStr) -> bool {
    value.to_str().is_some_and(|s| s.trim().is_empty())
}

#[cfg(test)]
mod tests;
