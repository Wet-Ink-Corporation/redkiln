//! `weighin.ci-check/2`, the CLI envelope `weighin ci-check --json` writes since #48.
//!
//! Its `data` is [`Data`] (SPEC CLI-01 to CLI-05). Beside `/1`'s report it says which journals
//! it read, committed files and the journal refs of the clone (journal, inbox and broken refs),
//! and counts the weighin events it could not read (EV-10).
//!
//! Field order is wire order. A reader accepts keys it does not know (CLI-04: additive within
//! `/2`); weighin builds a [`Data`] and serialises it, and Review reads it through
//! [`Data::from_envelope`].

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub use super::{Blocks, Source};
use crate::id::ItemId;
use crate::open::Status;
use crate::vocab::{Authority, Door, Kind};

/// The envelope's `schema`.
pub const SCHEMA: &str = "weighin.ci-check/2";
/// The envelope's `command`.
pub const COMMAND: &str = "ci-check";

/// The envelope's `data`: what a branch leaves open and decides, whether its journals were
/// only appended to, and what was read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Data {
    /// The base commit's full sha.
    pub base: String,
    /// The head commit's full sha.
    pub head: String,
    /// Items still open, by id: the committed journals at head and every journal source read.
    pub open_items: Vec<OpenItem>,
    /// The decisions the range adds, by (id, option); journal refs add none.
    pub decisions: Vec<Decision>,
    /// The append-only check of the committed journals and the journal refs.
    pub journal: JournalCheck,
    /// What was read.
    pub read: Read,
    /// Weighin events this build cannot read, skipped, by type (EV-10).
    pub unreadable: BTreeMap<String, u64>,
}

/// Why [`Data::from_envelope`] refused a document.
#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    /// The document is not a `/2` envelope, or its `data` is not this report's shape.
    #[error("not a {SCHEMA} envelope")]
    Shape(#[from] serde_json::Error),
    /// The envelope names another schema.
    #[error("the report is {found}, not {SCHEMA}")]
    Schema {
        /// The schema the envelope names.
        found: String,
    },
    /// The envelope is another command's.
    #[error("the envelope is of command {found}, not {COMMAND}")]
    Command {
        /// The command the envelope names.
        found: String,
    },
    /// ci-check could not run: the envelope says `ok: false`, and its `data` is empty.
    #[error("ci-check could not run: its envelope says ok false")]
    Failed,
}

/// The envelope keys a reader checks; the others (`findings`, `proves`) are passed over.
#[derive(Deserialize)]
struct EnvelopeIn {
    schema: String,
    ok: bool,
    command: String,
    data: serde_json::Value,
}

impl Data {
    /// The report in a whole `/2` envelope line, checked in order: shape, schema, command, ok.
    ///
    /// # Errors
    /// [`ReadError::Shape`] when it is not an envelope or its `data` not this shape,
    /// [`ReadError::Schema`] for another schema, [`ReadError::Command`] for another command,
    /// and [`ReadError::Failed`] when the run could not report.
    pub fn from_envelope(json: &str) -> Result<Self, ReadError> {
        let envelope: EnvelopeIn = serde_json::from_str(json)?;
        if envelope.schema != SCHEMA {
            return Err(ReadError::Schema {
                found: envelope.schema,
            });
        }
        if envelope.command != COMMAND {
            return Err(ReadError::Command {
                found: envelope.command,
            });
        }
        if !envelope.ok {
            return Err(ReadError::Failed);
        }
        Self::from_envelope_data(&envelope.data)
    }

    /// An envelope's `data`; the caller has checked its `schema` (as
    /// [`crate::status::Status::from_envelope_data`]).
    ///
    /// # Errors
    /// [`ReadError::Shape`] when `data` lacks a key, or a key holds another type or value.
    pub fn from_envelope_data(data: &serde_json::Value) -> Result<Self, ReadError> {
        Ok(Self::deserialize(data)?)
    }
}

