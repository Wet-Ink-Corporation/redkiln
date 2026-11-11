// Moved from `bench-ledger`'s `event.rs` (Move 2 step 3, PR 3.2) with their bodies unchanged.
// The imports name the types by their `bench-ledger` names: `LedgerEvent` is `BenchEvent`, and
// `TagKey` was the subject's key there, `SubjectKey` here.
use super::{
    AgentStatus, BenchEvent as LedgerEvent, BridgeUnavailable, EventKind, GrillConfirmed,
    MigrateContract, MigrateWave, Mode, ModeSet, OrchestrateRuled, REGISTRY_VERSION, RoundStarted,
    SubagentStatus, SubjectKey as TagKey, TestOutcome, TestRun, TokenUsage, UnknownEventKind,
    Verdict, VerdictSet, VerdictSource, WeighinParked,
};
use serde_json::json;

#[test]
fn grill_confirmed_carries_the_tree_hash_and_the_humans_words() {
    let value = json!({
        "type": "grill_confirmed",
        "plan": "design-tree.json",
        "hash": "sha256:00ff",
        "quote": "yes, that is what I want",
    });

    let event: LedgerEvent = serde_json::from_value(value.clone()).expect("registered");

    assert_eq!(
        event,
        LedgerEvent::GrillConfirmed(GrillConfirmed {
            plan: "design-tree.json".to_owned(),
            hash: Some("sha256:00ff".to_owned()),
            quote: Some("yes, that is what I want".to_owned()),
        }),
        "hash and quote are read"
    );
    assert_eq!(
        serde_json::to_value(&event).expect("serialises"),
        value,
        "and written back under the same keys"
    );
}

#[test]
fn a_grill_confirmed_written_before_hash_and_quote_still_reads() {
    let event: LedgerEvent =
        serde_json::from_value(json!({"type": "grill_confirmed", "plan": "p.md"}))
            .expect("registry v1 shape");

    assert_eq!(
        event,
        LedgerEvent::GrillConfirmed(GrillConfirmed {
            plan: "p.md".to_owned(),
            hash: None,
            quote: None,
        }),
        "the added fields default to none"
    );
}

#[test]
fn test_run_carries_the_exit_code_duration_and_timeout() {
    let value = json!({
        "type": "test_run",
        "task": "T1",
        "command": "cargo test",
        "outcome": "red",
        "exit": null,
        "duration_ms": 1500,
        "timeout_secs": 1,
    });

    let event: LedgerEvent = serde_json::from_value(value.clone()).expect("registered");

    assert_eq!(
        event,
        LedgerEvent::TestRun(TestRun {
            task: Some("T1".to_owned()),
            command: "cargo test".to_owned(),
            outcome: TestOutcome::Red,
            exit: None,
            duration_ms: Some(1500),
            timeout_secs: Some(1),
        }),
        "exit, duration and timeout are read"
    );
    assert_eq!(
        serde_json::to_value(&event).expect("serialises"),
        value,
        "and written back under the same keys"
    );
}

#[test]
fn a_test_run_written_before_exit_and_duration_still_reads() {
    let event: LedgerEvent = serde_json::from_value(
        json!({"type": "test_run", "command": "cargo test", "outcome": "green"}),
    )
    .expect("registry v2 shape");

    assert_eq!(
        event,
        LedgerEvent::TestRun(TestRun {
            task: None,
            command: "cargo test".to_owned(),
            outcome: TestOutcome::Green,
            exit: None,
            duration_ms: None,
            timeout_secs: None,
        }),
        "the added fields default to none"
    );
}

#[test]
fn mode_set_carries_the_mode_the_clean_flag_and_the_define_artifacts() {
    let value = json!({
        "type": "mode_set",
        "mode": "build",
        "clean": true,
        "plan": "docs/plans/x.md",
        "tree": "design-tree.json",
    });

    let event: LedgerEvent = serde_json::from_value(value.clone()).expect("registered");

    assert_eq!(
        event,
        LedgerEvent::ModeSet(ModeSet {
            mode: Mode::Build,
            clean: true,
            plan: Some("docs/plans/x.md".to_owned()),
            tree: Some("design-tree.json".to_owned()),
        }),
        "mode, clean, plan and tree are read"
    );
    assert_eq!(
        serde_json::to_value(&event).expect("serialises"),
        value,
        "and written back under the same keys"
    );
    assert_eq!(event.kind().as_str(), "mode_set", "stored as mode_set");
    assert_eq!(event.subject_value(), None, "no subject in the payload");
    assert_eq!(
        EventKind::ModeSet.subject(),
        TagKey::Worktree,
        "selected by the envelope's worktree tag"
    );
}

