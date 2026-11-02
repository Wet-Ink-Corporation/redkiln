//! The `patch-id` method (SPEC ID-09): `git patch-id --verbatim` of a diff pinned so that its id
//! depends only on the change, and no id at all when the diff cannot be taken whole.
//!
//! The pipeline is never run as written: the diff is spooled to an anonymous file on the
//! temporary disk, each process's status is checked, and only then is the spool fed to
//! patch-id. A diff that died part way (a missing object in a partial clone, a corrupt pack)
//! would otherwise give the id of a smaller change.

use std::ffi::OsStr;
use std::fs::File;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use super::{Method, Subject, SubjectError, one_line, resolve_commit, run};
use crate::git::{self, GitError};

/// The configuration the recipe pins, as `git -c` arguments; `core.useReplaceRefs=false` also
/// goes to every other git call it makes, so a replace ref changes neither the merge base nor
/// the diff.
const PINNED_CONFIG: [&str; 8] = [
    "-c",
    "core.quotePath=true",
    "-c",
    "diff.suppressBlankEmpty=false",
    "-c",
    "core.useReplaceRefs=false",
    "-c",
    "advice.graftFileDeprecated=false",
];

/// The diff's pinned flags, before `<base>...<commit>`.
const DIFF_FLAGS: [&str; 18] = [
    "diff",
    "--no-color",
    "--no-ext-diff",
    "--no-textconv",
    "--no-renames",
    "--no-relative",
    "--ignore-submodules=none",
    "--submodule=short",
    "--text",
    "--full-index",
    "-U3",
    "--inter-hunk-context=0",
    "--diff-algorithm=myers",
    "--indent-heuristic",
    "--src-prefix=a/",
    "--dst-prefix=b/",
    "-O/dev/null",
    // `<base>...<commit>` follows; both are full object ids by then.
    "--end-of-options",
];

/// Variables the recipe unsets: `GIT_DIFF_OPTS` overrides `-U`, `GIT_EXTERNAL_DIFF` names a
/// driver; the rest would point git at another object store or attribute source than the
/// repository's own.
const UNSET: [&str; 5] = [
    "GIT_DIFF_OPTS",
    "GIT_EXTERNAL_DIFF",
    "GIT_OBJECT_DIRECTORY",
    "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    "GIT_ATTR_SOURCE",
];

/// The oldest git whose `patch-id --verbatim` implies `--stable` (file order does not count).
const MIN_GIT: (u32, u32) = (2, 39);

/// The largest diff spooled: 1 GiB. A larger change has no id here rather than fill the disk.
const MAX_DIFF_BYTES: u64 = 1 << 30;

/// Why a change has no `patch-id` (ID-09). Nothing may bind to `patch-id:` then.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum NoPatchId {
    /// git is older than 2.39, whose `--verbatim` does not imply `--stable`.
    #[error("git {found} is older than 2.39")]
    OldGit {
        /// The version git reported.
        found: String,
    },
    /// The repository is shallow: its boundary can move the merge base.
    #[error("the repository is shallow; deepen or unshallow it first")]
    Shallow,
    /// `git merge-base --all` named other than one commit: none, or a criss-cross history.
    #[error("the base and commit have {found} merge bases, not one")]
    MergeBases {
        /// How many it named.
        found: usize,
    },
    /// `git diff` exited non-zero: it may have printed only part of the change.
    #[error("git diff failed: {stderr}")]
    DiffFailed {
        /// What git said, trimmed.
        stderr: String,
    },
    /// The diff is larger than this build spools.
    #[error("the diff is larger than {MAX_DIFF_BYTES} bytes")]
    TooLarge,
    /// `git patch-id` exited non-zero.
    #[error("git patch-id failed: {stderr}")]
    PatchIdFailed {
        /// What git said, trimmed.
        stderr: String,
    },
    /// The change is empty: patch-id printed nothing.
    #[error("the change is empty")]
    Empty,
    /// patch-id printed more than one line, or a line that is not an id.
    #[error("git patch-id printed something other than one id")]
    Output,
}

/// A `patch-id` subject and the base its diff was taken against, recorded together (ID-10).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PatchId {
    subject: Subject,
    base: String,
}

impl PatchId {
    /// The subject, `patch-id:<hash>`.
    #[must_use]
    pub const fn subject(&self) -> &Subject {
        &self.subject
    }

    /// The base commit, as its full object id.
    #[must_use]
    pub fn base(&self) -> &str {
        &self.base
    }
}

