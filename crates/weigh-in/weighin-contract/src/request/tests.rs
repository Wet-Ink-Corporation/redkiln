use super::*;
use redkiln_core::envelope::{Actor, Author, Writer, decode};
use redkiln_core::store::{Filter, Store, Wait};
use redkiln_core::trust::Trust;
use std::num::NonZeroUsize;
use std::path::PathBuf;

const TS: &str = "2026-11-08T12:00:00Z";

fn fixture(rel: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../spec/fixtures/decision-request")
        .join(rel);
    let text = std::fs::read_to_string(&path).expect("the SPEC's fixture is in the tree");
    text.trim_end().to_owned()
}

fn golden() -> Request {
    serde_json::from_str(&fixture("valid/dr-02.json")).expect("DR-02's golden reads")
}

fn ts(text: &str) -> Timestamp {
    Timestamp::parse(text).expect("a test timestamp")
}

fn refusal(request: &Request) -> RequestError {
    request.check(&ts(TS)).expect_err("the guard refuses it")
}

#[test]
fn the_golden_passes_every_guard_and_writes_back_byte_for_byte() {
    let request = golden();
    request
        .check(&ts(TS))
        .expect("DR-02's example is a valid request");
    assert_eq!(
        serde_json::to_string(&request).expect("writes"),
        fixture("valid/dr-02.json")
    );
}

#[test]
fn one_option_is_refused() {
    let request: Request =
        serde_json::from_str(&fixture("invalid/one-option.json")).expect("the shape reads");
    assert!(matches!(refusal(&request), RequestError::OptionCount(1)));
}

#[test]
fn five_options_are_refused() {
    let mut request = golden();
    let extra = request.options.first().expect("two options").clone();
    request
        .options
        .extend([extra.clone(), extra.clone(), extra]);
    assert!(matches!(refusal(&request), RequestError::OptionCount(5)));
}

#[test]
fn options_out_of_order_are_refused() {
    let mut request = golden();
    request.options.reverse();
    assert!(matches!(
        refusal(&request),
        RequestError::OptionOrder {
            position: 1,
            found: OptionId::B
        }
    ));
}

#[test]
fn an_option_id_past_d_does_not_read() {
    let err = serde_json::from_str::<Request>(&fixture("invalid/option-id-not-a-to-d.json"))
        .expect_err("`e` is no option id");
    assert!(err.to_string().contains("unknown variant `e`"), "{err}");
}

#[test]
fn an_unknown_door_does_not_read() {
    let err = serde_json::from_str::<Request>(&fixture("invalid/door-unknown.json"))
        .expect_err("`maybe` is no door");
    assert!(err.to_string().contains("unknown variant `maybe`"), "{err}");
}

#[test]
fn a_recommendation_that_names_no_option_is_refused() {
    let mut request = golden();
    request.recommendation = OptionId::C;
    assert!(matches!(
        refusal(&request),
        RequestError::Recommendation(OptionId::C)
    ));
}

#[test]
fn a_one_way_item_is_never_defaulted() {
    let request: Request =
        serde_json::from_str(&fixture("invalid/default-on-one-way.json")).expect("the shape reads");
    assert!(matches!(
        refusal(&request),
        RequestError::DefaultDoor(Door::OneWay)
    ));
}

#[test]
fn an_unsure_item_is_never_defaulted() {
    let mut request = golden();
    request.door = Door::Unsure;
    request.default = Some(OptionId::B);
    assert!(matches!(
        refusal(&request),
        RequestError::DefaultDoor(Door::Unsure)
    ));
}

#[test]
fn a_two_way_item_may_default_to_one_of_its_options_only() {
    let mut request = golden();
    request.door = Door::TwoWay;
    request.default = Some(OptionId::A);
    request
        .check(&ts(TS))
        .expect("a two-way default is allowed");
    request.default = Some(OptionId::D);
    assert!(matches!(
        refusal(&request),
        RequestError::Default(OptionId::D)
    ));
}

#[test]
fn a_deadline_must_be_later_than_the_ts() {
    let mut request = golden();
    request.deadline = Some("2026-11-08T12:00:00.001Z".to_owned());
    request
        .check(&ts(TS))
        .expect("one millisecond later is later");
    for same_or_earlier in [
        "2026-11-08T12:00:00Z",
        "2026-11-08T12:00:00.000Z",
        "2026-11-07T23:59:59Z",
    ] {
        request.deadline = Some(same_or_earlier.to_owned());
        assert!(
            matches!(refusal(&request), RequestError::DeadlinePassed { .. }),
            "{same_or_earlier}"
        );
    }
}

#[test]
fn a_deadline_with_milliseconds_compares_with_a_ts_without() {
    let mut request = golden();
    request.deadline = Some("2026-11-08T12:00:01Z".to_owned());
    request
        .check(&ts("2026-11-08T12:00:00.999Z"))
        .expect("a later second is later than any millisecond before it");
}

