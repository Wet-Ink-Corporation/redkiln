//! Journals through real stores and files (SPEC JR-01 to JR-14).

use std::io::Cursor;
use std::num::NonZeroUsize;
use std::path::Path;
use std::time::Duration;

use redkiln_core::envelope::{
    Actor, Author, Class, Envelope, Payload, Timestamp, Writer, decode, decode_all,
};
use redkiln_core::ids::SessionId;
use redkiln_core::journal::{
    Checkpoint, ImportCompleted, ImportReport, ImportSource, JournalName, Recording, checkpoint,
    import, import_file, lines,
};
use redkiln_core::store::{CoreKey, Filter, NewEvent, Store, Stored, Tag, Wait};
use redkiln_core::suite::RepoClass;
use redkiln_core::trust::{Level, Trust, Via};
use serde::{Deserialize, Serialize};

mod batches;
mod git_fixture;
mod lines;
mod refs;

const WAIT: Duration = Duration::from_secs(5);

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
    Timestamp::parse("2026-11-02T10:00:00Z").expect("valid")
}

fn author(trust: Trust) -> Author {
    Author {
        actor: Actor::Agent,
        trust,
        writer: Writer::new("weighin", "2.0.0").expect("valid"),
    }
}

fn raised(item: &str, session: Option<&str>, trust: Trust) -> NewEvent {
    let mut envelope = Envelope::new(
        ts(),
        author(trust),
        Raised {
            item: item.to_owned(),
        },
    );
    envelope.session = session.map(|s| SessionId::parse(s).expect("valid"));
    envelope
        .to_event([Tag::core(CoreKey::Item, item).expect("valid")])
        .expect("an event")
}

