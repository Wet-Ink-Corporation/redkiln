//! One journal line (SPEC JR-01 to JR-03): an EV-01 envelope with `origin` set, as UTF-8 with
//! no byte order mark, ended by `\n`.
//!
//! A line is the stored envelope's own bytes with `origin` inserted (before `cites`, or `data`,
//! where EV-01 puts it), and the caller's tags as `tags` before `data` when there are any, so
//! the payload keeps its key order and number forms and a line copied between journals stays
//! the same fact. An import does the reverse: `origin` and `tags` leave the bytes (`origin` is a
//! store-minted value, EV-02, kept in happenstance's identity columns; the tags are stored as
//! tags), and `trust` is rewritten to `asserted` with the line's level in `claimed` (JR-12,
//! TR-06).

use happenstance_core::EventId;

use super::members::{Member, members};
use super::tags;
use crate::envelope::{MAX_ENVELOPE_BYTES, Unusable, namespace_of, read_untyped, trust_json};
use crate::ids::SessionId;
use crate::store::{CoreKey, StoreError, Stored, Tag, TagError};

/// The longest line read: an envelope at its limit (EV-16), and the `origin` and `tags` a line
/// adds (at most 64 tags of 255 bytes, each at most doubled by JSON's escapes).
pub const MAX_LINE_BYTES: usize = 2 * MAX_ENVELOPE_BYTES;

/// Why a stored event has no line, or a line is not one an import can store.
#[derive(Debug, thiserror::Error)]
pub enum LineError {
    /// Not an envelope (EV-01), or one that breaks a rule of a typed read.
    #[error("not an envelope")]
    Envelope(#[source] Unusable),
    /// The envelope's `type` is not the stored event's.
    #[error("the envelope's type is not the stored event's")]
    TypeMismatch,
    /// A journal line with no `origin`: it names no identity to import under (JR-11).
    #[error("the line has no origin")]
    NoOrigin,
    /// The line is longer than [`MAX_LINE_BYTES`].
    #[error("the line is {len} bytes, more than {MAX_LINE_BYTES}")]
    TooLong {
        /// Its length.
        len: usize,
    },
    /// Rewritten for the store, the envelope is larger than [`MAX_ENVELOPE_BYTES`].
    #[error("the envelope would be {len} bytes, more than {MAX_ENVELOPE_BYTES}")]
    TooLarge {
        /// Its length.
        len: usize,
    },
    /// A tag Core sets from the envelope is refused.
    #[error("a tag of the envelope is refused")]
    Tag(#[source] TagError),
    /// A tag in the line's `tags` is refused: not `key:value`, a key Core sets, or a registered
    /// key with a value not its kind.
    #[error("a tag the line carries is refused")]
    CallerTag(#[source] TagError),
    /// The line carries more tags than an event holds.
    #[error(
        "the line carries {count} tags, more than {}",
        super::tags::MAX_LINE_TAGS
    )]
    TooManyTags {
        /// How many it carries.
        count: usize,
    },
    /// The store refuses the envelope's type as an event type.
    #[error("the store refuses the event type")]
    Refused(#[source] StoreError),
    /// The line claims the importing store's own identity at a position it does not hold
    /// (happenstance's `UnheldOwnIdentity`), so it cannot be the store's own fact.
    #[error("the line claims a position of this store that it does not hold")]
    UnheldOwnIdentity,
    /// The line's `origin` names a position past 2^63 - 1, the most the store holds; happenstance
    /// would refuse the whole batch for it.
    #[error("the origin's position {position} is past 2^63 - 1, the most the store holds")]
    PositionOutOfRange {
        /// The position the origin names.
        position: u64,
    },
    /// The plugin that read an old source could not map this record (ST-15).
    #[error("the plugin could not map the old record")]
    Unmapped,
}

/// The largest position the store holds: happenstance keeps positions as SQLite integers.
const MAX_POSITION: u64 = 0x7fff_ffff_ffff_ffff;

/// One line of a journal file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Line<'a> {
    /// Its number, from 1.
    pub number: u64,
    /// Where it starts in the bytes read.
    pub start: usize,
    /// Its bytes, without the `\n` and one `\r` before it.
    pub text: &'a [u8],
    /// Whether a `\n` ended it; only a file's last line can lack one.
    pub complete: bool,
}

impl Line<'_> {
    /// Whether the line is blank (ASCII whitespace only), which a reader skips (JR-03).
    #[must_use]
    pub fn is_blank(&self) -> bool {
        self.text.iter().all(u8::is_ascii_whitespace)
    }
}

