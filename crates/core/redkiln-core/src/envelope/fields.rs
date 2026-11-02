//! The envelope's own fields (SPEC EV-01): `ts`, `actor`, `writer`, `branch` and `head`.

use std::fmt;
use std::time::{SystemTime, UNIX_EPOCH};

use time::{Date, Month, OffsetDateTime, Time};

/// The SPEC version this build conforms to, written in every envelope's `writer.spec`.
pub const SPEC_VERSION: &str = "1.0.0";

pub(crate) const TIMESTAMP_FORM: &str =
    "must be RFC 3339 in UTC with a Z, to the second or the millisecond";
pub(crate) const PRINTABLE_WORD: &str = "must be non-empty, with no spaces or control characters";
pub(crate) const PRINTABLE_TEXT: &str = "must be non-empty, with no control characters";
pub(crate) const COMMIT_ID: &str = "must be a full lowercase commit id (40 or 64 hex)";
const ACTOR_FORMS: &str = "must be human:<name>, agent, hook, cli, ci or app:<name>";

/// An envelope key, as an error names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// `type`.
    Type,
    /// `version`.
    Version,
    /// `ts`.
    Ts,
    /// `actor`.
    Actor,
    /// `trust`.
    Trust,
    /// `writer`.
    Writer,
    /// `session`.
    Session,
    /// `worktree`.
    Worktree,
    /// `branch`.
    Branch,
    /// `head`.
    Head,
    /// `origin`.
    Origin,
    /// `cites`.
    Cites,
    /// `data`.
    Data,
}

impl Field {
    /// The key as written.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Type => "type",
            Self::Version => "version",
            Self::Ts => "ts",
            Self::Actor => "actor",
            Self::Trust => "trust",
            Self::Writer => "writer",
            Self::Session => "session",
            Self::Worktree => "worktree",
            Self::Branch => "branch",
            Self::Head => "head",
            Self::Origin => "origin",
            Self::Cites => "cites",
            Self::Data => "data",
        }
    }
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// An envelope field whose value breaks its rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("the envelope's {field} {reason}")]
pub struct FieldError {
    /// The field.
    pub field: Field,
    /// The rule it breaks.
    pub reason: &'static str,
}

const fn refuse(field: Field, reason: &'static str) -> FieldError {
    FieldError { field, reason }
}

/// When the writer wrote an event: RFC 3339 in UTC with a `Z`, to the second
/// (`2026-10-31T04:20:11Z`) or the millisecond (`2026-10-31T04:20:11.123Z`) (EV-01).
/// Advisory across stores (EV-15).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Timestamp(String);

impl Timestamp {
    /// Reads a timestamp in one of EV-01's two forms, refusing a date or time that does not
    /// exist (a 30 February, a leap second).
    ///
    /// # Errors
    /// [`FieldError`] on [`Field::Ts`] for any other text.
    pub fn parse(text: &str) -> Result<Self, FieldError> {
        parse_utc(text).ok_or(refuse(Field::Ts, TIMESTAMP_FORM))?;
        Ok(Self(text.to_owned()))
    }

    /// The moment `at`, to the millisecond: a writer passes the clock's reading
    /// (`SystemTime::now()`), so the library itself never reads a clock.
    ///
    /// # Errors
    /// [`FieldError`] on [`Field::Ts`] outside the years 0000 to 9999.
    pub fn from_system_time(at: SystemTime) -> Result<Self, FieldError> {
        let nanos = match at.duration_since(UNIX_EPOCH) {
            Ok(after) => i128::try_from(after.as_nanos()).ok(),
            Err(before) => i128::try_from(before.duration().as_nanos())
                .ok()
                .map(i128::saturating_neg),
        };
        Self::from_unix_nanos(nanos.ok_or(refuse(Field::Ts, TIMESTAMP_FORM))?)
    }

    /// The moment `millis` milliseconds after the Unix epoch, to the millisecond.
    ///
    /// # Errors
    /// [`FieldError`] on [`Field::Ts`] outside the years 0000 to 9999.
    pub fn from_unix_millis(millis: i64) -> Result<Self, FieldError> {
        Self::from_unix_nanos(i128::from(millis).saturating_mul(1_000_000))
    }

