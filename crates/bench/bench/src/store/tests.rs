// `every_registered_kind_is_a_valid_store_event_type`, `an_appended_event_reads_back_at_position_1`,
// `query_filters_by_type_tag_and_session`, `tail_returns_the_last_n_in_position_order`,
// `a_second_approval_of_the_same_plan_conflicts` and
// `a_condition_after_a_position_ignores_what_the_writer_already_saw` moved from `bench-ledger`'s
// `store/tests.rs` (Move 2 step 3, PR 3.4) with their assertions; their envelopes' actor `test`,
// which Core refuses, is `cli` (D9 iv), and the store file is a `ledger.sqlite3` in the temporary
// directory. The rest are new (plan test list 18 to 21).

use std::path::{Path, PathBuf};

use bench_contract::event::{PlanApproved, TaskDone, TestOutcome, TestRun, Verdict, VerdictSet};
use redkiln_core::store::{CoreKey, NewEvent, Store as CoreStore, Tag, Wait};

use super::{
    Appended, Condition, EventEnvelope, EventKind, Filter, LedgerEvent, LedgerTag, Recorded, Store,
    TYPES, TagKey, Timestamp,
};

fn ts() -> Timestamp {
    Timestamp::try_from("2026-10-11T09:30:00Z".to_owned()).expect("a valid timestamp")
}

fn in_session(event: LedgerEvent, session: &str) -> EventEnvelope {
    let mut envelope = EventEnvelope::new(event, "cli", ts());
    envelope.session_id = Some(session.to_owned());
    envelope
}

fn task_done(task: &str, session: &str) -> EventEnvelope {
    in_session(
        LedgerEvent::TaskDone(TaskDone {
            task: task.to_owned(),
        }),
        session,
    )
}

fn test_run(task: &str, session: &str) -> EventEnvelope {
    in_session(
        LedgerEvent::TestRun(TestRun {
            task: Some(task.to_owned()),
            command: "cargo test".to_owned(),
            outcome: TestOutcome::Green,
            exit: Some(0),
            duration_ms: Some(10),
            timeout_secs: None,
        }),
        session,
    )
}

fn plan_approved(plan: &str, hash: &str) -> EventEnvelope {
    in_session(
        LedgerEvent::PlanApproved(PlanApproved {
            plan: plan.to_owned(),
            hash: hash.to_owned(),
            by: None,
        }),
        "s1",
    )
}

fn verdict_set(pr: u64) -> EventEnvelope {
    in_session(
        LedgerEvent::VerdictSet(VerdictSet {
            pr,
            verdict: Verdict::Approved,
            source: Some(bench_contract::event::VerdictSource::Human),
            sha: None,
            patch_id: None,
            base: None,
            by: None,
        }),
        "s1",
    )
}

fn ledger_path(dir: &Path) -> PathBuf {
    dir.join("ledger.sqlite3")
}

fn positions(events: &[Recorded]) -> Vec<u64> {
    events.iter().map(|recorded| recorded.position).collect()
}

fn tag(text: &str) -> LedgerTag {
    text.parse().expect("a vocabulary tag")
}

/// Appends `event_type` with the envelope bytes `data` and `tags` straight through Core, as
/// another plugin (or a later Bench) would.
fn append_raw(path: &Path, event_type: &str, data: &str, tags: &[Tag]) -> u64 {
    let store = CoreStore::open(path, Wait::Command).expect("Core opens it");
    let event = tags.iter().fold(
        NewEvent::new(event_type, data.as_bytes().to_vec()).expect("a valid type"),
        |event, tag| event.with_tag(tag.clone()),
    );
    match store.append(vec![event], None).expect("appends") {
        redkiln_core::store::Appended::At(position) => position,
        redkiln_core::store::Appended::Conflict => panic!("no condition, no conflict"),
    }
}

/// An envelope of `event_type` at `version` with `data`, written by `binary`.
fn envelope_text(event_type: &str, version: u32, binary: &str, data: &str) -> String {
    format!(
        r#"{{"type":"{event_type}","version":{version},"ts":"2026-11-10T09:30:00Z","actor":"cli","trust":{{"level":"asserted"}},"writer":{{"binary":"{binary}","version":"1.0.0","spec":"1.0.0"}},"data":{data}}}"#
    )
}

#[test]
fn every_registered_kind_is_a_valid_store_event_type() {
    for kind in EventKind::ALL {
        assert!(
            NewEvent::new(kind.stored(), b"{}".to_vec()).is_ok(),
            "{kind} is a valid store event type"
        );
    }
    assert_eq!(TYPES.len(), EventKind::ALL.len(), "one stored name each");
}

