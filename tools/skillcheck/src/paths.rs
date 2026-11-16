//! Paths the evals resolve: git's answers about a directory, with every git call bounded
//! (H-13).
//!
//! A copy of what the evals used of Bench's `paths`. `eval uplift`'s default work directory is
//! `tmp` in the user scope's root (`redkiln_calibrate::scope::user_root`; wi-6b797a85db,
//! wi-ad764d8531), so the anneal home rule copied here was deleted in PR 5.6 (wi-ad764d8531,
//! option A). The git runner is duplicated until `redkiln-dispatch` (Move 4) absorbs the copies.

use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::time::Duration;

use crate::process::{self, WaitError};

/// Why a path could not be resolved.
#[derive(Debug, thiserror::Error)]
pub enum PathError {
    /// `git` could not be started or waited on.
    #[error("running git")]
    Git(#[source] io::Error),
    /// `git` did not finish within the timeout and was killed, or it exited but a process it
    /// started still held its output pipes at the deadline (that process is not killed).
    ///
    /// The limit is shown cut to whole milliseconds as `Duration` debugs it (`500ms`, `1s`,
    /// `1.5s`).
    #[error("git did not answer within {:?}", process::whole_millis(*timeout))]
    GitTimeout {
        /// The limit that was exceeded.
        timeout: Duration,
    },
    /// `dir` is not inside a git repository.
    #[error("{dir} is not in a git repository: {stderr}", dir = dir.display())]
    NotARepository {
        /// The directory git was run in.
        dir: PathBuf,
        /// What git said, trimmed.
        stderr: String,
    },
    /// git failed in `dir` for a reason other than `dir` not being in a repository, such as a
    /// repository owned by someone else ("dubious ownership") or a broken `.git` file.
    #[error("git failed in {dir}: {stderr}", dir = dir.display())]
    GitFailed {
        /// The directory git was run in.
        dir: PathBuf,
        /// What git said, trimmed.
        stderr: String,
    },
    /// git printed a path that is not UTF-8.
    #[error("git printed a path that is not UTF-8")]
    NonUtf8Output(#[source] std::string::FromUtf8Error),
}

/// The top-level directory of the worktree containing `cwd`, as `git rev-parse
/// --show-toplevel` prints it (`/`-separated on every platform, symlinks resolved), with git
/// killed after `timeout`.
///
/// # Errors
/// [`PathError::NotARepository`] outside a repository; [`PathError::GitFailed`] when git fails
/// for another reason, or in a directory that has no working tree (a bare repository, or inside
/// `.git`); [`PathError::Git`] or [`PathError::GitTimeout`] when git cannot run;
/// [`PathError::NonUtf8Output`] for a path that is not UTF-8.
pub(crate) fn worktree_root_within(cwd: &Path, timeout: Duration) -> Result<String, PathError> {
    let output = run_git_capped(
        cwd,
        &["rev-parse", "--show-toplevel"],
        timeout,
        MAX_CAPTURE_BYTES,
    )?;
    if !output.status.success() {
        return Err(git_failure(cwd, &output.stderr));
    }
    let printed = String::from_utf8(output.stdout).map_err(PathError::NonUtf8Output)?;
    Ok(printed.trim_end_matches(['\r', '\n']).to_owned())
}

/// What git says (untranslated: [`run_git_capped`] runs it under `LC_ALL=C`) when a directory
/// is in no repository.
const NOT_A_REPOSITORY: &str = "not a git repository";

/// What git 2.56 and later say for a `.git` file pointing nowhere, which older git reported as
/// [`NOT_A_REPOSITORY`] naming the missing target: the same directory, in no repository.
const GITFILE_POINTS_NOWHERE: &str = "gitfile does not point to a valid repository";

/// The error for a git call in `dir` that exited non-zero, from what it wrote on stderr:
/// [`PathError::NotARepository`] only when git says so, [`PathError::GitFailed`] otherwise.
pub(crate) fn git_failure(dir: &Path, stderr: &[u8]) -> PathError {
    let dir = dir.to_path_buf();
    let stderr = String::from_utf8_lossy(stderr).trim().to_owned();
    if stderr.contains(NOT_A_REPOSITORY) || stderr.contains(GITFILE_POINTS_NOWHERE) {
        PathError::NotARepository { dir, stderr }
    } else {
        PathError::GitFailed { dir, stderr }
    }
}

/// Runs git with a deadline, killing it if the deadline passes, and keeps at most `cap` bytes
/// of each output stream (the rest is read and dropped). Its messages are not translated, so
/// [`git_failure`] can read them. A caller that must not read a truncated answer passes one
/// byte more than it accepts and checks the length.
///
/// # Errors
/// [`PathError::Git`] when git cannot be started or waited on, [`PathError::GitTimeout`] past
/// `timeout`. A non-zero exit is not an error here: see [`git_failure`].
pub(crate) fn run_git_capped(
    cwd: &Path,
    args: &[&str],
    timeout: Duration,
    cap: u64,
) -> Result<Output, PathError> {
    let mut child = Command::new("git")
        .args(args)
        .env("LC_ALL", "C")
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(PathError::Git)?;
    wait_capped(&mut child, timeout, cap)
}

/// Most bytes kept from each of git's output streams (H-13).
const MAX_CAPTURE_BYTES: u64 = 1024 * 1024;

/// [`process::wait_with_deadline`] for git: at most `cap` bytes per stream, and a timeout is
/// [`PathError::GitTimeout`].
fn wait_capped(child: &mut Child, timeout: Duration, cap: u64) -> Result<Output, PathError> {
    process::wait_with_deadline(child, timeout, cap, "git").map_err(|error| match error {
        WaitError::Io(source) => PathError::Git(source),
        WaitError::Timeout => PathError::GitTimeout { timeout },
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::{PathError, git_failure};

    #[test]
    fn only_git_saying_so_is_not_a_repository() {
        let dir = PathBuf::from("/d");
        assert!(matches!(
            git_failure(&dir, b"fatal: not a git repository (or any parent)\n"),
            PathError::NotARepository { .. }
        ));
        assert!(matches!(
            git_failure(&dir, b"fatal: detected dubious ownership\n"),
            PathError::GitFailed { .. }
        ));
    }
}
