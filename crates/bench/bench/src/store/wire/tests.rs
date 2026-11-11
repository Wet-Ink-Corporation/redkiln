//! The adapter between the 0.x envelope and Core's (plan test list 15 to 17).

use std::num::NonZeroUsize;
use std::path::Path;

use redkiln_core::envelope::Field;
use redkiln_core::store::{Filter as CoreFilter, Store as CoreStore, Tag, Wait};

use crate::store::envelope::TimestampError;
use crate::store::{EventEnvelope, Filter, SchemaError, Store, StoreError};

/// A worktree id, as ID-06 writes one.
const WORKTREE: &str = "wt-0123456789ab";

/// One 0.x envelope of each of the 17 kinds, with every context field set and every optional
/// payload field written, as `bench ledger append` takes them.
const ONE_OF_EACH: [&str; 17] = [
    r#"{"type":"test_run","task":"T1","command":"cargo test","outcome":"green","exit":0,"duration_ms":12,"timeout_secs":60}"#,
    r#"{"type":"task_done","task":"T1"}"#,
    r#"{"type":"plan_approved","plan":"docs/plan.md","hash":"sha256:00","by":{"actor":"human","via":"mod"}}"#,
    r#"{"type":"grill_confirmed","plan":"docs/tree.md","hash":"sha256:01","quote":"yes, that"}"#,
    r#"{"type":"repro_red","task":"T2","test":"it::bug"}"#,
    r#"{"type":"verdict_set","pr":7,"verdict":"approved","source":"human","sha":"0123456789abcdef0123456789abcdef01234567","patch_id":"fedcba9876543210fedcba9876543210fedcba98","base":"main","by":{"actor":"human","via":"bash"}}"#,
    r#"{"type":"round_started","pr":7,"ticket":"T3","round":2}"#,
    r#"{"type":"gate_blocked","point":"guard.rm-rf","reason":"rm -rf /"}"#,
    r#"{"type":"stop_blocked","point":"stop.status","reason":"no Status block"}"#,
    r#"{"type":"mode_set","mode":"build","clean":false,"plan":"docs/plan.md","tree":"docs/tree.md"}"#,
    r#"{"type":"subagent_status","agent":"Explore","agent_id":"a1","status":"done","violation":"no status","missing":["Status"],"duration_ms":5,"tokens":{"input":1,"output":2,"cache_read":3,"cache_creation":4}}"#,
    r#"{"type":"migrate.wave.started","migration":"users-email","wave":1}"#,
    r#"{"type":"migrate.wave.done","migration":"users-email","wave":1}"#,
    r#"{"type":"migrate.contract","migration":"users-email"}"#,
    r#"{"type":"weighin_parked","item_id":"wi-e41e02","ticket":"36","decision":"wait"}"#,
    r#"{"type":"bridge.unavailable","ticket":"36","reason":"no weighin"}"#,
    r#"{"type":"orchestrate.ruled","ticket":"36","what":"keep the cache","why":"reversible","cost_if_wrong":"one revert","decision":"later"}"#,
];

/// `payload` (a 0.x JSON object without the envelope keys) with the envelope keys added.
fn with_context(payload: &str) -> String {
    let body = payload.strip_suffix('}').expect("an object");
    format!(
        r#"{body},"actor":"agent","ts":"2026-11-11T09:30:00.120Z","session_id":"s-1","worktree":"{WORKTREE}","branch":"main","head":"0123456789abcdef0123456789abcdef01234567"}}"#
    )
}

fn store_at(dir: &Path) -> Store {
    Store::open(&dir.join("ledger.sqlite3")).expect("opens")
}

/// The store's raw events, as Core reads them.
fn raw(dir: &Path) -> Vec<redkiln_core::store::Stored> {
    let store = CoreStore::open_existing(&dir.join("ledger.sqlite3"), Wait::Command)
        .expect("opens")
        .expect("exists");
    store
        .read(
            &CoreFilter::all(),
            0,
            NonZeroUsize::new(64).expect("non-zero"),
        )
        .expect("reads")
}

#[test]
#[cfg_attr(miri, ignore = "opens SQLite through FFI")]
fn every_kind_reads_back_as_the_envelope_written() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = store_at(dir.path());
    let written: Vec<EventEnvelope> = ONE_OF_EACH
        .iter()
        .map(|payload| {
            let value = serde_json::from_str(&with_context(payload)).expect("JSON");
            EventEnvelope::from_value(&value).unwrap_or_else(|e| panic!("{payload}: {e}"))
        })
        .collect();
    for envelope in &written {
        store.append(envelope).expect("appends");
    }

    let read: Vec<EventEnvelope> = store
        .query(&Filter::default())
        .expect("reads")
        .into_iter()
        .map(|recorded| recorded.event)
        .collect();

    assert_eq!(read, written, "Invariant 8: nothing lost, nothing added");
}

