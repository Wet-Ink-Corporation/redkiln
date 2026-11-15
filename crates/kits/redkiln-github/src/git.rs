//! git run with a deadline and capped output (H-13), for [`crate::diff`].
//!
//! A copy of what the diff used of Bench's `paths` git runner: the plan duplicates it here
//! until `redkiln-dispatch` (Move 4) absorbs the copies. Its two failures read as Bench's did.

use std::io;
use std::path::Path;
use std::process::{Child, Command, Output, Stdio};
use std::time::Duration;

use crate::process::{self, WaitError};

/// Why git could not be run to completion.
#[derive(Debug, thiserror::Error)]
pub enum GitError {
    /// `git` could not be started or waited on.
    #[error("running git")]
    Run(#[source] io::Error),
    /// `git` did not finish within the timeout and was killed, or it exited but a process it
    /// started still held its output pipes at the deadline (that process is not killed).
    ///
    /// The limit is shown cut to whole milliseconds as `Duration` debugs it (`500ms`, `1s`,
    /// `1.5s`).
    #[error("git did not answer within {:?}", process::whole_millis(*timeout))]
    Timeout {
        /// The limit that was exceeded.
        timeout: Duration,
    },
}

/// Runs git in `cwd` with `args`, killing it past `timeout`, and keeps at most `cap` bytes of
/// each output stream (the rest is read and dropped). Its messages are not translated. A
/// caller that must not read a truncated answer passes one byte more than it accepts and
/// checks the length.
///
/// # Errors
/// [`GitError::Run`] when git cannot be started or waited on, [`GitError::Timeout`] past
/// `timeout`. A non-zero exit is not an error here.
pub(crate) fn run_git_capped(
    cwd: &Path,
    args: &[&str],
    timeout: Duration,
    cap: u64,
) -> Result<Output, GitError> {
    let mut child = Command::new("git")
        .args(args)
        .env("LC_ALL", "C")
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(GitError::Run)?;
    wait_capped(&mut child, timeout, cap)
}

/// [`process::wait_with_deadline`] for git: at most `cap` bytes per stream, and a timeout is
/// [`GitError::Timeout`].
fn wait_capped(child: &mut Child, timeout: Duration, cap: u64) -> Result<Output, GitError> {
    process::wait_with_deadline(child, timeout, cap, "git").map_err(|error| match error {
        WaitError::Io(source) => GitError::Run(source),
        WaitError::Timeout => GitError::Timeout { timeout },
    })
}
