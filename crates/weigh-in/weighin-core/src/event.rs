//! The ledger's event vocabulary.
//!
//! Every state change of an item is one event, and every event after `item_raised` carries a
//! reason (Seeds: "every state transition takes a reason"). Event type names are `snake_case` to
//! match redkiln-rs's event-kind table; tags use redkiln's `key:value` shape (`item:`, `session:`,
//! `worktree:`, ...), so a later import into redkiln-rs's store is a tag remap, not a rewrite.
//!
//! Every payload struct here is persisted: its field names are the JSON keys stored in the
//! happenstance event store. A field marked `#[serde(default)]` may be missing from a stored or
//! submitted event and then decodes to `None`, `""` or `false`.

use crate::card::Card;
use crate::id::ItemId;
use crate::marker::{Authority, Door, Kind};
use serde::{Deserialize, Serialize};

/// Where a raised item came from, in falling order of precision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum Channel {
    /// A `weigh-in` block the agent emitted.
    #[default]
    Marker,
    /// A bullet under an "Open questions" heading, or a new TODO, in a written file.
    Scan,
    /// Found by the in-sitting sweep of the current conversation.
    Sweep,
    /// Raised by hand with `weighin raise`.
    Manual,
}

impl Channel {
    /// The wire spelling: `marker`, `scan`, `sweep` or `manual`.
    ///
    /// It is also the value of the `channel:` store tag on an `item_raised` event.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Marker => "marker",
            Self::Scan => "scan",
            Self::Sweep => "sweep",
            Self::Manual => "manual",
        }
    }
}

/// Where an item was shown or decided: the spellings [`Presented::surface`] and
/// [`Decided::via`] use.
///
/// Those fields stay strings, so a stored event with any other spelling still decodes; this is
/// the vocabulary writers use, so `weighin stats` and the journals keep the surfaces apart.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    /// A card in the chat that raised the item (a sitting).
    Chat,
    /// The claude.ai Artifact board.
    Board,
    /// The local review page.
    Local,
    /// The local board pane of a Claude Code mod, which never publishes (D-0017).
    Pane,
    /// A `weighin` command typed by hand.
    Cli,
}

impl Surface {
    /// The wire spelling: `chat`, `board`, `local`, `pane` or `cli`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Chat => "chat",
            Self::Board => "board",
            Self::Local => "local",
            Self::Pane => "pane",
            Self::Cli => "cli",
        }
    }
}

/// How far a wrong answer would reach (the blast radius), as the triage pass judged it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Blast {
    /// Confined to one file.
    File,
    /// Confined to one module.
    Module,
    /// Spans modules within the system.
    System,
    /// Reaches outside the system.
    External,
}

impl Blast {
    /// Parse the wire spelling, ignoring case and surrounding space.
    ///
    /// Returns `None` for anything but `file`, `module`, `system` or `external`.
    #[must_use]
    pub fn parse(v: &str) -> Option<Self> {
        match v.trim().to_ascii_lowercase().as_str() {
            "file" => Some(Self::File),
            "module" => Some(Self::Module),
            "system" => Some(Self::System),
            "external" => Some(Self::External),
            _ => None,
        }
    }
    /// The wire spelling: `file`, `module`, `system` or `external`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Module => "module",
            Self::System => "system",
            Self::External => "external",
        }
    }
}

/// How the triage pass routed an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Route {
    /// Needs Ryan: authority-bound, one-way, or an unstable recommendation.
    Present,
    /// Judgment-bound two-way door with a stable pick: decided by the agent and logged.
    Default,
    /// Not a question after all (settled, recoverable, duplicate, moot).
    Dismiss,
}

/// The decide-now test, recorded so a deferral can be audited.
///
/// Each field answers one check of the test (`SKILL.md` step 2.4; the research report's filter
/// 4). A deferral is legitimate only when every check points to waiting.
#[expect(
    clippy::struct_excessive_bools,
    reason = "persisted decide-now answers; the field names are the stored JSON keys"
)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct Timing {
    /// `decide-now` or `defer`.
    pub verdict: String,
    /// The specific fact that would flip the answer. None named means decide now.
    #[serde(default)]
    pub flip: String,
    /// The flip fact can be got now or cheaply, so get it and decide.
    #[serde(default)]
    pub obtainable_now: bool,
    /// The flip fact arrives only after the option expires, so waiting buys nothing.
    #[serde(default)]
    pub arrives_after_expiry: bool,
    /// What waiting costs in natural units (blocked work, rework); empty when not stated.
    #[serde(default)]
    pub cost_of_waiting: String,
    /// The top options are each acceptable as the only option: decide now, cheap to revisit.
    #[serde(default)]
    pub options_close: bool,
}

