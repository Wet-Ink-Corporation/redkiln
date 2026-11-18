//! The raiser's half of the decision request, with the `core` feature: the DR-04 guards, the item
//! id (DR-03), the event a raiser appends (DR-01) and the condition it appends under.

use super::{Request, RaiserNamespace, Subject, TrustNeeded, ID_DOMAIN, OptionId};
use crate::id::ItemId;
use crate::names::{ITEM_EVENTS, REQUEST_CHANNEL, REQUEST_RAISED};
use crate::vocab::Door;
use redkiln_core::envelope::{
    Class, Envelope, EnvelopeError, FieldError, Namespace, Payload, Timestamp, decode,
};
use redkiln_core::ids::{ContentId, HexLen, IdError, IdPrefix};
use redkiln_core::store::{Condition, CoreKey, Filter, NewEvent, StoreError, Stored, Tag, TagError};
use redkiln_core::subject::{Subject as CoreSubject, SubjectError};
use redkiln_core::trust::Level;

/// Why a request was refused (DR-04). Nothing was written.
#[derive(Debug, thiserror::Error)]
pub enum RequestError {
    /// A text the record requires is empty.
    #[error("`{0}` is empty")]
    Empty(&'static str),
    /// Fewer than two or more than four options; how many there are.
    #[error("a request has 2 to 4 options, not {0}")]
    OptionCount(usize),
    /// An option's id is not the next of `a` to `d`.
    #[error("option {position} has id `{found}`; ids are a to d in order")]
    OptionOrder {
        /// Its 1-based position.
        position: usize,
        /// The id it has.
        found: OptionId,
    },
    /// The recommendation names no option.
    #[error("the recommendation `{0}` is not an option")]
    Recommendation(OptionId),
    /// The default names no option.
    #[error("the default `{0}` is not an option")]
    Default(OptionId),
    /// A default on a door that is not two-way (DR-07, wi-4d8b6a).
    #[error("only a two-way item has a default (door {})", .0.as_str())]
    DefaultDoor(Door),
    /// The deadline is not one of EV-01's timestamp forms.
    #[error("the deadline `{deadline}` is not an RFC 3339 UTC timestamp")]
    Deadline {
        /// The deadline given.
        deadline: String,
        /// Core's reason.
        #[source]
        source: FieldError,
    },
    /// The deadline is not later than the event's `ts`.
    #[error("the deadline {deadline} is not later than the request's ts {ts}")]
    DeadlinePassed {
        /// The deadline.
        deadline: String,
        /// The event's `ts`.
        ts: String,
    },
    /// A CI raiser asked for `witnessed`, which no surface reaching CI gives.
    #[error("a raiser in CI (session {0}) needs `attested`")]
    CiTrust(String),
    /// The subject's hash is not a subject Core reads.
    #[error("the subject hash `{hash}` is not `<method>:<hash>` of a known method")]
    SubjectHash {
        /// The hash given.
        hash: String,
        /// Core's reason.
        #[source]
        source: SubjectError,
    },
    /// The subject's hash was taken by another method than it names.
    #[error("the subject names method `{method}` but its hash is `{hash}`")]
    SubjectMethod {
        /// The method named.
        method: String,
        /// The hash given.
        hash: String,
    },
    /// `attachments` is present but empty; leave it out instead.
    #[error("`attachments` is empty; leave it out when there are none")]
    NoAttachments,
    /// An attachment's hash is not `<method>:<lowercase hex>` (ID-07).
    #[error("the attachment hash `{0}` is not `<method>:<lowercase hex>`")]
    AttachmentHash(String),
    /// The subject's kind is neither one of AP-01's kinds nor `<namespace>.<name>`.
    #[error("the subject kind `{0}` is not an AP-01 kind or `<namespace>.<name>`")]
    SubjectKind(String),
    /// The subject's base is not a 40- or 64-hex commit.
    #[error("the subject base `{0}` is not 40 or 64 lowercase hex")]
    SubjectBase(String),
    /// The subject's locator holds a control character.
    #[error("the subject locator holds a control character")]
    SubjectLocator,
    /// The item id could not be minted.
    #[error("minting the request's item id")]
    Id(#[source] IdError),
    /// A tag Core refuses.
    #[error("tagging the request")]
    Tag(#[source] TagError),
    /// The envelope Core refuses.
    #[error("building the request's envelope")]
    Envelope(#[source] EnvelopeError),
    /// The raise condition's filter Core refuses; its type names are fixed words, so never in
    /// practice.
    #[error("building the raise condition")]
    Condition(#[source] StoreError),
}

impl Payload for Request {
    const TYPE: &'static str = REQUEST_RAISED.name;
    const VERSION: u32 = REQUEST_RAISED.version;
    const CLASS: Class = Class::Durable;
}

impl TrustNeeded {
    /// The Core trust level it names.
    #[must_use]
    pub const fn level(self) -> Level {
        match self {
            Self::Witnessed => Level::Witnessed,
            Self::Attested => Level::Attested,
        }
    }
}

impl From<Namespace> for RaiserNamespace {
    fn from(namespace: Namespace) -> Self {
        Self(namespace.as_str())
    }
}

impl Request {
    /// Checks every DR-04 guard, and the schema's per-field shapes, for a request written at `ts`.
    ///
    /// # Errors
    /// The first guard the record breaks, as a [`RequestError`].
    pub fn check(&self, ts: &Timestamp) -> Result<(), RequestError> {
        self.check_texts()?;
        self.check_options()?;
        if let Some(default) = self.default {
            if !self.has_option(default) {
                return Err(RequestError::Default(default));
            }
            if self.door != Door::TwoWay {
                return Err(RequestError::DefaultDoor(self.door));
            }
        }
        if let Some(deadline) = &self.deadline {
            check_deadline(deadline, ts)?;
        }
        if let Some(session) = &self.raiser.session
            && session.starts_with("gha:")
            && self.trust_needed != TrustNeeded::Attested
        {
            return Err(RequestError::CiTrust(session.clone()));
        }
        if let Some(subject) = &self.subject {
            check_subject(subject)?;
        }
        self.check_attachments()
    }

    fn check_attachments(&self) -> Result<(), RequestError> {
        let Some(attachments) = &self.attachments else {
            return Ok(());
        };
        if attachments.is_empty() {
            return Err(RequestError::NoAttachments);
        }
        attachments
            .iter()
            .filter_map(|a| a.hash.as_deref())
            .find(|hash| !is_method_hash(hash))
            .map_or(Ok(()), |hash| {
                Err(RequestError::AttachmentHash(hash.to_owned()))
            })
    }

    /// The request's item id at ID-04's default length: `wi-` and 10 hex of the domain
    /// [`ID_DOMAIN`] over the raiser's namespace and key (DR-03).
    ///
    /// # Errors
    /// [`RequestError::Id`] if Core refuses the domain, which is a fixed word.
    pub fn item_id(&self) -> Result<ItemId, RequestError> {
        self.item_id_at(HexLen::Ten)
    }

    /// As [`Request::item_id`], at `len`: a raiser that finds the id naming another request
    /// extends it to 12, then 16 (ID-04).
    ///
    /// # Errors
    /// [`RequestError::Id`] if Core refuses the domain, which is a fixed word.
    pub fn item_id_at(&self, len: HexLen) -> Result<ItemId, RequestError> {
        let fields = [self.raiser.namespace.as_str(), self.raiser.key.as_str()];
        let id =
            ContentId::mint(IdPrefix::Item, ID_DOMAIN, &fields, len).map_err(RequestError::Id)?;
        // Core mints the item prefix and 10, 12 or 16 lowercase hex digits, which the parse
        // always accepts. Were the two ever to disagree, the id is refused as Core refuses one.
        ItemId::parse(id.as_str()).ok_or(RequestError::Id(IdError::ContentHex))
    }

    fn has_option(&self, id: OptionId) -> bool {
        self.options.iter().any(|o| o.id == id)
    }

    fn check_texts(&self) -> Result<(), RequestError> {
        let texts = [
            ("question", Some(self.question.as_str())),
            ("raiser.key", Some(self.raiser.key.as_str())),
            ("raiser.run", self.raiser.run.as_deref()),
            ("raiser.session", self.raiser.session.as_deref()),
            (
                "subject.locator",
                self.subject.as_ref().map(|s| s.locator.as_str()),
            ),
        ];
        let options = self.options.iter().flat_map(|o| {
            [
                ("options.label", o.label.as_str()),
                ("options.holds_when", o.holds_when.as_str()),
                ("options.cost_if_wrong", o.cost_if_wrong.as_str()),
            ]
        });
        let attachments = self.attachments.iter().flatten().flat_map(|a| {
            [
                ("attachments.kind", a.kind.as_str()),
                ("attachments.locator", a.locator.as_str()),
            ]
        });
        texts
            .into_iter()
            .filter_map(|(field, text)| text.map(|t| (field, t)))
            .chain(options)
            .chain(attachments)
            .find(|(_, text)| text.trim().is_empty())
            .map_or(Ok(()), |(field, _)| Err(RequestError::Empty(field)))
    }

    fn check_options(&self) -> Result<(), RequestError> {
        let n = self.options.len();
        if !(2..=4).contains(&n) {
            return Err(RequestError::OptionCount(n));
        }
        for (index, (option, expected)) in self.options.iter().zip(OptionId::ALL).enumerate() {
            if option.id != expected {
                return Err(RequestError::OptionOrder {
                    position: index.saturating_add(1),
                    found: option.id,
                });
            }
        }
        if self.has_option(self.recommendation) {
            Ok(())
        } else {
            Err(RequestError::Recommendation(self.recommendation))
        }
    }
}

/// The event raising `envelope`'s request, checked at its `ts` (DR-01).
///
/// It is tagged with its item and the channel `weighin.channel:request`. The caller appends it
/// under [`raise_condition`] (DR-03).
///
/// # Errors
/// A guard the request breaks, or a tag or envelope Core refuses.
pub fn raise_event(envelope: &Envelope<Request>) -> Result<NewEvent, RequestError> {
    raise_event_at(envelope, HexLen::Ten)
}

/// [`raise_event`] with the item id at `len`: a raiser that found its 10-hex id taken by another
/// raise extends it to 12, then 16 (ID-04).
///
/// # Errors
/// A guard the request breaks, or a tag or envelope Core refuses.
pub fn raise_event_at(envelope: &Envelope<Request>, len: HexLen) -> Result<NewEvent, RequestError> {
    envelope.data.check(&envelope.ts)?;
    let id = envelope.data.item_id_at(len)?;
    let tags = [
        Tag::core(CoreKey::Item, id.as_str()),
        Tag::namespaced(Namespace::Weighin, "channel", REQUEST_CHANNEL),
    ]
    .into_iter()
    .collect::<Result<Vec<_>, _>>()
    .map_err(RequestError::Tag)?;
    envelope.to_event(tags).map_err(RequestError::Envelope)
}

/// The condition a raise is appended under: no `weighin.item_raised` and no
/// `weighin.request_raised` tagged `item:<id>` (DR-03). Weigh-In's own raise uses it too, so
/// neither kind of raise lands on an item the other holds.
///
/// # Errors
/// [`RequestError::Tag`] for an id Core's `item` key refuses; [`RequestError::Condition`] if
/// Core refuses a type name, which are fixed words.
pub fn raise_condition(id: &ItemId) -> Result<Condition, RequestError> {
    let tag = Tag::core(CoreKey::Item, id.as_str()).map_err(RequestError::Tag)?;
    let [raised, ..] = ITEM_EVENTS;
    let filter = Filter::all()
        .of_type(raised.name)
        .and_then(|f| f.of_type(REQUEST_RAISED.name))
        .map_err(RequestError::Condition)?;
    Ok(Condition::none_matching(filter.tagged(tag)))
}

/// Whether `stored` is a decision request of `request`'s raiser namespace and key: a retry of
/// the same raise, not a collision with another item.
#[must_use]
pub fn same_raiser(stored: &Stored, request: &Request) -> bool {
    decode::<Request>(stored).is_ok_and(|read| {
        read.data.raiser.namespace == request.raiser.namespace
            && read.data.raiser.key == request.raiser.key
    })
}

fn check_deadline(deadline: &str, ts: &Timestamp) -> Result<(), RequestError> {
    let parsed = Timestamp::parse(deadline).map_err(|source| RequestError::Deadline {
        deadline: deadline.to_owned(),
        source,
    })?;
    if moment(&parsed) > moment(ts) {
        Ok(())
    } else {
        Err(RequestError::DeadlinePassed {
            deadline: deadline.to_owned(),
            ts: ts.as_str().to_owned(),
        })
    }
}

/// A timestamp as a comparable key: its seconds part, then its milliseconds, 0 when it has none.
/// Both of EV-01's forms are fixed-width up to the seconds, so the text compares as time does.
fn moment(ts: &Timestamp) -> (&str, &str) {
    let text = ts.as_str();
    let (seconds, rest) = text.split_at_checked(19).unwrap_or((text, ""));
    let millis = rest
        .strip_prefix('.')
        .and_then(|r| r.get(..3))
        .unwrap_or("000");
    (seconds, millis)
}

/// AP-01's subject kinds that carry no namespace.
const SUBJECT_KINDS: [&str; 9] = [
    "plan",
    "spec",
    "design",
    "brief",
    "change",
    "pull_request",
    "dependency",
    "lint_exception",
    "test_removal",
];

/// A subject kind as AP-01 writes it: one of [`SUBJECT_KINDS`], or `<namespace>.<name>` with an
/// EV-03 namespace and a name of lowercase ASCII, digits and `_` that starts with a letter.
fn is_subject_kind(kind: &str) -> bool {
    if SUBJECT_KINDS.contains(&kind) {
        return true;
    }
    let Some((namespace, name)) = kind.split_once('.') else {
        return false;
    };
    namespace.parse::<Namespace>().is_ok()
        && name.chars().next().is_some_and(|c| c.is_ascii_lowercase())
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

/// `<method>:<hex>` as ID-07 writes a hash: a method of lowercase ASCII, digits and `-`, then
/// lowercase hex, both non-empty.
fn is_method_hash(hash: &str) -> bool {
    hash.split_once(':').is_some_and(|(method, hex)| {
        !method.is_empty()
            && method
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
            && is_lower_hex(hex)
    })
}

fn is_lower_hex(text: &str) -> bool {
    !text.is_empty() && text.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))
}

fn check_subject(subject: &Subject) -> Result<(), RequestError> {
    if !is_subject_kind(&subject.kind) {
        return Err(RequestError::SubjectKind(subject.kind.clone()));
    }
    if let Some(base) = &subject.base
        && !(matches!(base.len(), 40 | 64) && is_lower_hex(base))
    {
        return Err(RequestError::SubjectBase(base.clone()));
    }
    // The schema's `[^\x00-\x1f\x7f-\x9f]`: C0, DEL and C1, which `char::is_control` is.
    if subject.locator.chars().any(char::is_control) {
        return Err(RequestError::SubjectLocator);
    }
    let parsed = CoreSubject::parse(&subject.hash).map_err(|source| RequestError::SubjectHash {
        hash: subject.hash.clone(),
        source,
    })?;
    if parsed.method().as_str() == subject.method {
        Ok(())
    } else {
        Err(RequestError::SubjectMethod {
            method: subject.method.clone(),
            hash: subject.hash.clone(),
        })
    }
}
