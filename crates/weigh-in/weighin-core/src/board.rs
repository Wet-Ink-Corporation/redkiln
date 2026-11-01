//! The exchange format with the Artifact board: the state document Claude pushes to
//! `board/state`, and the answer documents the page writes under `answers/<item id>`.
//!
//! The page is `plugin/skills/weigh-in/assets/board.html`. It reads these documents by field
//! name, so the field names here are the page's and renaming one breaks the board.
//!
//! Answers read back are untrusted input (any viewer with edit rights can write them), so
//! [`validate_answer`] checks every field against the ledger before anything is applied, and
//! free text is only ever stored as a note.

use crate::card::{Card, CardOption, Criterion, Evidence, Excerpt, Recommendation};
use crate::marker::Door;
use crate::projection::{Decision, Item, Ledger, Status};
use serde::{Deserialize, Serialize};
use std::cmp::Reverse;

/// What the board's header says the document covers.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Scope {
    /// The repository the items come from, shown in the page header.
    pub repo: String,
    /// The sessions the items come from, shown in the page header (`weighin board export` writes
    /// `session <id>` or `all sessions`).
    pub session: String,
    /// Where the export ran; `weighin board export` writes the git branch here, empty when
    /// unknown. The page does not show it, and a document without it reads as empty.
    #[serde(default)]
    pub source: String,
    /// When the document was built, as an RFC 3339 UTC timestamp. [`export`] sets it from its
    /// `now` argument, whatever the caller passed here; the page shows it as the sync time.
    pub generated_at: String,
    /// The repository's board mode: `artifact`, `local` or `off` (wi-7c21e4). Only `artifact`
    /// repositories publish their board.
    pub board_mode: String,
}

/// Where an item was raised. The page shows it as "Raised in `path` · "quote"".
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Source {
    /// The file the item was raised in, else its `at` anchor, else `session <id>`.
    pub path: String,
    /// The first line of the text the item was raised from, at most 240 characters.
    pub quote: String,
}

/// The decision on a card in [`BoardState::decided`].
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct BoardDecision {
    /// The id of the chosen option.
    pub option: String,
    /// When the decision was recorded in the ledger (the decision event's timestamp).
    pub decided_at: String,
    /// `user-approved` (the recommendation, chosen with the trade-off in view) or `user-directed`
    /// (another option, or an answer the owner stated).
    pub provenance: String,
    /// Whether the choice matched the recommendation; `false` when that was not recorded. The
    /// page marks a `false` as an override.
    pub matched_recommendation: bool,
    /// When the decision was applied to the ledger. [`export`] writes the decision's timestamp,
    /// the same as `decided_at`.
    pub applied_at: String,
    /// The note recorded with the decision; empty when there was none.
    #[serde(default)]
    pub ledger_note: String,
}

/// One card on the board. Field names are the page's.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct BoardItem {
    /// The item's `wi-` id.
    pub id: String,
    /// The card's place in the "Needs you" queue, from 1; 0 on a card in [`BoardState::decided`].
    pub rank: usize,
    /// The question the item asks.
    pub question: String,
    /// The card's one-sentence reason the question matters.
    pub why: String,
    /// `one-way`, `two-way` or `unsure`: the triaged door, else the door it was raised with.
    pub door: String,
    /// The card's remark on the door, shown as "Door: …"; absent when the card has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub door_note: Option<String>,
    /// The triaged blast radius (`file`, `module`, `system` or `external`), or `unknown` before
    /// triage.
    pub blast: String,
    /// `authority` (needs the owner's authority) or `judgment` (the agent may decide and log it).
    pub authority: String,
    /// When the item must be settled, as triage recorded it, or `next sitting` when it named none.
    pub resolve_by: String,
    /// What the item blocks, as it was raised; omitted when it blocks nothing.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub blocks: Vec<String>,
    /// The card's one-sentence result of the decide-now test.
    pub timing: String,
    /// Where the item was raised.
    pub source: Source,
    /// The card's evidence.
    pub evidence: Vec<Evidence>,
    /// The card's code excerpt, if it has one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub excerpt: Option<Excerpt>,
    /// The card's options, in the card's order.
    pub options: Vec<CardOption>,
    /// The card's comparison of the options.
    pub criteria: Vec<Criterion>,
    /// The card's recommendation.
    pub recommendation: Recommendation,
    /// `recommended` or `options-first`.
    pub present: String,
    /// The grounding this card came from; the page echoes it so a stale answer is refused.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub card_rev: Option<u64>,
    /// The decision, on a card in [`BoardState::decided`]; absent on an open card.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<BoardDecision>,
}

/// A default taken without asking the owner. The page lists these under "Decided for you", where
/// each can be reopened.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct BoardDefault {
    /// The item's `wi-` id.
    pub id: String,
    /// The question the item asks.
    pub question: String,
    /// The title of the option taken, or its id when the item has no card naming it.
    pub taken: String,
    /// The titles of the card's other options, joined with `; `; empty without a card.
    pub alt: String,
    /// Why the default was taken.
    pub reason: String,
    /// `two-way`, `expired` or `unattended`.
    pub cause: String,
}

/// An earlier decision the page lists under "Settled", never to be re-asked.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct BoardSettled {
    /// The decision's id, as the page shows it.
    pub id: String,
    /// The question it answered.
    pub question: String,
    /// The answer, as the page shows it.
    pub answer: String,
    /// When it was decided.
    pub decided_at: String,
    /// How it was decided, as the page shows it.
    pub provenance: String,
}

