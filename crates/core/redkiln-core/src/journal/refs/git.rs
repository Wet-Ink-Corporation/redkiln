//! The git plumbing journal refs are written with (SPEC JR-07): objects, refs, fetches and pushes,
//! each through Core's bounded runner (`crate::git`), under a per-call timeout.
//!
//! git runs without an interactive prompt (`GIT_TERMINAL_PROMPT=0`, `GCM_INTERACTIVE=never`), so
//! a push that wants credentials fails instead of waiting, and commits under a fixed identity,
//! so a container with no `user.name` can still write: a journal commit is machine data.
//!
//! A step that talks to the remote (`ls-remote`, `fetch`, `push`) reports every failure as an
//! outcome, a timeout included, so a hung remote defers a journal instead of failing the
//! checkpoint; a local step's failure is a [`RefsError`].

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
    /// Any other failure, a timeout included, with what git said.
    Failed(String),
}

/// How a push ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Pushed {
    Done,
    /// git's own `[rejected]`: not a fast-forward, or a stale lease. Worth a retry.
    Rejected,
    /// Any other failure (a proxy's 403, a hook's refusal, no network, a timeout), with what
    /// git said.
    Failed(String),
}

/// What a ref holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Resolved {
    Absent,
    Commit(Oid),
    /// It exists but does not name a commit.
    NotACommit,
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
        let os: Vec<&OsStr> = args.iter().map(OsStr::new).collect();
        git::run(command, &os, self.dir, self.timeout).map_err(|source| step_error(args, source))
    }

    /// A step that talks to the remote: what git left, or what went wrong as a sentence.
    fn run_remote(&self, args: &[&str]) -> Result<Result<Captured, String>, RefsError> {
        match self.run(Self::command(), args) {
            Ok(captured) => Ok(Ok(captured)),
            Err(RefsError::Git {
                source: source @ (GitError::Timeout { .. } | GitError::TooMuchOutput),
                ..
            }) => Ok(Err(source.to_string())),
            Err(e) => Err(e),
        }
    }

    /// `git <args>`, which must succeed; its stdout.
    fn ok(&self, args: &[&str]) -> Result<Vec<u8>, RefsError> {
        let captured = self.run(Self::command(), args)?;
        self.succeeded(args, captured)
    }

    fn succeeded(&self, args: &[&str], captured: Captured) -> Result<Vec<u8>, RefsError> {
        if captured.status.success() {
            Ok(captured.stdout)
        } else {
            Err(step_error(
                args,
                GitError::Failed {
                    dir: self.dir.to_path_buf(),
                    stderr: trimmed(&captured.stderr),
                },
            ))
        }
    }

    /// `git <args>` reading `stdin`, which must succeed; its stdout.
    fn ok_with_input(&self, args: &[&str], stdin: &[u8]) -> Result<Vec<u8>, RefsError> {
        let io = |e| step_error(args, GitError::Io(e));
        let mut input = tempfile::tempfile().map_err(io)?;
        input.write_all(stdin).map_err(io)?;
        input.seek(SeekFrom::Start(0)).map_err(io)?;
        let mut command = Self::command();
        command.args(args);
        let mut spooled = git::run_spooled(command, self.dir, self.timeout, Some(input))
            .map_err(|source| step_error(args, source))?;
        let stdout =
            git::read_back(&mut spooled.stdout).map_err(|source| step_error(args, source))?;
        self.succeeded(
            args,
            Captured {
                status: spooled.status,
                stdout,
                stderr: spooled.stderr,
            },
        )
    }

    /// What the ref `name` holds.
    pub(crate) fn resolve(&self, name: &str) -> Result<Resolved, RefsError> {
        let spec = format!("{name}^{{commit}}");
        let captured = self.run(Self::command(), &["rev-parse", "-q", "--verify", &spec])?;
        if captured.status.success() {
            return Oid::from_output(&captured.stdout).map(Resolved::Commit);
        }
        let exists = self.run(Self::command(), &["show-ref", "--verify", "-q", name])?;
        Ok(if exists.status.success() {
            Resolved::NotACommit
        } else {
            Resolved::Absent
        })
    }

    /// The commit `name` points to, or `None` when it names none.
    pub(crate) fn commit_of(&self, name: &str) -> Result<Option<Oid>, RefsError> {
        Ok(match self.resolve(name)? {
            Resolved::Commit(oid) => Some(oid),
            Resolved::Absent | Resolved::NotACommit => None,
        })
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

    /// The entries of `commit`'s root tree, or of `path` in it; `None` when the listing is
    /// longer than git's output cap.
    pub(crate) fn entries(
        &self,
        commit: &Oid,
        path: Option<&str>,
    ) -> Result<Option<Vec<Entry>>, RefsError> {
        let mut args = vec!["ls-tree", "-z", "--full-tree", commit.as_str()];
        if let Some(path) = path {
            args.extend(["--", path]);
        }
        let out = match self.ok(&args) {
            Ok(out) => out,
            Err(RefsError::Git {
                source: GitError::TooMuchOutput,
                ..
            }) => return Ok(None),
            Err(e) => return Err(e),
        };
        out.split(|b| *b == 0)
            .filter(|record| !record.is_empty())
            .map(parse_entry)
            .collect::<Result<Vec<_>, _>>()
            .map(Some)
    }

    /// The blob `oid`, or `None` when it is larger than `cap` bytes; its size is checked
    /// before any of it is read.
    pub(crate) fn blob(&self, oid: &Oid, cap: u64) -> Result<Option<Vec<u8>>, RefsError> {
        let size = self.ok(&["cat-file", "-s", oid.as_str()])?;
        let size: u64 = String::from_utf8_lossy(&size)
            .trim()
            .parse()
            .map_err(|_parse| answer("cat-file -s", &size))?;
        if size > cap {
            return Ok(None);
        }
        let args = ["cat-file", "blob", oid.as_str()];
        let mut command = Self::command();
        command.args(args);
        let mut spooled = git::run_spooled(command, self.dir, self.timeout, None)
            .map_err(|source| step_error(&args, source))?;
        if !spooled.status.success() {
            return Err(step_error(
                &args,
                GitError::Failed {
                    dir: self.dir.to_path_buf(),
                    stderr: trimmed(&spooled.stderr),
                },
            ));
        }
        let mut bytes = Vec::new();
        (&mut spooled.stdout)
            .take(cap.saturating_add(1))
            .read_to_end(&mut bytes)
            .map_err(|e| step_error(&args, GitError::Io(e)))?;
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
        let io = |e| step_error(&["read-tree"], GitError::Io(e));
        let dir = tempfile::tempdir().map_err(io)?;
        let index = dir.path().join("index");
        let indexed = |args: &[&str]| {
            let mut command = Self::command();
            command.env("GIT_INDEX_FILE", &index);
            self.run(command, args)
                .and_then(|c| self.succeeded(args, c))
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

    /// Moves the local ref `name` to `new` only from `old` (`None`: only when it does not
    /// exist); `false`, and nothing moved, when another writer moved it meanwhile.
    pub(crate) fn move_ref(
        &self,
        name: &str,
        new: &Oid,
        old: Option<&Oid>,
    ) -> Result<bool, RefsError> {
        let args = [
            "update-ref",
            name,
            new.as_str(),
            old.map_or("", Oid::as_str),
        ];
        let captured = self.run(Self::command(), &args)?;
        if captured.status.success() {
            return Ok(true);
        }
        if String::from_utf8_lossy(&captured.stderr).contains("cannot lock ref") {
            return Ok(false);
        }
        self.succeeded(&args, captured).map(|_| false)
    }

    /// Deletes the local ref `name`, if it exists.
    pub(crate) fn delete_ref(&self, name: &str) -> Result<(), RefsError> {
        if self.resolve(name)? == Resolved::Absent {
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

    /// The branch the remote's `HEAD` names, `Err` with what went wrong when it cannot say.
    pub(crate) fn default_branch(&self, remote: &str) -> Result<Result<String, String>, RefsError> {
        let captured = match self.run_remote(&["ls-remote", "--symref", remote, "HEAD"])? {
            Ok(captured) if captured.status.success() => captured,
            Ok(captured) => return Ok(Err(trimmed(&captured.stderr))),
            Err(said) => return Ok(Err(said)),
        };
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
        let captured = match self.run_remote(&args)? {
            Ok(captured) => captured,
            Err(said) => return Ok(Fetched::Failed(said)),
        };
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
        let captured = match self.run_remote(&args)? {
            Ok(captured) => captured,
            Err(said) => return Ok(Pushed::Failed(said)),
        };
        if captured.status.success() {
            return Ok(Pushed::Done);
        }
        let stdout = String::from_utf8_lossy(&captured.stdout);
        let rejected = stdout
            .lines()
            .any(|line| line.starts_with('!') && line.contains("\t[rejected]"));
        if rejected {
            return Ok(Pushed::Rejected);
        }
        let said = trimmed(&captured.stderr);
        Ok(Pushed::Failed(if said.is_empty() {
            trimmed(&captured.stdout)
        } else {
            said
        }))
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
    let bad = || answer("ls-tree", record);
    let text = std::str::from_utf8(record).map_err(|_utf8| bad())?;
    let (meta, path) = text.split_once('\t').ok_or_else(bad)?;
    match meta.split(' ').collect::<Vec<_>>().as_slice() {
        [mode, kind, oid] => Ok(Entry {
            mode: (*mode).to_owned(),
            kind: (*kind).to_owned(),
            oid: Oid::parse(oid)?,
            path: path.to_owned(),
        }),
        _ => Err(bad()),
    }
}

/// A git failure, naming the subcommand that failed.
fn step_error(args: &[&str], source: GitError) -> RefsError {
    RefsError::Git {
        step: args
            .first()
            .map_or_else(String::new, |step| (*step).to_owned()),
        source,
    }
}

fn answer(step: &'static str, got: &[u8]) -> RefsError {
    RefsError::Answer {
        step,
        got: String::from_utf8_lossy(got).into_owned(),
    }
}

fn trimmed(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).trim().to_owned()
}
