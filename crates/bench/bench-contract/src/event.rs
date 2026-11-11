//! Bench's events: the 17 types, their payloads and the 0.x flat form anneal's ledger wrote.
//!
//! Each type is stored as `bench.<short name>` ([`EventKind::stored`]), version 1, durable
//! (SPEC EV-03, EV-04, EV-14); since step 4 of `docs/plan/move-2-bench.md` the `bench ledger`
//! CLI names it by that stored name too, and reads the short name ([`EventKind::as_str`]) only
//! as a `--type` filter, with a warning. Every payload field name
//! is a persisted JSON key, and every optional field is `#[serde(default)]`, so an event written
//! before the field existed still reads; changing a payload's bytes needs a new version (EV-06).
//!
//! With the `core` feature, every payload is a Core
//! [`Payload`](redkiln_core::envelope::Payload) under its stored name,
//! [`BenchEvent::subject_tag`] gives an event's subject tag and [`decode`] reads any stored
//! Bench event.

use std::borrow::Cow;
use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

mod by;
#[cfg(feature = "core")]
mod core;
mod payload;
#[cfg(test)]
mod tests;

#[cfg(feature = "core")]
pub use self::core::{MigrateWaveDone, MigrateWaveStarted, TYPES, decode};
pub use by::{Actor, By, EmptyBy, Via};
pub use payload::{
    AgentStatus, BridgeUnavailable, GateBlocked, GrillConfirmed, MigrateContract, MigrateWave,
    Mode, ModeSet, OrchestrateRuled, PlanApproved, ReproRed, RoundOfNothing, RoundStarted,
    StopBlocked, SubagentStatus, TaskDone, TestOutcome, TestRun, TokenUsage, Verdict, VerdictSet,
    VerdictSource, WeighinParked,
};

/// The version of anneal's 0.x event registry, the last whose shapes the one-time import reads.
///
/// Frozen at 10: Core versions each type on its own (EV-06), and `bench ledger` no longer
/// prints it (step 4). It was bumped when an event type
/// or a payload field was added, except `by` on `plan_approved` and `verdict_set` (#62), added
/// at 9 without a bump. Version 10 added the `wip` status of `subagent_status`.
pub const REGISTRY_VERSION: u32 = 10;

/// The name of an event type. The set is closed: an event whose type is not listed here is not
/// Bench's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EventKind {
    /// `test_run`
    TestRun,
    /// `task_done`
    TaskDone,
    /// `plan_approved`
    PlanApproved,
    /// `grill_confirmed`
    GrillConfirmed,
    /// `repro_red`
    ReproRed,
    /// `verdict_set`
    VerdictSet,
    /// `round_started`
    RoundStarted,
    /// `gate_blocked`
    GateBlocked,
    /// `stop_blocked`
    StopBlocked,
    /// `mode_set`
    ModeSet,
    /// `subagent_status`
    SubagentStatus,
    /// `migrate.wave.started`
    MigrateWaveStarted,
    /// `migrate.wave.done`
    MigrateWaveDone,
    /// `migrate.contract`
    MigrateContract,
    /// `weighin_parked`
    WeighinParked,
    /// `bridge.unavailable`
    BridgeUnavailable,
    /// `orchestrate.ruled`
    OrchestrateRuled,
}

impl EventKind {
    /// Every registered type, in registry order.
    pub const ALL: [Self; 17] = [
        Self::TestRun,
        Self::TaskDone,
        Self::PlanApproved,
        Self::GrillConfirmed,
        Self::ReproRed,
        Self::VerdictSet,
        Self::RoundStarted,
        Self::GateBlocked,
        Self::StopBlocked,
        Self::ModeSet,
        Self::SubagentStatus,
        Self::MigrateWaveStarted,
        Self::MigrateWaveDone,
        Self::MigrateContract,
        Self::WeighinParked,
        Self::BridgeUnavailable,
        Self::OrchestrateRuled,
    ];

