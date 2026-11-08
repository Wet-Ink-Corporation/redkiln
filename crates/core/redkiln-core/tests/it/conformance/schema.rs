//! The JSON Schemas (`spec/schemas/`) against the goldens and against Core's registries.
//!
//! A schema describes what a writer writes (EV-05): `valid/` goldens pass it, `invalid/`
//! fixtures fail it at exactly the `(instance path, keyword)` pairs their row lists, and
//! `lenient/` fixtures fail it while Core reads them (EV-01 `null`, EV-09, JR-10). A fixture
//! that passes its schema but that Core refuses shows what JSON cannot say (a key named twice,
//! a position past 2^63 - 1, tags out of order). The tables here are the index of every fixture
//! file.

use std::collections::BTreeSet;

use redkiln_core::envelope::Namespace;
use redkiln_core::journal::ImportCompleted;
use redkiln_core::store::CoreKey;
use redkiln_core::subject::Method;
use redkiln_core::trust::{Level, Trust, Via};
use serde_json::Value;

use super::{golden, spec_dir, value_of};

/// What Core makes of a fixture its schema refuses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Reader {
    /// Core refuses it where it reads that shape.
    Refuses,
    /// No Core code reads this shape (the CLI envelope, decision requests, approval data).
    NotCoreShape,
}

/// `(instance path, keyword)` pairs a fixture breaks its schema at.
type Expected = &'static [(&'static str, &'static str)];

/// Every schema, by stem.
const SCHEMAS: &[&str] = &[
    "envelope-1",
    "journal-line-1",
    "core.approval_recorded-1",
    "core.import_completed-1",
    "decision-request-1",
    "cli-envelope-1",
];

/// Valid goldens and the schema each passes; a `.jsonl` journal is checked line by line.
const VALID: &[(&str, &str)] = &[
    ("envelope/valid/ev-01-item-decided.json", "envelope-1"),
    ("envelope/valid/minimal-asserted.json", "envelope-1"),
    ("envelope/valid/ap-01-approval-recorded.json", "envelope-1"),
    (
        "envelope/valid/approval-attested-signed-commit.json",
        "envelope-1",
    ),
    (
        "envelope/valid/approval-github-review-asserted.json",
        "envelope-1",
    ),
    ("envelope/valid/import-completed-path.json", "envelope-1"),
    ("envelope/valid/import-completed-ref.json", "envelope-1"),
    (
        "envelope/valid/import-completed-artifact.json",
        "envelope-1",
    ),
    ("envelope/valid/import-completed-legacy.json", "envelope-1"),
    ("journal/valid/jr-02-item-raised.jsonl", "journal-line-1"),
    (
        "journal/valid/jr-02-item-raised.imported.json",
        "envelope-1",
    ),
    (
        "journal/valid/jr-02-item-raised.reexported.jsonl",
        "journal-line-1",
    ),
    ("journal/valid/ev-01-exported.jsonl", "journal-line-1"),
    ("journal/valid/ev-01-exported.imported.json", "envelope-1"),
    (
        "journal/valid/ev-01-exported.reexported.jsonl",
        "journal-line-1",
    ),
    ("journal/valid/tagged.jsonl", "journal-line-1"),
    ("journal/valid/tagged.imported.json", "envelope-1"),
    ("journal/valid/tagged.reexported.jsonl", "journal-line-1"),
    ("cli/valid/gate-failed.json", "cli-envelope-1"),
    ("cli/valid/ok-with-warning.json", "cli-envelope-1"),
    ("cli/valid/internal-error.json", "cli-envelope-1"),
    ("decision-request/valid/dr-02.json", "decision-request-1"),
];

/// Fixtures a writer must not write and a reader reads: their schema, and where they break it.
const LENIENT: &[(&str, &str, Expected)] = &[
    (
        "envelope/lenient/null-optional-keys.json",
        "envelope-1",
        &[
            ("/branch", "type"),
            ("/cites", "type"),
            ("/head", "type"),
            ("/origin", "type"),
            ("/session", "type"),
            ("/trust/ref", "type"),
            ("/trust/via", "enum"),
            ("/worktree", "type"),
        ],
    ),
    (
        "envelope/lenient/unknown-keys.json",
        "envelope-1",
        &[("", "additionalProperties")],
    ),
    (
        "journal/lenient/future-namespace.jsonl",
        "journal-line-1",
        &[("/type", "pattern")],
    ),
];

