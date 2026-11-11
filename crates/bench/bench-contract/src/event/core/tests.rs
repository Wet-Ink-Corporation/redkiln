use redkiln_core::envelope::{Actor, Author, Class, Envelope, Namespace, Payload, Skip, Timestamp};
use redkiln_core::envelope::{TypeName, Writer};
use redkiln_core::ids::parse_event_ref;
use redkiln_core::store::{CoreKey, Stored, TagError};
use redkiln_core::trust::Trust;

use super::{MigrateWaveDone, MigrateWaveStarted, TYPES, decode};
use crate::event::{
    AgentStatus, BenchEvent, BridgeUnavailable, EventKind, GateBlocked, GrillConfirmed,
    MigrateContract, MigrateWave, Mode, ModeSet, OrchestrateRuled, PlanApproved, ReproRed,
    RoundStarted, StopBlocked, SubagentStatus, TaskDone, TestOutcome, TestRun, TokenUsage, Verdict,
    VerdictSet, VerdictSource, WeighinParked,
};

/// An envelope of `data` written by `bench 0.0.5` as `cli` at a fixed time, asserted, with no
/// optional key.
fn envelope<P>(data: P) -> Envelope<P> {
    let author = Author {
        actor: Actor::Cli,
        trust: Trust::asserted(),
        writer: Writer::new("bench", "0.0.5").expect("a valid writer"),
    };
    Envelope::new(
        Timestamp::parse("2026-11-10T09:30:00Z").expect("a valid ts"),
        author,
        data,
    )
}

/// `envelope(data)`'s bytes, as text.
fn golden<P: Payload>(data: P) -> String {
    let bytes = envelope(data).encode().expect("encodes");
    String::from_utf8(bytes).expect("JSON is UTF-8")
}

/// `bytes` stored as an event of `event_type` at position 1, as a typed read sees it.
fn stored(event_type: &str, bytes: &[u8]) -> Stored {
    Stored {
        position: 1,
        id: parse_event_ref("0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:1").expect("an event ref"),
        recorded_at_millis: 0,
        event_type: event_type.to_owned(),
        data: bytes.to_vec(),
        tags: Vec::new(),
    }
}

/// The type, version and class a payload declares.
fn declared<P: Payload>() -> (&'static str, u32, Class) {
    (P::TYPE, P::VERSION, P::CLASS)
}

/// One payload of every type, in [`EventKind::ALL`]'s order, as the 0.x event.
fn one_of_each() -> Vec<BenchEvent> {
    let mut events = Vec::from(first_ten());
    events.extend(last_seven());
    events
}

/// [`one_of_each`]'s first ten: `test_run` to `mode_set`.
fn first_ten() -> [BenchEvent; 10] {
    let task = || "T1".to_owned();
    [
        BenchEvent::TestRun(TestRun {
            task: Some(task()),
            command: "cargo test".to_owned(),
            outcome: TestOutcome::Green,
            exit: Some(0),
            duration_ms: Some(1500),
            timeout_secs: None,
        }),
        BenchEvent::TaskDone(TaskDone { task: task() }),
        BenchEvent::PlanApproved(PlanApproved {
            plan: "docs/plans/x.md".to_owned(),
            hash: "sha256:00ff".to_owned(),
            by: None,
        }),
        BenchEvent::GrillConfirmed(GrillConfirmed {
            plan: "design-tree.json".to_owned(),
            hash: None,
            quote: Some("yes".to_owned()),
        }),
        BenchEvent::ReproRed(ReproRed {
            task: task(),
            test: "it::fails".to_owned(),
        }),
        BenchEvent::VerdictSet(VerdictSet {
            pr: 7,
            verdict: Verdict::Approved,
            source: Some(VerdictSource::Human),
            sha: None,
            patch_id: None,
            base: None,
            by: None,
        }),
        BenchEvent::RoundStarted(RoundStarted::for_pr(7, 2)),
        BenchEvent::GateBlocked(GateBlocked {
            point: "merge-gate".to_owned(),
            reason: "no verdict".to_owned(),
        }),
        BenchEvent::StopBlocked(StopBlocked {
            point: "completeness".to_owned(),
            reason: "open task".to_owned(),
        }),
        BenchEvent::ModeSet(ModeSet {
            mode: Mode::Diagnose,
            clean: true,
            plan: None,
            tree: None,
        }),
    ]
}

