use serde_json::json;

use super::{Listed, ListingError, read_listing};
use crate::event::Applies;
use crate::id::ItemId;
use crate::open::{OpenItem, Status};

/// The `(id, question, label)` of each decided item in `listing`, as D6 offers them.
fn decided(listing: &[Listed]) -> Vec<(String, String, String)> {
    listing
        .iter()
        .filter(|item| item.status == Some(Status::Decided))
        .filter_map(|item| {
            let label = item.decided_label()?;
            Some((
                item.id.as_str().to_owned(),
                item.question.clone(),
                label.to_owned(),
            ))
        })
        .collect()
}

fn read(value: &serde_json::Value) -> Result<Vec<Listed>, ListingError> {
    read_listing(value.to_string().as_bytes(), 4096)
}

#[test]
fn only_decided_items_with_a_weigh_in_id_are_read() {
    let listed = json!([
        {"id": "wi-e41e02", "kind": "question", "status": "decided",
         "question": "Should the cache be shared across tenants?",
         "decision": {"option": "shared", "decider": "human:ryan", "note": "test"},
         "deferral": null, "records": []},
        {"id": "wi-b6ea09", "status": "deferred", "question": "Second q?", "decision": null},
        {"id": "wi-58067d", "status": "decided", "question": "No decision?"},
        {"id": "not-wi", "status": "decided", "question": "Q", "decision": {"option": "x"}}
    ]);

    let found = read(&listed).expect("an array");

    assert_eq!(
        decided(&found),
        [(
            "wi-e41e02".to_owned(),
            "Should the cache be shared across tenants?".to_owned(),
            "shared".to_owned(),
        )]
    );
}

#[test]
fn an_empty_list_has_no_rulings_and_a_non_list_is_an_error() {
    assert_eq!(read_listing(b"[]", 4096).expect("an array"), []);
    let error = read_listing(b"{\"id\": \"wi-1\"}", 4096).expect_err("an object is not a list");
    let ListingError::Parse(error) = error else {
        panic!("not a parse error: {error:?}");
    };
    assert_eq!(error.classify(), serde_json::error::Category::Data);
}

#[test]
fn read_listing_ignores_keys_it_does_not_know() {
    let listed = json!([
        {"id": "wi-e41e02", "status": "decided", "question": "Shared?", "added_in_3_0": {"x": 1},
         "decision": {"option": "A", "applies": "always", "later": true},
         "card": {"options": [{"id": "A", "title": "Shared", "new_key": 1}], "new": []}}
    ]);

    let found = read(&listed).expect("unknown keys are ignored");

    let [item] = found.as_slice() else {
        panic!("one item: {found:?}");
    };
    assert_eq!(item.id.as_str(), "wi-e41e02");
    assert_eq!(
        item.decision.as_ref().and_then(|d| d.applies),
        Some(Applies::Always)
    );
    assert_eq!(item.decided_label(), Some("Shared"));
}

#[test]
fn read_listing_skips_an_item_whose_id_or_shape_does_not_read() {
    let listed = json!([
        {"id": "wi-XYZ", "status": "decided"},
        {"status": "decided"},
        {"id": 7},
        "not an object",
        {"id": "wi-e41e02", "status": "decided", "decision": {"option": 3}},
        {"id": "wi-b6ea09", "status": "deferred"}
    ]);

    let found = read(&listed).expect("an array");

    let ids: Vec<&str> = found.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(ids, ["wi-b6ea09"]);
}

#[test]
fn an_unknown_status_or_applies_word_reads_as_none_not_a_failure() {
    let listed = json!([
        {"id": "wi-e41e02", "status": "pondered", "decision": {"option": "A", "applies": "often"}},
        {"id": "wi-b6ea09", "status": null}
    ]);

    let found = read(&listed).expect("an array");

    let statuses: Vec<Option<Status>> = found.iter().map(|item| item.status).collect();
    assert_eq!(statuses, [None, None]);
    assert_eq!(
        found
            .first()
            .and_then(|item| item.decision.as_ref())
            .map(|d| d.applies),
        Some(None)
    );
}

#[test]
fn a_listing_as_long_as_the_cap_is_truncated() {
    let items = |count: usize| {
        let items: Vec<serde_json::Value> = (0..count)
            .map(|n| json!({"id": format!("wi-{n:06x}"), "status": "decided"}))
            .collect();
        serde_json::Value::Array(items).to_string()
    };

    let full = read_listing(items(3).as_bytes(), 3);
    let under = read_listing(items(2).as_bytes(), 3);

    assert!(
        matches!(full, Err(ListingError::Truncated { cap: 3 })),
        "{full:?}"
    );
    assert_eq!(under.expect("under the cap").len(), 2);
}

