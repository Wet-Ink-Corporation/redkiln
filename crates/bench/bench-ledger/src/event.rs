//! The event registry: every event type the ledger accepts, and its payload.
//!
//! The types live in Bench's contract, [`bench_contract::event`] (Move 2 step 3), and are
//! re-exported here at their old paths, [`LedgerEvent`] being the contract's
//! [`BenchEvent`](bench_contract::BenchEvent). Every payload field name is a persisted JSON
//! key, and every optional field is `#[serde(default)]`, so an event written before the field
//! existed still reads.

pub use bench_contract::event::{
    Actor, AgentStatus, BenchEvent as LedgerEvent, BridgeUnavailable, By, EmptyBy, EventKind,
    GateBlocked, GrillConfirmed, MigrateContract, MigrateWave, Mode, ModeSet, OrchestrateRuled,
    PlanApproved, REGISTRY_VERSION, ReproRed, RoundOfNothing, RoundStarted, StopBlocked,
    SubagentStatus, SubjectKey, TaskDone, TestOutcome, TestRun, TokenUsage, UnknownEventKind,
    Verdict, VerdictSet, VerdictSource, Via, WeighinParked,
};

// Moved from `event/by.rs` (Move 2 step 3, PR 3.2) unchanged: they read a `by` through this
// crate's envelope, so they stay beside it while the rest of `by`'s tests moved with the type.
#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Actor, By, Via};
    use crate::envelope::EventEnvelope;
    use crate::event::{LedgerEvent, PlanApproved, VerdictSet};

    #[test]
    fn a_plan_approval_and_a_verdict_carry_who_gave_them_and_through_what() {
        let approval = json!({
            "type": "plan_approved", "plan": "p.md", "hash": "sha256:00",
            "by": {"actor": "model", "via": "bash"},
            "actor": "cli", "ts": "2026-10-19T09:30:00Z",
        });
        let verdict = json!({
            "type": "verdict_set", "pr": 7, "verdict": "approved", "source": "human",
            "by": {"actor": "human", "via": "mod"},
            "actor": "cli", "ts": "2026-10-19T09:30:00Z",
        });

        let approval = EventEnvelope::from_value(&approval).expect("a registered field");
        let verdict = EventEnvelope::from_value(&verdict).expect("a registered field");

        let by = |event: &LedgerEvent| match event {
            LedgerEvent::PlanApproved(PlanApproved { by, .. })
            | LedgerEvent::VerdictSet(VerdictSet { by, .. }) => *by,
            _ => None,
        };
        assert_eq!(
            (by(&approval.event), by(&verdict.event)),
            (
                By::known(Some(Actor::Model), Some(Via::Bash)),
                By::known(Some(Actor::Human), Some(Via::Mod)),
            ),
            "the payload's `by`, apart from the envelope's `actor`"
        );
        assert_eq!(
            approval.actor, "cli",
            "the writer is still the envelope's actor"
        );
    }

    #[test]
    fn an_empty_by_does_not_read() {
        let approval = json!({
            "type": "plan_approved", "plan": "p.md", "hash": "sha256:00", "by": {},
            "actor": "cli", "ts": "2026-10-19T09:30:00Z",
        });

        let empty = serde_json::from_value::<By>(json!({}));
        let in_event = EventEnvelope::from_value(&approval);

        assert!(
            empty.is_err() && in_event.is_err(),
            "`by: {{}}` claims a giver and names none: {empty:?}, {in_event:?}"
        );
    }
}
