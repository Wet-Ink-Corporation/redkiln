//! `core.import_completed` goldens (JR-13, ST-15): each of the four sources round-trips, and the
//! path and legacy goldens are what Core's own import writes. The ref and artifact sources are
//! written only by `import_refs`, on real repositories (`tests/repos`), so their goldens are
//! pinned here by decoding and encoding back.

use redkiln_core::ids::parse_store_id;
use redkiln_core::journal::{
    ImportCompleted, ImportSource, Mapped, MappedImport, import, import_mapped,
};
use redkiln_core::store::{Filter, Store};

use super::envelope::round_trips;
use super::{fresh_store, golden, one_line, recording};

/// The bytes of the one `core.import_completed` event in `store`.
fn recorded(store: &Store) -> Vec<u8> {
    let filter = Filter::all()
        .of_type("core.import_completed")
        .expect("a type");
    let two = std::num::NonZeroUsize::new(2).expect("non-zero");
    let found = store.read(&filter, 0, two).expect("read");
    let Ok([only]) = <[_; 1]>::try_from(found) else {
        panic!("one import record");
    };
    only.data
}

#[test]
fn import_completed_goldens_round_trip() {
    for (rel, source) in [
        (
            "envelope/valid/import-completed-path.json",
            ImportSource::Path("journal/jr-02-item-raised.jsonl".to_owned()),
        ),
        (
            "envelope/valid/import-completed-ref.json",
            ImportSource::Ref {
                name: "refs/redkiln/journal/s1".to_owned(),
                commit: "e83c5163316f89bfbde7d9ab23ca2e25604af290".to_owned(),
            },
        ),
        (
            "envelope/valid/import-completed-artifact.json",
            ImportSource::Artifact("redkiln-journal-s1".to_owned()),
        ),
        (
            "envelope/valid/import-completed-legacy.json",
            ImportSource::Legacy {
                path: ".kb/_ledger/journal/s1.jsonl".to_owned(),
                store_id: parse_store_id("8f161dc3d6d01eac2c7949e3b17ffa04").expect("an id"),
            },
        ),
    ] {
        let envelope = round_trips::<ImportCompleted>(rel);
        assert_eq!(envelope.data.source, source, "{rel}");
    }
}

#[test]
fn import_writes_the_path_golden() {
    let (_dir, store) = fresh_store();
    let journal = golden("journal/valid/jr-02-item-raised.jsonl");
    let source = ImportSource::Path("journal/jr-02-item-raised.jsonl".to_owned());
    let report = import(&store, journal.as_slice(), recording(source)).expect("imports");
    assert_eq!((report.read, report.imported), (1, 1));
    let written = recorded(&store);
    assert_eq!(
        String::from_utf8_lossy(&written),
        String::from_utf8_lossy(&one_line("envelope/valid/import-completed-path.json"))
    );
    super::schema::envelope_conforms(&written);
}

#[test]
fn import_mapped_writes_the_legacy_golden() {
    let (_dir, store) = fresh_store();
    let source = ImportSource::Legacy {
        path: ".kb/_ledger/journal/s1.jsonl".to_owned(),
        store_id: parse_store_id("8f161dc3d6d01eac2c7949e3b17ffa04").expect("an id"),
    };
    let outcome =
        import_mapped(&store, [(1, Mapped::Unmapped)], recording(source), None).expect("imports");
    let MappedImport::Completed(report) = outcome else {
        panic!("completed: {outcome:?}");
    };
    assert_eq!((report.read, report.malformed), (1, 1));
    let written = recorded(&store);
    assert_eq!(
        String::from_utf8_lossy(&written),
        String::from_utf8_lossy(&one_line("envelope/valid/import-completed-legacy.json"))
    );
    super::schema::envelope_conforms(&written);
}