    /// The short name, as anneal's 0.x ledger wrote it: snake case. Bench writes it only
    /// inside its in-memory flat form and the outputs of the commands that record an event.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::TestRun => "test_run",
            Self::TaskDone => "task_done",
            Self::PlanApproved => "plan_approved",
            Self::GrillConfirmed => "grill_confirmed",
            Self::ReproRed => "repro_red",
            Self::VerdictSet => "verdict_set",
            Self::RoundStarted => "round_started",
            Self::GateBlocked => "gate_blocked",
            Self::StopBlocked => "stop_blocked",
            Self::ModeSet => "mode_set",
            Self::SubagentStatus => "subagent_status",
            Self::MigrateWaveStarted => "migrate.wave.started",
            Self::MigrateWaveDone => "migrate.wave.done",
            Self::MigrateContract => "migrate.contract",
            Self::WeighinParked => "weighin_parked",
            Self::BridgeUnavailable => "bridge.unavailable",
            Self::OrchestrateRuled => "orchestrate.ruled",
        }
    }

    /// The type as the suite store keeps it: `bench.` and the short name (EV-03).
    #[must_use]
    pub const fn stored(self) -> &'static str {
        match self {
            Self::TestRun => "bench.test_run",
            Self::TaskDone => "bench.task_done",
            Self::PlanApproved => "bench.plan_approved",
            Self::GrillConfirmed => "bench.grill_confirmed",
            Self::ReproRed => "bench.repro_red",
            Self::VerdictSet => "bench.verdict_set",
            Self::RoundStarted => "bench.round_started",
            Self::GateBlocked => "bench.gate_blocked",
            Self::StopBlocked => "bench.stop_blocked",
            Self::ModeSet => "bench.mode_set",
            Self::SubagentStatus => "bench.subagent_status",
            Self::MigrateWaveStarted => "bench.migrate.wave.started",
            Self::MigrateWaveDone => "bench.migrate.wave.done",
            Self::MigrateContract => "bench.migrate.contract",
            Self::WeighinParked => "bench.weighin_parked",
            Self::BridgeUnavailable => "bench.bridge.unavailable",
            Self::OrchestrateRuled => "bench.orchestrate.ruled",
        }
    }

    /// The kind stored as `name` ([`EventKind::stored`], `bench.task_done`), or `None` for any
    /// other name: another namespace's type, or a short name, which [`str::parse`] reads.
    #[must_use]
    pub fn from_stored(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.stored() == name)
    }

    /// The key of the subject tag events of this type carry.
    #[must_use]
    pub const fn subject(self) -> SubjectKey {
        match self {
            // An orchestrated ticket is the task its events are about.
            Self::TestRun
            | Self::TaskDone
            | Self::ReproRed
            | Self::WeighinParked
            | Self::BridgeUnavailable
            | Self::OrchestrateRuled => SubjectKey::Task,
            Self::PlanApproved | Self::GrillConfirmed => SubjectKey::Plan,
            Self::VerdictSet | Self::RoundStarted => SubjectKey::Pr,
            Self::GateBlocked | Self::StopBlocked => SubjectKey::Point,
            // The payload names no worktree: the envelope's `worktree` is the subject.
            Self::ModeSet => SubjectKey::Worktree,
            Self::SubagentStatus => SubjectKey::Agent,
            Self::MigrateWaveStarted | Self::MigrateWaveDone | Self::MigrateContract => {
                SubjectKey::Migration
            }
        }
    }
}

impl fmt::Display for EventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A name that is not a registered event type.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown event type {name:?} (registry v{REGISTRY_VERSION})")]
pub struct UnknownEventKind {
    /// The name that was given.
    pub name: String,
}

impl FromStr for EventKind {
    type Err = UnknownEventKind;

    /// Parses a short name (the 0.x one), not a stored `bench.` one.
    fn from_str(name: &str) -> Result<Self, Self::Err> {
        Self::ALL
            .into_iter()
            .find(|kind| kind.as_str() == name)
            .ok_or_else(|| UnknownEventKind {
                name: name.to_owned(),
            })
    }
}

/// The key of an event's subject tag: what the event is about.
///
/// The short keys are the 0.x ledger's (`task`, `plan`, ...). In the suite store `task` and
/// `pr` are Core's keys and the others `bench.<key>` (EV-12); `worktree` is the envelope's,
/// which Core tags itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SubjectKey {
    /// `task:<task id>`
    Task,
    /// `plan:<plan id>`
    Plan,
    /// `pr:<number>`
    Pr,
    /// `point:<gate or decision point>`
    Point,
    /// `agent:<subagent type>`
    Agent,
    /// `migration:<migration id>`
    Migration,
    /// `worktree:<worktree>`, from the envelope.
    Worktree,
}

impl SubjectKey {
    /// Every key, in documentation order.
    pub const ALL: [Self; 7] = [
        Self::Task,
        Self::Plan,
        Self::Pr,
        Self::Point,
        Self::Agent,
        Self::Migration,
        Self::Worktree,
    ];

    /// The short key, before the `:`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Task => "task",
            Self::Plan => "plan",
            Self::Pr => "pr",
            Self::Point => "point",
            Self::Agent => "agent",
            Self::Migration => "migration",
            Self::Worktree => "worktree",
        }
    }
}