/// [`one_of_each`]'s last seven: `subagent_status` to `orchestrate.ruled`.
fn last_seven() -> [BenchEvent; 7] {
    [
        BenchEvent::SubagentStatus(SubagentStatus {
            agent: "Explore".to_owned(),
            agent_id: None,
            status: Some(AgentStatus::Done),
            violation: None,
            missing: Vec::new(),
            duration_ms: None,
            tokens: Some(TokenUsage::default()),
        }),
        BenchEvent::MigrateWaveStarted(MigrateWave {
            migration: "users-email".to_owned(),
            wave: 1,
        }),
        BenchEvent::MigrateWaveDone(MigrateWave {
            migration: "users-email".to_owned(),
            wave: 1,
        }),
        BenchEvent::MigrateContract(MigrateContract {
            migration: "users-email".to_owned(),
        }),
        BenchEvent::WeighinParked(WeighinParked {
            item_id: "wi-e41e02".to_owned(),
            ticket: "36".to_owned(),
            decision: None,
        }),
        BenchEvent::BridgeUnavailable(BridgeUnavailable {
            ticket: "36".to_owned(),
            reason: "no weighin".to_owned(),
        }),
        BenchEvent::OrchestrateRuled(OrchestrateRuled {
            ticket: "36".to_owned(),
            what: "keep the cache".to_owned(),
            why: "reversible".to_owned(),
            cost_if_wrong: "one revert".to_owned(),
            decision: None,
        }),
    ]
}

/// The envelope bytes of `event` under its own payload type.
fn encode(event: BenchEvent) -> Vec<u8> {
    let bytes = match event {
        BenchEvent::TestRun(p) => envelope(p).encode(),
        BenchEvent::TaskDone(p) => envelope(p).encode(),
        BenchEvent::PlanApproved(p) => envelope(p).encode(),
        BenchEvent::GrillConfirmed(p) => envelope(p).encode(),
        BenchEvent::ReproRed(p) => envelope(p).encode(),
        BenchEvent::VerdictSet(p) => envelope(p).encode(),
        BenchEvent::RoundStarted(p) => envelope(p).encode(),
        BenchEvent::GateBlocked(p) => envelope(p).encode(),
        BenchEvent::StopBlocked(p) => envelope(p).encode(),
        BenchEvent::ModeSet(p) => envelope(p).encode(),
        BenchEvent::SubagentStatus(p) => envelope(p).encode(),
        BenchEvent::MigrateWaveStarted(p) => envelope(MigrateWaveStarted(p)).encode(),
        BenchEvent::MigrateWaveDone(p) => envelope(MigrateWaveDone(p)).encode(),
        BenchEvent::MigrateContract(p) => envelope(p).encode(),
        BenchEvent::WeighinParked(p) => envelope(p).encode(),
        BenchEvent::BridgeUnavailable(p) => envelope(p).encode(),
        BenchEvent::OrchestrateRuled(p) => envelope(p).encode(),
    };
    bytes.expect("encodes")
}

#[test]
fn every_kind_is_stored_as_bench_and_its_short_name_version_1_durable() {
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

    for (kind, (stored, version, class)) in EventKind::ALL.into_iter().zip(declared) {
        let name = TypeName::parse(stored).expect("a valid EV-03 type name");
        assert_eq!(
            (stored, name.namespace(), name.name(), version, class),
            (
                format!("bench.{kind}").as_str(),
                Namespace::Bench,
                kind.as_str(),
                1,
                Class::Durable
            ),
            "{kind}'s payload"
        );
        assert_eq!(kind.stored(), stored, "{kind}'s stored name");
        assert!(stored.len() <= 64, "{stored} fits EV-03's 64 bytes");
    }
}

#[test]
fn types_lists_every_stored_name_in_registry_order() {
    assert_eq!(TYPES, EventKind::ALL.map(EventKind::stored));
}

