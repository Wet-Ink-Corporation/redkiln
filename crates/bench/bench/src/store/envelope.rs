//! The 0.x flat envelope the `bench ledger` CLI reads and writes until step 4 of
//! `docs/plan/move-2-bench.md`: the event (its short `type` and payload, flattened) plus who
//! wrote it, from where, and when. Moved from `bench-ledger` (Move 2 step 3); the store keeps
//! each one as a Core envelope ([`super::wire`]).

use std::time::SystemTime;

use redkiln_core::envelope::{FieldError, Timestamp as CoreTimestamp};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::tag::{LedgerTag, TagError, TagKey};
use crate::store::{EventKind, LedgerEvent};

/// Why an event does not match the ledger schema.
#[derive(Debug, thiserror::Error)]
pub enum SchemaError {
    /// The JSON is not an event: an unknown `type`, a missing or mistyped field, or a `ts`
    /// that is not one of EV-01's two forms.
    #[error("not a ledger event")]
    Json(#[source] serde_json::Error),
    /// The event is not a JSON object.
    #[error("an event must be a JSON object")]
    NotAnObject,
    /// The event carries a key its type does not define.
    #[error("unknown field {field:?} for event type {kind}")]
    UnknownField {
        /// The event's type.
        kind: EventKind,
        /// The key that is not part of it.
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

/// One event in the 0.x flat form: the event, flattened, and its context.
///
/// On the wire: `{"type": "task_done", "task": "T1", "actor": "cli", "session_id": "s1",
/// "worktree": null, "branch": null, "head": null, "ts": "2026-10-11T09:30:00Z"}`.
/// Absent optional fields are written as `null` and read back from either `null` or a
/// missing key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventEnvelope {
    /// The event, flattened into the envelope's JSON object.
    #[serde(flatten)]
    pub event: LedgerEvent,
    /// Who wrote it: `cli`, `hook`, `agent` or `human:<name>`.
    pub actor: String,
    /// The writing agent session's id, if any. Tagged `session:`.
    #[serde(default)]
    pub session_id: Option<String>,
    /// The writer's git worktree id (`wt-` and 12 hex, ID-06), if any: `mode_set` events are
    /// selected by it. Tagged `worktree:`.
    #[serde(default)]
    pub worktree: Option<String>,
    /// The checked-out branch, if any.
    #[serde(default)]
    pub branch: Option<String>,
    /// The commit at `HEAD`, if any.
    #[serde(default)]
    pub head: Option<String>,
    /// When it was written (EV-01).
    pub ts: Timestamp,
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

    /// Reads an event from JSON bytes; see [`EventEnvelope::from_value`].
    ///
    /// # Errors
    /// As [`EventEnvelope::from_value`]; also [`SchemaError::Json`] for bytes that are not JSON.
    pub fn parse(json: &[u8]) -> Result<Self, SchemaError> {
        let value: Value = serde_json::from_slice(json).map_err(SchemaError::Json)?;
        Self::from_value(&value)
    }

    /// Reads an event from a JSON value, strictly: the `type` must be registered, every field
    /// it requires present and well-typed, no key outside the envelope and the type's payload,
    /// `ts` in one of EV-01's forms, and every tag value valid. The store's own checks (the
    /// actor, branch and head) run when it is appended.
    ///
    /// # Errors
    /// [`SchemaError::NotAnObject`], [`SchemaError::Json`] (unknown type, missing or mistyped
    /// field, bad `ts`), [`SchemaError::UnknownField`] or [`SchemaError::Tag`].
    pub fn from_value(value: &Value) -> Result<Self, SchemaError> {
        let Value::Object(input) = value else {
            return Err(SchemaError::NotAnObject);
        };
        let envelope = Self::deserialize(value).map_err(SchemaError::Json)?;
        // serde cannot deny unknown fields through `flatten`, and a reader ignores them (EV-09),
        // so compare against the keys the envelope writes back, nested objects included: every
        // field, `null` included.
        let written = serde_json::to_value(&envelope).map_err(SchemaError::Json)?;
        if let Some(field) = unknown_key(input, &written) {
            return Err(SchemaError::UnknownField {
                kind: envelope.event.kind(),
                field,
            });
        }
        envelope.tags()?;
        Ok(envelope)
    }

    /// The envelope as the 0.x JSON bytes the CLI prints.
    ///
    /// # Errors
    /// Serialising strings, numbers and enums does not fail in practice; the error is returned
    /// rather than unwrapped.
    pub fn to_json(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
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
