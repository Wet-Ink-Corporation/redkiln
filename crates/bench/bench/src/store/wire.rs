//! The 0.x flat [`EventEnvelope`] to and from the Core envelope the store keeps (SPEC EV-01):
//! `bench.<short name>`, version 1, durable, the payload nested under `data` without its
//! `type`, trust `asserted`, writer `bench <version>`.
//!
//! Every field Core refuses is an error naming it, and nothing is built (EV-05); no tag value is
//! truncated (EV-11). Reading back gives the envelope that was written (Invariant 8 of the
//! step 3 plan), so every reader above the store is unchanged.

use bench_contract::event::{self as contract, MigrateWaveDone, MigrateWaveStarted};
use redkiln_core::approval::ApprovalRecorded;
use redkiln_core::envelope::{
    Actor, Author, Envelope, Field, Payload, Skip, Timestamp as CoreTimestamp, Writer,
};
use redkiln_core::ids::{SessionId, WorktreeId};
use redkiln_core::store::{NewEvent, Stored, Tag};
use redkiln_core::trust::Trust;

use super::envelope::{EventEnvelope, Timestamp};
use super::{LedgerEvent, StoreError};

/// The binary every envelope Bench writes names (EV-01's `writer`).
///
/// # Errors
/// Never in practice: the name and version are fixed words.
pub(super) fn writer() -> Result<Writer, StoreError> {
    Writer::new("bench", env!("CARGO_PKG_VERSION")).map_err(StoreError::Field)
}

/// Every envelope key but `type`, `version` and `data`, each checked: the context one write
/// shares between its events (a plan approval and its Core record, D3). The one-time import
/// builds it from an old record's fields (`legacy_import`); the branch and head are checked
/// when an event is built from it.
#[derive(Debug, Clone)]
pub(crate) struct Context {
    /// When the event was written (EV-01).
    pub(crate) ts: CoreTimestamp,
    /// Who wrote it, at what trust, with which binary.
    pub(crate) author: Author,
    /// The writing session, if any.
    pub(crate) session: Option<SessionId>,
    /// The writing worktree, if any.
    pub(crate) worktree: Option<WorktreeId>,
    /// The checked-out branch, if any.
    pub(crate) branch: Option<String>,
    /// The commit at `HEAD`, if any.
    pub(crate) head: Option<String>,
}

impl Context {
    /// The context of `env`, checked as Core reads it.
    ///
    /// The store's envelope owns its fields while it encodes, and the caller keeps `env` (it
    /// prints it, or appends it again after losing a race), so the timestamp, branch and head
    /// are copied: a few short strings per write.
    ///
    /// # Errors
    /// [`StoreError::Field`] for an actor Core refuses, [`StoreError::Id`] for a session or a
    /// worktree that is not an id.
    pub(crate) fn of(env: &EventEnvelope, writer: &Writer) -> Result<Self, StoreError> {
        let id = |field: Field, value: &str, source| StoreError::Id {
            field,
            value: value.to_owned(),
            source,
        };
        Ok(Self {
            ts: env.ts.to_core(),
            author: Author {
                actor: Actor::parse(&env.actor).map_err(StoreError::Field)?,
                trust: Trust::asserted(),
                // Each envelope owns its writer: two short words.
                writer: writer.clone(),
            },
            session: env
                .session_id
                .as_deref()
                .map(|s| SessionId::parse(s).map_err(|e| id(Field::Session, s, e)))
                .transpose()?,
            worktree: env
                .worktree
                .as_deref()
                .map(|w| WorktreeId::parse(w).map_err(|e| id(Field::Worktree, w, e)))
                .transpose()?,
            branch: env.branch.clone(),
            head: env.head.clone(),
        })
    }

    /// The stored event of the Bench event `event` in this context.
    ///
    /// # Errors
    /// As [`to_new_event_in`].
    pub(crate) fn bench_event(self, event: &LedgerEvent) -> Result<NewEvent, StoreError> {
        to_new_event_in(self, event)
    }

    /// The stored Core record of a human's approval in this context.
    ///
    /// # Errors
    /// As [`approval_event`].
    pub(crate) fn approval_event(self, approval: ApprovalRecorded) -> Result<NewEvent, StoreError> {
        approval_event(self, approval)
    }

