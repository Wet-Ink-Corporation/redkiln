//! Opening and writing the store (SPEC ST-04 to ST-11, ID-01).

use std::num::NonZeroUsize;
use std::path::Path;

use happenstance_sqlite::event_store::SqliteEventStore;
use happenstance_sqlite::rusqlite::Connection;
use redkiln_core::store::{
    Appended, Condition, Filter, NewEvent, OpenError, STORE_SCHEMA, Store, StoreError, Tag, Wait,
};

use crate::support::{
    assert_truncate_layout, change_counter, header_user_version, in_wal, listing, raw_store_id,
    sibling,
};

const RAISED: &str = "weighin.item_raised";

fn item(id: &str) -> Tag {
    Tag::new("item", id).expect("a valid tag")
}

fn raise(id: &str) -> NewEvent {
    NewEvent::new(RAISED, br#"{"q":"?"}"#.to_vec())
        .expect("a valid type")
        .with_tag(item(id))
}

fn open(path: &Path) -> Store {
    Store::open(path, Wait::Command).expect("the store opens")
}

fn all(store: &Store) -> Vec<redkiln_core::store::Stored> {
    store
        .read(
            &Filter::all(),
            0,
            NonZeroUsize::new(1000).expect("non-zero"),
        )
        .expect("read")
}

fn refusal(path: &Path) -> OpenError {
    match Store::open(path, Wait::Command) {
        Err(StoreError::Open { source, .. }) => source,
        other => panic!("expected an open refusal, got {other:?}"),
    }
}

#[test]
fn a_new_store_is_stamped_runs_under_truncate_and_reopening_writes_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("redkiln/events.sqlite3");
    let store = open(&path);
    assert_eq!(
        store.append(vec![raise("wi-a")], None).expect("append"),
        Appended::At(1)
    );
    let id = store.store_id();
    drop(store);
    assert_eq!(i64::from(header_user_version(&path)), STORE_SCHEMA);
    assert_truncate_layout(&path);

    let before = change_counter(&path);
    let store = open(&path);
    assert_eq!(store.store_id(), id, "the store keeps its identity");
    let events = all(&store);
    drop(store);
    assert_eq!(
        change_counter(&path),
        before,
        "a current store is not written on open"
    );
    assert_eq!(events.len(), 1);
    let event = events.first().expect("one event");
    assert_eq!(event.event_type, RAISED);
    assert_eq!(event.id.store(), id);
    assert_eq!(event.tags, vec![item("wi-a")]);
    assert_truncate_layout(&path);
}

#[test]
fn concurrent_creators_share_one_schema_and_identity() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("events.sqlite3");
    let ids: Vec<_> = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for n in 0..6 {
            let path = &path;
            let handle = std::thread::Builder::new()
                .name(format!("creator-{n}"))
                .spawn_scoped(scope, move || {
                    let store = open(path);
                    let appended = store
                        .append(vec![raise(&format!("wi-{n}"))], None)
                        .expect("append");
                    assert!(matches!(appended, Appended::At(_)), "{appended:?}");
                    store.store_id()
                })
                .expect("spawn a creator");
            handles.push(handle);
        }
        handles
            .into_iter()
            .map(|h| h.join().expect("creator thread"))
            .collect()
    });
    assert!(
        ids.windows(2).all(|w| w[0] == w[1]),
        "one identity: {ids:?}"
    );
    assert_eq!(all(&open(&path)).len(), 6);
    let orphans: Vec<_> = listing(dir.path())
        .into_iter()
        .filter(|n| n.contains(".orphaned-"))
        .collect();
    assert!(
        orphans.is_empty(),
        "no creator's live journal is set aside: {orphans:?}"
    );
}

