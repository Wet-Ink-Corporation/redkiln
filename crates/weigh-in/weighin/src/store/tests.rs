use redkiln_core::store::{Filter, NewEvent, Tag};
use weighin_core::Status;
use weighin_core::event::{Acknowledged, Channel, DismissReason, Dismissed, LedgerEvent, Raised};
use weighin_core::marker::{Authority, Door, Kind};

use super::*;

fn env(event: LedgerEvent) -> EventEnvelope {
    EventEnvelope {
        event,
        actor: "cli".into(),
        session_id: Some("s1".into()),
        worktree: Some("wt-5d41402abc4b".into()),
        branch: None,
        head: None,
        ts: "2026-09-26T00:00:00Z".into(),
    }
}

fn raise(id: &ItemId) -> EventEnvelope {
    env(LedgerEvent::ItemRaised(Raised {
        item_id: id.clone(),
        kind: Kind::Question,
        question: "q?".into(),
        did: None,
        flip: None,
        at: None,
        sym: None,
        blocks: None,
        door_hint: Door::OneWay,
        authority_hint: Authority::Authority,
        channel: Channel::Marker,
        quote: "q?".into(),
        source_path: None,
        source_line: None,
    }))
}

fn dismiss(id: &ItemId) -> EventEnvelope {
    env(LedgerEvent::ItemDismissed(Dismissed {
        item_id: id.clone(),
        why: DismissReason::Moot,
        note: String::new(),
        reason: "r".into(),
    }))
}

fn ack(id: &ItemId) -> EventEnvelope {
    env(LedgerEvent::ItemAcknowledged(Acknowledged {
        item_id: id.clone(),
        reason: "r".into(),
    }))
}

fn store() -> (tempfile::TempDir, Store) {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&dir.path().join("redkiln/events.sqlite3"), Wait::Command)
        .expect("the store opens");
    (dir, store)
}

/// `event` of `event_type` with `data`, tagged with `id`'s item, written past the adapter.
fn raw(store: &Store, event_type: &str, data: &[u8], id: &ItemId) {
    let event = NewEvent::new(event_type, data.to_vec())
        .expect("a valid type")
        .with_tag(Tag::new("item", id.as_str()).expect("a valid tag"));
    store.inner.append(vec![event], None).expect("append");
}

fn last(written: &Written) -> Option<u64> {
    match written {
        Written::At { last, .. } => Some(*last),
        Written::Conflict => None,
    }
}

#[test]
fn a_second_raise_conflicts_and_of_two_writers_that_read_alike_the_first_wins() {
    let (_dir, store) = store();
    let id = ItemId::mint("s1", "q?");
    let raised = Guard::NotYetRaised(id.clone());

    assert_eq!(
        last(&store.append(vec![raise(&id)], &raised).expect("append")),
        Some(1)
    );
    assert_eq!(
        last(&store.append(vec![raise(&id)], &raised).expect("append")),
        None
    );
    let read_at = Guard::ItemUnchangedSince(id.clone(), 1);
    assert_eq!(
        last(&store.append(vec![dismiss(&id)], &read_at).expect("append")),
        Some(2)
    );
    assert_eq!(
        last(&store.append(vec![dismiss(&id)], &read_at).expect("append")),
        None
    );

    let loaded = store.ledger().expect("read");
    assert_eq!(
        loaded.ledger.get(&id).expect("the item").status,
        Status::Dismissed
    );
    assert_eq!((loaded.ledger.head, loaded.unusable.total()), (2, 0));
}

#[test]
fn an_append_hands_back_the_events_it_wrote() {
    let (_dir, store) = store();
    let id = ItemId::mint("s1", "q?");

    let written = store
        .append(vec![raise(&id), ack(&id)], &Guard::NotYetRaised(id.clone()))
        .expect("append");

    let Written::At { last, events } = written else {
        panic!("written: {written:?}");
    };
    assert_eq!((last, events), (2, vec![raise(&id), ack(&id)]));
}

#[test]
fn an_event_core_refuses_fails_the_whole_append_and_writes_nothing() {
    let (_dir, store) = store();
    let id = ItemId::mint("s1", "q?");
    let mut bad = ack(&id);
    bad.actor = "t".into();

    let err = store
        .append(vec![raise(&id), bad], &Guard::NotYetRaised(id.clone()))
        .expect_err("`t` is not an actor");

    assert!(format!("{err:#}").contains("the event's actor"), "{err:#}");
    assert!(store.read_all().expect("read").events.is_empty());
}