    fn from_unix_nanos(nanos: i128) -> Result<Self, FieldError> {
        match OffsetDateTime::from_unix_timestamp_nanos(nanos) {
            Ok(at) if (0..=9999).contains(&at.year()) => Ok(Self(write_millis(at))),
            _ => Err(refuse(Field::Ts, TIMESTAMP_FORM)),
        }
    }

    /// The timestamp as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Timestamp {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// `YYYY-MM-DDTHH:MM:SSZ` or `YYYY-MM-DDTHH:MM:SS.mmmZ`, as a real moment.
fn parse_utc(text: &str) -> Option<OffsetDateTime> {
    let (stamp, rest) = text.as_bytes().split_first_chunk::<19>()?;
    let millis = match rest {
        [b'Z'] => 0,
        [b'.', a, b, c, b'Z'] => number(&[*a, *b, *c])?,
        _ => return None,
    };
    let [
        y0,
        y1,
        y2,
        y3,
        b'-',
        mo0,
        mo1,
        b'-',
        d0,
        d1,
        b'T',
        h0,
        h1,
        b':',
        mi0,
        mi1,
        b':',
        s0,
        s1,
    ] = *stamp
    else {
        return None;
    };
    let year = i32::try_from(number(&[y0, y1, y2, y3])?).ok()?;
    let month = Month::try_from(u8::try_from(number(&[mo0, mo1])?).ok()?).ok()?;
    let day = u8::try_from(number(&[d0, d1])?).ok()?;
    let hour = u8::try_from(number(&[h0, h1])?).ok()?;
    let minute = u8::try_from(number(&[mi0, mi1])?).ok()?;
    let second = u8::try_from(number(&[s0, s1])?).ok()?;
    let date = Date::from_calendar_date(year, month, day).ok()?;
    let time = Time::from_hms_milli(hour, minute, second, u16::try_from(millis).ok()?).ok()?;
    Some(date.with_time(time).assume_utc())
}

/// The value of ASCII digits, `None` when any byte is not one.
fn number(digits: &[u8]) -> Option<u32> {
    digits.iter().try_fold(0u32, |n, &d| {
        d.is_ascii_digit()
            .then(|| n.checked_mul(10)?.checked_add(u32::from(d - b'0')))
            .flatten()
    })
}

fn write_millis(at: OffsetDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:03}Z",
        at.year(),
        u8::from(at.month()),
        at.day(),
        at.hour(),
        at.minute(),
        at.second(),
        at.millisecond()
    )
}

/// A person's or an App's name in an actor: non-empty, no spaces or control characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ActorName(String);

impl ActorName {
    /// The name `name`.
    ///
    /// # Errors
    /// [`FieldError`] on [`Field::Actor`] for an empty name or one with a space or control
    /// character.
    pub fn new(name: impl Into<String>) -> Result<Self, FieldError> {
        let name = name.into();
        if is_word(&name) {
            Ok(Self(name))
        } else {
            Err(refuse(Field::Actor, ACTOR_FORMS))
        }
    }

    /// The name as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Who the writer says acted (EV-01). Asserted text: [`crate::trust`] says whether to believe it.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Actor {
    /// `human:<name>`.
    Human(ActorName),
    /// `agent`.
    Agent,
    /// `hook`.
    Hook,
    /// `cli`.
    Cli,
    /// `ci`.
    Ci,
    /// `app:<name>`.
    App(ActorName),
}

impl Actor {
    /// Reads an actor in one of its six forms.
    ///
    /// # Errors
    /// [`FieldError`] on [`Field::Actor`] for any other text.
    pub fn parse(text: &str) -> Result<Self, FieldError> {
        match text {
            "agent" => Ok(Self::Agent),
            "hook" => Ok(Self::Hook),
            "cli" => Ok(Self::Cli),
            "ci" => Ok(Self::Ci),
            _ => {
                if let Some(name) = text.strip_prefix("human:") {
                    ActorName::new(name).map(Self::Human)
                } else if let Some(name) = text.strip_prefix("app:") {
                    ActorName::new(name).map(Self::App)
                } else {
                    Err(refuse(Field::Actor, ACTOR_FORMS))
                }
            }
        }
    }
}