/// Fixtures that break the SPEC: their schema, where they break it, and what Core makes of them.
/// An empty set is a break JSON Schema cannot see.
const INVALID: &[(&str, &str, Expected, Reader)] = &[
    (
        "envelope/invalid/data-flattened.json",
        "envelope-1",
        &[("", "additionalProperties"), ("", "required")],
        Reader::Refuses,
    ),
    (
        "envelope/invalid/version-zero.json",
        "envelope-1",
        &[("/version", "minimum")],
        Reader::Refuses,
    ),
    (
        "envelope/invalid/ts-offset.json",
        "envelope-1",
        &[("/ts", "pattern")],
        Reader::Refuses,
    ),
    (
        "envelope/invalid/trust-unknown-level.json",
        "envelope-1",
        &[("/trust/level", "enum")],
        Reader::Refuses,
    ),
    (
        "envelope/invalid/trust-witnessed-without-via.json",
        "envelope-1",
        &[("/trust", "required")],
        Reader::Refuses,
    ),
    (
        "envelope/invalid/worktree-path.json",
        "envelope-1",
        &[("/worktree", "pattern")],
        Reader::Refuses,
    ),
    (
        "envelope/invalid/cites-not-event-ref.json",
        "envelope-1",
        &[("/cites/0", "pattern")],
        Reader::Refuses,
    ),
    (
        "envelope/invalid/data-not-object.json",
        "envelope-1",
        &[("/data", "type")],
        Reader::Refuses,
    ),
    (
        "envelope/invalid/duplicate-key.json",
        "envelope-1",
        &[],
        Reader::Refuses,
    ),
    (
        "envelope/invalid-data/approval-verdict-unknown.json",
        "core.approval_recorded-1",
        &[("/verdict", "enum")],
        Reader::NotCoreShape,
    ),
    (
        "envelope/invalid-data/approval-subject-hash-wrong-method.json",
        "core.approval_recorded-1",
        &[("/subject/hash", "pattern")],
        Reader::NotCoreShape,
    ),
    (
        "envelope/invalid-data/import-source-two-kinds.json",
        "core.import_completed-1",
        &[("/source", "maxProperties")],
        Reader::Refuses,
    ),
    (
        "envelope/invalid-data/import-legacy-store-id-uppercase.json",
        "core.import_completed-1",
        &[("/source/legacy/store_id", "pattern")],
        Reader::Refuses,
    ),
    (
        "journal/invalid/no-origin.jsonl",
        "journal-line-1",
        &[("", "required")],
        Reader::Refuses,
    ),
    (
        "journal/invalid/tags-core-key.jsonl",
        "journal-line-1",
        &[("/tags/0", "not")],
        Reader::Refuses,
    ),
    (
        "journal/invalid/tags-not-list.jsonl",
        "journal-line-1",
        &[("/tags", "type")],
        Reader::Refuses,
    ),
    (
        "journal/invalid/tags-too-many.jsonl",
        "journal-line-1",
        &[("/tags", "maxItems")],
        Reader::Refuses,
    ),
    (
        "journal/invalid/origin-past-max.jsonl",
        "journal-line-1",
        &[],
        Reader::Refuses,
    ),
    (
        "journal/invalid/unsorted-tags.jsonl",
        "journal-line-1",
        &[],
        Reader::Refuses,
    ),
    (
        "journal/invalid/duplicate-tags.jsonl",
        "journal-line-1",
        &[("/tags", "uniqueItems")],
        Reader::Refuses,
    ),
    (
        "cli/invalid/ok-string.json",
        "cli-envelope-1",
        &[("/ok", "type")],
        Reader::NotCoreShape,
    ),
    (
        "cli/invalid/missing-findings.json",
        "cli-envelope-1",
        &[("", "required")],
        Reader::NotCoreShape,
    ),
    (
        "cli/invalid/schema-id-without-version.json",
        "cli-envelope-1",
        &[("/schema", "pattern")],
        Reader::NotCoreShape,
    ),
    (
        "cli/invalid/namespace-unregistered.json",
        "cli-envelope-1",
        &[("/schema", "pattern")],
        Reader::NotCoreShape,
    ),
    (
        "cli/invalid/finding-unknown-severity.json",
        "cli-envelope-1",
        &[("/findings/0/severity", "enum")],
        Reader::NotCoreShape,
    ),
    (
        "cli/invalid/finding-multiline-message.json",
        "cli-envelope-1",
        &[("/findings/0/message", "pattern")],
        Reader::NotCoreShape,
    ),
    (
        "decision-request/invalid/one-option.json",
        "decision-request-1",
        &[("/options", "minItems")],
        Reader::NotCoreShape,
    ),
    (
        "decision-request/invalid/option-id-not-a-to-d.json",
        "decision-request-1",
        &[("/options/1/id", "enum")],
        Reader::NotCoreShape,
    ),
    (
        "decision-request/invalid/door-unknown.json",
        "decision-request-1",
        &[("/door", "enum")],
        Reader::NotCoreShape,
    ),
    (
        "decision-request/invalid/default-on-one-way.json",
        "decision-request-1",
        &[("", "not")],
        Reader::NotCoreShape,
    ),
    (
        "decision-request/invalid/subject-hash-wrong-method.json",
        "decision-request-1",
        &[("/subject/hash", "pattern")],
        Reader::NotCoreShape,
    ),
];