/// The whole board: the document Claude writes to `board/state`.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct BoardState {
    /// The format version. [`export`] writes 1.
    pub schema: u32,
    /// What the document covers.
    pub scope: Scope,
    /// The open cards that need the owner ("Needs you"), ranked.
    pub items: Vec<BoardItem>,
    /// The newest decided cards, newest first.
    #[serde(default)]
    pub decided: Vec<BoardItem>,
    /// The newest unacknowledged defaults, at most [`BOARD_DEFAULTS_CAP`].
    #[serde(default)]
    pub defaults: Vec<BoardDefault>,
    /// Every unacknowledged default, shown or not; the digest covers those past the cap.
    #[serde(default)]
    pub defaults_total: usize,
    /// Earlier decisions. [`export`] leaves this empty.
    #[serde(default)]
    pub settled: Vec<BoardSettled>,
}

/// Whether the recommendation is pre-selected (earned and not a one-way door) or held back.
#[must_use]
pub fn presentation(item: &Item, card: &Card) -> &'static str {
    if item.door() == Door::OneWay || !card.recommendation_earned() {
        "options-first"
    } else {
        "recommended"
    }
}

fn board_item(item: &Item, rank: usize, card: &Card) -> BoardItem {
    BoardItem {
        id: item.id.to_string(),
        rank,
        question: item.question.clone(),
        why: card.why.clone(),
        door: item.door().as_str().to_owned(),
        door_note: card.door_note.clone(),
        blast: item.blast.map_or("unknown", |b| b.as_str()).to_owned(),
        authority: item.authority().as_str().to_owned(),
        resolve_by: item
            .resolve_by
            .clone()
            .unwrap_or_else(|| "next sitting".into()),
        blocks: item.blocks.iter().cloned().collect(),
        timing: card.timing.clone(),
        source: Source {
            path: item
                .source_path
                .clone()
                .or_else(|| item.at.clone())
                .unwrap_or_else(|| {
                    format!("session {}", item.session_id.as_deref().unwrap_or("?"))
                }),
            quote: item
                .quote
                .lines()
                .next()
                .unwrap_or("")
                .chars()
                .take(240)
                .collect(),
        },
        evidence: card.evidence.clone(),
        excerpt: card.excerpt.clone(),
        options: card.options.clone(),
        criteria: card.criteria.clone(),
        recommendation: card.recommendation.clone(),
        present: presentation(item, card).to_owned(),
        card_rev: item.card_rev,
        decision: None,
    }
}

/// The most defaults one board document carries.
///
/// The board state is a single document in the Artifact `db` store, capped at 256 KiB, and a
/// default takes about 370 bytes, so an uncapped list overflows it at about 700 (#19, wi-c34fb3).
pub const BOARD_DEFAULTS_CAP: usize = 200;

/// The most decided cards one board document carries.
const BOARD_DECIDED_CAP: usize = 20;

/// Build the board document from the ledger. Only grounded items make cards; raised-but-ungrounded
/// items make none and need a sitting before they can be shown.
///
/// `focus_cap` bounds the open cards (a cap of 0 still carries one; see `focus_cards`), and
/// `now` becomes [`Scope::generated_at`].
///
/// # Examples
///
/// ```
/// use weighin_core::board::{self, Scope};
/// use weighin_core::event::{Channel, Grounded, LedgerEvent, Raised};
/// use weighin_core::card::{Card, CardOption, Recommendation};
/// use weighin_core::marker::{Authority, Door, Kind};
/// use weighin_core::{EventEnvelope, ItemId, Ledger};
///
/// let id = ItemId::mint("s1", "Which cache?");
/// let envelope = |event| EventEnvelope {
///     event,
///     actor: "agent".into(),
///     session_id: Some("s1".into()),
///     worktree: None,
///     branch: None,
///     head: None,
///     ts: "2026-10-05T09:00:00Z".into(),
/// };
/// let option = |id: &str| CardOption { id: id.into(), title: format!("Option {id}"), ..Default::default() };
/// let events = [
///     envelope(LedgerEvent::ItemRaised(Raised {
///         item_id: id.clone(),
///         kind: Kind::Question,
///         question: "Which cache?".into(),
///         did: None,
///         flip: None,
///         at: None,
///         sym: None,
///         blocks: None,
///         door_hint: Door::TwoWay,
///         authority_hint: Authority::Judgment,
///         channel: Channel::Manual,
///         quote: "Which cache?".into(),
///         source_path: None,
///         source_line: None,
///     })),
///     envelope(LedgerEvent::ItemGrounded(Grounded {
///         item_id: id.clone(),
///         card: Card {
///             options: vec![option("A"), option("B")],
///             recommendation: Recommendation { option: "A".into(), ..Default::default() },
///             ..Default::default()
///         },
///         reason: "grounded".into(),
///     })),
/// ];
/// let ledger = Ledger::fold((1u64..).zip(events.iter()));
///
/// let state = board::export(&ledger, Scope::default(), 4, "2026-10-05T10:00:00Z");
///
/// assert_eq!(state.items.len(), 1);
/// assert_eq!(state.scope.generated_at, "2026-10-05T10:00:00Z");
/// ```
#[must_use]
pub fn export(ledger: &Ledger, mut scope: Scope, focus_cap: usize, now: &str) -> BoardState {
    let (defaults, defaults_total) = board_defaults(ledger);
    now.clone_into(&mut scope.generated_at);
    BoardState {
        schema: 1,
        scope,
        items: focus_cards(ledger, focus_cap),
        decided: decided_cards(ledger),
        defaults,
        defaults_total,
        settled: Vec::new(),
    }
}