#[test]
#[cfg_attr(miri, ignore = "opens SQLite through FFI")]
fn a_plan_approval_is_stored_as_core_writes_bench_plan_approved() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = store_at(dir.path());
    let envelope = EventEnvelope::parse(
        format!(
            r#"{{"type":"plan_approved","plan":"docs/plan.md","hash":"sha256:00","actor":"cli","ts":"2026-11-11T09:30:00Z","session_id":"s-1","worktree":"{WORKTREE}"}}"#
        )
        .as_bytes(),
    )
    .expect("a valid event");

    store.append(&envelope).expect("appends");

    let stored = raw(dir.path());
    let event = stored.first().expect("one event");
    let mut tags: Vec<&str> = event.tags.iter().map(Tag::as_str).collect();
    tags.sort_unstable();
    let golden = format!(
        r#"{{"type":"bench.plan_approved","version":1,"ts":"2026-11-11T09:30:00Z","actor":"cli","trust":{{"level":"asserted"}},"writer":{{"binary":"bench","version":"{}","spec":"1.0.0"}},"session":"s-1","worktree":"{WORKTREE}","data":{{"plan":"docs/plan.md","hash":"sha256:00"}}}}"#,
        env!("CARGO_PKG_VERSION")
    );
    assert_eq!(
        (
            event.event_type.as_str(),
            String::from_utf8_lossy(&event.data)
        ),
        ("bench.plan_approved", golden.into()),
        "EV-01's envelope, the payload under data"
    );
    assert_eq!(
        tags,
        [
            "bench.plan:docs/plan.md",
            "plugin:bench",
            "session:s-1",
            "trust:asserted",
            &format!("worktree:{WORKTREE}"),
        ],
        "the subject under bench, the rest Core's"
    );
}

#[test]
fn a_timestamp_core_refuses_is_a_ts_error_before_any_store() {
    for ts in ["2026-11-11T11:30:00+02:00", "2026-11-11T09:30:00.12Z"] {
        let result = EventEnvelope::parse(
            format!(r#"{{"type":"task_done","task":"T1","actor":"cli","ts":"{ts}"}}"#).as_bytes(),
        );

        assert!(
            matches!(&result, Err(SchemaError::Json(e)) if e.to_string().contains(ts)),
            "{ts}: {result:?}"
        );
    }
    assert!(
        matches!(
            crate::store::Timestamp::try_from("2026-11-11T09:30:00.12Z".to_owned()),
            Err(TimestampError::Parse { .. })
        ),
        "two fraction digits (EV-01)"
    );
}

#[test]
#[cfg_attr(miri, ignore = "opens SQLite through FFI")]
fn an_actor_a_worktree_or_a_subject_core_refuses_is_an_error_naming_it_and_writes_nothing() {
    let dir = tempfile::tempdir().expect("temp dir");
    let store = store_at(dir.path());
    // Built in code, as a caller would, so the adapter is what refuses (the CLI's parse
    // refuses a bad worktree or subject earlier, as a tag).
    let event = |actor: &str, worktree: Option<&str>, task: &str| {
        let ts = crate::store::Timestamp::try_from("2026-11-11T09:30:00Z".to_owned())
            .expect("a valid ts");
        let mut envelope = EventEnvelope::new(
            crate::store::LedgerEvent::TaskDone(bench_contract::event::TaskDone {
                task: task.to_owned(),
            }),
            actor,
            ts,
        );
        envelope.worktree = worktree.map(str::to_owned);
        envelope
    };
    let long = "T".repeat(4096);

    let actor = store.append(&event("t", None, "T1"));
    let worktree = store.append(&event("cli", Some("/repo"), "T1"));
    let subject = store.append(&event("cli", None, &long));

    assert!(
        matches!(&actor, Err(StoreError::Field(e)) if e.field == Field::Actor),
        "{actor:?}"
    );
    assert!(
        matches!(&worktree, Err(StoreError::Id { field: Field::Worktree, value, .. }) if value == "/repo"),
        "{worktree:?}"
    );
    assert!(
        matches!(&subject, Err(StoreError::Tag(_))),
        "never shortened (EV-11): {subject:?}"
    );
    assert!(
        !dir.path().join("ledger.sqlite3").exists() || raw(dir.path()).is_empty(),
        "nothing was written"
    );
}
