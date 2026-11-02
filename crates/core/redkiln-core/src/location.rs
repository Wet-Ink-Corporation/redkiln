//! Where the store is (SPEC ST-01 to ST-03).
//!
//! [`locate`] takes the first match of five steps: the `--store` flag, `$REDKILN_STORE`, `store`
//! in `.redkiln/suite.toml`, `<git-common-dir>/redkiln/events.sqlite3`, and outside git
//! `<folder>/.redkiln/events.sqlite3`. It reports the step that chose the file ([`Source`], the
//! SPEC's `store_source`).
//!
//! Core reads no harness protocol and no process state here: the working directory (the hook
//! input's `cwd`, or the process's, never `CLAUDE_PROJECT_DIR`: ST-02), the variable's value
//! and the home directory are the caller's to read and pass in [`Search`]. git is asked only
//! when neither the flag nor the variable chose the store, with a timeout (H-13). Only git's
//! "not a git repository" is the folder step; any other git failure is an error, so a repository
//! git refuses never gets a second, per-folder store (ST-01).
//!
//! Nothing here creates a file or directory: the store is created by [`crate::store::Store::open`].

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub use crate::git::GitError;
use crate::git::{self, GitDirs, normalise};
use crate::suite::Suite;

/// The variable that names the store (ST-01 step 2); its value is the caller's to read.
pub const STORE_ENV: &str = "REDKILN_STORE";

/// The store's file name in its directory (OW-1).
pub const STORE_FILE: &str = "events.sqlite3";

/// How long git may take to answer before it is killed, unless [`Search::git_timeout`] says
/// otherwise.
pub const GIT_TIMEOUT: Duration = Duration::from_secs(10);

/// The store's directory under the git common dir.
const COMMON_DIR_FOLDER: &str = "redkiln";

/// The store's directory in a folder outside git.
const FOLDER_DIR: &str = ".redkiln";

/// The step of ST-01 that chose the store (ST-03's `store_source`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// `--store <file>`.
    Flag,
    /// `$REDKILN_STORE`.
    Env,
    /// `store` in `.redkiln/suite.toml`.
    Config,
    /// `<git-common-dir>/redkiln/events.sqlite3`.
    GitCommonDir,
    /// `<folder>/.redkiln/events.sqlite3`, outside git.
    Folder,
}

impl Source {
    /// The SPEC's name for the step: `flag`, `env`, `config`, `git_common_dir` or `folder`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Flag => "flag",
            Self::Env => "env",
            Self::Config => "config",
            Self::GitCommonDir => "git_common_dir",
            Self::Folder => "folder",
        }
    }
}

/// Where a directory sits: in a git worktree, or in a folder outside git.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Workspace {
    /// In a git repository.
    Git {
        /// The top of the worktree holding the directory; suite.toml lives here.
        root: PathBuf,
        /// The common dir: the main checkout's `.git`, shared by every linked worktree.
        common_dir: PathBuf,
    },
    /// Outside git, as git said: the directory itself.
    Folder(PathBuf),
}

impl Workspace {
    /// Asks git where `cwd` sits, killing git after `timeout`.
    ///
    /// # Errors
    /// [`LocateError::RelativeCwd`] when `cwd` is relative, so it would depend on the process's
    /// own; [`LocateError::Git`] with [`GitError::Failed`] when git fails for any reason other
    /// than `cwd` being in no repository (a broken `.git` file, a gitdir that is gone, dubious
    /// ownership, a bare repository), or another [`GitError`] when git cannot be run or does not
    /// answer in time.
    pub fn discover(cwd: &Path, timeout: Duration) -> Result<Self, LocateError> {
        if !cwd.is_absolute() {
            return Err(LocateError::RelativeCwd {
                cwd: cwd.to_path_buf(),
            });
        }
        Ok(
            match git::discover(cwd, timeout).map_err(LocateError::Git)? {
                Some(GitDirs { common_dir, root }) => Self::Git { root, common_dir },
                None => Self::Folder(normalise(cwd)),
            },
        )
    }

    /// Where suite.toml is read and its `store` resolved: the worktree root, or the folder.
    #[must_use]
    pub fn root(&self) -> &Path {
        match self {
            Self::Git { root, .. } | Self::Folder(root) => root,
        }
    }
}

/// What [`locate`] starts from, every value read by the caller.
#[derive(Debug, Clone, Copy)]
pub struct Search<'a> {
    /// The directory resolution starts from: the hook input's `cwd`, or the process working
    /// directory (ST-02). Must be absolute.
    pub cwd: &'a Path,
    /// `--store <file>`, relative to `cwd`.
    pub flag: Option<&'a Path>,
    /// `$REDKILN_STORE`'s value ([`STORE_ENV`]), relative to `cwd`; empty or blank is unset.
    pub env: Option<&'a OsStr>,
    /// The home directory a leading `~/` expands to (`HOME`, or `USERPROFILE` on Windows).
    pub home: Option<&'a Path>,
    /// How long git may take to answer.
    pub git_timeout: Duration,
}