/// Why an item needed no human.
///
/// Stored as `{"kind": "<kebab-case variant>", "ref": "<payload>"}`; the unit variants have no
/// `ref`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", tag = "kind", content = "ref")]
pub enum DismissReason {
    /// Not worth anyone's attention; counted against the action rate.
    NotUseful,
    /// The repository already answers it (code, tests, docs or git history); the agent answered
    /// it and put the evidence in the dismissal's reason.
    Recoverable,
    /// Already settled; the payload cites where (a `kb-…` atom id, or `chat:"<the user's words>"`).
    AlreadyDecided(String),
    /// Asks the same thing as another item; the payload is that item's id.
    DuplicateOf(String),
    /// Belongs to someone else; the payload names who.
    NotMyCall(String),
    /// No longer matters (the code or the plan moved on).
    Moot,
}

impl DismissReason {
    /// Parse `not-useful`, `recoverable`, `moot`, `already-decided:<ref>`, `duplicate-of:<id>`,
    /// `not-my-call:<who>`.
    ///
    /// `duplicate:<id>` is accepted for `duplicate-of`. Returns `None` for an unknown reason, or
    /// for a reason that needs a reference and has none.
    #[must_use]
    pub fn parse(v: &str) -> Option<Self> {
        let (k, r) = v.split_once(':').map_or((v, ""), |(k, r)| (k, r));
        let r = r.trim().to_owned();
        match k.trim() {
            "not-useful" => Some(Self::NotUseful),
            "recoverable" => Some(Self::Recoverable),
            "moot" => Some(Self::Moot),
            "already-decided" if !r.is_empty() => Some(Self::AlreadyDecided(r)),
            "duplicate-of" | "duplicate" if !r.is_empty() => Some(Self::DuplicateOf(r)),
            "not-my-call" if !r.is_empty() => Some(Self::NotMyCall(r)),
            _ => None,
        }
    }
    /// The reason in the form [`DismissReason::parse`] accepts, such as `not-my-call:legal`.
    #[must_use]
    pub fn label(&self) -> String {
        match self {
            Self::NotUseful => "not-useful".into(),
            Self::Recoverable => "recoverable".into(),
            Self::Moot => "moot".into(),
            Self::AlreadyDecided(r) => format!("already-decided:{r}"),
            Self::DuplicateOf(r) => format!("duplicate-of:{r}"),
            Self::NotMyCall(r) => format!("not-my-call:{r}"),
        }
    }
}

/// How the human arrived at a decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Provenance {
    /// Ryan chose the recommended option with the trade-off in view.
    UserApproved,
    /// Ryan chose something other than the recommendation, or stated the answer himself.
    UserDirected,
}

impl Provenance {
    /// The wire spelling: `user-approved` or `user-directed`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UserApproved => "user-approved",
            Self::UserDirected => "user-directed",
        }
    }
}

/// Why an option was taken without a human choosing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DefaultCause {
    /// Judgment-bound two-way door: the agent's own call, logged.
    TwoWay,
    /// Resolve-by passed with nobody acting.
    Expired,
    /// Taken by an unattended run and reported in the digest.
    Unattended,
}

impl DefaultCause {
    /// Parse the wire spelling, ignoring surrounding space but not case.
    ///
    /// Returns `None` for anything but `two-way`, `expired` or `unattended`.
    #[must_use]
    pub fn parse(v: &str) -> Option<Self> {
        match v.trim() {
            "two-way" => Some(Self::TwoWay),
            "expired" => Some(Self::Expired),
            "unattended" => Some(Self::Unattended),
            _ => None,
        }
    }
    /// The wire spelling: `two-way`, `expired` or `unattended`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TwoWay => "two-way",
            Self::Expired => "expired",
            Self::Unattended => "unattended",
        }
    }
}

// ----------------------------------------------------------------------------------------------
// Payloads
// ----------------------------------------------------------------------------------------------

