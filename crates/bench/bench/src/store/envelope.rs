//! Bench's in-memory event: the 0.x flat envelope (the event, its short `type` and payload
//! flattened, plus who wrote it, from where, and when), moved from `bench-ledger` (Move 2 step
//! 3); the store keeps each one as a Core envelope ([`super::wire`]). Since step 4 (PR 4.3)
//! `bench ledger append` reads Core's EV-01 envelope into it ([`EventEnvelope::from_ev01`]).

use std::time::SystemTime;

use redkiln_core::envelope::{FieldError, Timestamp as CoreTimestamp};
use serde::de::IgnoredAny;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::tag::{LedgerTag, TagError, TagKey};
use crate::store::{EventKind, LedgerEvent};

/// Why an event given to `bench ledger append` is not one it writes.
#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
    /// The JSON is not an envelope `append` takes: a missing or mistyped key, a key EV-01 does
    /// not define, a payload field missing or mistyped, or a `ts` that is not one of EV-01's
    /// two forms.
    #[error("not a ledger event")]
    Json(#[source] serde_json::Error),
    /// The event is not a JSON object.
    #[error("an event must be a JSON object")]
    NotAnObject,
    /// The envelope sets a key Bench writes itself: `trust` and `writer` (every Bench event is
    /// `asserted`, written by this `bench`), `origin` (the store's identity, ID-02) or `cites`.
    #[error("{field} is written by bench, not given: leave it out")]
    SetByBench {
        /// The key.
        field: &'static str,
    },
    /// The `type` is not one of Bench's 17.
    #[error("unknown event type {name:?}: one of bench.test_run, bench.task_done, ... (EV-03)")]
    UnknownType {
        /// The type given.
        name: String,
    },
    /// The `type` is a 0.x short name: the envelope names the stored type.
    #[error("event type {name:?} is the 0.x short name: write {}", kind.stored())]
    ShortType {
        /// The type given.
        name: String,
        /// The type it names.
        kind: EventKind,
    },
    /// The `version` is not the one Bench writes (EV-06).
    #[error("{} is written at version {WRITTEN_VERSION}, not {version}", kind.stored())]
    Version {
        /// The event's type.
        kind: EventKind,
        /// The version given.
        version: u64,
    },
    /// The payload carries a key its type does not define, as a dotted path from `data`.
    #[error("unknown field {field:?} for event type {}", kind.stored())]
    UnknownField {
        /// The event's type.
        kind: EventKind,
        /// The key that is not part of it, `data.` first.
        field: String,
    },
    /// A tag value (session, worktree or subject) is not a valid tag.
    #[error(transparent)]
    Tag(#[from] TagError),
}

/// Why a timestamp could not be read or made.
#[derive(Debug, thiserror::Error)]
pub enum TimestampError {
    /// The text is not in one of EV-01's two forms (`…:SSZ` or `…:SS.mmmZ`, UTC).
    #[error(
        "ts {value:?} is not an RFC 3339 UTC timestamp to the second or the millisecond (EV-01)"
    )]
    Parse {
        /// The text that was given.
        value: String,
        /// What Core refused.
        #[source]
        source: FieldError,
    },
    /// The clock's time cannot be written as a timestamp (outside the years 0000 to 9999).
    #[error("the system clock cannot be written as a timestamp")]
    Clock(#[source] FieldError),
}

/// An EV-01 timestamp, kept as written: UTC with a `Z`, to the second or the millisecond.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Timestamp(CoreTimestamp);

impl Timestamp {
    /// The current UTC time to the second, e.g. `2026-10-11T09:30:00Z`.
    ///
    /// # Errors
    /// [`TimestampError::Clock`] when the clock reads outside the years 0000 to 9999.
    pub fn now() -> Result<Self, TimestampError> {
        let millis = Self::now_millis()?.0.unix_millis();
        let second = millis.div_euclid(1000).saturating_mul(1000);
        let at = CoreTimestamp::from_unix_millis(second).map_err(TimestampError::Clock)?;
        // Core writes the millisecond form; the second form drops its `.000`.
        match at.as_str().split_at_checked(19) {
            Some((whole, _)) => CoreTimestamp::parse(&format!("{whole}Z"))
                .map(Self)
                .map_err(TimestampError::Clock),
            None => Ok(Self(at)),
        }
    }

    /// The current UTC time to the millisecond, always three digits, e.g.
    /// `2026-10-11T09:30:00.250Z`. For an event whose time is compared with file times, which
    /// a whole second would blur. Like [`Timestamp::now`], it reads the wall clock here
    /// because `Timestamp` is the ledger's clock adapter (Core reads no clock).
    ///
    /// # Errors
    /// [`TimestampError::Clock`] when the clock reads outside the years 0000 to 9999.
    pub fn now_millis() -> Result<Self, TimestampError> {
        CoreTimestamp::from_system_time(SystemTime::now())
            .map(Self)
            .map_err(TimestampError::Clock)
    }

