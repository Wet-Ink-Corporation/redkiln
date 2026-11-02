//! git through the command line, bounded (H-13): every call has a deadline, after which git is
//! killed and reaped, and its output is capped.
//!
//! git's stdout and stderr go to anonymous files on the local temporary disk, not to pipes, so no
//! reader thread is needed and a full pipe can never stall git: the deadline is the only wait.
//! git runs under `LC_ALL=C`, so its messages are untranslated and [`discover`] can tell "not a
//! git repository" from every other failure.

use std::ffi::OsStr;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

/// The most bytes kept of each of git's output streams; more is [`GitError::TooMuchOutput`].
pub(crate) const MAX_OUTPUT_BYTES: u64 = 64 * 1024;

/// How often a running git is checked for completion.
const POLL: Duration = Duration::from_millis(5);

/// What git says, untranslated, when a directory is in no repository.
const NOT_A_REPOSITORY: &str = "not a git repository";

/// Why git could not answer.
#[derive(Debug, thiserror::Error)]
pub enum GitError {
    /// git could not be started (it is not installed, or not on `PATH`).
    #[error("starting git")]
    Spawn(#[source] io::Error),
    /// Its output files could not be made or read, or it could not be waited on or killed.
    #[error("running git")]
    Io(#[source] io::Error),
    /// git did not finish within the timeout, and was killed.
    #[error("git did not answer within {timeout:?}")]
    Timeout {
        /// The limit.
        timeout: Duration,
    },
    /// git wrote more than 64 KiB to one stream.
    #[error("git wrote more output than expected")]
    TooMuchOutput,
    /// git failed in `dir` for a reason other than `dir` being in no repository: a repository
    /// owned by someone else, a broken `.git` file, a bare repository or the inside of `.git`.
    #[error("git failed in {}: {stderr}", dir.display())]
    Failed {
        /// The directory git ran in.
        dir: PathBuf,
        /// What git said, trimmed.
        stderr: String,
    },
    /// git printed a path that is not UTF-8.
    #[error("git printed a path that is not UTF-8")]
    NotUtf8(#[source] std::string::FromUtf8Error),
    /// git answered with something other than two paths, one per line.
    #[error("git's answer was not two paths")]
    Answer,
}

/// What a finished git left.
struct Captured {
    status: ExitStatus,
    stdout: Vec<u8>,
    stderr: Vec<u8>,
}

/// A git repository's directories, as git names them (absolute).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GitDirs {
    /// The common dir: the main checkout's `.git`, shared by every linked worktree.
    pub(crate) common_dir: PathBuf,
    /// The top of the worktree holding the directory.
    pub(crate) root: PathBuf,
}

/// The repository holding `cwd`, or `None` when git says it is in none.
///
/// # Errors
/// Any other git failure ([`GitError::Failed`]), and git that cannot run or answer in time.
pub(crate) fn discover(cwd: &Path, timeout: Duration) -> Result<Option<GitDirs>, GitError> {
    let args = [
        "rev-parse",
        "--path-format=absolute",
        "--git-common-dir",
        "--show-toplevel",
    ]
    .map(OsStr::new);
    let captured = run(OsStr::new("git"), &args, cwd, timeout)?;
    if !captured.status.success() {
        let stderr = String::from_utf8_lossy(&captured.stderr).trim().to_owned();
        if stderr.contains(NOT_A_REPOSITORY) {
            return Ok(None);
        }
        return Err(GitError::Failed {
            dir: cwd.to_path_buf(),
            stderr,
        });
    }
    let text = String::from_utf8(captured.stdout).map_err(GitError::NotUtf8)?;
    match text.lines().collect::<Vec<_>>().as_slice() {
        [common, root] if !common.is_empty() && !root.is_empty() => Ok(Some(GitDirs {
            common_dir: normalise(Path::new(common)),
            root: normalise(Path::new(root)),
        })),
        _ => Err(GitError::Answer),
    }
}

/// `path` rebuilt from its components: one separator form (git on Windows prints `C:/a/b`), and
/// no `.` or doubled separators. `..` is kept: folding it would need the file system.
pub(crate) fn normalise(path: &Path) -> PathBuf {
    path.components().collect()
}

/// Runs `program` with `args` in `cwd`, killing it once `timeout` has passed.
fn run(
    program: &OsStr,
    args: &[&OsStr],
    cwd: &Path,
    timeout: Duration,
) -> Result<Captured, GitError> {
    let mut stdout = tempfile::tempfile().map_err(GitError::Io)?;
    let mut stderr = tempfile::tempfile().map_err(GitError::Io)?;
    let mut child = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .env("LC_ALL", "C")
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout.try_clone().map_err(GitError::Io)?))
        .stderr(Stdio::from(stderr.try_clone().map_err(GitError::Io)?))
        .spawn()
        .map_err(GitError::Spawn)?;
    let status = wait(&mut child, timeout)?;
    Ok(Captured {
        status,
        stdout: read_back(&mut stdout)?,
        stderr: read_back(&mut stderr)?,
    })
}