#[test]
fn a_mode_set_without_plan_or_tree_reads_as_none() {
    let event: LedgerEvent =
        serde_json::from_value(json!({"type": "mode_set", "mode": "diagnose", "clean": false}))
            .expect("plan and tree are optional");

    assert_eq!(
        event,
        LedgerEvent::ModeSet(ModeSet {
            mode: Mode::Diagnose,
            clean: false,
            plan: None,
            tree: None,
        }),
        "the optional fields default to none"
    );
}

#[test]
fn subagent_status_carries_the_status_missing_sections_timing_and_tokens() {
    let value = json!({
        "type": "subagent_status",
        "agent": "temper:rust-implementer",
        "agent_id": "a1",
        "status": "done_with_concerns",
        "violation": "missing sections: Review",
        "missing": ["Review"],
        "duration_ms": 61_000,
        "tokens": {"input": 10, "output": 20, "cache_read": 30, "cache_creation": 40},
    });

    let event: LedgerEvent = serde_json::from_value(value.clone()).expect("registered");

    assert_eq!(
        event,
        LedgerEvent::SubagentStatus(SubagentStatus {
            agent: "temper:rust-implementer".to_owned(),
            agent_id: Some("a1".to_owned()),
            status: Some(AgentStatus::DoneWithConcerns),
            violation: Some("missing sections: Review".to_owned()),
            missing: vec!["Review".to_owned()],
            duration_ms: Some(61_000),
            tokens: Some(TokenUsage {
                input: 10,
                output: 20,
                cache_read: 30,
                cache_creation: 40,
            }),
        }),
        "every field is read"
    );
    assert_eq!(
        serde_json::to_value(&event).expect("serialises"),
        value,
        "and written back under the same keys"
    );
    assert_eq!(
        (EventKind::SubagentStatus.subject(), event.subject_value()),
        (TagKey::Agent, Some("temper:rust-implementer".into())),
        "its subject is the agent type"
    );
}

#[test]
fn a_subagent_status_with_only_the_agent_reads_as_a_bare_violation_record() {
    let event: LedgerEvent =
        serde_json::from_value(json!({"type": "subagent_status", "agent": "Explore"}))
            .expect("only agent is required");

    assert_eq!(
        event,
        LedgerEvent::SubagentStatus(SubagentStatus {
            agent: "Explore".to_owned(),
            agent_id: None,
            status: None,
            violation: None,
            missing: Vec::new(),
            duration_ms: None,
            tokens: None,
        }),
        "the optional fields default to none"
    );
}

#[test]
fn every_agent_status_has_its_stored_name() {
    let names: Vec<String> = [
        AgentStatus::Done,
        AgentStatus::DoneWithConcerns,
        AgentStatus::NeedsContext,
        AgentStatus::Blocked,
        AgentStatus::Wip,
    ]
    .iter()
    .map(|status| {
        serde_json::to_value(status)
            .expect("serialises")
            .to_string()
    })
    .collect();

    assert_eq!(
        names,
        [
            "\"done\"",
            "\"done_with_concerns\"",
            "\"needs_context\"",
            "\"blocked\"",
            "\"wip\""
        ],
        "snake case, as the ledger's other enums"
    );
}