#[test]
fn an_open_waits_for_a_creation_in_progress_and_leaves_its_journal() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("events.sqlite3");
    // Another Core opener midway through creating the store: it holds the open lock, and its
    // creation transaction has a live journal beside a file with no pages yet.
    let lock = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(sibling(&path, ".lock"))
        .expect("the lock file");
    lock.lock().expect("hold the open lock");
    let creator = Connection::open(&path).expect("connect");
    creator
        .execute_batch("PRAGMA journal_mode = truncate; BEGIN IMMEDIATE; CREATE TABLE t (x);")
        .expect("a creation in progress");
    let journal = sibling(&path, "-journal");
    assert!(journal.exists(), "the creation has a live journal");

    match Store::open(&path, Wait::Hook) {
        Err(StoreError::Open {
            source: OpenError::LockBusy,
            ..
        }) => {}
        other => panic!("expected the open to wait for the lock, got {other:?}"),
    }
    assert!(journal.exists(), "the live journal keeps its name");
    assert!(
        !listing(dir.path()).iter().any(|n| n.contains(".orphaned-")),
        "nothing is set aside while a creation is in progress"
    );
    creator.execute_batch("ROLLBACK").expect("roll back");
}

#[test]
fn conditions_make_the_first_writer_win() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = open(&dir.path().join("events.sqlite3"));
    let not_yet = Condition::not_yet(RAISED, item("wi-a")).expect("condition");
    assert_eq!(
        store
            .append(vec![raise("wi-a")], Some(&not_yet))
            .expect("append"),
        Appended::At(1)
    );
    assert_eq!(
        store
            .append(vec![raise("wi-a")], Some(&not_yet))
            .expect("append"),
        Appended::Conflict,
        "a second raise of the same item conflicts"
    );
    assert_eq!(
        store
            .append(
                vec![raise("wi-b")],
                Some(&Condition::not_yet(RAISED, item("wi-b")).expect("condition"))
            )
            .expect("append"),
        Appended::At(2),
        "another item is not held back"
    );

    // Two writers that both read the store at position 1 decide about wi-a: one wins.
    let since = Condition::unchanged_since(item("wi-a"), 1);
    let decided = || {
        NewEvent::new("weighin.item_decided", b"{}".to_vec())
            .expect("type")
            .with_tag(item("wi-a"))
    };
    assert_eq!(
        store.append(vec![decided()], Some(&since)).expect("append"),
        Appended::At(3)
    );
    assert_eq!(
        store.append(vec![decided()], Some(&since)).expect("append"),
        Appended::Conflict
    );
}

#[test]
fn reads_page_after_a_position() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = open(&dir.path().join("events.sqlite3"));
    for n in 0..5 {
        store
            .append(vec![raise(&format!("wi-{n}"))], None)
            .expect("append");
    }
    let two = NonZeroUsize::new(2).expect("non-zero");
    let page: Vec<u64> = store
        .read(&Filter::all(), 2, two)
        .expect("read")
        .iter()
        .map(|e| e.position)
        .collect();
    assert_eq!(page, vec![3, 4]);
    let tagged = store
        .read(&Filter::all().tagged(item("wi-4")), 0, two)
        .expect("read");
    assert_eq!(
        tagged.iter().map(|e| e.position).collect::<Vec<_>>(),
        vec![5]
    );
    assert_eq!(
        store.read(&Filter::all(), u64::MAX, two).expect("read"),
        vec![]
    );
}

#[test]
fn a_wal_store_converts_to_truncate_and_keeps_its_events() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("events.sqlite3");
    let id = {
        let store = open(&path);
        store.append(vec![raise("wi-a")], None).expect("append");
        store.store_id()
    };
    {
        let conn = Connection::open(&path).expect("open");
        let mode: String = conn
            .query_row("PRAGMA journal_mode = wal", [], |r| r.get(0))
            .expect("wal");
        assert_eq!(mode, "wal");
    }
    assert!(in_wal(&path), "the fixture is in WAL");
    let store = open(&path);
    assert_eq!(store.store_id(), id);
    assert_eq!(all(&store).len(), 1);
    drop(store);
    assert!(!in_wal(&path), "converted");
    assert_truncate_layout(&path);
}

#[test]
fn a_stale_wal_beside_a_truncate_store_is_set_aside_not_deleted() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("events.sqlite3");
    drop(open(&path));
    std::fs::write(sibling(&path, "-wal"), b"stale frames").expect("write");
    let store = open(&path);
    assert_eq!(all(&store).len(), 0);
    drop(store);
    assert_truncate_layout(&path);
    let orphans = listing(dir.path())
        .into_iter()
        .filter(|n| n.contains("-wal.orphaned-"))
        .count();
    assert_eq!(orphans, 1, "the stale WAL is renamed aside");
}

