//! Envelope goldens (`spec/fixtures/envelope/`) through Core: EV-01, EV-08 to EV-12, TR-01,
//! TR-10 and AP-01. Each valid golden decodes, encodes back to its own bytes, and is stored with
//! the tags Core sets; lenient ones decode as their valid twin; invalid ones are unusable.

use std::fmt::Debug;

use redkiln_core::envelope::{Envelope, Field, Payload, Skip, Unusable, decode};
use redkiln_core::journal::ImportCompleted;
use redkiln_core::store::{CoreKey, Tag};
use redkiln_core::subject::Subject;
use redkiln_core::trust::TrustError;

use super::schema::envelope_conforms;
use super::{
    ApprovalRecorded, ItemDecided, ItemRaised, all, fresh_store, golden, one_line, stored_of,
};

/// The golden `rel`, decoded as `P`.
fn decoded<P: Payload>(rel: &str) -> Envelope<P> {
    decode::<P>(&stored_of(&one_line(rel))).unwrap_or_else(|e| panic!("{rel} decodes: {e:?}"))
}

/// The tags Core sets on `envelope`'s event (EV-12): `plugin` and `trust`, and `session` and
/// `worktree` when it has them, sorted.
fn core_tags<P: Payload>(envelope: &Envelope<P>) -> Vec<String> {
    let namespace = P::TYPE.split('.').next().expect("a namespace");
    let mut tags = vec![
        format!("plugin:{namespace}"),
        format!("trust:{}", envelope.trust.level()),
    ];
    if let Some(session) = &envelope.session {
        tags.push(format!("session:{}", session.as_str()));
    }
    if let Some(worktree) = &envelope.worktree {
        tags.push(format!("worktree:{}", worktree.as_str()));
    }
    tags.sort();
    tags
}

/// The golden `rel` decodes as `P`, encodes back to its bytes, passes its schema, and is what
/// Core stores for it, with the tags Core sets and no other.
pub(super) fn round_trips<P: Payload + Debug>(rel: &str) -> Envelope<P> {
    let bytes = one_line(rel);
    let envelope = decoded::<P>(rel);
    let encoded = envelope.encode().expect("encodes");
    assert_eq!(
        String::from_utf8_lossy(&encoded),
        String::from_utf8_lossy(&bytes),
        "{rel} byte for byte"
    );
    envelope_conforms(&encoded);

    let (_dir, store) = fresh_store();
    let event = envelope.to_event([]).expect("an event");
    store.append(vec![event], None).expect("append");
    let stored = all(&store);
    let [only] = stored.as_slice() else {
        panic!("one event: {stored:?}");
    };
    assert_eq!(only.event_type, P::TYPE);
    assert_eq!(only.data, bytes, "{rel} stored byte for byte");
    let mut tags: Vec<String> = only.tags.iter().map(|t| t.as_str().to_owned()).collect();
    tags.sort();
    assert_eq!(tags, core_tags(&envelope), "{rel}");
    envelope
}

/// Whether Core's typed read of an envelope golden's `bytes` gives an envelope, for the type it
/// names.
pub(super) fn decodes(bytes: &[u8]) -> bool {
    let stored = stored_of(bytes);
    match stored.event_type.as_str() {
        ItemDecided::TYPE => decode::<ItemDecided>(&stored).is_ok(),
        ItemRaised::TYPE => decode::<ItemRaised>(&stored).is_ok(),
        ImportCompleted::TYPE => decode::<ImportCompleted>(&stored).is_ok(),
        ApprovalRecorded::TYPE => decode::<ApprovalRecorded>(&stored).is_ok(),
        other => panic!("no payload type for {other}"),
    }
}

#[test]
fn ev_01_golden_decodes_and_encodes_back_byte_for_byte() {
    let envelope = round_trips::<ItemDecided>("envelope/valid/ev-01-item-decided.json");
    assert_eq!(envelope.data.option, "a");
    assert_eq!(envelope.cites.len(), 1);
}

#[test]
fn ev_01_golden_is_what_core_stores_with_core_tags() {
    // `round_trips` stores the envelope and checks the stored tags are `core_tags`'s.
    let envelope = round_trips::<ItemDecided>("envelope/valid/ev-01-item-decided.json");
    assert_eq!(
        core_tags(&envelope),
        [
            "plugin:weighin",
            "session:8b6fb94b-0975-5275-b434-6c9e60544265",
            "trust:witnessed",
            "worktree:wt-5d41402abc4b",
        ]
    );
}

