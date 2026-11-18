//! The decision request (SPEC DR-01 to DR-04): how any plugin asks the human, through Weigh-In.
//!
//! The record ([`Request`] and its parts) is always here, so weighin-core folds a request into an
//! item without linking Core (DR-01). With the `core` feature a raiser builds a [`Request`],
//! checks it (`Request::check`), mints its item id (`Request::item_id`) and appends the event
//! `raise_event` builds, conditioned on the item not being raised yet (`raise_condition`, DR-03).
//! The guards run here, so a sibling cannot raise a malformed request by skipping Weigh-In's CLI
//! (Weigh-In call B): DR-04's cross-field rules, and each per-field shape of
//! `decision-request-1.schema.json` the types do not already hold (empty or whitespace-only
//! texts, an empty attachment list, the hash, kind, base and locator forms). Weigh-In folds each
//! request into a grounded item of channel `request`.

use crate::event::Blast;
use crate::vocab::{Authority, Door};
use serde::{Deserialize, Serialize};
use std::fmt;

#[cfg(feature = "core")]
mod raise;
#[cfg(feature = "core")]
pub use raise::{RequestError, raise_condition, raise_event, raise_event_at, same_raiser};

/// The domain of a request's item id (DR-03, ID-04).
pub const ID_DOMAIN: &str = "redkiln.id/weighin.request/1";

/// The anchor attachment kind: its locator (`path:line`) becomes the item's `at`, so a context
/// pack finds the code a request is about.
pub const ANCHOR: &str = "anchor";

/// The request record (DR-02). Optional keys are left out when absent, never written as `null`.
/// Field order is the wire order of the SPEC's golden.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    /// The question, whole.
    pub question: String,
    /// Two to four options, ids `a` to `d` in order.
    pub options: Vec<RequestOption>,
    /// The option recommended.
    pub recommendation: OptionId,
    /// How reversible the choice is; `unsure` counts as one-way everywhere.
    pub door: Door,
    /// Whose call it is.
    pub authority: Authority,
    /// How far a wrong answer reaches.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blast: Option<Blast>,
    /// The option an unattended run takes on a two-way item; never on one-way or unsure.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<OptionId>,
    /// RFC 3339 UTC: when the raiser stops waiting and takes the default or gives up.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline: Option<String>,
    /// What waits on it.
    pub blocks: Blocks,
    /// The trust an answer needs (TR-07).
    pub trust_needed: TrustNeeded,
    /// Who asked.
    pub raiser: Raiser,
    /// What the decision is about, in the approval record's subject shape (AP-01).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub subject: Option<Subject>,
    /// Files, diffs or logs the human should see.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attachments: Option<Vec<Attachment>>,
}

/// One option of a request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RequestOption {
    /// `a` to `d`, in order.
    pub id: OptionId,
    /// The option in a few words.
    pub label: String,
    /// When it is the right answer.
    pub holds_when: String,
    /// What it costs if it is the wrong one.
    pub cost_if_wrong: String,
}

/// An option's id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OptionId {
    /// The first option.
    A,
    /// The second.
    B,
    /// The third.
    C,
    /// The fourth.
    D,
}

impl OptionId {
    /// The ids in the order options carry them.
    pub const ALL: [Self; 4] = [Self::A, Self::B, Self::C, Self::D];

    /// The id as written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::A => "a",
            Self::B => "b",
            Self::C => "c",
            Self::D => "d",
        }
    }
}

impl fmt::Display for OptionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What waits on a request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Blocks {
    /// A merge.
    Merge,
    /// A release.
    Release,
    /// A Loop run.
    Run,
    /// Nothing.
    None,
    /// The raiser cannot say.
    Unknown,
}

impl Blocks {
    /// The word as written: `merge`, `release`, `run`, `none` or `unknown`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Merge => "merge",
            Self::Release => "release",
            Self::Run => "run",
            Self::None => "none",
            Self::Unknown => "unknown",
        }
    }
}

/// The trust an answer needs (TR-07).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TrustNeeded {
    /// An answer from a surface the agent cannot drive (TR-03).
    Witnessed,
    /// An answer verifiable against an identity outside the agent's reach (TR-04).
    Attested,
}

/// The raising plugin, its own correlation key, and the run or session that asked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Raiser {
    /// The raising plugin's namespace (EV-03).
    pub namespace: RaiserNamespace,
    /// The raiser's own key: raising it again finds the same item (DR-03).
    pub key: String,
    /// The run that asked.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub run: Option<String>,
    /// The session that asked; a `gha:` session is a CI run (ID-05).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session: Option<String>,
}

/// What a decision is about (AP-01's subject shape).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Subject {
    /// What kind of thing it is, such as `dependency` or `rust.lint_exception`.
    pub kind: String,
    /// The hash method (ID-09).
    pub method: String,
    /// `<method>:<hash>`.
    pub hash: String,
    /// The base commit, for a method that hashes a change.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<String>,
    /// A worktree-relative path, `pr:<number>` or a commit.
    pub locator: String,
}

/// A file, diff or log the human should see.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attachment {
    /// What it is.
    pub kind: String,
    /// Where it is.
    pub locator: String,
    /// Its digest, `<method>:<lowercase hex>` (ID-07).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hash: Option<String>,
}

/// EV-03's registered namespaces, as written. Core's `Namespace::ALL` is the same list; a test
/// under the `core` feature pins the two together (ARC-15).
const NAMESPACES: [&str; 8] = [
    "core",
    "weighin",
    "bench",
    "rust",
    "review",
    "loop",
    "knowledge",
    "backlog",
];

/// A raiser's namespace, written as its EV-03 word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RaiserNamespace(&'static str);

impl RaiserNamespace {
    /// The namespace `word` names; `None` when it is not one of EV-03's.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        NAMESPACES
            .into_iter()
            .find(|known| *known == word)
            .map(Self)
    }

    /// The word as written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        self.0
    }
}

impl Serialize for RaiserNamespace {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.0)
    }
}

impl<'de> Deserialize<'de> for RaiserNamespace {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::Error as _;
        let word = String::deserialize(deserializer)?;
        Self::parse(&word).ok_or_else(|| {
            D::Error::custom(format_args!(
                "unknown namespace `{word}`: not one of EV-03's registered namespaces"
            ))
        })
    }
}

#[cfg(test)]
mod record_tests;
#[cfg(test)]
mod tests;