#[test]
fn an_empty_file_with_a_failed_wal_beside_it_becomes_a_store() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("events.sqlite3");
    std::fs::write(&path, b"").expect("write");
    std::fs::write(sibling(&path, "-wal"), b"not a wal").expect("write");
    let store = open(&path);
    assert_eq!(
        store.append(vec![raise("wi-a")], None).expect("append"),
        Appended::At(1)
    );
    drop(store);
    assert_truncate_layout(&path);
}

fn assert_refused_untouched(path: &Path, check: impl Fn(&OpenError) -> bool) {
    let dir = path.parent().expect("a parent");
    let (bytes, names) = (std::fs::read(path).expect("read"), listing(dir));
    let err = refusal(path);
    assert!(check(&err), "unexpected refusal: {err:?}");
    assert_eq!(
        std::fs::read(path).expect("read"),
        bytes,
        "the file is unchanged"
    );
    assert_eq!(
        listing(dir),
        names,
        "nothing was added or removed beside it"
    );
}

#[test]
fn a_newer_major_is_refused_and_a_newer_minor_opens_with_its_stamp_kept() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("events.sqlite3");
    drop(open(&path));
    let stamp = |v: i64| {
        let conn = Connection::open(&path).expect("open");
        conn.query_row("PRAGMA journal_mode = truncate", [], |r| {
            r.get::<_, String>(0)
        })
        .expect("truncate");
        conn.execute_batch(&format!("PRAGMA user_version = {v}"))
            .expect("stamp");
    };
    stamp(1003);
    let store = open(&path);
    store
        .append(vec![raise("wi-a")], None)
        .expect("a newer minor appends");
    drop(store);
    assert_eq!(
        header_user_version(&path),
        1003,
        "the stamp is never lowered"
    );

    stamp(2000);
    assert_refused_untouched(&path, |e| {
        matches!(
            e,
            OpenError::NewerMajor {
                found: 2000,
                supported: 1000
            }
        )
    });
}

#[test]
fn a_database_that_is_not_a_store_is_refused_untouched() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("other.sqlite3");
    {
        let conn = Connection::open(&path).expect("open");
        conn.execute_batch("PRAGMA journal_mode = truncate; CREATE TABLE notes (t TEXT);")
            .expect("foreign schema");
    }
    assert_refused_untouched(&path, |e| matches!(e, OpenError::NotAStore));
}

#[test]
fn an_old_ledger_is_refused_untouched_whatever_its_journal() {
    let dir = tempfile::tempdir().expect("tempdir");
    let stamped = crate::support::fixture("weighin-1.2.0.sqlite3", dir.path());
    assert_refused_untouched(&stamped, |e| matches!(e, OpenError::OldLedger { found: 1 }));
    // Unstamped: the header cannot tell a 0.x ledger from another program's database.
    let truncate = crate::support::fixture("weighin-0.2.2.sqlite3", dir.path());
    assert_refused_untouched(&truncate, |e| matches!(e, OpenError::NotAStore));
    let wal = crate::support::fixture("weighin-0.1.x.sqlite3", dir.path());
    assert!(in_wal(&wal));
    assert_refused_untouched(&wal, |e| matches!(e, OpenError::NotAStore));
    // A stale -wal beside a refused ledger stays where it is.
    std::fs::write(sibling(&truncate, "-wal"), b"stale").expect("write");
    assert_refused_untouched(&truncate, |e| matches!(e, OpenError::NotAStore));
    assert!(sibling(&truncate, "-wal").exists());
}

#[test]
fn a_store_copied_to_another_path_gets_a_new_identity_and_keeps_its_events() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("a/events.sqlite3");
    let original = {
        let store = open(&path);
        store.append(vec![raise("wi-a")], None).expect("append");
        store.store_id()
    };
    let copy = dir.path().join("b/events.sqlite3");
    std::fs::create_dir_all(copy.parent().expect("parent")).expect("mkdir");
    std::fs::copy(&path, &copy).expect("copy");
    let raw_before = raw_store_id(&copy);

    let store = open(&copy);
    assert_ne!(store.store_id(), original, "the copy re-mints its identity");
    let events = all(&store);
    assert_eq!(
        events.first().map(|e| e.id.store()),
        Some(original),
        "old events keep their origin"
    );
    let appended = store.append(vec![raise("wi-b")], None).expect("append");
    assert_eq!(appended, Appended::At(2));
    let fresh = store.store_id();
    drop(store);
    assert_ne!(raw_store_id(&copy), raw_before);

    let again = open(&copy);
    assert_eq!(
        again.store_id(),
        fresh,
        "reopening the copy where it now is keeps its identity"
    );
    assert_eq!(
        open(&path).store_id(),
        original,
        "the original is unaffected"
    );
}