/// Payload of `item_raised`: a question, assumption, deferral or call enters the ledger.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Raised {
    /// The id minted from the raising session and the question.
    pub item_id: ItemId,
    /// What the agent raised: a question, an assumption, a deferral or a call.
    pub kind: Kind,
    /// The question as asked, trimmed; its normalised form is the duplicate key.
    pub question: String,
    /// What the agent did meanwhile (`blocked` when it took no one-way path).
    ///
    /// `None` when the raise gave no `did:` (a marker's `none` or `-` counts as none), and for
    /// scanned items.
    #[serde(default)]
    pub did: Option<String>,
    /// The agent's guess at the fact that would flip the answer (the marker's `flip:`).
    ///
    /// `None` when the agent named none, and for scanned items.
    #[serde(default)]
    pub flip: Option<String>,
    /// `path:line` anchor, repo-relative.
    ///
    /// `None` when the item points at no code. A scanned item is anchored at its source line when
    /// that line was found.
    #[serde(default)]
    pub at: Option<String>,
    /// The symbol the item is about (the marker's `sym:` or `symbol:`), for the context pack.
    ///
    /// `None` when not given; the context pack then takes a symbol from the anchor line.
    #[serde(default)]
    pub sym: Option<String>,
    /// What this item blocks, as the marker's `blocks:` gave it (free text).
    ///
    /// `None` when not given (a marker's `none` or `-` counts as not given). An item that blocks
    /// something ranks ahead of one that does not.
    #[serde(default)]
    pub blocks: Option<String>,
    /// The agent's reversibility guess; triage may override it.
    pub door_hint: Door,
    /// The agent's authority-or-judgment guess; triage may override it.
    pub authority_hint: Authority,
    /// How the item was captured.
    pub channel: Channel,
    /// The text exactly as the agent or file had it. Transcripts are deleted after 30 days; this
    /// quote is what keeps the item auditable.
    pub quote: String,
    /// File the item was scanned from, so a decision can annotate it.
    ///
    /// Repo-relative. `None` for items that did not come from a file scan.
    #[serde(default)]
    pub source_path: Option<String>,
    /// The 1-based line of `quote` in `source_path` when it was scanned.
    ///
    /// `None` when the item was not scanned, or the quoted line was not found on disk.
    #[serde(default)]
    pub source_line: Option<usize>,
}

/// Payload of `item_triaged`: the triage pass's classification and routing of an item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Triaged {
    /// The item triaged.
    pub item_id: ItemId,
    /// Reversibility as triaged; it takes precedence over the raise's `door_hint`.
    pub door: Door,
    /// How far a wrong answer would reach.
    pub blast: Blast,
    /// Authority or judgment as triaged; it takes precedence over the raise's `authority_hint`.
    pub authority: Authority,
    /// Where the item goes next.
    pub route: Route,
    /// The decide-now test; `None` when the triage did not record one.
    #[serde(default)]
    pub timing: Option<Timing>,
    /// `before-merge:<branch>`, `before-release`, an ISO date, or `blocks:<item>`.
    ///
    /// `None` when the triage set no deadline.
    #[serde(default)]
    pub resolve_by: Option<String>,
    /// Why the item was routed this way.
    pub reason: String,
}

/// Payload of `item_grounded`: the decision card built for an item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grounded {
    /// The item grounded.
    pub item_id: ItemId,
    /// The validated card; a later grounding replaces it.
    pub card: Card,
    /// Why it was grounded; `grounded` when the caller gave no reason.
    pub reason: String,
}

/// Payload of `item_presented`: an item was shown to the human.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Presented {
    /// The item shown.
    pub item_id: ItemId,
    /// `chat`, `board`, `local` or `pane` (a [`Surface`]).
    pub surface: String,
    /// The tree the human saw (redkiln-rs ADR-0105 clause 4: temporary index, untracked excluded).
    ///
    /// `None` outside a git repository, or when git could not compute it.
    #[serde(default)]
    pub tree_hash: Option<String>,
    /// Why it was shown, such as `shown in chat`.
    pub reason: String,
}

