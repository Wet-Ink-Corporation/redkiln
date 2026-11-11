//! The approval record: `core.approval_recorded`, version 1, durable (SPEC AP-01).
//!
//! Any plugin records an approval through Core, so that an approval given in one plugin can
//! answer a gate in another without either reading the other's internals. The record binds the
//! approval to the hash of its subject (ID-09); a gate recomputes that hash before it accepts
//! one (AP-02), so a reader checks `subject.hash` with [`Subject::parse`] rather than trust it.
//! The event carries the tag `subject:<method>:<hash>`, and `item:<id>` when `request` is set:
//! the writer passes them to [`crate::envelope::Envelope::to_event`].
//!
//! The fields are the SPEC's keys in its order, so a record encodes byte for byte as the
//! goldens in `spec/fixtures/envelope/valid/` do. Optional keys are written only when set, and
//! unknown keys are ignored on read (EV-09).

use serde::{Deserialize, Serialize};

use crate::envelope::{Class, Payload};
use crate::subject::Subject;

/// The data of `core.approval_recorded` (AP-01).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRecorded {
    /// What was approved.
    pub subject: ApprovalSubject,
    /// The decision.
    pub verdict: Verdict,
    /// Where it holds; absent reads as [`Scope::Repository`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<Scope>,
    /// The gates that may accept it, as `<namespace>.<gate>`; absent means any gate whose subject
    /// matches.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gates: Option<Vec<String>>,
    /// Who decided: `github:<login>` or `human:<name>`. Required: AP-01 refuses a record with no
    /// decider, so with no reported identity and no owner (ST-12) nothing is recorded.
    pub decider: String,
    /// When it stops applying, RFC 3339 UTC; absent means never.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires: Option<String>,
    /// The `wi-` item it answers, when it answers a decision request.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request: Option<String>,
}

impl Payload for ApprovalRecorded {
    const TYPE: &'static str = "core.approval_recorded";
    const VERSION: u32 = 1;
    const CLASS: Class = Class::Durable;
}

/// What an approval binds to (AP-01's `subject`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalSubject {
    /// What sort of thing: `plan`, `spec`, `pull_request` and the rest of AP-01's list, or
    /// `<namespace>.<kind>`.
    pub kind: String,
    /// The hash method's name (ID-09), as [`crate::subject::Method::as_str`] writes it.
    pub method: String,
    /// The subject in its written form, `<method>:<hash>`.
    pub hash: String,
    /// For `patch-id`, the commit the diff was taken against.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    /// Where the subject is: a worktree-relative path with `/` separators, `pr:<number>`, or a
    /// commit.
    pub locator: String,
}

impl ApprovalSubject {
    /// The subject of `kind` at `locator` whose method and hash are `subject`'s, so a subject
    /// built here keeps the two in agreement. The fields are public, so a value built or read
    /// another way may not: readers re-check `hash` against `method` (AP-02). No base: see
    /// [`ApprovalSubject::with_base`].
    #[must_use]
    pub fn new(kind: impl Into<String>, subject: &Subject, locator: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            method: subject.method().as_str().to_owned(),
            hash: subject.as_str().to_owned(),
            base: None,
            locator: locator.into(),
        }
    }

    /// The same subject, taken against the commit `base` (a `patch-id`'s base).
    #[must_use]
    pub fn with_base(mut self, base: impl Into<String>) -> Self {
        self.base = Some(base.into());
        self
    }
}

/// An approval's decision (AP-01's `verdict`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Verdict {
    /// Approved.
    Approved,
    /// Rejected: voids an earlier approval of the same subject at the same or a lower trust
    /// level (AP-02).
    Rejected,
    /// Withdrawn: voids an earlier approval as a rejection does.
    Withdrawn,
}

/// Where an approval holds (AP-01's `scope`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scope {
    /// The worktree it was given in.
    Worktree,
    /// The branch it was given on.
    Branch,
    /// The whole repository.
    Repository,
}