/// The lines of a journal's `bytes`, in order, blank ones included.
pub fn lines(bytes: &[u8]) -> impl Iterator<Item = Line<'_>> {
    (1_u64..)
        .zip(bytes.split_inclusive(|&b| b == b'\n'))
        .scan(0_usize, |next, (number, piece)| {
            let start = *next;
            *next = start.saturating_add(piece.len());
            Some(match piece.strip_suffix(b"\n") {
                Some(text) => Line {
                    number,
                    start,
                    text: text.strip_suffix(b"\r").unwrap_or(text),
                    complete: true,
                },
                None => Line {
                    number,
                    start,
                    text: piece,
                    complete: false,
                },
            })
        })
}

/// `stored`'s journal line (JR-02): its envelope with `origin` set to its identity, and `\n`.
///
/// # Errors
/// [`LineError::Envelope`] when the stored bytes are not an envelope (an event written through
/// [`crate::store::NewEvent`] directly), and [`LineError::TypeMismatch`] when its `type` is not
/// the stored type: such an event is not exported.
pub fn encode_line(stored: &Stored) -> Result<Vec<u8>, LineError> {
    encode(stored).map(|(_, line)| line)
}

/// [`encode_line`], with the envelope's session, which names its journal (JR-06).
pub(crate) fn encode(stored: &Stored) -> Result<(Option<SessionId>, Vec<u8>), LineError> {
    encode_parts(stored.id, &stored.event_type, &stored.data, &stored.tags)
}

/// [`encode`] of an event given by its parts: identity `id`, type, envelope bytes and tags.
pub(crate) fn encode_parts(
    id: EventId,
    event_type: &str,
    data: &[u8],
    tags: &[Tag],
) -> Result<(Option<SessionId>, Vec<u8>), LineError> {
    let (name, _, envelope) =
        read_untyped(data, MAX_ENVELOPE_BYTES).map_err(LineError::Envelope)?;
    if name != event_type {
        return Err(LineError::TypeMismatch);
    }
    let text = text_of(data)?;
    let origin = format!("\"origin\":\"{id}\"");
    let tags = tags::member(tags)?;
    // `origin` goes before `cites`, or else `data`, where EV-01 puts it; `tags` before `data`.
    // `read_untyped` refused an envelope with no `data` object, so both are always written.
    let mut origin_written = false;
    let mut line = rebuilt(text, &members_of(text)?, |key| match key {
        "origin" | "tags" => Edit::Drop,
        "cites" => {
            origin_written = true;
            Edit::Before([Some(origin.as_bytes()), None])
        }
        "data" => {
            let origin = (!origin_written).then_some(origin.as_bytes());
            origin_written = true;
            Edit::Before([origin, tags.as_deref()])
        }
        _ => Edit::Keep,
    })?;
    // Valid JSON holds no raw line break inside a string, so any is whitespace between tokens.
    line.retain(|b| !matches!(b, b'\n' | b'\r'));
    line.push(b'\n');
    Ok((envelope.session, line))
}

/// What a line already in a journal holds, as a checkpoint reads it (JR-03, JR-08).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Held {
    /// An envelope with this `origin`.
    Origin(EventId),
    /// An envelope naming no origin: kept, holding no event.
    NoOrigin,
    /// Not an envelope: as a last line with no `\n`, the mark of a writer that stopped mid-line.
    Torn,
}

/// What the line `text` holds: whether it reads as an envelope does not depend on this build
/// knowing its namespace (JR-10), so a later SPEC minor's line is never taken for a torn one.
pub(crate) fn held(text: &[u8]) -> Held {
    match read_untyped(text, MAX_LINE_BYTES) {
        Ok((_, _, envelope)) => envelope.origin.map_or(Held::NoOrigin, Held::Origin),
        Err(_) => Held::Torn,
    }
}

/// A journal line read for an import (JR-10 to JR-12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReadLine {
    /// The identity it is stored under.
    pub(crate) origin: EventId,
    /// Its type, in EV-03's grammar; its namespace may be one this build does not know.
    pub(crate) event_type: String,
    /// The bytes stored: the line without `origin`, `trust` imported.
    pub(crate) data: Vec<u8>,
    /// Core's tags from the envelope (`plugin`, `trust`, `session`, `worktree`), then the
    /// caller's tags the line carries.
    pub(crate) tags: Vec<Tag>,
    /// The line's `ts`, as the recorded time an import keeps.
    pub(crate) recorded_at_millis: i64,
}

