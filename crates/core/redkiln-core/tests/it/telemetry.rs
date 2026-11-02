//! Telemetry never waits (SPEC EV-14, ST-09): dropped and counted on a busy lock.

use std::num::NonZeroUsize;
use std::path::Path;
use std::time::Instant;

use happenstance_sqlite::rusqlite::Connection;
use redkiln_core::envelope::{Actor, Author, Class, Envelope, Payload, Timestamp, Writer};
use redkiln_core::store::{Filter, NewEvent, Recorded, Store, StoreError, TelemetryEvent, Wait};
use redkiln_core::trust::Trust;
use serde::{Deserialize, Serialize};

use crate::support::listing;

#[derive(Debug, Serialize, Deserialize)]
struct ToolTimed {
    millis: u64,
}

impl Payload for ToolTimed {
    const TYPE: &'static str = "core.tool_timed";
    const VERSION: u32 = 1;
    const CLASS: Class = Class::Telemetry;
}

fn timing(millis: u64) -> TelemetryEvent {
    let author = Author {
        actor: Actor::Hook,
        trust: Trust::asserted(),
        writer: Writer::new("weighin", "2.0.0").expect("valid writer"),
    };
    Envelope::new(
        Timestamp::parse("2026-10-31T04:20:11Z").expect("valid ts"),
        author,
        ToolTimed { millis },
    )
    .to_telemetry([])
    .expect("a telemetry type")
}

fn count(store: &Store) -> usize {
    store
        .read(&Filter::all(), 0, NonZeroUsize::new(100).expect("non-zero"))
        .expect("read")
        .len()
}

/// Another process's connection, under the store's TRUNCATE journal.
fn other_connection(path: &Path) -> Connection {
    let conn = Connection::open(path).expect("open");
    conn.query_row("PRAGMA journal_mode = truncate", [], |r| {
        r.get::<_, String>(0)
    })
    .expect("truncate");
    conn
}

#[test]
fn a_telemetry_write_is_dropped_and_counted_while_another_writer_holds_the_lock() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("events.sqlite3");
    // A command's store: its durable appends would wait 10 seconds.
    let mut store = Store::open(&path, Wait::Command).expect("opens");
    let holder = other_connection(&path);
    holder
        .execute_batch("BEGIN IMMEDIATE")
        .expect("take the write lock");

    let started = Instant::now();
    assert_eq!(
        store.record_telemetry(timing(1)).expect("no error"),
        Recorded::Dropped
    );
    assert_eq!(
        store.record_telemetry(timing(2)).expect("no error"),
        Recorded::Dropped
    );
    assert!(
        started.elapsed() < Wait::Hook.timeout(),
        "telemetry never waits for the lock: {:?}",
        started.elapsed()
    );
    assert_eq!(
        store.telemetry_dropped(),
        2,
        "each drop is counted, none retried"
    );

    holder.execute_batch("ROLLBACK").expect("release the lock");
    assert_eq!(
        store.record_telemetry(timing(3)).expect("written"),
        Recorded::At(1)
    );
    assert_eq!(count(&store), 1, "the dropped events were never written");
    assert_eq!(store.telemetry_dropped(), 2);
}

#[test]
fn a_reader_holding_the_file_at_commit_drops_the_event_and_writes_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("events.sqlite3");
    let mut store = Store::open(&path, Wait::Command).expect("opens");
    let reader = other_connection(&path);
    // A read transaction keeps its shared lock, so a commit cannot take the file.
    reader
        .execute_batch("BEGIN; SELECT count(*) FROM event;")
        .expect("hold a read lock");

    assert_eq!(
        store.record_telemetry(timing(1)).expect("no error"),
        Recorded::Dropped
    );
    assert_eq!(store.telemetry_dropped(), 1);

    reader
        .execute_batch("COMMIT")
        .expect("release the read lock");
    assert_eq!(count(&store), 0, "the busy commit was rolled back");
    assert_eq!(
        store.record_telemetry(timing(2)).expect("written"),
        Recorded::At(1)
    );
    assert_eq!(count(&store), 1);
}

#[test]
fn a_telemetry_connection_opened_while_the_lock_is_busy_is_opened_again_later() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("events.sqlite3");
    let mut store = Store::open(&path, Wait::Command).expect("opens");
    // Another opener holds Core's open lock (it is creating or converting the store).
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .open(dir.path().join("events.sqlite3.lock"))
        .expect("the open lock file");
    lock.lock().expect("hold the open lock");

    assert_eq!(
        store.record_telemetry(timing(1)).expect("no error"),
        Recorded::Dropped
    );
    lock.unlock().expect("release the open lock");
    assert_eq!(
        store.record_telemetry(timing(2)).expect("written"),
        Recorded::At(1)
    );
    assert_eq!(store.telemetry_dropped(), 1);
}