#[test]
#[cfg_attr(miri, ignore = "opens SQLite through FFI")]
fn an_appended_event_reads_back_at_position_1() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = Store::open(&ledger_path(dir.path())).expect("opens");
    let event = task_done("T1", "s1");

    let position = store.append(&event).expect("appends");
    let events = store.query(&Filter::default()).expect("reads");

    assert_eq!(position, 1, "the first event is at 1");
    assert_eq!(events, [Recorded { position: 1, event }], "reads back");
}

#[test]
#[cfg_attr(miri, ignore = "opens SQLite through FFI")]
fn query_filters_by_type_tag_and_session() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = Store::open(&ledger_path(dir.path())).expect("opens");
    for event in [
        task_done("T1", "s1"),
        test_run("T1", "s2"),
        task_done("T2", "s2"),
    ] {
        store.append(&event).expect("appends");
    }

    let by_type = Filter {
        types: vec![EventKind::TaskDone],
        ..Filter::default()
    };
    let by_task = Filter {
        tags: vec![tag("task:T1")],
        ..Filter::default()
    };
    let by_session_and_type = Filter {
        types: vec![EventKind::TaskDone, EventKind::TestRun],
        tags: vec![LedgerTag::new(TagKey::Session, "s2").expect("valid")],
    };

    assert_eq!(
        positions(&store.query(&by_type).expect("reads")),
        [1, 3],
        "type"
    );
    assert_eq!(
        positions(&store.query(&by_task).expect("reads")),
        [1, 2],
        "tag"
    );
    assert_eq!(
        positions(&store.query(&by_session_and_type).expect("reads")),
        [2, 3],
        "types are any-of, tags all-of"
    );
}

#[test]
#[cfg_attr(miri, ignore = "opens SQLite through FFI")]
fn tail_returns_the_last_n_in_position_order() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = Store::open(&ledger_path(dir.path())).expect("opens");
    for task in ["T1", "T2", "T3", "T4"] {
        store.append(&task_done(task, "s1")).expect("appends");
    }

    let last_two = store.tail(&Filter::default(), 2).expect("reads");
    let more_than_all = store.tail(&Filter::default(), 10).expect("reads");

    assert_eq!(positions(&last_two), [3, 4], "oldest first");
    assert_eq!(positions(&more_than_all), [1, 2, 3, 4], "n past the end");
}

#[test]
#[cfg_attr(miri, ignore = "opens SQLite through FFI")]
fn a_second_approval_of_the_same_plan_conflicts() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = Store::open(&ledger_path(dir.path())).expect("opens");
    let first_approval_of = |plan: &str| {
        Condition::none_matching(Filter {
            types: vec![EventKind::PlanApproved],
            tags: vec![LedgerTag::new(TagKey::Plan, plan).expect("valid")],
        })
    };

    let first = store.append_if(&plan_approved("p.md", "h1"), &first_approval_of("p.md"));
    let second = store.append_if(&plan_approved("p.md", "h2"), &first_approval_of("p.md"));
    let other = store.append_if(&plan_approved("q.md", "h1"), &first_approval_of("q.md"));

    assert_eq!(
        first.expect("appends"),
        Appended::At(1),
        "first writer wins"
    );
    assert_eq!(
        second.expect("no error"),
        Appended::Conflict,
        "second loses"
    );
    assert_eq!(
        other.expect("appends"),
        Appended::At(2),
        "another plan is free"
    );
    assert_eq!(
        store.query(&Filter::default()).expect("reads").len(),
        2,
        "the conflicting event was not written"
    );
}

#[test]
#[cfg_attr(miri, ignore = "opens SQLite through FFI")]
fn a_condition_after_a_position_ignores_what_the_writer_already_saw() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = Store::open(&ledger_path(dir.path())).expect("opens");
    store.append(&task_done("T1", "s1")).expect("appends");
    let unchanged_since_1 = Condition::none_matching(Filter {
        tags: vec![tag("task:T1")],
        ..Filter::default()
    })
    .after(1);

    let first = store.append_if(&test_run("T1", "s1"), &unchanged_since_1);
    let stale = store.append_if(&test_run("T1", "s2"), &unchanged_since_1);

    assert_eq!(first.expect("appends"), Appended::At(2), "nothing after 1");
    assert_eq!(stale.expect("no error"), Appended::Conflict, "2 is after 1");
}

