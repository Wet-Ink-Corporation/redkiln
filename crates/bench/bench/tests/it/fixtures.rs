//! The old-ledger fixtures (`tests/fixtures/anneal/`, how each was made in its `README.md`) stay
//! exactly as committed: ST-15 and ST-16 forbid writing to an old ledger, and each stands for
//! one. Every test reads a fixture from a copy (Core's `legacy::read_ledger` copies it itself);
//! this test catches anything that opens one in place: an SQLite connection to the WAL fixture,
//! even a read-only one, checkpoints or leaves `-shm` beside it, and a writer moves the bytes.
//! It also pins what each fixture is: its journal mode and stamp from the header, its
//! happenstance schema from its DDL, and its event types as Core's reader reads them.

use std::path::{Path, PathBuf};

use redkiln_core::ids::Digest;
use redkiln_core::legacy::{LegacyLedger, read_ledger};

/// Each fixture and the sha256 of its committed bytes; the directory also holds `README.md`.
const FIXTURES: [(&str, &str); 6] = [
    (
        "anneal-0.0.5.sqlite3",
        "b4e13efdbce61560a8df1022f2509f814c340d2d849a32d3f15daedb5ca48b51",
    ),
    (
        "anneal-older.sqlite3",
        "5885e78bde269b8a8a73817e3a2f30e1efeee7a8b4fc37ec971a5474d9526431",
    ),
    (
        "anneal-wal.sqlite3",
        "5a75c0a7e17faeb7565565332e75ce4c62dd0b91181f968d106f45b3be26019b",
    ),
    (
        "anneal-wal.sqlite3-wal",
        "cc5d4c1866c1966aea0d7b50a619f4671c087db6184a742699a73751cdb31703",
    ),
    (
        "bench-ledger-hs032.sqlite3",
        "09d9a8f5037306eccb9f265e865a96558b0caaf7a00e92c7b45118c8e1414d5b",
    ),
    (
        "bench-ledger-hs060.sqlite3",
        "5122655e37417a6ed7346e57615debe012a328005b9b946c78dd3e7571bd09d5",
    ),
];

/// Every one of anneal 0.0.5's events, in position order: all 17 types, `plan_approved` with a
/// human, a model, no and a via-only `by`, and a `task_done` whose actor Core refuses.
const ANNEAL_005_TYPES: [&str; 23] = [
    "plan_approved",
    "plan_approved",
    "plan_approved",
    "plan_approved",
    "verdict_set",
    "verdict_set",
    "test_run",
    "task_done",
    "grill_confirmed",
    "repro_red",
    "round_started",
    "round_started",
    "gate_blocked",
    "stop_blocked",
    "mode_set",
    "subagent_status",
    "migrate.wave.started",
    "migrate.wave.done",
    "migrate.contract",
    "weighin_parked",
    "bridge.unavailable",
    "orchestrate.ruled",
    "task_done",
];

/// How a fixture's header and schema say it was left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Kind {
    /// Header bytes 18 and 19 both 2.
    wal: bool,
    /// `user_version`, header bytes 60 to 64.
    stamp: u32,
    /// happenstance's schema: 2 has `write_group` in `event`'s DDL, 1 does not.
    happenstance: u8,
}

fn dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/anneal")
}

fn kind(name: &str) -> Kind {
    let bytes = std::fs::read(dir().join(name)).expect("read the fixture");
    let header = bytes.first_chunk::<100>().expect("an SQLite header");
    // The schema's DDL sits on page 1 as text; 4 KiB is the page size of every fixture.
    let page_one = bytes.get(..4096).expect("a first page");
    let ddl_names_write_group = page_one
        .windows(b"write_group".len())
        .any(|w| w == b"write_group");
    Kind {
        wal: header[18] == 2 && header[19] == 2,
        stamp: u32::from_be_bytes(header[60..64].try_into().expect("four bytes")),
        happenstance: if ddl_names_write_group { 2 } else { 1 },
    }
}

fn read(name: &str) -> LegacyLedger {
    read_ledger(&dir().join(name)).expect("Core reads the fixture from a copy")
}

fn types(ledger: &LegacyLedger) -> Vec<&str> {
    ledger
        .events
        .iter()
        .map(|e| e.event_type.as_str())
        .collect()
}

#[test]
fn the_fixtures_directory_holds_only_the_fixtures_as_committed() {
    let mut listing: Vec<String> = std::fs::read_dir(dir())
        .expect("read the directory")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    listing.sort();
    let mut expected = vec!["README.md"];
    expected.extend(FIXTURES.map(|(name, _)| name));
    assert_eq!(
        listing, expected,
        "nothing beside the fixtures: a -shm or -journal means one was opened in place; copy it \
         to a temporary directory first"
    );
    for (name, sha256) in FIXTURES {
        let bytes = std::fs::read(dir().join(name)).expect("read the fixture");
        assert_eq!(
            Digest::sha256(&bytes).hex(),
            sha256,
            "{name} changed: a test wrote to it in place"
        );
    }
}

#[test]
fn each_fixture_is_the_ledger_its_readme_says() {
    let rollback = |happenstance| Kind {
        wal: false,
        stamp: 1,
        happenstance,
    };
    assert_eq!(kind("anneal-0.0.5.sqlite3"), rollback(1));
    assert_eq!(kind("anneal-older.sqlite3"), rollback(1));
    assert_eq!(kind("bench-ledger-hs032.sqlite3"), rollback(1));
    assert_eq!(kind("bench-ledger-hs060.sqlite3"), rollback(2));
    assert_eq!(
        kind("anneal-wal.sqlite3"),
        Kind {
            wal: true,
            stamp: 1,
            happenstance: 1
        }
    );
}

#[test]
fn core_reads_every_fixture_s_events_from_a_copy() {
    let anneal = read("anneal-0.0.5.sqlite3");
    assert_eq!(types(&anneal), ANNEAL_005_TYPES);
    assert_eq!(
        types(&read("anneal-older.sqlite3")),
        [
            "test_run",
            "plan_approved",
            "grill_confirmed",
            "verdict_set",
            "round_started",
            "mode_set",
            "subagent_status",
        ]
    );
    assert_eq!(
        types(&read("bench-ledger-hs032.sqlite3")),
        ["task_done", "test_run", "task_done"]
    );
    assert_eq!(
        types(&read("bench-ledger-hs060.sqlite3")),
        [
            "plan_approved",
            "verdict_set",
            "test_run",
            "mode_set",
            "subagent_status"
        ]
    );
    let wal = read("anneal-wal.sqlite3");
    assert_eq!(
        (wal.store_id, wal.events),
        (anneal.store_id, anneal.events),
        "the WAL fixture is anneal 0.0.5's ledger, its last seven events only in the -wal"
    );
}
