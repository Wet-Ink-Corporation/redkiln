//! `.redkiln/bench/config.json` (anneal's `.anneal/config.json` as a 0.x fallback): the
//! repository's `check:fast` command.
//!
//! ```json
//! {"check_fast": ["cargo", "check", "--quiet"]}
//! ```
//!
//! `check_fast` is an argv array, run without a shell. Other keys belong to other readers and
//! are ignored here. The file is read through `repo_config::read_text`, which the merge gate's
//! settings share; each error names the file read.

use std::io;
use std::path::Path;

use serde::Deserialize;

#[cfg(test)]
use crate::repo_config::CONFIG_PATH;
use crate::repo_config::{self, FileError};

/// The fields this hook reads.
#[derive(Debug, Deserialize)]
struct ConfigDto {
    #[serde(default)]
    check_fast: Option<Vec<String>>,
}

/// Why the config cannot give a `check:fast` command.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ConfigError {
    /// The file exists but cannot be read.
    #[error("{path} cannot be read")]
    Read {
        /// The file, relative to the worktree root.
        path: &'static str,
        /// The failure.
        #[source]
        source: io::Error,
    },
    /// The path is not a regular file (a directory, a symlink, a FIFO): never opened.
    #[error("{path} is not a regular file")]
    NotAFile {
        /// The file, relative to the worktree root.
        path: &'static str,
    },
    /// The file is over the config's size cap.
    #[error("{}", FileError::TooLarge { path })]
    TooLarge {
        /// The file, relative to the worktree root.
        path: &'static str,
    },
    /// Not a JSON object, or `check_fast` is not an array of strings.
    #[error("{path} is not a config object with check_fast as an array of strings")]
    Json {
        /// The file, relative to the worktree root.
        path: &'static str,
        /// What did not parse.
        #[source]
        source: serde_json::Error,
    },
    /// `check_fast` is an empty array: there is no program to run.
    #[error("{path}: check_fast is empty; give the program and its arguments")]
    EmptyCheckFast {
        /// The file, relative to the worktree root.
        path: &'static str,
    },
}

impl From<FileError> for ConfigError {
    fn from(error: FileError) -> Self {
        match error {
            FileError::Read { path, source } => Self::Read { path, source },
            FileError::NotAFile { path } => Self::NotAFile { path },
            FileError::TooLarge { path } => Self::TooLarge { path },
        }
    }
}

/// The `check:fast` argv of the text of [`CONFIG_PATH`]; `None` when it sets none.
///
/// # Errors
/// The [`ConfigError`] when the config is not valid for it.
#[cfg(test)]
pub(crate) fn check_fast(text: &str) -> Result<Option<Vec<String>>, ConfigError> {
    check_fast_in(CONFIG_PATH, text)
}

/// The `check:fast` argv of `text`, read from `path`; `None` when it sets none.
///
/// # Errors
/// The [`ConfigError`] when the config is not valid for it.
fn check_fast_in(path: &'static str, text: &str) -> Result<Option<Vec<String>>, ConfigError> {
    let dto: ConfigDto =
        serde_json::from_str(text).map_err(|source| ConfigError::Json { path, source })?;
    match dto.check_fast {
        Some(argv) if argv.is_empty() => Err(ConfigError::EmptyCheckFast { path }),
        argv => Ok(argv),
    }
}

/// The `check:fast` argv of the config in the worktree `root`; `None` when there is no config
/// file or it sets none.
///
/// # Errors
/// The [`ConfigError`] when the file cannot be read or is not valid.
pub(crate) fn read(root: &Path) -> Result<Option<Vec<String>>, ConfigError> {
    match repo_config::read_text(root)? {
        Some(config) => check_fast_in(config.path, &config.text),
        None => Ok(None),
    }
}
