//! Raising a request in a temporary store: once, again, past a collision, refused.

use std::num::NonZeroUsize;

use bench_contract::event::{BenchEvent, TaskDone};
use redkiln_core::envelope::{Envelope, Namespace, decode};
use redkiln_core::ids::HexLen;
use redkiln_core::store::{CoreKey, Filter, NewEvent, Tag};
use redkiln_core::trust::Level;
use tempfile::TempDir;
use weighin_contract::ItemId;
use weighin_contract::request::{
    ANCHOR, Attachment, Blocks, OptionId, Raiser, Request, RequestOption, TrustNeeded,
};
use weighin_contract::vocab::{Authority, Door};

use super::{RaiseError, Raised, raise};
use crate::store::{EventEnvelope, Store, Timestamp};

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

/// A store of its own in a temporary directory, kept alive with it.
fn store() -> TestResult<(TempDir, Store)> {
    let dir = tempfile::tempdir()?;
    let store = Store::open(&dir.path().join("events.sqlite3"))?;
    Ok((dir, store))
}

/// A park's request with raiser key `key`: one-way, two options, due at the end of 2099.
fn request(key: &str) -> Request {
    let option = |id, label: &str| RequestOption {
        id,
        label: label.to_owned(),
        holds_when: format!("{label} holds"),
        cost_if_wrong: "a migration".to_owned(),
    };
    Request {
        question: "Should the cache be shared across tenants?".to_owned(),
        options: vec![
            option(OptionId::A, "Per tenant"),
            option(OptionId::B, "Shared"),
        ],
        recommendation: OptionId::A,
        door: Door::OneWay,
        authority: Authority::Authority,
        blast: None,
        default: None,
        deadline: Some("2099-12-31T23:59:59Z".to_owned()),
        blocks: Blocks::Run,
        trust_needed: TrustNeeded::Witnessed,
        raiser: Raiser {
            namespace: Namespace::Bench.into(),
            key: key.to_owned(),
            run: None,
            session: None,
        },
        subject: None,
        attachments: Some(vec![Attachment {
            kind: ANCHOR.to_owned(),
            locator: "src/cache.rs:12".to_owned(),
            hash: None,
        }]),
    }
}

/// `request` in the context Bench writes a `task_done` now.
fn envelope(store: &Store, request: Request) -> TestResult<Envelope<Request>> {
    let probe = EventEnvelope::new(
        BenchEvent::TaskDone(TaskDone {
            task: "7".to_owned(),
        }),
        "cli",
        Timestamp::now()?,
    );
    Ok(store.envelope_for(&probe, request)?)
}

/// Every `weighin.request_raised` in `store`.
fn requests(store: &Store) -> TestResult<Vec<redkiln_core::store::Stored>> {
    let filter = Filter::all().of_type("weighin.request_raised")?;
    Ok(store.read_foreign(&filter, NonZeroUsize::MIN.saturating_add(15))?)
}

/// Writes a captured `weighin.item_raised` holding `id`, as weighin's own raise tags it.
fn hold(store: &Store, id: &ItemId) -> TestResult {
    let event = NewEvent::new("weighin.item_raised", br#"{"q":"another"}"#.to_vec())?
        .with_tag(Tag::core(CoreKey::Item, id.as_str())?);
    let filter = Filter::all().of_type("bench.none")?;
    store.append_foreign_if(
        event,
        &redkiln_core::store::Condition::none_matching(filter),
    )?;
    Ok(())
}

#[test]
fn a_request_is_appended_once_with_its_item_tag_and_channel() -> TestResult {
    let (_dir, store) = store()?;
    let envelope = envelope(&store, request("orchestrate/park/7/0123456789abcdef"))?;

    let raised = raise(&store, &envelope)?;

    let id = envelope.data.item_id()?;
    let Raised::New { item, position } = raised else {
        panic!("a first raise is new: {raised:?}");
    };
    assert_eq!(item, id);
    let stored = requests(&store)?;
    let [only] = stored.as_slice() else {
        panic!("one request: {stored:?}");
    };
    assert_eq!(only.position, position);
    let tags: Vec<&str> = only.tags.iter().map(Tag::as_str).collect();
    assert!(tags.contains(&format!("item:{id}").as_str()), "{tags:?}");
    assert!(tags.contains(&"weighin.channel:request"), "{tags:?}");
    Ok(())
}

#[test]
fn the_same_raiser_key_raised_again_is_already_raised_at_the_first_position() -> TestResult {
    let (_dir, store) = store()?;
    let first = envelope(&store, request("orchestrate/park/7/0123456789abcdef"))?;
    let again = envelope(&store, request("orchestrate/park/7/0123456789abcdef"))?;

    let first = raise(&store, &first)?;
    let again = raise(&store, &again)?;

    let Raised::New { item, position } = first else {
        panic!("new: {first:?}");
    };
    assert_eq!(again, Raised::Already { item, position });
    assert_eq!(
        requests(&store)?.len(),
        1,
        "nothing written the second time"
    );
    Ok(())
}

#[test]
fn a_captured_item_holding_the_id_extends_it_to_twelve_hex() -> TestResult {
    let (_dir, store) = store()?;
    let envelope = envelope(&store, request("orchestrate/park/7/0123456789abcdef"))?;
    hold(&store, &envelope.data.item_id_at(HexLen::Ten)?)?;

    let raised = raise(&store, &envelope)?;

    let Raised::New { item, .. } = raised else {
        panic!("new: {raised:?}");
    };
    assert_eq!(item, envelope.data.item_id_at(HexLen::Twelve)?);
    assert_eq!(item.as_str().len(), "wi-".len() + 12);
    Ok(())
}

#[test]
fn all_three_lengths_taken_is_id_taken_and_writes_nothing() -> TestResult {
    let (_dir, store) = store()?;
    let envelope = envelope(&store, request("orchestrate/park/7/0123456789abcdef"))?;
    for len in [HexLen::Ten, HexLen::Twelve, HexLen::Sixteen] {
        hold(&store, &envelope.data.item_id_at(len)?)?;
    }

    let raised = raise(&store, &envelope);

    let ten = envelope.data.item_id_at(HexLen::Ten)?;
    assert!(
        matches!(&raised, Err(RaiseError::IdTaken { item }) if *item == ten),
        "{raised:?}"
    );
    assert_eq!(requests(&store)?.len(), 0);
    Ok(())
}

#[test]
fn a_refused_request_writes_nothing() -> TestResult {
    let (_dir, store) = store()?;
    let mut one_option = request("orchestrate/park/7/0123456789abcdef");
    one_option.options.truncate(1);
    let envelope = envelope(&store, one_option)?;

    let raised = raise(&store, &envelope);

    assert!(matches!(raised, Err(RaiseError::Refused(_))), "{raised:?}");
    assert_eq!(requests(&store)?.len(), 0);
    Ok(())
}

#[test]
fn the_request_reads_back_through_cores_decode() -> TestResult {
    let (_dir, store) = store()?;
    let written = request("orchestrate/park/7/0123456789abcdef");
    // The envelope owns the request; the test keeps a copy to compare with what reads back.
    let envelope = envelope(&store, written.clone())?;

    raise(&store, &envelope)?;

    let stored = requests(&store)?;
    let [only] = stored.as_slice() else {
        panic!("one request: {stored:?}");
    };
    let read = decode::<Request>(only)?;
    assert_eq!(read.data, written, "DR-02's record, as written");
    assert_eq!(read.writer.binary(), "bench");
    assert_eq!(read.trust.level(), Level::Asserted);
    Ok(())
}