impl fmt::Display for Actor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Human(name) => write!(f, "human:{}", name.as_str()),
            Self::Agent => f.write_str("agent"),
            Self::Hook => f.write_str("hook"),
            Self::Cli => f.write_str("cli"),
            Self::Ci => f.write_str("ci"),
            Self::App(name) => write!(f, "app:{}", name.as_str()),
        }
    }
}

/// The binary that wrote an event (EV-01): its name, its version, and the SPEC version it
/// conforms to.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Writer {
    binary: String,
    version: String,
    spec: String,
}

impl Writer {
    /// `binary` at `version`, conforming to this build's [`SPEC_VERSION`].
    ///
    /// # Errors
    /// [`FieldError`] on [`Field::Writer`] for an empty value or one with a space or control
    /// character.
    pub fn new(binary: impl Into<String>, version: impl Into<String>) -> Result<Self, FieldError> {
        Self::from_parts(binary.into(), version.into(), SPEC_VERSION.to_owned())
    }

    /// A writer as read from an envelope, whatever SPEC version it names.
    pub(crate) fn from_parts(
        binary: String,
        version: String,
        spec: String,
    ) -> Result<Self, FieldError> {
        if [&binary, &version, &spec].into_iter().all(|v| is_word(v)) {
            Ok(Self {
                binary,
                version,
                spec,
            })
        } else {
            Err(refuse(Field::Writer, PRINTABLE_WORD))
        }
    }

    /// The binary's name.
    #[must_use]
    pub fn binary(&self) -> &str {
        &self.binary
    }

    /// The binary's version.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// The SPEC version the binary conforms to.
    #[must_use]
    pub fn spec(&self) -> &str {
        &self.spec
    }
}

/// `branch`: the checked-out branch, non-empty with no control characters.
pub(crate) fn check_branch(branch: &str) -> Result<(), FieldError> {
    if branch.is_empty() || branch.chars().any(char::is_control) {
        Err(refuse(Field::Branch, PRINTABLE_TEXT))
    } else {
        Ok(())
    }
}

/// `head`: the commit at `HEAD`, 40 lowercase hex (SHA-1) or 64 (SHA-256).
pub(crate) fn check_head(head: &str) -> Result<(), FieldError> {
    if matches!(head.len(), 40 | 64) && crate::ids::is_lower_hex(head) {
        Ok(())
    } else {
        Err(refuse(Field::Head, COMMIT_ID))
    }
}