    /// The durable event of `data` in this context, with the caller's `tags` (Core adds
    /// `plugin`, `trust`, `session` and `worktree`).
    ///
    /// # Errors
    /// [`StoreError::Envelope`] naming the first rule of EV-05 the envelope breaks.
    fn event<P: Payload>(self, data: P, tags: Vec<Tag>) -> Result<NewEvent, StoreError> {
        let Self {
            ts,
            author,
            session,
            worktree,
            branch,
            head,
        } = self;
        let mut envelope = Envelope::new(ts, author, data);
        envelope.session = session;
        envelope.worktree = worktree;
        envelope.branch = branch;
        envelope.head = head;
        envelope
            .to_event(tags)
            .map_err(|source| StoreError::Envelope {
                event_type: P::TYPE,
                source: Box::new(source),
            })
    }
}

/// The event the store keeps for `env`.
///
/// # Errors
/// As [`Context::of`]; [`StoreError::Tag`] for a subject Core refuses as a tag value;
/// [`StoreError::Envelope`] for a field Core refuses when it builds the envelope (the branch,
/// the head, the size).
pub(super) fn to_new_event(env: &EventEnvelope, writer: &Writer) -> Result<NewEvent, StoreError> {
    to_new_event_in(Context::of(env, writer)?, &env.event)
}

/// The event the store keeps for `event` in `context`.
pub(super) fn to_new_event_in(
    context: Context,
    event: &LedgerEvent,
) -> Result<NewEvent, StoreError> {
    let tags: Vec<Tag> = event
        .subject_tag()
        .map_err(StoreError::Tag)?
        .into_iter()
        .collect();
    // Core's envelope owns its payload while it encodes; the caller keeps its event.
    match event.clone() {
        LedgerEvent::TestRun(p) => context.event(p, tags),
        LedgerEvent::TaskDone(p) => context.event(p, tags),
        LedgerEvent::PlanApproved(p) => context.event(p, tags),
        LedgerEvent::GrillConfirmed(p) => context.event(p, tags),
        LedgerEvent::ReproRed(p) => context.event(p, tags),
        LedgerEvent::VerdictSet(p) => context.event(p, tags),
        LedgerEvent::RoundStarted(p) => context.event(p, tags),
        LedgerEvent::GateBlocked(p) => context.event(p, tags),
        LedgerEvent::StopBlocked(p) => context.event(p, tags),
        LedgerEvent::ModeSet(p) => context.event(p, tags),
        LedgerEvent::SubagentStatus(p) => context.event(p, tags),
        LedgerEvent::MigrateWaveStarted(p) => context.event(MigrateWaveStarted(p), tags),
        LedgerEvent::MigrateWaveDone(p) => context.event(MigrateWaveDone(p), tags),
        LedgerEvent::MigrateContract(p) => context.event(p, tags),
        LedgerEvent::WeighinParked(p) => context.event(p, tags),
        LedgerEvent::BridgeUnavailable(p) => context.event(p, tags),
        LedgerEvent::OrchestrateRuled(p) => context.event(p, tags),
    }
}

/// The Core record of a human's approval (AP-01, D3) in `context`, tagged with its subject.
///
/// # Errors
/// [`StoreError::Tag`] for a subject hash Core refuses; as [`Context::event`].
pub(super) fn approval_event(
    context: Context,
    approval: ApprovalRecorded,
) -> Result<NewEvent, StoreError> {
    let subject = Tag::core(
        redkiln_core::store::CoreKey::Subject,
        &approval.subject.hash,
    )
    .map_err(StoreError::Tag)?;
    context.event(approval, vec![subject])
}

/// A stored event as the 0.x flat envelope.
///
/// # Errors
/// [`Skip::Foreign`] for another namespace's event, [`Skip::Unusable`] for a Bench type this
/// build cannot use (EV-10).
pub(super) fn from_stored(stored: &Stored) -> Result<EventEnvelope, Skip> {
    let read = contract::decode(stored)?;
    Ok(EventEnvelope {
        event: read.data,
        actor: read.actor.to_string(),
        session_id: read.session.map(|s| s.as_str().to_owned()),
        worktree: read.worktree.map(|w| w.as_str().to_owned()),
        branch: read.branch,
        head: read.head,
        ts: Timestamp::from_core(read.ts),
    })
}

#[cfg(test)]
mod tests;
