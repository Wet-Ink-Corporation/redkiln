//! When an open takes the open lock (`<store>.lock`, ST-05), and what it refuses before it writes
//! anything (ID-01, ST-08): R-SQLITE's opener item and #75's follow-ups.

use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};

use happenstance_sqlite::event_store::SqliteEventStoreError;
use happenstance_sqlite::rusqlite::Connection;
use redkiln_core::store::{OpenError, Store, StoreError, Wait};

use crate::support::{fixture, listing, raw_store_id, sibling};

/// The open lock beside `path`, held until the file is dropped, as another opener holds it while
/// it creates or converts the store.
fn hold_open_lock(path: &Path) -> File {
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(sibling(path, ".lock"))
        .expect("the lock file");
    lock.lock().expect("hold the open lock");
    lock
}

/// A store created at `dir/a/events.sqlite3` and closed.
fn created(dir: &Path) -> PathBuf {
    let path = dir.join("a/events.sqlite3");
    drop(Store::open(&path, Wait::Command).expect("the store opens"));
    path
}

/// A copy of the store at `path` in `dir/b/`, with nothing beside it.
fn copied(path: &Path, dir: &Path) -> PathBuf {
    let copy = dir.join("b/events.sqlite3");
    std::fs::create_dir_all(copy.parent().expect("parent")).expect("mkdir");
    std::fs::copy(path, &copy).expect("copy");
    copy
}

/// A TRUNCATE connection to the store at `path`, for writing to it behind Core's back.
fn raw(path: &Path) -> Connection {
    let conn = Connection::open(path).expect("open");
    conn.query_row("PRAGMA journal_mode = truncate", [], |r| {
        r.get::<_, String>(0)
    })
    .expect("truncate");
    conn
}

/// Records `path` as the store's own canonical path in `core_meta`, as an open there would have,
/// so that no re-mint is due when it is opened where it is.
fn record_own_path(path: &Path) {
    let canonical = std::fs::canonicalize(path)
        .expect("canonical")
        .into_os_string()
        .into_encoded_bytes();
    raw(path)
        .execute(
            "INSERT OR REPLACE INTO core_meta (k, v) VALUES ('path', ?1)",
            [canonical],
        )
        .expect("record the path");
}

/// happenstance's own schema version, its `store_meta` row.
fn happenstance_schema(path: &Path) -> i64 {
    raw(path)
        .query_row(
            "SELECT CAST(v AS INTEGER) FROM store_meta WHERE k = 'schema_version'",
            [],
            |r| r.get(0),
        )
        .expect("the schema version row")
}

#[test]
fn a_store_at_its_recorded_path_opens_while_the_open_lock_is_held() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = created(dir.path());
    let lock = hold_open_lock(&path);
    let store = Store::open(&path, Wait::Hook).expect("no lock is needed to open it");
    drop(store);
    drop(lock);
}

#[test]
fn a_copy_at_another_path_waits_for_the_open_lock_before_it_re_mints() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = created(dir.path());
    let copy = copied(&path, dir.path());
    let before = raw_store_id(&copy);
    let lock = hold_open_lock(&copy);
    match Store::open(&copy, Wait::Hook) {
        Err(StoreError::Open {
            source: OpenError::LockBusy,
            ..
        }) => {}
        other => panic!("expected the open to wait for the lock, got {other:?}"),
    }
    drop(lock);
    assert_eq!(raw_store_id(&copy), before, "nothing was re-minted");
}

#[test]
fn a_store_with_a_stale_wal_beside_it_waits_for_the_open_lock() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = created(dir.path());
    std::fs::write(sibling(&path, "-wal"), b"stale").expect("a stale wal");
    let lock = hold_open_lock(&path);
    match Store::open(&path, Wait::Hook) {
        Err(StoreError::Open {
            source: OpenError::LockBusy,
            ..
        }) => {}
        other => panic!("expected the open to wait for the lock, got {other:?}"),
    }
    drop(lock);
    assert_eq!(
        std::fs::read(sibling(&path, "-wal")).expect("still there"),
        b"stale",
        "nothing is set aside without the lock"
    );
}

/// `path`'s bytes and its folder's listing, to show an open left both as they were.
fn snapshot(path: &Path) -> (Vec<u8>, Vec<String>) {
    let folder = path.parent().expect("a folder");
    (std::fs::read(path).expect("read"), listing(folder))
}

#[test]
fn a_store_from_a_newer_happenstance_at_another_path_is_refused_before_any_re_mint() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = created(dir.path());
    let newer = happenstance_sqlite::event_store::SCHEMA_VERSION
        .checked_add(1)
        .expect("no overflow");
    raw(&path)
        .execute(
            "UPDATE store_meta SET v = ?1 WHERE k = 'schema_version'",
            [newer],
        )
        .expect("a newer happenstance schema");
    let copy = copied(&path, dir.path());
    let before = snapshot(&copy);
    match Store::open(&copy, Wait::Command) {
        Err(StoreError::Open {
            source: OpenError::Store(SqliteEventStoreError::UnsupportedSchemaVersion { found, .. }),
            ..
        }) => assert_eq!(found, newer),
        other => panic!("expected happenstance's refusal, got {other:?}"),
    }
    assert_eq!(snapshot(&copy), before, "no identity was rewritten");
}

/// Opens `path` and expects the missing identity's refusal, with the file as it was.
fn assert_refused_with_no_identity(path: &Path) {
    let before = snapshot(path);
    match Store::open(path, Wait::Command) {
        Err(StoreError::Open {
            source: OpenError::MissingIdentity,
            ..
        }) => {}
        other => panic!("expected the missing identity's refusal, got {other:?}"),
    }
    assert_eq!(snapshot(path), before, "no identity was minted");
}

#[test]
fn a_store_with_no_identity_row_is_refused_where_it_was_recorded_and_elsewhere() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = created(dir.path());
    raw(&path)
        .execute("DELETE FROM store_meta WHERE k = 'store_id'", [])
        .expect("drop the identity row");
    let copy = copied(&path, dir.path());
    assert_refused_with_no_identity(&path);
    assert_refused_with_no_identity(&copy);
}

#[test]
fn a_happenstance_schema_1_store_with_no_identity_row_is_refused_not_migrated() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = fixture("core-store-hs1.sqlite3", dir.path());
    // Its recorded path is this one, so no re-mint is due.
    record_own_path(&path);
    raw(&path)
        .execute("DELETE FROM store_meta WHERE k = 'store_id'", [])
        .expect("drop the identity row");
    assert_refused_with_no_identity(&path);
}

#[test]
fn a_store_needing_a_happenstance_migration_at_its_recorded_path_waits_for_the_open_lock() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = fixture("core-store-hs1.sqlite3", dir.path());
    record_own_path(&path);
    let written_as = raw_store_id(&path);
    let lock = hold_open_lock(&path);
    match Store::open(&path, Wait::Hook) {
        Err(StoreError::Open {
            source: OpenError::LockBusy,
            ..
        }) => {}
        other => panic!("expected the migration to wait for the lock, got {other:?}"),
    }
    assert_eq!(
        happenstance_schema(&path),
        1,
        "nothing migrated without the lock"
    );
    drop(lock);
    drop(Store::open(&path, Wait::Hook).expect("opens once the lock is free"));
    assert_eq!(
        happenstance_schema(&path),
        i64::from(happenstance_sqlite::event_store::SCHEMA_VERSION),
        "migrated under the lock"
    );
    assert_eq!(
        raw_store_id(&path),
        written_as,
        "at its recorded path: no re-mint"
    );
}
