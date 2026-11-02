//! The event envelope (SPEC EV-01 to EV-16).
//!
//! Every fact in the store is one [`Envelope`]: a JSON object with the payload nested under
//! `data`, stored as the data of one happenstance event whose type is the envelope's `type`.
//! A type's owner declares it with [`Payload`]: its name, its newest version, its [`Class`] and
//! how it reads older versions (EV-04, EV-06, EV-07, EV-14).
//!
//! # Writing is strict (EV-05)
//!
//! [`Envelope::to_event`] and [`Envelope::to_telemetry`] build nothing unless every rule holds:
//! a registered type in the grammar within 64 bytes, a version from 1, a valid `ts`, actor,
//! trust and writer, a `data` that is a JSON object with no NaN or infinite float, at most
//! [`MAX_ENVELOPE_BYTES`] in all, and
//! tags in Core's registry. Core adds the `plugin`, `trust`, `session` and `worktree` tags itself
//! (EV-12).
//!
//! # Reading is lenient (EV-08 to EV-10)
//!
//! [`decode`] skips another type ([`Skip::Foreign`]), ignores unknown keys, reads `null` as
//! absent, and reports a known type it cannot use ([`Skip::Unusable`]) without failing the
//! read; [`decode_all`] counts both. An envelope naming a key twice is unusable, never read
//! last-wins.
//!
//! # Example
//!
//! ```
//! use redkiln_core::envelope::{
//!     Actor, Author, Class, Envelope, Payload, Timestamp, Writer, decode,
//! };
//! use redkiln_core::store::{CoreKey, Store, Tag, Wait};
//! use redkiln_core::trust::Trust;
//!
//! #[derive(Debug, PartialEq, serde::Serialize, serde::Deserialize)]
//! struct ItemRaised {
//!     question: String,
//! }
//!
//! impl Payload for ItemRaised {
//!     const TYPE: &'static str = "weighin.item_raised";
//!     const VERSION: u32 = 1;
//!     const CLASS: Class = Class::Durable;
//! }
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let author = Author {
//!     actor: Actor::Cli,
//!     trust: Trust::asserted(),
//!     writer: Writer::new("weighin", "2.0.0")?,
//! };
//! let ts = Timestamp::from_system_time(std::time::SystemTime::now())?;
//! let raised = Envelope::new(ts, author, ItemRaised { question: "Which store?".into() });
//! let event = raised.to_event([Tag::core(CoreKey::Item, "wi-3fa9c01b2e")?])?;
//!
//! let dir = tempfile::tempdir()?;
//! let store = Store::open(&dir.path().join("events.sqlite3"), Wait::Command)?;
//! store.append(vec![event], None)?;
//! let stored = store.read(&redkiln_core::store::Filter::all(), 0, 10.try_into()?)?;
//! let read = stored.first().map(decode::<ItemRaised>).transpose()?;
//! assert_eq!(read.map(|e| e.data), Some(raised.data));
//! # Ok(())
//! # }
//! ```

mod fields;
mod finite;
mod name;
mod read;
mod strict;
mod wire;

use happenstance_core::EventId;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::canonical::{self, CanonicalError};
use crate::ids::{Digest, SessionId, WorktreeId};
use crate::store::{CoreKey, NewEvent, StoreError, Tag, TagError, TelemetryEvent};
use crate::trust::Trust;

pub use fields::{Actor, ActorName, Field, FieldError, SPEC_VERSION, Timestamp, Writer};
pub use name::{MAX_TYPE_BYTES, NameError, Namespace, TypeName};
pub use read::{Decoded, Skip, Unusable, decode, decode_all};

pub(crate) use name::is_dotted_name;

/// The largest envelope, serialised (EV-16): 64 KiB.
pub const MAX_ENVELOPE_BYTES: usize = 64 * 1024;

/// A type's class (EV-14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Class {
    /// A fact a decision, gate or record depends on: written under the busy timeout, exported,
    /// never pruned.
    Durable,
    /// Measurement: dropped when the lock is busy (ST-09), never exported.
    Telemetry,
}

