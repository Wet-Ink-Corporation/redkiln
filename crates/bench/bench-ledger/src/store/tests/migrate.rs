//! A ledger anneal wrote on happenstance 0.3.2 (happenstance schema 1) opens on 0.6.0, which
//! migrates it to schema 2 in place and keeps its events, its stamp and its TRUNCATE journal.
//! The fixture `tests/fixtures/bench-ledger-hs032.sqlite3` is three events (`task.done`,
//! `test.run`, `task.done`, session `s1`) written by anneal-ledger on happenstance 0.3.2.

use std::path::{Path, PathBuf};

use happenstance_sqlite::event_store::{SCHEMA_VERSION, SqliteEventStoreError};

use super::super::schema::{LEDGER_SCHEMA, user_version};
use super::super::{Filter, OpenError, Store, StoreError};
use super::{journal_mode_of, positions, raw, sibling, task_done};
use crate::event::EventKind;

const FIXTURE: &str = "bench-ledger-hs032.sqlite3";

/// A copy of the fixture in `dir`, so the test never writes the checked-in file.
fn fixture(dir: &Path) -> PathBuf {
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(FIXTURE);
    let path = dir.join(FIXTURE);
    std::fs::copy(&source, &path).expect("copies the fixture");
    path
}

/// happenstance's own schema version, its `store_meta` row.
fn happenstance_schema(path: &Path) -> i64 {
    raw(path)
        .query_row(
            "SELECT CAST(v AS INTEGER) FROM store_meta WHERE k = 'schema_version'",
            [],
            |row| row.get(0),
        )
        .expect("the schema version row")
}

#[test]
#[cfg_attr(miri, ignore = "opens SQLite through FFI")]
fn the_fixture_is_a_happenstance_schema_1_ledger() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = fixture(dir.path());

    assert_eq!(happenstance_schema(&path), 1, "happenstance 0.3.2's schema");
    assert_eq!(
        user_version(&raw(&path)).expect("readable"),
        LEDGER_SCHEMA,
        "stamped by anneal"
    );
}

#[test]
#[cfg_attr(miri, ignore = "opens SQLite through FFI")]
fn a_ledger_written_on_happenstance_0_3_2_opens_with_its_events() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = fixture(dir.path());

    let store = Store::open(&path).expect("the 0.3.2 ledger opens");
    let events = store.query(&Filter::default()).expect("reads");
    let kinds: Vec<EventKind> = events.iter().map(|e| e.event.event.kind()).collect();
    let appended = store.append(&task_done("T2", "s1")).expect("appends");

    assert_eq!(positions(&events), [1, 2, 3], "the positions are kept");
    assert_eq!(
        kinds,
        [EventKind::TaskDone, EventKind::TestRun, EventKind::TaskDone],
        "the events are kept"
    );
    assert_eq!(appended, 4, "a new event follows them");
}

#[test]
#[cfg_attr(miri, ignore = "opens SQLite through FFI")]
fn opening_migrates_the_schema_and_keeps_the_stamp_and_the_truncate_journal() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = fixture(dir.path());

    drop(Store::open(&path).expect("opens"));

    assert_eq!(
        happenstance_schema(&path),
        i64::from(SCHEMA_VERSION),
        "migrated to this build's happenstance schema"
    );
    assert_eq!(
        user_version(&raw(&path)).expect("readable"),
        LEDGER_SCHEMA,
        "the ledger's own stamp is kept"
    );
    assert_eq!(journal_mode_of(&path), "delete", "not left in WAL");
    for suffix in ["-wal", "-shm"] {
        assert!(!sibling(&path, suffix).exists(), "no {suffix} beside it");
    }
    let journal = std::fs::metadata(sibling(&path, "-journal")).expect("the journal is kept");
    assert_eq!(
        journal.len(),
        0,
        "migrated under TRUNCATE: truncated, not deleted"
    );
}

#[test]
#[cfg_attr(miri, ignore = "opens SQLite through FFI")]
fn a_schema_1_ledger_with_no_identity_is_refused_untouched() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = fixture(dir.path());
    raw(&path)
        .execute("DELETE FROM store_meta WHERE k = 'store_id'", [])
        .expect("set up");
    let before = std::fs::read(&path).expect("readable");

    let result = Store::open(&path);

    assert!(
        matches!(
            &result,
            Err(StoreError::Open {
                source: OpenError::Store(SqliteEventStoreError::MissingIdentity),
                ..
            })
        ),
        "{result:?}"
    );
    assert_eq!(std::fs::read(&path).expect("readable"), before, "untouched");
    assert_eq!(happenstance_schema(&path), 1, "not migrated");
}

#[test]
#[cfg_attr(miri, ignore = "opens SQLite through FFI")]
fn a_schema_1_ledger_whose_identity_is_not_sixteen_bytes_is_refused_untouched() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = fixture(dir.path());
    raw(&path)
        .execute(
            "UPDATE store_meta SET v = randomblob(15) WHERE k = 'store_id'",
            [],
        )
        .expect("set up");
    let before = std::fs::read(&path).expect("readable");

    let result = Store::open(&path);

    assert!(
        matches!(
            &result,
            Err(StoreError::Open {
                source: OpenError::Store(SqliteEventStoreError::MalformedIdentity { len: 15 }),
                ..
            })
        ),
        "{result:?}"
    );
    assert_eq!(std::fs::read(&path).expect("readable"), before, "untouched");
}

#[test]
#[cfg_attr(miri, ignore = "opens SQLite through FFI")]
fn a_ledger_at_a_newer_happenstance_schema_is_refused_untouched() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = fixture(dir.path());
    let newer = SCHEMA_VERSION + 1;
    raw(&path)
        .execute(
            "UPDATE store_meta SET v = ?1 WHERE k = 'schema_version'",
            [newer],
        )
        .expect("set up");
    let before = std::fs::read(&path).expect("readable");

    let result = Store::open(&path);

    assert!(
        matches!(
            &result,
            Err(StoreError::Open {
                source: OpenError::Store(SqliteEventStoreError::UnsupportedSchemaVersion { found, .. }),
                ..
            }) if *found == newer
        ),
        "{result:?}"
    );
    assert_eq!(std::fs::read(&path).expect("readable"), before, "untouched");
}