/// Journals read only by Core's scenario tests (`journal.rs`): several lines, some malformed or
/// torn on purpose, so no one schema verdict describes them.
const SCENARIO: &[&str] = &[
    "journal/scenario/ev-08-foreign-and-unusable.jsonl",
    "journal/scenario/jr-10-malformed.jsonl",
    "journal/scenario/jr-03-torn-tail.jsonl",
];

/// Every file the tables name, as `fixtures/<rel>` or `schemas/<stem>.schema.json`.
pub(super) fn listed() -> impl Iterator<Item = String> {
    let fixtures = VALID
        .iter()
        .map(|(rel, _)| *rel)
        .chain(LENIENT.iter().map(|(rel, _, _)| *rel))
        .chain(INVALID.iter().map(|(rel, _, _, _)| *rel))
        .chain(SCENARIO.iter().copied())
        .map(|rel| format!("fixtures/{rel}"));
    let schemas = SCHEMAS
        .iter()
        .map(|stem| format!("schemas/{stem}.schema.json"));
    fixtures.chain(schemas)
}

/// `spec/schemas/<stem>.schema.json` as JSON.
fn schema(stem: &str) -> Value {
    let path = spec_dir()
        .join("schemas")
        .join(format!("{stem}.schema.json"));
    let bytes = std::fs::read(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    serde_json::from_slice(&bytes).expect("a schema is JSON")
}

/// The Draft 2020-12 validator of `<stem>`.
fn validator(stem: &str) -> jsonschema::Validator {
    jsonschema::draft202012::new(&schema(stem)).unwrap_or_else(|e| panic!("{stem} compiles: {e}"))
}

/// Where `doc` breaks `<stem>`, as `(instance path, keyword)` pairs.
fn violations(stem: &str, doc: &Value) -> BTreeSet<(String, String)> {
    validator(stem)
        .iter_errors(doc)
        .map(|error| {
            (
                error.instance_path().as_str().to_owned(),
                error.kind().keyword().to_owned(),
            )
        })
        .collect()
}

/// The documents of a golden: each line of a `.jsonl` journal, or the one JSON line.
fn documents(rel: &str) -> Vec<Value> {
    golden(rel)
        .split(|&b| b == b'\n')
        .filter(|line| !line.is_empty())
        .map(value_of)
        .collect()
}

/// Asserts `doc` passes `<stem>`, saying `what` it is when it does not.
fn assert_valid(stem: &str, doc: &Value, what: &str) {
    let found = violations(stem, doc);
    assert!(found.is_empty(), "{what} breaks {stem}: {found:?}");
}

/// Asserts an envelope's `bytes` pass `envelope-1` and, for Core's own types, their `data`
/// passes the type's schema: how every envelope Core writes in these tests is checked.
pub(super) fn envelope_conforms(bytes: &[u8]) {
    let doc = value_of(bytes);
    assert_valid("envelope-1", &doc, &String::from_utf8_lossy(bytes));
    data_conforms(&doc);
}

/// Asserts each line of a journal's `bytes` passes `journal-line-1`, and its core data its type.
pub(super) fn lines_conform(bytes: &[u8]) {
    for line in bytes.split(|&b| b == b'\n').filter(|l| !l.is_empty()) {
        let doc = value_of(line);
        assert_valid("journal-line-1", &doc, &String::from_utf8_lossy(line));
        data_conforms(&doc);
    }
}

/// For a `core.*` type, its `data` passes `<type>-1`.
fn data_conforms(doc: &Value) {
    let event_type = doc["type"].as_str().expect("a type");
    if event_type.starts_with("core.") {
        assert_valid(&format!("{event_type}-1"), &doc["data"], event_type);
    }
}

/// Whether Core reads the fixture `rel`, where Core reads that shape.
fn core_reads(rel: &str) -> Option<bool> {
    let bytes = golden(rel);
    if rel.starts_with("envelope/invalid-data/import-") {
        Some(serde_json::from_slice::<ImportCompleted>(&bytes).is_ok())
    } else if rel.starts_with("envelope/invalid-data/") {
        None
    } else if rel.starts_with("envelope/") {
        Some(super::envelope::decodes(&bytes))
    } else if rel.starts_with("journal/") {
        Some(super::journal::imports_whole(&bytes))
    } else {
        None
    }
}

/// The strings of the JSON array at `pointer` in `doc`.
fn strings_at(doc: &Value, pointer: &str) -> Vec<String> {
    doc.pointer(pointer)
        .and_then(Value::as_array)
        .unwrap_or_else(|| panic!("an array at {pointer}"))
        .iter()
        .map(|v| v.as_str().expect("a string").to_owned())
        .collect()
}

/// The string at `pointer` in `doc`.
fn string_at<'a>(doc: &'a Value, pointer: &str) -> &'a str {
    doc.pointer(pointer)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("a string at {pointer}"))
}