#[test]
fn a_deadline_that_is_not_a_timestamp_is_refused() {
    let mut request = golden();
    request.deadline = Some("2026-02-30T00:00:00Z".to_owned());
    assert!(matches!(refusal(&request), RequestError::Deadline { .. }));
}

#[test]
fn a_ci_raiser_must_ask_for_attested() {
    let mut request = golden();
    request.trust_needed = TrustNeeded::Witnessed;
    assert!(matches!(refusal(&request), RequestError::CiTrust(s) if s.starts_with("gha:")));
    request.raiser.session = Some("0b5c6a3e-1f2d-4c8b-9a7e-2d3f4a5b6c7d".to_owned());
    request
        .check(&ts(TS))
        .expect("a local session may ask for witnessed");
}

#[test]
fn a_subject_hash_of_another_method_is_refused() {
    let request: Request = serde_json::from_str(&fixture("invalid/subject-hash-wrong-method.json"))
        .expect("the shape reads");
    assert!(matches!(
        refusal(&request),
        RequestError::SubjectMethod { .. }
    ));
}

#[test]
fn a_subject_hash_core_cannot_read_is_refused() {
    let mut request = golden();
    if let Some(subject) = request.subject.as_mut() {
        subject.hash = "rust-fingerprint:XYZ".to_owned();
    }
    assert!(matches!(
        refusal(&request),
        RequestError::SubjectHash { .. }
    ));
}

#[test]
fn an_empty_question_or_option_text_is_refused() {
    let mut request = golden();
    request.question = "  ".to_owned();
    assert!(matches!(refusal(&request), RequestError::Empty("question")));
    let mut request = golden();
    if let Some(option) = request.options.last_mut() {
        option.cost_if_wrong = String::new();
    }
    assert!(matches!(
        refusal(&request),
        RequestError::Empty("options.cost_if_wrong")
    ));
}

fn subject_mut(request: &mut Request) -> &mut Subject {
    request
        .subject
        .as_mut()
        .expect("DR-02's golden has a subject")
}

fn attachment(hash: Option<&str>) -> Attachment {
    Attachment {
        kind: "log".to_owned(),
        locator: "target/bench.log".to_owned(),
        hash: hash.map(str::to_owned),
    }
}

#[test]
fn an_empty_run_session_locator_or_attachment_text_is_refused() {
    let mut request = golden();
    request.raiser.run = Some(String::new());
    assert!(matches!(
        refusal(&request),
        RequestError::Empty("raiser.run")
    ));

    let mut request = golden();
    request.raiser.session = Some(" ".to_owned());
    assert!(matches!(
        refusal(&request),
        RequestError::Empty("raiser.session")
    ));

    let mut request = golden();
    subject_mut(&mut request).locator = String::new();
    assert!(matches!(
        refusal(&request),
        RequestError::Empty("subject.locator")
    ));

    let mut request = golden();
    let mut blank = attachment(None);
    blank.kind = String::new();
    request.attachments = Some(vec![blank]);
    assert!(matches!(
        refusal(&request),
        RequestError::Empty("attachments.kind")
    ));

    let mut request = golden();
    let mut blank = attachment(None);
    blank.locator = "\t".to_owned();
    request.attachments = Some(vec![blank]);
    assert!(matches!(
        refusal(&request),
        RequestError::Empty("attachments.locator")
    ));
}

#[test]
fn an_empty_attachment_list_is_refused_and_a_full_one_passes() {
    let mut request = golden();
    request.attachments = Some(Vec::new());
    assert!(matches!(refusal(&request), RequestError::NoAttachments));

    request.attachments = Some(vec![attachment(None), attachment(Some("sha256:0a1b"))]);
    request
        .check(&ts(TS))
        .expect("attachments with good hashes pass");
}

#[test]
fn an_attachment_hash_that_is_not_method_and_lowercase_hex_is_refused() {
    for hash in [
        "sha256",
        "sha256:",
        ":0a1b",
        "SHA256:0a1b",
        "sha256:0A1B",
        "sha_256:0a",
    ] {
        let mut request = golden();
        request.attachments = Some(vec![attachment(Some(hash))]);
        assert!(
            matches!(refusal(&request), RequestError::AttachmentHash(ref h) if h == hash),
            "{hash}"
        );
    }
}

#[test]
fn a_subject_kind_outside_ap_01_is_refused() {
    for kind in [
        "Plan",
        "commit",
        "bench.",
        "bench.9run",
        "nobody.run",
        "bench.Run",
    ] {
        let mut request = golden();
        subject_mut(&mut request).kind = kind.to_owned();
        assert!(
            matches!(refusal(&request), RequestError::SubjectKind(ref k) if k == kind),
            "{kind}"
        );
    }
    for kind in ["plan", "test_removal", "bench.run_2", "core.import"] {
        let mut request = golden();
        subject_mut(&mut request).kind = kind.to_owned();
        request.check(&ts(TS)).expect(kind);
    }
}