    /// The timestamp as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }

    /// Milliseconds since the Unix epoch (negative before it).
    #[must_use]
    pub fn unix_millis(&self) -> i64 {
        self.0.unix_millis()
    }

    /// Core's timestamp, for the envelope the store keeps; copied, since the caller keeps its
    /// envelope.
    pub(super) fn to_core(&self) -> CoreTimestamp {
        self.0.clone()
    }

    /// The timestamp of a stored envelope, which Core read.
    pub(super) const fn from_core(ts: CoreTimestamp) -> Self {
        Self(ts)
    }
}

impl TryFrom<String> for Timestamp {
    type Error = TimestampError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match CoreTimestamp::parse(&value) {
            Ok(ts) => Ok(Self(ts)),
            Err(source) => Err(TimestampError::Parse { value, source }),
        }
    }
}

impl From<Timestamp> for String {
    fn from(ts: Timestamp) -> Self {
        ts.as_str().to_owned()
    }
}

/// The version every Bench type is written at (EV-06): bench-contract declares each of the 17
/// at version 1 (its `event::core` table).
pub const WRITTEN_VERSION: u32 = 1;

/// The keys of an EV-01 envelope that `bench ledger append` does not take, because Bench
/// writes them.
const SET_BY_BENCH: [&str; 4] = ["trust", "writer", "origin", "cites"];

/// One event in the 0.x flat form: the event, flattened, and its context.
///
/// Bench's in-memory form of an event, which the store converts to and from Core's EV-01
/// envelope (the store's `wire` module), and which the commands that record an event (`grill confirm`,
/// `plan approve`, `mode`, ...) still print. On the wire: `{"type": "task_done", "task": "T1",
/// "actor": "cli", "session_id": "s1", "worktree": null, "branch": null, "head": null, "ts":
/// "2026-10-11T09:30:00Z"}`. `bench ledger` reads and prints Core's envelope instead
/// ([`EventEnvelope::from_ev01`], [`super::StoredEvent`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EventEnvelope {
    /// The event, flattened into the envelope's JSON object.
    #[serde(flatten)]
    pub event: LedgerEvent,
    /// Who wrote it: `cli`, `hook`, `agent` or `human:<name>`.
    pub actor: String,
    /// The writing agent session's id, if any. Tagged `session:`.
    pub session_id: Option<String>,
    /// The writer's git worktree id (`wt-` and 12 hex, ID-06), if any: `mode_set` events are
    /// selected by it. Tagged `worktree:`.
    pub worktree: Option<String>,
    /// The checked-out branch, if any.
    pub branch: Option<String>,
    /// The commit at `HEAD`, if any.
    pub head: Option<String>,
    /// When it was written (EV-01).
    pub ts: Timestamp,
}

/// The keys of an EV-01 envelope `bench ledger append` takes; `null` reads as absent.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Written {
    #[serde(rename = "type")]
    event_type: String,
    #[serde(default)]
    version: Option<u64>,
    ts: Timestamp,
    actor: String,
    #[serde(default)]
    session: Option<String>,
    #[serde(default)]
    worktree: Option<String>,
    #[serde(default)]
    branch: Option<String>,
    #[serde(default)]
    head: Option<String>,
    data: serde_json::Map<String, Value>,
    // Bench's keys, here only as `null` (any other value is refused first, SET_BY_BENCH).
    #[serde(default, rename = "trust")]
    _trust: Option<IgnoredAny>,
    #[serde(default, rename = "writer")]
    _writer: Option<IgnoredAny>,
    #[serde(default, rename = "origin")]
    _origin: Option<IgnoredAny>,
    #[serde(default, rename = "cites")]
    _cites: Option<IgnoredAny>,
}

impl EventEnvelope {
    /// An envelope for `event` with no session, worktree, branch or head.
    #[must_use]
    pub fn new(event: LedgerEvent, actor: impl Into<String>, ts: Timestamp) -> Self {
        Self {
            event,
            actor: actor.into(),
            session_id: None,
            worktree: None,
            branch: None,
            head: None,
            ts,
        }
    }

