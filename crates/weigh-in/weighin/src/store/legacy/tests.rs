use std::num::NonZeroUsize;
use std::path::Path;
use std::time::Duration;

use redkiln_core::StoreId;
use redkiln_core::envelope::Timestamp;
use redkiln_core::ids::{parse_event_ref, parse_store_id};
use redkiln_core::journal::{ImportReport, ImportSource};
use redkiln_core::legacy::{LegacyLedger, read_ledger};
use redkiln_core::store::Wait;
use serde_json::Value;

use super::identity::JOURNAL_STORE;
use super::*;

/// A 1.x raise, as 1.x wrote it to a journal.
const RAISED: &str = r#"{"type":"item_raised","item_id":"wi-b29e54","kind":"question","question":"Keep the cache warm or cold?","door_hint":"unsure","authority_hint":"authority","channel":"manual","quote":"Keep the cache warm or cold?","actor":"cli","session_id":"s","worktree":"repo","ts":"2026-10-09T03:29:36Z"}"#;
/// A 1.x presentation of the same item.
const PRESENTED: &str = r#"{"type":"item_presented","item_id":"wi-b29e54","surface":"chat","reason":"shown in chat","actor":"cli","session_id":"s","worktree":"repo","ts":"2026-10-09T03:29:37Z"}"#;

fn store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&dir.path().join("redkiln/events.sqlite3"), Wait::Command)
        .expect("the store opens");
    (dir, store)
}

/// Core's 1.x ledger fixture `name`, read from a scratch copy (never in place).
fn ledger(name: &str) -> LegacyLedger {
    let from = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../core/redkiln-core/tests/fixtures")
        .join(name);
    let dir = tempfile::tempdir().expect("tempdir");
    let copy = dir.path().join(name);
    std::fs::copy(from, &copy).expect("copied");
    read_ledger(&copy).expect("the ledger reads")
}

/// An import with no deadline, recorded as from `path` under `store_id`.
fn recording(store: &Store, path: &str, store_id: StoreId) -> Import {
    Import {
        recording: Recording {
            source: ImportSource::Legacy {
                path: path.to_owned(),
                store_id,
            },
            ts: Timestamp::parse("2026-11-08T10:00:00Z").expect("valid"),
            author: store.author("cli").expect("an author"),
        },
        deadline: None,
    }
}

fn completed(import: OldImport) -> (ImportReport, Vec<(u64, String)>) {
    match import.outcome {
        MappedImport::Completed(report) => (report, import.problems),
        MappedImport::Stopped(report) => panic!("stopped: {report:?}"),
    }
}

fn journal(
    store: &Store,
    bytes: &str,
    held: &Held,
    path: &str,
) -> (ImportReport, Vec<(u64, String)>) {
    completed(
        store
            .import_journal(
                bytes.as_bytes(),
                held,
                recording(store, path, JOURNAL_STORE),
            )
            .expect("imported"),
    )
}

/// Every weighin event in the store, as stored.
fn weighin_events(store: &Store) -> Vec<Stored> {
    store
        .core()
        .read(
            &wire::filter().expect("a filter"),
            0,
            NonZeroUsize::new(1000).expect("non-zero"),
        )
        .expect("read")
}

/// A 1.x ledger event at `at`, holding `line`.
fn stored_at(at: &str, line: &str) -> Stored {
    let env = EventEnvelope::from_json(line.as_bytes()).expect("a 1.x event");
    Stored {
        position: parse_event_ref(at).expect("a ref").position().get(),
        id: parse_event_ref(at).expect("a ref"),
        recorded_at_millis: 0,
        event_type: env.event.event_type().to_owned(),
        data: line.as_bytes().to_vec(),
        tags: Vec::new(),
    }
}

/// Every weighin event the store holds, read with no deadline.
fn all_held(store: &Store) -> Held {
    match store.held(None).expect("held") {
        HeldRead::All(held) => held,
        HeldRead::PastDeadline => panic!("no deadline to pass"),
    }
}

