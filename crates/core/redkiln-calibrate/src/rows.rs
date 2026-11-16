//! Reading decision logs into what calibration needs: every file, oldest first, into the
//! answered decisions, the unanswered counts and the latest label of each id (and whether it
//! came through a mod).
//!
//! This reads anneal's v1 rows only, the JSONL that `bench decide` and `decide label` write
//! (`{"kind": "decision", "id", "ts_ms", "point", "question", "probability", "top",
//! "probabilities", "action", ...}` and `{"kind": "label", "id", "outcome", "via"?}`); it is not
//! the format of Core's calibration records, which step 6 defines. The point and question are
//! the caller's types (`P`, `Q`), so a row naming a point or question the caller does not know
//! is skipped, as one this build cannot read.
//!
//! Each file is read whole, and must be a regular file (checked by `symlink_metadata` before
//! it is opened) of at most [`MAX_LOG_FILE_BYTES`]: the log's 16 MiB rotation size plus
//! 64 rows of its 8 KiB row cap. The log appends without a lock, so concurrent
//! writers can each pass its size check and leave the live file a few rows past 16 MiB; a file
//! larger still was not written by the log and is an error. Rows are read tolerantly (unknown
//! fields ignored), as a newer writer may add some; a line that is not a decision or label row
//! this build reads, or a probability outside 0..=1, is counted in `skipped_rows`, not dropped
//! silently and not fatal, since one torn line must not stop calibration.

use std::collections::{BTreeMap, HashMap};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde::de::DeserializeOwned;

use crate::log::{DecisionLog, MAX_FILE_BYTES};
use crate::score::Probability;

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

/// One line of the log.
#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[serde(bound = "P: DeserializeOwned, Q: DeserializeOwned")]
enum RowDto<P, Q> {
    Decision(DecisionDto<P, Q>),
    Label(LabelDto),
}

#[derive(Debug, Deserialize)]
#[serde(bound = "P: DeserializeOwned, Q: DeserializeOwned")]
struct DecisionDto<P, Q> {
    id: String,
    /// Read as any JSON value, so a row whose `ts_ms` is not a non-negative JSON integer still
    /// reads; such a row is left out of `answered_ms`.
    ts_ms: Option<serde_json::Value>,
    point: P,
    question: Q,
    probability: Option<f64>,
    top: Option<String>,
    probabilities: Option<BTreeMap<String, f64>>,
    action: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LabelDto {
    id: String,
    outcome: String,
    /// Read as text, so a label through something a newer build names still reads; absent on
    /// rows written before `--via` and on rows written without it.
    via: Option<String>,
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
            read_into(&bytes, &mut rows);
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

/// Adds the rows of one file to `rows`.
fn read_into<P, Q>(bytes: &[u8], rows: &mut LogRows<P, Q>)
where
    P: DeserializeOwned + Ord,
    Q: DeserializeOwned + Ord,
{
    for line in bytes.split(|byte| *byte == b'\n') {
        if line.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        match serde_json::from_slice(line) {
            Ok(RowDto::Decision(decision)) => add_decision(decision, rows),
            Ok(RowDto::Label(label)) => add_label(label, rows),
            Err(_not_a_row) => rows.skipped_rows = rows.skipped_rows.saturating_add(1),
        }
    }
}

/// Records `label` as its id's latest, with where it came from: a later label replaces an
/// earlier one's outcome and its `via` alike.
fn add_label<P, Q>(label: LabelDto, rows: &mut LogRows<P, Q>) {
    let via_mod = label.via.as_deref() == Some(VIA_MOD);
    rows.labels.insert(
        label.id,
        Label {
            outcome: label.outcome,
            via_mod,
        },
    );
}

fn add_decision<P: Ord, Q: Ord>(decision: DecisionDto<P, Q>, rows: &mut LogRows<P, Q>) {
    let Some(probability) = decision.probability else {
        let unanswered = rows
            .unanswered
            .entry((decision.point, decision.question))
            .or_insert(0);
        *unanswered = unanswered.saturating_add(1);
        return;
    };
    let Ok(probability) = Probability::try_from(probability) else {
        rows.skipped_rows = rows.skipped_rows.saturating_add(1);
        return;
    };
    rows.answered.push(Answered {
        id: decision.id,
        point: decision.point,
        question: decision.question,
        probability,
        top: decision.top,
        // A row without `probabilities` names no options, so a choice label cannot match one
        // and counts as unusable; nothing is hidden.
        options: decision
            .probabilities
            .map(|options| options.into_keys().collect())
            .unwrap_or_default(),
        asked: decision.action.as_deref().is_some_and(is_ask),
    });
    rows.answered_ms
        .push(decision.ts_ms.as_ref().and_then(serde_json::Value::as_u64));
}

/// An action that hands the decision to a human: `ask`, or a name starting `ask_`.
fn is_ask(action: &str) -> bool {
    action == "ask" || action.starts_with("ask_")
}

#[cfg(test)]
mod tests;
