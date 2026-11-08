//! The JSON `weighin open --json` writes: an array of [`OpenItem`], each an item's current state.
//!
//! anneal's `/orchestrate` and its D6 hook read it, so its bytes are kept as weighin 1.2.0 wrote
//! them: field order is wire order, and `reviewed_ts` is left out until a decision is reviewed.
//! weighin-core's projection folds the events into its own `Item` and converts it into this
//! type; a reader deserialises the same type.

use crate::card::Card;
use crate::event::{
    Blast, Channel, DefaultCause, Deferred, DismissReason, Provenance, Route, Timing,
};
use crate::id::ItemId;
use crate::vocab::{Authority, Door, Kind};
use serde::{Deserialize, Serialize};

/// Where an item is in its lifecycle.
///
/// The order is the lifecycle's: a later triage or grounding never moves an open item backwards.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// Captured and not yet triaged, or reopened before it had a card.
    Raised,
    /// Classified and routed, with no card yet.
    Triaged,
    /// Has a decision card that has not been shown, or was reopened after it had one.
    Grounded,
    /// Shown to the human and waiting for an answer.
    Presented,
    /// Waiting on a named fact; open, but parked behind the deferral.
    Deferred,
    /// The human chose an option.
    Decided,
    /// An option was taken without a human.
    Defaulted,
    /// Needed no human.
    Dismissed,
    /// Something else answers it.
    Superseded,
}

impl Status {
    /// The wire spelling, such as `raised` or `defaulted`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Raised => "raised",
            Self::Triaged => "triaged",
            Self::Grounded => "grounded",
            Self::Presented => "presented",
            Self::Deferred => "deferred",
            Self::Decided => "decided",
            Self::Defaulted => "defaulted",
            Self::Dismissed => "dismissed",
            Self::Superseded => "superseded",
        }
    }
    /// Still waiting on someone (a deferred item waits on its trigger).
    #[must_use]
    pub fn is_open(self) -> bool {
        matches!(
            self,
            Self::Raised | Self::Triaged | Self::Grounded | Self::Presented | Self::Deferred
        )
    }
}

/// The human's decision on an item, from its latest `item_decided` event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decision {
    /// The chosen card option id, or the answer itself for an item with no card.
    pub option: String,
    /// Who decided, as `human:<owner>`.
    pub decider: String,
    /// Whether the choice followed the recommendation or was the human's own.
    pub provenance: Provenance,
    /// The human's own words with the choice; empty when none.
    pub note: String,
    /// Where it was decided: `chat`, `board`, `pane` or `cli`.
    pub via: String,
    /// Whether the choice was the card's recommendation; `None` when the item had no card.
    pub matched_recommendation: Option<bool>,
    /// The tree when it was decided; `None` outside git or when git could not compute it.
    pub tree_hash: Option<String>,
    /// Seconds from the latest presentation to the decision; `None` when unknown.
    pub seconds_to_decide: Option<u64>,
    /// When the decision was written, by the writer's clock (RFC 3339 UTC).
    pub ts: String,
    /// When the decision was last reviewed and left standing (`item_acknowledged`), by the
    /// writer's clock; `None` until it is. Absent from the serialised item until then.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reviewed_ts: Option<String>,
}

/// An option taken without a human, from the item's latest `item_defaulted` event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DefaultTaken {
    /// The option taken, or what the agent did for its own two-way call.
    pub option: String,
    /// Why no human chose.
    pub cause: DefaultCause,
    /// The explanation shown in the digest.
    pub reason: String,
    /// When the default was written, by the writer's clock (RFC 3339 UTC).
    pub ts: String,
    /// The human saw it in the digest and left it standing (`item_acknowledged`).
    pub acknowledged: bool,
}

/// A `.kb` atom written for an item, from a `record_written` event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    /// Repo-relative path of the atom file.
    pub path: String,
    /// The atom's id, as in its frontmatter.
    pub atom_id: String,
    /// `decision` or `open_question`.
    pub atom_kind: String,
}