#[test]
#[cfg_attr(miri, ignore = "opens SQLite through FFI")]
fn another_plugins_events_neither_conflict_nor_read_and_keep_positions() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = ledger_path(dir.path());
    let store = Store::open(&path).expect("opens");
    store.append(&verdict_set(7)).expect("appends");
    let after_read = Condition::none_matching(Filter {
        tags: vec![tag("pr:7")],
        ..Filter::default()
    })
    .after(1);
    let pr7 = Tag::core(CoreKey::Pr, "7").expect("a pr tag");
    append_raw(
        &path,
        "weighin.item_raised",
        &envelope_text(
            "weighin.item_raised",
            1,
            "weighin",
            r#"{"item":"wi-3fa9c01b2e"}"#,
        ),
        &[],
    );
    append_raw(
        &path,
        "review.verdict",
        &envelope_text("review.verdict", 1, "review", r#"{"pr":7}"#),
        &[pr7],
    );

    let appended = store
        .append_if(&verdict_set(7), &after_read)
        .expect("no error");
    let read = store.query_read(&Filter::default()).expect("reads");
    let tail = store.tail(&Filter::default(), 2).expect("reads");

    assert_eq!(
        appended,
        Appended::At(4),
        "EV-08: only Bench's events count"
    );
    assert_eq!(
        (positions(&read.events), read.unusable),
        (vec![1, 4], 0),
        "foreign events are neither read nor counted"
    );
    assert_eq!(positions(&tail), [1, 4], "nor in a tail");
}

#[test]
#[cfg_attr(miri, ignore = "opens SQLite through FFI")]
fn a_later_version_and_undecodable_data_are_skipped_and_counted() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = ledger_path(dir.path());
    let store = Store::open(&path).expect("opens");
    store.append(&task_done("T1", "s1")).expect("appends");
    append_raw(
        &path,
        "bench.verdict_set",
        &envelope_text("bench.verdict_set", 2, "bench", r#"{"pr":7}"#),
        &[],
    );
    append_raw(
        &path,
        "bench.verdict_set",
        &envelope_text("bench.verdict_set", 1, "bench", r#"{"pr":"seven"}"#),
        &[],
    );

    let read = store.query_read(&Filter::default()).expect("reads");
    let newest = store.tail_read(&Filter::default(), 1).expect("reads");

    assert_eq!(
        (positions(&read.events), read.unusable),
        (vec![1], 2),
        "EV-10: skipped, not an error"
    );
    assert_eq!(
        (positions(&newest.events), newest.unusable),
        (Vec::new(), 1),
        "a tail of one whose newest is unusable returns nothing (Invariant 4)"
    );
}

#[test]
#[cfg_attr(miri, ignore = "opens SQLite through FFI")]
fn unusable_events_are_counted_by_their_stored_type_and_foreign_ones_not_at_all() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = ledger_path(dir.path());
    let store = Store::open(&path).expect("opens");
    store.append(&task_done("T1", "s1")).expect("appends");
    for data in [r#"{"pr":7}"#, r#"{"pr":8}"#] {
        let text = envelope_text("bench.verdict_set", 2, "bench", data);
        append_raw(&path, "bench.verdict_set", &text, &[]);
    }
    let undecodable = envelope_text("bench.task_done", 1, "bench", r#"{"task":7}"#);
    append_raw(&path, "bench.task_done", &undecodable, &[]);
    let foreign = envelope_text("review.verdict", 2, "review", r#"{"pr":7}"#);
    append_raw(&path, "review.verdict", &foreign, &[]);

    let by_type = store.unusable_by_type().expect("reads");

    assert_eq!(
        by_type.into_iter().collect::<Vec<_>>(),
        [
            ("bench.task_done".to_owned(), 1),
            ("bench.verdict_set".to_owned(), 2)
        ],
        "EV-10, by type; another plugin's event is not Bench's to count (EV-08)"
    );
    assert_eq!(store.schema(), "1.0", "the store's stamp, ST-06");
}

#[test]
#[cfg_attr(miri, ignore = "opens SQLite through FFI")]
fn append_stored_returns_core_s_envelope_in_ev01_s_key_order() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = Store::open(&ledger_path(dir.path())).expect("opens");

    let appended = store
        .append_stored(&task_done("T1", "s1"))
        .expect("appends");

    assert_eq!(appended.position, 1, "append reports its position");
    let text = serde_json::to_string(&appended.event).expect("serialises");
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&text).expect("JSON"),
        serde_json::json!({
            "type": "bench.task_done", "version": 1, "ts": "2026-10-11T09:30:00Z",
            "actor": "cli", "trust": {"level": "asserted"},
            "writer": {"binary": "bench", "version": env!("CARGO_PKG_VERSION"), "spec": "1.0.0"},
            "session": "s1", "data": {"task": "T1"},
        }),
        "EV-01's envelope, absent keys left out, the payload under data"
    );
    assert!(
        text.starts_with(r#"{"type":"bench.task_done","version":1,"ts":"#)
            && text.ends_with(r#""session":"s1","data":{"task":"T1"}}"#),
        "EV-01's key order: {text}"
    );
}

#[test]
#[cfg_attr(miri, ignore = "opens SQLite through FFI")]
fn the_stored_reads_print_core_s_envelope_as_stored_and_skip_what_the_gates_skip() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = ledger_path(dir.path());
    let store = Store::open(&path).expect("opens");
    let appended = store
        .append_stored(&task_done("T1", "s1"))
        .expect("appends");
    let other = envelope_text(
        "bench.task_done",
        1,
        "bench",
        r#"{"task":"T2"},"origin":"0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:41","later":true"#,
    );
    append_raw(&path, "bench.task_done", &other, &[]);
    append_raw(
        &path,
        "bench.verdict_set",
        &envelope_text("bench.verdict_set", 2, "bench", r#"{"pr":7}"#),
        &[],
    );

    let read = store.query_stored(&Filter::default()).expect("reads");
    let tail = store.tail_stored(&Filter::default(), 2).expect("reads");
    let printed: Vec<serde_json::Value> = read
        .events
        .iter()
        .map(|stored| serde_json::to_value(stored).expect("serialises"))
        .collect();

    assert_eq!(
        (read.events.len(), read.unusable),
        (2, 1),
        "the newer version is skipped and counted (EV-10)"
    );
    assert_eq!(
        read.events.first().map(|stored| &stored.event),
        Some(&appended.event),
        "a query prints what append printed"
    );
    assert_eq!(
        printed.get(1),
        Some(&serde_json::json!({
            "position": 2,
            "event": serde_json::from_str::<serde_json::Value>(&other).expect("JSON"),
        })),
        "origin and a key a later SPEC adds are kept as stored"
    );
    assert_eq!(
        (
            tail.events.iter().map(|s| s.position).collect::<Vec<_>>(),
            tail.unusable
        ),
        (vec![2], 1),
        "a tail of two: the last usable one, the newest counted"
    );
}

#[test]
#[cfg_attr(miri, ignore = "opens SQLite through FFI")]
fn a_query_reads_every_page_and_a_tail_only_benchs_last_events() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = ledger_path(dir.path());
    let store = Store::open(&path).expect("opens");
    let count = super::PAGE.get().saturating_add(1);
    for n in 0..count {
        store
            .append(&task_done(&format!("T{n}"), "s1"))
            .expect("appends");
    }
    append_raw(
        &path,
        "weighin.item_raised",
        &envelope_text(
            "weighin.item_raised",
            1,
            "weighin",
            r#"{"item":"wi-3fa9c01b2e"}"#,
        ),
        &[],
    );

    let all = store.query(&Filter::default()).expect("reads");
    let last_three = store.tail(&Filter::default(), 3).expect("reads");

    let expected: Vec<u64> = (1..=u64::try_from(count).expect("fits")).collect();
    assert_eq!(positions(&all), expected, "past one page, in order");
    let end = u64::try_from(count).expect("fits");
    assert_eq!(
        positions(&last_three),
        [end - 2, end - 1, end],
        "the last three Bench events, not the foreign one after them"
    );
}