impl fmt::Display for SubjectKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An event in the 0.x flat form: its short type (the `type` key) and its payload, flattened
/// into one JSON object.
///
/// The form anneal's ledger records have, which the one-time import reads, and Bench's
/// in-memory event; `bench ledger` reads and prints Core's EV-01 envelope instead (its `type`
/// [`EventKind::stored`], this payload under `data`) since step 4.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BenchEvent {
    /// A test command ran.
    TestRun(TestRun),
    /// A task was marked done.
    TaskDone(TaskDone),
    /// A plan was approved at a content hash.
    PlanApproved(PlanApproved),
    /// The human confirmed a plan's grilling.
    GrillConfirmed(GrillConfirmed),
    /// A reproduction test was seen failing before a fix.
    ReproRed(ReproRed),
    /// A review verdict was set on a pull request.
    VerdictSet(VerdictSet),
    /// A review round started on a pull request.
    RoundStarted(RoundStarted),
    /// A gate refused to let work proceed.
    GateBlocked(GateBlocked),
    /// The Stop hook refused to let the session stop.
    StopBlocked(StopBlocked),
    /// A worktree entered or left a bench mode.
    ModeSet(ModeSet),
    /// A subagent stopped, with the status block its last message gave (or the violation).
    SubagentStatus(SubagentStatus),
    /// A wave of a migration started (registry v7).
    #[serde(rename = "migrate.wave.started")]
    MigrateWaveStarted(MigrateWave),
    /// A wave of a migration finished (registry v7).
    #[serde(rename = "migrate.wave.done")]
    MigrateWaveDone(MigrateWave),
    /// A migration's contract step ran: the old shape is removed (registry v7).
    #[serde(rename = "migrate.contract")]
    MigrateContract(MigrateContract),
    /// A one-way call was parked in weigh-in with a default (registry v9).
    WeighinParked(WeighinParked),
    /// The weigh-in bridge could not park a call, so it was ruled instead (registry v9).
    #[serde(rename = "bridge.unavailable")]
    BridgeUnavailable(BridgeUnavailable),
    /// The orchestrator ruled on a reversible call itself (registry v9).
    #[serde(rename = "orchestrate.ruled")]
    OrchestrateRuled(OrchestrateRuled),
}

impl BenchEvent {
    /// The event's registered type.
    #[must_use]
    pub const fn kind(&self) -> EventKind {
        match self {
            Self::TestRun(_) => EventKind::TestRun,
            Self::TaskDone(_) => EventKind::TaskDone,
            Self::PlanApproved(_) => EventKind::PlanApproved,
            Self::GrillConfirmed(_) => EventKind::GrillConfirmed,
            Self::ReproRed(_) => EventKind::ReproRed,
            Self::VerdictSet(_) => EventKind::VerdictSet,
            Self::RoundStarted(_) => EventKind::RoundStarted,
            Self::GateBlocked(_) => EventKind::GateBlocked,
            Self::StopBlocked(_) => EventKind::StopBlocked,
            Self::ModeSet(_) => EventKind::ModeSet,
            Self::SubagentStatus(_) => EventKind::SubagentStatus,
            Self::MigrateWaveStarted(_) => EventKind::MigrateWaveStarted,
            Self::MigrateWaveDone(_) => EventKind::MigrateWaveDone,
            Self::MigrateContract(_) => EventKind::MigrateContract,
            Self::WeighinParked(_) => EventKind::WeighinParked,
            Self::BridgeUnavailable(_) => EventKind::BridgeUnavailable,
            Self::OrchestrateRuled(_) => EventKind::OrchestrateRuled,
        }
    }

    /// The value of the event's subject tag (its key is [`EventKind::subject`]), or `None` for
    /// a `test_run` that names no task. Borrowed except for a pull request number, which is
    /// formatted.
    #[must_use]
    pub fn subject_value(&self) -> Option<Cow<'_, str>> {
        match self {
            Self::TestRun(TestRun { task, .. }) => task.as_deref().map(Cow::Borrowed),
            Self::TaskDone(TaskDone { task }) | Self::ReproRed(ReproRed { task, .. }) => {
                Some(Cow::Borrowed(task))
            }
            Self::WeighinParked(WeighinParked { ticket, .. })
            | Self::BridgeUnavailable(BridgeUnavailable { ticket, .. })
            | Self::OrchestrateRuled(OrchestrateRuled { ticket, .. }) => {
                Some(Cow::Borrowed(ticket))
            }
            Self::PlanApproved(PlanApproved { plan, .. })
            | Self::GrillConfirmed(GrillConfirmed { plan, .. }) => Some(Cow::Borrowed(plan)),
            Self::VerdictSet(VerdictSet { pr, .. }) => Some(Cow::Owned(pr.to_string())),
            // A ticket round has no pull request, so no subject tag.
            Self::RoundStarted(round) => round.pr().map(|pr| Cow::Owned(pr.to_string())),
            Self::GateBlocked(GateBlocked { point, .. })
            | Self::StopBlocked(StopBlocked { point, .. }) => Some(Cow::Borrowed(point)),
            Self::SubagentStatus(SubagentStatus { agent, .. }) => Some(Cow::Borrowed(agent)),
            Self::MigrateWaveStarted(MigrateWave { migration, .. })
            | Self::MigrateWaveDone(MigrateWave { migration, .. })
            | Self::MigrateContract(MigrateContract { migration }) => {
                Some(Cow::Borrowed(migration))
            }
            // Its subject is the envelope's `worktree`, tagged from the envelope.
            Self::ModeSet(_) => None,
        }
    }
}