fn witnessed() -> Trust {
    Trust::new(Level::Witnessed, Some(Via::AskUserQuestion))
        .and_then(|t| t.with_reference("toolu_01"))
        .expect("valid")
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

/// A store with two events in session `A1`, one in `b2`, one with none, and one stored raw.
fn source_store(dir: &Path) -> Store {
    let store = Store::open(&dir.join("events.sqlite3"), Wait::Command).expect("opens");
    let raw =
        NewEvent::new("weighin.item_raised", br#"{"not":"an envelope"}"#.to_vec()).expect("event");
    store
        .append(
            vec![
                raised("wi-000001", Some("A1"), witnessed()),
                raised("wi-000002", Some("b2"), Trust::asserted()),
                raised("wi-000003", None, Trust::asserted()),
                raw,
                raised("wi-000004", Some("A1"), Trust::asserted()),
            ],
            None,
        )
        .expect("appended");
    store
}

fn journal_lines(path: &Path) -> Vec<String> {
    let bytes = std::fs::read(path).expect("a journal");
    assert!(bytes.ends_with(b"\n"), "each line ended by a newline");
    assert!(!bytes.starts_with(b"\xef\xbb\xbf"), "no BOM");
    String::from_utf8(bytes)
        .expect("utf-8")
        .lines()
        .map(str::to_owned)
        .collect()
}

fn written(outcome: Checkpoint) -> redkiln_core::journal::Exported {
    match outcome {
        Checkpoint::Written(exported) => exported,
        Checkpoint::Client => panic!("an own repository is exported"),
    }
}

#[test]
fn a_client_repository_writes_no_journal() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = source_store(dir.path());
    let journals = dir.path().join("journals");
    let outcome = checkpoint(&store, RepoClass::Client, &journals, WAIT).expect("no failure");
    assert!(matches!(outcome, Checkpoint::Client), "{outcome:?}");
    assert!(!journals.exists(), "nothing created");
}

#[test]
fn a_checkpoint_writes_each_session_its_journal_once() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = source_store(dir.path());
    let journals = dir.path().join("journals");
    let first = written(checkpoint(&store, RepoClass::Own, &journals, WAIT).expect("written"));
    assert_eq!(first.appended, 4);
    assert_eq!(first.unexportable, 1, "the raw event is no envelope");
    let stored = all(&store);
    let a1 = journal_lines(&journals.join("a1.jsonl"));
    assert_eq!(a1.len(), 2);
    assert!(
        a1[0].contains(&format!("\"origin\":\"{}\"", stored[0].id)),
        "{}",
        a1[0]
    );
    assert!(
        a1[1].contains(&format!("\"origin\":\"{}\"", stored[4].id)),
        "{}",
        a1[1]
    );
    assert!(
        a1[0].contains(r#""trust":{"level":"witnessed""#),
        "trust as stored"
    );
    assert_eq!(journal_lines(&journals.join("b2.jsonl")).len(), 1);
    let none = JournalName::of_store(store.store_id()).file_name();
    assert_eq!(journal_lines(&journals.join(none)).len(), 1);

    let again = written(checkpoint(&store, RepoClass::Own, &journals, WAIT).expect("written"));
    assert_eq!(again.appended, 0, "idempotent");
    assert_eq!(journal_lines(&journals.join("a1.jsonl")).len(), 2);
    store
        .append(
            vec![raised("wi-000005", Some("A1"), Trust::asserted())],
            None,
        )
        .expect("appended");
    let third = written(checkpoint(&store, RepoClass::Own, &journals, WAIT).expect("written"));
    assert_eq!(third.appended, 1, "only what the journal lacks");
    assert_eq!(journal_lines(&journals.join("a1.jsonl")).len(), 3);
}

#[test]
fn a_journal_another_writer_holds_is_reported_busy_and_the_others_are_written() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = source_store(dir.path());
    let journals = dir.path().join("journals");
    std::fs::create_dir(&journals).expect("the directory");
    let held = std::fs::File::create(journals.join("a1.jsonl")).expect("a journal");
    held.try_lock().expect("another writer holds a1");

    let outcome = written(
        checkpoint(&store, RepoClass::Own, &journals, Duration::from_millis(50))
            .expect("no failure"),
    );
    let a1 = JournalName::for_event(
        Some(&SessionId::parse("A1").expect("valid")),
        store.store_id(),
    );
    assert_eq!(outcome.busy, [a1], "the held journal is named");
    assert_eq!(outcome.appended, 2, "b2 and the sessionless journal");
    assert_eq!(outcome.journals.len(), 2, "{:?}", outcome.journals);
    // Windows' whole-file lock keeps readers out too, so the check reads after the release.
    drop(held);
    assert!(
        std::fs::read(journals.join("a1.jsonl"))
            .expect("read")
            .is_empty(),
        "nothing written under the lock"
    );
    assert_eq!(journal_lines(&journals.join("b2.jsonl")).len(), 1);

    let after = written(checkpoint(&store, RepoClass::Own, &journals, WAIT).expect("written"));
    assert_eq!(
        (after.appended, after.busy.len()),
        (2, 0),
        "caught up next time"
    );
}

#[test]
fn a_checkpoint_drops_a_torn_last_line_and_appends_its_event_again() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = source_store(dir.path());
    let journals = dir.path().join("journals");
    written(checkpoint(&store, RepoClass::Own, &journals, WAIT).expect("written"));
    let path = journals.join("a1.jsonl");
    let whole = std::fs::read(&path).expect("read");
    let second = whole.iter().position(|b| *b == b'\n').expect("two lines") + 1;
    // A writer that stopped mid-line, after another wrote a line by hand.
    let mut torn = whole[..second].to_vec();
    torn.extend_from_slice(b"\r\n\n");
    torn.extend_from_slice(&whole[second..whole.len() - 10]);
    std::fs::write(&path, &torn).expect("tear it");

    let again = written(checkpoint(&store, RepoClass::Own, &journals, WAIT).expect("written"));
    assert_eq!(again.appended, 1, "the torn line's event again");
    let mut expected = whole[..second].to_vec();
    expected.extend_from_slice(b"\r\n\n");
    expected.extend_from_slice(&whole[second..]);
    assert_eq!(
        std::fs::read(&path).expect("read"),
        expected,
        "complete lines kept in place"
    );
}