#[test]
fn a_subject_base_must_be_a_40_or_64_hex_commit() {
    for base in ["abc", &"A".repeat(40), &"0".repeat(41), &"g".repeat(64)] {
        let mut request = golden();
        subject_mut(&mut request).base = Some(base.to_owned());
        assert!(
            matches!(refusal(&request), RequestError::SubjectBase(ref b) if b == base),
            "{base}"
        );
    }
    for base in ["0".repeat(40), "f".repeat(64)] {
        let mut request = golden();
        subject_mut(&mut request).base = Some(base);
        request.check(&ts(TS)).expect("a commit base passes");
    }
}

#[test]
fn a_subject_locator_with_a_control_character_is_refused() {
    for locator in ["Cargo\u{0}.toml", "Cargo.toml\n", "a\u{7f}", "a\u{85}b"] {
        let mut request = golden();
        subject_mut(&mut request).locator = locator.to_owned();
        assert!(
            matches!(refusal(&request), RequestError::SubjectLocator),
            "{locator:?}"
        );
    }
}

#[test]
fn an_unknown_namespace_does_not_read() {
    let text =
        fixture("valid/dr-02.json").replace("\"namespace\":\"rust\"", "\"namespace\":\"rusty\"");
    let err = serde_json::from_str::<Request>(&text).expect_err("not an EV-03 namespace");
    assert!(
        err.to_string().contains("unknown namespace `rusty`"),
        "{err}"
    );
}

#[test]
fn the_item_id_is_core_s_content_id_of_namespace_and_key() {
    let request = golden();
    let id = request.item_id().expect("mints");
    let core = ContentId::mint(
        IdPrefix::Item,
        "redkiln.id/weighin.request/1",
        &["rust", "h16/dependency/jiff"],
        HexLen::Ten,
    )
    .expect("Core mints");
    assert_eq!(id.as_str(), core.as_str());
    assert_eq!(id.as_str().len(), "wi-".len() + 10);
}

#[test]
fn the_same_key_raised_again_finds_the_same_item_and_another_does_not() {
    let first = golden();
    let mut again = golden();
    again.question = "Reworded?".to_owned();
    assert_eq!(
        first.item_id().expect("mints"),
        again.item_id().expect("mints")
    );
    let mut other = golden();
    other.raiser.key = "h16/dependency/chrono".to_owned();
    assert_ne!(
        first.item_id().expect("mints"),
        other.item_id().expect("mints")
    );
    let extended = first.item_id_at(HexLen::Twelve).expect("mints");
    assert!(
        extended
            .as_str()
            .starts_with(first.item_id().expect("mints").as_str())
    );
}

/// A sibling's agent writing through its own binary.
fn author() -> Author {
    Author {
        actor: Actor::parse("agent").expect("an actor"),
        trust: Trust::asserted(),
        writer: Writer::new("rust", "2.0.0").expect("a writer"),
    }
}

#[test]
fn the_raise_event_is_tagged_with_its_item_and_the_request_channel() {
    let author = author();
    let request = golden();
    let id = request.item_id().expect("mints");
    let envelope = Envelope::new(ts(TS), author, request);
    let event = raise_event(&envelope).expect("a valid request makes an event");
    let dir = tempfile::tempdir().expect("a temp dir");
    let store = Store::open(&dir.path().join("events.sqlite3"), Wait::Command).expect("store");
    store.append(vec![event], None).expect("append");
    let read = store
        .read(&Filter::all(), 0, NonZeroUsize::MIN)
        .expect("read");
    assert_eq!(read.len(), 1, "{read:?}");
    let stored = read.first().expect("one event");
    assert_eq!(stored.event_type, "weighin.request_raised");
    let tags: Vec<&str> = stored.tags.iter().map(Tag::as_str).collect();
    let item = format!("item:{id}");
    assert!(tags.contains(&item.as_str()), "{tags:?}");
    assert!(tags.contains(&"weighin.channel:request"), "{tags:?}");
    let back = decode::<Request>(stored).expect("the request reads back");
    assert_eq!(back.data, golden());
}

#[test]
fn the_raise_event_checks_the_guards_first() {
    let author = author();
    let mut request = golden();
    request.deadline = Some("2026-01-01T00:00:00Z".to_owned());
    let envelope = Envelope::new(ts(TS), author, request);
    assert!(matches!(
        raise_event(&envelope),
        Err(RequestError::DeadlinePassed { .. })
    ));
}
