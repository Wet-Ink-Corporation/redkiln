//! Telemetry never waits (SPEC EV-14, ST-09): dropped and counted on a busy lock.

use std::num::NonZeroUsize;
use std::path::Path;
use std::time::Instant;

use happenstance_sqlite::rusqlite::Connection;
use redkiln_core::envelope::{Actor, Author, Class, Envelope, Payload, Timestamp, Writer};
use redkiln_core::store::{Filter, Recorded, Store, TelemetryEvent, Wait};
use redkiln_core::trust::Trust;
use serde::{Deserialize, Serialize};

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