/// The `patch-id` of `commit` against `base` in the repository holding `dir`, by ID-09's pinned
/// recipe, with `base` resolved to its commit id.
///
/// # Errors
/// [`SubjectError::NoPatchId`] when ID-09 gives the change no id: git before 2.39, a shallow
/// repository, other than one merge base, a diff or patch-id that failed, an empty change, or
/// output that is not one id. [`SubjectError::Git`] when `base` or `commit` names no commit or
/// git cannot run or answer in time; [`SubjectError::Answer`] for an answer git should not give.
pub fn patch_id(
    dir: &Path,
    base: &str,
    commit: &str,
    timeout: Duration,
) -> Result<PatchId, SubjectError> {
    let git = || Command::new("git");
    patch_id_with(
        &Recipe {
            git: &git,
            dir,
            timeout,
        },
        base,
        commit,
    )
}

/// Where and how the recipe runs git.
struct Recipe<'a> {
    /// Makes the `Command` each call starts from: `git` with the caller's environment (the
    /// tests' seam: they set variables on it, as a shell would on the process).
    git: &'a dyn Fn() -> Command,
    dir: &'a Path,
    timeout: Duration,
}

impl Recipe<'_> {
    /// git under the recipe's environment and pinned configuration.
    fn pinned(&self) -> Command {
        let mut command = git::isolated((self.git)());
        for name in UNSET {
            command.env_remove(name);
        }
        command
            .env("GIT_GRAFT_FILE", "/dev/null")
            .args(PINNED_CONFIG);
        command
    }

    fn run(&self, args: &[&OsStr]) -> Result<git::Captured, SubjectError> {
        run(self.pinned(), args, self.dir, self.timeout)
    }

    /// Exactly one merge base of `base` and `commit`, as a criss-cross history has two.
    fn one_merge_base(&self, base: &str, commit: &str) -> Result<(), SubjectError> {
        let found = self.run(&["merge-base", "--all", base, commit].map(OsStr::new))?;
        let named = String::from_utf8_lossy(&found.stdout).lines().count();
        // `merge-base` exits 1, printing nothing, when there is no merge base.
        if named == 1 && found.status.success() {
            Ok(())
        } else {
            Err(no_id(NoPatchId::MergeBases { found: named }))
        }
    }

    /// The pinned diff of `base...commit`, in a rewound spool file.
    fn diff(&self, base: &str, commit: &str) -> Result<File, SubjectError> {
        let mut command = self.pinned();
        command.args(DIFF_FLAGS).arg(format!("{base}...{commit}"));
        let diff =
            git::run_spooled(command, self.dir, self.timeout, None).map_err(SubjectError::Git)?;
        if !diff.status.success() {
            return Err(no_id(NoPatchId::DiffFailed {
                stderr: trimmed(&diff.stderr),
            }));
        }
        let size = diff
            .stdout
            .metadata()
            .map_err(|e| SubjectError::Git(GitError::Io(e)))?;
        if size.len() > MAX_DIFF_BYTES {
            return Err(no_id(NoPatchId::TooLarge));
        }
        Ok(diff.stdout)
    }

    /// `git patch-id --verbatim` of the diff in `spool`: the first field of its one line.
    fn id_of(&self, spool: File) -> Result<Subject, SubjectError> {
        let mut command = self.pinned();
        command.args(["patch-id", "--verbatim"]);
        let mut spooled = git::run_spooled(command, self.dir, self.timeout, Some(spool))
            .map_err(SubjectError::Git)?;
        if !spooled.status.success() {
            return Err(no_id(NoPatchId::PatchIdFailed {
                stderr: trimmed(&spooled.stderr),
            }));
        }
        let stdout = git::read_back(&mut spooled.stdout).map_err(SubjectError::Git)?;
        let text = String::from_utf8_lossy(&stdout);
        let hash = match text.lines().collect::<Vec<_>>().as_slice() {
            [] => return Err(no_id(NoPatchId::Empty)),
            // The id is the first field: `<patch id> <commit id>`.
            [line] => line.split_once(' ').map_or(*line, |(id, _)| id),
            _ => return Err(no_id(NoPatchId::Output)),
        };
        Subject::checked(Method::PatchId, hash).ok_or(no_id(NoPatchId::Output))
    }
}

/// [`patch_id`] by `recipe`: each refusal of ID-09 in turn, then the diff and its id.
fn patch_id_with(recipe: &Recipe<'_>, base: &str, commit: &str) -> Result<PatchId, SubjectError> {
    check_version(recipe.pinned(), recipe.dir, recipe.timeout)?;
    let shallow = recipe.run(&["rev-parse", "--is-shallow-repository"].map(OsStr::new))?;
    if one_line(shallow, recipe.dir)? != "false" {
        return Err(no_id(NoPatchId::Shallow));
    }
    let base = resolve_commit(recipe.pinned(), recipe.dir, base, recipe.timeout)?;
    let commit = resolve_commit(recipe.pinned(), recipe.dir, commit, recipe.timeout)?;
    recipe.one_merge_base(&base, &commit)?;
    let subject = recipe.id_of(recipe.diff(&base, &commit)?)?;
    Ok(PatchId { subject, base })
}

