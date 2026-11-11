//! Envelopes through a real store (SPEC EV-01, EV-08 to EV-12).

use std::num::NonZeroUsize;

use redkiln_core::envelope::{
    Actor, Author, Class, Envelope, Payload, Timestamp, Writer, decode, decode_all,
};
use redkiln_core::ids::{SessionId, WorktreeId, parse_event_ref};
use redkiln_core::store::{Appended, CoreKey, Filter, NewEvent, Store, Stored, Tag, Wait};
use redkiln_core::trust::{Level, Trust, Via};
use serde::{Deserialize, Serialize};

/// The SPEC's EV-01 example, as one line.
const SPEC_EXAMPLE: &str = r#"{"type":"weighin.item_decided","version":1,"ts":"2026-10-31T04:20:11Z","actor":"human:ryan","trust":{"level":"witnessed","via":"ask-user-question","ref":"toolu_01XyZ"},"writer":{"binary":"weighin","version":"2.0.0","spec":"1.0.0"},"session":"8b6fb94b-0975-5275-b434-6c9e60544265","worktree":"wt-5d41402abc4b","branch":"main","head":"bd234009c1e5a7d4f1c3b2a0e9f8d7c6b5a49382","cites":["0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:41"],"data":{"item":"wi-3fa9c01b2e","option":"a","applies":"once"}}"#;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct ItemDecided {
    item: String,
    option: String,
    applies: String,
}

impl Payload for ItemDecided {
    const TYPE: &'static str = "weighin.item_decided";
    const VERSION: u32 = 1;
    const CLASS: Class = Class::Durable;
}

fn spec_example() -> Envelope<ItemDecided> {
    let trust = Trust::new(Level::Witnessed, Some(Via::AskUserQuestion))
        .and_then(|t| t.with_reference("toolu_01XyZ"))
        .expect("valid trust");
    let author = Author {
        actor: Actor::parse("human:ryan").expect("valid actor"),
        trust,
        writer: Writer::new("weighin", "2.0.0").expect("valid writer"),
    };
    let mut envelope = Envelope::new(
        Timestamp::parse("2026-10-31T04:20:11Z").expect("valid ts"),
        author,
        ItemDecided {
            item: "wi-3fa9c01b2e".to_owned(),
            option: "a".to_owned(),
            applies: "once".to_owned(),
        },
    );
    envelope.session = Some(SessionId::parse("8b6fb94b-0975-5275-b434-6c9e60544265").expect("ok"));
    envelope.worktree = Some(WorktreeId::parse("wt-5d41402abc4b").expect("valid"));
    envelope.branch = Some("main".to_owned());
    envelope.head = Some("bd234009c1e5a7d4f1c3b2a0e9f8d7c6b5a49382".to_owned());
    envelope.cites = vec![parse_event_ref("0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:41").expect("valid")];
    envelope
}

fn all(store: &Store) -> Vec<Stored> {
    store
        .read(&Filter::all(), 0, NonZeroUsize::new(100).expect("non-zero"))
        .expect("read")
}

fn raw(event_type: &str, data: &str) -> NewEvent {
    NewEvent::new(event_type, data.as_bytes().to_vec()).expect("a valid type")
}

#[test]
fn the_spec_example_is_stored_as_written_and_read_back() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&dir.path().join("events.sqlite3"), Wait::Command).expect("opens");
    let item = Tag::core(CoreKey::Item, "wi-3fa9c01b2e").expect("an item tag");
    let event = spec_example().to_event([item]).expect("valid");
    assert_eq!(
        store.append(vec![event], None).expect("append"),
        Appended::At(1)
    );

    let stored = all(&store);
    let [only] = stored.as_slice() else {
        panic!("one event: {stored:?}");
    };
    assert_eq!(only.event_type, "weighin.item_decided");
    assert_eq!(only.data, SPEC_EXAMPLE.as_bytes(), "stored byte for byte");
    let tags: Vec<&str> = only.tags.iter().map(Tag::as_str).collect();
    for tag in [
        "plugin:weighin",
        "trust:witnessed",
        "session:8b6fb94b-0975-5275-b434-6c9e60544265",
        "worktree:wt-5d41402abc4b",
        "item:wi-3fa9c01b2e",
    ] {
        assert!(tags.contains(&tag), "{tag} in {tags:?}");
    }
    assert_eq!(decode::<ItemDecided>(only).expect("usable"), spec_example());
}

/// EV-08's conformance case: foreign events are skipped, never fatal, and a known type this
/// build cannot use is counted (EV-10).
#[test]
fn foreign_events_are_skipped_and_unusable_ones_counted() {
    let dir = tempfile::tempdir().expect("tempdir");
    let store = Store::open(&dir.path().join("events.sqlite3"), Wait::Command).expect("opens");
    let newer = SPEC_EXAMPLE.replace("\"version\":1", "\"version\":2");
    let events = vec![
        raw("bench.point", r#"{"not":"an envelope we know"}"#),
        spec_example().to_event([]).expect("valid"),
        raw("loop.migrate.wave.started", "[]"),
        raw("weighin.item_decided", &newer),
        raw("weighin.item_decided", "not json"),
        spec_example().to_event([]).expect("valid"),
    ];
    store.append(events, None).expect("append");

    let decoded = decode_all::<ItemDecided>(&all(&store));
    assert_eq!(decoded.events, vec![spec_example(), spec_example()]);
    assert_eq!(decoded.foreign, 2);
    assert_eq!(decoded.unusable, 2);
}

#[test]
fn unix_millis_agrees_with_from_unix_millis_in_both_forms() {
    let seconds = Timestamp::parse("2026-10-31T04:20:11Z").expect("valid ts");
    let millis = Timestamp::parse("2026-10-31T04:20:11.123Z").expect("valid ts");
    assert_eq!(seconds.unix_millis(), 1_793_420_411_000);
    assert_eq!(millis.unix_millis(), 1_793_420_411_123);
    assert_eq!(
        Timestamp::from_unix_millis(millis.unix_millis()).expect("in range"),
        millis,
        "the millisecond form round-trips as written"
    );
    assert_eq!(
        Timestamp::from_unix_millis(seconds.unix_millis())
            .expect("in range")
            .as_str(),
        "2026-10-31T04:20:11.000Z",
        "the second form comes back at the same instant, in the millisecond form"
    );
    let before_epoch = Timestamp::parse("1969-12-31T23:59:59.999Z").expect("valid ts");
    assert_eq!(before_epoch.unix_millis(), -1);
}
