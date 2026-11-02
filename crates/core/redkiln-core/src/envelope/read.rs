//! Lenient typed reads (SPEC EV-08 to EV-10).

use serde_json::Value;

use super::fields::{self, Field, FieldError};
use super::strict::Strict;
use super::wire::{EnvelopeIn, TrustIn};
use super::{Actor, Envelope, MAX_ENVELOPE_BYTES, Payload, Timestamp, Writer};
use crate::ids::{IdError, SessionId, WorktreeId, parse_event_ref};
use crate::store::Stored;
use crate::trust::{Trust, TrustError};

/// Why an event was skipped rather than read.
#[derive(Debug, thiserror::Error)]
pub enum Skip {
    /// Another type than the one asked for: skipped without error (EV-08).
    #[error("an event of another type")]
    Foreign,
    /// The type asked for, in a form this build cannot use (EV-10). A gate treats it as absent.
    #[error("an event this build cannot use")]
    Unusable(#[source] Unusable),
}

/// Why an event of a known type cannot be used (EV-10).
#[derive(Debug, thiserror::Error)]
pub enum Unusable {
    /// The stored bytes are larger than any envelope (EV-16).
    #[error("the envelope is {len} bytes, more than {MAX_ENVELOPE_BYTES}")]
    TooLarge {
        /// The stored length.
        len: usize,
    },
    /// The stored bytes are not an envelope: not a JSON object, an object naming a key twice at
    /// any depth, or missing a required key.
    #[error("the stored bytes are not an envelope")]
    Json(#[source] serde_json::Error),
    /// The envelope's `type` is not the stored event's type.
    #[error("the envelope says type {envelope}, the store {stored}")]
    TypeMismatch {
        /// The stored event's type.
        stored: String,
        /// The envelope's `type`.
        envelope: String,
    },
    /// A newer version than this build's contract knows.
    #[error("version {found} is newer than the {known} this build reads")]
    NewerVersion {
        /// The envelope's version.
        found: u64,
        /// The newest version this build reads.
        known: u32,
    },
    /// An envelope field breaks its rule.
    #[error(transparent)]
    Field(FieldError),
    /// The trust object breaks TR-01's rules, or names a level or `via` this build does not
    /// know. Never read as a lower level: the event is unusable, so a gate fails closed.
    #[error("the envelope's trust is not one this build accepts")]
    Trust(#[source] TrustError),
    /// An id field (`session`, `worktree`, `origin`, `cites`) is malformed.
    #[error("the envelope's {field} is malformed")]
    Id {
        /// The field.
        field: Field,
        /// What is wrong with it.
        #[source]
        source: IdError,
    },
    /// The payload does not decode at its version, or no upcaster reads that version.
    #[error("the payload does not decode")]
    Payload(#[source] serde_json::Error),
}

/// The events of one type in a read, and how many others were skipped (EV-08, EV-10). The
/// `unusable` count is what a plugin's `status --json` reports (CLI-12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Decoded<P> {
    /// The usable events, in the order read.
    pub events: Vec<Envelope<P>>,
    /// Events of other types, skipped.
    pub foreign: u64,
    /// Events of this type this build cannot use, skipped.
    pub unusable: u64,
}

/// Reads `stored` as an envelope of `P`.
///
/// # Errors
/// [`Skip::Foreign`] for another type; [`Skip::Unusable`] for this type in a form this build
/// cannot use. Neither is a failure of the read: the caller skips the event (and counts it).
pub fn decode<P: Payload>(stored: &Stored) -> Result<Envelope<P>, Skip> {
    if stored.event_type != P::TYPE {
        return Err(Skip::Foreign);
    }
    read_known(stored).map_err(Skip::Unusable)
}

/// Reads every event of `P` among `stored`, skipping and counting the rest.
pub fn decode_all<'a, P: Payload>(stored: impl IntoIterator<Item = &'a Stored>) -> Decoded<P> {
    let mut decoded = Decoded {
        events: Vec::new(),
        foreign: 0,
        unusable: 0,
    };
    for event in stored {
        match decode(event) {
            Ok(envelope) => decoded.events.push(envelope),
            Err(Skip::Foreign) => decoded.foreign = decoded.foreign.saturating_add(1),
            Err(Skip::Unusable(_)) => decoded.unusable = decoded.unusable.saturating_add(1),
        }
    }
    decoded
}

fn read_known<P: Payload>(stored: &Stored) -> Result<Envelope<P>, Unusable> {
    let len = stored.data.len();
    if len > MAX_ENVELOPE_BYTES {
        return Err(Unusable::TooLarge { len });
    }
    // Strictly, so a repeated key is unusable rather than read last-wins; and an object first,
    // so serde never reads an array into the envelope's fields.
    let Strict(value) = serde_json::from_slice(&stored.data).map_err(Unusable::Json)?;
    let Value::Object(object) = value else {
        return Err(Unusable::Json(serde::de::Error::custom(
            "an envelope is a JSON object",
        )));
    };
    let wire: EnvelopeIn = serde_json::from_value(Value::Object(object)).map_err(Unusable::Json)?;
    if wire.event_type != stored.event_type {
        return Err(Unusable::TypeMismatch {
            stored: stored.event_type.clone(),
            envelope: wire.event_type,
        });
    }
    let version = read_version::<P>(wire.version)?;
    let envelope = Envelope {
        ts: Timestamp::parse(&wire.ts).map_err(Unusable::Field)?,
        actor: Actor::parse(&wire.actor).map_err(Unusable::Field)?,
        trust: read_trust(wire.trust).map_err(Unusable::Trust)?,
        writer: Writer::from_parts(wire.writer.binary, wire.writer.version, wire.writer.spec)
            .map_err(Unusable::Field)?,
        session: read_id(wire.session.as_deref(), SessionId::parse, Field::Session)?,
        worktree: read_id(wire.worktree.as_deref(), WorktreeId::parse, Field::Worktree)?,
        branch: checked(wire.branch, fields::check_branch)?,
        head: checked(wire.head, fields::check_head)?,
        origin: read_id(wire.origin.as_deref(), parse_event_ref, Field::Origin)?,
        // An absent `cites` is no citation.
        cites: wire
            .cites
            .unwrap_or_default()
            .iter()
            .map(|cite| parse_event_ref(cite))
            .collect::<Result<_, _>>()
            .map_err(|source| Unusable::Id {
                field: Field::Cites,
                source,
            })?,
        data: read_payload::<P>(version, wire.data).map_err(Unusable::Payload)?,
    };
    Ok(envelope)
}

/// The envelope's version when `P` reads it: from 1, and no newer than [`Payload::VERSION`].
fn read_version<P: Payload>(found: u64) -> Result<u32, Unusable> {
    match u32::try_from(found) {
        Ok(0) => Err(Unusable::Field(FieldError {
            field: Field::Version,
            reason: "must be an integer from 1",
        })),
        Ok(version) if version <= P::VERSION => Ok(version),
        _ => Err(Unusable::NewerVersion {
            found,
            known: P::VERSION,
        }),
    }
}

/// An optional id field, parsed when present.
fn read_id<T>(
    text: Option<&str>,
    parse: fn(&str) -> Result<T, IdError>,
    field: Field,
) -> Result<Option<T>, Unusable> {
    text.map(parse)
        .transpose()
        .map_err(|source| Unusable::Id { field, source })
}

/// The payload at `version`, upcast when it is older than the newest.
fn read_payload<P: Payload>(version: u32, data: Value) -> Result<P, serde_json::Error> {
    if version == P::VERSION {
        serde_json::from_value(data)
    } else {
        P::upcast(version, data)
    }
}

/// The trust object under [`Trust::new`]'s rules, with its evidence and claimed level.
fn read_trust(wire: TrustIn) -> Result<Trust, TrustError> {
    let via = wire.via.as_deref().map(str::parse).transpose()?;
    let mut trust = Trust::new(wire.level.parse()?, via)?;
    if let Some(by) = wire.by {
        trust = trust.with_by(by)?;
    }
    if let Some(reference) = wire.reference {
        trust = trust.with_reference(reference)?;
    }
    let claimed = wire.claimed.as_deref().map(str::parse).transpose()?;
    Ok(trust.with_claimed(claimed))
}

/// `value` when `check` accepts it.
fn checked(
    value: Option<String>,
    check: fn(&str) -> Result<(), FieldError>,
) -> Result<Option<String>, Unusable> {
    if let Some(text) = &value {
        check(text).map_err(Unusable::Field)?;
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use happenstance_core::{EventId, SequencePosition};

    use super::*;
    use crate::envelope::Class;
    use crate::envelope::testing::{ItemDecided, SPEC_EXAMPLE, ToolTimed, spec_example};
    use crate::ids::parse_store_id;

    fn stored(event_type: &str, data: &str) -> Stored {
        let store = parse_store_id("0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f").expect("valid");
        let position = SequencePosition::new(1).expect("non-zero");
        Stored {
            position: 1,
            id: EventId::new(store, position),
            recorded_at_millis: 0,
            event_type: event_type.to_owned(),
            data: data.as_bytes().to_vec(),
            tags: Vec::new(),
        }
    }

    fn example_with(edit: impl FnOnce(&mut Value)) -> String {
        let mut value: Value = serde_json::from_str(SPEC_EXAMPLE).expect("json");
        edit(&mut value);
        value.to_string()
    }

    #[test]
    fn the_spec_example_reads_back() {
        let read = decode::<ItemDecided>(&stored("weighin.item_decided", SPEC_EXAMPLE));
        assert_eq!(read.expect("usable"), spec_example());
    }

    #[test]
    fn another_type_is_skipped_as_foreign() {
        let read = decode::<ToolTimed>(&stored("weighin.item_decided", SPEC_EXAMPLE));
        assert!(matches!(read, Err(Skip::Foreign)), "{read:?}");
    }

    #[test]
    fn unknown_keys_are_ignored_in_the_envelope_and_in_data() {
        let text = example_with(|v| {
            v["future"] = serde_json::json!({"anything": [1, 2]});
            v["data"]["later_field"] = serde_json::json!("ignored");
        });
        let read = decode::<ItemDecided>(&stored("weighin.item_decided", &text));
        assert_eq!(read.expect("usable"), spec_example());
    }

    #[test]
    fn null_optional_keys_read_as_absent() {
        let text = example_with(|v| {
            for key in ["session", "worktree", "branch", "head", "origin", "cites"] {
                v[key] = Value::Null;
            }
            v["trust"]["by"] = Value::Null;
        });
        let read = decode::<ItemDecided>(&stored("weighin.item_decided", &text)).expect("usable");
        assert_eq!(read.session, None);
        assert_eq!(read.worktree, None);
        assert_eq!(read.branch, None);
        assert_eq!(read.head, None);
        assert_eq!(read.origin, None);
        assert!(read.cites.is_empty(), "{:?}", read.cites);
        assert_eq!(read.trust.by(), None);
    }

    #[test]
    fn a_newer_version_is_unusable_not_an_error() {
        let text = example_with(|v| v["version"] = serde_json::json!(2));
        let read = decode::<ItemDecided>(&stored("weighin.item_decided", &text));
        assert!(
            matches!(
                read,
                Err(Skip::Unusable(Unusable::NewerVersion {
                    found: 2,
                    known: 1
                }))
            ),
            "{read:?}"
        );
    }

    #[test]
    fn a_payload_that_does_not_decode_is_unusable() {
        let text = example_with(|v| v["data"] = serde_json::json!({"item": 7}));
        let read = decode::<ItemDecided>(&stored("weighin.item_decided", &text));
        assert!(
            matches!(read, Err(Skip::Unusable(Unusable::Payload(_)))),
            "{read:?}"
        );
    }

    #[test]
    fn an_older_version_without_an_upcaster_is_unusable() {
        let text = example_with(|v| v["version"] = serde_json::json!(1));
        // ItemDecided is at version 1, so test the rule through a type at version 2.
        let read = decode::<Revised>(&stored("weighin.item_decided", &text));
        assert!(
            matches!(read, Err(Skip::Unusable(Unusable::Payload(_)))),
            "{read:?}"
        );
    }

    #[test]
    fn an_upcaster_reads_an_older_version() {
        let read = decode::<Upcast>(&stored("weighin.item_decided", SPEC_EXAMPLE)).expect("usable");
        assert_eq!(read.data, Upcast { from: 1 });
    }

    #[test]
    fn a_bad_envelope_field_makes_the_event_unusable() {
        for (key, value) in [
            ("ts", serde_json::json!("yesterday")),
            ("actor", serde_json::json!("bot:x")),
            ("worktree", serde_json::json!("my-checkout")),
            ("cites", serde_json::json!(["not-a-ref"])),
            ("version", serde_json::json!(0)),
            ("type", serde_json::json!("weighin.item_raised")),
        ] {
            let text = example_with(|v| v[key] = value);
            let read = decode::<ItemDecided>(&stored("weighin.item_decided", &text));
            assert!(matches!(read, Err(Skip::Unusable(_))), "{key}: {read:?}");
        }
        let text =
            example_with(|v| v["trust"] = serde_json::json!({"level": "attested", "via": "board"}));
        let read = decode::<ItemDecided>(&stored("weighin.item_decided", &text));
        assert!(
            matches!(read, Err(Skip::Unusable(Unusable::Trust(_)))),
            "a level its via cannot back fails closed: {read:?}"
        );
    }

    #[test]
    fn bytes_that_are_not_an_envelope_are_unusable() {
        for data in ["", "[]", "{\"type\":", "{}"] {
            let read = decode::<ItemDecided>(&stored("weighin.item_decided", data));
            assert!(
                matches!(read, Err(Skip::Unusable(Unusable::Json(_)))),
                "{data:?}: {read:?}"
            );
        }
    }

    #[test]
    fn a_repeated_key_makes_the_event_unusable_not_read_last_wins() {
        let edits = [
            // A second `trust`, lower than the first: last-wins would read it as asserted.
            (
                "trust",
                SPEC_EXAMPLE.replacen(
                    r#""writer":"#,
                    r#""trust":{"level":"asserted"},"writer":"#,
                    1,
                ),
            ),
            (
                "type",
                SPEC_EXAMPLE.replacen(
                    r#""version":"#,
                    r#""type":"weighin.item_decided","version":"#,
                    1,
                ),
            ),
            (
                "trust.level",
                SPEC_EXAMPLE.replacen(
                    r#""level":"witnessed""#,
                    r#""level":"witnessed","level":"asserted""#,
                    1,
                ),
            ),
        ];
        for (key, text) in edits {
            assert_ne!(text, SPEC_EXAMPLE, "{key}: the edit applied");
            let read = decode::<ItemDecided>(&stored("weighin.item_decided", &text));
            match read {
                Err(Skip::Unusable(Unusable::Json(error))) => {
                    assert!(
                        error.to_string().contains("duplicate key"),
                        "{key}: {error}"
                    );
                }
                other => panic!("{key}: expected Unusable::Json, got {other:?}"),
            }
        }
    }

    #[test]
    fn an_oversized_envelope_is_unusable() {
        let text = example_with(|v| {
            v["data"]["later_field"] = serde_json::json!("x".repeat(MAX_ENVELOPE_BYTES));
        });
        let read = decode::<ItemDecided>(&stored("weighin.item_decided", &text));
        assert!(
            matches!(read, Err(Skip::Unusable(Unusable::TooLarge { .. }))),
            "{read:?}"
        );
    }

    #[test]
    fn decoding_many_counts_what_it_skips() {
        let newer = example_with(|v| v["version"] = serde_json::json!(9));
        let events = [
            stored("weighin.item_decided", SPEC_EXAMPLE),
            stored("bench.point", "{}"),
            stored("weighin.item_decided", &newer),
            stored("loop.run_started", "{}"),
        ];
        let decoded = decode_all::<ItemDecided>(&events);
        assert_eq!(decoded.events, vec![spec_example()]);
        assert_eq!(decoded.foreign, 2);
        assert_eq!(decoded.unusable, 1);
    }

    /// `ItemDecided` at a version 2 with no upcaster.
    #[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    struct Revised {
        item: String,
    }

    impl Payload for Revised {
        const TYPE: &'static str = "weighin.item_decided";
        const VERSION: u32 = 2;
        const CLASS: Class = Class::Durable;
    }

    /// `ItemDecided` at a version 2 whose upcaster reads version 1.
    #[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
    struct Upcast {
        from: u32,
    }

    impl Payload for Upcast {
        const TYPE: &'static str = "weighin.item_decided";
        const VERSION: u32 = 2;
        const CLASS: Class = Class::Durable;

        fn upcast(version: u32, data: Value) -> Result<Self, serde_json::Error> {
            assert!(data.is_object(), "the older payload is handed over");
            Ok(Self { from: version })
        }
    }
}
