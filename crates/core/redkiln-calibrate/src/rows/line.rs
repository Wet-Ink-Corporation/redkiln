//! One line of a decision log, in either form (CA-04): an anneal v1 row, upcast onto the
//! record types by CA-04's field map, or a record, an EV-01 envelope of a CA-02 type.
//!
//! [`parse`] gives `None` for a line this build cannot read, which the caller counts as
//! skipped: neither form, a v1 row the map cannot take, or a record of another type, another
//! `version`, another producer, a `ts` that is not EV-01's form, `data` that is not the type,
//! or more bytes than the log's row cap.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{Map, Value};

use super::ROW_BYTES;
use crate::record::{
    Answer, JUDGEMENT_LABELLED, JUDGEMENT_RECORDED, JudgementId, JudgementLabelled,
    JudgementRecorded, LabelActor, LabelBy, Mode, Outcome, Producer, VERSION, Vocab,
};
use crate::score::Probability;

/// A failure's kind when a v1 row names none.
const UNKNOWN_FAILURE: &str = "unknown";

/// A judgement's model when a v1 row names none as text: CA-04 maps it to `""`, the record's
/// form of "not recorded" (a non-text `model` stays in `detail`).
const MODEL_NOT_RECORDED: &str = "";

/// A line read: when it was written (the v1 `ts_ms`, or the envelope's `ts`, in milliseconds
/// since the Unix epoch) and what it holds.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Line {
    /// When it was written; `None` for a v1 row whose `ts_ms` is not a non-negative integer.
    pub(super) ms: Option<u64>,
    /// The record it is, or was upcast to.
    pub(super) entry: Entry,
}

/// A line's record.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Entry {
    /// A judgement.
    Judgement(JudgementRecorded),
    /// A label.
    Label(JudgementLabelled),
}

impl Entry {
    fn producer(&self) -> &str {
        match self {
            Self::Judgement(judgement) => judgement.producer.as_str(),
            Self::Label(label) => label.producer.as_str(),
        }
    }
}

/// An anneal v1 row.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum RowDto {
    Decision(DecisionDto),
    Label(LabelDto),
}

/// A v1 decision row: the keys the record's typed fields take, and every other in `rest`.
#[derive(Debug, Deserialize)]
struct DecisionDto {
    id: String,
    /// Read as any JSON value, so a row whose `ts_ms` is not a non-negative JSON integer still
    /// reads, with no time.
    ts_ms: Option<Value>,
    point: String,
    question: String,
    probability: Option<f64>,
    top: Option<String>,
    probabilities: Option<BTreeMap<String, f64>>,
    action: Option<String>,
    #[serde(flatten)]
    rest: Map<String, Value>,
}

/// A v1 label row; its other keys are not kept (the type has no `detail`).
#[derive(Debug, Deserialize)]
struct LabelDto {
    id: String,
    ts_ms: Option<Value>,
    outcome: String,
    /// Read as any value, so a label by someone this build does not name still reads, without
    /// its actor (anneal's reader never read the key).
    actor: Option<Value>,
    /// Read as text, so a label through something a newer build names still reads.
    via: Option<String>,
}

/// A record line's envelope; the keys a reader does not need are ignored (EV-09).
#[derive(Debug, Deserialize)]
struct EnvelopeDto {
    #[serde(rename = "type")]
    event_type: String,
    version: u32,
    ts: String,
    data: Value,
}

/// The line `bytes` of a log whose folder names `producer`, or `None` when this build cannot
/// read it.
pub(super) fn parse(bytes: &[u8], producer: Option<&str>) -> Option<Line> {
    match serde_json::from_slice(bytes) {
        Ok(RowDto::Decision(decision)) => upcast_decision(decision),
        Ok(RowDto::Label(label)) => upcast_label(label),
        Err(_not_a_v1_row) => record(bytes, producer),
    }
}

/// A v1 decision row as `core.judgement_recorded` (CA-04's field map).
fn upcast_decision(row: DecisionDto) -> Option<Line> {
    let mut rest = row.rest;
    rest.retain(|_, value| !value.is_null());
    let model = take_string(&mut rest, "model").unwrap_or_else(|| MODEL_NOT_RECORDED.to_owned());
    let mode = match take_string(&mut rest, "mode").as_deref() {
        Some("enforce") => Mode::Enforce,
        Some("shadow") | None => Mode::Shadow,
        Some(other) => {
            rest.insert("mode".to_owned(), Value::from(other));
            Mode::Shadow
        }
    };
    let answer = if let Some(probability) = row.probability {
        Answer::Given {
            probability: Probability::try_from(probability).ok()?,
            options: match row.probabilities {
                // A bad option skips the row.
                Some(options) => options_of(options)?,
                // A row without `probabilities` names no options.
                None => BTreeMap::new(),
            },
            top: row.top,
            asked: row.action.as_deref().is_some_and(is_ask),
            action: row.action,
        }
    } else {
        // Answer keys beside no probability are the producer's to keep.
        if let Some(options) = row.probabilities {
            let numbers = options.into_iter().map(|(k, v)| (k, Value::from(v)));
            rest.insert("probabilities".to_owned(), numbers.collect());
        }
        for (key, value) in [("top", row.top), ("action", row.action)] {
            if let Some(value) = value {
                rest.insert(key.to_owned(), Value::String(value));
            }
        }
        Answer::Failed {
            kind: take_string(&mut rest, "error").unwrap_or_else(|| UNKNOWN_FAILURE.to_owned()),
            reason: take_string(&mut rest, "error_reason"),
        }
    };
    let record = JudgementRecorded {
        producer: Producer::v1(),
        id: JudgementId::try_from(row.id).ok()?,
        point: Vocab::try_from(row.point).ok()?,
        question: Vocab::try_from(row.question).ok()?,
        model,
        mode,
        answer,
        subject: None,
        detail: Some(rest).filter(|rest| !rest.is_empty()),
    };
    Some(Line {
        ms: row.ts_ms.as_ref().and_then(Value::as_u64),
        entry: Entry::Judgement(record),
    })
}

