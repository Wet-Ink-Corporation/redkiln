//! Journal refs (SPEC JR-05 to JR-08, OW-13): each journal lives in its own append-only ref,
//! `refs/redkiln/journal/<name>`, whose commit's tree holds exactly `<name>.jsonl`.
//!
//! [`checkpoint_refs`] exports the store's durable events into them. Where the writer may push a
//! ref it does so directly, fetching and merging first; where it may not (Claude Code cloud
//! sessions' git proxy refuses anything outside `refs/heads/*`), it pushes the inbox branch
//! `redkiln/inbox/<name>` instead, which [`relay`] (run by a repository's relay workflow) merges
//! into the ref and deletes. Every write is a merge by `origin` (JR-07), so it only appends.
//! [`import_refs`] reads them back, with refs set aside as broken and inboxes not yet relayed.
//!
//! A network failure, a timeout included, is an outcome, not an error: the events stay in the
//! store, and the next checkpoint exports them (JR-07). So is a journal ref or inbox that cannot
//! be read whole. [`RefsError`] is kept for the store and local git.

mod checkpoint;
mod direct;
mod git;
mod oid;
mod read;
mod relay;

use std::path::Path;
use std::time::{Duration, Instant};

pub use checkpoint::{Deferral, RefsCheckpoint, RefsExported, checkpoint_refs};
pub use read::{BROKEN_REFS, ImportedRef, RefsImport, import_refs};
pub use relay::{Deleted, Refusal, Relayed, relay};

use self::git::{Repo, Resolved};
use self::oid::Oid;
use super::export::MAX_JOURNAL_BYTES;
use super::import::ImportError;
use super::name::JournalName;
use crate::git::GitError;
use crate::store::StoreError;

/// Where journal refs live: `refs/redkiln/journal/<name>`.
pub const JOURNAL_REFS: &str = "refs/redkiln/journal/";
/// Where inbox branches live on the remote: `refs/heads/redkiln/inbox/<name>`.
pub const INBOX_BRANCHES: &str = "refs/heads/redkiln/inbox/";
/// Where a clone keeps its copies of the inbox branches: `refs/redkiln/inbox/<name>`.
pub const INBOX_REFS: &str = "refs/redkiln/inbox/";
/// Where an inbox commit puts its journal: `.redkiln/inbox/<name>.jsonl`.
pub const INBOX_DIR: &str = ".redkiln/inbox/";
/// Where a checkpoint keeps the remote default branch's tip, an inbox commit's parent.
const BASE_REF: &str = "refs/redkiln/base";
/// git's mode of a regular, non-executable file: the only kind a journal is read from.
pub(crate) const REGULAR: &str = "100644";
/// How many times a push rejected as not a fast-forward or a stale lease is retried (JR-07).
pub const RETRIES: u32 = 3;

/// Why a journal ref, or an inbox branch, was not written.
#[derive(Debug, thiserror::Error)]
pub enum RefsError {
    /// The store could not be read.
    #[error("reading the store")]
    Store(#[source] StoreError),
    /// git could not run, or failed at a local step (an object or a local ref).
    #[error("running git {step}")]
    Git {
        /// The git subcommand.
        step: String,
        /// What went wrong.
        #[source]
        source: GitError,
    },
    /// git printed something the step cannot read.
    #[error("git {step} printed `{got}`")]
    Answer {
        /// The git subcommand, or what was being read.
        step: &'static str,
        /// What it printed.
        got: String,
    },
    /// An import from git stopped; the imports stored before it stay.
    #[error("importing a journal from git")]
    Import(#[source] ImportError),
    /// The branch given to [`relay`] is not an inbox branch.
    #[error("`{0}` is not an inbox branch, redkiln/inbox/<name>")]
    NotInbox(String),
}

/// How a journal ref breaks JR-05, so no writer moves it until a person sets it aside to
/// `refs/redkiln/broken/<name>/<sha>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Breach {
    /// A commit reachable from it has more than one parent: not a single-parent chain.
    Merge,
    /// It does not name a commit, or its tree holds something other than exactly the regular
    /// file `<name>.jsonl`.
    Tree,
    /// Its file is not empty and its last byte is not `\n`.
    Unended,
    /// Its file is larger than [`MAX_JOURNAL_BYTES`], so it cannot be merged.
    TooLarge,
}

/// Where journal refs are written: a repository, the remote it pushes to and fetches from, and
/// how long each git call and the whole write may take.
#[derive(Debug, Clone, Copy)]
pub struct RefsTarget<'a> {
    /// The repository: any directory in its worktree.
    pub repo: &'a Path,
    /// The remote's name, `origin` as a rule.
    pub remote: &'a str,
    /// How long one git call may take before it is killed.
    pub timeout: Duration,
    /// After this, no journal is started; `None` waits as long as the calls take.
    pub deadline: Option<Instant>,
}

