//! The diff a pull request makes, and [`PrError`].
//!
//! The diff is read through the GitHub kit's [`redkiln_github::diff`], the bounded git calls
//! `verdict`, `ship`, `brief build` and the merge gate share. [`PrError`] wraps Bench's gate
//! settings errors and maps to Bench's exit codes, so it stays here.

use crate::exit_codes::{self, Code};
use crate::paths::PathError;
use crate::repo_config::{FileError, GateConfigError};
use crate::ship::skip_rule::NumstatError;
pub(crate) use redkiln_github::diff::{
    Budget, DiffError, TextLimits, default_base, diff_text, is_ancestor, is_object_id, numstat,
    patch_id, resolve_commit,
};
use redkiln_guard::glob::GlobError;

/// Why a pull request's change could not be read: its worktree, a commit, its base, its diff
/// or the repository's gate settings. Shared by `verdict`, `ship` and the merge gate.
#[derive(Debug, thiserror::Error)]
pub enum PrError {
    /// The working directory is not in a git worktree.
    #[error("bench {command} needs a git worktree")]
    NotAWorktree {
        /// The command, as typed.
        command: &'static str,
        /// What git said.
        #[source]
        source: PathError,
    },
    /// A commit flag names no commit.
    #[error("{flag} {rev}: names no commit in this repository (fetch it first?)")]
    UnknownCommit {
        /// The flag.
        flag: &'static str,
        /// The revision as given.
        rev: String,
    },
    /// No base was given and neither default names a commit.
    #[error("no base: neither origin/main nor main names a commit; pass --base <ref>")]
    NoBase,
    /// The base names no commit.
    #[error("base {base}: names no commit in this repository; pass --base <ref>")]
    UnknownBase {
        /// The base as given or recorded.
        base: String,
    },
    /// The repository's gate settings could not be read.
    #[error(transparent)]
    Config(#[from] GateConfigError),
    /// A default sensitive glob does not compile (a test keeps them valid).
    #[error("a default sensitive glob")]
    DefaultGlob(#[source] GlobError),
    /// git could not read the diff.
    #[error(transparent)]
    Diff(#[from] DiffError),
    /// `git diff --numstat` printed a record it does not document.
    #[error(transparent)]
    Numstat(#[from] NumstatError),
}

impl PrError {
    /// The documented exit code: a usage error (2) for what the caller can fix (the worktree,
    /// a commit, the base, the config file's content), else an internal error (70).
    #[must_use]
    pub const fn code(&self) -> Code {
        match self {
            Self::NotAWorktree { .. }
            | Self::UnknownCommit { .. }
            | Self::NoBase
            | Self::UnknownBase { .. }
            | Self::Config(
                GateConfigError::File(
                    FileError::NotAFile { .. }
                    | FileError::NotOwn { .. }
                    | FileError::TooLarge { .. },
                )
                | GateConfigError::Json { .. }
                | GateConfigError::TooManyGlobs { .. }
                | GateConfigError::Glob { .. },
            ) => exit_codes::USAGE,
            Self::Config(GateConfigError::File(FileError::Read { .. }))
            | Self::DefaultGlob(_)
            | Self::Diff(_)
            | Self::Numstat(_) => exit_codes::INTERNAL,
        }
    }
}
