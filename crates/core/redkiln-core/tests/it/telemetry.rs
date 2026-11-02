//! Telemetry never waits (SPEC EV-14, ST-09): dropped and counted on a busy lock. It is kept in
//! a side store beside the log, `<store>.telemetry`, for 30 days, and never read past them.

use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use happenstance_sqlite::rusqlite::Connection;
use redkiln_core::envelope::{
    Actor, Author, Class, Envelope, Namespace, Payload, Timestamp, Writer,
};
use redkiln_core::store::{
    Filter, NewEvent, Page, Recorded, Store, StoreError, TELEMETRY_WINDOW, Tag, TelemetryEvent,
    Wait,
};
use redkiln_core::trust::Trust;
use serde::{Deserialize, Serialize};

use crate::support::{assert_truncate_layout, header_user_version, listing, sibling};

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

/// The telemetry a reader sees now: the positions in the side store, within the window.
fn telemetry(store: &Store) -> Vec<u64> {
    telemetry_at(store, SystemTime::now(), 0, 100)
}

fn telemetry_at(store: &Store, now: SystemTime, after: u64, limit: usize) -> Vec<u64> {
    store
        .read_telemetry(
            &Filter::all(),
            Page {
                after,
                limit: NonZeroUsize::new(limit).expect("non-zero"),
            },
            now,
        )
        .expect("read telemetry")
        .iter()
        .map(|stored| stored.position)
        .collect()
}

/// The side store beside the store at `path`.
fn side(path: &Path) -> PathBuf {
    sibling(path, ".telemetry")
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
    assert_eq!(
        store
            .record_telemetry(timing(0), SystemTime::now())
            .expect("written"),
        Recorded::At(1),
        "the side store exists"
    );
    // Telemetry's lock is the side store's: another telemetry writer holds it.
    let holder = other_connection(&side(&path));
    holder
        .execute_batch("BEGIN IMMEDIATE")
        .expect("take the write lock");

    let started = Instant::now();
    assert_eq!(
        store
            .record_telemetry(timing(1), SystemTime::now())
            .expect("no error"),
        Recorded::Dropped
    );
    assert_eq!(
        store
            .record_telemetry(timing(2), SystemTime::now())
            .expect("no error"),
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
        store
            .record_telemetry(timing(3), SystemTime::now())
            .expect("written"),
        Recorded::At(2)
    );
    assert_eq!(
        telemetry(&store),
        [1, 2],
        "the dropped events were never written"
    );
    assert_eq!(store.telemetry_dropped(), 2);
}

#[test]
fn the_write_that_would_create_the_side_store_drops_while_its_lock_is_held() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("events.sqlite3");
    let mut store = Store::open(&path, Wait::Command).expect("opens");
    let holder = other_connection(&side(&path));
    holder
        .execute_batch("BEGIN IMMEDIATE")
        .expect("take the write lock");

    let started = Instant::now();
    assert_eq!(
        store
            .record_telemetry(timing(1), SystemTime::now())
            .expect("no error"),
        Recorded::Dropped
    );
    assert!(
        started.elapsed() < Wait::Hook.timeout(),
        "{:?}",
        started.elapsed()
    );
    holder.execute_batch("ROLLBACK").expect("release the lock");
    assert_eq!(
        store
            .record_telemetry(timing(2), SystemTime::now())
            .expect("written"),
        Recorded::At(1)
    );
    assert_eq!(store.telemetry_dropped(), 1);
}