/// The open, grounded, not deferred items in rank order, at most `focus_cap` of them and at least
/// one when any qualifies: the cap is checked after each card, so a cap of 0 acts as 1.
fn focus_cards(ledger: &Ledger, focus_cap: usize) -> Vec<BoardItem> {
    let mut items = Vec::new();
    for i in ledger.open() {
        if i.status == Status::Deferred {
            continue;
        }
        if let Some(card) = &i.card {
            items.push(board_item(i, items.len() + 1, card));
            if items.len() >= focus_cap {
                break;
            }
        }
    }
    items
}

/// The sort key that puts the newest decision last: its time, then the item's position.
fn decided_order(item: &Item) -> (Option<&str>, u64) {
    (
        item.decision.as_ref().map(|d| d.ts.as_str()),
        item.last_position,
    )
}

/// Decided items that have a card, newest first, at most [`BOARD_DECIDED_CAP`] of them.
fn decided_cards(ledger: &Ledger) -> Vec<BoardItem> {
    let mut decided: Vec<(&Item, &Card)> = ledger
        .items
        .values()
        .filter(|i| i.status == Status::Decided)
        .filter_map(|i| i.card.as_ref().map(|card| (i, card)))
        .collect();
    // Newest decision first. A review (D-0022) appends an event but decides nothing, so the
    // order is by decision time, then by position for decisions stamped in the same second.
    decided.sort_by(|(a, _), (b, _)| decided_order(b).cmp(&decided_order(a)));
    decided
        .into_iter()
        .take(BOARD_DECIDED_CAP)
        .map(|(i, card)| BoardItem {
            decision: i.decision.as_ref().map(board_decision),
            ..board_item(i, 0, card)
        })
        .collect()
}

fn board_decision(d: &Decision) -> BoardDecision {
    BoardDecision {
        option: d.option.clone(),
        decided_at: d.ts.clone(),
        provenance: d.provenance.as_str().to_owned(),
        matched_recommendation: d.matched_recommendation.unwrap_or(false),
        applied_at: d.ts.clone(),
        ledger_note: d.note.clone(),
    }
}

/// The newest unacknowledged defaults, at most [`BOARD_DEFAULTS_CAP`], and how many there are.
fn board_defaults(ledger: &Ledger) -> (Vec<BoardDefault>, usize) {
    let mut unreviewed = ledger.unacknowledged_defaults();
    let total = unreviewed.len();
    unreviewed.sort_by_key(|i| Reverse(i.last_position));
    let defaults = unreviewed
        .into_iter()
        .take(BOARD_DEFAULTS_CAP)
        .filter_map(board_default)
        .collect();
    (defaults, total)
}

fn board_default(i: &Item) -> Option<BoardDefault> {
    let d = i.default.as_ref()?;
    let alt = i
        .card
        .as_ref()
        .map(|c| {
            c.options
                .iter()
                .filter(|o| o.id != d.option)
                .map(|o| o.title.clone())
                .collect::<Vec<_>>()
                .join("; ")
        })
        .unwrap_or_default();
    let taken = i
        .card
        .as_ref()
        .and_then(|c| c.option(&d.option))
        .map_or_else(|| d.option.clone(), |o| o.title.clone());
    Some(BoardDefault {
        id: i.id.to_string(),
        question: i.question.clone(),
        taken,
        alt,
        reason: d.reason.clone(),
        cause: d.cause.as_str().to_owned(),
    })
}

/// An answer document as the page writes it. Other fields the page writes are ignored.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct Answer {
    /// The `wi-` id of the item answered; an unambiguous prefix also resolves.
    pub item_id: String,
    /// `choose`, `defer`, `dismiss`, `not-my-call` or `reopen`.
    pub action: String,
    /// The id of the chosen option, for `choose`.
    #[serde(default)]
    pub option: Option<String>,
    /// The viewer's free-text note. It is only ever stored as a note.
    #[serde(default)]
    pub note: Option<String>,
    /// Why the item is dismissed, for `dismiss`: `not-useful`, `already-decided`,
    /// `recoverable`, `duplicate` or `moot`.
    #[serde(default)]
    pub reason: Option<String>,
    /// Who should decide instead, for `not-my-call`.
    #[serde(default)]
    pub who: Option<String>,
    /// The deferral, for `defer`.
    #[serde(default)]
    pub defer: Option<AnswerDefer>,
    /// When the viewer saved the answer, by the browser's clock.
    #[serde(default)]
    pub decided_at: Option<String>,
    /// Seconds from opening the card to saving the answer.
    #[serde(default)]
    pub seconds_to_decide: Option<u64>,
    /// When the answer was applied to the ledger; an answer that carries one is not applied
    /// again.
    #[serde(default)]
    pub applied_at: Option<String>,
    /// The card revision the viewer answered.
    #[serde(default)]
    pub card_rev: Option<u64>,
}

/// The deferral in a `defer` answer. All three parts are required.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
pub struct AnswerDefer {
    /// The fact the deferral waits for.
    pub trigger: String,
    /// When the item must be settled.
    pub resolve_by: String,
    /// The id of the option that applies if nobody acts.
    pub default: String,
}