fn content_id(line: &str) -> EventId {
    let env = EventEnvelope::from_json(line.as_bytes()).expect("a 1.x event");
    Canonical::of(env)
        .expect("encoded")
        .0
        .content_id()
        .expect("an identity")
}

#[test]
fn a_ledgers_events_keep_their_store_id_and_position_as_weighin_types() {
    let (_dir, store) = store();
    let old = ledger("weighin-1.2.0.sqlite3");

    let (report, problems) = completed(
        store
            .import_ledger(&old.events, recording(&store, "ledger", old.store_id))
            .expect("imported"),
    );

    assert_eq!(problems, Vec::new());
    let count = u64::try_from(old.events.len()).expect("fits");
    assert_eq!(
        (report.read, report.imported, report.malformed),
        (count, count, 0)
    );
    let stored = weighin_events(&store);
    let ids: Vec<_> = stored.iter().map(|s| s.id).collect();
    let expected: Vec<_> = old.events.iter().map(|e| e.id).collect();
    assert_eq!(ids, expected, "each under its ledger identity");
    for (stored, old) in stored.iter().zip(&old.events) {
        assert_eq!(stored.id.store(), old.id.store());
        assert_eq!(stored.event_type, format!("weighin.{}", old.event_type));
        let envelope: Value = serde_json::from_slice(&stored.data).expect("json");
        assert_eq!(envelope.get("worktree"), None, "{envelope}");
    }
}

#[test]
fn a_ledger_event_core_refuses_is_skipped_as_malformed_not_fatal() {
    let (_dir, store) = store();
    let at = "0123456789abcdef0123456789abcdef";
    let bad = PRESENTED.replace("2026-10-09T03:29:37Z", "yesterday");
    let events = [
        stored_at(&format!("{at}:1"), &bad),
        stored_at(&format!("{at}:2"), RAISED),
    ];

    let (report, problems) = completed(
        store
            .import_ledger(
                &events,
                recording(&store, "ledger", parse_store_id(at).expect("ID-01")),
            )
            .expect("imported"),
    );

    assert_eq!((report.read, report.imported, report.malformed), (2, 1, 1));
    let [(number, why)] = problems.as_slice() else {
        panic!("one problem: {problems:?}");
    };
    assert_eq!(*number, 1);
    assert!(why.contains("ts"), "{why}");
}

#[test]
fn a_journal_line_equal_to_a_held_event_takes_its_identity_and_adds_nothing() {
    let (_dir, store) = store();
    let at = "0123456789abcdef0123456789abcdef";
    let events = [stored_at(&format!("{at}:5"), RAISED)];
    let _ledger = completed(
        store
            .import_ledger(
                &events,
                recording(&store, "ledger", parse_store_id(at).expect("ID-01")),
            )
            .expect("imported"),
    );
    let held = all_held(&store);

    let (report, _) = journal(&store, &format!("{RAISED}\n"), &held, "s.jsonl");

    assert_eq!((report.read, report.imported, report.present), (1, 0, 1));
    assert_eq!(weighin_events(&store).len(), 1);
}

#[test]
fn a_journal_line_matching_nothing_takes_the_content_identity() {
    let (_dir, store) = store();

    let (report, _) = journal(
        &store,
        &format!("{PRESENTED}\n"),
        &Held::default(),
        "s.jsonl",
    );

    assert_eq!((report.read, report.imported), (1, 1));
    let ids: Vec<_> = weighin_events(&store).iter().map(|s| s.id).collect();
    assert_eq!(ids, [content_id(PRESENTED)]);
    assert_eq!(ids.first().map(|id| id.store()), Some(JOURNAL_STORE));
}

#[test]
fn the_same_line_in_two_journals_is_stored_once() {
    let (_dir, store) = store();
    let held = Held::default();

    let (first, _) = journal(&store, &format!("{RAISED}\n"), &held, "a.jsonl");
    let (second, _) = journal(&store, &format!("{RAISED}\r\n"), &held, "b.jsonl");

    assert_eq!((first.imported, second.imported, second.present), (1, 0, 1));
    assert_eq!(weighin_events(&store).len(), 1);
}