/// Refuses git older than [`MIN_GIT`]: `git version 2.43.0` (Git for Windows adds
/// `.windows.1`).
fn check_version(git: Command, dir: &Path, timeout: Duration) -> Result<(), SubjectError> {
    let line = one_line(run(git, &[OsStr::new("--version")], dir, timeout)?, dir)?;
    version_of(&line).map_err(no_id)
}

/// `Ok` when `git --version`'s line names [`MIN_GIT`] or later; anything unreadable is refused.
fn version_of(line: &str) -> Result<(), NoPatchId> {
    let found = line.strip_prefix("git version ").unwrap_or(line);
    let mut parts = found.split('.').map(str::parse::<u32>);
    match (parts.next(), parts.next()) {
        (Some(Ok(major)), Some(Ok(minor))) if (major, minor) >= MIN_GIT => Ok(()),
        _ => Err(NoPatchId::OldGit {
            found: found.to_owned(),
        }),
    }
}

const fn no_id(why: NoPatchId) -> SubjectError {
    SubjectError::NoPatchId(why)
}

fn trimmed(stderr: &[u8]) -> String {
    String::from_utf8_lossy(stderr).trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TIMEOUT: Duration = Duration::from_secs(30);

    fn git(dir: &Path, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@example.invalid"])
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git runs");
        assert!(out.status.success(), "git {args:?}");
        String::from_utf8(out.stdout)
            .expect("utf-8")
            .trim()
            .to_owned()
    }

    /// A repository whose change has context lines: `(dir, base, head)`.
    fn change() -> (tempfile::TempDir, String, String) {
        let dir = tempfile::tempdir().expect("tempdir");
        let text = (1..=20)
            .map(|n| format!("line {n}"))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        std::fs::write(dir.path().join("a.txt"), &text).expect("write");
        git(dir.path(), &["init", "-q"]);
        git(dir.path(), &["add", "-A"]);
        git(dir.path(), &["commit", "-q", "-m", "base"]);
        let base = git(dir.path(), &["rev-parse", "HEAD"]);
        std::fs::write(dir.path().join("a.txt"), text.replace("line 10\n", "ten\n"))
            .expect("write");
        git(dir.path(), &["commit", "-q", "-am", "change"]);
        let head = git(dir.path(), &["rev-parse", "HEAD"]);
        (dir, base, head)
    }

    /// A plain `git diff | git patch-id --verbatim` under `env`: the control.
    fn plain(dir: &Path, base: &str, head: &str, env: &[(&str, &str)]) -> Vec<u8> {
        let diff = Command::new("git")
            .args(["diff", &format!("{base}...{head}")])
            .envs(env.iter().copied())
            .current_dir(dir)
            .output()
            .expect("diff");
        let mut spool = tempfile::tempfile().expect("spool");
        std::io::Write::write_all(&mut spool, &diff.stdout).expect("spool");
        std::io::Seek::rewind(&mut spool).expect("rewind");
        Command::new("git")
            .args(["patch-id", "--verbatim"])
            .stdin(spool)
            .current_dir(dir)
            .output()
            .expect("patch-id")
            .stdout
    }

    #[test]
    fn the_diff_environment_the_caller_inherited_never_reaches_the_recipe() {
        let (dir, base, head) = change();
        let clean = patch_id(dir.path(), &base, &head, TIMEOUT).expect("an id");
        let hostile = [
            [("GIT_DIFF_OPTS", "-u10")],
            [("GIT_EXTERNAL_DIFF", "false")],
            [("GIT_CONFIG_PARAMETERS", "'diff.context'='10'")],
        ];
        let unchanged = plain(dir.path(), &base, &head, &[]);
        for env in hostile {
            assert_ne!(
                plain(dir.path(), &base, &head, &env),
                unchanged,
                "{env:?}: the control moves"
            );
            let seam = || {
                let mut command = Command::new("git");
                command.envs(env.iter().copied());
                command
            };
            let recipe = Recipe {
                git: &seam,
                dir: dir.path(),
                timeout: TIMEOUT,
            };
            let id = patch_id_with(&recipe, &base, &head).expect("an id");
            assert_eq!(id, clean, "{env:?}");
        }
    }

    #[test]
    fn git_before_2_39_gives_no_id() {
        for (said, refused) in [
            ("git version 2.38.1", true),
            ("git version 1.99.0", true),
            ("git version 2.39.0", false),
            ("git version 2.43.0.windows.1", false),
            ("not git", true),
        ] {
            let found = version_of(said);
            assert_eq!(
                found.is_err_and(|e| matches!(e, NoPatchId::OldGit { .. })),
                refused,
                "{said}"
            );
        }
    }
}