#[test]
fn a_reader_holding_the_file_at_commit_drops_the_event_and_writes_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("events.sqlite3");
    let mut store = Store::open(&path, Wait::Command).expect("opens");
    assert_eq!(
        store
            .record_telemetry(timing(0), SystemTime::now())
            .expect("written"),
        Recorded::At(1),
        "the side store exists"
    );
    let reader = other_connection(&side(&path));
    // A read transaction keeps its shared lock, so a commit cannot take the file.
    reader
        .execute_batch("BEGIN; SELECT count(*) FROM event;")
        .expect("hold a read lock");

    assert_eq!(
        store
            .record_telemetry(timing(1), SystemTime::now())
            .expect("no error"),
        Recorded::Dropped
    );
    assert_eq!(store.telemetry_dropped(), 1);

    reader
        .execute_batch("COMMIT")
        .expect("release the read lock");
    assert_eq!(telemetry(&store), [1], "the busy commit was rolled back");
    assert_eq!(
        store
            .record_telemetry(timing(2), SystemTime::now())
            .expect("written"),
        Recorded::At(2)
    );
    assert_eq!(telemetry(&store), [1, 2]);
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
        store
            .record_telemetry(timing(1), SystemTime::now())
            .expect("no error"),
        Recorded::Dropped
    );
    lock.unlock().expect("release the open lock");
    assert_eq!(
        store
            .record_telemetry(timing(2), SystemTime::now())
            .expect("written"),
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

    let refused = store.record_telemetry(timing(1), SystemTime::now());
    match refused {
        Err(StoreError::IdentityChanged { opened, found }) => {
            assert_eq!(opened, store.store_id());
            assert_ne!(found, opened);
        }
        other => panic!("expected IdentityChanged, got {other:?}"),
    }
    assert_eq!(rows(&path), 1, "only the durable event; no telemetry row");
    assert!(!side(&path).exists(), "no side store made beside it");
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

    let refused = store.record_telemetry(timing(1), SystemTime::now());
    match refused {
        Err(StoreError::IdentityChanged { opened, found }) => {
            assert_eq!(opened, store.store_id());
            assert_ne!(found, opened);
        }
        other => panic!("expected IdentityChanged, got {other:?}"),
    }
    assert_eq!(rows(&path), 0, "no telemetry row in the replacing file");
    assert!(!side(&path).exists(), "no side store made beside it");
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

    let refused = store.record_telemetry(timing(1), SystemTime::now());
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

#[test]
fn telemetry_is_kept_in_a_side_store_and_never_in_the_log() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("events.sqlite3");
    let mut store = Store::open(&path, Wait::Command).expect("opens");
    store.append(vec![durable()], None).expect("appended");
    assert_eq!(
        store
            .record_telemetry(timing(1), SystemTime::now())
            .expect("written"),
        Recorded::At(1),
        "the side store has positions of its own"
    );

    assert_eq!(count(&store), 1, "the log holds the durable event only");
    assert_eq!(rows(&path), 1);
    assert_eq!(telemetry(&store), [1]);
    let side = side(&path);
    assert_eq!(header_user_version(&side), 1000, "a Core store of its own");
    assert_truncate_layout(&side);
    assert!(
        sibling(&side, ".lock").exists(),
        "with its own open lock beside it"
    );
}

#[test]
fn a_store_with_no_telemetry_reads_none_and_creates_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("events.sqlite3");
    let store = Store::open(&path, Wait::Command).expect("opens");
    let before = listing(dir.path());
    assert!(telemetry(&store).is_empty());
    assert_eq!(listing(dir.path()), before, "no side store made for a read");
}

const HOUR_MILLIS: i64 = 60 * 60 * 1000;
const DAY_MILLIS: i64 = 24 * HOUR_MILLIS;

/// Moves the side store's rows at `positions` back by `millis`, as if recorded then.
fn backdate(path: &Path, positions: &[u64], millis: i64) {
    let conn = other_connection(&side(path));
    for position in positions {
        let position = i64::try_from(*position).expect("small");
        conn.execute(
            "UPDATE event SET recorded_at = recorded_at - ?1 WHERE position = ?2",
            [millis, position],
        )
        .expect("backdate");
    }
}

/// The side store's positions, its `event_tag` positions and its `tag_cardinality` rows, read
/// raw, so a prune that left anything behind shows.
fn raw_side(path: &Path) -> (Vec<i64>, Vec<i64>, Vec<(String, i64)>) {
    let conn = other_connection(&side(path));
    let column = |sql: &str| -> Vec<i64> {
        let mut stmt = conn.prepare(sql).expect("prepare");
        stmt.query_map([], |r| r.get(0))
            .expect("query")
            .collect::<Result<_, _>>()
            .expect("rows")
    };
    let positions = column("SELECT position FROM event ORDER BY position");
    let tagged = column("SELECT position FROM event_tag ORDER BY position");
    let mut stmt = conn
        .prepare("SELECT tag, events FROM tag_cardinality ORDER BY tag")
        .expect("prepare");
    let counts = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .expect("query")
        .collect::<Result<_, _>>()
        .expect("rows");
    (positions, tagged, counts)
}

fn tagged_timing(millis: u64) -> TelemetryEvent {
    channel_timing(millis, "cli")
}