/// Payload of `item_decided`: the human chose an option.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Decided {
    /// The item decided.
    pub item_id: ItemId,
    /// The chosen card option id (`A`, `B`, ...), or the answer itself for an item with no card.
    pub option: String,
    /// Who decided, as `human:<owner>` with the owner from the repository's config.
    pub decider: String,
    /// Whether the choice followed the recommendation or was the human's own.
    pub provenance: Provenance,
    /// The human's own words with the choice; empty when none.
    #[serde(default)]
    pub note: String,
    /// The tree when the decision was taken; `None` outside git or when git could not compute it.
    #[serde(default)]
    pub tree_hash: Option<String>,
    /// Whether `option` is the card's recommended option; `None` when the item had no card.
    #[serde(default)]
    pub matched_recommendation: Option<bool>,
    /// `chat`, `board`, `pane` or `cli` (a [`Surface`]).
    pub via: String,
    /// Seconds from the latest presentation to the decision, unless the caller reported a figure.
    ///
    /// `None` when the item was never presented and the caller gave no figure.
    #[serde(default)]
    pub seconds_to_decide: Option<u64>,
    /// Why, such as `chose the recommended option` or `chose against the recommendation`.
    pub reason: String,
}

/// Payload of `item_deferred`: the item waits on a named fact, with a default if nobody acts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deferred {
    /// The item deferred.
    pub item_id: ItemId,
    /// The fact being waited for; may be empty for an unqualified deferral.
    pub flip: String,
    /// The event that brings the item back, such as `next sitting`.
    pub trigger: String,
    /// The deadline after which `default_option` applies, in the forms `Triaged` accepts.
    pub resolve_by: String,
    /// The option taken if nobody acts by `resolve_by`.
    pub default_option: String,
    /// Who watches the trigger; the repository's configured owner unless the caller named one.
    pub owner: String,
    /// Deferred without a passing decide-now test (counted in metrics).
    #[serde(default)]
    pub unqualified: bool,
    /// Why it was deferred; may be empty.
    pub reason: String,
}

/// Payload of `item_defaulted`: an option was taken without a human choosing it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Defaulted {
    /// The item defaulted.
    pub item_id: ItemId,
    /// The option taken, or for the agent's own two-way call, what it did (the marker's `did:`).
    pub option: String,
    /// Why no human chose.
    pub cause: DefaultCause,
    /// The explanation shown in the digest.
    pub reason: String,
}

/// Payload of `item_dismissed`: the item needed no human after all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dismissed {
    /// The item dismissed.
    pub item_id: ItemId,
    /// The category of dismissal, with its reference where it needs one.
    pub why: DismissReason,
    /// Free text from the dismisser; empty when none.
    #[serde(default)]
    pub note: String,
    /// The audit reason for the dismissal.
    pub reason: String,
}

/// Payload of `item_reopened`: a closed or deferred item is open again.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Reopened {
    /// The item reopened.
    pub item_id: ItemId,
    /// Free text from whoever reopened it; empty when none.
    #[serde(default)]
    pub note: String,
    /// Where or why it was reopened, such as `reopened from the digest`; may be empty.
    pub reason: String,
}

/// Payload of `item_superseded`: something else now answers this item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Superseded {
    /// The item superseded.
    pub item_id: ItemId,
    /// What supersedes it (an item id or an atom id), as the caller gave it.
    pub by: String,
    /// Why it was superseded.
    pub reason: String,
}

/// Payload of `item_linked`: the item points at something outside it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Linked {
    /// The item linked.
    pub item_id: ItemId,
    /// `commit`, `pr`, `task`, `atom`, `raised-again`.
    ///
    /// `raised-again` records another session raising the same question; its target is that
    /// session's id.
    pub kind: String,
    /// What is linked, per `kind`: a commit, a PR, a task, an atom or a session id.
    pub target: String,
    /// Why the link was made; `linked` when the caller gave no reason.
    pub reason: String,
}

/// Payload of `record_written`: a `.kb` atom was written for the item.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecordWritten {
    /// The item recorded.
    pub item_id: ItemId,
    /// Repo-relative path of the atom file (under `.kb/_intake/`).
    pub path: String,
    /// The atom's id, as in its frontmatter.
    pub atom_id: String,
    /// `decision` or `open_question`.
    pub atom_kind: String,
    /// Why the atom was written.
    pub reason: String,
}

/// Payload of `item_acknowledged`: the human saw a default in the digest and left it standing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Acknowledged {
    /// The defaulted item acknowledged.
    pub item_id: ItemId,
    /// Why it was left as defaulted.
    pub reason: String,
}