#[test]
#[cfg_attr(miri, ignore = "opens SQLite through FFI")]
fn a_read_creates_no_store_and_an_existing_one_opens() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = ledger_path(dir.path());

    let missing = Store::open_existing(&path, Wait::Hook).expect("no error");
    drop(Store::open(&path).expect("creates"));
    let present = Store::open_existing(&path, Wait::Hook).expect("no error");

    assert!(missing.is_none(), "nothing there");
    assert!(present.is_some(), "the store opens");
}

#[test]
#[cfg_attr(miri, ignore = "opens SQLite through FFI")]
fn opening_an_existing_store_that_is_absent_leaves_nothing_behind() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("nested").join("ledger.sqlite3");

    let missing = Store::open_existing(&path, Wait::Command).expect("no error");

    assert!(missing.is_none(), "nothing there");
    assert_eq!(
        std::fs::read_dir(dir.path()).expect("lists").count(),
        0,
        "neither the store nor its directory was created"
    );
}

#[test]
fn an_event_that_does_not_read_back_says_why_and_that_it_was_written() {
    use std::error::Error as _;

    let missing = super::StoreError::NotReadBack {
        position: 7,
        cause: super::ReadBack::Missing,
    };
    let skipped = super::StoreError::NotReadBack {
        position: 7,
        cause: super::ReadBack::Skipped(redkiln_core::envelope::Skip::Foreign),
    };

    assert_eq!(
        [&missing, &skipped]
            .map(|error| (error.to_string(), error.source().map(ToString::to_string))),
        [
            (
                "the event was written at position 7 but did not read back; do not append it \
                 again"
                    .to_owned(),
                Some("no event at that position".to_owned())
            ),
            (
                "the event was written at position 7 but did not read back; do not append it \
                 again"
                    .to_owned(),
                Some("the read skipped it".to_owned())
            ),
        ],
        "the position, that it was written, and the cause"
    );
    assert_eq!(
        skipped
            .source()
            .and_then(std::error::Error::source)
            .map(ToString::to_string),
        Some("an event of another type".to_owned()),
        "the skip itself"
    );
}