/// A [`RefsTarget`] as the writers use it.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Ctx<'a> {
    pub(crate) repo: Repo<'a>,
    /// The remote to push to; `None` writes local refs only.
    pub(crate) remote: Option<&'a str>,
    pub(crate) deadline: Option<Instant>,
    /// The largest journal file read or written: [`MAX_JOURNAL_BYTES`], smaller in tests.
    pub(crate) max_bytes: u64,
}

impl<'a> Ctx<'a> {
    /// `target`, with no remote until the caller has checked it is configured.
    pub(crate) fn local(target: &RefsTarget<'a>) -> Self {
        Self {
            repo: Repo {
                dir: target.repo,
                timeout: target.timeout,
            },
            remote: None,
            deadline: target.deadline,
            max_bytes: MAX_JOURNAL_BYTES,
        }
    }

    /// Whether the deadline has passed.
    pub(crate) fn late(&self) -> bool {
        self.deadline
            .is_some_and(|deadline| Instant::now() >= deadline)
    }
}

/// A journal ref as read (JR-05).
#[derive(Debug)]
pub(crate) enum State {
    Absent,
    Good { tip: Oid, file: Vec<u8> },
    Broken(Breach),
}

pub(crate) fn journal_ref(name: &JournalName) -> String {
    format!("{JOURNAL_REFS}{name}")
}

pub(crate) fn inbox_branch(name: &JournalName) -> String {
    format!("{INBOX_BRANCHES}{name}")
}

pub(crate) fn inbox_ref(name: &JournalName) -> String {
    format!("{INBOX_REFS}{name}")
}

pub(crate) fn inbox_path(name: &JournalName) -> String {
    format!("{INBOX_DIR}{}", name.file_name())
}

/// The local journal ref of `name`, checked against JR-05.
pub(crate) fn read_journal(ctx: &Ctx<'_>, name: &JournalName) -> Result<State, RefsError> {
    let repo = ctx.repo;
    let tip = match repo.resolve(&journal_ref(name))? {
        Resolved::Absent => return Ok(State::Absent),
        Resolved::NotACommit => return Ok(State::Broken(Breach::Tree)),
        Resolved::Commit(tip) => tip,
    };
    if repo.has_merge(&tip)? {
        return Ok(State::Broken(Breach::Merge));
    }
    let Some(entries) = repo.entries(&tip, None)? else {
        return Ok(State::Broken(Breach::Tree));
    };
    let blob = match entries.as_slice() {
        [only] if only.mode == REGULAR && only.kind == "blob" && only.path == name.file_name() => {
            &only.oid
        }
        _ => return Ok(State::Broken(Breach::Tree)),
    };
    let Some(file) = repo.blob(blob, ctx.max_bytes)? else {
        return Ok(State::Broken(Breach::TooLarge));
    };
    if file.last().is_some_and(|last| *last != b'\n') {
        return Ok(State::Broken(Breach::Unended));
    }
    Ok(State::Good { tip, file })
}

/// The clone's copy of an inbox branch as read.
#[derive(Debug)]
pub(crate) enum InboxCopy {
    Absent,
    /// Its tip and its file (empty when it holds none).
    Read {
        tip: Oid,
        file: Vec<u8>,
    },
    /// Its file is larger than the context's cap, so it cannot be merged.
    TooLarge,
}

/// The local copy of `name`'s inbox branch.
pub(crate) fn read_inbox(ctx: &Ctx<'_>, name: &JournalName) -> Result<InboxCopy, RefsError> {
    let repo = ctx.repo;
    let Some(tip) = repo.commit_of(&inbox_ref(name))? else {
        return Ok(InboxCopy::Absent);
    };
    let path = inbox_path(name);
    let file = match repo.entries(&tip, Some(&path))?.as_deref() {
        Some([entry]) if entry.mode == REGULAR && entry.kind == "blob" && entry.path == path => {
            match repo.blob(&entry.oid, ctx.max_bytes)? {
                Some(file) => file,
                None => return Ok(InboxCopy::TooLarge),
            }
        }
        _ => Vec::new(),
    };
    Ok(InboxCopy::Read { tip, file })
}