/// A v1 label row as `core.judgement_labelled`.
fn upcast_label(row: LabelDto) -> Option<Line> {
    let actor = row.actor.as_ref().and_then(label_actor);
    let by = (actor.is_some() || row.via.is_some()).then_some(LabelBy {
        actor,
        via: row.via,
    });
    let label = JudgementLabelled {
        producer: Producer::v1(),
        judgement: JudgementId::try_from(row.id).ok()?,
        outcome: Outcome::try_from(row.outcome).ok()?,
        by,
    };
    Some(Line {
        ms: row.ts_ms.as_ref().and_then(Value::as_u64),
        entry: Entry::Label(label),
    })
}

/// A record line of `producer`'s log.
fn record(bytes: &[u8], producer: Option<&str>) -> Option<Line> {
    // The row cap counts the line's newline, which `bytes` leaves out.
    if u64::try_from(bytes.len()).map_or(true, |len| len >= ROW_BYTES) {
        return None;
    }
    let envelope: EnvelopeDto = serde_json::from_slice(bytes).ok()?;
    if envelope.version != VERSION {
        return None;
    }
    let ms = unix_millis(&envelope.ts)?;
    let entry = match envelope.event_type.as_str() {
        JUDGEMENT_RECORDED => Entry::Judgement(serde_json::from_value(envelope.data).ok()?),
        JUDGEMENT_LABELLED => Entry::Label(serde_json::from_value(envelope.data).ok()?),
        _other => return None,
    };
    (producer == Some(entry.producer())).then_some(Line {
        ms: Some(ms),
        entry,
    })
}

/// Removes `key` from `rest` and returns it when it is a string; any other value stays.
fn take_string(rest: &mut Map<String, Value>, key: &str) -> Option<String> {
    match rest.remove(key) {
        Some(Value::String(text)) => Some(text),
        Some(other) => {
            rest.insert(key.to_owned(), other);
            None
        }
        None => None,
    }
}

fn options_of(options: BTreeMap<String, f64>) -> Option<BTreeMap<String, Probability>> {
    options
        .into_iter()
        .map(|(name, value)| Probability::try_from(value).ok().map(|p| (name, p)))
        .collect()
}

/// An action that hands the decision to a human: `ask`, or a name starting `ask_`.
fn is_ask(action: &str) -> bool {
    action == "ask" || action.starts_with("ask_")
}

/// A v1 label's `actor` when it is one CA-02 names; any other value is not kept (CA-04).
fn label_actor(actor: &Value) -> Option<LabelActor> {
    match actor.as_str() {
        Some("human") => Some(LabelActor::Human),
        Some("model") => Some(LabelActor::Model),
        _ => None,
    }
}

/// The milliseconds since the Unix epoch of an EV-01 `ts`, `YYYY-MM-DDTHH:MM:SS(.mmm)?Z`, from
/// 1970 on; `None` for anything else, a day its month lacks (31 February) among them.
pub(super) fn unix_millis(ts: &str) -> Option<u64> {
    let (date, time) = ts.strip_suffix('Z')?.split_once('T')?;
    let (clock, millis) = match time.split_once('.') {
        Some((clock, millis)) if millis.len() == 3 => (clock, number(millis)?),
        Some(_) => return None,
        None => (time, 0),
    };
    let [year, month, day] = fields(date, '-', [4, 2, 2])?;
    let [hour, minute, second] = fields(clock, ':', [2, 2, 2])?;
    if year < 1970
        || !(1..=days_in_month(year, month)).contains(&day)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return None;
    }
    let seconds = days_from_civil(year, month, day)
        .checked_mul(86_400)?
        .checked_add(hour * 3600 + minute * 60 + second)?;
    seconds.checked_mul(1000)?.checked_add(millis)
}

/// The days in `month` of `year` (Gregorian); 0 for a month outside 1 to 12, so no day fits.
const fn days_in_month(year: u64, month: u64) -> u64 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) => {
            29
        }
        2 => 28,
        _ => 0,
    }
}

/// `text` split at `separator` into three numbers of exactly `widths` digits.
fn fields(text: &str, separator: char, widths: [usize; 3]) -> Option<[u64; 3]> {
    let mut parts = text.split(separator);
    let mut out = [0; 3];
    for (slot, width) in out.iter_mut().zip(widths) {
        let part = parts.next().filter(|part| part.len() == width)?;
        *slot = number(part)?;
    }
    parts.next().is_none().then_some(out)
}

/// `text` as a number when it is only ASCII digits.
fn number(text: &str) -> Option<u64> {
    if text.bytes().all(|byte| byte.is_ascii_digit()) {
        text.parse().ok()
    } else {
        None
    }
}

/// Days from 1970-01-01 to a date from 1970 on (Howard Hinnant's days-from-civil); the year,
/// month and day are checked by the caller.
fn days_from_civil(year: u64, month: u64, day: u64) -> u64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year / 400;
    let yoe = year % 400;
    let shifted = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * shifted + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    // 719 468 is the day count from 0000-03-01 to 1970-01-01; a year from 1970 is past it.
    (era * 146_097 + doe).saturating_sub(719_468)
}