/// A durable event, so the store holds something of its own before telemetry is written.
fn durable() -> NewEvent {
    NewEvent::new("weighin.item_raised", br#"{"q":"?"}"#.to_vec()).expect("a valid type")
}

/// The events in the file at `path`, counted through a connection of its own.
fn rows(path: &Path) -> i64 {
    other_connection(path)
        .query_row("SELECT count(*) FROM event", [], |r| r.get(0))
        .expect("count")
}

#[test]
fn telemetry_is_refused_by_a_store_whose_identity_changed_in_place() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("events.sqlite3");
    let mut store = Store::open(&path, Wait::Command).expect("opens");
    store.append(vec![durable()], None).expect("appended");
    // A restore of another store's bytes, written in place: the file keeps its path, not its
    // identity. Windows cannot replace a file this process holds open, so this is the form of
    // a replacement every platform can test.
    other_connection(&path)
        .execute(
            "UPDATE store_meta SET v = randomblob(16) WHERE k = 'store_id'",
            [],
        )
        .expect("re-mint in place");

    let refused = store.record_telemetry(timing(1));
    match refused {
        Err(StoreError::IdentityChanged { opened, found }) => {
            assert_eq!(opened, store.store_id());
            assert_ne!(found, opened);
        }
        other => panic!("expected IdentityChanged, got {other:?}"),
    }
    assert_eq!(rows(&path), 1, "only the durable event; no telemetry row");
    assert_eq!(store.telemetry_dropped(), 0, "a refusal is not a drop");
}

/// Renaming over a file another handle holds open is refused on Windows, and the store holds
/// its file open; the in-place test above covers the guard there.
#[cfg(unix)]
#[test]
fn telemetry_is_refused_by_another_store_renamed_over_the_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("events.sqlite3");
    let mut store = Store::open(&path, Wait::Command).expect("opens");
    store.append(vec![durable()], None).expect("appended");
    let elsewhere = tempfile::tempdir().expect("tempdir");
    let other = elsewhere.path().join("events.sqlite3");
    drop(Store::open(&other, Wait::Command).expect("another store"));
    let copy = dir.path().join("copy.sqlite3");
    std::fs::copy(&other, &copy).expect("copy it beside the store");
    std::fs::rename(&copy, &path).expect("rename it over the store");

    let refused = store.record_telemetry(timing(1));
    match refused {
        Err(StoreError::IdentityChanged { opened, found }) => {
            assert_eq!(opened, store.store_id());
            assert_ne!(found, opened);
        }
        other => panic!("expected IdentityChanged, got {other:?}"),
    }
    assert_eq!(rows(&path), 0, "no telemetry row in the replacing file");
}

/// Deleting a file another handle holds open is refused on Windows.
#[cfg(unix)]
#[test]
fn telemetry_never_creates_a_store_where_the_opened_one_vanished() {
    use redkiln_core::store::OpenError;

    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("events.sqlite3");
    let mut store = Store::open(&path, Wait::Command).expect("opens");
    std::fs::remove_file(&path).expect("the store vanishes");
    let before = listing(dir.path());

    let refused = store.record_telemetry(timing(1));
    assert!(
        matches!(
            refused,
            Err(StoreError::Open {
                source: OpenError::NoStore,
                ..
            })
        ),
        "{refused:?}"
    );
    assert!(!path.exists(), "no new store at the path");
    assert_eq!(
        listing(dir.path()),
        before,
        "nothing created or set aside beside it"
    );
}

/// nextest runs each test in a process of its own; under `cargo test` this test changes the
/// working directory of the whole binary for its duration, which no other test here reads.
#[test]
fn the_telemetry_connection_opens_the_store_whatever_the_working_directory_becomes() {
    let first = tempfile::tempdir().expect("tempdir");
    let second = tempfile::tempdir().expect("tempdir");
    let original = std::env::current_dir().expect("working directory");
    std::env::set_current_dir(first.path()).expect("enter the first");
    let opened = Store::open(Path::new("events.sqlite3"), Wait::Command);
    std::env::set_current_dir(second.path()).expect("enter the second");
    let recorded = opened.map(|mut store| (store.record_telemetry(timing(1)), store));
    // Left before any assertion, so neither directory is in use when it is removed.
    std::env::set_current_dir(original).expect("restore the working directory");

    let (recorded, store) = recorded.expect("opens");
    assert_eq!(recorded.expect("written"), Recorded::At(1));
    assert_eq!(count(&store), 1);
    assert_eq!(rows(&first.path().join("events.sqlite3")), 1);
    assert!(
        listing(second.path()).is_empty(),
        "nothing at the relative path in the new working directory"
    );
}