/// The envelope bytes before `data` of a `bench.<kind>` event from [`envelope`].
fn head(kind: &str) -> String {
    format!(
        r#"{{"type":"bench.{kind}","version":1,"ts":"2026-11-10T09:30:00Z","actor":"cli","trust":{{"level":"asserted"}},"writer":{{"binary":"bench","version":"0.0.5","spec":"1.0.0"}},"data":"#
    )
}

/// Each short name and the `data` bytes of [`one_of_each`]'s event of that kind, in order.
fn data_goldens() -> [(&'static str, &'static str); 17] {
    [
        (
            "test_run",
            r#"{"task":"T1","command":"cargo test","outcome":"green","exit":0,"duration_ms":1500,"timeout_secs":null}"#,
        ),
        ("task_done", r#"{"task":"T1"}"#),
        (
            "plan_approved",
            r#"{"plan":"docs/plans/x.md","hash":"sha256:00ff"}"#,
        ),
        (
            "grill_confirmed",
            r#"{"plan":"design-tree.json","hash":null,"quote":"yes"}"#,
        ),
        ("repro_red", r#"{"task":"T1","test":"it::fails"}"#),
        (
            "verdict_set",
            r#"{"pr":7,"verdict":"approved","source":"human","sha":null,"patch_id":null,"base":null}"#,
        ),
        ("round_started", r#"{"pr":7,"round":2}"#),
        (
            "gate_blocked",
            r#"{"point":"merge-gate","reason":"no verdict"}"#,
        ),
        (
            "stop_blocked",
            r#"{"point":"completeness","reason":"open task"}"#,
        ),
        (
            "mode_set",
            r#"{"mode":"diagnose","clean":true,"plan":null,"tree":null}"#,
        ),
        (
            "subagent_status",
            r#"{"agent":"Explore","agent_id":null,"status":"done","violation":null,"missing":[],"duration_ms":null,"tokens":{"input":0,"output":0,"cache_read":0,"cache_creation":0}}"#,
        ),
        (
            "migrate.wave.started",
            r#"{"migration":"users-email","wave":1}"#,
        ),
        (
            "migrate.wave.done",
            r#"{"migration":"users-email","wave":1}"#,
        ),
        ("migrate.contract", r#"{"migration":"users-email"}"#),
        ("weighin_parked", r#"{"item_id":"wi-e41e02","ticket":"36"}"#),
        (
            "bridge.unavailable",
            r#"{"ticket":"36","reason":"no weighin"}"#,
        ),
        (
            "orchestrate.ruled",
            r#"{"ticket":"36","what":"keep the cache","why":"reversible","cost_if_wrong":"one revert"}"#,
        ),
    ]
}

#[test]
fn each_type_writes_its_version_1_golden() {
    let goldens = data_goldens();
    assert_eq!(goldens.len(), one_of_each().len(), "one golden per type");

    for ((kind, data), event) in goldens.into_iter().zip(one_of_each()) {
        assert_eq!(event.kind().as_str(), kind, "the golden's order");
        let bytes = encode(event);
        assert_eq!(
            String::from_utf8(bytes).expect("JSON is UTF-8"),
            format!("{}{data}}}", head(kind)),
            "{kind}"
        );
    }
}

#[test]
fn the_typed_envelope_writes_the_same_bytes_as_the_dispatch() {
    assert_eq!(
        golden(TaskDone {
            task: "T1".to_owned()
        }),
        format!(r#"{}{{"task":"T1"}}}}"#, head("task_done")),
        "the typed envelope is what the dispatch encodes"
    );
}

#[test]
fn each_kind_tags_its_subject_under_cores_key_or_benchs() {
    let tags: Vec<Option<String>> = one_of_each()
        .iter()
        .map(|event| {
            event
                .subject_tag()
                .expect("a valid tag")
                .map(|tag| tag.as_str().to_owned())
        })
        .collect();

    let expected = [
        Some("task:T1"),
        Some("task:T1"),
        Some("bench.plan:docs/plans/x.md"),
        Some("bench.plan:design-tree.json"),
        Some("task:T1"),
        Some("pr:7"),
        Some("pr:7"),
        Some("bench.point:merge-gate"),
        Some("bench.point:completeness"),
        None,
        Some("bench.agent:Explore"),
        Some("bench.migration:users-email"),
        Some("bench.migration:users-email"),
        Some("bench.migration:users-email"),
        Some("task:36"),
        Some("task:36"),
        Some("task:36"),
    ]
    .map(|tag| tag.map(str::to_owned));
    assert_eq!(
        tags, expected,
        "mode_set's subject is the envelope's worktree"
    );
}

#[test]
fn a_ticket_round_and_a_task_less_test_run_have_no_subject_tag() {
    let round = BenchEvent::RoundStarted(RoundStarted::for_ticket("36", 1));
    let run = BenchEvent::TestRun(TestRun {
        task: None,
        command: "cargo test".to_owned(),
        outcome: TestOutcome::Red,
        exit: Some(101),
        duration_ms: None,
        timeout_secs: None,
    });

    assert_eq!(
        (
            round.subject_tag().expect("no tag"),
            run.subject_tag().expect("no tag")
        ),
        (None, None)
    );
}

#[test]
fn pull_request_0_is_not_a_pr_tag() {
    let verdict = BenchEvent::VerdictSet(VerdictSet {
        pr: 0,
        verdict: Verdict::Approved,
        source: None,
        sha: None,
        patch_id: None,
        base: None,
        by: None,
    });

    assert_eq!(
        verdict.subject_tag(),
        Err(TagError::Value { key: CoreKey::Pr }),
        "Core's pr key holds a number from 1"
    );
}

#[test]
fn every_stored_kind_decodes_to_its_event() {
    for event in one_of_each() {
        let kind = event.kind();
        let bytes = encode(event.clone());

        let decoded = decode(&stored(kind.stored(), &bytes)).expect("usable");

        assert_eq!(decoded.data, event, "{kind}");
        assert_eq!(
            (decoded.actor, decoded.writer.binary()),
            (Actor::Cli, "bench"),
            "{kind}'s envelope"
        );
    }
}

#[test]
fn another_namespaces_event_and_an_unknown_bench_type_are_foreign() {
    let raised = br#"{"type":"weighin.item_raised","version":1,"ts":"2026-11-10T09:30:00Z","actor":"cli","trust":{"level":"asserted"},"writer":{"binary":"weighin","version":"2.0.0","spec":"1.0.0"},"data":{"item":"wi-3fa9c01b2e"}}"#;
    let later = br#"{"type":"bench.task_started","version":1,"ts":"2026-11-10T09:30:00Z","actor":"cli","trust":{"level":"asserted"},"writer":{"binary":"bench","version":"0.0.5","spec":"1.0.0"},"data":{"task":"T1"}}"#;

    assert!(
        matches!(
            decode(&stored("weighin.item_raised", raised)),
            Err(Skip::Foreign)
        ),
        "EV-08: skipped without error"
    );
    assert!(
        matches!(
            decode(&stored("bench.task_started", later)),
            Err(Skip::Foreign)
        ),
        "a Bench type this build does not declare is not one it reads"
    );
}

#[test]
fn a_newer_version_and_undecodable_data_are_unusable() {
    let newer = br#"{"type":"bench.task_done","version":2,"ts":"2026-11-10T09:30:00Z","actor":"cli","trust":{"level":"asserted"},"writer":{"binary":"bench","version":"0.0.5","spec":"1.0.0"},"data":{"task":"T1"}}"#;
    let garbled = br#"{"type":"bench.verdict_set","version":1,"ts":"2026-11-10T09:30:00Z","actor":"cli","trust":{"level":"asserted"},"writer":{"binary":"bench","version":"0.0.5","spec":"1.0.0"},"data":{"pr":"seven"}}"#;

    assert!(
        matches!(
            decode(&stored("bench.task_done", newer)),
            Err(Skip::Unusable(_))
        ),
        "EV-10: version 2 is newer than this build reads"
    );
    assert!(
        matches!(
            decode(&stored("bench.verdict_set", garbled)),
            Err(Skip::Unusable(_))
        ),
        "EV-10: the payload does not decode"
    );
}
