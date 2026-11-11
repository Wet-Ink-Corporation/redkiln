// Moved from `bench-ledger`'s `envelope.rs` (Move 2 step 3, PR 3.4). Changed under D9:
// `a_timestamp_reads_as_milliseconds_since_the_unix_epoch_and_an_offset_is_refused` (was `..._in_any_offset`,
// class ii), `missing_optional_fields_read_as_none` (its `+02:00` ts is `Z`, class ii),
// `now_millis_is_the_clock_s_millisecond_in_utc` (three digits always, class ii) and
// `tags_are_session_worktree_and_the_subject` (a `wt-` worktree id, class iii). The others are
// as they were. Changed under C2 (Move 2 step 4, PR 4.3): every parse reads Core's EV-01
// envelope (`bench.*` type, payload under `data`, `session`) where it read the 0.x flat form,
// unknown payload keys are named from `data.`, and an unknown type is its own error.

use super::{EventEnvelope, SchemaError, Timestamp, TimestampError};
use crate::store::tag::{TagError, TagKey};
use crate::store::{EventKind, LedgerEvent};
use bench_contract::event::{TaskDone, Verdict, VerdictSet};
use serde_json::json;

fn ts() -> Timestamp {
    Timestamp::try_from("2026-10-11T09:30:00Z".to_owned()).expect("a valid timestamp")
}

#[test]
fn a_timestamp_reads_as_milliseconds_since_the_unix_epoch_and_an_offset_is_refused() {
    let utc = Timestamp::try_from("1970-01-01T00:00:01.250Z".to_owned()).expect("valid");
    let offset = Timestamp::try_from("1970-01-01T02:00:01+02:00".to_owned());

    assert_eq!(utc.unix_millis(), 1250, "fractions are kept");
    assert!(
        matches!(offset, Err(TimestampError::Parse { .. })),
        "EV-01 timestamps are UTC with a Z: {offset:?}"
    );
}

fn task_done(task: &str) -> EventEnvelope {
    let mut envelope = EventEnvelope::new(
        LedgerEvent::TaskDone(TaskDone {
            task: task.to_owned(),
        }),
        "cli",
        ts(),
    );
    envelope.session_id = Some("s1".to_owned());
    envelope
}

#[test]
fn an_ev01_envelope_reads_into_the_event_which_serialises_flat() {
    let envelope = task_done("T1");

    let json = serde_json::to_value(&envelope).expect("serialises");
    let parsed = EventEnvelope::from_ev01(&json!({
        "type": "bench.task_done", "version": 1, "ts": "2026-10-11T09:30:00Z", "actor": "cli",
        "session": "s1", "worktree": null, "data": {"task": "T1"},
    }))
    .expect("reads");

    assert_eq!(
        json,
        json!({
            "type": "task_done", "task": "T1", "actor": "cli", "session_id": "s1",
            "worktree": null, "branch": null, "head": null, "ts": "2026-10-11T09:30:00Z",
        }),
        "type and payload are flattened beside the context"
    );
    assert_eq!(parsed, envelope, "the envelope's payload and context");
}

#[test]
fn missing_optional_fields_read_as_none() {
    let parsed = EventEnvelope::from_ev01(&json!({
        "type": "bench.verdict_set", "data": {"pr": 12, "verdict": "noise_only"},
        "actor": "hook", "ts": "2026-10-11T09:30:00Z",
    }))
    .expect("optional fields may be left out");

    assert_eq!(
        parsed.event,
        LedgerEvent::VerdictSet(VerdictSet {
            pr: 12,
            verdict: Verdict::NoiseOnly,
            source: None,
            sha: None,
            patch_id: None,
            base: None,
            by: None,
        }),
        "payload read"
    );
    assert_eq!(parsed.session_id, None, "no session");
}

#[test]
fn unknown_types_and_missing_fields_are_schema_errors() {
    let unknown = EventEnvelope::from_ev01(&json!({
        "type": "bench.task_started", "data": {}, "actor": "cli", "ts": "2026-10-11T09:30:00Z",
    }));
    let missing = EventEnvelope::from_ev01(&json!({
        "type": "bench.task_done", "data": {}, "actor": "cli", "ts": "2026-10-11T09:30:00Z",
    }));

    assert!(
        matches!(&unknown, Err(SchemaError::UnknownType { name }) if name == "bench.task_started"),
        "{unknown:?}"
    );
    assert!(
        matches!(&missing, Err(SchemaError::Json(e)) if e.to_string().contains("task")),
        "{missing:?}"
    );
}

#[test]
fn a_short_name_another_plugin_s_type_or_another_version_is_refused() {
    let event = |kind: &str, version: u64| {
        EventEnvelope::from_ev01(&json!({
            "type": kind, "version": version, "data": {"task": "T1"}, "actor": "cli",
            "ts": "2026-10-11T09:30:00Z",
        }))
    };

    let short = event("task_done", 1);
    let foreign = event("weighin.task_done", 1);
    let newer = event("bench.task_done", 2);

    assert!(
        matches!(&short, Err(error @ SchemaError::ShortType { kind: EventKind::TaskDone, .. })
            if error.to_string().contains("bench.task_done")),
        "the error names the stored type: {short:?}"
    );
    assert!(
        matches!(&foreign, Err(SchemaError::UnknownType { .. })),
        "{foreign:?}"
    );
    assert!(
        matches!(
            &newer,
            Err(SchemaError::Version {
                kind: EventKind::TaskDone,
                version: 2
            })
        ),
        "{newer:?}"
    );
}

