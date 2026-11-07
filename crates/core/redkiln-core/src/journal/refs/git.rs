//! The git plumbing journal refs are written with (SPEC JR-07): objects, refs, fetches and pushes,
//! each through Core's bounded runner (`crate::git`), under a per-call timeout.
//!
//! git runs without an interactive prompt (`GIT_TERMINAL_PROMPT=0`, `GCM_INTERACTIVE=never`), so
//! a push that wants credentials fails instead of waiting, and commits under a fixed identity,
//! so a container with no `user.name` can still write: a journal commit is machine data.

use std::ffi::OsStr;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use super::{RefsError, oid::Oid};
use crate::git::{self, Captured, GitError};

/// The identity journal commits are made under.
const NAME: &str = "redkiln";
const EMAIL: &str = "journal@redkiln.invalid";

/// git in one repository, every call bounded by `timeout`.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Repo<'a> {
    pub(crate) dir: &'a Path,
    pub(crate) timeout: Duration,
}

/// One entry of a tree, as `ls-tree` prints it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Entry {
    pub(crate) mode: String,
    pub(crate) kind: String,
    pub(crate) oid: Oid,
    pub(crate) path: String,
}

/// How a fetch ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Fetched {
    Done,
    /// A ref named without a wildcard is not on the remote.
    Missing,
    /// Any other failure, with what git said.
    Failed(String),
}

/// How a push ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Pushed {
    Done,
    /// git's own `[rejected]`: not a fast-forward, or a stale lease. Worth a retry.
    Rejected,
    /// Any other failure (a proxy's 403, a hook's refusal, no network), with what git said.
    Failed(String),
}

