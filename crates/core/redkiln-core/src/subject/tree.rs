//! The `git-tree` method (SPEC ID-09): the tree the human is looking at, the current index plus
//! tracked modifications with untracked files excluded, written on a temporary index (Loop
//! ADR-0105 clause 4, weigh-in's `git.rs`).
//!
//! The temporary index is a copy of the worktree's on the local temporary disk, never beside the
//! store or in the repository: git replaces an index by rename, and Cowork's folders refuse the
//! delete that cleaning it up would take. The worktree's own index is never written.

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use super::{Method, Subject, SubjectError, one_line, run, succeeded};
use crate::git;

/// The `git-tree` subject of the worktree holding `dir`: a gate on exactly this snapshot.
///
/// # Errors
/// [`SubjectError::Git`] when git fails (`dir` is in no worktree) or cannot answer in time;
/// [`SubjectError::TempIndex`] when the temporary index cannot be made;
/// [`SubjectError::Answer`] for an answer that is not one object id.
pub fn git_tree(dir: &Path, timeout: Duration) -> Result<Subject, SubjectError> {
    let git = || git::isolated(Command::new("git"));
    let ask = ["rev-parse", "--path-format=absolute", "--git-path", "index"].map(OsStr::new);
    let index = PathBuf::from(one_line(run(git(), &ask, dir, timeout)?, dir)?);
    let tmp = tempfile::tempdir().map_err(SubjectError::TempIndex)?;
    let copy = tmp.path().join("index");
    let with_copy = || {
        let mut command = git();
        command.env("GIT_INDEX_FILE", &copy);
        command
    };
    match std::fs::copy(&index, &copy) {
        Ok(_) => {}
        // A repository with no commit and nothing staged has no index yet.
        Err(missing) if missing.kind() == std::io::ErrorKind::NotFound => {
            let empty = ["read-tree", "--empty"].map(OsStr::new);
            succeeded(&run(with_copy(), &empty, dir, timeout)?, dir)?;
        }
        Err(other) => return Err(SubjectError::TempIndex(other)),
    }
    let top = ["rev-parse", "--show-toplevel"].map(OsStr::new);
    let root = PathBuf::from(one_line(run(git(), &top, dir, timeout)?, dir)?);
    let update = ["add", "-u"].map(OsStr::new);
    succeeded(&run(with_copy(), &update, &root, timeout)?, dir)?;
    let write = [OsStr::new("write-tree")];
    let oid = one_line(run(with_copy(), &write, dir, timeout)?, dir)?;
    tmp.close().map_err(SubjectError::TempIndex)?;
    Subject::checked(Method::GitTree, &oid).ok_or(SubjectError::Answer)
}
