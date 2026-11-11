//! An old anneal or dev `bench` ledger's records mapped for Core's import (Move 2 step 3.5, SPEC
//! ST-15, JR-07, AP-05).
//!
//! Each record is the 0.x flat envelope anneal wrote, read leniently (any older registry's
//! shape the `bench.*` payloads still read, unknown keys ignored), and becomes the `bench.<name>`
//! envelope of its type under its old identity, written by this `bench`. Field by field (D8):
//!
//! - `ts`: kept when Core takes it, else normalised through its Unix milliseconds (anneal
//!   dropped a millisecond stamp's trailing zeros, and a hand-written one may carry an offset);
//! - `actor` and `session_id`: as Core parses them; a record Core refuses there is skipped and
//!   reported (JR-10);
//! - `worktree`: anneal wrote the top-level path, ID-06's input, so it becomes the id this build
//!   computes for it; a worktree, branch or head Core refuses is dropped and counted, the event
//!   kept.
//!
//! A `plan_approved` whose `by.actor` is `human` is also a `core.approval_recorded` (AP-05, D3)
//! decided by the suite owner, stored just before it under the identity D4 derives
//! ([`approval_store`]); one by a model, with no `by`, with a hash that is no subject or with no
//! owner is not, and is counted.

use std::path::Path;

use bench_contract::event::{Actor as ByActor, By, PlanApproved};
use redkiln_core::envelope::{Actor, Author, EnvelopeError, Field, FieldError, Timestamp, Writer};
use redkiln_core::ids::{Host, SessionId, WorktreeId};
use redkiln_core::journal::Mapped;
use redkiln_core::store::{NewEvent, Stored};
use redkiln_core::subject::Subject;
use redkiln_core::suite::Owner;
use redkiln_core::trust::Trust;
use redkiln_core::{EventId, StoreId};
use serde::Deserialize;
use sha2::{Digest as _, Sha256};

use crate::error_chain::reason;
use crate::plan::decider;
use crate::store::{EventContext, LedgerEvent, StoreError};

/// The most problems one source keeps, in record order; the counts hold every one.
pub(super) const MAX_PROBLEMS: usize = 32;

/// The domain D4's derived identity hashes before the old store id.
const APPROVAL_DOMAIN: &str = "bench-approval:";

/// What mapping one source found beyond Core's counts.
#[derive(Debug, Default)]
pub(crate) struct Tally {
    /// Human plan approvals also imported as `core.approval_recorded`.
    pub(crate) approvals: u64,
    /// Human plan approvals with no owner to decide them: not recorded for other plugins.
    pub(crate) no_owner: u64,
    /// Plan approvals by a model or by no one known: Bench's event only.
    pub(crate) model_or_unknown: u64,
    /// Human plan approvals whose hash is no `sha256:` subject: Bench's event only.
    pub(crate) bad_hash: u64,
    /// Worktree, branch and head values Core refuses, dropped from events that were kept.
    pub(crate) context_dropped: u64,
    /// Why records were skipped or an approval was not recorded, by record number: the first
    /// [`MAX_PROBLEMS`].
    pub(crate) problems: Vec<(u64, String)>,
    /// Why the owner could not be asked of git, when it could not.
    pub(crate) owner_error: Option<String>,
}

impl Tally {
    fn problem(&mut self, number: u64, why: String) {
        if self.problems.len() < MAX_PROBLEMS {
            self.problems.push((number, why));
        }
    }
}

/// The records of one old ledger as Core imports them, produced lazily; what mapping them found
/// is kept in [`Records::into_tally`].
#[derive(Debug)]
pub(super) struct Records<'a, F> {
    events: std::slice::Iter<'a, Stored>,
    writer: &'a Writer,
    approvals: StoreId,
    owner: AskedOwner<F>,
    /// The Bench event that follows the approval just produced, under the same number.
    pending: Option<(u64, Mapped)>,
    tally: Tally,
}

/// The suite owner, asked for once, when a human approval first needs it.
#[derive(Debug)]
enum AskedOwner<F> {
    Unasked(F),
    Known(Option<Owner>),
    /// Asked: taken out while the question runs.
    Asking,
}

impl<'a, F: FnOnce() -> Result<Option<Owner>, String>> Records<'a, F> {
    /// The records of `events`, read from the old ledger whose id is `store_id`, written by
    /// `writer`; `owner` names the suite owner who decides a human's approval.
    pub(super) fn new(
        events: &'a [Stored],
        store_id: StoreId,
        writer: &'a Writer,
        owner: F,
    ) -> Self {
        Self {
            events: events.iter(),
            writer,
            approvals: approval_store(store_id),
            owner: AskedOwner::Unasked(owner),
            pending: None,
            tally: Tally::default(),
        }
    }

    /// What mapping found.
    pub(super) fn into_tally(self) -> Tally {
        self.tally
    }