impl Repo<'_> {
    fn command() -> Command {
        let mut command = git::isolated(Command::new("git"));
        command
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GCM_INTERACTIVE", "never")
            .env("GIT_AUTHOR_NAME", NAME)
            .env("GIT_AUTHOR_EMAIL", EMAIL)
            .env("GIT_COMMITTER_NAME", NAME)
            .env("GIT_COMMITTER_EMAIL", EMAIL);
        command
    }

    fn run(&self, command: Command, args: &[&str]) -> Result<Captured, RefsError> {
        let args: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
        git::run(command, &args, self.dir, self.timeout).map_err(RefsError::Git)
    }

    /// `git <args>`, which must succeed; its stdout.
    fn ok(&self, args: &[&str]) -> Result<Vec<u8>, RefsError> {
        let captured = self.run(Self::command(), args)?;
        self.succeeded(captured)
    }

    fn succeeded(&self, captured: Captured) -> Result<Vec<u8>, RefsError> {
        if captured.status.success() {
            Ok(captured.stdout)
        } else {
            Err(RefsError::Git(GitError::Failed {
                dir: self.dir.to_path_buf(),
                stderr: trimmed(&captured.stderr),
            }))
        }
    }

    /// `git <args>` reading `stdin`, which must succeed; its stdout.
    fn ok_with_input(&self, args: &[&str], stdin: &[u8]) -> Result<Vec<u8>, RefsError> {
        let io = |e| RefsError::Git(GitError::Io(e));
        let mut input = tempfile::tempfile().map_err(io)?;
        input.write_all(stdin).map_err(io)?;
        input.seek(SeekFrom::Start(0)).map_err(io)?;
        let mut command = Self::command();
        command.args(args);
        let mut spooled = git::run_spooled(command, self.dir, self.timeout, Some(input))
            .map_err(RefsError::Git)?;
        let stdout = git::read_back(&mut spooled.stdout).map_err(RefsError::Git)?;
        self.succeeded(Captured {
            status: spooled.status,
            stdout,
            stderr: spooled.stderr,
        })
    }

    /// The commit `name` points to, or `None` when it does not exist.
    pub(crate) fn resolve(&self, name: &str) -> Result<Option<Oid>, RefsError> {
        let spec = format!("{name}^{{commit}}");
        let captured = self.run(Self::command(), &["rev-parse", "-q", "--verify", &spec])?;
        if !captured.status.success() {
            return Ok(None);
        }
        Oid::from_output(&captured.stdout).map(Some)
    }

    /// Whether any commit reachable from `tip` has more than one parent.
    pub(crate) fn has_merge(&self, tip: &Oid) -> Result<bool, RefsError> {
        let out = self.ok(&["rev-list", "--min-parents=2", "-n1", tip.as_str()])?;
        Ok(!out.trim_ascii().is_empty())
    }

    /// `commit`'s parents.
    pub(crate) fn parents(&self, commit: &Oid) -> Result<Vec<Oid>, RefsError> {
        let out = self.ok(&["rev-list", "--parents", "-n1", commit.as_str()])?;
        let text = String::from_utf8_lossy(&out);
        text.split_ascii_whitespace()
            .skip(1)
            .map(Oid::parse)
            .collect()
    }

    /// The entries of `commit`'s root tree, or of `path` in it.
    pub(crate) fn entries(
        &self,
        commit: &Oid,
        path: Option<&str>,
    ) -> Result<Vec<Entry>, RefsError> {
        let mut args = vec!["ls-tree", "-z", "--full-tree", commit.as_str()];
        if let Some(path) = path {
            args.extend(["--", path]);
        }
        let out = self.ok(&args)?;
        out.split(|b| *b == 0)
            .filter(|record| !record.is_empty())
            .map(parse_entry)
            .collect()
    }

    /// The blob `oid`, at most `cap` bytes; `None` when it is larger.
    pub(crate) fn blob(&self, oid: &Oid, cap: u64) -> Result<Option<Vec<u8>>, RefsError> {
        let io = |e| RefsError::Git(GitError::Io(e));
        let mut command = Self::command();
        command.args(["cat-file", "blob", oid.as_str()]);
        let mut spooled =
            git::run_spooled(command, self.dir, self.timeout, None).map_err(RefsError::Git)?;
        if !spooled.status.success() {
            return Err(RefsError::Git(GitError::Failed {
                dir: self.dir.to_path_buf(),
                stderr: trimmed(&spooled.stderr),
            }));
        }
        let mut bytes = Vec::new();
        (&mut spooled.stdout)
            .take(cap.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(io)?;
        Ok((u64::try_from(bytes.len()).unwrap_or(u64::MAX) <= cap).then_some(bytes))
    }

    /// Writes `bytes` as a blob.
    pub(crate) fn hash_blob(&self, bytes: &[u8]) -> Result<Oid, RefsError> {
        Oid::from_output(&self.ok_with_input(&["hash-object", "-w", "--stdin"], bytes)?)
    }

    /// A tree holding the one regular file `path` with `blob`.
    pub(crate) fn single_file_tree(&self, path: &str, blob: &Oid) -> Result<Oid, RefsError> {
        let input = format!("100644 blob {blob}\t{path}\0");
        Oid::from_output(&self.ok_with_input(&["mktree", "-z"], input.as_bytes())?)
    }

    /// `base`'s tree with the regular file `path` set to `blob`, built in a temporary index.
    pub(crate) fn tree_with(&self, base: &Oid, path: &str, blob: &Oid) -> Result<Oid, RefsError> {
        let io = |e| RefsError::Git(GitError::Io(e));
        let dir = tempfile::tempdir().map_err(io)?;
        let index = dir.path().join("index");
        let indexed = |args: &[&str]| {
            let mut command = Self::command();
            command.env("GIT_INDEX_FILE", &index);
            self.run(command, args).and_then(|c| self.succeeded(c))
        };
        indexed(&["read-tree", base.as_str()])?;
        let info = format!("100644,{blob},{path}");
        indexed(&["update-index", "--add", "--cacheinfo", &info])?;
        let tree = Oid::from_output(&indexed(&["write-tree"])?)?;
        dir.close().map_err(io)?;
        Ok(tree)
    }

    /// A commit of `tree` with `parent`, or a root commit.
    pub(crate) fn commit(
        &self,
        tree: &Oid,
        parent: Option<&Oid>,
        message: &str,
    ) -> Result<Oid, RefsError> {
        let mut args = vec!["commit-tree", tree.as_str(), "-m", message];
        if let Some(parent) = parent {
            args.extend(["-p", parent.as_str()]);
        }
        Oid::from_output(&self.ok(&args)?)
    }

    /// Points the local ref `name` at `commit`.
    pub(crate) fn set_ref(&self, name: &str, commit: &Oid) -> Result<(), RefsError> {
        self.ok(&["update-ref", name, commit.as_str()]).map(drop)
    }

    /// Deletes the local ref `name`, if it exists.
    pub(crate) fn delete_ref(&self, name: &str) -> Result<(), RefsError> {
        if self.resolve(name)?.is_none() {
            return Ok(());
        }
        self.ok(&["update-ref", "-d", name]).map(drop)
    }

    /// Whether `remote` names a configured remote with a URL.
    pub(crate) fn has_remote(&self, remote: &str) -> Result<bool, RefsError> {
        let key = format!("remote.{remote}.url");
        let captured = self.run(Self::command(), &["config", "--get", &key])?;
        Ok(captured.status.success() && !captured.stdout.trim_ascii().is_empty())
    }

    /// The branch the remote's `HEAD` names, `Err` with what git said when it cannot say.
    pub(crate) fn default_branch(&self, remote: &str) -> Result<Result<String, String>, RefsError> {
        let captured = self.run(Self::command(), &["ls-remote", "--symref", remote, "HEAD"])?;
        if !captured.status.success() {
            return Ok(Err(trimmed(&captured.stderr)));
        }
        let text = String::from_utf8_lossy(&captured.stdout);
        let branch = text.lines().find_map(|line| {
            line.strip_prefix("ref: refs/heads/")
                .and_then(|rest| rest.strip_suffix("\tHEAD"))
                .map(str::to_owned)
        });
        Ok(branch.ok_or_else(|| "the remote names no default branch".to_owned()))
    }

    /// `git fetch` of `specs` from `remote`, pruning their destinations when `prune`.
    pub(crate) fn fetch(
        &self,
        remote: &str,
        specs: &[String],
        prune: bool,
    ) -> Result<Fetched, RefsError> {
        let mut args = vec![
            "fetch",
            "-q",
            "--no-tags",
            "--no-write-fetch-head",
            "--no-auto-gc",
        ];
        if prune {
            args.push("--prune");
        }
        args.push(remote);
        args.extend(specs.iter().map(String::as_str));
        let captured = self.run(Self::command(), &args)?;
        if captured.status.success() {
            return Ok(Fetched::Done);
        }
        let said = trimmed(&captured.stderr);
        if said.contains("couldn't find remote ref") {
            Ok(Fetched::Missing)
        } else {
            Ok(Fetched::Failed(said))
        }
    }

    /// `git push` of `spec` to `remote`, with `--force-with-lease=<lease>` when given.
    pub(crate) fn push(
        &self,
        remote: &str,
        spec: &str,
        lease: Option<&str>,
    ) -> Result<Pushed, RefsError> {
        let lease = lease.map(|lease| format!("--force-with-lease={lease}"));
        let mut args = vec!["push", "--porcelain", "--no-verify", "-q"];
        if let Some(lease) = &lease {
            args.push(lease);
        }
        args.extend([remote, spec]);
        let captured = self.run(Self::command(), &args)?;
        if captured.status.success() {
            return Ok(Pushed::Done);
        }
        let stdout = String::from_utf8_lossy(&captured.stdout);
        let rejected = stdout
            .lines()
            .any(|line| line.starts_with('!') && line.contains("\t[rejected]"));
        if rejected {
            Ok(Pushed::Rejected)
        } else {
            let said = trimmed(&captured.stderr);
            Ok(Pushed::Failed(if said.is_empty() {
                trimmed(&captured.stdout)
            } else {
                said
            }))
        }
    }

    /// The paths `commit` changes against `parent`, as `diff-tree --raw -z` prints them.
    pub(crate) fn changes(&self, parent: &Oid, commit: &Oid) -> Result<Vec<u8>, RefsError> {
        self.ok(&[
            "diff-tree",
            "-r",
            "--no-renames",
            "--raw",
            "-z",
            parent.as_str(),
            commit.as_str(),
        ])
    }
}

/// One `ls-tree -z` record: `<mode> <kind> <oid>\t<path>`.
fn parse_entry(record: &[u8]) -> Result<Entry, RefsError> {
    let Ok(text) = std::str::from_utf8(record) else {
        return Err(answer("ls-tree"));
    };
    let (meta, path) = text.split_once('\t').ok_or_else(|| answer("ls-tree"))?;
    match meta.split(' ').collect::<Vec<_>>().as_slice() {
        [mode, kind, oid] => Ok(Entry {
            mode: (*mode).to_owned(),
            kind: (*kind).to_owned(),
            oid: Oid::parse(oid)?,
            path: path.to_owned(),
        }),
        _ => Err(answer("ls-tree")),
    }
}

fn answer(command: &str) -> RefsError {
    RefsError::Answer {
        command: command.to_owned(),
    }
}

fn trimmed(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_owned()
}