#[test]
fn another_namespace_neither_conflicts_a_write_nor_reads_as_weighin() {
    let (_dir, store) = store();
    let id = ItemId::mint("s1", "q?");
    let raised = store.append(vec![raise(&id)], &Guard::NotYetRaised(id.clone()));
    assert_eq!(last(&raised.expect("append")), Some(1));
    raw(&store, "bench.run_finished", b"{}", &id);
    raw(&store, "core.import_completed", b"{}", &id);

    let written = store.append(
        vec![dismiss(&id)],
        &Guard::ItemUnchangedSince(id.clone(), 1),
    );

    assert_eq!(last(&written.expect("append")), Some(4));
    let read = store.read_all().expect("read");
    let positions: Vec<u64> = read.events.iter().map(|(p, _)| *p).collect();
    assert_eq!((positions, read.unusable.total()), (vec![1, 4], 0));
}

#[test]
fn a_weighin_event_this_build_cannot_use_is_skipped_and_counted() {
    let (_dir, store) = store();
    let (a, b) = (ItemId::mint("s1", "a?"), ItemId::mint("s1", "b?"));
    let raised = store.append(vec![raise(&a)], &Guard::NotYetRaised(a.clone()));
    assert_eq!(last(&raised.expect("append")), Some(1));
    let mut newer = String::from_utf8(
        store
            .inner
            .read(&Filter::all(), 0, NonZeroUsize::MIN)
            .expect("read")
            .pop()
            .expect("the raise")
            .data,
    )
    .expect("utf-8");
    newer = newer.replacen(r#""version":1"#, r#""version":2"#, 1);
    raw(&store, wire::RAISED, newer.as_bytes(), &b);
    raw(
        &store,
        wire::RAISED,
        br#"{"type":"weighin.item_raised"}"#,
        &b,
    );

    let loaded = store
        .ledger()
        .expect("an unusable event does not fail the read");

    assert_eq!(loaded.unusable.total(), 2);
    assert!(loaded.ledger.get(&a).is_some_and(|i| i.status.is_open()));
    assert_eq!(
        loaded.ledger.get(&b),
        None,
        "an item only unusable events name"
    );
    let blocked = store.append(vec![ack(&b)], &Guard::ItemUnchangedSince(b.clone(), 1));
    assert_eq!(
        last(&blocked.expect("append")),
        None,
        "fail closed on what it cannot read"
    );
}

#[test]
fn unusable_events_are_counted_by_type() {
    let (_dir, store) = store();
    let id = ItemId::mint("s1", "a?");
    let dismissed = "weighin.item_dismissed";
    raw(
        &store,
        wire::RAISED,
        br#"{"type":"weighin.item_raised"}"#,
        &id,
    );
    raw(
        &store,
        wire::RAISED,
        br#"{"type":"weighin.item_raised"}"#,
        &id,
    );
    raw(
        &store,
        dismissed,
        br#"{"type":"weighin.item_dismissed"}"#,
        &id,
    );

    let read = store.read_all().expect("read");

    assert_eq!(read.unusable.total(), 3);
    let by_type: Vec<(String, u64)> = read.unusable.into_by_type().into_iter().collect();
    assert_eq!(
        by_type,
        [(dismissed.to_owned(), 1), (wire::RAISED.to_owned(), 2)]
    );
}

#[test]
fn a_read_longer_than_a_page_returns_every_event_in_order() {
    let (_dir, store) = store();
    let id = ItemId::mint("s1", "q?");
    let count = PAGE.get() + 1;
    let raised = store.append(vec![raise(&id)], &Guard::NotYetRaised(id.clone()));
    let mut at = last(&raised.expect("append")).expect("raised");
    // happenstance guarantees 128 events a batch, not a page's worth.
    for chunk in (1..count).collect::<Vec<_>>().chunks(100) {
        let acks = chunk.iter().map(|_| ack(&id)).collect();
        let written = store.append(acks, &Guard::ItemUnchangedSince(id.clone(), at));
        at = last(&written.expect("append")).expect("unchanged since the last chunk");
    }
    assert_eq!(at, u64::try_from(count).expect("small"));

    let read = store.read_all().expect("read");
    let positions: Vec<u64> = read.events.iter().map(|(p, _)| *p).collect();
    let expected: Vec<u64> = (1..=u64::try_from(count).expect("small")).collect();
    assert_eq!(positions, expected);
}

#[test]
fn only_a_file_core_refuses_as_a_store_is_a_refusal_however_it_is_wrapped() {
    let dir = tempfile::tempdir().expect("tempdir");
    // A copy: Core's fixtures are never opened in place.
    let old = dir.path().join("ledger.sqlite3");
    std::fs::copy(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../core/redkiln-core/tests/fixtures/weighin-1.2.0.sqlite3"),
        &old,
    )
    .expect("fixture copied");
    let blocked = old.join("events.sqlite3");

    let refusal = Store::open(&old, Wait::Command).expect_err("an old ledger is refused");
    let other = Store::open(&blocked, Wait::Command).expect_err("a file is no directory");

    assert!(is_refused(&refusal), "{refusal:#}");
    assert!(is_refused(&refusal.context("reading the ledger")));
    assert!(!is_refused(&other), "{other:#}");
}
