//! The user scope (blueprint, User scope): `~/.redkiln/`, the per-user folder Core manages, and
//! where in it each producer keeps its decision rows.
//!
//! The root is `$REDKILN_HOME` when set, else `$HOME/.redkiln`, else `%USERPROFILE%\.redkiln`.
//! A producer's rows live in `<root>/calibrate/<producer>/`. anneal kept them in
//! `<anneal home>/decisions/` (`$ANNEAL_HOME`, else `~/.anneal`); per Ryan's D1 (option A,
//! 2026-11-14) that folder is read alongside the new one and never written or deleted, so
//! [`UserScope::decisions`] names both. Empty values count as unset; a relative one is refused,
//! since it would move with the working directory.
//!
//! A caller that needs only the root takes [`user_root`], which never reads anneal's home, so a
//! relative `$ANNEAL_HOME` does not stop it (wi-ad764d8531, option A).
//!
//! The environment is read only through the lookup [`UserScope::from_lookup`] or [`user_root`]
//! is given, once, by the binary (ARC-11).

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// The variable naming the user scope's root.
pub const HOME_VAR: &str = "REDKILN_HOME";

/// The root's folder inside a home directory.
pub(crate) const HOME_DIR: &str = ".redkiln";

/// The variable naming anneal's home, read only for its old decision log.
pub(crate) const LEGACY_HOME_VAR: &str = "ANNEAL_HOME";

/// anneal's home folder inside a home directory.
pub(crate) const LEGACY_HOME_DIR: &str = ".anneal";

/// The folder inside the root that holds each producer's decision rows.
pub(crate) const CALIBRATE_DIR: &str = "calibrate";

/// anneal's decision log folder inside its home.
pub(crate) const LEGACY_DECISIONS_DIR: &str = crate::log::LOG_DIR;

/// The home directory variables, in the order they are tried.
const HOME_VARS: [&str; 2] = ["HOME", "USERPROFILE"];

/// Why the user scope could not be resolved.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ScopeError {
    /// None of `REDKILN_HOME`, `HOME` or `USERPROFILE` is set.
    #[error("cannot find a home directory: set REDKILN_HOME, HOME or USERPROFILE")]
    NoHome,
    /// The variable holds a relative path, which would move with the working directory.
    #[error("{var} must be an absolute path, got {value}", value = value.display())]
    RelativeHome {
        /// The variable that was read.
        var: &'static str,
        /// The path it gave.
        value: PathBuf,
    },
    /// A producer name that is not one plain folder name.
    #[error("{name:?} is not a producer name (lowercase letters, digits and `-`, not first)")]
    Producer {
        /// The name given.
        name: String,
    },
}

/// The user scope's root, and anneal's home when one is set or implied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserScope {
    root: PathBuf,
    legacy_home: Option<PathBuf>,
}

/// Where a producer's decision rows are: written only to `current`, read from `legacy` first
/// (when there is one) and then `current`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionDirs {
    /// `<root>/calibrate/<producer>`: the only folder written.
    pub current: PathBuf,
    /// anneal's `<anneal home>/decisions`, read alongside and never written or deleted; `None`
    /// without an anneal home, or when it is `current` itself.
    pub legacy: Option<PathBuf>,
}

impl UserScope {
    /// The scope `lookup` names; `lookup` reads one environment variable.
    ///
    /// # Errors
    /// [`ScopeError::NoHome`] when neither `REDKILN_HOME`, `HOME` nor `USERPROFILE` is set;
    /// [`ScopeError::RelativeHome`] when the root, or anneal's home, would be relative.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<OsString>) -> Result<Self, ScopeError> {
        let read = |var| read(&lookup, var);
        let home = HOME_VARS.into_iter().find_map(read);
        let root = root(read(HOME_VAR), home.as_ref())?;
        let legacy_home = read(LEGACY_HOME_VAR)
            .or_else(|| home.map(|(var, dir)| (var, dir.join(LEGACY_HOME_DIR))))
            .map(absolute)
            .transpose()?;
        Ok(Self { root, legacy_home })
    }

    /// The root, `~/.redkiln` or `$REDKILN_HOME`.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// anneal's home (`$ANNEAL_HOME`, else `$HOME/.anneal`, else `%USERPROFILE%\.anneal`),
    /// which a producer that moved from anneal reads alongside its folder in the root and never
    /// writes or deletes; `None` when no variable names one.
    #[must_use]
    pub fn legacy_home(&self) -> Option<&Path> {
        self.legacy_home.as_deref()
    }

    /// Where `producer`'s decision rows are (see [`DecisionDirs`]).
    ///
    /// # Errors
    /// [`ScopeError::Producer`] unless `producer` is lowercase ASCII letters, digits and `-`,
    /// not starting with `-`, so it names exactly one folder.
    pub fn decisions(&self, producer: &str) -> Result<DecisionDirs, ScopeError> {
        if !is_plain(producer) {
            return Err(ScopeError::Producer {
                name: producer.to_owned(),
            });
        }
        let current = self.root.join(CALIBRATE_DIR).join(producer);
        let legacy = self
            .legacy_home
            .as_ref()
            .map(|home| home.join(LEGACY_DECISIONS_DIR))
            .filter(|legacy| *legacy != current);
        Ok(DecisionDirs { current, legacy })
    }
}

/// The user scope's root alone, as [`UserScope::root`] gives it.
///
/// For a caller that needs nothing else: anneal's home is not read, so a relative
/// `ANNEAL_HOME` does not stop it (Ryan's wi-ad764d8531, option A). `lookup` reads one
/// environment variable.
///
/// # Errors
/// [`ScopeError::NoHome`] when neither `REDKILN_HOME`, `HOME` nor `USERPROFILE` is set;
/// [`ScopeError::RelativeHome`] when the root would be relative.
pub fn user_root(lookup: impl Fn(&str) -> Option<OsString>) -> Result<PathBuf, ScopeError> {
    let read = |var| read(&lookup, var);
    root(
        read(HOME_VAR),
        HOME_VARS.into_iter().find_map(read).as_ref(),
    )
}

/// `var` and its value as a path, through `lookup`; `None` when unset or empty.
fn read(
    lookup: impl Fn(&str) -> Option<OsString>,
    var: &'static str,
) -> Option<(&'static str, PathBuf)> {
    lookup(var)
        .filter(|value| !value.is_empty())
        .map(|value| (var, PathBuf::from(value)))
}

/// The root: `REDKILN_HOME`'s value `over` when set, else `.redkiln` in the home directory
/// `home`; it must be absolute.
fn root(
    over: Option<(&'static str, PathBuf)>,
    home: Option<&(&'static str, PathBuf)>,
) -> Result<PathBuf, ScopeError> {
    over.or_else(|| home.map(|(var, dir)| (*var, dir.join(HOME_DIR))))
        .ok_or(ScopeError::NoHome)
        .and_then(absolute)
}

/// `path`, when absolute.
fn absolute((var, path): (&'static str, PathBuf)) -> Result<PathBuf, ScopeError> {
    if path.is_absolute() {
        Ok(path)
    } else {
        Err(ScopeError::RelativeHome { var, value: path })
    }
}

/// Whether `name` is one plain lowercase folder name.
fn is_plain(name: &str) -> bool {
    !name.starts_with('-')
        && !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

#[cfg(test)]
mod tests;
