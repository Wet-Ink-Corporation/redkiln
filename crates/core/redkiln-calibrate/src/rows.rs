//! Reading decision logs into what calibration needs: every file, oldest first, into the
//! answered decisions, the unanswered counts and the latest label of each id (and whether it
//! came through a mod).
//!
//! A line is either form of SPEC CA-04, read alike: a calibration record (an EV-01 envelope
//! of a [`crate::record`] type, without `origin` or tags), or one of anneal's v1 rows, the
//! JSONL that `bench decide` and `decide label` write (`{"kind": "decision", "id", "ts_ms",
//! "point", "question", "probability", "top", "probabilities", "action", ...}` and
//! `{"kind": "label", "id", "outcome", "actor"?, "via"?}`), upcast onto the record types by
//! CA-04's field map, so both reach [`Answered`] and [`Label`] the same way. A record counts
//! only in the log of its producer, the log's folder name (`<root>/calibrate/<producer>/`,
//! ST-17); every v1 row is Bench's. The point and question are the caller's types (`P`, `Q`),
//! read from the record's text, so a line naming a point or question the caller does not know
//! is skipped, as one this build cannot read.
//!
//! Each file is read whole, and must be a regular file (checked by `symlink_metadata` before
//! it is opened) of at most [`MAX_LOG_FILE_BYTES`]: the log's 16 MiB rotation size plus
//! 64 rows of its 8 KiB row cap. The log appends without a lock, so concurrent
//! writers can each pass its size check and leave the live file a few rows past 16 MiB; a file
//! larger still was not written by the log and is an error. Lines are read tolerantly (unknown
//! fields ignored), as a newer writer may add some; a line this build cannot read (CA-04 lists
//! them: a torn line, another record type or version, a probability outside 0..=1, ...) is
//! counted in `skipped_rows`, not dropped silently and not fatal, since one torn line must not
//! stop calibration.

use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use serde::de::value::{Error as TextError, StrDeserializer};
use serde::de::{DeserializeOwned, IntoDeserializer};

use crate::log::{DecisionLog, MAX_FILE_BYTES};
use crate::record::{Answer, JudgementLabelled, JudgementRecorded, Vocab};
use crate::score::Probability;

use line::{Entry, Line};

/// The log's row cap ([`crate::log::MAX_ROW_BYTES`]) as a `u64`; a test keeps them equal.
pub(crate) const ROW_BYTES: u64 = 8 * 1024;

/// How many rows past the rotation size a live file may hold: concurrent appends that each
/// passed the log's size check before any of them landed.
pub(crate) const HEADROOM_ROWS: u64 = 64;

/// The largest log file read: the rotation size plus the headroom (16.5 MiB).
pub const MAX_LOG_FILE_BYTES: u64 = MAX_FILE_BYTES + HEADROOM_ROWS * ROW_BYTES;

/// A label's `via` when it came through a mod.
pub const VIA_MOD: &str = "mod";

/// An answered decision: one with a probability.
#[derive(Debug, Clone, PartialEq)]
pub struct Answered<P, Q> {
    /// The decision's id, which labels refer to.
    pub id: String,
    /// The decision point.
    pub point: P,
    /// The question type.
    pub question: Q,
    /// The probability the thresholds applied to (a row outside 0..=1 is skipped).
    pub probability: Probability,
    /// The top option of a choice.
    pub top: Option<String>,
    /// The options a probability was given for, by name.
    pub options: Vec<String>,
    /// Whether the action handed the decision to a human (`ask`, `ask_*`).
    pub asked: bool,
}

/// The latest label of a decision id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    /// The outcome it records.
    pub outcome: String,
    /// Whether it came through a mod (`via: mod`).
    pub via_mod: bool,
}

/// What the logs hold, as calibration needs it.
#[derive(Debug)]
pub struct LogRows<P, Q> {
    /// The files read, oldest first.
    pub files: Vec<PathBuf>,
    /// Answered decisions, in log order.
    pub answered: Vec<Answered<P, Q>>,
    /// Decisions without an answer, per point and question.
    pub unanswered: BTreeMap<(P, Q), u32>,
    /// The latest label of each decision id.
    pub labels: HashMap<String, Label>,
    /// Lines that are not a row this build reads.
    pub skipped_rows: u32,
    /// When each of `answered` was decided (its `ts_ms`), by index: [`read`] pushes one entry
    /// per answered decision, `None` for a row whose `ts_ms` is missing or not a non-negative
    /// JSON integer. A reader treats a missing entry as `None`.
    pub answered_ms: Vec<Option<u64>>,
}

impl<P, Q> Default for LogRows<P, Q> {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            answered: Vec::new(),
            unanswered: BTreeMap::new(),
            labels: HashMap::new(),
            skipped_rows: 0,
            answered_ms: Vec::new(),
        }
    }
}

/// Why the logs could not be read.
#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    /// A file could not be read.
    #[error("reading {path}", path = path.display())]
    Io {
        /// The file.
        path: PathBuf,
        /// The failure.
        #[source]
        source: io::Error,
    },
    /// Something other than a regular file is where a log file goes.
    #[error("{path} is not a regular file", path = path.display())]
    NotAFile {
        /// The path.
        path: PathBuf,
    },
    /// A file is larger than any the log writes.
    #[error("{path} is larger than {cap} bytes", path = path.display())]
    TooLarge {
        /// The file.
        path: PathBuf,
        /// The most read.
        cap: u64,
    },
}