/// What an answer turns into, once validated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidAnswer {
    /// Decide the item with an option.
    Choose {
        /// The id of an option on the item's card that fails no MUST.
        option: String,
        /// The viewer's note, cleaned; empty when there was none.
        note: String,
        /// Seconds from opening the card to answering, as the page measured it.
        seconds: Option<u64>,
    },
    /// Defer the item until a fact arrives.
    Defer {
        /// The fact the deferral waits for, cleaned.
        trigger: String,
        /// When the item must be settled, cleaned.
        resolve_by: String,
        /// The id of the option that applies if nobody acts; it fails no MUST.
        default: String,
        /// The viewer's note, cleaned; empty when there was none.
        note: String,
    },
    /// Dismiss the item as needing no decision.
    Dismiss {
        /// `not-useful`, `already-decided`, `recoverable`, `duplicate` or `moot`.
        reason: String,
        /// The viewer's note, cleaned; empty when there was none.
        note: String,
    },
    /// Hand the item to someone else to decide.
    NotMyCall {
        /// Who should decide, cleaned and not blank.
        who: String,
        /// The viewer's note, cleaned; empty when there was none.
        note: String,
    },
    /// Reopen a decided, defaulted or dismissed item as a question.
    Reopen {
        /// The viewer's note, cleaned; empty when there was none.
        note: String,
    },
}

/// Why a board answer cannot be applied.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AnswerError {
    /// No item in the ledger matches the answer's id.
    #[error("no item {0} in the ledger")]
    UnknownItem(String),
    /// The item is not open, or for a reopen not decided, defaulted or dismissed; with its status.
    #[error("{0} is not open (status {1})")]
    NotOpen(String, String),
    /// The item's card has no option with that id.
    #[error("{0} has no option `{1}`")]
    UnknownOption(String, String),
    /// The option fails a MUST.
    #[error("option `{1}` of {0} fails a MUST and cannot be chosen")]
    Ineligible(String, String),
    /// A deferral without a trigger, a resolve-by or a default.
    #[error("a deferral needs a trigger, a resolve-by and a default")]
    IncompleteDeferral,
    /// A dismissal reason that is not one of the five, or a hand-off naming no decider.
    #[error(
        "dismiss reason `{0}` is not one of not-useful, already-decided, recoverable, duplicate, moot"
    )]
    BadReason(String),
    /// An action the board does not have.
    #[error("unknown action `{0}`")]
    UnknownAction(String),
    /// The answer already carries `applied_at`.
    #[error("already applied")]
    AlreadyApplied,
    /// The item was grounded again after the viewer read the card: the item's card revision and
    /// the answer's.
    #[error(
        "{0} was re-grounded after this answer was given (card {1:?}, answer {2:?}); ask again"
    )]
    StaleCard(String, Option<u64>, Option<u64>),
}

fn text(v: Option<&String>) -> String {
    v.map(|s| {
        s.chars()
            .filter(|c| !c.is_control() || *c == '\n')
            .take(2000)
            .collect()
    })
    .unwrap_or_default()
}

/// Check an answer against the ledger.
///
/// The checks run in a fixed order, so an answer that breaks several rules gets the first:
/// already applied, unknown item, then (for `reopen`) whether the item is closed, then whether
/// the item is open, then whether a `choose` or `defer` was read from the current card, then the
/// action's own fields. Free text loses its control characters (newlines stay) and is cut to
/// 2000 characters.
///
/// # Errors
/// Returns why the answer cannot be applied.
///
/// # Examples
///
/// ```
/// use weighin_core::Ledger;
/// use weighin_core::board::{self, Answer, AnswerError};
///
/// let answer = Answer {
///     item_id: "wi-0a1b2c".into(),
///     action: "choose".into(),
///     option: Some("A".into()),
///     ..Default::default()
/// };
///
/// let refused = board::validate_answer(&Ledger::default(), &answer);
///
/// assert_eq!(refused, Err(AnswerError::UnknownItem("wi-0a1b2c".into())));
/// ```
pub fn validate_answer(ledger: &Ledger, a: &Answer) -> Result<ValidAnswer, AnswerError> {
    if a.applied_at.is_some() {
        return Err(AnswerError::AlreadyApplied);
    }
    let item = ledger
        .resolve(&a.item_id)
        .ok_or_else(|| AnswerError::UnknownItem(a.item_id.clone()))?;
    let id = item.id.to_string();
    let note = text(a.note.as_ref());
    if a.action == "reopen" {
        return validate_reopen(item, id, note);
    }
    if !item.status.is_open() {
        return Err(AnswerError::NotOpen(id, item.status.as_str().into()));
    }
    // An option letter means nothing without the card it was read from.
    if matches!(a.action.as_str(), "choose" | "defer")
        && item.card_rev.is_some()
        && a.card_rev != item.card_rev
    {
        return Err(AnswerError::StaleCard(id, item.card_rev, a.card_rev));
    }
    match a.action.as_str() {
        "choose" => validate_choose(a, item, &id, note),
        "defer" => validate_defer(a, item, &id, note),
        "dismiss" => validate_dismiss(a, note),
        "not-my-call" => validate_hand_off(a, note),
        other => Err(AnswerError::UnknownAction(other.into())),
    }
}

/// Only a decided, defaulted or dismissed item can be reopened.
fn validate_reopen(item: &Item, id: String, note: String) -> Result<ValidAnswer, AnswerError> {
    if matches!(
        item.status,
        Status::Defaulted | Status::Dismissed | Status::Decided
    ) {
        Ok(ValidAnswer::Reopen { note })
    } else {
        Err(AnswerError::NotOpen(id, item.status.as_str().into()))
    }
}

/// The option exists on the item's card and fails no MUST.
fn eligible_option(item: &Item, id: &str, option: &str) -> Result<(), AnswerError> {
    match item.card.as_ref().and_then(|c| c.option(option)) {
        Some(opt) if opt.fails_must.is_some() => {
            Err(AnswerError::Ineligible(id.to_owned(), option.into()))
        }
        Some(_) => Ok(()),
        None => Err(AnswerError::UnknownOption(id.to_owned(), option.into())),
    }
}

