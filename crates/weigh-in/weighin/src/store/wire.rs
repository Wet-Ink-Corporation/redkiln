//! Weigh-In's 13 event types as Core envelopes (SPEC EV-01): `weighin.<type>`, version 1,
//! durable, the 1.x payload nested under `data` without its `type`.
//!
//! weighin-core's [`EventEnvelope`] stays the in-memory model (the projection, the 1.x journal
//! lines and `stats` read it); this module maps it to and from the envelope the store keeps. The
//! payloads are Core [`Payload`]s in `weighin-contract` (`names`, Move 1 step 5).

use anyhow::{Context as _, Result};
use redkiln_core::envelope::{
    Actor, Author, Envelope, Namespace, Payload, Skip, Timestamp, Writer, decode,
};
use redkiln_core::ids::{SessionId, WorktreeId};
use redkiln_core::store::{CoreKey, Filter, NewEvent, StoreError, Stored, Tag};
use redkiln_core::trust::Trust;
use weighin_core::event::{
    Acknowledged, Decided, Defaulted, Deferred, Dismissed, EventEnvelope, Grounded, LedgerEvent,
    Linked, Presented, Raised, RecordWritten, Reopened, Superseded, Triaged,
};

/// The binary every envelope weighin writes names (EV-01's `writer`).
///
/// # Errors
/// Never in practice: the name and version are fixed words.
pub(super) fn writer() -> Result<Writer> {
    Writer::new("weighin", env!("CARGO_PKG_VERSION")).context("naming weighin as the writer")
}

/// One table of `(variant, payload)` for the type list and both dispatches (ARC-15).
macro_rules! wire_types {
    ($($variant:ident($payload:ty),)+) => {
        /// Every weighin type of the ledger, in weighin-core's `EVENT_TYPES` order.
        pub(super) const TYPES: [&str; 13] = [$(<$payload as Payload>::TYPE),+];

        /// Each type's declaration, for the test that pins the table.
        #[cfg(test)]
        const DECLARED: [(&str, u32, redkiln_core::envelope::Class); 13] = [$((
            <$payload as Payload>::TYPE,
            <$payload as Payload>::VERSION,
            <$payload as Payload>::CLASS,
        )),+];

        /// The stored event of `event` in `frame`, and both handed back for the ledger.
        fn encode(frame: Frame, event: LedgerEvent, tags: Vec<Tag>) -> Result<(NewEvent, Frame, LedgerEvent)> {
            match event {
                $(LedgerEvent::$variant(payload) => {
                    let (stored, frame, payload) = frame.event(payload, tags)?;
                    Ok((stored, frame, LedgerEvent::$variant(payload)))
                })+
            }
        }

        /// The stored event as weighin-core's envelope; another namespace's type is
        /// [`Skip::Foreign`].
        ///
        /// # Errors
        /// [`Skip::Unusable`] for a weighin type this build cannot use (EV-10).
        pub(super) fn from_stored(stored: &Stored) -> Result<EventEnvelope, Skip> {
            $(
                if stored.event_type == <$payload as Payload>::TYPE {
                    return decode::<$payload>(stored)
                        .map(|read| to_ledger(read, LedgerEvent::$variant));
                }
            )+
            Err(Skip::Foreign)
        }
    };
}

wire_types! {
    ItemRaised(Raised),
    ItemTriaged(Triaged),
    ItemGrounded(Grounded),
    ItemPresented(Presented),
    ItemDecided(Decided),
    ItemDeferred(Deferred),
    ItemDefaulted(Defaulted),
    ItemDismissed(Dismissed),
    ItemReopened(Reopened),
    ItemSuperseded(Superseded),
    ItemLinked(Linked),
    RecordWritten(RecordWritten),
    ItemAcknowledged(Acknowledged),
}

/// The type `item_raised` is written as: the `NotYetRaised` guard's.
pub(super) const RAISED: &str = <Raised as Payload>::TYPE;

/// Every weighin event, of any of the 13 types.
///
/// # Errors
/// Never in practice: the type names are fixed and valid.
pub(super) fn filter() -> Result<Filter, StoreError> {
    TYPES.into_iter().try_fold(Filter::all(), Filter::of_type)
}