/// A timing tagged `weighin.channel:<channel>`.
fn channel_timing(millis: u64, channel: &str) -> TelemetryEvent {
    let author = Author {
        actor: Actor::Hook,
        trust: Trust::asserted(),
        writer: Writer::new("weighin", "2.0.0").expect("valid writer"),
    };
    let tag = Tag::namespaced(Namespace::Weighin, "channel", channel).expect("a tag");
    Envelope::new(
        Timestamp::parse("2026-10-31T04:20:11Z").expect("valid ts"),
        author,
        ToolTimed { millis },
    )
    .to_telemetry([tag])
    .expect("a telemetry type")
}

#[test]
fn telemetry_recorded_before_the_window_is_never_read() {
    assert_eq!(TELEMETRY_WINDOW, Duration::from_hours(30 * 24), "30 days");
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("events.sqlite3");
    let mut store = Store::open(&path, Wait::Command).expect("opens");
    for n in 1..=3 {
        store
            .record_telemetry(timing(n), SystemTime::now())
            .expect("written");
    }
    // Past the window by an hour: within the day of slack before it is deleted.
    backdate(&path, &[1], 30 * DAY_MILLIS + HOUR_MILLIS);

    let now = SystemTime::now();
    assert_eq!(telemetry_at(&store, now, 0, 100), [2, 3]);
    assert_eq!(
        telemetry_at(&store, now, 0, 1),
        [2],
        "a page is filled from inside the window"
    );
    // Two hours back: the hour-old row is in that reader's window, and rows two hours ahead of
    // it are within the day of skew it accepts.
    let earlier = now
        .checked_sub(Duration::from_hours(2))
        .expect("two hours ago");
    assert_eq!(
        telemetry_at(&store, earlier, 0, 100),
        [1, 2, 3],
        "the window is measured from the reader's now"
    );
    let far_behind = now
        .checked_sub(Duration::from_hours(2 * 24))
        .expect("two days ago");
    assert_eq!(
        telemetry_at(&store, far_behind, 0, 100),
        [1],
        "rows dated more than a day past the reader's now are outside its window"
    );

    store
        .record_telemetry(timing(4), SystemTime::now())
        .expect("written");
    assert_eq!(
        raw_side(&path).0,
        [1, 2, 3, 4],
        "within the slack, a row is not deleted yet"
    );
    assert_eq!(telemetry_at(&store, now, 0, 100), [2, 3, 4]);
}

#[test]
fn rows_past_the_window_are_deleted_from_the_side_store_with_their_tags() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("events.sqlite3");
    let mut store = Store::open(&path, Wait::Command).expect("opens");
    store.append(vec![durable()], None).expect("appended");
    for n in 1..=3 {
        store
            .record_telemetry(tagged_timing(n), SystemTime::now())
            .expect("written");
    }
    backdate(&path, &[1, 2], 32 * DAY_MILLIS);
    let (_, tagged, counts) = raw_side(&path);
    // Core's tags (`plugin`, `trust`) and the caller's, the same on every row.
    let per_row = tagged.len() / 3;
    assert_eq!(per_row, 3, "{tagged:?}");
    assert!(counts.iter().all(|(_, n)| *n == 3), "{counts:?}");

    assert_eq!(
        store
            .record_telemetry(tagged_timing(4), SystemTime::now())
            .expect("written"),
        Recorded::At(4),
        "positions are never reused"
    );
    let (positions, tagged, counts) = raw_side(&path);
    assert_eq!(positions, [3, 4], "the rows past the window are gone");
    assert_eq!(tagged, [3, 3, 3, 4, 4, 4], "with their tag rows");
    assert!(
        counts.iter().all(|(_, n)| *n == 2),
        "and their counts: {counts:?}"
    );
    assert_eq!(telemetry(&store), [3, 4]);
    assert_eq!(count(&store), 1, "the log is untouched");
    assert_eq!(rows(&path), 1);
}

fn write(store: &mut Store, event: TelemetryEvent) -> Recorded {
    store
        .record_telemetry(event, SystemTime::now())
        .expect("no error")
}

/// A store with `n` telemetry rows written now.
fn with_rows(n: u64) -> (tempfile::TempDir, PathBuf, Store) {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("events.sqlite3");
    let mut store = Store::open(&path, Wait::Command).expect("opens");
    for millis in 1..=n {
        assert_eq!(write(&mut store, timing(millis)), Recorded::At(millis));
    }
    (dir, path, store)
}