#[test]
fn a_listing_counts_unreadable_items_towards_the_cap() {
    let listing = json!([{"id": "bad"}, {"id": "worse"}]).to_string();

    let read = read_listing(listing.as_bytes(), 2);

    assert!(
        matches!(read, Err(ListingError::Truncated { cap: 2 })),
        "{read:?}"
    );
}

#[test]
fn a_non_list_is_a_parse_error() {
    for text in ["1 open in repo\n", "{}", "null", ""] {
        let read = read_listing(text.as_bytes(), 4096);
        assert!(
            matches!(read, Err(ListingError::Parse(_))),
            "{text}: {read:?}"
        );
    }
}

#[test]
fn decided_label_is_the_card_title_of_the_decided_option() {
    let listed = json!([
        {"id": "wi-000001", "status": "decided",
         "decision": {"option": "b"},
         "card": {"options": [{"id": "a", "title": "Keep it"}, {"id": "b", "title": "Drop it"}]}},
        {"id": "wi-000002", "status": "decided", "decision": {"option": "free text"}},
        {"id": "wi-000003", "status": "decided", "decision": {"option": "c"},
         "card": {"options": [{"id": "a", "title": "Keep it"}]}},
        {"id": "wi-000004", "status": "decided", "decision": {"option": "a"},
         "card": {"options": [{"id": "a", "title": "  "}]}},
        {"id": "wi-000005", "status": "deferred"}
    ]);

    let found = read(&listed).expect("an array");

    let labels: Vec<Option<&str>> = found.iter().map(Listed::decided_label).collect();
    assert_eq!(
        labels,
        [
            Some("Drop it"),
            Some("free text"),
            Some("c"),
            Some("a"),
            None
        ]
    );
}

/// A decided item as weighin's `open --json` writes it (`OpenItem`, every key).
const WRITTEN: &str = r#"{"id":"wi-0c9f04eb9a","kind":"question","question":"Which store?","question_key":"0c9f04eb9a0c9f04","did":null,"flip_hint":null,"at":"src/lib.rs:3","sym":null,"blocks":null,"door_hint":"one-way","authority_hint":"judgment","channel":"manual","quote":"Which store?","source_path":null,"source_line":null,"session_id":"s1","worktree":null,"branch":null,"raised_ts":"2026-11-17T09:00:00Z","raised_position":1,"door":"one-way","blast":null,"authority":"judgment","route":null,"timing":null,"resolve_by":null,"card":{"why":"It sets the format.","timing":"","evidence":[],"options":[{"id":"a","title":"SQLite","summary":"","holds_if":"","if_wrong":""},{"id":"b","title":"Files","summary":"","holds_if":"","if_wrong":""}],"criteria":[],"recommendation":{"option":"a","flip":"","tradeoff":"","why_wrong":""}},"card_rev":2,"presented_ts":null,"presented_tree":null,"times_presented":1,"status":"decided","decision":{"option":"a","decider":"human:ryan","provenance":"user-approved","note":"","via":"chat","matched_recommendation":true,"tree_hash":null,"seconds_to_decide":12,"ts":"2026-11-17T09:00:00Z","applies":"once"},"deferral":null,"default":null,"dismissed":null,"superseded_by":null,"records":[],"links":[],"also_raised_in":[],"reopened":0,"last_position":3,"last_ts":"2026-11-17T09:00:00Z"}"#;

#[test]
fn read_listing_reads_what_weighin_writes() {
    let item: OpenItem = serde_json::from_str(WRITTEN).expect("an item weighin writes");
    let written = serde_json::to_string(&item).expect("an item writes");
    let listing = format!("[{written}]");

    let found = read_listing(listing.as_bytes(), 4096).expect("weighin's listing reads");

    assert_eq!(written, WRITTEN, "the fixture is OpenItem's bytes");
    let [listed] = found.as_slice() else {
        panic!("one item: {found:?}");
    };
    assert_eq!(listed.id, item.id);
    assert_eq!(listed.id, ItemId::parse("wi-0c9f04eb9a").expect("an id"));
    assert_eq!(listed.status, Some(item.status));
    assert_eq!(listed.question, item.question);
    assert_eq!(
        listed
            .decision
            .as_ref()
            .map(|d| (d.option.as_str(), d.applies)),
        Some(("a", Some(Applies::Once)))
    );
    assert_eq!(listed.decided_label(), Some("SQLite"));
}