#[test]
fn trust_writer_origin_and_cites_are_bench_s_to_write() {
    for (key, value) in [
        ("trust", json!({"level": "witnessed"})),
        (
            "writer",
            json!({"binary": "bench", "version": "0.1.0", "spec": "1.0.0"}),
        ),
        ("origin", json!("0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:41")),
        ("cites", json!([])),
    ] {
        let mut envelope = json!({
            "type": "bench.task_done", "data": {"task": "T1"}, "actor": "cli",
            "ts": "2026-10-11T09:30:00Z",
        });
        envelope[key] = value;

        let result = EventEnvelope::from_ev01(&envelope);

        assert!(
            matches!(&result, Err(SchemaError::SetByBench { field }) if *field == key),
            "{key}: {result:?}"
        );
    }
}

#[test]
fn a_key_outside_the_envelope_is_refused_and_null_context_reads_as_absent() {
    let flat_key = EventEnvelope::from_ev01(&json!({
        "type": "bench.task_done", "data": {"task": "T1"}, "actor": "cli",
        "session_id": "s1", "ts": "2026-10-11T09:30:00Z",
    }));
    let nulls = EventEnvelope::from_ev01(&json!({
        "type": "bench.task_done", "version": null, "data": {"task": "T1"}, "actor": "cli",
        "session": null, "worktree": null, "branch": null, "head": null, "trust": null,
        "ts": "2026-10-11T09:30:00Z",
    }));

    assert!(
        matches!(&flat_key, Err(SchemaError::Json(e)) if e.to_string().contains("session_id")),
        "{flat_key:?}"
    );
    let nulls = nulls.expect("null is absent");
    assert_eq!(
        (nulls.session_id, nulls.worktree, nulls.branch, nulls.head),
        (None, None, None, None),
        "no context"
    );
}

#[test]
fn a_key_outside_tokens_written_on_the_command_line_is_an_unknown_field() {
    let result = EventEnvelope::from_ev01(&json!({
        "type": "bench.subagent_status", "data": {"agent": "Explore",
        "tokens": {"input": 1, "output": 2, "cache_read": 3, "cache_creation": 4, "reasoning": 5}},
        "actor": "hook", "ts": "2026-10-11T09:30:00Z",
    }));

    assert!(
        matches!(&result, Err(SchemaError::UnknownField { kind: EventKind::SubagentStatus, field }) if field == "data.tokens.reasoning"),
        "{result:?}"
    );
}

#[test]
fn a_key_outside_the_type_is_an_unknown_field() {
    let result = EventEnvelope::from_ev01(&json!({
        "type": "bench.task_done", "data": {"task": "T1", "plan": "p.md"}, "actor": "cli",
        "ts": "2026-10-11T09:30:00Z",
    }));
    let typed = EventEnvelope::from_ev01(&json!({
        "type": "bench.task_done", "data": {"task": "T1", "type": "task_done"}, "actor": "cli",
        "ts": "2026-10-11T09:30:00Z",
    }));

    assert!(
        matches!(&result, Err(SchemaError::UnknownField { kind: EventKind::TaskDone, field }) if field == "data.plan"),
        "{result:?}"
    );
    assert!(
        matches!(&typed, Err(SchemaError::UnknownField { kind: EventKind::TaskDone, field }) if field == "data.type"),
        "the payload names no type: {typed:?}"
    );
}

#[test]
fn a_bad_timestamp_or_a_non_object_is_refused() {
    let bad_ts = EventEnvelope::from_ev01(&json!({
        "type": "bench.task_done", "data": {"task": "T1"}, "actor": "cli", "ts": "yesterday",
    }));
    let array = EventEnvelope::from_ev01(&json!([1, 2]));

    assert!(
        matches!(&bad_ts, Err(SchemaError::Json(e)) if e.to_string().contains("RFC 3339")),
        "{bad_ts:?}"
    );
    assert!(
        matches!(array, Err(SchemaError::NotAnObject)),
        "not an object"
    );
    assert!(
        matches!(
            Timestamp::try_from("2026-13-01T00:00:00Z".to_owned()),
            Err(TimestampError::Parse { .. })
        ),
        "month 13"
    );
}

#[test]
fn tags_are_session_worktree_and_the_subject() {
    let mut envelope = task_done("T1");
    envelope.worktree = Some("wt-0123456789ab".to_owned());

    let tags: Vec<String> = envelope
        .tags()
        .expect("valid tags")
        .iter()
        .map(ToString::to_string)
        .collect();

    assert_eq!(
        tags,
        ["session:s1", "worktree:wt-0123456789ab", "task:T1"],
        "tags"
    );
}