#[test]
fn a_tag_only_pruned_rows_carried_loses_its_count() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("events.sqlite3");
    let mut store = Store::open(&path, Wait::Command).expect("opens");
    write(&mut store, channel_timing(1, "old"));
    write(&mut store, channel_timing(2, "old"));
    write(&mut store, channel_timing(3, "cli"));
    backdate(&path, &[1, 2], 32 * DAY_MILLIS);

    write(&mut store, channel_timing(4, "cli"));
    let (positions, _, counts) = raw_side(&path);
    assert_eq!(positions, [3, 4]);
    let tags: Vec<&str> = counts.iter().map(|(tag, _)| tag.as_str()).collect();
    assert!(
        !tags.contains(&"weighin.channel:old"),
        "no count left for a tag no row carries: {counts:?}"
    );
    assert!(tags.contains(&"weighin.channel:cli"), "{counts:?}");
}

#[test]
fn a_future_dated_first_row_does_not_stop_the_prune_and_is_never_read() {
    let (_dir, path, mut store) = with_rows(3);
    backdate(&path, &[1], -40 * DAY_MILLIS);
    backdate(&path, &[2], 32 * DAY_MILLIS);
    assert_eq!(
        telemetry(&store),
        [3],
        "neither the future row nor the old one"
    );

    write(&mut store, timing(4));
    assert_eq!(
        raw_side(&path).0,
        [3, 4],
        "the old row is deleted, and the future one with it"
    );
}

#[test]
fn a_future_dated_head_row_never_deletes_the_live_window() {
    let (_dir, path, mut store) = with_rows(3);
    backdate(&path, &[3], -40 * DAY_MILLIS);

    write(&mut store, timing(4));
    assert_eq!(
        raw_side(&path).0,
        [1, 2, 4],
        "the window is the writer's now, not the head row's"
    );
    assert_eq!(telemetry(&store), [1, 2, 4]);
}

#[test]
fn telemetry_is_written_while_another_writer_holds_the_logs_lock() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("events.sqlite3");
    let mut store = Store::open(&path, Wait::Command).expect("opens");
    let holder = other_connection(&path);
    holder
        .execute_batch("BEGIN IMMEDIATE")
        .expect("take the log's write lock");

    let started = Instant::now();
    assert_eq!(write(&mut store, timing(1)), Recorded::At(1));
    assert!(
        started.elapsed() < Wait::Hook.timeout(),
        "{:?}",
        started.elapsed()
    );
    holder.execute_batch("ROLLBACK").expect("release");
    assert_eq!(store.telemetry_dropped(), 0);
}

#[test]
fn a_prune_is_skipped_while_the_side_store_is_busy_and_runs_once_it_is_free() {
    let (_dir, path, mut store) = with_rows(3);
    backdate(&path, &[1], 32 * DAY_MILLIS);
    let holder = other_connection(&side(&path));
    holder
        .execute_batch("BEGIN IMMEDIATE")
        .expect("take the side store's write lock");

    let started = Instant::now();
    assert_eq!(write(&mut store, timing(4)), Recorded::Dropped);
    assert!(
        started.elapsed() < Wait::Hook.timeout(),
        "{:?}",
        started.elapsed()
    );
    assert_eq!(raw_side(&path).0, [1, 2, 3], "nothing deleted while busy");

    holder.execute_batch("ROLLBACK").expect("release");
    assert_eq!(write(&mut store, timing(5)), Recorded::At(4));
    assert_eq!(raw_side(&path).0, [2, 3, 4]);
}

#[test]
fn a_side_store_whose_identity_changed_in_place_is_not_pruned() {
    let (_dir, path, mut store) = with_rows(3);
    backdate(&path, &[1], 32 * DAY_MILLIS);
    other_connection(&side(&path))
        .execute(
            "UPDATE store_meta SET v = randomblob(16) WHERE k = 'store_id'",
            [],
        )
        .expect("re-mint in place");

    let refused = store.record_telemetry(timing(4), SystemTime::now());
    assert!(
        matches!(refused, Err(StoreError::Append(_))),
        "happenstance refuses the append to a moved identity: {refused:?}"
    );
    assert_eq!(
        raw_side(&path).0,
        [1, 2, 3],
        "rows are deleted only from the side store the handle opened"
    );
}