/// A line's `text` as an import stores it.
///
/// # Errors
/// [`LineError`] naming why the line is malformed.
pub(crate) fn read_line(text: &[u8]) -> Result<ReadLine, LineError> {
    if text.len() > MAX_LINE_BYTES {
        return Err(LineError::TooLong { len: text.len() });
    }
    let (name, _, envelope) = read_untyped(text, MAX_LINE_BYTES).map_err(LineError::Envelope)?;
    let origin = envelope.origin.ok_or(LineError::NoOrigin)?;
    let position = origin.position().get();
    if position > MAX_POSITION {
        return Err(LineError::PositionOutOfRange { position });
    }
    let trust = envelope.trust.imported();
    let text = text_of(text)?;
    let new_trust = trust_json(&trust).map_err(|e| LineError::Envelope(Unusable::Json(e)))?;
    let mut trust_member = b"\"trust\":".to_vec();
    trust_member.extend_from_slice(&new_trust);
    let callers = tags::read(text.as_bytes())?;
    let data = rebuilt(text, &members_of(text)?, |key| match key {
        // Both live in the store's own columns, never in the bytes.
        "origin" | "tags" => Edit::Drop,
        "trust" => Edit::Replace(&trust_member),
        _ => Edit::Keep,
    })?;
    if data.len() > MAX_ENVELOPE_BYTES {
        return Err(LineError::TooLarge { len: data.len() });
    }
    let core = |key, value: &str| Tag::core(key, value).map_err(LineError::Tag);
    // `read_untyped` checked the type's grammar, so it has a namespace. Not one this build may
    // know, so the tag is built as it is, not checked against the registry (JR-10).
    let namespace = namespace_of(&name).ok_or_else(not_an_object)?;
    let mut tags = vec![
        Tag::new(CoreKey::Plugin.as_str(), namespace).map_err(LineError::Tag)?,
        core(CoreKey::Trust, trust.level().as_str())?,
    ];
    if let Some(session) = &envelope.session {
        tags.push(core(CoreKey::Session, session.as_str())?);
    }
    if let Some(worktree) = &envelope.worktree {
        tags.push(core(CoreKey::Worktree, worktree.as_str())?);
    }
    tags.extend(callers);
    Ok(ReadLine {
        origin,
        event_type: name,
        data,
        tags,
        recorded_at_millis: envelope.ts.unix_millis(),
    })
}

/// `bytes` as text; an envelope has been read from them, so they are UTF-8.
fn text_of(bytes: &[u8]) -> Result<&str, LineError> {
    std::str::from_utf8(bytes)
        .map_err(|e| LineError::Envelope(Unusable::Json(serde::de::Error::custom(e))))
}

fn members_of(text: &str) -> Result<Vec<Member>, LineError> {
    members(text).ok_or_else(not_an_object)
}

fn not_an_object() -> LineError {
    LineError::Envelope(Unusable::Json(serde::de::Error::custom(
        "an envelope is a JSON object",
    )))
}

