//! Opening only a store that exists (`Store::open_existing`): a read never creates the store or
//! its directory (weighin's B-01), and an old ledger is refused exactly as `Store::open` refuses
//! it, never taken for a missing store.

use std::path::Path;

use redkiln_core::store::{Appended, NewEvent, OpenError, Store, StoreError, Tag, Wait};

use crate::support::{fixture, listing};

fn existing(path: &Path) -> Result<Option<Store>, StoreError> {
    Store::open_existing(path, Wait::Command)
}

fn refusal(result: Result<Option<Store>, StoreError>) -> OpenError {
    match result {
        Err(StoreError::Open { source, .. }) => source,
        Ok(found) => panic!("expected a refusal, got {found:?}"),
        Err(other) => panic!("expected an open refusal, got {other:?}"),
    }
}

#[test]
fn opening_only_an_existing_store_at_a_missing_path_is_none_and_creates_nothing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("redkiln").join("events.sqlite3");

    let opened = existing(&path).expect("no failure");

    assert!(opened.is_none(), "{opened:?}");
    assert!(
        listing(dir.path()).is_empty(),
        "no file, directory or lock was created"
    );
}

#[test]
fn opening_only_an_existing_store_opens_one_and_refuses_an_old_ledger_as_open_does() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("events.sqlite3");
    let created = Store::open(&path, Wait::Command).expect("created");
    let event = NewEvent::new("weighin.item_raised", br#"{"q":"?"}"#.to_vec())
        .expect("a valid type")
        .with_tag(Tag::new("item", "wi-a").expect("a valid tag"));
    assert_eq!(
        created.append(vec![event], None).expect("append"),
        Appended::At(1)
    );
    let id = created.store_id();
    drop(created);

    let reopened = existing(&path).expect("opens").expect("the store exists");

    assert_eq!(reopened.store_id(), id);
    let old = tempfile::tempdir().expect("tempdir");
    for (name, expected) in [
        ("weighin-1.2.0.sqlite3", "OldLedger"),
        ("weighin-0.2.2.sqlite3", "NotAStore"),
        ("weighin-0.1.x.sqlite3", "NotAStore"),
    ] {
        let ledger = fixture(name, old.path());
        let (bytes, names) = (std::fs::read(&ledger).expect("read"), listing(old.path()));
        let from_open = refusal(Store::open(&ledger, Wait::Command).map(Some));
        let from_existing = refusal(existing(&ledger));
        assert_eq!(
            (variant(&from_existing), variant(&from_open)),
            (expected, expected),
            "{name}"
        );
        assert_eq!(std::fs::read(&ledger).expect("read"), bytes, "{name}");
        assert_eq!(listing(old.path()), names, "{name}: nothing beside it");
    }
    let foreign = old.path().join("other.sqlite3");
    let conn = happenstance_sqlite::rusqlite::Connection::open(&foreign).expect("open");
    conn.execute_batch("CREATE TABLE t (x)").expect("another program's table");
    drop(conn);
    assert_eq!(variant(&refusal(existing(&foreign))), "NotAStore");
}

/// The refusal's name, so two refusals compare whatever their fields.
fn variant(err: &OpenError) -> &'static str {
    match err {
        OpenError::OldLedger { .. } => "OldLedger",
        OpenError::NotAStore => "NotAStore",
        OpenError::NoStore => "NoStore",
        _ => "other",
    }
}
