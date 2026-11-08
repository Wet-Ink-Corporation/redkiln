//! Journal goldens (`spec/fixtures/journal/`) through Core's import and export: JR-02, JR-03,
//! JR-10 to JR-12, EV-08 and EV-10. A valid line imports to its `.imported.json` golden and
//! exports again to its `.reexported.jsonl` golden, which is a fixed point of the two.

use redkiln_core::envelope::decode_all;
use redkiln_core::ids::parse_event_ref;
use redkiln_core::journal::{ImportReport, ImportSource, LineError, encode_line, import};
use redkiln_core::store::{Store, Stored, Tag};

use super::schema::{envelope_conforms, lines_conform};
use super::{ItemRaised, all, fresh_store, golden, one_line, recording, value_of};

/// `bytes` imported into `store` as a journal file, recorded with one fixed path.
fn import_into(store: &Store, bytes: &[u8]) -> ImportReport {
    let source = ImportSource::Path("journal/golden.jsonl".to_owned());
    import(store, bytes, recording(source)).expect("the import runs")
}

/// Whether every line of a journal's `bytes` imports into a fresh store.
pub(super) fn imports_whole(bytes: &[u8]) -> bool {
    let (_dir, store) = fresh_store();
    let report = import_into(&store, bytes);
    report.read > 0 && report.malformed == 0 && report.imported == report.read
}

/// The event `store` holds under the one-line journal golden `rel`'s `origin`.
fn stored_under_origin(store: &Store, rel: &str) -> Stored {
    let line = value_of(&one_line(rel));
    let origin = parse_event_ref(line["origin"].as_str().expect("an origin")).expect("a ref");
    all(store)
        .into_iter()
        .find(|event| event.id == origin)
        .unwrap_or_else(|| panic!("{rel} is stored under its origin"))
}