/// Waits for `child` until `timeout`; past it the child is killed and reaped. A timeout too
/// large to add to the clock waits for as long as the child runs.
fn wait(child: &mut Child, timeout: Duration) -> Result<ExitStatus, GitError> {
    let Some(deadline) = Instant::now().checked_add(timeout) else {
        return child.wait().map_err(GitError::Io);
    };
    loop {
        if let Some(status) = child.try_wait().map_err(GitError::Io)? {
            return Ok(status);
        }
        if Instant::now() >= deadline {
            // It may have exited since `try_wait`; the kill then finds a process to reap, which
            // `wait` does, and the answer is still the timeout.
            let killed = child.kill();
            child.wait().map_err(GitError::Io)?;
            return match killed {
                Ok(()) => Err(GitError::Timeout { timeout }),
                Err(e) if e.kind() == io::ErrorKind::InvalidInput => {
                    Err(GitError::Timeout { timeout })
                }
                Err(e) => Err(GitError::Io(e)),
            };
        }
        std::thread::sleep(POLL);
    }
}

/// The whole of a spool file, refused past [`MAX_OUTPUT_BYTES`].
fn read_back(file: &mut File) -> Result<Vec<u8>, GitError> {
    file.seek(SeekFrom::Start(0)).map_err(GitError::Io)?;
    let mut bytes = Vec::new();
    file.take(MAX_OUTPUT_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(GitError::Io)?;
    if u64::try_from(bytes.len()).map_or(true, |n| n > MAX_OUTPUT_BYTES) {
        return Err(GitError::TooMuchOutput);
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    fn sh(dir: &Path, script: &str, timeout: Duration) -> Result<Captured, GitError> {
        run(
            OsStr::new("sh"),
            &[OsStr::new("-c"), OsStr::new(script)],
            dir,
            timeout,
        )
    }

    #[cfg(unix)]
    #[test]
    fn a_child_past_its_deadline_is_killed_and_reported() {
        let dir = tempfile::tempdir().expect("tempdir");
        let started = Instant::now();
        let outcome = sh(dir.path(), "sleep 30", Duration::from_millis(100));
        assert!(
            matches!(outcome, Err(GitError::Timeout { timeout }) if timeout == Duration::from_millis(100)),
            "{:?}",
            outcome.map(|c| c.status)
        );
        assert!(
            started.elapsed() < Duration::from_secs(10),
            "killed, not waited for: {:?}",
            started.elapsed()
        );
    }

    #[cfg(unix)]
    #[test]
    fn output_is_captured_and_capped() {
        let dir = tempfile::tempdir().expect("tempdir");
        let small = sh(
            dir.path(),
            "printf out; printf err >&2; exit 3",
            Duration::from_secs(10),
        )
        .expect("ran");
        assert_eq!(small.stdout, b"out");
        assert_eq!(small.stderr, b"err");
        assert_eq!(small.status.code(), Some(3));
        let large = sh(
            dir.path(),
            "head -c 70000 /dev/zero",
            Duration::from_secs(10),
        );
        assert!(
            matches!(large, Err(GitError::TooMuchOutput)),
            "{:?}",
            large.map(|c| c.stdout.len())
        );
    }

    #[test]
    fn a_program_that_cannot_start_is_a_spawn_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let outcome = run(
            OsStr::new("redkiln-no-such-program"),
            &[],
            dir.path(),
            Duration::from_secs(1),
        );
        assert!(
            matches!(outcome, Err(GitError::Spawn(_))),
            "{:?}",
            outcome.map(|c| c.status)
        );
    }

    #[test]
    fn paths_are_rebuilt_from_their_components() {
        assert_eq!(normalise(Path::new("/a//b/./c/")), PathBuf::from("/a/b/c"));
        assert_eq!(
            normalise(Path::new("/a/../b")),
            PathBuf::from("/a/../b"),
            "`..` kept"
        );
    }
}
