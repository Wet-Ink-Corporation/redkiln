//! The SPEC's golden fixtures and JSON Schemas (`spec/fixtures/`, `spec/schemas/`) against Core
//! (Move 1 step 3, #18): every golden is read here, Core writes back the goldens it writes, and
//! each SPEC example equals its golden, so the SPEC's text, its fixtures and Core cannot drift
//! apart without a red test. `spec/fixtures/README.md` indexes the files and their classes.

mod cli;
mod decision_request;
mod envelope;
mod journal;
mod records;
mod schema;
mod spec_text;
mod status;

use std::collections::BTreeSet;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};

use redkiln_core::envelope::{Actor, Author, Class, Payload, Timestamp, Writer};
use redkiln_core::ids::parse_event_ref;
use redkiln_core::journal::{ImportSource, Recording};
use redkiln_core::store::{Filter, Store, Stored, Wait};
use redkiln_core::trust::Trust;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tempfile::TempDir;

/// The SPEC's folder, beside the workspace's crates.
pub(crate) fn spec_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../spec")
}

/// The bytes of `spec/fixtures/<rel>`.
pub(crate) fn golden(rel: &str) -> Vec<u8> {
    let path = spec_dir().join("fixtures").join(rel);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

/// A one-line golden's bytes without its final `\n`: it holds exactly one `\n`, last.
pub(crate) fn one_line(rel: &str) -> Vec<u8> {
    let bytes = golden(rel);
    let Some(line) = bytes.strip_suffix(b"\n") else {
        panic!("{rel} ends in a \\n");
    };
    assert!(!line.contains(&b'\n'), "{rel} is one line");
    line.to_vec()
}

/// `bytes` as JSON.
pub(crate) fn value_of(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).expect("the golden is JSON")
}

/// `bytes` as a stored event of the type its `type` names: a fixed identity, position 1 and no
/// tags, as a typed read sees it.
pub(crate) fn stored_of(bytes: &[u8]) -> Stored {
    let event_type = value_of(bytes)["type"]
        .as_str()
        .expect("the golden names its type")
        .to_owned();
    Stored {
        position: 1,
        id: parse_event_ref("0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:1").expect("an event ref"),
        recorded_at_millis: 0,
        event_type,
        data: bytes.to_vec(),
        tags: Vec::new(),
    }
}

/// A new store in a temporary folder.
pub(crate) fn fresh_store() -> (TempDir, Store) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&dir.path().join("events.sqlite3"), Wait::Command).expect("opens");
    (dir, store)
}

/// How the goldens' imports are recorded: by `weighin` 2.0.0 from the command line, at the time
/// `envelope/valid/import-completed-*.json` carry.
pub(crate) fn recording(source: ImportSource) -> Recording {
    Recording {
        source,
        ts: Timestamp::parse("2026-11-08T09:00:00Z").expect("a ts"),
        author: Author {
            actor: Actor::Cli,
            trust: Trust::asserted(),
            writer: Writer::new("weighin", "2.0.0").expect("a writer"),
        },
    }
}

/// Every event of `store`, in order.
pub(crate) fn all(store: &Store) -> Vec<Stored> {
    store
        .read(&Filter::all(), 0, NonZeroUsize::new(100).expect("non-zero"))
        .expect("read")
}

/// `weighin.item_decided`, the EV-01 example's payload, keys in the golden's order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ItemDecided {
    pub(crate) item: String,
    pub(crate) option: String,
    pub(crate) applies: String,
}

impl Payload for ItemDecided {
    const TYPE: &'static str = "weighin.item_decided";
    const VERSION: u32 = 1;
    const CLASS: Class = Class::Durable;
}

/// `weighin.item_raised`, the JR-02 example's payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ItemRaised {
    pub(crate) item: String,
    pub(crate) question: String,
}

impl Payload for ItemRaised {
    const TYPE: &'static str = "weighin.item_raised";
    const VERSION: u32 = 1;
    const CLASS: Class = Class::Durable;
}

/// The files under `dir`, relative to it, `/`-separated.
fn files_under(dir: &Path, prefix: &str, out: &mut BTreeSet<String>) {
    for entry in std::fs::read_dir(dir).expect("read the directory") {
        let entry = entry.expect("an entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        let rel = format!("{prefix}{name}");
        if entry.file_type().expect("a file type").is_dir() {
            files_under(&entry.path(), &format!("{rel}/"), out);
        } else {
            out.insert(rel);
        }
    }
}

#[test]
fn every_fixture_and_schema_is_listed() {
    let mut on_disk = BTreeSet::new();
    files_under(&spec_dir().join("fixtures"), "fixtures/", &mut on_disk);
    // The index, and ID-09's recipe check, which CI runs as a script.
    on_disk.remove("fixtures/README.md");
    on_disk.retain(|rel| !rel.starts_with("fixtures/patch-id/"));
    files_under(&spec_dir().join("schemas"), "schemas/", &mut on_disk);
    let listed: BTreeSet<String> = schema::listed().collect();
    assert_eq!(
        on_disk, listed,
        "every file is in a table, every table entry is a file"
    );
}