/// The tag naming `id`'s item.
///
/// # Errors
/// When the id is not one Core's `item` key holds (EV-12).
pub(super) fn item_tag(id: &weighin_core::ItemId) -> Result<Tag> {
    Tag::core(CoreKey::Item, id.as_str()).with_context(|| format!("tagging the event item:{id}"))
}

/// `env` as the event the store keeps, and `env` handed back unchanged for the ledger. Each
/// envelope field Core refuses is an error naming it, and nothing is built (EV-05); no tag value
/// is truncated (EV-11).
///
/// # Errors
/// A `ts`, `actor`, `session`, `worktree`, `branch`, `head` or tag Core refuses.
pub(super) fn to_new_event(
    env: EventEnvelope,
    writer: &Writer,
) -> Result<(NewEvent, EventEnvelope)> {
    let tags = tags_of(&env.event)?;
    let EventEnvelope {
        event,
        actor,
        session_id,
        worktree,
        branch,
        head,
        ts,
    } = env;
    let frame = Frame {
        ts: Timestamp::parse(&ts).context("the event's ts")?,
        // Each envelope owns its writer: two small words, copied per event.
        author: Author {
            actor: Actor::parse(&actor).context("the event's actor")?,
            trust: Trust::asserted(),
            writer: writer.clone(),
        },
        session: session_id
            .as_deref()
            .map(SessionId::parse)
            .transpose()
            .context("the event's session")?,
        worktree: worktree
            .as_deref()
            .map(WorktreeId::parse)
            .transpose()
            .context("the event's worktree")?,
        branch,
        head,
    };
    let (stored, Frame { branch, head, .. }, event) = encode(frame, event, tags)?;
    Ok((
        stored,
        EventEnvelope {
            event,
            actor,
            session_id,
            worktree,
            branch,
            head,
            ts,
        },
    ))
}

/// The caller's tags of `event` (Core adds `plugin`, `trust`, `session` and `worktree`): its
/// item, and for a raise its channel, kind and door (EV-12).
fn tags_of(event: &LedgerEvent) -> Result<Vec<Tag>> {
    let mut tags = vec![item_tag(event.item_id())?];
    if let LedgerEvent::ItemRaised(raised) = event {
        for (key, value) in [
            ("channel", raised.channel.as_str()),
            ("kind", raised.kind.as_str()),
            ("door", raised.door_hint.as_str()),
        ] {
            tags.push(
                Tag::namespaced(Namespace::Weighin, key, value)
                    .with_context(|| format!("tagging the event weighin.{key}:{value}"))?,
            );
        }
    }
    Ok(tags)
}

/// Every envelope key but `type`, `version` and `data`, checked, for any payload.
struct Frame {
    ts: Timestamp,
    author: Author,
    session: Option<SessionId>,
    worktree: Option<WorktreeId>,
    branch: Option<String>,
    head: Option<String>,
}

impl Frame {
    /// The durable event of `data` in this frame with `tags`, and the frame and `data` back.
    fn event<P: Payload>(self, data: P, tags: Vec<Tag>) -> Result<(NewEvent, Self, P)> {
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
        let stored = envelope
            .to_event(tags)
            .with_context(|| format!("building the {} envelope", P::TYPE))?;
        let Envelope {
            ts,
            actor,
            trust,
            writer,
            session,
            worktree,
            branch,
            head,
            data,
            ..
        } = envelope;
        let frame = Self {
            ts,
            author: Author {
                actor,
                trust,
                writer,
            },
            session,
            worktree,
            branch,
            head,
        };
        Ok((stored, frame, data))
    }
}

/// weighin-core's envelope of a read `envelope`, its payload wrapped by `variant`.
fn to_ledger<P>(envelope: Envelope<P>, variant: fn(P) -> LedgerEvent) -> EventEnvelope {
    EventEnvelope {
        event: variant(envelope.data),
        actor: envelope.actor.to_string(),
        session_id: envelope.session.map(|s| s.as_str().to_owned()),
        worktree: envelope.worktree.map(|w| w.as_str().to_owned()),
        branch: envelope.branch,
        head: envelope.head,
        ts: envelope.ts.as_str().to_owned(),
    }
}

#[cfg(test)]
mod tests;
