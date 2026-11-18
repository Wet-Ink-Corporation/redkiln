use crate::event::{
    Channel, DefaultCause, EventEnvelope, LedgerEvent, Provenance, Raised, Requested, Route,
};
use crate::id::ItemId;
use crate::marker::{Authority, Door, Kind};
use crate::projection::{Item, Ledger, Status};
use serde_json::{Value, json};
use weighin_contract::event::Decided;
use weighin_contract::open::OpenItem;
use weighin_contract::request::Request;

/// A two-way judgment request with three options, blocking a run, and no default.
fn record() -> Value {
    json!({
        "question": "Keep the retry at three attempts?",
        "options": [
            {"id": "a", "label": "Three", "holds_when": "Flakes are rare", "cost_if_wrong": "A red run"},
            {"id": "b", "label": "Five", "holds_when": "Flakes are common", "cost_if_wrong": "Slow failures"},
            {"id": "c", "label": "None", "holds_when": "Flakes are bugs", "cost_if_wrong": "Hidden flakes"}
        ],
        "recommendation": "b",
        "door": "two-way",
        "authority": "judgment",
        "blast": "module",
        "blocks": "run",
        "trust_needed": "witnessed",
        "raiser": {"namespace": "bench", "key": "retry/attempts", "session": "s-raiser"}
    })
}

fn request(value: Value) -> Request {
    serde_json::from_value(value).expect("a request record")
}

fn id(text: &str) -> ItemId {
    ItemId::parse(text).expect("an item id")
}

fn requested(item: &ItemId, value: Value, session: Option<&str>) -> EventEnvelope {
    EventEnvelope {
        event: LedgerEvent::RequestRaised(Requested {
            item_id: item.clone(),
            request: request(value),
        }),
        actor: "agent".into(),
        session_id: session.map(str::to_owned),
        worktree: Some("tree".into()),
        branch: Some("main".into()),
        head: None,
        ts: "2026-11-18T10:00:00Z".into(),
        cites: Vec::new(),
    }
}

fn fold(events: &[EventEnvelope]) -> Ledger {
    Ledger::fold((1u64..).zip(events))
}

fn only(ledger: &Ledger) -> &Item {
    assert_eq!(ledger.items.len(), 1, "one item: {:?}", ledger.items.keys());
    ledger.items.values().next().expect("one item")
}

#[test]
fn a_request_folds_into_one_grounded_item_with_its_card() {
    let item_id = id("wi-0a1b2c3d4e");
    let ledger = fold(&[requested(&item_id, record(), Some("s1"))]);
    let item = only(&ledger);

    assert_eq!(item.id, item_id);
    assert_eq!(
        (item.status, item.channel, item.kind),
        (Status::Grounded, Channel::Request, Kind::Question)
    );
    assert_eq!(item.question, "Keep the retry at three attempts?");
    assert_eq!(item.quote, item.question);
    assert_eq!(item.door, Some(Door::TwoWay));
    assert_eq!(item.authority, Some(Authority::Judgment));
    assert_eq!(item.blocks.as_deref(), Some("run"));
    assert_eq!(item.route, Some(Route::Present));
    assert_eq!(item.card_rev, Some(1));
    let record = item.request.as_ref().expect("the request record");
    assert_eq!(
        (record.position, &record.request),
        (1, &request(self::record()))
    );
    assert_eq!(
        (
            item.session_id.as_deref(),
            item.worktree.as_deref(),
            item.branch.as_deref()
        ),
        (Some("s1"), Some("tree"), Some("main"))
    );
    assert!(
        item.needs_attention(),
        "a grounded request waits on a human"
    );
}