/// `rel` imported alone into a fresh store: its report, and the store.
fn imported_alone(rel: &str) -> (tempfile::TempDir, Store, ImportReport) {
    let (dir, store) = fresh_store();
    let report = import_into(&store, &golden(rel));
    (dir, store, report)
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

const LINES: [&str; 3] = ["jr-02-item-raised", "ev-01-exported", "tagged"];

#[test]
fn valid_lines_import_and_export_to_their_goldens() {
    for name in LINES {
        let line = format!("journal/valid/{name}.jsonl");
        let (_dir, store, report) = imported_alone(&line);
        assert_eq!((report.read, report.imported), (1, 1), "{name}");
        let stored = stored_under_origin(&store, &line);
        assert_eq!(
            text(&stored.data),
            text(&one_line(&format!("journal/valid/{name}.imported.json"))),
            "JR-12: {name} is stored asserted, its level claimed, with no origin"
        );
        envelope_conforms(&stored.data);
        let exported = encode_line(&stored).expect("exports");
        assert_eq!(
            text(&exported),
            text(&golden(&format!("journal/valid/{name}.reexported.jsonl"))),
            "{name}"
        );
        lines_conform(&exported);
    }
}

#[test]
fn a_reexported_line_is_a_fixed_point() {
    for name in LINES {
        let rel = format!("journal/valid/{name}.reexported.jsonl");
        let (_dir, store, report) = imported_alone(&rel);
        assert_eq!(report.imported, 1, "{name}");
        let again = encode_line(&stored_under_origin(&store, &rel)).expect("exports");
        assert_eq!(text(&again), text(&golden(&rel)), "{name}");
    }
}

#[test]
fn tagged_line_restores_caller_tags_and_exports_them_sorted() {
    let rel = "journal/valid/tagged.jsonl";
    let (_dir, store, _) = imported_alone(rel);
    let stored = stored_under_origin(&store, rel);
    let mut tags: Vec<&str> = stored.tags.iter().map(Tag::as_str).collect();
    tags.sort_unstable();
    assert_eq!(
        tags,
        [
            "item:wi-3fa9c01b2e",
            "plugin:weighin",
            "session:8b6fb94b-0975-5275-b434-6c9e60544265",
            "trust:asserted",
            "weighin.channel:request",
            "worktree:wt-5d41402abc4b",
        ]
    );
    let exported = text(&encode_line(&stored).expect("exports"));
    assert!(
        exported.contains(r#","tags":["item:wi-3fa9c01b2e","weighin.channel:request"],"data":"#),
        "{exported}"
    );
}

#[test]
fn foreign_and_unusable_events_are_skipped_and_counted() {
    let (_dir, store, report) = imported_alone("journal/scenario/ev-08-foreign-and-unusable.jsonl");
    assert_eq!((report.read, report.imported, report.malformed), (3, 3, 0));
    let decoded = decode_all::<ItemRaised>(&all(&store));
    assert_eq!(decoded.events.len(), 1, "the known version");
    assert_eq!(decoded.unusable, 1, "EV-10: version 2 is counted");
    assert_eq!(
        decoded.foreign, 2,
        "EV-08: the bench event and the import's own record"
    );
}

#[test]
fn malformed_lines_are_reported_by_number() {
    let (_dir, _store, report) = imported_alone("journal/scenario/jr-10-malformed.jsonl");
    assert_eq!(
        (
            report.read,
            report.imported,
            report.present,
            report.malformed
        ),
        (5, 2, 0, 3)
    );
    let numbers: Vec<u64> = report.problems.iter().map(|p| p.line).collect();
    assert_eq!(numbers, [2, 3, 4]);
    let [not_json, no_origin, core_tag] = [0, 1, 2].map(|i| &report.problems[i].reason);
    assert!(matches!(not_json, LineError::Envelope(_)), "{not_json:?}");
    assert!(matches!(no_origin, LineError::NoOrigin), "{no_origin:?}");
    assert!(matches!(core_tag, LineError::CallerTag(_)), "{core_tag:?}");
}

#[test]
fn torn_tail_and_crlf_are_tolerated() {
    let lf = golden("journal/scenario/jr-03-torn-tail.jsonl");
    assert!(!lf.ends_with(b"\n"), "the last line is torn");
    let crlf = text(&lf).replace('\n', "\r\n").into_bytes();
    let mut stored = Vec::new();
    for bytes in [lf, crlf] {
        let (_dir, store) = fresh_store();
        let report = import_into(&store, &bytes);
        assert_eq!((report.read, report.imported), (2, 2));
        assert!(report.incomplete, "JR-03: the torn tail is passed over");
        assert_eq!(report.malformed, 0);
        stored.push(all(&store).into_iter().map(|e| e.data).collect::<Vec<_>>());
    }
    assert_eq!(stored[0], stored[1], "CRLF reads as LF");
}

#[test]
fn lenient_lines_import() {
    let (_dir, store, report) = imported_alone("journal/lenient/future-namespace.jsonl");
    assert_eq!(
        report.imported, 1,
        "JR-10: an unregistered namespace is imported"
    );
    let stored = stored_under_origin(&store, "journal/lenient/future-namespace.jsonl");
    assert_eq!(stored.event_type, "acme.thing_done");
    assert!(
        stored.tags.iter().any(|t| t.as_str() == "plugin:acme"),
        "{:?}",
        stored.tags
    );
}

/// Whether a reason is the one a fixture expects.
type Reason = fn(&LineError) -> bool;

/// Each invalid line, and the reason Core's import gives for skipping it.
const INVALID: &[(&str, Reason)] = &[
    ("no-origin.jsonl", |e| matches!(e, LineError::NoOrigin)),
    ("tags-core-key.jsonl", |e| {
        matches!(e, LineError::CallerTag(_))
    }),
    ("tags-not-list.jsonl", |e| {
        matches!(e, LineError::Envelope(_))
    }),
    ("unsorted-tags.jsonl", |e| {
        matches!(e, LineError::Envelope(_))
    }),
    ("duplicate-tags.jsonl", |e| {
        matches!(e, LineError::Envelope(_))
    }),
    ("tags-too-many.jsonl", |e| {
        matches!(e, LineError::TooManyTags { count: 65 })
    }),
    ("origin-past-max.jsonl", |e| {
        matches!(
            e,
            LineError::PositionOutOfRange {
                position: 9_223_372_036_854_775_808
            }
        )
    }),
];

#[test]
fn invalid_lines_are_malformed_with_their_reason() {
    for (name, expected) in INVALID {
        let (_dir, _store, report) = imported_alone(&format!("journal/invalid/{name}"));
        assert_eq!((report.malformed, report.imported), (1, 0), "{name}");
        let [problem] = report.problems.as_slice() else {
            panic!("{name}: one problem");
        };
        assert!(expected(&problem.reason), "{name}: {:?}", problem.reason);
    }
}