#[test]
fn a_path_with_no_absolute_form_is_refused_before_anything_is_opened() {
    let err = refusal(Path::new(""));
    assert!(matches!(err, OpenError::AbsolutePath(_)), "{err:?}");
}

#[test]
fn the_store_refuses_to_run_inside_an_async_runtime() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("events.sqlite3");
    let store = open(&path);
    let runtime = tokio::runtime::Builder::new_current_thread()
        .build()
        .expect("runtime");
    let inside = runtime.block_on(async { store.append(vec![raise("wi-a")], None) });
    assert!(
        matches!(inside, Err(StoreError::InsideRuntime)),
        "{inside:?}"
    );
    let opened = runtime.block_on(async { Store::open(&path, Wait::Hook) });
    assert!(
        matches!(opened, Err(StoreError::InsideRuntime)),
        "{opened:?}"
    );
}

#[test]
fn happenstance_own_open_would_have_switched_to_wal() {
    // The reason Core never opens the store through happenstance (ST-05).
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("plain.sqlite3");
    drop(SqliteEventStore::open(&path).expect("happenstance opens"));
    assert!(in_wal(&path));
}

#[test]
fn concurrent_first_opens_of_a_copy_re_mint_once_and_all_append() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("a/events.sqlite3");
    let original = {
        let store = open(&path);
        store.append(vec![raise("wi-a")], None).expect("append");
        store.store_id()
    };
    let copy = dir.path().join("b/events.sqlite3");
    std::fs::create_dir_all(copy.parent().expect("parent")).expect("mkdir");
    std::fs::copy(&path, &copy).expect("copy");

    let ids: Vec<_> = std::thread::scope(|scope| {
        let mut handles = Vec::new();
        for n in 0..8 {
            let copy = &copy;
            let handle = std::thread::Builder::new()
                .name(format!("opener-{n}"))
                .spawn_scoped(scope, move || {
                    let store = open(copy);
                    let appended = store
                        .append(vec![raise(&format!("wi-{n}"))], None)
                        .expect("every opener appends");
                    assert!(matches!(appended, Appended::At(_)), "{appended:?}");
                    store.store_id()
                })
                .expect("spawn an opener");
            handles.push(handle);
        }
        handles
            .into_iter()
            .map(|h| h.join().expect("opener thread"))
            .collect()
    });
    assert!(ids.windows(2).all(|w| w[0] == w[1]), "one re-mint: {ids:?}");
    assert_ne!(ids.first().copied(), Some(original));
    assert_eq!(all(&open(&copy)).len(), 9);
}

#[test]
fn a_file_that_is_not_sqlite_is_refused_with_its_wal_left_in_place() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("notes.txt");
    std::fs::write(&path, "a mistyped --store\n".repeat(20)).expect("write");
    std::fs::write(sibling(&path, "-wal"), b"someone's").expect("write");
    assert_refused_untouched(&path, |e| matches!(e, OpenError::NotAStore));
    let tiny = dir.path().join("tiny");
    std::fs::write(&tiny, b"x").expect("write");
    assert_refused_untouched(&tiny, |e| matches!(e, OpenError::NotAStore));
}

#[test]
fn a_journal_left_by_a_failed_creation_is_set_aside_not_deleted() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("events.sqlite3");
    std::fs::write(&path, b"").expect("write");
    std::fs::write(sibling(&path, "-journal"), b"").expect("write");
    let store = open(&path);
    assert_eq!(
        store.append(vec![raise("wi-a")], None).expect("append"),
        Appended::At(1)
    );
    drop(store);
    assert_truncate_layout(&path);
    let orphans = listing(dir.path())
        .into_iter()
        .filter(|n| n.contains("-journal.orphaned-"))
        .count();
    assert_eq!(orphans, 1, "the leftover journal is renamed aside");
}