/// One ledger event. The serde tag is the event type, so the payload bytes stored under a type
/// always decode to that variant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LedgerEvent {
    /// `item_raised`: an item enters the ledger.
    ItemRaised(Raised),
    /// `item_triaged`: the item was classified and routed.
    ItemTriaged(Triaged),
    /// `item_grounded`: the item got a decision card.
    ItemGrounded(Grounded),
    /// `item_presented`: the item was shown to the human.
    ItemPresented(Presented),
    /// `item_decided`: the human chose an option.
    ItemDecided(Decided),
    /// `item_deferred`: the item waits on a named fact.
    ItemDeferred(Deferred),
    /// `item_defaulted`: an option was taken without a human.
    ItemDefaulted(Defaulted),
    /// `item_dismissed`: the item needed no human.
    ItemDismissed(Dismissed),
    /// `item_reopened`: the item is open again.
    ItemReopened(Reopened),
    /// `item_superseded`: something else answers the item.
    ItemSuperseded(Superseded),
    /// `item_linked`: the item points at a commit, PR, task, atom or another session.
    ItemLinked(Linked),
    /// `record_written`: a `.kb` atom was written for the item.
    RecordWritten(RecordWritten),
    /// `item_acknowledged`: a default was seen and left standing.
    ItemAcknowledged(Acknowledged),
}

/// Every event type, for queries that select them all.
pub const EVENT_TYPES: &[&str] = &[
    "item_raised",
    "item_triaged",
    "item_grounded",
    "item_presented",
    "item_decided",
    "item_deferred",
    "item_defaulted",
    "item_dismissed",
    "item_reopened",
    "item_superseded",
    "item_linked",
    "record_written",
    "item_acknowledged",
];

impl LedgerEvent {
    /// The event type the store files this event under; one of [`EVENT_TYPES`].
    #[must_use]
    pub fn event_type(&self) -> &'static str {
        match self {
            Self::ItemRaised(_) => "item_raised",
            Self::ItemTriaged(_) => "item_triaged",
            Self::ItemGrounded(_) => "item_grounded",
            Self::ItemPresented(_) => "item_presented",
            Self::ItemDecided(_) => "item_decided",
            Self::ItemDeferred(_) => "item_deferred",
            Self::ItemDefaulted(_) => "item_defaulted",
            Self::ItemDismissed(_) => "item_dismissed",
            Self::ItemReopened(_) => "item_reopened",
            Self::ItemSuperseded(_) => "item_superseded",
            Self::ItemLinked(_) => "item_linked",
            Self::RecordWritten(_) => "record_written",
            Self::ItemAcknowledged(_) => "item_acknowledged",
        }
    }

    /// The item this event is about.
    #[must_use]
    pub fn item_id(&self) -> &ItemId {
        match self {
            Self::ItemRaised(e) => &e.item_id,
            Self::ItemTriaged(e) => &e.item_id,
            Self::ItemGrounded(e) => &e.item_id,
            Self::ItemPresented(e) => &e.item_id,
            Self::ItemDecided(e) => &e.item_id,
            Self::ItemDeferred(e) => &e.item_id,
            Self::ItemDefaulted(e) => &e.item_id,
            Self::ItemDismissed(e) => &e.item_id,
            Self::ItemReopened(e) => &e.item_id,
            Self::ItemSuperseded(e) => &e.item_id,
            Self::ItemLinked(e) => &e.item_id,
            Self::RecordWritten(e) => &e.item_id,
            Self::ItemAcknowledged(e) => &e.item_id,
        }
    }

    /// Whether this event closes the item (it no longer needs anyone).
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            Self::ItemDecided(_)
                | Self::ItemDefaulted(_)
                | Self::ItemDismissed(_)
                | Self::ItemSuperseded(_)
        )
    }
}