#[test]
fn a_malformed_journal_line_is_skipped_and_reported_and_the_rest_imported() {
    let (_dir, store) = store();

    let (report, problems) = journal(
        &store,
        &format!("{RAISED}\nnot json\n\n{PRESENTED}\n"),
        &Held::default(),
        "s.jsonl",
    );

    assert_eq!(
        (report.read, report.imported, report.malformed),
        (3, 2, 1),
        "the blank line is not read"
    );
    let [(number, why)] = problems.as_slice() else {
        panic!("one problem: {problems:?}");
    };
    assert_eq!(*number, 2);
    assert!(why.contains("not a 1.x ledger event"), "{why}");
}

#[test]
fn a_torn_last_line_is_passed_over_not_counted() {
    let (_dir, first) = store();
    let (_other_dir, other) = store();
    let cut = PRESENTED.get(..40).expect("long enough");
    let accented = PRESENTED.replace("shown in chat", "warm — cold");
    let dash = accented.find('—').expect("a dash");
    let mut inside = format!("{RAISED}\n").into_bytes();
    inside.extend(accented.as_bytes().get(..=dash).expect("long enough"));

    let (torn, problems) = journal(&first, &format!("{RAISED}\n{cut}"), &Held::default(), "a");
    let (split, _) = completed(
        other
            .import_journal(
                &inside,
                &Held::default(),
                recording(&other, "b", JOURNAL_STORE),
            )
            .expect("imported"),
    );

    assert_eq!((torn.read, torn.imported, torn.malformed), (1, 1, 0));
    assert_eq!(problems, Vec::new());
    assert_eq!((split.read, split.imported, split.malformed), (1, 1, 0));
}

#[test]
fn a_line_differing_only_in_worktree_from_a_ledger_event_adds_nothing() {
    let (_dir, store) = store();
    let at = "0123456789abcdef0123456789abcdef";
    let events = [stored_at(&format!("{at}:1"), PRESENTED)];
    let _ledger = completed(
        store
            .import_ledger(
                &events,
                recording(&store, "ledger", parse_store_id(at).expect("ID-01")),
            )
            .expect("imported"),
    );
    let elsewhere = PRESENTED.replace(r#""worktree":"repo""#, r#""worktree":"other-checkout""#);

    let (report, _) = journal(&store, &elsewhere, &all_held(&store), "s.jsonl");

    assert_eq!((report.imported, report.present), (0, 1));
    let ids: Vec<_> = weighin_events(&store).iter().map(|s| s.id).collect();
    assert_eq!(ids, [parse_event_ref(&format!("{at}:1")).expect("a ref")]);
}

#[test]
fn held_reads_every_page_of_the_store() {
    let (_dir, store) = store();
    let lines: String = (0..600)
        .map(|n| PRESENTED.replace("shown in chat", &format!("shown {n}")) + "\n")
        .collect();

    let (report, _) = journal(&store, &lines, &Held::default(), "s.jsonl");
    let held = all_held(&store);

    assert_eq!(report.imported, 600);
    assert_eq!(held.0.len(), 600, "every page read");
}

#[test]
fn held_past_its_deadline_reads_no_page() {
    let (_dir, store) = store();
    let (report, _) = journal(
        &store,
        &format!("{PRESENTED}\n"),
        &Held::default(),
        "s.jsonl",
    );
    let passed = Instant::now()
        .checked_sub(Duration::from_secs(1))
        .expect("the clock has run that long");

    let read = store.held(Some(passed)).expect("no store error");

    assert_eq!(report.imported, 1, "the store holds an event to read");
    assert!(
        matches!(read, HeldRead::PastDeadline),
        "no page read past the deadline (wi-233b23): {read:?}"
    );
}
