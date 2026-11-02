//! Subjects: what an approval, a verdict or a gate result binds to, the hash of its subject by a
//! named method, written `<method>:<hash>` (SPEC ID-09, ID-10).
//!
//! | Method | Hash of | Taken by |
//! | --- | --- | --- |
//! | `sha256` | a file's bytes | [`Subject::sha256`], [`sha256_file`] |
//! | `git-commit` | a commit | [`git_commit`] |
//! | `git-tree` | the worktree's tree: the index plus tracked modifications | [`git_tree`] |
//! | `patch-id` | the pinned diff from `<base>...<commit>`, with its base | [`patch_id`] |
//! | `rust-fingerprint` | Redkiln Rust's change fingerprint | not here: Redkiln Rust's, read only |
//!
//! ID-10 says which binds what: an approved file is `sha256`, a review verdict `patch-id` plus its
//! base, a gate on exactly this snapshot `git-tree`, what CI ran `git-commit`, and any other gate
//! its declared method.
//!
//! Every git call goes through Core's bounded runner: a deadline after which git is killed,
//! output spooled to the local temporary disk and capped, and none of the variables that would
//! steer git to another repository or add configuration.

mod patch_id;
mod tree;

use std::ffi::OsStr;
use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use sha2::{Digest as _, Sha256};

use crate::git::{self, GitError};
use crate::ids::{is_lower_hex, to_hex};

pub use patch_id::{NoPatchId, PatchId, patch_id};
pub use tree::git_tree;

/// A hash method (ID-09). A new method is a SPEC minor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Method {
    /// A file's bytes: 64 hex.
    Sha256,
    /// A commit: 40 or 64 hex, the repository's object format.
    GitCommit,
    /// The worktree's tree, untracked files excluded: 40 or 64 hex.
    GitTree,
    /// `git patch-id --verbatim` of the pinned diff, recorded with its base: 40 or 64 hex.
    PatchId,
    /// Redkiln Rust's change fingerprint over Rust pathspecs, temper 1.x's byte for byte: hex.
    RustFingerprint,
}

impl Method {
    /// Every method.
    pub const ALL: [Self; 5] = [
        Self::Sha256,
        Self::GitCommit,
        Self::GitTree,
        Self::PatchId,
        Self::RustFingerprint,
    ];

    /// The method's name, as a subject writes it.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Sha256 => "sha256",
            Self::GitCommit => "git-commit",
            Self::GitTree => "git-tree",
            Self::PatchId => "patch-id",
            Self::RustFingerprint => "rust-fingerprint",
        }
    }

    /// Whether `hex`, already lowercase hex, has this method's length.
    const fn sized(self, len: usize) -> bool {
        match self {
            Self::Sha256 => len == 64,
            Self::GitCommit | Self::GitTree | Self::PatchId => len == 40 || len == 64,
            Self::RustFingerprint => true,
        }
    }
}

impl fmt::Display for Method {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A subject: a method and the hash it took, `<method>:<lowercase hex>`. Hashes of two object
/// formats (40 and 64 hex) never compare equal.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Subject {
    method: Method,
    /// `<method>:<hash>`, the hash valid for the method.
    text: String,
}

impl Subject {
    /// The `sha256` subject of `bytes`: an approved file's contents.
    #[must_use]
    pub fn sha256(bytes: &[u8]) -> Self {
        Self::of(Method::Sha256, &to_hex(&Sha256::digest(bytes)))
    }

    /// Reads a subject in its written form.
    ///
    /// # Errors
    /// [`SubjectError::Form`] for an unknown method, or a hash that is not lowercase hex of the
    /// method's length.
    pub fn parse(text: &str) -> Result<Self, SubjectError> {
        let (name, hash) = text.split_once(':').ok_or(SubjectError::Form)?;
        let method = Method::ALL
            .into_iter()
            .find(|m| m.as_str() == name)
            .ok_or(SubjectError::Form)?;
        Self::checked(method, hash).ok_or(SubjectError::Form)
    }

    /// The subject as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// Its method.
    #[must_use]
    pub const fn method(&self) -> Method {
        self.method
    }

    /// Its hash, after the colon.
    #[must_use]
    pub fn hash(&self) -> &str {
        self.text.split_once(':').map_or("", |(_, hash)| hash)
    }

    /// `method:hash` when the hash is lowercase hex of the method's length.
    fn checked(method: Method, hash: &str) -> Option<Self> {
        (is_lower_hex(hash) && method.sized(hash.len())).then(|| Self::of(method, hash))
    }

    fn of(method: Method, hash: &str) -> Self {
        Self {
            method,
            text: format!("{method}:{hash}"),
        }
    }
}

