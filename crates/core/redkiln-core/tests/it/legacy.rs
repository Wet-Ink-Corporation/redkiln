//! Reading old weighin ledgers from a copy (SPEC ST-15, ST-16), against ledgers written by
//! weighin 0.2.2 and 1.2.0, and the 0.1.x layout (the 0.2.2 ledger under WAL).

use std::path::Path;

use redkiln_core::legacy::{LegacyError, read_ledger};
use redkiln_core::store::{STORE_SCHEMA, Store, Wait};

use crate::support::{fixture, in_wal, listing, sibling};

/// Reads the fixture and checks that its file and its directory are left as they were.
fn read_untouched(path: &Path) -> redkiln_core::legacy::LegacyLedger {
    let dir = path.parent().expect("a parent");
    let (bytes, names) = (std::fs::read(path).expect("read"), listing(dir));
    let ledger = read_ledger(path).expect("the ledger reads");
    assert_eq!(
        std::fs::read(path).expect("read"),
        bytes,
        "the ledger is unchanged"
    );
    assert_eq!(
        listing(dir),
        names,
        "nothing was added or removed beside it"
    );
    ledger
}

fn summary(ledger: &redkiln_core::legacy::LegacyLedger) -> Vec<(u64, String, Vec<String>)> {
    ledger
        .events
        .iter()
        .map(|e| {
            (
                e.position,
                e.event_type.clone(),
                e.tags.iter().map(|t| t.as_str().to_owned()).collect(),
            )
        })
        .collect()
}

#[test]
fn a_1_2_0_ledger_reads_with_its_identity_and_tags() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = fixture("weighin-1.2.0.sqlite3", dir.path());
    let ledger = read_untouched(&path);
    assert_eq!(ledger.user_version, 1);
    assert_eq!(
        summary(&ledger),
        vec![
            (
                1,
                "item_raised".to_owned(),
                vec![
                    "channel:manual".to_owned(),
                    "door:two-way".to_owned(),
                    "item:wi-bee566".to_owned(),
                    "kind:question".to_owned(),
                    "session:s1".to_owned(),
                    "worktree:w120".to_owned(),
                ]
            ),
            (
                2,
                "item_raised".to_owned(),
                vec![
                    "channel:manual".to_owned(),
                    "door:one-way".to_owned(),
                    "item:wi-930e15".to_owned(),
                    "kind:question".to_owned(),
                    "session:s1".to_owned(),
                    "worktree:w120".to_owned(),
                ]
            ),
            (
                3,
                "item_dismissed".to_owned(),
                vec![
                    "item:wi-bee566".to_owned(),
                    "session:s1".to_owned(),
                    "worktree:w120".to_owned(),
                ]
            ),
        ]
    );
    assert_eq!(
        ledger.store_id.to_string(),
        "787c5e52af350ef7871cd73e3e5b6c8f",
        "the identity the ledger minted under"
    );
    for (n, event) in (1_u64..).zip(&ledger.events) {
        assert_eq!(event.id.store(), ledger.store_id);
        assert_eq!(u64::from(event.id.position()), n);
    }
    let first = ledger.events.first().expect("an event");
    assert!(
        first
            .data
            .starts_with(br#"{"type":"item_raised","item_id":"wi-bee566""#),
        "the payload is the 1.x envelope as written"
    );
}

#[test]
fn a_0_2_2_ledger_reads() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = fixture("weighin-0.2.2.sqlite3", dir.path());
    let ledger = read_untouched(&path);
    assert_eq!(ledger.user_version, 0);
    let types: Vec<_> = summary(&ledger)
        .into_iter()
        .map(|(p, t, _)| (p, t))
        .collect();
    assert_eq!(
        types,
        vec![
            (1, "item_raised".to_owned()),
            (2, "item_raised".to_owned()),
            (3, "item_dismissed".to_owned()),
        ]
    );
}

#[test]
fn a_0_1_x_wal_ledger_reads_and_stays_in_wal() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = fixture("weighin-0.1.x.sqlite3", dir.path());
    assert!(in_wal(&path));
    let ledger = read_untouched(&path);
    assert_eq!(ledger.events.len(), 3);
    assert!(in_wal(&path), "the ledger keeps its journal mode");
    assert!(!sibling(&path, "-shm").exists());
}

#[test]
fn a_wal_ledger_with_uncheckpointed_frames_reads_them_from_the_copy() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = fixture("weighin-0.1.x.sqlite3", dir.path());
    // Hold a connection open while another appends, so the frame stays in the -wal file.
    let holder = happenstance_sqlite::rusqlite::Connection::open(&path).expect("open");
    holder
        .execute_batch("PRAGMA wal_autocheckpoint = 0; SELECT count(*) FROM event;")
        .expect("hold");
    holder
        .execute(
            "INSERT INTO event (event_type, data, tags, origin_store, origin_position, recorded_at)
             SELECT 'item_raised', data, tags, origin_store, 4, recorded_at FROM event WHERE position = 1",
            [],
        )
        .expect("append a frame");
    assert!(sibling(&path, "-wal").exists(), "the frame is in the WAL");
    let ledger = read_ledger(&path).expect("reads");
    assert_eq!(ledger.events.len(), 4, "the WAL's frame is read");
    drop(holder);
}

#[test]
fn a_core_store_a_missing_file_and_a_non_database_are_not_old_ledgers() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = dir.path().join("events.sqlite3");
    drop(Store::open(&store, Wait::Command).expect("open"));
    assert!(matches!(
        read_ledger(&store),
        Err(LegacyError::CoreStore { found, .. }) if found == STORE_SCHEMA
    ));
    assert!(matches!(
        read_ledger(&dir.path().join("absent.sqlite3")),
        Err(LegacyError::Missing { .. })
    ));
    let text = dir.path().join("notes.txt");
    std::fs::write(
        &text,
        "not a database, and longer than a header would be: ".repeat(4),
    )
    .expect("write");
    assert!(matches!(
        read_ledger(&text),
        Err(LegacyError::NotADatabase { .. })
    ));
}

#[test]
fn a_ledger_mid_commit_is_reported_rather_than_copied_torn() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = fixture("weighin-0.2.2.sqlite3", dir.path());
    std::fs::write(sibling(&path, "-journal"), b"a hot journal").expect("write");
    assert!(matches!(read_ledger(&path), Err(LegacyError::HotJournal)));
}