/// An item still open: `/1`'s fields, as its own type so a later `/2` key reads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenItem {
    /// The item.
    pub id: ItemId,
    /// One of the open statuses.
    pub status: Status,
    /// Its kind.
    pub kind: Kind,
    /// The triaged door, or `null` before triage; never the raiser's hint.
    pub door: Option<Door>,
    /// The triaged authority, or `null` before triage; never the raiser's hint.
    pub authority: Option<Authority>,
    /// What it holds up.
    pub blocks: Blocks,
    /// Its deadline: the deferral's when deferred, else the triage's.
    pub resolve_by: Option<String>,
    /// The question as raised.
    pub question: String,
}

/// One decision the branch adds: `/1`'s fields, as its own type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decision {
    /// The item decided.
    pub id: ItemId,
    /// The option, when a source names one (atoms do not, B-28).
    pub option: Option<String>,
    /// The commit that records it: the trailer's, else the atom's, else the journal line's.
    pub commit: Option<String>,
    /// Where it is attested, each once, in [`Source`]'s order.
    pub sources: Vec<Source>,
}

/// The append-only result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct JournalCheck {
    /// No violation.
    pub ok: bool,
    /// Each break, in [`Violation`]'s order.
    pub violations: Vec<Violation>,
}

/// One break of the append-only rule. The derive order is the sort order and the field order.
///
/// A `file` violation names a committed journal by its repo-relative path and has no `commit`;
/// a `ref` violation names a full ref and always has one, unless it is `unreadable` (the ref,
/// or a ref prefix, could not be read whole).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Violation {
    /// A repo-relative path (kind `file`) or a full ref name (kind `ref`).
    pub journal: String,
    /// Which kind of journal.
    pub kind: JournalKind,
    /// The ref commit at fault; `null` for files and for `unreadable`.
    pub commit: Option<String>,
    /// The 1-based line: the base line for `missing` and `altered`, the head or ref-file line
    /// for `undecodable`; else `null`.
    pub line: Option<u64>,
    /// What is wrong.
    pub reason: Reason,
}

/// Which kind of journal a violation names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JournalKind {
    /// A journal committed under `.kb/_ledger/journal/`.
    File,
    /// A journal ref, inbox copy or broken ref.
    Ref,
}

/// Why a journal breaks the append-only rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Reason {
    /// The file is gone at head.
    Removed,
    /// A complete base line's event is nowhere in the head file.
    Missing,
    /// As `missing`, and the head line at that number holds an event the base file did not.
    Altered,
    /// A complete line that is not an event: of a head file, or of a ref's file that is no
    /// JR-02 line with an `origin`.
    Undecodable,
    /// A merge commit in a journal ref's history.
    Merge,
    /// A journal ref, or one of its commits, whose tree is not exactly `<name>.jsonl`, or that
    /// names no commit.
    Tree,
    /// A journal ref's file at a commit whose last byte is not `\n`.
    Unended,
    /// A journal ref's file at a commit past the journal size cap.
    TooLarge,
    /// A journal ref's file at a commit that does not start with the previous one's.
    Rewritten,
    /// A ref ci-check could not read whole: a name no journal has, a history or listing past
    /// the cap, an inbox file past the cap.
    Unreadable,
}

/// What ci-check read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Read {
    /// The committed journals at head, by repo-relative path.
    pub files: Vec<String>,
    /// Every ref source read; empty when the clone holds none.
    pub refs: Vec<RefRead>,
}

/// One ref source read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefRead {
    /// The full ref name.
    #[serde(rename = "ref")]
    pub refname: String,
    /// Which kind of source.
    pub kind: RefKind,
    /// The tip's full object id.
    pub tip: String,
    /// The commits read: a journal ref's first-parent chain, a broken ref's history, 1 for an
    /// inbox.
    pub commits: u64,
}

/// Which kind of ref source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RefKind {
    /// `refs/redkiln/journal/<name>`: checked whole.
    Journal,
    /// `refs/redkiln/inbox/<name>`, the clone's copy of an inbox branch: read, never checked.
    Inbox,
    /// `refs/redkiln/broken/<name>/<sha>`, set aside: read, never checked.
    Broken,
}

#[cfg(test)]
mod tests;