#[test]
fn a_request_items_card_is_the_requests_options_and_recommendation() {
    let ledger = fold(&[requested(&id("wi-0a1b2c3d4e"), record(), Some("s1"))]);
    let card = only(&ledger).card.as_ref().expect("the request's card");

    assert_eq!(
        card.options
            .iter()
            .map(|o| o.id.as_str())
            .collect::<Vec<_>>(),
        ["a", "b", "c"]
    );
    let b = card.options.get(1).expect("option b");
    assert_eq!(
        (b.title.as_str(), b.holds_if.as_str(), b.if_wrong.as_str()),
        ("Five", "Flakes are common", "Slow failures")
    );
    assert_eq!(card.recommendation.option, "b");
    assert_eq!(
        card.why,
        "Asked by bench through a decision request (retry/attempts)"
    );
}

#[test]
fn a_request_with_no_envelope_session_takes_the_raisers() {
    let ledger = fold(&[requested(&id("wi-0a1b2c3d4e"), record(), None)]);
    assert_eq!(only(&ledger).session_id.as_deref(), Some("s-raiser"));
}

#[test]
fn a_request_raised_again_keeps_the_first_item() {
    let item_id = id("wi-0a1b2c3d4e");
    let mut again = record();
    again["question"] = json!("Reworded?");
    let ledger = fold(&[
        requested(&item_id, record(), Some("s1")),
        requested(&item_id, again, Some("s2")),
    ]);
    let item = only(&ledger);

    assert_eq!(item.question, "Keep the retry at three attempts?");
    assert_eq!(item.request.as_ref().map(|r| r.position), Some(1));
    assert_eq!(item.also_raised_in, ["s2"]);
}

#[test]
fn a_request_anchor_attachment_becomes_the_items_at() {
    let mut value = record();
    value["attachments"] = json!([
        {"kind": "log", "locator": "target/run.log"},
        {"kind": "anchor", "locator": "src/retry.rs:42"},
        {"kind": "anchor", "locator": "src/other.rs:1"}
    ]);
    let ledger = fold(&[requested(&id("wi-0a1b2c3d4e"), value, Some("s1"))]);
    let item = only(&ledger);

    assert_eq!(item.at.as_deref(), Some("src/retry.rs:42"));
    let evidence: Vec<(&str, &str)> = item
        .card
        .as_ref()
        .expect("a card")
        .evidence
        .iter()
        .map(|e| (e.src.as_str(), e.text.as_str()))
        .collect();
    assert_eq!(
        evidence,
        [
            ("target/run.log", "log"),
            ("src/retry.rs:42", "anchor"),
            ("src/other.rs:1", "anchor")
        ]
    );
}

#[test]
fn blocks_none_ranks_behind_blocks_run() {
    let mut idle = record();
    idle["blocks"] = json!("none");
    idle["raiser"]["key"] = json!("retry/idle");
    let ledger = fold(&[
        requested(&id("wi-1111111111"), idle, Some("s1")),
        requested(&id("wi-2222222222"), record(), Some("s1")),
    ]);
    let ranked: Vec<&str> = ledger.open().iter().map(|i| i.id.as_str()).collect();

    assert_eq!(ranked, ["wi-2222222222", "wi-1111111111"]);
    assert_eq!(
        ledger.get(&id("wi-1111111111")).map(|i| i.blocks.clone()),
        Some(None)
    );
}

#[test]
fn deciding_a_request_item_by_option_a_decides_it() {
    let item_id = id("wi-0a1b2c3d4e");
    let decided = EventEnvelope {
        event: LedgerEvent::ItemDecided(Decided {
            item_id: item_id.clone(),
            option: "a".into(),
            decider: "human:ryan".into(),
            provenance: Provenance::UserDirected,
            note: String::new(),
            tree_hash: None,
            matched_recommendation: Some(false),
            via: "cli".into(),
            seconds_to_decide: None,
            reason: "decided".into(),
            applies: None,
            confirmation: None,
        }),
        actor: "human:ryan".into(),
        session_id: None,
        worktree: None,
        branch: None,
        head: None,
        ts: "2026-11-18T11:00:00Z".into(),
        cites: Vec::new(),
    };
    let ledger = fold(&[requested(&item_id, record(), Some("s1")), decided]);
    let item = only(&ledger);

    assert_eq!(item.status, Status::Decided);
    assert_eq!(item.decision.as_ref().map(|d| d.option.as_str()), Some("a"));
    assert!(ledger.orphans.is_empty(), "{:?}", ledger.orphans);
}