#[test]
fn every_schema_compiles() {
    for stem in SCHEMAS {
        let doc = schema(stem);
        assert_eq!(
            string_at(&doc, "/$schema"),
            "https://json-schema.org/draft/2020-12/schema",
            "{stem}"
        );
        let name = stem.strip_suffix("-1").expect("version 1");
        assert_eq!(
            string_at(&doc, "/$id"),
            format!("urn:redkiln:spec:{name}:1")
        );
        drop(validator(stem));
    }
}

#[test]
fn shared_defs_are_identical() {
    let envelope = schema("envelope-1");
    let line = schema("journal-line-1");
    assert_eq!(
        line["$defs"], envelope["$defs"],
        "journal-line-1 copies $defs"
    );
    let line_properties = line["properties"].as_object().expect("properties");
    let envelope_properties = envelope["properties"].as_object().expect("properties");
    assert!(
        line_properties.get("tags").is_some_and(Value::is_object),
        "a line has tags"
    );
    // serde_json's map iterates in key order, so equal maps give equal sequences.
    assert!(
        line_properties
            .iter()
            .filter(|(key, _)| *key != "tags")
            .eq(envelope_properties.iter()),
        "every other property is envelope-1's"
    );
    assert_eq!(
        schema("decision-request-1")["$defs"]["subject"],
        schema("core.approval_recorded-1")["$defs"]["subject"],
        "decision-request-1 copies AP-01's subject"
    );
}

#[test]
fn closed_lists_equal_cores_registries() {
    let namespaces: Vec<&str> = Namespace::ALL.iter().map(|n| n.as_str()).collect();
    let alternation = format!("({})\\.", namespaces.join("|"));
    for (stem, pointer) in [
        ("envelope-1", "/$defs/typeName/pattern"),
        ("journal-line-1", "/$defs/typeName/pattern"),
        ("cli-envelope-1", "/properties/schema/pattern"),
        (
            "core.approval_recorded-1",
            "/properties/gates/items/pattern",
        ),
        (
            "core.approval_recorded-1",
            "/$defs/subject/properties/kind/pattern",
        ),
    ] {
        let doc = schema(stem);
        let pattern = string_at(&doc, pointer);
        assert!(pattern.contains(&alternation), "{stem}{pointer}: {pattern}");
    }
    assert_eq!(
        strings_at(&schema("decision-request-1"), "/$defs/namespace/enum"),
        namespaces
    );
    let core_set: Vec<&str> = CoreKey::ALL
        .iter()
        .filter(|k| k.set_by_core())
        .map(|k| k.as_str())
        .collect();
    assert_eq!(
        string_at(
            &schema("journal-line-1"),
            "/properties/tags/items/not/pattern"
        ),
        format!("^({}):", core_set.join("|"))
    );
}

