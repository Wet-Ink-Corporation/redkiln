//! git through the command line, bounded (H-13): every call has a deadline, after which git is
//! killed and reaped, and its output is capped.
//!
//! git's stdout and stderr go to anonymous files on the local temporary disk, not to pipes, so no
//! reader thread is needed and a full pipe can never stall git: the deadline is the only wait.
//! git runs under `LC_ALL=C`, so its messages are untranslated and [`discover`] can tell "not a
//! git repository" from every other failure, and without the variables that would steer it to
//! another repository ([`STEERING`]) or add configuration ([`CONFIG`]).
//!
//! Only the child is killed at the deadline. On Windows the `git.exe` on `PATH` may be Git for
//! Windows' launcher, whose own child, the real git, outlives the kill; it holds only its copies
//! of the spool files, which nothing here waits on, so the call still returns at the deadline.

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

/// What git's discovery says, untranslated, when no directory from the start up is a
/// repository: `fatal: not a git repository (or any of the parent directories): .git`, or, where
/// discovery stopped at a file system boundary, `(or any parent up to mount point /m)`. git's
/// other "not a git repository: <path>" names a gitdir that is gone (a pruned linked worktree's
/// `.git` file), which is a broken repository, not none (ST-01).
const NO_REPOSITORY: [&str; 2] = [
    "not a git repository (or any of the parent directories)",
    "not a git repository (or any parent up to mount point",
];

/// git's variables that would point it at another repository, index or search than the
/// directory's own: a git hook running the caller hands them on. Removed from git's
/// environment, so discovery always starts from the directory given.
const STEERING: [&str; 6] = [
    "GIT_DIR",
    "GIT_WORK_TREE",
    "GIT_COMMON_DIR",
    "GIT_INDEX_FILE",
    "GIT_CEILING_DIRECTORIES",
    "GIT_DISCOVERY_ACROSS_FILESYSTEM",
];

/// git's variables that add configuration on top of its files: `git -c`'s, which git hands to
/// its children, and `GIT_CONFIG_COUNT`'s. Without a count git reads no `GIT_CONFIG_KEY_<n>` or
/// `GIT_CONFIG_VALUE_<n>`, so those are inert once it is removed and need not be named (the
/// library reads no process state, so it could not list them).
const CONFIG: [&str; 2] = ["GIT_CONFIG_PARAMETERS", "GIT_CONFIG_COUNT"];

/// `git` without the variables that steer it to another repository ([`STEERING`]) or add
/// configuration ([`CONFIG`]): every git call Core makes runs through this.
pub(crate) fn isolated(mut git: Command) -> Command {
    for name in STEERING.iter().chain(&CONFIG) {
        git.env_remove(name);
    }
    git
}

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
    discover_with(Command::new("git"), cwd, timeout)
}

/// [`discover`] through `git`, a `Command` for git that may carry the caller's environment (the
/// tests' seam: they set git's variables on it, as a git hook would on the process).
fn discover_with(git: Command, cwd: &Path, timeout: Duration) -> Result<Option<GitDirs>, GitError> {
    let git = isolated(git);
    let args = [
        "rev-parse",
        "--path-format=absolute",
        "--git-common-dir",
        "--show-toplevel",
    ]
    .map(OsStr::new);
    let captured = run(git, &args, cwd, timeout)?;
    if !captured.status.success() {
        let stderr = String::from_utf8_lossy(&captured.stderr).trim().to_owned();
        if NO_REPOSITORY.iter().any(|said| stderr.contains(said)) {
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

/// Runs `command` with `args` in `cwd`, killing it once `timeout` has passed.
fn run(
    mut command: Command,
    args: &[&OsStr],
    cwd: &Path,
    timeout: Duration,
) -> Result<Captured, GitError> {
    let mut stdout = tempfile::tempfile().map_err(GitError::Io)?;
    let mut stderr = tempfile::tempfile().map_err(GitError::Io)?;
    let mut child = command
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
            match child.kill() {
                // Killed, or it exited since `try_wait` (`InvalidInput`): either way it ends now,
                // so reaping it cannot wait.
                Ok(()) => {}
                Err(e) if e.kind() == io::ErrorKind::InvalidInput => {}
                // Not killed: waiting for it could outlast the deadline, so it is left unreaped
                // (std reaps nothing on drop) rather than waited for.
                Err(e) => return Err(GitError::Io(e)),
            }
            child.wait().map_err(GitError::Io)?;
            return Err(GitError::Timeout { timeout });
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
            Command::new("sh"),
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
            Command::new("redkiln-no-such-program"),
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

    /// A repository at `<dir>/<name>`, its `.git` canonicalised.
    fn init(dir: &Path, name: &str) -> (PathBuf, PathBuf) {
        let repo = dir.join(name);
        std::fs::create_dir_all(repo.join("sub")).expect("dirs");
        let status = Command::new("git")
            .args(["init", "-q"])
            .current_dir(&repo)
            .status()
            .expect("git init");
        assert!(status.success());
        let git_dir = std::fs::canonicalize(repo.join(".git")).expect(".git");
        (repo, git_dir)
    }

    #[test]
    fn git_variables_the_caller_inherited_never_steer_discovery() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (here, here_git) = init(dir.path(), "here");
        let (_, other_git) = init(dir.path(), "other");
        let steered = [
            ("GIT_DIR", other_git.as_os_str()),
            ("GIT_COMMON_DIR", other_git.as_os_str()),
            ("GIT_WORK_TREE", dir.path().as_os_str()),
            ("GIT_INDEX_FILE", other_git.as_os_str()),
            ("GIT_CEILING_DIRECTORIES", here.as_os_str()),
            ("GIT_DISCOVERY_ACROSS_FILESYSTEM", OsStr::new("0")),
        ];
        for (name, value) in steered {
            // As a git hook's environment would hand them to the caller.
            let mut command = Command::new("git");
            command.env(name, value);
            let found = discover_with(command, &here.join("sub"), Duration::from_secs(10))
                .expect("discovered")
                .expect("a repository");
            assert_eq!(
                std::fs::canonicalize(&found.common_dir).expect("common dir"),
                here_git,
                "{name}"
            );
        }
    }

    #[test]
    fn configuration_the_caller_inherited_never_reaches_git() {
        let dir = tempfile::tempdir().expect("tempdir");
        let (here, _) = init(dir.path(), "here");
        let parameters = [("GIT_CONFIG_PARAMETERS", "'redkiln.probe'='set'")];
        let counted = [
            ("GIT_CONFIG_COUNT", "1"),
            ("GIT_CONFIG_KEY_0", "redkiln.probe"),
            ("GIT_CONFIG_VALUE_0", "set"),
        ];
        let args = ["config", "--get", "redkiln.probe"].map(OsStr::new);
        for variables in [&parameters[..], &counted[..]] {
            let mut seen = Command::new("git");
            seen.envs(variables.iter().copied());
            let control = run(seen, &args, &here, Duration::from_secs(10)).expect("ran");
            assert_eq!(control.stdout, b"set\n", "{variables:?}: git reads it");
            let mut command = Command::new("git");
            command.envs(variables.iter().copied());
            let got = run(isolated(command), &args, &here, Duration::from_secs(10)).expect("ran");
            assert_eq!(got.status.code(), Some(1), "{variables:?}: unset for git");
        }
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
