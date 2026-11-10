//! The fixtures stay exactly as committed: ST-15 and ST-16 forbid writing to an old ledger, and
//! three fixtures stand for one; the fourth, a Core store under happenstance schema 1, is what an
//! open migrates (ST-08), so only its copies may move. Every test copies a fixture before it reads it;
//! this test catches anything that opens one in place: an SQLite connection to the WAL-mode
//! 0.1.x ledger, even a read-only one, leaves `-shm` and `-wal` beside it, and a writer would
//! move its bytes.

use std::path::Path;

use redkiln_core::ids::Digest;

use crate::support::listing;

/// Each fixture and the sha256 of its committed bytes.
const FIXTURES: [(&str, &str); 4] = [
    (
        "core-store-hs1.sqlite3",
        "a018c66ff23fcf02453ac27897b70e73e03cfe6a8b9a4752f0ca4f6e3f2a4d07",
    ),
    (
        "weighin-0.1.x.sqlite3",
        "f41ad7bdc6b45bb75122e8d6838a94faa22a89d2de814dfabce50fa5c33b247c",
    ),
    (
        "weighin-0.2.2.sqlite3",
        "0c36196f6377ed010da55f21f516e599c4274f5d6b701ce8595ab5a6da72047f",
    ),
    (
        "weighin-1.2.0.sqlite3",
        "8c4b4c1d2a54b4db2437fb7ad5528f770f42514257451eb886e830d64f666349",
    ),
];

#[test]
fn the_fixtures_directory_holds_only_the_fixtures_as_committed() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    assert_eq!(
        listing(&dir),
        FIXTURES.map(|(name, _)| name),
        "nothing beside the fixtures: a -shm, -wal or -journal means one was opened in place; \
         copy it to a temporary directory first (support::fixture)"
    );
    for (name, sha256) in FIXTURES {
        let bytes = std::fs::read(dir.join(name)).expect("read the fixture");
        assert_eq!(
            Digest::sha256(&bytes).hex(),
            sha256,
            "{name} changed: a test wrote to it in place"
        );
    }
}
