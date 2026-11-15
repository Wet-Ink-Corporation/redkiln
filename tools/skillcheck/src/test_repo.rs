//! Scratch git repositories for unit tests: a `main` branch with a base commit, and helpers to
//! write files and commit them. Test-only; every helper returns a `Result` so a failed setup is
//! a test error with git's message. A copy of the part of Bench's `test_repo` the eval tests
//! use.

use std::error::Error;
use std::path::Path;
use std::process::Command;

use tempfile::TempDir;

/// A test's result, with any setup error.
pub(crate) type Setup<T> = Result<T, Box<dyn Error>>;

/// A scratch repository, removed when dropped.
pub(crate) struct ScratchRepo {
    dir: TempDir,
}

impl ScratchRepo {
    /// A repository on `main` with one commit holding `README.md`.
    pub(crate) fn new() -> Setup<Self> {
        let repo = Self {
            dir: tempfile::tempdir()?,
        };
        repo.git(&["init", "-q"])?;
        repo.write("README.md", "scratch\n")?;
        repo.commit("base")?;
        Ok(repo)
    }

    /// The working tree's root.
    pub(crate) fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Runs `git <args>`, isolated from the caller's repository and identity, and returns its
    /// stdout without the final newline; a failure is an error carrying git's stderr.
    fn git(&self, args: &[&str]) -> Setup<String> {
        let output = Command::new("git")
            .args([
                "-c",
                "user.name=skillcheck test",
                "-c",
                "user.email=test@example.invalid",
                "-c",
                "commit.gpgsign=false",
                "-c",
                "init.defaultBranch=main",
                "-c",
                "core.autocrlf=false",
            ])
            .args(args)
            .current_dir(self.path())
            .env_remove("GIT_DIR")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_COMMON_DIR")
            .env_remove("GIT_INDEX_FILE")
            .output()?;
        if !output.status.success() {
            return Err(
                format!("git {args:?}: {}", String::from_utf8_lossy(&output.stderr)).into(),
            );
        }
        Ok(String::from_utf8(output.stdout)?.trim_end().to_owned())
    }

    /// Writes `content` to `rel`, creating its directories.
    pub(crate) fn write(&self, rel: &str, content: &str) -> Setup<()> {
        let path = self.path().join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        Ok(std::fs::write(path, content)?)
    }

    /// Commits everything as `message` and returns the new commit's id.
    pub(crate) fn commit(&self, message: &str) -> Setup<String> {
        self.git(&["add", "-A"])?;
        self.git(&["commit", "-q", "--allow-empty", "-m", message])?;
        self.git(&["rev-parse", "HEAD"])
    }
}
