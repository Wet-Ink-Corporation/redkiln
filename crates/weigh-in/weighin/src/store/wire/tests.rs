use std::num::NonZeroUsize;

use redkiln_core::envelope::{Class, TypeName};
use redkiln_core::store::{Store, Wait};
use serde_json::{Value, json};
use weighin_contract::names::ITEM_EVENTS;
use weighin_core::event::EVENT_TYPES;

use super::*;

const TS: &str = "2026-10-02T21:40:02Z";
const SESSION: &str = "8b6fb94b-0975-5275-b434-6c9e60544265";
const WORKTREE: &str = "wt-5d41402abc4b";
const HEAD: &str = "bd234009c1e5a7d4f1c3b2a0e9f8d7c6b5a49382";

/// A 1.x flat envelope, exactly as weighin 1.x stored and journals it, of `event` (its `type`
/// and payload) with every header key set.
fn flat(event: &Value) -> EventEnvelope {
    let mut object = event.clone();
    let header = json!({
        "actor": "cli", "session_id": SESSION, "worktree": WORKTREE,
        "branch": "main", "head": HEAD, "ts": TS,
    });
    if let (Some(object), Some(header)) = (object.as_object_mut(), header.as_object()) {
        object.extend(header.clone());
    }
    serde_json::from_value(object).expect("a 1.x envelope")
}

fn raise_json() -> Value {
    json!({
        "type": "item_raised", "item_id": "wi-930e15", "kind": "question",
        "question": "Which region hosts the backup?", "did": null, "flip": null,
        "at": "src/backup.rs:12", "sym": null, "blocks": null, "door_hint": "one-way",
        "authority_hint": "authority", "channel": "marker",
        "quote": "Which region hosts the backup?", "source_path": null, "source_line": null,
    })
}

/// One event of each of the 13 types, every optional field set where it has one.
fn one_of_each() -> Vec<EventEnvelope> {
    let id = "wi-930e15";
    let card = json!({
        "why": "It fixes the restore path.", "timing": "Decide now.",
        "options": [{"id": "A", "title": "eu-west"}, {"id": "B", "title": "us-east"}],
        "recommendation": {"option": "A"}
    });
    [
        raise_json(),
        json!({"type": "item_triaged", "item_id": id, "door": "one-way", "blast": "system",
               "authority": "authority", "route": "present",
               "timing": {"verdict": "now", "flip": "f", "obtainable_now": true,
                          "arrives_after_expiry": false, "cost_of_waiting": "c",
                          "options_close": true},
               "resolve_by": "2026-11-01", "reason": "r"}),
        json!({"type": "item_grounded", "item_id": id, "card": card, "reason": "r"}),
        json!({"type": "item_presented", "item_id": id, "surface": "chat",
               "tree_hash": "abc", "reason": "r"}),
        json!({"type": "item_decided", "item_id": id, "option": "A", "decider": "human:ryan",
               "provenance": "user-approved", "note": "n", "tree_hash": "abc",
               "matched_recommendation": true, "via": "chat", "seconds_to_decide": 5,
               "reason": "r"}),
        json!({"type": "item_deferred", "item_id": id, "flip": "f", "trigger": "t",
               "resolve_by": "2026-11-01", "default_option": "A", "owner": "ryan",
               "unqualified": true, "reason": "r"}),
        json!({"type": "item_defaulted", "item_id": id, "option": "A", "cause": "expired",
               "reason": "r"}),
        json!({"type": "item_dismissed", "item_id": id,
               "why": {"kind": "duplicate-of", "ref": "wi-bee566"}, "note": "n", "reason": "r"}),
        json!({"type": "item_reopened", "item_id": id, "note": "n", "reason": "r"}),
        json!({"type": "item_superseded", "item_id": id, "by": "kb-0001", "reason": "r"}),
        json!({"type": "item_linked", "item_id": id, "kind": "relates", "target": "wi-bee566",
               "reason": "r"}),
        json!({"type": "record_written", "item_id": id, "path": ".kb/x.md",
               "atom_id": "kb-decision-1", "atom_kind": "decision", "reason": "r"}),
        json!({"type": "item_acknowledged", "item_id": id, "reason": "r"}),
    ]
    .iter()
    .map(flat)
    .collect()
}

/// `envs` appended to a new store through [`to_new_event`], and the store read back.
fn stored(envs: Vec<EventEnvelope>) -> Vec<Stored> {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&dir.path().join("events.sqlite3"), Wait::Command).expect("store");
    let writer = writer().expect("writer");
    let events = envs
        .into_iter()
        .map(|env| to_new_event(env, &writer).expect("an envelope").0)
        .collect();
    store.append(events, None).expect("append");
    store
        .read(&Filter::all(), 0, NonZeroUsize::new(100).expect("non-zero"))
        .expect("read")
}