#[test]
fn a_two_way_judgment_request_with_a_default_and_no_deadline_folds_defaulted() {
    let mut ruling = record();
    ruling["default"] = json!("b");
    let ledger = fold(&[requested(&id("wi-0a1b2c3d4e"), ruling, Some("s1"))]);
    let item = only(&ledger);
    let default = item.default.as_ref().expect("taken");

    assert_eq!(item.status, Status::Defaulted);
    assert_eq!(item.route, Some(Route::Default));
    assert_eq!(
        (default.option.as_str(), default.cause, default.acknowledged),
        ("b", DefaultCause::TwoWay, false)
    );
    assert_eq!(default.reason, "the raiser's own two-way call, logged");
    assert_eq!(default.ts, "2026-11-18T10:00:00Z");
    let digest: Vec<&str> = ledger
        .unacknowledged_defaults()
        .iter()
        .map(|i| i.id.as_str())
        .collect();
    assert_eq!(digest, ["wi-0a1b2c3d4e"], "the digest shows it");
}

#[test]
fn a_one_way_request_is_never_folded_defaulted() {
    let cases = [
        ("one-way", "judgment", None),
        ("unsure", "judgment", None),
        ("two-way", "authority", None),
        ("two-way", "judgment", Some("2026-11-20T00:00:00Z")),
    ];
    for (door, authority, deadline) in cases {
        let mut value = record();
        value["door"] = json!(door);
        value["authority"] = json!(authority);
        value["default"] = json!("b");
        if let Some(deadline) = deadline {
            value["deadline"] = json!(deadline);
        }
        let ledger = fold(&[requested(&id("wi-0a1b2c3d4e"), value, Some("s1"))]);
        let item = only(&ledger);

        assert_eq!(
            item.status,
            Status::Grounded,
            "{door} {authority} {deadline:?}"
        );
        assert_eq!(item.default, None, "{door} {authority} {deadline:?}");
        assert_eq!(item.resolve_by.as_deref(), deadline);
    }
}

#[test]
fn an_item_with_no_request_serialises_as_before() {
    let raised = EventEnvelope {
        event: LedgerEvent::ItemRaised(Raised {
            item_id: ItemId::mint("s1", "q?"),
            kind: Kind::Question,
            question: "q?".into(),
            did: None,
            flip: None,
            at: None,
            sym: None,
            blocks: None,
            door_hint: Door::TwoWay,
            authority_hint: Authority::Judgment,
            channel: Channel::Marker,
            quote: "q?".into(),
            source_path: None,
            source_line: None,
        }),
        actor: "agent".into(),
        session_id: Some("s1".into()),
        worktree: None,
        branch: None,
        head: None,
        ts: "2026-11-18T10:00:00Z".into(),
        cites: Vec::new(),
    };
    let ledger = fold(&[raised]);
    let item = only(&ledger);

    let text = serde_json::to_string(&OpenItem::from(item)).expect("writes");

    assert!(!text.contains("\"request\""), "{text}");
    assert!(
        text.ends_with(r#""last_ts":"2026-11-18T10:00:00Z"}"#),
        "{text}"
    );
    assert_eq!(text, serde_json::to_string(item).expect("writes"));
}

#[test]
fn a_request_item_writes_and_reads_back_through_the_contract() {
    let ledger = fold(&[requested(&id("wi-0a1b2c3d4e"), record(), Some("s1"))]);
    let item = only(&ledger);
    let open = OpenItem::from(item);

    let text = serde_json::to_string(&open).expect("writes");

    assert_eq!(text, serde_json::to_string(item).expect("writes"));
    assert_eq!(
        serde_json::from_str::<OpenItem>(&text).expect("reads"),
        open
    );
    assert!(text.contains(r#""channel":"request""#), "{text}");
}