#[test]
fn a_last_line_that_decodes_without_its_newline_is_kept_and_ended() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = source_store(dir.path());
    let journals = dir.path().join("journals");
    written(checkpoint(&store, RepoClass::Own, &journals, WAIT).expect("written"));
    let path = journals.join("a1.jsonl");
    let mut bytes = std::fs::read(&path).expect("read");
    bytes.pop();
    std::fs::write(&path, &bytes).expect("unended");
    store
        .append(
            vec![raised("wi-000006", Some("A1"), Trust::asserted())],
            None,
        )
        .expect("appended");
    let again = written(checkpoint(&store, RepoClass::Own, &journals, WAIT).expect("written"));
    assert_eq!(again.appended, 1, "the decodable line holds its event");
    assert_eq!(journal_lines(&path).len(), 3);
}

fn importer() -> Author {
    author(Trust::asserted())
}

fn imported(store: &Store, path: &Path) -> ImportReport {
    import_file(store, path, ts(), importer()).expect("imported")
}

#[test]
fn an_import_appends_each_fact_once_at_asserted_and_records_itself() {
    let dir = tempfile::tempdir().expect("tempdir");
    let source = source_store(dir.path());
    let journals = dir.path().join("journals");
    written(checkpoint(&source, RepoClass::Own, &journals, WAIT).expect("written"));
    let path = journals.join("a1.jsonl");
    let before = std::fs::read(&path).expect("read");

    let other = tempfile::tempdir().expect("tempdir");
    let target = Store::open(&other.path().join("events.sqlite3"), Wait::Command).expect("opens");
    let first = imported(&target, &path);
    assert_eq!(
        (first.read, first.imported, first.present, first.malformed),
        (2, 2, 0, 0)
    );
    let second = imported(&target, &path);
    assert_eq!(
        (second.read, second.imported, second.present),
        (2, 0, 2),
        "idempotent"
    );
    assert_eq!(
        std::fs::read(&path).expect("read"),
        before,
        "the source is untouched"
    );

    let stored = all(&target);
    assert_imported_facts(&source, &stored);

    let records = decode_all::<ImportCompleted>(&stored);
    assert_eq!(records.events.len(), 2, "one record per import");
    let record = &records.events[0].data;
    assert_eq!(
        record.source,
        ImportSource::Path(path.to_string_lossy().into_owned())
    );
    assert_eq!(
        (
            record.read,
            record.imported,
            record.present,
            record.malformed
        ),
        (2, 2, 0, 0)
    );
}

/// `stored`, imported from `source`'s journal `a1`, holds its two facts once each, under their
/// origins, at `asserted` with the line's level claimed.
fn assert_imported_facts(source: &Store, stored: &[Stored]) {
    let source_ids: Vec<_> = all(source).iter().map(|s| s.id).collect();
    let facts: Vec<_> = stored
        .iter()
        .filter(|s| s.event_type == Raised::TYPE)
        .collect();
    assert_eq!(facts.len(), 2, "each fact once");
    assert_eq!(facts[0].id, source_ids[0], "stored under its origin");
    assert_eq!(facts[1].id, source_ids[4]);
    let read = decode::<Raised>(facts[0]).expect("usable");
    assert_eq!(read.trust.level(), Level::Asserted, "asserted on import");
    assert_eq!(
        read.trust.claimed(),
        Some(Level::Witnessed),
        "its line's level kept"
    );
    assert_eq!(read.trust.via(), Some(Via::AskUserQuestion));
    assert_eq!(
        read.origin, None,
        "identity lives in the store, not the bytes"
    );
    assert!(
        facts[0].tags.iter().any(|t| t.as_str() == "trust:asserted"),
        "{:?}",
        facts[0].tags
    );
}

