//! An import over more lines than one batch holds (128), across the batches' edges.

use std::io::Cursor;

use redkiln_core::journal::{ImportSource, Recording, checkpoint, import};
use redkiln_core::store::{Store, Wait};
use redkiln_core::suite::RepoClass;
use redkiln_core::trust::Trust;

use super::{WAIT, all, importer, journal_lines, raised, ts, written};

const EVENTS: usize = 310;

#[test]
fn an_import_across_batches_counts_every_line_once() {
    let dir = tempfile::tempdir().expect("tempdir");
    let source = Store::open(&dir.path().join("events.sqlite3"), Wait::Command).expect("opens");
    let events: Vec<_> = (1..=EVENTS)
        .map(|n| raised(&format!("wi-{n:06}"), Some("big"), Trust::asserted()))
        .collect();
    for chunk in events.chunks(100) {
        source.append(chunk.to_vec(), None).expect("appended");
    }
    let journals = dir.path().join("journals");
    written(checkpoint(&source, RepoClass::Own, &journals, WAIT).expect("written"));
    let exported = journal_lines(&journals.join("big.jsonl"));
    assert_eq!(exported.len(), EVENTS);

    let target = Store::open(&dir.path().join("target.sqlite3"), Wait::Command).expect("opens");
    let first = exported.first().expect("a line");
    let source_id = source.store_id().to_string();
    let unheld = first.replace(
        &format!("\"origin\":\"{source_id}:1\""),
        &format!("\"origin\":\"{}:999\"", target.store_id()),
    );
    assert_ne!(&unheld, first, "the origin was rewritten");
    // Lines 1 to 129 are the first 129 events; 130 repeats line 1, which the first batch
    // stored; 131 claims a position of the target it does not hold; then the rest. The second
    // batch (lines 129 to 256) holds both, and the third ends short.
    let mut journal: Vec<&str> = exported.iter().take(129).map(String::as_str).collect();
    journal.push(first);
    journal.push(&unheld);
    journal.extend(exported.iter().skip(129).map(String::as_str));
    let text = format!("{}\n", journal.join("\n"));

    let recording = Recording {
        source: ImportSource::Artifact("big".to_owned()),
        ts: ts(),
        author: importer(),
    };
    let outcome = import(&target, Cursor::new(text.as_bytes()), recording).expect("imported");
    let events_u64 = u64::try_from(EVENTS).expect("fits");
    assert_eq!(
        (
            outcome.read,
            outcome.imported,
            outcome.present,
            outcome.malformed
        ),
        (events_u64 + 2, events_u64, 1, 1)
    );
    let numbers: Vec<u64> = outcome.problems.iter().map(|p| p.line).collect();
    assert_eq!(numbers, [131], "the unheld position, by its line");
    let facts = all(&target)
        .into_iter()
        .filter(|s| s.event_type == "weighin.item_raised")
        .count();
    assert_eq!(facts, EVENTS, "each fact once");
}