#[test]
fn the_table_is_weighin_cores_types_in_the_weighin_namespace_at_version_one_and_durable() {
    let expected: Vec<String> = EVENT_TYPES.iter().map(|t| format!("weighin.{t}")).collect();

    assert_eq!(TYPES.to_vec(), expected);
    assert_eq!(DECLARED.map(|(ty, ..)| ty), TYPES);
    assert_eq!(ITEM_EVENTS.map(|e| e.name), TYPES, "the contract's table");
    for (ty, version, class) in DECLARED {
        let name = TypeName::parse(ty).expect("a valid type name");
        assert_eq!(
            (name.namespace(), version, class),
            (Namespace::Weighin, 1, Class::Durable),
            "{ty}"
        );
    }
}

#[test]
fn a_raise_is_stored_as_an_envelope_with_its_payload_under_data_and_its_tags() {
    let read = stored(vec![flat(&raise_json())]);

    let [event] = read.as_slice() else {
        panic!("one event: {read:?}");
    };
    let golden = format!(
        r#"{{"type":"weighin.item_raised","version":1,"ts":"{TS}","actor":"cli","trust":{{"level":"asserted"}},"writer":{{"binary":"weighin","version":"{}","spec":"1.0.0"}},"session":"{SESSION}","worktree":"{WORKTREE}","branch":"main","head":"{HEAD}","data":{{"item_id":"wi-930e15","kind":"question","question":"Which region hosts the backup?","did":null,"flip":null,"at":"src/backup.rs:12","sym":null,"blocks":null,"door_hint":"one-way","authority_hint":"authority","channel":"marker","quote":"Which region hosts the backup?","source_path":null,"source_line":null}}}}"#,
        env!("CARGO_PKG_VERSION")
    );
    assert_eq!(String::from_utf8_lossy(&event.data), golden);
    assert_eq!(event.event_type, "weighin.item_raised");
    let mut tags: Vec<&str> = event.tags.iter().map(Tag::as_str).collect();
    tags.sort_unstable();
    let session = format!("session:{SESSION}");
    let worktree = format!("worktree:{WORKTREE}");
    let mut expected = vec![
        "item:wi-930e15",
        "plugin:weighin",
        session.as_str(),
        "trust:asserted",
        "weighin.channel:marker",
        "weighin.door:one-way",
        "weighin.kind:question",
        worktree.as_str(),
    ];
    expected.sort_unstable();
    assert_eq!(tags, expected);
}

#[test]
fn every_type_reads_back_as_the_envelope_that_was_written() {
    let written = one_of_each();
    assert_eq!(
        written
            .iter()
            .map(|e| e.event.event_type())
            .collect::<Vec<_>>(),
        EVENT_TYPES,
        "one value per type, in the table's order"
    );

    let read: Vec<EventEnvelope> = stored(written.clone())
        .iter()
        .map(|s| from_stored(s).expect("usable"))
        .collect();

    assert_eq!(read, written);
}

#[test]
fn a_field_core_refuses_is_an_error_naming_it() {
    let writer = writer().expect("writer");
    let with = |key: &str, value: Value| {
        let mut env = serde_json::to_value(flat(&raise_json())).expect("json");
        env[key] = value;
        serde_json::from_value::<EventEnvelope>(env).expect("a 1.x envelope")
    };
    let long_session = "s".repeat(300);
    for (key, value, named) in [
        ("actor", json!("t"), "actor"),
        ("ts", json!("2026-10-02T21:40:02.123456789Z"), "ts"),
        ("worktree", json!("redkiln"), "worktree"),
        ("session_id", json!(long_session), "session"),
    ] {
        let err = to_new_event(with(key, value), &writer).expect_err(key);

        assert_eq!(err.to_string(), format!("the event's {named}"), "{err:#}");
    }
}

#[test]
fn an_item_id_core_refuses_as_a_tag_is_an_error_naming_the_tag_never_cut() {
    // weighin's own tag values are bounded (item ids, enum names), so EV-11's "too long" is
    // reached only through Core's `session` tag (the case above); an item id read from 1.x
    // JSON is any string, and one Core does not take as an item id is refused whole.
    let writer = writer().expect("writer");
    let long_id = format!("wi-{}", "a".repeat(300));
    let mut raise = raise_json();
    raise["item_id"] = json!(long_id);

    let err = to_new_event(flat(&raise), &writer).expect_err("a 303-byte item id");

    assert_eq!(err.to_string(), format!("tagging the event item:{long_id}"));
    assert!(
        format!("{err:#}").contains("not a valid item tag value"),
        "{err:#}"
    );
}