/// Non-empty, with no whitespace or control characters.
fn is_word(text: &str) -> bool {
    !text.is_empty() && !text.chars().any(|c| c.is_whitespace() || c.is_control())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_timestamp_is_utc_to_the_second_or_the_millisecond() {
        for text in [
            "2026-10-31T04:20:11Z",
            "2026-10-31T04:20:11.123Z",
            "2024-02-29T23:59:59Z",
        ] {
            assert_eq!(Timestamp::parse(text).expect(text).as_str(), text);
        }
    }

    #[test]
    fn other_timestamp_forms_are_refused() {
        for text in [
            "",
            "2026-10-31T04:20:11",
            "2026-10-31T04:20:11+00:00",
            "2026-10-31t04:20:11z",
            "2026-10-31 04:20:11Z",
            "2026-10-31T04:20:11.1Z",
            "2026-10-31T04:20:11.123456Z",
            "2026-10-31T04:20Z",
            "2026-02-30T04:20:11Z",
            "2025-02-29T04:20:11Z",
            "2026-10-31T24:00:00Z",
            "2026-10-31T23:59:60Z",
            "+2026-10-31T04:20:11Z",
            "2026-1-31T04:20:11Z",
        ] {
            assert_eq!(
                Timestamp::parse(text),
                Err(FieldError {
                    field: Field::Ts,
                    reason: TIMESTAMP_FORM
                }),
                "{text:?}"
            );
        }
    }

    #[test]
    fn a_timestamp_from_unix_milliseconds_is_written_to_the_millisecond() {
        let ts = Timestamp::from_unix_millis(1_793_420_411_123).expect("in range");
        assert_eq!(ts.as_str(), "2026-10-31T04:20:11.123Z");
        assert_eq!(Timestamp::parse(ts.as_str()), Ok(ts));
    }

    #[test]
    fn a_clock_reading_is_written_to_the_millisecond() {
        let at = UNIX_EPOCH + std::time::Duration::from_nanos(1_793_420_411_123_999_999);
        let ts = Timestamp::from_system_time(at).expect("in range");
        assert_eq!(
            ts.as_str(),
            "2026-10-31T04:20:11.123Z",
            "truncated, not rounded"
        );
        let before = UNIX_EPOCH - std::time::Duration::from_millis(1);
        assert_eq!(
            Timestamp::from_system_time(before)
                .expect("in range")
                .as_str(),
            "1969-12-31T23:59:59.999Z"
        );
    }

    #[test]
    fn actors_take_the_six_forms() {
        let human = Actor::Human(ActorName::new("ryan").expect("a name"));
        assert_eq!(human.to_string(), "human:ryan");
        assert_eq!(Actor::parse("human:ryan"), Ok(human));
        for (text, actor) in [
            ("agent", Actor::Agent),
            ("hook", Actor::Hook),
            ("cli", Actor::Cli),
            ("ci", Actor::Ci),
        ] {
            assert_eq!(Actor::parse(text), Ok(actor.clone()));
            assert_eq!(actor.to_string(), text);
        }
        let app = Actor::parse("app:assay").expect("an app");
        assert_eq!(app, Actor::App(ActorName::new("assay").expect("a name")));
    }

    #[test]
    fn other_actors_are_refused() {
        for text in [
            "",
            "human:",
            "human",
            "app:",
            "bot:x",
            "Agent",
            "human:ry an",
            "human:a\tb",
        ] {
            assert!(
                matches!(
                    Actor::parse(text),
                    Err(FieldError {
                        field: Field::Actor,
                        ..
                    })
                ),
                "{text:?}"
            );
        }
    }

    #[test]
    fn a_writer_states_the_spec_this_build_conforms_to() {
        let writer = Writer::new("weighin", "2.0.0").expect("valid");
        assert_eq!(writer.binary(), "weighin");
        assert_eq!(writer.version(), "2.0.0");
        assert_eq!(writer.spec(), SPEC_VERSION);
        let refused = |field| FieldError {
            field,
            reason: PRINTABLE_WORD,
        };
        assert_eq!(Writer::new("", "2.0.0"), Err(refused(Field::Writer)));
        assert_eq!(Writer::new("weighin", "2 0"), Err(refused(Field::Writer)));
    }

    #[test]
    fn a_head_is_a_full_lowercase_commit_id() {
        assert_eq!(
            check_head("bd234009c1e5a7d4f1c3b2a0e9f8d7c6b5a49382"),
            Ok(())
        );
        assert_eq!(check_head(&"a".repeat(64)), Ok(()), "SHA-256 repositories");
        for text in ["bd23400", "BD234009C1E5A7D4F1C3B2A0E9F8D7C6B5A49382", ""] {
            assert_eq!(
                check_head(text),
                Err(FieldError {
                    field: Field::Head,
                    reason: COMMIT_ID
                }),
                "{text:?}"
            );
        }
    }

    #[test]
    fn a_branch_is_non_empty_printable_text() {
        assert_eq!(check_branch("claude/project-thread-kdpm1k"), Ok(()));
        for text in ["", "a\nb"] {
            assert_eq!(
                check_branch(text),
                Err(FieldError {
                    field: Field::Branch,
                    reason: PRINTABLE_TEXT
                }),
                "{text:?}"
            );
        }
    }
}