    /// Reads an event from the EV-01 envelope `bench ledger append` takes, strictly: `type` one
    /// of Bench's 17 stored names (`bench.task_done`), `version` absent or
    /// [`WRITTEN_VERSION`], `ts` in one of EV-01's forms, `actor`, the optional `session`,
    /// `worktree`, `branch` and `head` (`null` is absent), and `data` holding exactly the
    /// type's payload; every tag value valid. `trust`, `writer`, `origin` and `cites` are
    /// Bench's to write, and any other key is refused. The store's own checks (the actor,
    /// session and worktree ids, branch and head) run when it is appended.
    ///
    /// # Errors
    /// [`SchemaError::NotAnObject`], [`SchemaError::SetByBench`], [`SchemaError::Json`] (a
    /// missing, mistyped or unknown key, a payload that does not read, a bad `ts`),
    /// [`SchemaError::UnknownType`], [`SchemaError::ShortType`], [`SchemaError::Version`],
    /// [`SchemaError::UnknownField`] or [`SchemaError::Tag`].
    pub fn from_ev01(value: &Value) -> Result<Self, SchemaError> {
        let Value::Object(input) = value else {
            return Err(SchemaError::NotAnObject);
        };
        if let Some(field) = SET_BY_BENCH
            .into_iter()
            .find(|key| input.get(*key).is_some_and(|value| !value.is_null()))
        {
            return Err(SchemaError::SetByBench { field });
        }
        let written = Written::deserialize(value).map_err(SchemaError::Json)?;
        let kind = kind_of(&written.event_type)?;
        if let Some(version) = written
            .version
            .filter(|version| *version != u64::from(WRITTEN_VERSION))
        {
            return Err(SchemaError::Version { kind, version });
        }
        let event = payload(kind, written.data)?;
        let envelope = Self {
            event,
            actor: written.actor,
            session_id: written.session,
            worktree: written.worktree,
            branch: written.branch,
            head: written.head,
            ts: written.ts,
        };
        envelope.tags()?;
        Ok(envelope)
    }

    /// The tags the store indexes this event by: `session:` and `worktree:` when set (Core
    /// sets them from the envelope), and the subject tag of the event's type when the event
    /// names a subject.
    ///
    /// # Errors
    /// [`TagError::Value`] when a value is not a valid tag value.
    pub fn tags(&self) -> Result<Vec<LedgerTag>, TagError> {
        let context = [
            (TagKey::Session, self.session_id.as_deref()),
            (TagKey::Worktree, self.worktree.as_deref()),
        ];
        let subject = self.event.subject_value();
        context
            .into_iter()
            .chain([(
                TagKey::from(self.event.kind().subject()),
                subject.as_deref(),
            )])
            .filter_map(|(key, value)| value.map(|value| LedgerTag::new(key, value)))
            .collect()
    }
}

/// The kind whose stored name is `name`; a short name is refused with the stored one.
fn kind_of(name: &str) -> Result<EventKind, SchemaError> {
    if let Some(kind) = super::kind_of_stored(name) {
        return Ok(kind);
    }
    match name.parse::<EventKind>() {
        Ok(kind) => Err(SchemaError::ShortType {
            name: name.to_owned(),
            kind,
        }),
        Err(_unknown) => Err(SchemaError::UnknownType {
            name: name.to_owned(),
        }),
    }
}

/// The payload of `kind` in `data`, strictly: every key it defines, nested ones included, and
/// none other.
fn payload(
    kind: EventKind,
    mut data: serde_json::Map<String, Value>,
) -> Result<LedgerEvent, SchemaError> {
    // `type` names the payload in the 0.x form the event deserialises from; a payload has no
    // field of that name.
    if data.contains_key("type") {
        return Err(SchemaError::UnknownField {
            kind,
            field: "data.type".to_owned(),
        });
    }
    data.insert("type".to_owned(), Value::from(kind.as_str()));
    let data = Value::Object(data);
    let event = LedgerEvent::deserialize(&data).map_err(SchemaError::Json)?;
    // serde cannot deny unknown fields through an internally tagged enum, and a reader ignores
    // them (EV-09), so compare against the keys the event writes back, nested objects
    // included: every field, `null` included.
    let written = serde_json::to_value(&event).map_err(SchemaError::Json)?;
    if let Some(field) = data
        .as_object()
        .and_then(|given| unknown_key(given, &written))
    {
        return Err(SchemaError::UnknownField {
            kind,
            field: format!("data.{field}"),
        });
    }
    Ok(event)
}

/// The first key of `input`, as a dotted path, that `written` lacks, looking into the objects
/// both hold under the same key.
fn unknown_key(input: &serde_json::Map<String, Value>, written: &Value) -> Option<String> {
    input
        .iter()
        .find_map(|(key, value)| match written.get(key.as_str()) {
            None => Some(key.clone()),
            Some(inner) => match value {
                Value::Object(nested) => {
                    unknown_key(nested, inner).map(|deeper| format!("{key}.{deeper}"))
                }
                _ => None,
            },
        })
}

#[cfg(test)]
mod tests;