/// One item as `open --json` lists it: its first raise plus every later event, folded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpenItem {
    /// The item's id.
    pub id: ItemId,
    /// What was raised: a question, an assumption, a deferral or a call.
    pub kind: Kind,
    /// The question as first raised.
    pub question: String,
    /// The normalised question, the key duplicates are matched on.
    pub question_key: String,
    /// What the agent did meanwhile; `None` when the raise said nothing.
    pub did: Option<String>,
    /// The flip fact the agent named when raising; `None` when it named none.
    pub flip_hint: Option<String>,
    /// The raise's repo-relative `path:line` anchor; `None` when the item points at no code.
    pub at: Option<String>,
    /// The symbol the raise named; `None` when it named none.
    pub sym: Option<String>,
    /// What the item blocks, as the raise gave it; `None` when it blocks nothing.
    pub blocks: Option<String>,
    /// The agent's reversibility guess at raise time.
    pub door_hint: Door,
    /// The agent's authority-or-judgment guess at raise time.
    pub authority_hint: Authority,
    /// How the item was captured.
    pub channel: Channel,
    /// The raised text exactly as the agent or file had it.
    pub quote: String,
    /// The repo-relative file the item was scanned from; `None` unless it came from a scan.
    pub source_path: Option<String>,
    /// The 1-based line of the quote in `source_path`; `None` when unknown.
    pub source_line: Option<usize>,
    /// The session that first raised it; `None` when that writer had no session.
    pub session_id: Option<String>,
    /// The worktree the first raise was written from; `None` outside git.
    pub worktree: Option<String>,
    /// The branch the first raise was written on; `None` outside git or on a detached head.
    pub branch: Option<String>,
    /// When the first raise was written, by the writer's clock (RFC 3339 UTC).
    pub raised_ts: String,
    /// Store position of the first raise.
    pub raised_position: u64,
    /// Reversibility from the latest triage; `None` until triaged.
    pub door: Option<Door>,
    /// Blast radius from the latest triage; `None` until triaged.
    pub blast: Option<Blast>,
    /// Authority from the latest triage; `None` until triaged.
    pub authority: Option<Authority>,
    /// Route from the latest triage; `None` until triaged.
    pub route: Option<Route>,
    /// The latest triage's decide-now test; `None` until triaged, or when it recorded none.
    pub timing: Option<Timing>,
    /// The latest triage's deadline; `None` until triaged, or when it set none.
    pub resolve_by: Option<String>,
    /// The card from the latest grounding; `None` until grounded.
    pub card: Option<Card>,
    /// Position of the grounding that produced `card`; `None` until grounded.
    pub card_rev: Option<u64>,
    /// When the item was last presented; `None` if it never was.
    pub presented_ts: Option<String>,
    /// The tree at the latest presentation; `None` if never presented or no tree was recorded.
    pub presented_tree: Option<String>,
    /// How many times the item was presented.
    pub times_presented: u32,
    /// Where the item is in its lifecycle.
    pub status: Status,
    /// The latest decision; `None` if never decided, or reopened since.
    pub decision: Option<Decision>,
    /// The latest deferral; `None` if never deferred, or cleared since.
    pub deferral: Option<Deferred>,
    /// The latest default; `None` if never defaulted, or reopened since.
    pub default: Option<DefaultTaken>,
    /// Why it was dismissed; `None` if never dismissed, or reopened since.
    pub dismissed: Option<DismissReason>,
    /// What superseded it, as the caller gave it; `None` if never superseded.
    pub superseded_by: Option<String>,
    /// Atoms written for the item, oldest first.
    pub records: Vec<Record>,
    /// `(kind, target)` links other than `raised-again`, oldest first.
    pub links: Vec<(String, String)>,
    /// Other sessions that raised the same question, each once, in the order they did.
    pub also_raised_in: Vec<String>,
    /// How many times the item was reopened.
    pub reopened: u32,
    /// Store position of the latest event applied to the item.
    pub last_position: u64,
    /// When the latest event applied to the item was written, by the writer's clock.
    pub last_ts: String,
}