fn validate_choose(
    a: &Answer,
    item: &Item,
    id: &str,
    note: String,
) -> Result<ValidAnswer, AnswerError> {
    let option = a.option.clone().unwrap_or_default();
    eligible_option(item, id, &option)?;
    Ok(ValidAnswer::Choose {
        option,
        note,
        seconds: a.seconds_to_decide,
    })
}

fn validate_defer(
    a: &Answer,
    item: &Item,
    id: &str,
    note: String,
) -> Result<ValidAnswer, AnswerError> {
    let d = a.defer.as_ref().ok_or(AnswerError::IncompleteDeferral)?;
    if d.trigger.trim().is_empty() || d.resolve_by.trim().is_empty() || d.default.trim().is_empty()
    {
        return Err(AnswerError::IncompleteDeferral);
    }
    eligible_option(item, id, &d.default)?;
    Ok(ValidAnswer::Defer {
        trigger: text(Some(&d.trigger)),
        resolve_by: text(Some(&d.resolve_by)),
        default: d.default.clone(),
        note,
    })
}

fn validate_dismiss(a: &Answer, note: String) -> Result<ValidAnswer, AnswerError> {
    let reason = a.reason.clone().unwrap_or_default();
    if !matches!(
        reason.as_str(),
        "not-useful" | "already-decided" | "recoverable" | "duplicate" | "moot"
    ) {
        return Err(AnswerError::BadReason(reason));
    }
    Ok(ValidAnswer::Dismiss { reason, note })
}

