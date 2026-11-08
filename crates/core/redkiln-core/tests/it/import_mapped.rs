//! The mapped import (SPEC ST-15, JR-10 to JR-13): events a plugin mapped from an old source,
//! stored under the identities it gave them through the journal import's own line rules, and an
//! origin position past what the store holds read as a malformed line on both import paths.

use std::io::Cursor;
use std::num::NonZeroUsize;
use std::path::Path;
use std::time::Instant;

use redkiln_core::EventId;
use redkiln_core::envelope::{
    Actor, Author, Class, Envelope, Payload, Timestamp, Writer, decode_all,
};
use redkiln_core::ids::{SessionId, parse_event_ref};
use redkiln_core::journal::{
    ImportCompleted, ImportSource, LineError, Mapped, MappedImport, Recording, import,
    import_mapped,
};
use redkiln_core::store::{CoreKey, Filter, NewEvent, Store, Stored, Tag, Wait};
use redkiln_core::trust::{Level, Trust, Via};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The first 32 hex of `sha256("weighin-1x-journal")`, the store id M1-A gives 1.x journal lines.
const FOREIGN: &str = "8f161dc3d6d01eac2c7949e3b17ffa04";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Raised {
    item: String,
}

impl Payload for Raised {
    const TYPE: &'static str = "weighin.item_raised";
    const VERSION: u32 = 1;
    const CLASS: Class = Class::Durable;
}

fn ts() -> Timestamp {
    Timestamp::parse("2026-11-08T10:00:00Z").expect("valid")
}

fn author(trust: Trust) -> Author {
    Author {
        actor: Actor::Agent,
        trust,
        writer: Writer::new("weighin", "2.0.0").expect("valid"),
    }
}

/// A raise of `item` in session `s1`, witnessed, with a caller tag.
fn raised(item: &str) -> NewEvent {
    let witnessed = Trust::new(Level::Witnessed, Some(Via::AskUserQuestion))
        .and_then(|t| t.with_reference("toolu_01"))
        .expect("valid");
    let mut envelope = Envelope::new(
        ts(),
        author(witnessed),
        Raised {
            item: item.to_owned(),
        },
    );
    envelope.session = Some(SessionId::parse("s1").expect("valid"));
    envelope
        .to_event([Tag::core(CoreKey::Item, item).expect("valid")])
        .expect("an event")
}

fn id(text: &str) -> EventId {
    parse_event_ref(text).expect("an event ref")
}

fn store(dir: &Path) -> Store {
    Store::open(&dir.join("events.sqlite3"), Wait::Command).expect("opens")
}

fn all(store: &Store) -> Vec<Stored> {
    store
        .read(
            &Filter::all(),
            0,
            NonZeroUsize::new(1000).expect("non-zero"),
        )
        .expect("read")
}

fn recording(path: &str) -> Recording {
    Recording {
        source: ImportSource::Legacy {
            path: path.to_owned(),
            store_id: FOREIGN.to_owned(),
        },
        ts: ts(),
        author: author(Trust::asserted()),
    }
}

fn completed(outcome: MappedImport) -> redkiln_core::journal::ImportReport {
    match outcome {
        MappedImport::Completed(report) => report,
        MappedImport::Stopped(report) => panic!("stopped: {report:?}"),
    }
}

fn mapped(at: &str, item: &str) -> Mapped {
    Mapped::Event {
        id: id(at),
        event: raised(item),
    }
}

#[test]
fn a_mapped_event_is_stored_under_the_identity_it_was_given_at_asserted_trust() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = store(dir.path());
    let given = format!("{FOREIGN}:7");

    let report = completed(
        import_mapped(
            &target,
            [(1, mapped(&given, "wi-000001"))],
            recording("s1.jsonl"),
            None,
        )
        .expect("imported"),
    );

    assert_eq!((report.read, report.imported, report.malformed), (1, 1, 0));
    let stored = all(&target);
    let fact = stored
        .iter()
        .find(|s| s.event_type == Raised::TYPE)
        .expect("the fact");
    assert_eq!(fact.id, id(&given));
    let envelope: Value = serde_json::from_slice(&fact.data).expect("json");
    assert_eq!(envelope["trust"]["level"], "asserted");
    assert_eq!(envelope["trust"]["claimed"], "witnessed");
    let mut tags: Vec<&str> = fact.tags.iter().map(Tag::as_str).collect();
    tags.sort_unstable();
    assert_eq!(
        tags,
        [
            "item:wi-000001",
            "plugin:weighin",
            "session:s1",
            "trust:asserted"
        ]
    );
}

#[test]
fn sparse_positions_under_a_foreign_store_id_are_all_stored() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = store(dir.path());
    let positions = ["7", "1", "4611686018427387904", "9223372036854775807"];
    let records = || {
        (1_u64..)
            .zip(positions)
            .map(|(n, p)| {
                (
                    n,
                    mapped(&format!("{FOREIGN}:{p}"), &format!("wi-00000{n}")),
                )
            })
            .collect::<Vec<_>>()
    };

    let first =
        completed(import_mapped(&target, records(), recording("a.jsonl"), None).expect("imported"));
    let second =
        completed(import_mapped(&target, records(), recording("b.jsonl"), None).expect("imported"));

    assert_eq!((first.imported, first.present, first.malformed), (4, 0, 0));
    assert_eq!((second.imported, second.present), (0, 4), "idempotent");
    let ids: Vec<String> = all(&target)
        .iter()
        .filter(|s| s.event_type == Raised::TYPE)
        .map(|s| s.id.to_string())
        .collect();
    let expected: Vec<String> = positions.iter().map(|p| format!("{FOREIGN}:{p}")).collect();
    assert_eq!(ids, expected, "each under its exact id");
}