impl fmt::Display for Subject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

/// Why no subject was taken.
#[derive(Debug, thiserror::Error)]
pub enum SubjectError {
    /// The text is not `<method>:<hash>` with a known method and a hash of its form.
    #[error("not a subject: a known method, a colon and lowercase hex of the method's length")]
    Form,
    /// The file to hash could not be read.
    #[error("reading the file to hash")]
    Read(#[source] io::Error),
    /// git failed, or could not run or answer in time.
    #[error("running git")]
    Git(#[source] GitError),
    /// git answered with something other than one object id.
    #[error("git's answer was not one object id")]
    Answer,
    /// The temporary index for `git-tree` could not be made on the temporary disk.
    #[error("making the temporary index")]
    TempIndex(#[source] io::Error),
    /// The change has no `patch-id` (ID-09's conditions), so nothing may bind to one.
    #[error("the change has no patch-id")]
    NoPatchId(#[source] NoPatchId),
}

/// The `sha256` subject of the file at `path`, read in chunks: it is never held whole.
///
/// # Errors
/// [`SubjectError::Read`] when the file cannot be read.
pub fn sha256_file(path: &Path) -> Result<Subject, SubjectError> {
    let mut file = File::open(path).map_err(SubjectError::Read)?;
    let mut hasher = Sha256::new();
    let mut chunk = [0_u8; 8192];
    loop {
        let read = file.read(&mut chunk).map_err(SubjectError::Read)?;
        let Some(bytes) = chunk.get(..read).filter(|b| !b.is_empty()) else {
            break;
        };
        hasher.update(bytes);
    }
    Ok(Subject::of(Method::Sha256, &to_hex(&hasher.finalize())))
}

/// The `git-commit` subject of `rev` in the repository holding `dir`: what CI ran.
///
/// # Errors
/// [`SubjectError::Git`] when git fails (`rev` names no commit, `dir` is in no repository) or
/// cannot answer in time; [`SubjectError::Answer`] for an answer that is not one object id.
pub fn git_commit(dir: &Path, rev: &str, timeout: Duration) -> Result<Subject, SubjectError> {
    let oid = resolve_commit(git::isolated(Command::new("git")), dir, rev, timeout)?;
    Subject::checked(Method::GitCommit, &oid).ok_or(SubjectError::Answer)
}

/// The commit `rev` names, as its full object id: `rev-parse --verify` with `rev` after
/// `--end-of-options`, so it is never read as an option.
fn resolve_commit(
    git: Command,
    dir: &Path,
    rev: &str,
    timeout: Duration,
) -> Result<String, SubjectError> {
    let peeled = format!("{rev}^{{commit}}");
    let args = ["rev-parse", "--verify", "--end-of-options", &peeled].map(OsStr::new);
    one_line(run(git, &args, dir, timeout)?, dir)
}

/// [`git::run`], its failure a [`SubjectError::Git`].
fn run(
    git: Command,
    args: &[&OsStr],
    dir: &Path,
    timeout: Duration,
) -> Result<git::Captured, SubjectError> {
    git::run(git, args, dir, timeout).map_err(SubjectError::Git)
}

/// `Ok` when git exited 0; otherwise [`GitError::Failed`] with what it said.
fn succeeded(captured: &git::Captured, dir: &Path) -> Result<(), SubjectError> {
    if captured.status.success() {
        return Ok(());
    }
    Err(SubjectError::Git(GitError::Failed {
        dir: dir.to_path_buf(),
        stderr: String::from_utf8_lossy(&captured.stderr).trim().to_owned(),
    }))
}

/// The one line git printed on success, trimmed; a failure is [`GitError::Failed`].
fn one_line(captured: git::Captured, dir: &Path) -> Result<String, SubjectError> {
    succeeded(&captured, dir)?;
    // An object id is ASCII; any other byte makes the answer one git should not give.
    let Ok(text) = String::from_utf8(captured.stdout) else {
        return Err(SubjectError::Answer);
    };
    match text.lines().collect::<Vec<_>>().as_slice() {
        [line] if !line.is_empty() => Ok((*line).to_owned()),
        _ => Err(SubjectError::Answer),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sha256_of_a_file_is_sha256_of_its_bytes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("plan.md");
        let bytes: Vec<u8> = (0..20_000_u32).flat_map(u32::to_le_bytes).collect();
        std::fs::write(&path, &bytes).expect("write");
        assert_eq!(sha256_file(&path).expect("hashed"), Subject::sha256(&bytes));
        assert_eq!(
            Subject::sha256(b"").as_str(),
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }
}