fn validate_hand_off(a: &Answer, note: String) -> Result<ValidAnswer, AnswerError> {
    let who = text(a.who.as_ref());
    if who.trim().is_empty() {
        return Err(AnswerError::BadReason(
            "not-my-call without a decider".into(),
        ));
    }
    Ok(ValidAnswer::NotMyCall { who, note })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::Stability;
    use crate::event::{
        Acknowledged, Channel, Decided, DefaultCause, Defaulted, Deferred, EventEnvelope, Grounded,
        LedgerEvent, Provenance, Raised,
    };
    use crate::id::ItemId;
    use crate::marker::{Authority, Kind};

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn env(event: LedgerEvent) -> EventEnvelope {
        EventEnvelope {
            event,
            actor: "test".into(),
            session_id: Some("s".into()),
            worktree: None,
            branch: None,
            head: None,
            ts: "2026-09-26T00:00:00Z".into(),
        }
    }

    fn id(q: &str) -> ItemId {
        ItemId::mint("s", q)
    }

    fn raised(q: &str, door: Door) -> EventEnvelope {
        env(LedgerEvent::ItemRaised(Raised {
            item_id: id(q),
            kind: Kind::Question,
            question: q.into(),
            did: None,
            flip: None,
            at: None,
            sym: None,
            blocks: None,
            door_hint: door,
            authority_hint: Authority::Authority,
            channel: Channel::Marker,
            quote: format!("{q}\nsecond line"),
            source_path: Some("src/lib.rs".into()),
            source_line: Some(7),
        }))
    }

    fn opt(id: &str) -> CardOption {
        CardOption {
            id: id.into(),
            title: format!("Option {id}"),
            ..Default::default()
        }
    }

    fn card() -> Card {
        let mut ineligible = opt("C");
        ineligible.fails_must = Some("breaks the public contract".into());
        Card {
            why: "because".into(),
            options: vec![opt("A"), opt("B"), ineligible],
            recommendation: Recommendation {
                option: "A".into(),
                ..Default::default()
            },
            ..Default::default()
        }
    }

    fn grounded(q: &str) -> EventEnvelope {
        env(LedgerEvent::ItemGrounded(Grounded {
            item_id: id(q),
            card: card(),
            reason: "grounded".into(),
        }))
    }

    fn decided(q: &str) -> EventEnvelope {
        env(LedgerEvent::ItemDecided(Decided {
            item_id: id(q),
            option: "B".into(),
            decider: "human:ryan".into(),
            provenance: Provenance::UserDirected,
            note: "a note".into(),
            tree_hash: None,
            matched_recommendation: Some(false),
            via: "board".into(),
            seconds_to_decide: Some(12),
            reason: "picked".into(),
        }))
    }

    fn deferred(q: &str) -> EventEnvelope {
        env(LedgerEvent::ItemDeferred(Deferred {
            item_id: id(q),
            flip: "the DPA".into(),
            trigger: "the DPA review".into(),
            resolve_by: "2026-10-15".into(),
            default_option: "A".into(),
            owner: "ryan".into(),
            unqualified: false,
            reason: "waiting".into(),
        }))
    }

    fn defaulted(q: &str, option: &str) -> EventEnvelope {
        env(LedgerEvent::ItemDefaulted(Defaulted {
            item_id: id(q),
            option: option.into(),
            cause: DefaultCause::TwoWay,
            reason: format!("default for {q}"),
        }))
    }

    fn fold(evs: &[EventEnvelope]) -> Ledger {
        Ledger::fold((1u64..).zip(evs.iter()))
    }

    /// Two open cards, one deferred card, one ungrounded item, two decided items (one without a
    /// card) and two defaults (one without a card).
    fn mixed_ledger() -> Ledger {
        fold(&[
            raised("open two-way?", Door::TwoWay),
            grounded("open two-way?"),
            raised("open one-way?", Door::OneWay),
            grounded("open one-way?"),
            raised("deferred?", Door::TwoWay),
            grounded("deferred?"),
            deferred("deferred?"),
            raised("ungrounded?", Door::TwoWay),
            raised("decided old?", Door::TwoWay),
            grounded("decided old?"),
            decided("decided old?"),
            raised("decided new?", Door::TwoWay),
            grounded("decided new?"),
            decided("decided new?"),
            raised("decided bare?", Door::TwoWay),
            decided("decided bare?"),
            raised("default carded?", Door::TwoWay),
            grounded("default carded?"),
            defaulted("default carded?", "B"),
            raised("default bare?", Door::TwoWay),
            defaulted("default bare?", "X"),
        ])
    }

    fn scope() -> Scope {
        Scope {
            repo: "weigh-in".into(),
            session: "session s".into(),
            source: "main".into(),
            generated_at: "ignored".into(),
            board_mode: "artifact".into(),
        }
    }

    fn ids(items: &[BoardItem]) -> Vec<String> {
        items.iter().map(|b| b.id.clone()).collect()
    }

    #[test]
    fn export_shows_open_grounded_cards_ranked_one_way_first() {
        let state = export(&mixed_ledger(), scope(), 5, "2026-10-05T10:00:00Z");
        assert_eq!(
            ids(&state.items),
            vec![
                id("open one-way?").to_string(),
                id("open two-way?").to_string()
            ]
        );
        let ranks: Vec<usize> = state.items.iter().map(|b| b.rank).collect();
        assert_eq!(ranks, vec![1, 2]);
    }

    #[test]
    fn export_stops_at_the_focus_cap() {
        let state = export(&mixed_ledger(), scope(), 1, "now");
        assert_eq!(ids(&state.items), vec![id("open one-way?").to_string()]);
    }

    #[test]
    fn export_stamps_the_scope_and_schema() {
        let state = export(&mixed_ledger(), scope(), 5, "2026-10-05T10:00:00Z");
        let mut expected = scope();
        expected.generated_at = "2026-10-05T10:00:00Z".into();
        assert_eq!(state.scope, expected);
        assert_eq!(state.schema, 1);
        assert_eq!(state.settled, Vec::new());
    }

    #[test]
    fn export_lists_decided_cards_newest_first_with_their_decision() -> TestResult {
        let state = export(&mixed_ledger(), scope(), 5, "now");
        assert_eq!(
            ids(&state.decided),
            vec![
                id("decided new?").to_string(),
                id("decided old?").to_string()
            ]
        );
        let first = state.decided.first().ok_or("no decided card")?;
        assert_eq!(first.rank, 0);
        assert_eq!(
            first.decision,
            Some(BoardDecision {
                option: "B".into(),
                decided_at: "2026-09-26T00:00:00Z".into(),
                provenance: "user-directed".into(),
                matched_recommendation: false,
                applied_at: "2026-09-26T00:00:00Z".into(),
                ledger_note: "a note".into(),
            })
        );
        Ok(())
    }

    /// A review (D-0022) is a later event on an older decision; it must not move that decision
    /// ahead of one taken after it.
    #[test]
    fn a_reviewed_old_decision_stays_behind_a_newer_decision() {
        let at = |ts: &str, event: LedgerEvent| EventEnvelope {
            ts: ts.into(),
            ..env(event)
        };
        let state = export(
            &fold(&[
                raised("decided old?", Door::TwoWay),
                grounded("decided old?"),
                at("2026-09-01T00:00:00Z", decided("decided old?").event),
                raised("decided new?", Door::TwoWay),
                grounded("decided new?"),
                at("2026-09-20T00:00:00Z", decided("decided new?").event),
                at(
                    "2026-12-01T00:00:00Z",
                    LedgerEvent::ItemAcknowledged(Acknowledged {
                        item_id: id("decided old?"),
                        reason: "still holds".into(),
                    }),
                ),
            ]),
            scope(),
            5,
            "now",
        );
        assert_eq!(
            ids(&state.decided),
            vec![
                id("decided new?").to_string(),
                id("decided old?").to_string()
            ]
        );
    }

    #[test]
    fn export_keeps_the_twenty_newest_decided_cards() -> TestResult {
        let mut evs = Vec::new();
        for n in 0..21 {
            let q = format!("decided {n}?");
            evs.push(raised(&q, Door::TwoWay));
            evs.push(grounded(&q));
            evs.push(decided(&q));
        }
        let state = export(&fold(&evs), scope(), 5, "now");
        assert_eq!(state.decided.len(), 20);
        let first = state.decided.first().ok_or("no decided card")?;
        assert_eq!(first.id, id("decided 20?").to_string());
        assert!(!ids(&state.decided).contains(&id("decided 0?").to_string()));
        Ok(())
    }

    #[test]
    fn export_lists_defaults_with_taken_and_alternatives() {
        let state = export(&mixed_ledger(), scope(), 5, "now");
        assert_eq!(state.defaults_total, 2);
        assert_eq!(
            state.defaults,
            vec![
                BoardDefault {
                    id: id("default bare?").to_string(),
                    question: "default bare?".into(),
                    taken: "X".into(),
                    alt: String::new(),
                    reason: "default for default bare?".into(),
                    cause: "two-way".into(),
                },
                BoardDefault {
                    id: id("default carded?").to_string(),
                    question: "default carded?".into(),
                    taken: "Option B".into(),
                    alt: "Option A; Option C".into(),
                    reason: "default for default carded?".into(),
                    cause: "two-way".into(),
                },
            ]
        );
    }

    #[test]
    fn export_caps_defaults_but_counts_them_all() -> TestResult {
        let mut evs = Vec::new();
        for n in 0..=BOARD_DEFAULTS_CAP {
            let q = format!("default {n}?");
            evs.push(raised(&q, Door::TwoWay));
            evs.push(defaulted(&q, "A"));
        }
        let state = export(&fold(&evs), scope(), 5, "now");
        assert_eq!(state.defaults.len(), BOARD_DEFAULTS_CAP);
        assert_eq!(state.defaults_total, BOARD_DEFAULTS_CAP + 1);
        let newest = state.defaults.first().ok_or("no default")?;
        assert_eq!(
            newest.id,
            id(&format!("default {BOARD_DEFAULTS_CAP}?")).to_string()
        );
        Ok(())
    }

    #[test]
    fn export_renders_a_card_field_by_field() -> TestResult {
        let state = export(&mixed_ledger(), scope(), 5, "now");
        let item = state.items.first().ok_or("no card")?;
        let json = serde_json::to_string(item)?;
        let expected = format!(
            concat!(
                r#"{{"id":"{}","rank":1,"question":"open one-way?","why":"because","door":"one-way","#,
                r#""blast":"unknown","authority":"authority","resolve_by":"next sitting","#,
                r#""timing":"","source":{{"path":"src/lib.rs","quote":"open one-way?"}},"evidence":[],"#,
                r#""options":[{{"id":"A","title":"Option A","summary":"","holds_if":"","if_wrong":""}},"#,
                r#"{{"id":"B","title":"Option B","summary":"","holds_if":"","if_wrong":""}},"#,
                r#"{{"id":"C","title":"Option C","summary":"","holds_if":"","if_wrong":"","#,
                r#""fails_must":"breaks the public contract"}}],"criteria":[],"#,
                r#""recommendation":{{"option":"A","flip":"","tradeoff":"","why_wrong":""}},"#,
                r#""present":"options-first","card_rev":4}}"#
            ),
            id("open one-way?")
        );
        assert_eq!(json, expected);
        Ok(())
    }

    #[test]
    fn presentation_holds_back_an_unstable_recommendation() -> TestResult {
        let l = fold(&[raised("two?", Door::TwoWay), grounded("two?")]);
        let item = l.get(&id("two?")).ok_or("not raised")?;
        let mut c = card();
        assert_eq!(presentation(item, &c), "recommended");
        c.recommendation.stability = Some(Stability {
            agree: 1,
            of: 4,
            ..Default::default()
        });
        assert_eq!(presentation(item, &c), "options-first");
        Ok(())
    }

    // ---- validate_answer -------------------------------------------------------------------

    fn answer(q: &str, action: &str) -> Answer {
        Answer {
            item_id: id(q).to_string(),
            action: action.into(),
            card_rev: Some(4),
            ..Default::default()
        }
    }

    /// `open?` is grounded at position 2, so its card revision is 2.
    fn card_ledger() -> Ledger {
        fold(&[
            raised("open?", Door::TwoWay),
            grounded("open?"),
            raised("decided?", Door::TwoWay),
            grounded("decided?"),
            decided("decided?"),
        ])
    }

    fn open_answer(action: &str) -> Answer {
        Answer {
            card_rev: Some(2),
            ..answer("open?", action)
        }
    }

    #[test]
    fn an_applied_answer_is_refused_before_anything_else() {
        let a = Answer {
            item_id: "wi-ffffff".into(),
            applied_at: Some("2026-10-05T00:00:00Z".into()),
            ..Default::default()
        };
        assert_eq!(
            validate_answer(&card_ledger(), &a),
            Err(AnswerError::AlreadyApplied)
        );
    }

    #[test]
    fn an_unknown_item_is_refused() {
        let a = Answer {
            item_id: "wi-ffffff".into(),
            action: "choose".into(),
            ..Default::default()
        };
        assert_eq!(
            validate_answer(&card_ledger(), &a),
            Err(AnswerError::UnknownItem("wi-ffffff".into()))
        );
    }

    #[test]
    fn reopen_applies_to_a_closed_item_and_keeps_the_note() {
        let a = Answer {
            note: Some("rethink\u{7}\nplease".into()),
            ..answer("decided?", "reopen")
        };
        assert_eq!(
            validate_answer(&card_ledger(), &a),
            Ok(ValidAnswer::Reopen {
                note: "rethink\nplease".into()
            })
        );
    }

    #[test]
    fn reopen_of_an_open_item_is_refused() {
        assert_eq!(
            validate_answer(&card_ledger(), &open_answer("reopen")),
            Err(AnswerError::NotOpen(
                id("open?").to_string(),
                "grounded".into()
            ))
        );
    }

    #[test]
    fn an_answer_to_a_closed_item_is_refused_before_the_card_check() {
        let a = Answer {
            card_rev: Some(99),
            ..answer("decided?", "choose")
        };
        assert_eq!(
            validate_answer(&card_ledger(), &a),
            Err(AnswerError::NotOpen(
                id("decided?").to_string(),
                "decided".into()
            ))
        );
    }

    #[test]
    fn a_choice_read_from_an_older_card_is_stale() {
        let a = Answer {
            option: Some("Z".into()),
            card_rev: Some(1),
            ..open_answer("choose")
        };
        assert_eq!(
            validate_answer(&card_ledger(), &a),
            Err(AnswerError::StaleCard(
                id("open?").to_string(),
                Some(2),
                Some(1)
            ))
        );
    }

    #[test]
    fn a_deferral_read_from_no_card_is_stale() {
        let a = Answer {
            card_rev: None,
            ..open_answer("defer")
        };
        assert_eq!(
            validate_answer(&card_ledger(), &a),
            Err(AnswerError::StaleCard(
                id("open?").to_string(),
                Some(2),
                None
            ))
        );
    }

    #[test]
    fn a_dismissal_ignores_the_card_revision() {
        let a = Answer {
            reason: Some("moot".into()),
            card_rev: None,
            ..open_answer("dismiss")
        };
        assert_eq!(
            validate_answer(&card_ledger(), &a),
            Ok(ValidAnswer::Dismiss {
                reason: "moot".into(),
                note: String::new()
            })
        );
    }

    #[test]
    fn a_choice_of_an_eligible_option_is_accepted() {
        let a = Answer {
            option: Some("B".into()),
            note: Some("fine".into()),
            seconds_to_decide: Some(30),
            ..open_answer("choose")
        };
        assert_eq!(
            validate_answer(&card_ledger(), &a),
            Ok(ValidAnswer::Choose {
                option: "B".into(),
                note: "fine".into(),
                seconds: Some(30)
            })
        );
    }

    #[test]
    fn a_choice_of_an_unknown_option_is_refused() {
        assert_eq!(
            validate_answer(&card_ledger(), &open_answer("choose")),
            Err(AnswerError::UnknownOption(
                id("open?").to_string(),
                String::new()
            ))
        );
    }

    #[test]
    fn a_choice_of_an_option_that_fails_a_must_is_refused() {
        let a = Answer {
            option: Some("C".into()),
            ..open_answer("choose")
        };
        assert_eq!(
            validate_answer(&card_ledger(), &a),
            Err(AnswerError::Ineligible(id("open?").to_string(), "C".into()))
        );
    }

    #[test]
    fn a_choice_on_an_ungrounded_item_names_no_option() {
        let l = fold(&[raised("bare?", Door::TwoWay)]);
        let a = Answer {
            option: Some("A".into()),
            card_rev: None,
            ..answer("bare?", "choose")
        };
        assert_eq!(
            validate_answer(&l, &a),
            Err(AnswerError::UnknownOption(
                id("bare?").to_string(),
                "A".into()
            ))
        );
    }

    fn defer(trigger: &str, resolve_by: &str, default: &str) -> Answer {
        Answer {
            defer: Some(AnswerDefer {
                trigger: trigger.into(),
                resolve_by: resolve_by.into(),
                default: default.into(),
            }),
            ..open_answer("defer")
        }
    }

    #[test]
    fn a_deferral_without_its_parts_is_incomplete() {
        let l = card_ledger();
        assert_eq!(
            validate_answer(&l, &open_answer("defer")),
            Err(AnswerError::IncompleteDeferral)
        );
        assert_eq!(
            validate_answer(&l, &defer(" ", "2026-10-15", "A")),
            Err(AnswerError::IncompleteDeferral)
        );
        assert_eq!(
            validate_answer(&l, &defer("the DPA", "", "A")),
            Err(AnswerError::IncompleteDeferral)
        );
        assert_eq!(
            validate_answer(&l, &defer("the DPA", "2026-10-15", "\t")),
            Err(AnswerError::IncompleteDeferral)
        );
    }

    #[test]
    fn a_deferral_to_an_ineligible_default_is_refused() {
        assert_eq!(
            validate_answer(&card_ledger(), &defer("the DPA", "2026-10-15", "C")),
            Err(AnswerError::Ineligible(id("open?").to_string(), "C".into()))
        );
    }

    #[test]
    fn a_complete_deferral_is_accepted_with_its_text_cleaned() {
        let a = Answer {
            note: Some("later".into()),
            ..defer("the\u{1b} DPA", "2026-10-15", "A")
        };
        assert_eq!(
            validate_answer(&card_ledger(), &a),
            Ok(ValidAnswer::Defer {
                trigger: "the DPA".into(),
                resolve_by: "2026-10-15".into(),
                default: "A".into(),
                note: "later".into()
            })
        );
    }

    #[test]
    fn a_dismissal_needs_a_known_reason() {
        let a = Answer {
            reason: Some("boring".into()),
            ..open_answer("dismiss")
        };
        assert_eq!(
            validate_answer(&card_ledger(), &a),
            Err(AnswerError::BadReason("boring".into()))
        );
    }

    #[test]
    fn a_hand_off_needs_a_decider() {
        let a = Answer {
            who: Some(" \u{7} ".into()),
            ..open_answer("not-my-call")
        };
        assert_eq!(
            validate_answer(&card_ledger(), &a),
            Err(AnswerError::BadReason(
                "not-my-call without a decider".into()
            ))
        );
    }

    #[test]
    fn a_hand_off_names_the_decider() {
        let a = Answer {
            who: Some("legal".into()),
            note: Some("theirs".into()),
            ..open_answer("not-my-call")
        };
        assert_eq!(
            validate_answer(&card_ledger(), &a),
            Ok(ValidAnswer::NotMyCall {
                who: "legal".into(),
                note: "theirs".into()
            })
        );
    }

    #[test]
    fn an_unknown_action_is_refused() {
        assert_eq!(
            validate_answer(&card_ledger(), &open_answer("shrug")),
            Err(AnswerError::UnknownAction("shrug".into()))
        );
    }

    #[test]
    fn free_text_is_capped_at_two_thousand_characters() -> TestResult {
        let a = Answer {
            option: Some("A".into()),
            note: Some("x".repeat(2100)),
            ..open_answer("choose")
        };
        let ValidAnswer::Choose { note, .. } = validate_answer(&card_ledger(), &a)? else {
            return Err("not a choice".into());
        };
        assert_eq!(note.chars().count(), 2000);
        Ok(())
    }
}
