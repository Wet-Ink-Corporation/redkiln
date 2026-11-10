//! A store written under happenstance schema 1 is migrated to schema 2 when Core opens it (SPEC
//! ST-08), against `core-store-hs1.sqlite3`: a Core store of three raised items, written by
//! weighin 1.2.0's Core on happenstance 0.4.5.

use std::num::NonZeroUsize;
use std::path::Path;

use happenstance_sqlite::event_store::{SCHEMA_VERSION, SqliteEventStoreError};
use happenstance_sqlite::rusqlite::Connection;
use redkiln_core::store::{Filter, OpenError, STORE_SCHEMA, Store, StoreError, Wait};

use crate::support::{
    assert_truncate_layout, change_counter, fixture, header_user_version, in_wal, listing,
    raw_store_id,
};

const HS1: &str = "core-store-hs1.sqlite3";
const RAISED: &str = "weighin.item_raised";

/// happenstance's own schema version, its `store_meta` row, read through a TRUNCATE connection.
fn happenstance_schema(path: &Path) -> i64 {
    let conn = Connection::open(path).expect("open");
    conn.query_row("PRAGMA journal_mode = truncate", [], |r| {
        r.get::<_, String>(0)
    })
    .expect("truncate");
    conn.query_row(
        "SELECT CAST(v AS INTEGER) FROM store_meta WHERE k = 'schema_version'",
        [],
        |r| r.get(0),
    )
    .expect("the schema version row")
}

fn event_types(store: &Store) -> Vec<String> {
    store
        .read(&Filter::all(), 0, NonZeroUsize::new(100).expect("non-zero"))
        .expect("read")
        .into_iter()
        .map(|e| e.event_type)
        .collect()
}

fn positions(store: &Store) -> Vec<u64> {
    store
        .read(&Filter::all(), 0, NonZeroUsize::new(100).expect("non-zero"))
        .expect("read")
        .into_iter()
        .map(|e| e.position)
        .collect()
}

#[test]
fn the_fixture_is_at_happenstance_schema_1() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = fixture(HS1, dir.path());
    assert_eq!(happenstance_schema(&path), 1);
    assert_eq!(i64::from(header_user_version(&path)), STORE_SCHEMA);
}

#[test]
fn an_open_migrates_the_store_and_keeps_its_events_stamp_and_journal() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = fixture(HS1, dir.path());
    let written_as = raw_store_id(&path);
    let store = Store::open(&path, Wait::Command).expect("the store opens");
    assert_eq!(event_types(&store), vec![RAISED; 3], "the events are kept");
    assert_eq!(positions(&store), vec![1, 2, 3], "and their positions");
    assert_eq!(store.schema().stamp(), STORE_SCHEMA);
    drop(store);

    assert_ne!(
        raw_store_id(&path),
        written_as,
        "opened at another path than it recorded, the store was re-minted (ID-01)"
    );
    assert_eq!(happenstance_schema(&path), i64::from(SCHEMA_VERSION));
    assert_eq!(
        i64::from(header_user_version(&path)),
        STORE_SCHEMA,
        "Core's stamp is left as it was"
    );
    assert!(!in_wal(&path));
    assert_truncate_layout(&path);
}

#[test]
fn opening_an_existing_store_migrates_it_and_a_second_open_writes_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = fixture(HS1, dir.path());
    let store = Store::open_existing(&path, Wait::Hook)
        .expect("opens")
        .expect("a store");
    store
        .append(
            vec![
                redkiln_core::store::NewEvent::new(RAISED, br#"{"q":"?"}"#.to_vec())
                    .expect("a valid type"),
            ],
            None,
        )
        .expect("an append after the migration");
    drop(store);
    assert_eq!(happenstance_schema(&path), i64::from(SCHEMA_VERSION));

    let before = change_counter(&path);
    let store = Store::open_existing(&path, Wait::Hook)
        .expect("opens")
        .expect("a store");
    assert_eq!(event_types(&store), vec![RAISED; 4]);
    drop(store);
    assert_eq!(
        change_counter(&path),
        before,
        "a migrated store is not written on open"
    );
}

#[test]
fn concurrent_opens_of_an_old_store_all_succeed_and_append() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = fixture(HS1, dir.path());
    std::thread::scope(|scope| {
        let handles: Vec<_> = (0..8)
            .map(|n| {
                let path = &path;
                std::thread::Builder::new()
                    .name(format!("opener-{n}"))
                    .spawn_scoped(scope, move || {
                        let store = Store::open(path, Wait::Command).expect("opens");
                        store
                            .append(
                                vec![
                                    redkiln_core::store::NewEvent::new(
                                        RAISED,
                                        br#"{"q":"?"}"#.to_vec(),
                                    )
                                    .expect("a valid type"),
                                ],
                                None,
                            )
                            .expect("append");
                    })
                    .expect("spawn")
            })
            .collect();
        for handle in handles {
            handle.join().expect("the opener finished");
        }
    });
    let store = Store::open(&path, Wait::Command).expect("opens");
    assert_eq!(event_types(&store).len(), 3 + 8);
    drop(store);
    assert_eq!(happenstance_schema(&path), i64::from(SCHEMA_VERSION));
    assert_truncate_layout(&path);
}

#[test]
fn a_store_from_a_newer_happenstance_is_refused_untouched() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("events.sqlite3");
    drop(Store::open(&path, Wait::Command).expect("the store opens"));
    {
        let conn = Connection::open(&path).expect("open");
        conn.query_row("PRAGMA journal_mode = truncate", [], |r| {
            r.get::<_, String>(0)
        })
        .expect("truncate");
        let newer = SCHEMA_VERSION.checked_add(1).expect("no overflow");
        conn.execute(
            "UPDATE store_meta SET v = ?1 WHERE k = 'schema_version'",
            [newer],
        )
        .expect("a newer schema version");
    }
    let (bytes, names) = (std::fs::read(&path).expect("read"), listing(dir.path()));
    let newer = SCHEMA_VERSION.checked_add(1).expect("no overflow");
    match Store::open(&path, Wait::Command) {
        Err(StoreError::Open {
            source:
                OpenError::Store(SqliteEventStoreError::UnsupportedSchemaVersion { found, supported }),
            ..
        }) => assert_eq!((found, supported), (newer, SCHEMA_VERSION)),
        other => panic!("expected happenstance's refusal, got {other:?}"),
    }
    assert_eq!(
        std::fs::read(&path).expect("read"),
        bytes,
        "the file is unchanged"
    );
    assert_eq!(listing(dir.path()), names, "nothing was added or removed");
}