    /// The owner, asking once.
    fn owner(&mut self) -> Option<&Owner> {
        if let AskedOwner::Unasked(_) = self.owner
            && let AskedOwner::Unasked(ask) = std::mem::replace(&mut self.owner, AskedOwner::Asking)
        {
            let known = ask().unwrap_or_else(|why| {
                self.tally.owner_error = Some(why);
                None
            });
            self.owner = AskedOwner::Known(known);
        }
        match &self.owner {
            AskedOwner::Known(owner) => owner.as_ref(),
            AskedOwner::Unasked(_) | AskedOwner::Asking => None,
        }
    }

    /// `stored` as Core imports it: the approval to store first, if any, and its Bench event.
    fn map(&mut self, stored: &Stored) -> Result<(Option<Mapped>, Mapped), String> {
        let old: OldEnvelope = serde_json::from_slice(&stored.data)
            .map_err(|e| format!("not an anneal event: {e}"))?;
        if old.event.kind().as_str() != stored.event_type {
            return Err(format!(
                "the event is {} but stored as {}",
                old.event.kind().as_str(),
                stored.event_type
            ));
        }
        let (event, context) = self.context(old)?;
        let (built, context) = self.bench_event(context, &event)?;
        let approval = match &event {
            LedgerEvent::PlanApproved(approved) => self.approval(stored, approved, context),
            _ => None,
        };
        Ok((
            approval,
            Mapped::Event {
                id: stored.id,
                event: built,
            },
        ))
    }

    /// The event of `old` and its envelope context, the worktree dropped and counted when Core
    /// refuses it.
    fn context(&mut self, old: OldEnvelope) -> Result<(LedgerEvent, EventContext), String> {
        let ts = timestamp(&old.ts).ok_or_else(|| format!("the ts {:?} is no time", old.ts))?;
        let actor = Actor::parse(&old.actor)
            .map_err(|e| format!("the actor {:?} is refused: {}", old.actor, reason(&e)))?;
        let session = old
            .session_id
            .as_deref()
            .map(|s| SessionId::parse(s).map_err(|e| format!("the session {s:?}: {}", reason(&e))))
            .transpose()?;
        let worktree = old.worktree.as_deref().and_then(|value| {
            let id = worktree(value);
            if id.is_none() {
                self.tally.context_dropped = self.tally.context_dropped.saturating_add(1);
            }
            id
        });
        let context = EventContext {
            ts,
            author: Author {
                actor,
                trust: Trust::asserted(),
                // Each record's envelope owns its writer: two short words.
                writer: self.writer.clone(),
            },
            session,
            worktree,
            branch: old.branch,
            head: old.head,
        };
        Ok((old.event, context))
    }

    /// `event` in `context`, dropping and counting a branch or head Core refuses; the context
    /// the event was built in.
    fn bench_event(
        &mut self,
        mut context: EventContext,
        event: &LedgerEvent,
    ) -> Result<(NewEvent, EventContext), String> {
        loop {
            // Core's envelope owns its context while it encodes; a retry, and the approval
            // record, need it again.
            match context.clone().bench_event(event) {
                Ok(built) => return Ok((built, context)),
                Err(error) => {
                    let dropped = match refused_field(&error) {
                        Some(Field::Branch) => context.branch.take().is_some(),
                        Some(Field::Head) => context.head.take().is_some(),
                        _ => false,
                    };
                    if !dropped {
                        return Err(reason(&error));
                    }
                    self.tally.context_dropped = self.tally.context_dropped.saturating_add(1);
                }
            }
        }
    }

    /// The Core record of `approved` when a human gave it and an owner decides it (AP-05),
    /// under D4's identity; counted when not.
    fn approval(
        &mut self,
        stored: &Stored,
        approved: &PlanApproved,
        context: EventContext,
    ) -> Option<Mapped> {
        if approved.by.and_then(By::actor) != Some(ByActor::Human) {
            self.tally.model_or_unknown = self.tally.model_or_unknown.saturating_add(1);
            return None;
        }
        let Ok(subject) = Subject::parse(&approved.hash) else {
            self.tally.bad_hash = self.tally.bad_hash.saturating_add(1);
            return None;
        };
        let Some(owner) = self.owner() else {
            self.tally.no_owner = self.tally.no_owner.saturating_add(1);
            return None;
        };
        let record = decider::approval_of(owner, &subject, &approved.plan);
        match context.approval_event(record) {
            Ok(event) => {
                self.tally.approvals = self.tally.approvals.saturating_add(1);
                Some(Mapped::Event {
                    id: EventId::new(self.approvals, stored.id.position()),
                    event,
                })
            }
            Err(error) => {
                let why = format!(
                    "the approval was not recorded for other plugins: {}",
                    reason(&error)
                );
                self.tally.problem(stored.position, why);
                None
            }
        }
    }
}