#[test]
fn an_import_reports_malformed_lines_and_passes_over_a_torn_tail() {
    let dir = tempfile::tempdir().expect("tempdir");
    let source = source_store(dir.path());
    let journals = dir.path().join("journals");
    written(checkpoint(&source, RepoClass::Own, &journals, WAIT).expect("written"));
    let good = std::fs::read(journals.join("b2.jsonl")).expect("read");
    let mut journal = b"not json\n\n".to_vec();
    journal.extend_from_slice(&good);
    journal.extend_from_slice(b"{\"type\":\"weighin.item_raised\"}\r\n");
    journal.extend_from_slice(&good[..good.len() / 2]);

    let target = Store::open(&dir.path().join("target.sqlite3"), Wait::Command).expect("opens");
    let source_name = ImportSource::Artifact("review-run-7".to_owned());
    let recording = Recording {
        source: source_name.clone(),
        ts: ts(),
        author: importer(),
    };
    let outcome = import(&target, Cursor::new(&journal), recording).expect("imported");
    assert_eq!(
        (outcome.read, outcome.imported, outcome.malformed),
        (3, 1, 2)
    );
    assert!(outcome.incomplete, "the torn last line is passed over");
    let numbers: Vec<u64> = outcome.problems.iter().map(|p| p.line).collect();
    assert_eq!(numbers, [1, 4], "reported with their line numbers");
    let records = decode_all::<ImportCompleted>(&all(&target));
    assert_eq!(records.events[0].data.source, source_name);
}

#[test]
fn a_store_reimporting_its_own_journal_finds_it_present_and_refuses_a_position_it_lacks() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = source_store(dir.path());
    let journals = dir.path().join("journals");
    written(checkpoint(&store, RepoClass::Own, &journals, WAIT).expect("written"));
    let mut journal = std::fs::read(journals.join("a1.jsonl")).expect("read");
    let own = store.store_id().to_string();
    let claimed = std::str::from_utf8(&journal)
        .expect("utf-8")
        .lines()
        .next()
        .expect("a line")
        .replace(
            &format!("\"origin\":\"{own}:1\""),
            &format!("\"origin\":\"{own}:999\""),
        );
    journal.extend_from_slice(claimed.as_bytes());
    journal.push(b'\n');

    let recording = Recording {
        source: ImportSource::Artifact("own".to_owned()),
        ts: ts(),
        author: importer(),
    };
    let outcome = import(&store, Cursor::new(&journal), recording).expect("imported");
    assert_eq!(
        (
            outcome.read,
            outcome.imported,
            outcome.present,
            outcome.malformed
        ),
        (3, 0, 2, 1)
    );
    let numbers: Vec<u64> = outcome.problems.iter().map(|p| p.line).collect();
    assert_eq!(numbers, [3], "the unheld position is the malformed line");
}

#[test]
fn a_line_longer_than_the_limit_is_malformed_and_the_next_one_still_read() {
    let dir = tempfile::tempdir().expect("tempdir");
    let source = source_store(dir.path());
    let journals = dir.path().join("journals");
    written(checkpoint(&source, RepoClass::Own, &journals, WAIT).expect("written"));
    let mut journal = vec![b' '; redkiln_core::journal::MAX_LINE_BYTES + 10];
    journal.insert(0, b'x');
    journal.push(b'\n');
    journal.extend_from_slice(&std::fs::read(journals.join("b2.jsonl")).expect("read"));

    let target = Store::open(&dir.path().join("target.sqlite3"), Wait::Command).expect("opens");
    let recording = Recording {
        source: ImportSource::Artifact("long".to_owned()),
        ts: ts(),
        author: importer(),
    };
    let outcome = import(&target, Cursor::new(&journal), recording).expect("imported");
    assert_eq!(
        (outcome.read, outcome.imported, outcome.malformed),
        (2, 1, 1)
    );
    assert!(!outcome.incomplete);
}

#[test]
fn every_line_of_a_journal_reads_as_a_line() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = source_store(dir.path());
    let journals = dir.path().join("journals");
    written(checkpoint(&store, RepoClass::Own, &journals, WAIT).expect("written"));
    let bytes = std::fs::read(journals.join("a1.jsonl")).expect("read");
    assert!(lines(&bytes).all(|l| l.complete && !l.is_blank()));
}
