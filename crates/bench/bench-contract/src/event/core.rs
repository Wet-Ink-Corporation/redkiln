//! Core's half of the contract (feature `core`): each payload is a Core [`Payload`] under its
//! `bench.*` name, an event's subject tag in Core's vocabulary (EV-12), and the read of any
//! stored Bench event (EV-08, EV-10).
//!
//! One table (`bench_types!`, ARC-15) generates the [`Payload`] impls, [`TYPES`] and
//! [`decode`]'s dispatch; a test pins it to [`EventKind::ALL`].

use redkiln_core::envelope::{self, Class, Envelope, Namespace, Payload, Skip};
use redkiln_core::store::{CoreKey, Stored, Tag, TagError};
use serde::{Deserialize, Serialize};

use super::{
    BenchEvent, EventKind, GateBlocked, GrillConfirmed, MigrateContract, MigrateWave, ModeSet,
    OrchestrateRuled, PlanApproved, ReproRed, RoundStarted, StopBlocked, SubagentStatus,
    SubjectKey, TaskDone, TestRun, VerdictSet,
};

#[cfg(test)]
mod tests;

/// `bench.migrate.wave.started`'s payload: a [`MigrateWave`] under its own type, since a Core
/// [`Payload`] names one type and [`MigrateWave`] is the payload of two. Its bytes are the
/// wave's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MigrateWaveStarted(pub MigrateWave);

/// `bench.migrate.wave.done`'s payload: a [`MigrateWave`] under its own type, as
/// [`MigrateWaveStarted`]. Its bytes are the wave's.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MigrateWaveDone(pub MigrateWave);

/// The 15 types: each kind, its payload and how the payload becomes a [`BenchEvent`].
macro_rules! bench_types {
    ($($kind:ident: $payload:ty => $wrap:expr,)+) => {
        $(
            impl Payload for $payload {
                const TYPE: &'static str = EventKind::$kind.stored();
                const VERSION: u32 = 1;
                const CLASS: Class = Class::Durable;
            }
        )+

        /// Every Bench type as stored, in [`EventKind::ALL`]'s order.
        pub const TYPES: [&str; 15] = [$(<$payload as Payload>::TYPE),+];

        /// Reads any stored Bench event as an envelope of its [`BenchEvent`].
        ///
        /// # Errors
        /// [`Skip::Foreign`] for an event of another namespace, or a `bench.*` type this build
        /// does not declare (it is not one of the types this build reads, EV-08);
        /// [`Skip::Unusable`] for one of the 15 types in a form this build cannot use: a newer
        /// version, or a payload that does not decode (EV-10), and for every event of a
        /// [retired type](super::RETIRED_TYPES) (D6 B). Neither is a failure of the
        /// read: the caller skips the event and counts the unusable ones.
        pub fn decode(stored: &Stored) -> Result<Envelope<BenchEvent>, Skip> {
            $(
                if stored.event_type == <$payload as Payload>::TYPE {
                    return envelope::decode::<$payload>(stored).map(|read| rewrap(read, $wrap));
                }
            )+
            retired(stored)
        }
    };
}

/// The payload of a retired type (D6 B): it has no value, so Core reads every old event of it
/// as [`Skip::Unusable`] (EV-10), never as a payload.
macro_rules! retired_payload {
    ($($name:ident: $retired:path,)+) => {
        $(
            #[derive(Debug, Serialize, Deserialize)]
            enum $name {}

            impl Payload for $name {
                const TYPE: &'static str = $retired.stored;
                const VERSION: u32 = 1;
                const CLASS: Class = Class::Durable;
            }
        )+

        /// A retired type's event is [`Skip::Unusable`], any other [`Skip::Foreign`].
        fn retired(stored: &Stored) -> Result<Envelope<BenchEvent>, Skip> {
            $(
                if stored.event_type == <$name as Payload>::TYPE {
                    return envelope::decode::<$name>(stored).map(|read| match read.data {});
                }
            )+
            Err(Skip::Foreign)
        }
    };
}

retired_payload! {
    RetiredWeighinParked: super::WEIGHIN_PARKED,
    RetiredBridgeUnavailable: super::BRIDGE_UNAVAILABLE,
}

bench_types! {
    TestRun: TestRun => BenchEvent::TestRun,
    TaskDone: TaskDone => BenchEvent::TaskDone,
    PlanApproved: PlanApproved => BenchEvent::PlanApproved,
    GrillConfirmed: GrillConfirmed => BenchEvent::GrillConfirmed,
    ReproRed: ReproRed => BenchEvent::ReproRed,
    VerdictSet: VerdictSet => BenchEvent::VerdictSet,
    RoundStarted: RoundStarted => BenchEvent::RoundStarted,
    GateBlocked: GateBlocked => BenchEvent::GateBlocked,
    StopBlocked: StopBlocked => BenchEvent::StopBlocked,
    ModeSet: ModeSet => BenchEvent::ModeSet,
    SubagentStatus: SubagentStatus => BenchEvent::SubagentStatus,
    MigrateWaveStarted: MigrateWaveStarted => |MigrateWaveStarted(wave)| BenchEvent::MigrateWaveStarted(wave),
    MigrateWaveDone: MigrateWaveDone => |MigrateWaveDone(wave)| BenchEvent::MigrateWaveDone(wave),
    MigrateContract: MigrateContract => BenchEvent::MigrateContract,
    OrchestrateRuled: OrchestrateRuled => BenchEvent::OrchestrateRuled,
}

/// `read` with its payload moved into a [`BenchEvent`] by `wrap`; every other field moved as
/// it is.
fn rewrap<P>(read: Envelope<P>, wrap: impl FnOnce(P) -> BenchEvent) -> Envelope<BenchEvent> {
    let Envelope {
        ts,
        actor,
        trust,
        writer,
        session,
        worktree,
        branch,
        head,
        origin,
        cites,
        data,
    } = read;
    Envelope {
        ts,
        actor,
        trust,
        writer,
        session,
        worktree,
        branch,
        head,
        origin,
        cites,
        data: wrap(data),
    }
}

impl BenchEvent {
    /// The event's caller tag (EV-12): its subject tag, when it names a subject. `task` and
    /// `pr` are Core's keys; `plan`, `point`, `agent` and `migration` are `bench.<key>`. A
    /// `mode_set`'s subject is the envelope's `worktree`, which Core tags itself, so it has
    /// none; neither has a ticket round or a `test_run` with no task. Core adds `plugin`,
    /// `trust`, `session` and `worktree`.
    ///
    /// # Errors
    /// [`TagError`] when the value is not one its key holds: empty, too long (never shortened,
    /// EV-11), a control character, or pull request 0.
    pub fn subject_tag(&self) -> Result<Option<Tag>, TagError> {
        let Some(value) = self.subject_value() else {
            return Ok(None);
        };
        let tag = match self.kind().subject() {
            SubjectKey::Task => Tag::core(CoreKey::Task, &value),
            SubjectKey::Pr => Tag::core(CoreKey::Pr, &value),
            key @ (SubjectKey::Plan
            | SubjectKey::Point
            | SubjectKey::Agent
            | SubjectKey::Migration) => Tag::namespaced(Namespace::Bench, key.as_str(), &value),
            SubjectKey::Worktree => return Ok(None),
        };
        tag.map(Some)
    }
}