impl<F: FnOnce() -> Result<Option<Owner>, String>> Iterator for Records<'_, F> {
    type Item = (u64, Mapped);

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(pending) = self.pending.take() {
            return Some(pending);
        }
        let stored = self.events.next()?;
        let number = stored.position;
        match self.map(stored) {
            Ok((Some(approval), event)) => {
                self.pending = Some((number, event));
                Some((number, approval))
            }
            Ok((None, event)) => Some((number, event)),
            Err(why) => {
                self.tally.problem(number, why);
                Some((number, Mapped::Unmapped))
            }
        }
    }
}

/// An old record as anneal wrote it: the event flattened beside its context. Read leniently:
/// a key no type defines is ignored, and every optional field may be missing.
#[derive(Debug, Deserialize)]
struct OldEnvelope {
    #[serde(flatten)]
    event: LedgerEvent,
    actor: String,
    #[serde(default)]
    session_id: Option<String>,
    #[serde(default)]
    worktree: Option<String>,
    #[serde(default)]
    branch: Option<String>,
    #[serde(default)]
    head: Option<String>,
    ts: String,
}

/// The envelope field Core refused when it built an event, if that was the reason.
fn refused_field(error: &StoreError) -> Option<Field> {
    match error {
        StoreError::Envelope { source, .. } => match source.as_ref() {
            EnvelopeError::Field(FieldError { field, .. }) => Some(*field),
            _ => None,
        },
        _ => None,
    }
}

/// The store id of the approvals imported from the old ledger `old` (D4): the first 16 bytes
/// of `sha256("bench-approval:" ‖ <old store id, 32 hex>)`. Deterministic, so a second import
/// finds them present.
pub(super) fn approval_store(old: StoreId) -> StoreId {
    let digest: [u8; 32] = Sha256::digest(format!("{APPROVAL_DOMAIN}{old}")).into();
    let mut id = [0u8; 16];
    for (slot, byte) in id.iter_mut().zip(digest) {
        *slot = byte;
    }
    StoreId::from_bytes(id)
}

/// The worktree id for an old `worktree` value: the id itself, or the id of the top-level path
/// anneal wrote; `None` for a value Core refuses as either (a relative path).
pub(super) fn worktree(value: &str) -> Option<WorktreeId> {
    if let Ok(id) = WorktreeId::parse(value) {
        return Some(id);
    }
    // A refused value is dropped and counted by the caller (D8).
    WorktreeId::of_toplevel(Path::new(value), Host::current()).ok()
}

/// An old `ts` as Core's timestamp: kept when Core takes it, else read as RFC 3339 (any count
/// of fraction digits, `Z` or an offset) and written through its Unix milliseconds. Digits past
/// the millisecond are dropped. `None` for text that is no RFC 3339 time.
pub(super) fn timestamp(text: &str) -> Option<Timestamp> {
    if let Ok(ts) = Timestamp::parse(text) {
        return Some(ts);
    }
    let (stamp, rest) = text.split_at_checked(19)?;
    let (date, time) = stamp.split_at_checked(10)?;
    let time = time.strip_prefix(['T', 't'])?;
    let (fraction, zone) = match rest.strip_prefix('.') {
        Some(after) => {
            let digits = after.bytes().take_while(u8::is_ascii_digit).count();
            match after.split_at_checked(digits)? {
                ("", _) => return None,
                split => split,
            }
        }
        None => ("", rest),
    };
    let millis: String = fraction.chars().chain("000".chars()).take(3).collect();
    let offset = offset_minutes(zone)?;
    let utc = Timestamp::parse(&format!("{date}T{time}.{millis}Z")).ok()?;
    let shifted = utc.unix_millis().checked_sub(offset.checked_mul(60_000)?)?;
    Timestamp::from_unix_millis(shifted).ok()
}

/// The offset `zone` names, in minutes east of UTC: `Z` (either case) or `±HH:MM`.
fn offset_minutes(zone: &str) -> Option<i64> {
    if zone.eq_ignore_ascii_case("z") {
        return Some(0);
    }
    let (sign, clock) = zone.split_at_checked(1)?;
    let (hours, minutes) = clock.split_once(':')?;
    let two_digits = |part: &str| {
        (part.len() == 2 && part.bytes().all(|b| b.is_ascii_digit()))
            .then(|| part.parse::<i64>().ok())
            .flatten()
    };
    let (hours, minutes) = (two_digits(hours)?, two_digits(minutes)?);
    if hours > 23 || minutes > 59 {
        return None;
    }
    let east = hours.checked_mul(60)?.checked_add(minutes)?;
    match sign {
        "+" => Some(east),
        "-" => east.checked_neg(),
        _ => None,
    }
}

#[cfg(test)]
mod tests;
