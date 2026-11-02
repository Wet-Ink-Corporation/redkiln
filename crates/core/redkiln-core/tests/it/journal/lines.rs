//! What a line carries through a checkpoint and an import: a caller's tags (JR-02), and a type
//! whose namespace this build does not know (JR-10).

use std::io::Cursor;

use redkiln_core::journal::{ImportSource, Recording, checkpoint, import};
use redkiln_core::store::{Store, Stored, Wait};
use redkiln_core::suite::RepoClass;

use super::{WAIT, all, importer, journal_lines, source_store, ts, written};

/// The tags of `stored` as text, sorted.
fn tags(stored: &Stored) -> Vec<String> {
    let mut tags: Vec<String> = stored.tags.iter().map(|t| t.as_str().to_owned()).collect();
    tags.sort();
    tags
}

fn import_text(target: &Store, journal: &str) -> redkiln_core::journal::ImportReport {
    let recording = Recording {
        source: ImportSource::Artifact("lines".to_owned()),
        ts: ts(),
        author: importer(),
    };
    import(target, Cursor::new(journal.as_bytes()), recording).expect("imported")
}

#[test]
fn a_callers_tags_go_out_in_the_line_and_come_back_on_import() {
    let dir = tempfile::tempdir().expect("tempdir");
    let source = source_store(dir.path());
    let journals = dir.path().join("journals");
    written(checkpoint(&source, RepoClass::Own, &journals, WAIT).expect("written"));
    let line = journal_lines(&journals.join("b2.jsonl")).remove(0);
    assert!(
        line.contains(r#","tags":["item:wi-000002"],"data":"#),
        "the caller's tag only, before data: {line}"
    );

    let target = Store::open(&dir.path().join("target.sqlite3"), Wait::Command).expect("opens");
    let outcome = import_text(&target, &format!("{line}\n"));
    assert_eq!((outcome.imported, outcome.malformed), (1, 0));
    let stored = all(&target);
    let fact = stored
        .iter()
        .find(|s| s.event_type == "weighin.item_raised")
        .expect("the fact");
    assert_eq!(
        tags(fact),
        [
            "item:wi-000002",
            "plugin:weighin",
            "session:b2",
            "trust:asserted"
        ]
    );
    assert!(
        !String::from_utf8_lossy(&fact.data).contains("\"tags\""),
        "tags live in the store's tags, not the bytes"
    );
    let source_fact = all(&source)
        .into_iter()
        .find(|s| s.id == fact.id)
        .expect("the source's fact");
    assert_eq!(
        tags(fact),
        tags(&source_fact),
        "the same tags as the source"
    );
}

#[test]
fn a_line_with_no_tags_reads_as_before() {
    let dir = tempfile::tempdir().expect("tempdir");
    let source = source_store(dir.path());
    let journals = dir.path().join("journals");
    written(checkpoint(&source, RepoClass::Own, &journals, WAIT).expect("written"));
    let line = journal_lines(&journals.join("b2.jsonl"))
        .remove(0)
        .replace(r#""tags":["item:wi-000002"],"#, "");
    assert!(!line.contains("\"tags\""), "an older writer's line: {line}");

    let target = Store::open(&dir.path().join("target.sqlite3"), Wait::Command).expect("opens");
    let outcome = import_text(&target, &format!("{line}\n"));
    assert_eq!((outcome.imported, outcome.malformed), (1, 0));
    let stored = all(&target);
    let fact = stored
        .iter()
        .find(|s| s.event_type == "weighin.item_raised")
        .expect("the fact");
    assert_eq!(
        tags(fact),
        ["plugin:weighin", "session:b2", "trust:asserted"],
        "Core's tags only"
    );
}

#[test]
fn a_line_whose_tags_name_a_key_core_sets_is_malformed() {
    let dir = tempfile::tempdir().expect("tempdir");
    let source = source_store(dir.path());
    let journals = dir.path().join("journals");
    written(checkpoint(&source, RepoClass::Own, &journals, WAIT).expect("written"));
    let line = journal_lines(&journals.join("b2.jsonl")).remove(0).replace(
        r#""tags":["item:wi-000002"]"#,
        r#""tags":["trust:witnessed"]"#,
    );

    let target = Store::open(&dir.path().join("target.sqlite3"), Wait::Command).expect("opens");
    let outcome = import_text(&target, &format!("{line}\n"));
    assert_eq!(
        (outcome.imported, outcome.malformed),
        (0, 1),
        "a line cannot set its own trust tag"
    );
}

/// `line` with its type in a namespace a later SPEC minor may register, and another origin.
fn newer(line: &str, origin: &str) -> String {
    let (before, after) = line.split_once("\"origin\":\"").expect("an origin");
    let (_, rest) = after.split_once('"').expect("its end");
    format!("{before}\"origin\":\"{origin}\"{rest}")
        .replace("weighin.item_raised", "assay.run_opened")
}

#[test]
fn a_line_of_a_namespace_this_build_does_not_know_is_imported() {
    let dir = tempfile::tempdir().expect("tempdir");
    let source = source_store(dir.path());
    let journals = dir.path().join("journals");
    written(checkpoint(&source, RepoClass::Own, &journals, WAIT).expect("written"));
    let line = newer(
        &journal_lines(&journals.join("b2.jsonl")).remove(0),
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:9",
    );

    let target = Store::open(&dir.path().join("target.sqlite3"), Wait::Command).expect("opens");
    let outcome = import_text(&target, &format!("{line}\n"));
    assert_eq!((outcome.imported, outcome.malformed), (1, 0), "JR-10");
    let stored = all(&target);
    let fact = stored
        .iter()
        .find(|s| s.event_type == "assay.run_opened")
        .expect("stored under its own type");
    assert!(
        tags(fact).contains(&"plugin:assay".to_owned()),
        "{:?}",
        tags(fact)
    );
}

#[test]
fn a_last_line_of_a_namespace_this_build_does_not_know_is_kept_not_cut() {
    let dir = tempfile::tempdir().expect("tempdir");
    let source = source_store(dir.path());
    let journals = dir.path().join("journals");
    written(checkpoint(&source, RepoClass::Own, &journals, WAIT).expect("written"));
    let path = journals.join("b2.jsonl");
    let line = newer(
        &journal_lines(&path).remove(0),
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:9",
    );
    let mut bytes = std::fs::read(&path).expect("read");
    bytes.extend_from_slice(line.as_bytes());
    std::fs::write(&path, &bytes).expect("a sibling's line, its newline not yet written");

    let again = written(checkpoint(&source, RepoClass::Own, &journals, WAIT).expect("written"));
    assert_eq!(again.appended, 0, "nothing missing");
    bytes.push(b'\n');
    assert_eq!(
        std::fs::read(&path).expect("read"),
        bytes,
        "kept and ended, never taken for a torn line"
    );
}