#[test]
fn verdict_set_carries_the_level_source_commit_patch_id_and_base() {
    let value = json!({
        "type": "verdict_set",
        "pr": 33,
        "verdict": "tests_verified",
        "source": "review_plugin",
        "sha": "a21c5e7b17ac7075034918873814cd7950807243",
        "patch_id": "0f1e2d3c4b5a69788796a5b4c3d2e1f00f1e2d3c",
        "base": "origin/main",
    });

    let event: LedgerEvent = serde_json::from_value(value.clone()).expect("registered");

    assert_eq!(
        event,
        LedgerEvent::VerdictSet(VerdictSet {
            pr: 33,
            verdict: Verdict::TestsVerified,
            source: Some(VerdictSource::ReviewPlugin),
            sha: Some("a21c5e7b17ac7075034918873814cd7950807243".to_owned()),
            patch_id: Some("0f1e2d3c4b5a69788796a5b4c3d2e1f00f1e2d3c".to_owned()),
            base: Some("origin/main".to_owned()),
            by: None,
        }),
        "every field is read"
    );
    assert_eq!(
        serde_json::to_value(&event).expect("serialises"),
        value,
        "and written back under the same keys"
    );
}

#[test]
fn every_verdict_and_source_has_its_stored_name() {
    let verdicts = [
        Verdict::TestsVerified,
        Verdict::Reviewed,
        Verdict::Approved,
        Verdict::ChangesRequested,
        Verdict::NoiseOnly,
    ]
    .map(|verdict| serde_json::to_value(verdict).expect("serialises"));
    let sources = [
        VerdictSource::Local,
        VerdictSource::ReviewPlugin,
        VerdictSource::Human,
    ]
    .map(|source| serde_json::to_value(source).expect("serialises"));

    assert_eq!(
        (verdicts.to_vec(), sources.to_vec()),
        (
            vec![
                json!("tests_verified"),
                json!("reviewed"),
                json!("approved"),
                json!("changes_requested"),
                json!("noise_only"),
            ],
            vec![json!("local"), json!("review_plugin"), json!("human")],
        ),
        "snake case, as the ledger's other enums"
    );
}

#[test]
fn the_registry_is_version_10() {
    assert_eq!(
        REGISTRY_VERSION, 10,
        "the migrate.wave.started, migrate.wave.done and migrate.contract types were added \
         in version 7, round_started's ticket (with pr optional) in version 8, \
         weighin_parked, bridge.unavailable and orchestrate.ruled in version 9, \
         subagent_status's wip in version 10"
    );
}

#[test]
fn round_started_carries_a_ticket_without_a_pull_request() {
    let value = json!({"type": "round_started", "ticket": "36", "round": 2});

    let event: LedgerEvent = serde_json::from_value(value.clone()).expect("registered");

    assert_eq!(
        event,
        LedgerEvent::RoundStarted(RoundStarted::for_ticket("36", 2)),
        "ticket and round are read"
    );
    assert_eq!(
        serde_json::to_value(&event).expect("serialises"),
        value,
        "and written back under the same keys, with no pr key"
    );
    assert_eq!(event.subject_value(), None, "no pull request, no pr: tag");
}

#[test]
fn a_round_started_written_before_ticket_still_reads_with_its_pull_request() {
    let v6 = json!({"type": "round_started", "pr": 7, "round": 1});

    let event: LedgerEvent = serde_json::from_value(v6.clone()).expect("registry v6 shape");

    assert_eq!(
        event.subject_value().as_deref(),
        Some("7"),
        "the pr: tag is kept"
    );
    assert_eq!(
        event,
        LedgerEvent::RoundStarted(RoundStarted::for_pr(7, 1)),
        "the ticket defaults to none"
    );
    assert_eq!(
        serde_json::to_value(&event).expect("serialises"),
        v6,
        "a pull request round is still written in v6's shape, with no ticket key"
    );
}

#[test]
fn a_round_started_naming_neither_a_pull_request_nor_a_ticket_is_refused() {
    for value in [
        json!({"type": "round_started", "round": 1}),
        json!({"type": "round_started", "pr": null, "ticket": null, "round": 1}),
        json!({"type": "round_started", "ticket": "", "round": 1}),
    ] {
        assert!(
            serde_json::from_value::<LedgerEvent>(value.clone()).is_err(),
            "a round of nothing: {value}"
        );
    }
}