/// A payload type, declared by its namespace's owner (EV-04): the type it is written as, its
/// newest version, its class, and how it reads an older version (EV-07).
pub trait Payload: Serialize + DeserializeOwned {
    /// The type, `<namespace>.<name>` (EV-03).
    const TYPE: &'static str;
    /// The newest version, from 1: the one written, and the one read without upcasting (EV-06).
    const VERSION: u32;
    /// Durable or telemetry (EV-14).
    const CLASS: Class;

    /// Reads `data` written at `version`, older than [`Self::VERSION`] (EV-07). By default no
    /// older version is readable, and its events are [`Unusable::Payload`].
    ///
    /// # Errors
    /// A serde error when `data` does not decode at `version`.
    fn upcast(version: u32, data: Value) -> Result<Self, serde_json::Error> {
        drop(data);
        Err(serde::de::Error::custom(format_args!(
            "{} has no upcaster from version {version}",
            Self::TYPE
        )))
    }
}

/// Why an envelope was not built (EV-05). Nothing was written.
#[derive(Debug, thiserror::Error)]
pub enum EnvelopeError {
    /// The payload's type is not a registered, well-formed type (EV-03).
    #[error("invalid event type")]
    Type(#[source] NameError),
    /// The payload's version is 0; versions start at 1 (EV-01).
    #[error("a payload version starts at 1")]
    Version,
    /// The type's class is not the one this path writes: a telemetry type goes through
    /// [`Envelope::to_telemetry`], a durable one through [`Envelope::to_event`].
    #[error("the type is declared {declared:?}, which this path does not write")]
    Class {
        /// The class the type declares.
        declared: Class,
    },
    /// An envelope field breaks its rule.
    #[error(transparent)]
    Field(FieldError),
    /// The payload does not serialise to a JSON object (EV-01).
    #[error("the payload must serialise to a JSON object")]
    DataNotObject,
    /// The payload holds a NaN or an infinity, which JSON cannot hold: `serde_json` would write
    /// `null`, and the event would read back unusable.
    #[error("the payload holds a NaN or infinite float, which JSON cannot hold")]
    NonFinite,
    /// The payload or the envelope could not be serialised.
    #[error("serialising the envelope")]
    Json(#[source] serde_json::Error),
    /// The envelope serialises to more than [`MAX_ENVELOPE_BYTES`] (EV-16).
    #[error("the envelope is {len} bytes, more than {MAX_ENVELOPE_BYTES}")]
    TooLarge {
        /// Its serialised length.
        len: usize,
    },
    /// A tag is invalid, outside Core's registry, or one Core sets itself (EV-11, EV-12).
    #[error("invalid tag")]
    Tag(#[source] TagError),
    /// happenstance refused the event.
    #[error("happenstance refused the event")]
    Event(#[source] StoreError),
}

/// Who wrote an envelope and on whose say: its `actor`, `trust` and `writer` (EV-01), the three
/// keys an [`Envelope::new`] takes together.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Author {
    /// Who the writer says acted.
    pub actor: Actor,
    /// The trust level and its evidence.
    pub trust: Trust,
    /// The binary that wrote it.
    pub writer: Writer,
}

/// An event's envelope (EV-01) around its payload `data`.
///
/// `type` and `version` are the payload's ([`Payload::TYPE`], [`Payload::VERSION`]); the
/// store's own values (position, `recorded_at`) are not part of it (EV-02).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope<P> {
    /// When the writer wrote it.
    pub ts: Timestamp,
    /// Who the writer says acted.
    pub actor: Actor,
    /// The trust level and its evidence.
    pub trust: Trust,
    /// The binary that wrote it.
    pub writer: Writer,
    /// The agent session the write belongs to.
    pub session: Option<SessionId>,
    /// The writer's worktree id.
    pub worktree: Option<WorktreeId>,
    /// The checked-out branch; absent outside git or on a detached head.
    pub branch: Option<String>,
    /// The commit at `HEAD`; absent outside git or before the first commit.
    pub head: Option<String>,
    /// The event's identity, once it has left its first store (ID-02).
    pub origin: Option<EventId>,
    /// Events this one answers or depends on (ID-03); empty is absent.
    pub cites: Vec<EventId>,
    /// The payload.
    pub data: P,
}

impl<P> Envelope<P> {
    /// An envelope of `data`, written at `ts` by `author`, with every optional key absent.
    #[must_use]
    pub fn new(ts: Timestamp, author: Author, data: P) -> Self {
        let Author {
            actor,
            trust,
            writer,
        } = author;
        Self {
            ts,
            actor,
            trust,
            writer,
            session: None,
            worktree: None,
            branch: None,
            head: None,
            origin: None,
            cites: Vec::new(),
            data,
        }
    }
}

impl<P: Payload> Envelope<P> {
    /// The envelope's JSON bytes, as stored.
    ///
    /// # Errors
    /// [`EnvelopeError`] naming the first rule of EV-05 the envelope breaks.
    pub fn encode(&self) -> Result<Vec<u8>, EnvelopeError> {
        self.encode_named().map(|(_, bytes)| bytes)
    }

    /// A durable event of this envelope, with Core's tags and then `tags`.
    ///
    /// # Errors
    /// [`EnvelopeError::Class`] for a telemetry type; otherwise as [`Envelope::encode`], and
    /// [`EnvelopeError::Tag`] for a tag outside Core's registry or one Core sets.
    pub fn to_event(&self, tags: impl IntoIterator<Item = Tag>) -> Result<NewEvent, EnvelopeError> {
        match P::CLASS {
            Class::Durable => self.new_event(tags),
            declared @ Class::Telemetry => Err(EnvelopeError::Class { declared }),
        }
    }

    /// A telemetry event of this envelope, for [`crate::store::Store::record_telemetry`], which
    /// drops it rather than wait on a busy lock (ST-09).
    ///
    /// # Errors
    /// [`EnvelopeError::Class`] for a durable type; otherwise as [`Envelope::to_event`].
    pub fn to_telemetry(
        &self,
        tags: impl IntoIterator<Item = Tag>,
    ) -> Result<TelemetryEvent, EnvelopeError> {
        match P::CLASS {
            Class::Telemetry => self.new_event(tags).map(TelemetryEvent::new),
            declared @ Class::Durable => Err(EnvelopeError::Class { declared }),
        }
    }

    /// The payload's canonical digest, over `data` alone (ID-08, EV-13).
    ///
    /// # Errors
    /// As [`canonical::digest_of`].
    pub fn data_digest(&self) -> Result<Digest, CanonicalError> {
        canonical::digest_of(&self.data)
    }

    fn encode_named(&self) -> Result<(TypeName, Vec<u8>), EnvelopeError> {
        let name = TypeName::parse(P::TYPE).map_err(EnvelopeError::Type)?;
        if P::VERSION == 0 {
            return Err(EnvelopeError::Version);
        }
        if let Some(branch) = &self.branch {
            fields::check_branch(branch).map_err(EnvelopeError::Field)?;
        }
        if let Some(head) = &self.head {
            fields::check_head(head).map_err(EnvelopeError::Field)?;
        }
        // The payload is written as its type serialises it, keys in its own order; a JSON
        // value would sort them.
        let data = serde_json::to_vec(&self.data).map_err(EnvelopeError::Json)?;
        finite::check(&self.data).map_err(|found| match found {
            finite::Found::NonFinite => EnvelopeError::NonFinite,
            // The payload's `Serialize` failed this time where it succeeded for serde_json.
            finite::Found::Custom(message) => {
                EnvelopeError::Json(serde::ser::Error::custom(message))
            }
        })?;
        if data.first() != Some(&b'{') {
            return Err(EnvelopeError::DataNotObject);
        }
        let wire = wire::EnvelopeOut::new(self, &name, P::VERSION);
        let bytes = wire::with_data(&wire, &data).map_err(EnvelopeError::Json)?;
        if bytes.len() > MAX_ENVELOPE_BYTES {
            return Err(EnvelopeError::TooLarge { len: bytes.len() });
        }
        Ok((name, bytes))
    }

    fn new_event(&self, tags: impl IntoIterator<Item = Tag>) -> Result<NewEvent, EnvelopeError> {
        let (name, bytes) = self.encode_named()?;
        let core = |key, value: &str| Tag::core(key, value).map_err(EnvelopeError::Tag);
        let mut event = NewEvent::new(name.as_str(), bytes)
            .map_err(EnvelopeError::Event)?
            .with_tag(core(CoreKey::Plugin, name.namespace().as_str())?)
            .with_tag(core(CoreKey::Trust, self.trust.level().as_str())?);
        if let Some(session) = &self.session {
            event = event.with_tag(core(CoreKey::Session, session.as_str())?);
        }
        if let Some(worktree) = &self.worktree {
            event = event.with_tag(core(CoreKey::Worktree, worktree.as_str())?);
        }
        for tag in tags {
            crate::store::check_caller_tag(&tag).map_err(EnvelopeError::Tag)?;
            event = event.with_tag(tag);
        }
        Ok(event)
    }
}

#[cfg(test)]
pub(crate) mod testing {
    //! Payloads and envelopes the unit tests share.

    use serde::Deserialize;

    use super::*;
    use crate::ids::parse_event_ref;
    use crate::trust::{Level, Via};

    /// The SPEC's EV-01 example, as one line.
    pub(crate) const SPEC_EXAMPLE: &str = r#"{"type":"weighin.item_decided","version":1,"ts":"2026-10-31T04:20:11Z","actor":"human:ryan","trust":{"level":"witnessed","via":"ask-user-question","ref":"toolu_01XyZ"},"writer":{"binary":"weighin","version":"2.0.0","spec":"1.0.0"},"session":"8b6fb94b-0975-5275-b434-6c9e60544265","worktree":"wt-5d41402abc4b","branch":"main","head":"bd234009c1e5a7d4f1c3b2a0e9f8d7c6b5a49382","cites":["0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:41"],"data":{"item":"wi-3fa9c01b2e","option":"a","applies":"once"}}"#;

    /// The example's payload.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub(crate) struct ItemDecided {
        pub(crate) item: String,
        pub(crate) option: String,
        pub(crate) applies: String,
    }

    impl Payload for ItemDecided {
        const TYPE: &'static str = "weighin.item_decided";
        const VERSION: u32 = 1;
        const CLASS: Class = Class::Durable;
    }

    /// A telemetry type.
    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    pub(crate) struct ToolTimed {
        pub(crate) millis: u64,
    }

    impl Payload for ToolTimed {
        const TYPE: &'static str = "core.tool_timed";
        const VERSION: u32 = 1;
        const CLASS: Class = Class::Telemetry;
    }

    /// The SPEC example as an envelope.
    pub(crate) fn spec_example() -> Envelope<ItemDecided> {
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
        envelope.session =
            Some(SessionId::parse("8b6fb94b-0975-5275-b434-6c9e60544265").expect("valid"));
        envelope.worktree = Some(WorktreeId::parse("wt-5d41402abc4b").expect("valid"));
        envelope.branch = Some("main".to_owned());
        envelope.head = Some("bd234009c1e5a7d4f1c3b2a0e9f8d7c6b5a49382".to_owned());
        envelope.cites =
            vec![parse_event_ref("0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:41").expect("valid")];
        envelope
    }

    /// A minimal envelope of `data`.
    pub(crate) fn minimal<P>(data: P) -> Envelope<P> {
        let author = Author {
            actor: Actor::Cli,
            trust: Trust::asserted(),
            writer: Writer::new("weighin", "2.0.0").expect("valid writer"),
        };
        Envelope::new(
            Timestamp::parse("2026-10-31T04:20:11Z").expect("valid ts"),
            author,
            data,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::testing::*;
    use super::*;

    #[test]
    fn the_spec_example_is_written_byte_for_byte() {
        let bytes = spec_example().encode().expect("valid");
        assert_eq!(String::from_utf8(bytes).expect("utf-8"), SPEC_EXAMPLE);
    }

    #[test]
    fn optional_keys_are_left_out_not_written_null() {
        let bytes = minimal(ToolTimed { millis: 7 }).encode().expect("valid");
        assert_eq!(
            String::from_utf8(bytes).expect("utf-8"),
            r#"{"type":"core.tool_timed","version":1,"ts":"2026-10-31T04:20:11Z","actor":"cli","trust":{"level":"asserted"},"writer":{"binary":"weighin","version":"2.0.0","spec":"1.0.0"},"data":{"millis":7}}"#
        );
    }

    #[test]
    fn an_event_carries_the_core_tags_then_the_callers() {
        let envelope = spec_example();
        let subject = Tag::core(CoreKey::Item, "wi-3fa9c01b2e").expect("an item tag");
        let channel = Tag::namespaced(Namespace::Weighin, "channel", "chat").expect("a tag");
        let event = envelope
            .to_event([subject.clone(), channel.clone()])
            .expect("valid");
        let tag = |key, value| Tag::core(key, value).expect("a core tag");
        let expected = NewEvent::new("weighin.item_decided", SPEC_EXAMPLE.as_bytes().to_vec())
            .expect("valid type")
            .with_tag(tag(CoreKey::Plugin, "weighin"))
            .with_tag(tag(CoreKey::Trust, "witnessed"))
            .with_tag(tag(
                CoreKey::Session,
                "8b6fb94b-0975-5275-b434-6c9e60544265",
            ))
            .with_tag(tag(CoreKey::Worktree, "wt-5d41402abc4b"))
            .with_tag(subject)
            .with_tag(channel);
        assert_eq!(event, expected);
    }

    #[test]
    fn a_caller_cannot_set_the_tags_core_sets() {
        for key in [
            CoreKey::Plugin,
            CoreKey::Trust,
            CoreKey::Session,
            CoreKey::Worktree,
        ] {
            let value = match key {
                CoreKey::Plugin => "weighin",
                CoreKey::Trust => "attested",
                CoreKey::Session => "s",
                _ => "wt-5d41402abc4b",
            };
            let tag = Tag::core(key, value).expect("a valid tag");
            assert!(
                matches!(
                    spec_example().to_event([tag]),
                    Err(EnvelopeError::Tag(TagError::CoreSet { key: k })) if k == key
                ),
                "{key:?}"
            );
        }
    }

    #[test]
    fn a_tag_outside_the_registry_is_refused() {
        let raw = Tag::new("channel", "chat").expect("a valid happenstance tag");
        assert!(matches!(
            spec_example().to_event([raw]),
            Err(EnvelopeError::Tag(TagError::Unregistered))
        ));
    }

    #[test]
    fn a_type_outside_the_registry_is_refused() {
        // Unit payloads: the type and version are checked before `data` is serialised.
        #[derive(Serialize, serde::Deserialize)]
        struct Foreign;
        impl Payload for Foreign {
            const TYPE: &'static str = "anneal.wave_started";
            const VERSION: u32 = 1;
            const CLASS: Class = Class::Durable;
        }
        assert!(matches!(
            minimal(Foreign).encode(),
            Err(EnvelopeError::Type(NameError::Namespace))
        ));
    }

    #[derive(Debug, Serialize, serde::Deserialize)]
    struct Versionless;

    impl Payload for Versionless {
        const TYPE: &'static str = "core.versionless";
        const VERSION: u32 = 0;
        const CLASS: Class = Class::Durable;
    }

    #[test]
    fn version_zero_is_refused() {
        assert!(matches!(
            minimal(Versionless).encode(),
            Err(EnvelopeError::Version)
        ));
    }

    #[derive(Debug, Serialize, serde::Deserialize)]
    struct Bare(String);

    impl Payload for Bare {
        const TYPE: &'static str = "core.bare";
        const VERSION: u32 = 1;
        const CLASS: Class = Class::Durable;
    }

    #[test]
    fn data_that_is_not_an_object_is_refused() {
        assert!(matches!(
            minimal(Bare("x".to_owned())).encode(),
            Err(EnvelopeError::DataNotObject)
        ));
    }

    #[derive(Debug, Serialize, serde::Deserialize)]
    struct Measured {
        ratio: f64,
        samples: Vec<Option<f32>>,
    }

    impl Payload for Measured {
        const TYPE: &'static str = "core.measured";
        const VERSION: u32 = 1;
        const CLASS: Class = Class::Telemetry;
    }

    #[test]
    fn a_non_finite_float_is_refused_not_written_as_null() {
        let finite = minimal(Measured {
            ratio: 0.5,
            samples: vec![Some(1.5), None],
        });
        finite.to_telemetry([]).expect("finite floats are written");
        for ratio in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let refused = minimal(Measured {
                ratio,
                samples: Vec::new(),
            })
            .encode();
            assert!(
                matches!(refused, Err(EnvelopeError::NonFinite)),
                "{ratio}: {refused:?}"
            );
        }
        let nested = minimal(Measured {
            ratio: 0.5,
            samples: vec![None, Some(f32::NAN)],
        });
        assert!(
            matches!(nested.to_telemetry([]), Err(EnvelopeError::NonFinite)),
            "found at any depth"
        );
    }

    #[derive(Debug, Serialize, serde::Deserialize)]
    struct Blob {
        text: String,
    }

    impl Payload for Blob {
        const TYPE: &'static str = "core.blob";
        const VERSION: u32 = 1;
        const CLASS: Class = Class::Durable;
    }

    #[test]
    fn an_envelope_over_64_kib_is_refused() {
        let empty = minimal(Blob {
            text: String::new(),
        })
        .encode()
        .expect("small")
        .len();
        let fits = minimal(Blob {
            text: "x".repeat(MAX_ENVELOPE_BYTES - empty),
        });
        assert_eq!(
            fits.encode().expect("exactly 64 KiB").len(),
            MAX_ENVELOPE_BYTES
        );
        let over = minimal(Blob {
            text: "x".repeat(MAX_ENVELOPE_BYTES - empty + 1),
        });
        assert!(matches!(
            over.encode(),
            Err(EnvelopeError::TooLarge { len }) if len == MAX_ENVELOPE_BYTES + 1
        ));
    }

    #[test]
    fn a_bad_branch_or_head_is_refused() {
        let mut envelope = spec_example();
        envelope.branch = Some(String::new());
        assert!(matches!(
            envelope.encode(),
            Err(EnvelopeError::Field(FieldError {
                field: Field::Branch,
                ..
            }))
        ));
        let mut envelope = spec_example();
        envelope.head = Some("HEAD".to_owned());
        assert!(matches!(
            envelope.encode(),
            Err(EnvelopeError::Field(FieldError {
                field: Field::Head,
                ..
            }))
        ));
    }

    #[test]
    fn each_class_has_its_own_path_to_the_store() {
        assert!(matches!(
            minimal(ToolTimed { millis: 1 }).to_event([]),
            Err(EnvelopeError::Class {
                declared: Class::Telemetry
            })
        ));
        assert!(matches!(
            spec_example().to_telemetry([]),
            Err(EnvelopeError::Class {
                declared: Class::Durable
            })
        ));
        let telemetry = minimal(ToolTimed { millis: 1 })
            .to_telemetry([])
            .expect("a telemetry type");
        let expected = NewEvent::new(
            "core.tool_timed",
            minimal(ToolTimed { millis: 1 }).encode().expect("valid"),
        )
        .expect("valid type")
        .with_tag(Tag::core(CoreKey::Plugin, "core").expect("a tag"))
        .with_tag(Tag::core(CoreKey::Trust, "asserted").expect("a tag"));
        assert_eq!(telemetry, TelemetryEvent::new(expected));
    }

    #[test]
    fn the_payload_digest_is_over_data_alone() {
        let envelope = spec_example();
        let mut other = spec_example();
        other.branch = None;
        other.ts = Timestamp::parse("2027-01-01T00:00:00Z").expect("valid");
        let digest = envelope.data_digest().expect("digest");
        assert_eq!(
            digest,
            other.data_digest().expect("digest"),
            "envelope keys excluded"
        );
        assert_eq!(
            digest,
            canonical::digest(
                &serde_json::json!({"item":"wi-3fa9c01b2e","option":"a","applies":"once"})
            )
            .expect("digest")
        );
    }
}