#[test]
fn an_empty_subject_is_a_tag_error() {
    let result = EventEnvelope::from_ev01(&json!({
        "type": "bench.task_done", "data": {"task": ""}, "actor": "cli",
        "ts": "2026-10-11T09:30:00Z",
    }));

    assert!(
        matches!(
            result,
            Err(SchemaError::Tag(TagError::Value {
                key: TagKey::Task,
                ..
            }))
        ),
        "an empty task id cannot be a tag"
    );
}

#[test]
fn now_is_a_parseable_second_precision_timestamp() {
    let now = Timestamp::now().expect("the clock is before 9999");

    assert!(now.as_str().ends_with('Z'), "UTC: {}", now.as_str());
    assert!(
        !now.as_str().contains('.'),
        "whole seconds: {}",
        now.as_str()
    );
    assert!(
        Timestamp::try_from(now.as_str().to_owned()).is_ok(),
        "now parses back"
    );
}

/// The system clock in whole Unix milliseconds, as the test reads it.
fn clock_millis() -> i64 {
    let since = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the clock is after 1970");
    i64::try_from(since.as_millis()).expect("milliseconds fit an i64")
}

#[test]
fn now_millis_is_the_clock_s_millisecond_in_utc() {
    let before = clock_millis();
    let now = Timestamp::now_millis().expect("the clock is before 9999");
    let after = clock_millis();

    let stamped = now.unix_millis();
    assert!(now.as_str().ends_with('Z'), "UTC: {}", now.as_str());
    assert_eq!(
        now.as_str()
            .split_once('.')
            .map(|(_, fraction)| fraction.len()),
        Some(4),
        "three fraction digits and the Z, always (EV-01): {}",
        now.as_str()
    );
    assert!(
        (before..=after).contains(&stamped),
        "{before} <= {stamped} <= {after}: {}",
        now.as_str()
    );
    assert!(
        Timestamp::try_from(now.as_str().to_owned()).is_ok(),
        "it parses back"
    );
}

// Moved from `bench-ledger`'s `event.rs` (Move 2 step 3, PR 3.4) unchanged: they read a `by`
// through this envelope.

#[test]
fn a_plan_approval_and_a_verdict_carry_who_gave_them_and_through_what() {
    use bench_contract::event::{Actor, By, PlanApproved, Via};

    let approval = json!({
        "type": "bench.plan_approved", "data": {"plan": "p.md", "hash": "sha256:00",
        "by": {"actor": "model", "via": "bash"}},
        "actor": "cli", "ts": "2026-10-19T09:30:00Z",
    });
    let verdict = json!({
        "type": "bench.verdict_set", "data": {"pr": 7, "verdict": "approved", "source": "human",
        "by": {"actor": "human", "via": "mod"}},
        "actor": "cli", "ts": "2026-10-19T09:30:00Z",
    });

    let approval = EventEnvelope::from_ev01(&approval).expect("a registered field");
    let verdict = EventEnvelope::from_ev01(&verdict).expect("a registered field");

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
    use bench_contract::event::By;

    let approval = json!({
        "type": "bench.plan_approved", "data": {"plan": "p.md", "hash": "sha256:00", "by": {}},
        "actor": "cli", "ts": "2026-10-19T09:30:00Z",
    });

    let empty = serde_json::from_value::<By>(json!({}));
    let in_event = EventEnvelope::from_ev01(&approval);

    assert!(
        empty.is_err() && in_event.is_err(),
        "`by: {{}}` claims a giver and names none: {empty:?}, {in_event:?}"
    );
}

#[test]
fn every_bench_type_is_declared_at_the_version_bench_writes() {
    use bench_contract::event::{
        BridgeUnavailable, GateBlocked, GrillConfirmed, MigrateContract, MigrateWaveDone,
        MigrateWaveStarted, ModeSet, OrchestrateRuled, PlanApproved, ReproRed, RoundStarted,
        StopBlocked, SubagentStatus, TYPES, TestRun, WeighinParked,
    };
    use redkiln_core::envelope::Payload;

    fn declared<P: Payload>() -> (&'static str, u32) {
        (P::TYPE, P::VERSION)
    }
    let declared = [
        declared::<TestRun>(),
        declared::<TaskDone>(),
        declared::<PlanApproved>(),
        declared::<GrillConfirmed>(),
        declared::<ReproRed>(),
        declared::<VerdictSet>(),
        declared::<RoundStarted>(),
        declared::<GateBlocked>(),
        declared::<StopBlocked>(),
        declared::<ModeSet>(),
        declared::<SubagentStatus>(),
        declared::<MigrateWaveStarted>(),
        declared::<MigrateWaveDone>(),
        declared::<MigrateContract>(),
        declared::<WeighinParked>(),
        declared::<BridgeUnavailable>(),
        declared::<OrchestrateRuled>(),
    ];

    assert_eq!(
        declared.map(|(name, _)| name),
        TYPES,
        "every type the contract declares, in its order"
    );
    assert_eq!(
        declared.map(|(_, version)| version),
        [super::WRITTEN_VERSION; 17],
        "append refuses any version but the one the contract declares"
    );
}