impl<'a> Search<'a> {
    /// A search from `cwd` with no flag, no variable, no home directory and [`GIT_TIMEOUT`].
    #[must_use]
    pub const fn new(cwd: &'a Path) -> Self {
        Self {
            cwd,
            flag: None,
            env: None,
            home: None,
            git_timeout: GIT_TIMEOUT,
        }
    }
}

/// The store [`locate`] found, and what it learned on the way.
#[derive(Debug)]
pub struct Located {
    /// The store file (ST-03's `store`). It may not exist yet.
    pub store: PathBuf,
    /// The step that chose it (ST-03's `store_source`).
    pub source: Source,
    /// Where the working directory sits, when git was asked (steps 3 to 5).
    pub workspace: Option<Workspace>,
    /// The suite config, when it was read (steps 3 to 5): its repository class and warnings.
    pub suite: Option<Suite>,
}

/// Why the store could not be located.
#[derive(Debug, thiserror::Error)]
pub enum LocateError {
    /// The working directory given is relative, so it would depend on the process's own.
    #[error("the working directory {} is not absolute", cwd.display())]
    RelativeCwd {
        /// The directory given.
        cwd: PathBuf,
    },
    /// A path starts `~/`, and no home directory was given.
    #[error("the store path from {} starts `~/`, and there is no home directory", step.as_str())]
    NoHome {
        /// The step whose path it is.
        step: Source,
    },
    /// git could not say where the working directory sits.
    #[error("asking git where the store is")]
    Git(#[source] GitError),
}

/// The store for `search`, first match wins (ST-01).
///
/// # Errors
/// [`LocateError::RelativeCwd`] for a relative working directory; [`LocateError::NoHome`] for a
/// `~/` path with no home directory; [`LocateError::Git`] when git is needed and fails for any
/// reason other than the directory being in no repository, or cannot run in time.
pub fn locate(search: &Search<'_>) -> Result<Located, LocateError> {
    let cwd = search.cwd;
    if !cwd.is_absolute() {
        return Err(LocateError::RelativeCwd {
            cwd: cwd.to_path_buf(),
        });
    }
    let chosen = |store: PathBuf, source| Located {
        store: normalise(&store),
        source,
        workspace: None,
        suite: None,
    };
    if let Some(flag) = search.flag {
        return Ok(chosen(cwd.join(flag), Source::Flag));
    }
    if let Some(value) = search.env.filter(|v| !blank(v)) {
        let path = expand_home(Path::new(value), search.home, Source::Env)?;
        return Ok(chosen(cwd.join(path), Source::Env));
    }
    let workspace = Workspace::discover(cwd, search.git_timeout)?;
    let suite = Suite::load(workspace.root());
    let (store, source) = match (suite.store(), &workspace) {
        (Some(configured), _) => {
            let path = expand_home(configured, search.home, Source::Config)?;
            (workspace.root().join(path), Source::Config)
        }
        (None, Workspace::Git { common_dir, .. }) => (
            common_dir.join(COMMON_DIR_FOLDER).join(STORE_FILE),
            Source::GitCommonDir,
        ),
        (None, Workspace::Folder(folder)) => {
            (folder.join(FOLDER_DIR).join(STORE_FILE), Source::Folder)
        }
    };
    Ok(Located {
        store: normalise(&store),
        source,
        workspace: Some(workspace),
        suite: Some(suite),
    })
}

/// Empty, or only white space.
fn blank(value: &OsStr) -> bool {
    value.to_str().is_some_and(|s| s.trim().is_empty())
}

/// `path` with a leading `~/` replaced by `home`; anything else as it is (`~user` is not
/// expanded).
fn expand_home(path: &Path, home: Option<&Path>, source: Source) -> Result<PathBuf, LocateError> {
    let tilde_slash = path.as_os_str().as_encoded_bytes().starts_with(b"~/")
        || (cfg!(windows) && path.as_os_str().as_encoded_bytes().starts_with(b"~\\"));
    match path.strip_prefix("~") {
        Ok(rest) if tilde_slash => home
            .map(|home| home.join(rest))
            .ok_or(LocateError::NoHome { step: source }),
        _ => Ok(path.to_path_buf()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_leading_tilde_slash_is_expanded() {
        let home = Path::new("/home/r");
        let expand = |p: &str| expand_home(Path::new(p), Some(home), Source::Env).expect("expands");
        assert_eq!(expand("~/a/b"), Path::new("/home/r/a/b"));
        assert_eq!(expand("~"), Path::new("~"), "a bare tilde is a name");
        assert_eq!(expand("~r/a"), Path::new("~r/a"));
        assert_eq!(expand("a/~/b"), Path::new("a/~/b"));
    }

    #[test]
    fn blank_is_empty_or_white_space() {
        assert!(blank(OsStr::new("")));
        assert!(blank(OsStr::new(" \t ")));
        assert!(!blank(OsStr::new(" x ")));
    }
}