#[test]
fn trust_lists_equal_cores() {
    let envelope = schema("envelope-1");
    let vias: Vec<&str> = Via::ALL.iter().map(|v| v.as_str()).collect();
    assert_eq!(strings_at(&envelope, "/$defs/via/enum"), vias);
    for level in strings_at(&envelope, "/$defs/level/enum") {
        let parsed: Level = level.parse().expect("a level Core knows");
        assert_eq!(parsed.as_str(), level);
    }
    let levels = [Level::Asserted, Level::Witnessed, Level::Attested];
    assert_eq!(
        strings_at(&envelope, "/$defs/level/enum").len(),
        levels.len()
    );
    // TR-03, TR-04, TR-10: the vias that may back each level, as `Trust::new` allows them.
    for (index, level) in [(0, Level::Witnessed), (1, Level::Attested)] {
        let backing: Vec<&str> = Via::ALL
            .iter()
            .filter(|v| Trust::new(level, Some(**v)).is_ok())
            .map(|v| v.as_str())
            .collect();
        let pointer = format!("/$defs/trust/allOf/{index}/then/properties/via/enum");
        assert_eq!(strings_at(&envelope, &pointer), backing, "{level}");
        let when = format!("/$defs/trust/allOf/{index}/if/properties/level/const");
        assert_eq!(string_at(&envelope, &when), level.as_str());
    }
}

#[test]
fn hash_methods_equal_cores() {
    let approval = schema("core.approval_recorded-1");
    let methods: Vec<&str> = Method::ALL.iter().map(|m| m.as_str()).collect();
    assert_eq!(
        strings_at(&approval, "/$defs/subject/properties/method/enum"),
        methods
    );
    let rules = approval
        .pointer("/$defs/subject/allOf")
        .and_then(Value::as_array)
        .expect("one rule per method");
    let methods_ruled: Vec<&str> = rules
        .iter()
        .map(|rule| string_at(rule, "/if/properties/method/const"))
        .collect();
    assert_eq!(methods_ruled, methods);
    for rule in rules {
        let method = string_at(rule, "/if/properties/method/const");
        let hash = string_at(rule, "/then/properties/hash/pattern");
        assert!(hash.starts_with(&format!("^{method}:")), "{method}: {hash}");
    }
}

#[test]
fn valid_goldens_pass() {
    for (rel, stem) in VALID {
        for doc in documents(rel) {
            assert_valid(stem, &doc, rel);
            if *stem != "cli-envelope-1" && *stem != "decision-request-1" {
                data_conforms(&doc);
            }
        }
    }
}

#[test]
fn lenient_fixtures_break_the_schema_and_core_reads_them() {
    for (rel, stem, expected) in LENIENT {
        let [doc] = documents(rel).try_into().expect("one document");
        assert_eq!(violations(stem, &doc), owned(expected), "{rel}");
        assert_eq!(core_reads(rel), Some(true), "Core reads {rel}");
    }
}

#[test]
fn invalid_fixtures_fail_at_their_listed_paths() {
    for (rel, stem, expected, reader) in INVALID {
        let [doc] = documents(rel).try_into().expect("one document");
        assert_eq!(violations(stem, &doc), owned(expected), "{rel}");
        let read = core_reads(rel);
        match reader {
            Reader::Refuses => assert_eq!(read, Some(false), "Core refuses {rel}"),
            Reader::NotCoreShape => assert_eq!(read, None, "{rel}"),
        }
    }
}

#[test]
fn reader_only_fixtures_pass_the_schema_and_core_refuses_them() {
    let reader_only: Vec<&str> = INVALID
        .iter()
        .filter(|(_, _, expected, _)| expected.is_empty())
        .map(|(rel, _, _, reader)| {
            assert_eq!(*reader, Reader::Refuses, "{rel}");
            *rel
        })
        .collect();
    assert_eq!(
        reader_only,
        [
            "envelope/invalid/duplicate-key.json",
            "journal/invalid/origin-past-max.jsonl",
            "journal/invalid/unsorted-tags.jsonl",
        ]
    );
}

fn owned(expected: Expected) -> BTreeSet<(String, String)> {
    expected
        .iter()
        .map(|(path, keyword)| ((*path).to_owned(), (*keyword).to_owned()))
        .collect()
}