#[test]
fn an_unmapped_record_is_read_and_skipped_as_malformed_with_its_number() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = store(dir.path());

    let report = completed(
        import_mapped(
            &target,
            [
                (3, mapped(&format!("{FOREIGN}:3"), "wi-000003")),
                (5, Mapped::Unmapped),
            ],
            recording("s1.jsonl"),
            None,
        )
        .expect("imported"),
    );

    assert_eq!((report.read, report.imported, report.malformed), (2, 1, 1));
    let [problem] = report.problems.as_slice() else {
        panic!("one problem: {:?}", report.problems);
    };
    assert_eq!(problem.line, 5);
    assert!(matches!(problem.reason, LineError::Unmapped), "{problem:?}");
}

/// JR-02's example line at `origin`.
fn line_at(origin: &str, item: &str) -> String {
    format!(
        r#"{{"type":"weighin.item_raised","version":1,"ts":"2026-10-31T04:12:40Z","actor":"agent","trust":{{"level":"asserted"}},"writer":{{"binary":"weighin","version":"2.0.0","spec":"1.0.0"}},"origin":"{origin}","data":{{"item":"{item}"}}}}"#
    )
}

#[test]
fn a_line_whose_origin_position_passes_i64_max_is_malformed_and_the_rest_imported() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = store(dir.path());
    let journal = [
        line_at(&format!("{FOREIGN}:1"), "wi-000001"),
        line_at(&format!("{FOREIGN}:9223372036854775808"), "wi-000002"),
        line_at(&format!("{FOREIGN}:2"), "wi-000003"),
    ]
    .join("\n");
    let recording = Recording {
        source: ImportSource::Artifact("past".to_owned()),
        ts: ts(),
        author: author(Trust::asserted()),
    };

    let report = import(&target, Cursor::new(journal.into_bytes()), recording).expect("imported");

    assert_eq!((report.read, report.imported, report.malformed), (3, 2, 1));
    let [problem] = report.problems.as_slice() else {
        panic!("one problem: {:?}", report.problems);
    };
    assert_eq!(problem.line, 2);
    assert!(
        matches!(
            problem.reason,
            LineError::PositionOutOfRange {
                position: 9_223_372_036_854_775_808
            }
        ),
        "{problem:?}"
    );
}

#[test]
fn a_mapped_event_past_i64_max_is_malformed_not_a_failed_import() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = store(dir.path());

    let report = completed(
        import_mapped(
            &target,
            [
                (
                    1,
                    mapped(&format!("{FOREIGN}:18446744073709551615"), "wi-000001"),
                ),
                (2, mapped(&format!("{FOREIGN}:2"), "wi-000002")),
            ],
            recording("s1.jsonl"),
            None,
        )
        .expect("imported"),
    );

    assert_eq!((report.read, report.imported, report.malformed), (2, 1, 1));
    assert!(
        matches!(
            report.problems.first().map(|p| (p.line, &p.reason)),
            Some((1, LineError::PositionOutOfRange { .. }))
        ),
        "{:?}",
        report.problems
    );
}

#[test]
fn a_mapped_import_is_recorded_once_naming_its_legacy_source_and_store_id() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = store(dir.path());

    let report = completed(
        import_mapped(
            &target,
            [
                (1, mapped(&format!("{FOREIGN}:1"), "wi-000001")),
                (2, Mapped::Unmapped),
            ],
            recording("/repo/.kb/_ledger/journal/s1.jsonl"),
            None,
        )
        .expect("imported"),
    );

    let stored = all(&target);
    let records = decode_all::<ImportCompleted>(&stored);
    let [record] = records.events.as_slice() else {
        panic!("one record: {:?}", records.events.len());
    };
    assert_eq!(
        (
            record.data.read,
            record.data.imported,
            record.data.present,
            record.data.malformed
        ),
        (
            report.read,
            report.imported,
            report.present,
            report.malformed
        )
    );
    let raw = stored
        .iter()
        .find(|s| s.event_type == ImportCompleted::TYPE)
        .expect("the record");
    let envelope: Value = serde_json::from_slice(&raw.data).expect("json");
    assert_eq!(
        envelope["data"]["source"],
        serde_json::json!({"legacy": {"path": "/repo/.kb/_ledger/journal/s1.jsonl", "store_id": FOREIGN}})
    );
}

#[test]
fn a_mapped_import_past_its_deadline_stores_no_batch_and_records_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = store(dir.path());
    let deadline = Instant::now();

    let outcome = import_mapped(
        &target,
        [(1, mapped(&format!("{FOREIGN}:1"), "wi-000001"))],
        recording("s1.jsonl"),
        Some(deadline),
    )
    .expect("no failure");

    let MappedImport::Stopped(report) = outcome else {
        panic!("stopped: {outcome:?}");
    };
    assert_eq!(report.imported, 0);
    assert_eq!(all(&target), Vec::new(), "nothing stored, nothing recorded");
}

#[test]
fn a_second_mapped_import_appends_only_its_record() {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = store(dir.path());
    let records = || [(1, mapped(&format!("{FOREIGN}:1"), "wi-000001"))];

    let first =
        completed(import_mapped(&target, records(), recording("s1.jsonl"), None).expect("first"));
    assert_eq!(first.imported, 1);
    let before = all(&target).len();
    let second =
        completed(import_mapped(&target, records(), recording("s1.jsonl"), None).expect("second"));

    assert_eq!((second.imported, second.present), (0, 1));
    assert_eq!(
        all(&target).len(),
        before.saturating_add(1),
        "its record only"
    );
}