#[test]
fn another_namespace_is_foreign() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&dir.path().join("events.sqlite3"), Wait::Command).expect("store");
    let bench = NewEvent::new("bench.run_finished", b"{}".to_vec()).expect("a valid type");
    store.append(vec![bench], None).expect("append");
    let read = store
        .read(&Filter::all(), 0, NonZeroUsize::MIN)
        .expect("read");

    let [event] = read.as_slice() else {
        panic!("one event: {read:?}");
    };
    assert!(matches!(from_stored(event), Err(Skip::Foreign)));
}

/// The names in `dir` with a digest of each file's bytes, to show a read changed nothing.
fn listing(dir: &std::path::Path) -> Vec<(String, Option<redkiln_core::ids::Digest>)> {
    let mut files = std::fs::read_dir(dir)
        .expect("the ledger's directory")
        .map(|entry| {
            let entry = entry.expect("an entry");
            // A directory beside the ledger (1.x's state files) is listed by name only.
            let file = entry.file_type().expect("an entry's type").is_file();
            let digest = file.then(|| {
                redkiln_core::ids::Digest::sha256(&std::fs::read(entry.path()).expect("a file"))
            });
            (entry.file_name().to_string_lossy().into_owned(), digest)
        })
        .collect::<Vec<_>>();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}

/// Step 8's mapping covers real data: every event of the 1.x ledger `WEIGHIN_LIVE_LEDGER`
/// names reads from a copy, decodes as a 1.x envelope and, its 1.x worktree (a directory name)
/// dropped, maps through the wire; the ledger and its directory are left as they were. Here
/// rather than in `tests/it/`, which cannot reach the binary's `wire`.
#[test]
#[ignore = "manual (#17): reads the 1.x ledger WEIGHIN_LIVE_LEDGER names"]
fn a_live_ledger_reads_untouched_and_every_event_maps_through_the_wire() {
    let path = std::env::var_os("WEIGHIN_LIVE_LEDGER")
        .map(std::path::PathBuf::from)
        .expect("WEIGHIN_LIVE_LEDGER names a 1.x ledger");
    let dir = path.parent().expect("the ledger is in a directory");
    let before = listing(dir);
    let writer = writer().expect("the writer");

    let ledger = redkiln_core::legacy::read_ledger(&path).expect("the 1.x ledger reads");

    let mut refused = Vec::new();
    for event in &ledger.events {
        let mut envelope: EventEnvelope = serde_json::from_slice(&event.data)
            .unwrap_or_else(|e| panic!("event {} is a 1.x envelope: {e}", event.position));
        envelope.worktree = None;
        if let Err(e) = to_new_event(envelope, &writer) {
            refused.push(format!("{} {}: {e:#}", event.position, event.event_type));
        }
    }
    assert!(!ledger.events.is_empty(), "the ledger has events");
    assert_eq!(refused, Vec::<String>::new(), "events the wire refuses");
    assert_eq!(listing(dir), before, "the ledger's directory is unchanged");
}

/// A JR-02 line of `event_type` at `version`, with an `origin` when `origin`, no `\n`.
fn journal_line(event_type: &str, version: u32, origin: bool) -> String {
    let origin = if origin {
        r#""origin":"0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:7","#
    } else {
        ""
    };
    format!(
        r#"{{"type":"{event_type}","version":{version},"ts":"{TS}","actor":"agent","trust":{{"level":"asserted"}},"writer":{{"binary":"weighin","version":"2.0.0","spec":"1.0.0"}},{origin}"data":{{"item_id":"wi-930e15"}}}}"#
    )
}

#[test]
fn a_weighin_event_exported_as_a_line_reads_back_as_the_same_ledger_event() {
    let written = one_of_each();

    let read: Vec<EventEnvelope> = stored(written.clone())
        .iter()
        .zip(1_u64..)
        .map(|(stored, number)| {
            let line = redkiln_core::journal::encode_line(stored).expect("a line");
            let text = line.strip_suffix(b"\n").expect("ended");
            from_line(number, text).expect("usable")
        })
        .collect();

    assert_eq!(read, written);
}

#[test]
fn a_line_of_another_namespace_is_foreign() {
    let line = journal_line("bench.run_finished", 1, true);

    let skipped = from_line(1, line.as_bytes());

    assert!(matches!(skipped, Err(LineSkip::Foreign)), "{skipped:?}");
}

#[test]
fn a_weighin_line_of_a_newer_version_is_unusable_naming_its_type() {
    let line = journal_line("weighin.item_raised", 2, true);

    let skipped = from_line(1, line.as_bytes());

    assert!(
        matches!(&skipped, Err(LineSkip::Unusable { event_type }) if event_type == "weighin.item_raised"),
        "{skipped:?}"
    );
}

#[test]
fn a_line_with_no_origin_is_malformed() {
    let no_origin = journal_line("weighin.item_raised", 1, false);

    for text in [no_origin.as_bytes(), b"not an envelope".as_slice()] {
        let skipped = from_line(1, text);
        assert!(matches!(skipped, Err(LineSkip::Malformed)), "{skipped:?}");
    }
}