/// What a rebuild does with one member, by its key.
enum Edit<'m> {
    /// Keeps it as it is.
    Keep,
    /// Leaves it out.
    Drop,
    /// Writes this member in its place.
    Replace(&'m [u8]),
    /// Writes these members, those given, before it.
    Before([Option<&'m [u8]>; 2]),
}

/// The object `text` rebuilt from its `members`, compact between them, each member's own text
/// kept unless `edit` says otherwise.
fn rebuilt<'m>(
    text: &str,
    members: &[Member],
    mut edit: impl FnMut(&str) -> Edit<'m>,
) -> Result<Vec<u8>, LineError> {
    let mut out = Vec::with_capacity(text.len().saturating_add(64));
    out.push(b'{');
    let push = |out: &mut Vec<u8>, piece: &[u8]| {
        if out.len() > 1 {
            out.push(b',');
        }
        out.extend_from_slice(piece);
    };
    for member in members {
        // A `Range` is not `Copy`; the span is rebuilt for `get`.
        let own = text
            .get(member.span.start..member.span.end)
            .ok_or_else(not_an_object)?
            .as_bytes();
        match edit(&member.key) {
            Edit::Keep => push(&mut out, own),
            Edit::Drop => {}
            Edit::Replace(new) => push(&mut out, new),
            Edit::Before(new) => {
                for piece in new.into_iter().flatten() {
                    push(&mut out, piece);
                }
                push(&mut out, own);
            }
        }
    }
    out.push(b'}');
    Ok(out)
}

#[cfg(test)]
mod tests {
    use happenstance_core::SequencePosition;

    use super::*;
    use crate::envelope::testing::SPEC_EXAMPLE;
    use crate::ids::{parse_event_ref, parse_store_id};

    /// JR-02's example line.
    const JR_02: &str = r#"{"type":"weighin.item_raised","version":1,"ts":"2026-10-31T04:12:40Z","actor":"agent","trust":{"level":"asserted"},"writer":{"binary":"weighin","version":"2.0.0","spec":"1.0.0"},"session":"8b6fb94b-0975-5275-b434-6c9e60544265","worktree":"wt-5d41402abc4b","origin":"0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:40","data":{"item":"wi-3fa9c01b2e","question":"Which journal location?"}}"#;

    fn stored(id: &str, event_type: &str, data: &str) -> Stored {
        Stored {
            position: 7,
            id: parse_event_ref(id).expect("an event ref"),
            recorded_at_millis: 0,
            event_type: event_type.to_owned(),
            data: data.as_bytes().to_vec(),
            tags: Vec::new(),
        }
    }

    fn without_origin(line: &str) -> String {
        line.replace(r#""origin":"0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:40","#, "")
    }

    #[test]
    fn a_stored_envelope_is_written_as_jr_02_with_its_identity_as_origin() {
        let event = stored(
            "0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:40",
            "weighin.item_raised",
            &without_origin(JR_02),
        );
        assert_eq!(
            encode_line(&event).expect("a line"),
            format!("{JR_02}\n").into_bytes()
        );
    }

    #[test]
    fn origin_goes_before_cites_and_replaces_one_already_there() {
        let event = stored(
            "0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:41",
            "weighin.item_decided",
            SPEC_EXAMPLE,
        );
        let line = String::from_utf8(encode_line(&event).expect("a line")).expect("utf-8");
        let expected = SPEC_EXAMPLE.replace(
            r#""cites""#,
            r#""origin":"0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:41","cites""#,
        );
        assert_eq!(line, format!("{expected}\n"));
        let moved = stored(
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:3",
            "weighin.item_raised",
            JR_02,
        );
        let line = String::from_utf8(encode_line(&moved).expect("a line")).expect("utf-8");
        assert_eq!(line.matches("\"origin\"").count(), 1, "{line}");
        assert!(
            line.contains(r#""origin":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa:3""#),
            "{line}"
        );
    }

    #[test]
    fn a_line_is_one_line_even_when_the_stored_json_is_not_compact() {
        let pretty = "{\"type\":\"weighin.item_raised\",\"version\":1,\"ts\":\"2026-10-31T04:12:40Z\",\r\n\"actor\":\"agent\",\"trust\":{\"level\":\"asserted\"},\"writer\":{\"binary\":\"weighin\",\"version\":\"2.0.0\",\"spec\":\"1.0.0\"},\"data\":{\n  \"text\": \"a\\nb\"\n}}";
        let line = encode_line(&stored(
            "0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:1",
            "weighin.item_raised",
            pretty,
        ))
        .expect("a line");
        let (last, body) = line.split_last().expect("not empty");
        assert_eq!(*last, b'\n');
        assert!(
            !body.contains(&b'\n') && !body.contains(&b'\r'),
            "{}",
            String::from_utf8_lossy(&line)
        );
        let value: serde_json::Value = serde_json::from_slice(body).expect("json");
        assert_eq!(value["data"]["text"], "a\nb");
    }

    #[test]
    fn what_is_not_an_envelope_is_not_written() {
        for data in [
            r#"{"q":"?"}"#,
            "not json",
            r#"{"type":"weighin.item_raised","version":1}"#,
        ] {
            let event = stored(
                "0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:1",
                "weighin.item_raised",
                data,
            );
            assert!(
                matches!(encode_line(&event), Err(LineError::Envelope(_))),
                "{data}"
            );
        }
        let other = stored(
            "0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:1",
            "weighin.item_decided",
            JR_02,
        );
        assert!(matches!(encode_line(&other), Err(LineError::TypeMismatch)));
    }

    #[test]
    fn lines_tolerate_one_carriage_return_and_mark_a_torn_tail() {
        let bytes = b"one\r\n\n  \r\ntwo\r\r\nthree";
        let read: Vec<(u64, &[u8], bool, bool)> = lines(bytes)
            .map(|l| (l.number, l.text, l.complete, l.is_blank()))
            .collect();
        assert_eq!(
            read,
            [
                (1, &b"one"[..], true, false),
                (2, &b""[..], true, true),
                (3, &b"  "[..], true, true),
                (4, &b"two\r"[..], true, false),
                (5, &b"three"[..], false, false),
            ]
        );
        let starts: Vec<usize> = lines(bytes).map(|l| l.start).collect();
        assert_eq!(
            starts,
            [0, 5, 6, 10, 16],
            "where each line starts, its `\\r` counted"
        );
        assert_eq!(lines(b"").count(), 0);
        assert_eq!(lines(b"x\n").count(), 1);
    }

    #[test]
    fn an_imported_line_is_stored_asserted_with_its_claim_and_without_its_origin() {
        let witnessed = SPEC_EXAMPLE.replace(
            r#""cites""#,
            r#""origin":"0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:41","cites""#,
        );
        let read = read_line(witnessed.as_bytes()).expect("well formed");
        let store = parse_store_id("0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f").expect("valid");
        assert_eq!(
            read.origin,
            EventId::new(store, SequencePosition::new(41).expect("non-zero"))
        );
        let expected = SPEC_EXAMPLE.replace(
            r#""trust":{"level":"witnessed","via":"ask-user-question","ref":"toolu_01XyZ"}"#,
            r#""trust":{"level":"asserted","via":"ask-user-question","ref":"toolu_01XyZ","claimed":"witnessed"}"#,
        );
        assert_eq!(std::str::from_utf8(&read.data).expect("utf-8"), expected);
        assert_eq!(read.event_type.as_str(), "weighin.item_decided");
        let tags: Vec<String> = read.tags.iter().map(|t| t.as_str().to_owned()).collect();
        assert_eq!(
            tags,
            [
                "plugin:weighin",
                "trust:asserted",
                "session:8b6fb94b-0975-5275-b434-6c9e60544265",
                "worktree:wt-5d41402abc4b"
            ]
        );
        // `date -u -d 2026-10-31T04:20:11Z +%s` is 1793420411.
        assert_eq!(read.recorded_at_millis, 1_793_420_411_000, "the line's ts");
    }

    /// JR-02's line under a namespace a later SPEC minor may register.
    fn newer_namespace() -> String {
        JR_02.replace("weighin.item_raised", "assay.run_opened")
    }

    #[test]
    fn a_line_of_a_namespace_this_build_does_not_know_is_imported_under_its_plugin() {
        let read = read_line(newer_namespace().as_bytes()).expect("well formed (JR-10)");
        assert_eq!(read.event_type.as_str(), "assay.run_opened");
        let tags: Vec<&str> = read.tags.iter().map(Tag::as_str).collect();
        assert!(tags.contains(&"plugin:assay"), "{tags:?}");
        for outside in ["Assay.run_opened", "assay", "assay.Run", "9assay.run"] {
            let line = JR_02.replace("weighin.item_raised", outside);
            assert!(
                matches!(read_line(line.as_bytes()), Err(LineError::Envelope(_))),
                "{outside:?} is outside EV-03's grammar"
            );
        }
    }

    #[test]
    fn a_line_of_a_namespace_this_build_does_not_know_is_held_not_torn() {
        let origin = parse_event_ref("0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:40").expect("a ref");
        assert_eq!(held(newer_namespace().as_bytes()), Held::Origin(origin));
        let stored = stored(
            "0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:40",
            "assay.run_opened",
            &without_origin(&newer_namespace()),
        );
        assert_eq!(
            encode_line(&stored).expect("exported again (JR-08)"),
            format!("{}\n", newer_namespace()).into_bytes()
        );
    }

    #[test]
    fn an_import_line_needs_an_origin_and_an_envelope() {
        assert!(matches!(
            read_line(SPEC_EXAMPLE.as_bytes()),
            Err(LineError::NoOrigin)
        ));
        assert!(matches!(
            read_line(b"{\"type\":1}"),
            Err(LineError::Envelope(_))
        ));
        assert!(matches!(read_line(b"\xff"), Err(LineError::Envelope(_))));
        let twice = JR_02.replace(r#""actor":"agent""#, r#""actor":"agent","actor":"cli""#);
        assert!(
            matches!(read_line(twice.as_bytes()), Err(LineError::Envelope(_))),
            "a key named twice"
        );
        assert_eq!(
            read_line(JR_02.as_bytes())
                .expect("JR-02's line")
                .origin
                .to_string(),
            "0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:40"
        );
    }
}