/// Reads every file of each of `logs` in turn, each oldest first: pass an older log before a
/// newer one, so a newer label is the latest.
///
/// # Errors
/// [`ReadError::Io`] when a file cannot be read; [`ReadError::NotAFile`] when one is not a
/// regular file; [`ReadError::TooLarge`] when one is over [`MAX_LOG_FILE_BYTES`].
pub fn read<'a, P, Q>(
    logs: impl IntoIterator<Item = &'a DecisionLog>,
) -> Result<LogRows<P, Q>, ReadError>
where
    P: DeserializeOwned + Ord,
    Q: DeserializeOwned + Ord,
{
    let mut rows = LogRows::default();
    for log in logs {
        for path in log.files() {
            let Some(bytes) = read_file(&path)? else {
                continue;
            };
            // A record counts only in its producer's folder (ST-17).
            let producer = log.dir().file_name().and_then(|name| name.to_str());
            read_into(&bytes, producer, &mut rows);
            rows.files.push(path);
        }
    }
    Ok(rows)
}

/// The whole of the log file at `path`, or `None` when there is none.
fn read_file(path: &Path) -> Result<Option<Vec<u8>>, ReadError> {
    let io_error = |source| ReadError::Io {
        path: path.to_path_buf(),
        source,
    };
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_file() => {}
        Ok(_other) => {
            return Err(ReadError::NotAFile {
                path: path.to_path_buf(),
            });
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(io_error(source)),
    }
    let file = match File::open(path) {
        Ok(file) => file,
        // Rotated away since its metadata was read.
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(io_error(source)),
    };
    let mut bytes = Vec::new();
    // One byte past the cap, so a file over it is reported, never silently cut.
    file.take(MAX_LOG_FILE_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if u64::try_from(bytes.len()).map_or(true, |len| len > MAX_LOG_FILE_BYTES) {
        return Err(ReadError::TooLarge {
            path: path.to_path_buf(),
            cap: MAX_LOG_FILE_BYTES,
        });
    }
    Ok(Some(bytes))
}

/// Adds the lines of one file of `producer`'s log to `rows`.
fn read_into<P, Q>(bytes: &[u8], producer: Option<&str>, rows: &mut LogRows<P, Q>)
where
    P: DeserializeOwned + Ord,
    Q: DeserializeOwned + Ord,
{
    for bytes in bytes.split(|byte| *byte == b'\n') {
        if bytes.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        let added = match line::parse(bytes, producer) {
            Some(Line {
                ms,
                entry: Entry::Judgement(judgement),
            }) => add_judgement(judgement, ms, rows),
            Some(Line {
                entry: Entry::Label(label),
                ..
            }) => {
                add_label(label, rows);
                true
            }
            None => false,
        };
        if !added {
            rows.skipped_rows = rows.skipped_rows.saturating_add(1);
        }
    }
}

/// Records `label` as its judgement's latest, with where it came from: a later label replaces
/// an earlier one's outcome and its `via` alike.
fn add_label<P, Q>(label: JudgementLabelled, rows: &mut LogRows<P, Q>) {
    let via_mod = label
        .by
        .as_ref()
        .is_some_and(|by| by.via.as_deref() == Some(VIA_MOD));
    rows.labels.insert(
        label.judgement.into_string(),
        Label {
            outcome: label.outcome.into_string(),
            via_mod,
        },
    );
}

/// Adds `judgement`, written at `ms`, as answered or unanswered; `false` when its point or
/// question is not the caller's.
fn add_judgement<P, Q>(
    judgement: JudgementRecorded,
    ms: Option<u64>,
    rows: &mut LogRows<P, Q>,
) -> bool
where
    P: DeserializeOwned + Ord,
    Q: DeserializeOwned + Ord,
{
    let (Ok(point), Ok(question)) = (
        caller_type::<P>(&judgement.point),
        caller_type::<Q>(&judgement.question),
    ) else {
        return false;
    };
    match judgement.answer {
        Answer::Failed { .. } => {
            let unanswered = rows.unanswered.entry((point, question)).or_insert(0);
            *unanswered = unanswered.saturating_add(1);
        }
        Answer::Given {
            probability,
            options,
            top,
            asked,
            ..
        } => {
            rows.answered.push(Answered {
                id: judgement.id.into_string(),
                point,
                question,
                probability,
                top,
                // A judgement without options names none, so a choice label cannot match one
                // and counts as unusable; nothing is hidden.
                options: options.into_keys().collect(),
                asked,
            });
            rows.answered_ms.push(ms);
        }
    }
    true
}

/// The caller's point or question type read from a record's text.
fn caller_type<T: DeserializeOwned>(text: &Vocab) -> Result<T, TextError> {
    let text: StrDeserializer<'_, TextError> = text.as_str().into_deserializer();
    T::deserialize(text)
}

mod line;

#[cfg(test)]
mod tests;