/// Who wrote an event, where, and when.
///
/// `ts` is the writer's clock; the store's own `recorded_at` and position are kept separately and
/// positions are the only ordering key. The event's fields sit beside the envelope's in one JSON
/// object, with the event type under `type`.
///
/// # Examples
///
/// ```
/// use weighin_core::event::{Acknowledged, EventEnvelope, LedgerEvent};
/// use weighin_core::id::ItemId;
///
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let envelope = EventEnvelope {
///         event: LedgerEvent::ItemAcknowledged(Acknowledged {
///             item_id: ItemId::mint("s1", "Keep the retry at three attempts?"),
///             reason: "left as defaulted after the digest".into(),
///         }),
///         actor: "cli".into(),
///         session_id: Some("s1".into()),
///         worktree: None,
///         branch: None,
///         head: None,
///         ts: "2026-10-02T21:40:02Z".into(),
///     };
///     let bytes = envelope.to_json()?;
///     assert!(String::from_utf8(bytes.clone())?.contains(r#""type":"item_acknowledged""#));
///     assert_eq!(EventEnvelope::from_json(&bytes)?, envelope);
///     Ok(())
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventEnvelope {
    /// The event, flattened into the envelope's JSON object.
    #[serde(flatten)]
    pub event: LedgerEvent,
    /// `agent`, `hook`, `human:<name>` or `cli`.
    pub actor: String,
    /// The writing agent session's id; `None` when the writer had no session.
    #[serde(default)]
    pub session_id: Option<String>,
    /// The directory name of the writer's git worktree; `None` outside git.
    #[serde(default)]
    pub worktree: Option<String>,
    /// The checked-out branch; `None` outside git or on a detached head.
    #[serde(default)]
    pub branch: Option<String>,
    /// The commit at `HEAD`; `None` outside git or before the first commit.
    #[serde(default)]
    pub head: Option<String>,
    /// RFC 3339 UTC.
    pub ts: String,
}

impl EventEnvelope {
    /// `key:value` tags for the store. Values are trimmed to the store's tag limits by the caller.
    #[must_use]
    pub fn tags(&self) -> Vec<(String, String)> {
        let mut t = vec![("item".to_owned(), self.event.item_id().to_string())];
        if let Some(s) = &self.session_id {
            t.push(("session".to_owned(), s.clone()));
        }
        if let Some(w) = &self.worktree {
            t.push(("worktree".to_owned(), w.clone()));
        }
        if let LedgerEvent::ItemRaised(r) = &self.event {
            t.push(("channel".to_owned(), r.channel.as_str().to_owned()));
            t.push(("kind".to_owned(), r.kind.as_str().to_owned()));
            t.push(("door".to_owned(), r.door_hint.as_str().to_owned()));
        }
        t
    }

    /// The envelope as the JSON bytes the store keeps.
    ///
    /// # Errors
    /// Serialisation of a value made of strings and enums does not fail in practice; the error is
    /// surfaced rather than unwrapped.
    pub fn to_json(&self) -> serde_json::Result<Vec<u8>> {
        serde_json::to_vec(self)
    }

    /// Decode an envelope from stored JSON bytes.
    ///
    /// # Errors
    /// Returns the decode error for bytes that are not an envelope.
    pub fn from_json(bytes: &[u8]) -> serde_json::Result<Self> {
        serde_json::from_slice(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_with_the_type_tag() {
        let e = EventEnvelope {
            event: LedgerEvent::ItemDismissed(Dismissed {
                item_id: ItemId::parse("wi-3fa9c1").unwrap(),
                why: DismissReason::DuplicateOf("wi-000aaa".into()),
                note: String::new(),
                reason: "same question raised twice".into(),
            }),
            actor: "cli".into(),
            session_id: Some("s1".into()),
            worktree: None,
            branch: None,
            head: None,
            ts: "2026-09-26T00:00:00Z".into(),
        };
        let bytes = e.to_json().unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert!(text.contains("\"type\":\"item_dismissed\""), "{text}");
        assert_eq!(EventEnvelope::from_json(&bytes).unwrap(), e);
        assert!(EVENT_TYPES.contains(&e.event.event_type()));
    }

    #[test]
    fn every_surface_has_its_wire_spelling() {
        let all = [
            Surface::Chat,
            Surface::Board,
            Surface::Local,
            Surface::Pane,
            Surface::Cli,
        ];

        let spelled = all.map(Surface::as_str);

        assert_eq!(spelled, ["chat", "board", "local", "pane", "cli"]);
    }

    #[test]
    fn dismiss_reasons_parse() {
        assert_eq!(
            DismissReason::parse("not-my-call:legal team"),
            Some(DismissReason::NotMyCall("legal team".into()))
        );
        assert_eq!(DismissReason::parse("already-decided"), None, "needs a ref");
        assert_eq!(DismissReason::parse("moot"), Some(DismissReason::Moot));
    }
}