#[test]
fn migrate_events_are_stored_under_their_dotted_names_with_the_migration_as_subject() {
    let cases = [
        (
            json!({"type": "migrate.wave.started", "migration": "users-email", "wave": 1}),
            LedgerEvent::MigrateWaveStarted(MigrateWave {
                migration: "users-email".to_owned(),
                wave: 1,
            }),
        ),
        (
            json!({"type": "migrate.wave.done", "migration": "users-email", "wave": 2}),
            LedgerEvent::MigrateWaveDone(MigrateWave {
                migration: "users-email".to_owned(),
                wave: 2,
            }),
        ),
        (
            json!({"type": "migrate.contract", "migration": "users-email"}),
            LedgerEvent::MigrateContract(MigrateContract {
                migration: "users-email".to_owned(),
            }),
        ),
    ];

    for (value, expected) in cases {
        let event: LedgerEvent = serde_json::from_value(value.clone()).expect("registered");
        assert_eq!(event, expected, "{value} reads");
        assert_eq!(
            serde_json::to_value(&event).expect("serialises"),
            value,
            "and is written back under the same keys"
        );
        assert_eq!(
            Some(event.kind().as_str()),
            value["type"].as_str(),
            "its kind is the stored name"
        );
        assert_eq!(
            (event.kind().subject(), event.subject_value()),
            (TagKey::Migration, Some("users-email".into())),
            "its subject is the migration"
        );
    }
}

#[test]
fn orchestrate_events_carry_their_ticket_as_the_task_subject() {
    let cases = [
        (
            json!({"type": "weighin_parked", "item_id": "wi-e41e02", "ticket": "36",
                   "decision": "d-0123"}),
            LedgerEvent::WeighinParked(WeighinParked {
                item_id: "wi-e41e02".to_owned(),
                ticket: "36".to_owned(),
                decision: Some("d-0123".to_owned()),
            }),
        ),
        (
            json!({"type": "bridge.unavailable", "ticket": "36",
                   "reason": "no weighin: WEIGHIN_BIN is unset and none is on PATH"}),
            LedgerEvent::BridgeUnavailable(BridgeUnavailable {
                ticket: "36".to_owned(),
                reason: "no weighin: WEIGHIN_BIN is unset and none is on PATH".to_owned(),
            }),
        ),
        (
            json!({"type": "orchestrate.ruled", "ticket": "36", "what": "keep the cache",
                   "why": "reversible", "cost_if_wrong": "one revert"}),
            LedgerEvent::OrchestrateRuled(OrchestrateRuled {
                ticket: "36".to_owned(),
                what: "keep the cache".to_owned(),
                why: "reversible".to_owned(),
                cost_if_wrong: "one revert".to_owned(),
                decision: None,
            }),
        ),
    ];

    for (value, expected) in cases {
        let event: LedgerEvent = serde_json::from_value(value.clone()).expect("registered");
        assert_eq!(event, expected, "{value} reads");
        assert_eq!(
            serde_json::to_value(&event).expect("serialises"),
            value,
            "and is written back under the same keys, an absent decision left out"
        );
        assert_eq!(
            Some(event.kind().as_str()),
            value["type"].as_str(),
            "its kind is the stored name"
        );
        assert_eq!(
            (event.kind().subject(), event.subject_value()),
            (TagKey::Task, Some("36".into())),
            "its subject is the ticket, as a task"
        );
    }
}

#[test]
fn every_kind_parses_from_its_stored_name() {
    for kind in EventKind::ALL {
        assert_eq!(kind.as_str().parse::<EventKind>(), Ok(kind), "{kind}");
    }
}

#[test]
fn every_kind_is_found_by_its_stored_name_and_no_other_name() {
    for kind in EventKind::ALL {
        assert_eq!(
            (
                EventKind::from_stored(kind.stored()),
                EventKind::from_stored(kind.as_str())
            ),
            (Some(kind), None),
            "{kind}: the stored name, never the short one"
        );
    }
    assert_eq!(
        EventKind::from_stored("weighin.decided"),
        None,
        "another plugin's type"
    );
}

#[test]
fn an_unregistered_name_is_unknown() {
    assert_eq!(
        "task_started".parse::<EventKind>(),
        Err(UnknownEventKind {
            name: "task_started".to_owned()
        }),
        "only registered names parse"
    );
}