#[test]
fn minimal_asserted_golden_round_trips_with_no_optional_keys() {
    let envelope = round_trips::<ItemRaised>("envelope/valid/minimal-asserted.json");
    assert_eq!(
        (
            &envelope.session,
            &envelope.worktree,
            &envelope.branch,
            &envelope.head,
            &envelope.origin,
        ),
        (&None, &None, &None, &None, &None)
    );
    assert!(envelope.cites.is_empty(), "no cites");
    assert_eq!(envelope.trust.via(), None);
}

#[test]
fn approval_goldens_round_trip() {
    for rel in [
        "envelope/valid/ap-01-approval-recorded.json",
        "envelope/valid/approval-attested-signed-commit.json",
        "envelope/valid/approval-github-review-asserted.json",
    ] {
        let envelope = round_trips::<ApprovalRecorded>(rel);
        // AP-01: when `trust.by` is present, `decider` equals it.
        if let Some(by) = envelope.trust.by() {
            assert_eq!(envelope.data.decider, by, "{rel}");
        }
    }
}

#[test]
fn approval_subjects_parse_as_core_subjects_of_their_method() {
    for rel in [
        "envelope/valid/ap-01-approval-recorded.json",
        "envelope/valid/approval-attested-signed-commit.json",
        "envelope/valid/approval-github-review-asserted.json",
    ] {
        let subject = decoded::<ApprovalRecorded>(rel).data.subject;
        let parsed = Subject::parse(&subject.hash).expect("a subject Core reads");
        assert_eq!(parsed.method().as_str(), subject.method, "{rel}");
        // The subject tag AP-01 names is a valid core tag.
        Tag::core(CoreKey::Subject, parsed.as_str()).expect("a subject tag");
    }
}

#[test]
fn lenient_envelopes_decode_as_their_valid_twin() {
    assert_eq!(
        decoded::<ItemRaised>("envelope/lenient/null-optional-keys.json"),
        decoded::<ItemRaised>("envelope/valid/minimal-asserted.json"),
        "EV-01: null reads as absent"
    );
    assert_eq!(
        decoded::<ItemDecided>("envelope/lenient/unknown-keys.json"),
        decoded::<ItemDecided>("envelope/valid/ev-01-item-decided.json"),
        "EV-09: unknown keys are ignored"
    );
}

/// Whether a reason is the one a fixture expects.
type Reason = fn(&Unusable) -> bool;

/// Each invalid envelope, and the reason Core gives for not using it.
const INVALID: &[(&str, Reason)] = &[
    ("data-flattened.json", |u| matches!(u, Unusable::Json(_))),
    (
        "version-zero.json",
        |u| matches!(u, Unusable::Field(f) if f.field == Field::Version),
    ),
    (
        "ts-offset.json",
        |u| matches!(u, Unusable::Field(f) if f.field == Field::Ts),
    ),
    ("trust-unknown-level.json", |u| {
        matches!(u, Unusable::Trust(TrustError::UnknownLevel))
    }),
    ("trust-witnessed-without-via.json", |u| {
        matches!(u, Unusable::Trust(TrustError::MissingVia { .. }))
    }),
    ("worktree-path.json", |u| {
        matches!(
            u,
            Unusable::Id {
                field: Field::Worktree,
                ..
            }
        )
    }),
    ("cites-not-event-ref.json", |u| {
        matches!(
            u,
            Unusable::Id {
                field: Field::Cites,
                ..
            }
        )
    }),
    ("data-not-object.json", |u| {
        matches!(u, Unusable::Payload(_))
    }),
    ("duplicate-key.json", |u| matches!(u, Unusable::Json(_))),
];

#[test]
fn invalid_envelopes_are_unusable() {
    for (name, expected) in INVALID {
        let bytes = golden(&format!("envelope/invalid/{name}"));
        match decode::<ItemDecided>(&stored_of(&bytes)) {
            Err(Skip::Unusable(reason)) => assert!(expected(&reason), "{name}: {reason:?}"),
            other => panic!("{name} is unusable: {other:?}"),
        }
    }
}
