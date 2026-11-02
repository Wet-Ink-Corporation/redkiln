//! The import (SPEC JR-10 to JR-14): a journal read line by line, each well-formed envelope
//! stored under its `origin` (ID-02) at `asserted` trust (JR-12), and the import recorded as one
//! `core.import_completed` event (JR-13).
//!
//! Identity makes it idempotent (JR-11): happenstance skips, inside the write, an event whose
//! identity the store already holds. A malformed line is skipped and reported, never fatal
//! (JR-10). The source is only read (JR-14): a file is opened read-only.
//!
//! Lines are stored in batches of [`BATCH`] events, each in one transaction, so an import that
//! fails part way keeps the batches it stored and the next import appends only the rest. Memory
//! is bounded by a batch of lines at [`MAX_LINE_BYTES`] each.

use std::fs::File;
use std::io::{self, BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::line::{LineError, MAX_LINE_BYTES, ReadLine, read_line};
use crate::envelope::{Author, Class, Envelope, EnvelopeError, Payload, Timestamp};
use crate::store::{Foreign, Ingest, NewEvent, Store, StoreError};

/// Lines stored per transaction: happenstance's smallest supported ingest batch.
const BATCH: usize = 128;

/// The most problems an import reports by line; [`ImportReport::malformed`] counts every one.
pub const MAX_PROBLEMS: usize = 100;

/// How an import is recorded (JR-13): where it read its journal, when and by whom.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recording {
    /// Where the journal was read.
    pub source: ImportSource,
    /// When the import ran.
    pub ts: Timestamp,
    /// Who ran it.
    pub author: Author,
}

/// Where an import read its journal (JR-13).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ImportSource {
    /// A file, by its path.
    Path(String),
    /// A journal ref at a commit (JR-07).
    Ref {
        /// The ref, `refs/redkiln/journal/<name>`.
        name: String,
        /// The commit read.
        commit: String,
    },
    /// A workflow artifact, by its name.
    Artifact(String),
}

/// The record of one import (JR-13): `core.import_completed`, version 1, durable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportCompleted {
    /// Where the journal was read.
    pub source: ImportSource,
    /// Lines read, blank ones and a torn last line left out.
    pub read: u64,
    /// Events appended.
    pub imported: u64,
    /// Lines skipped: the store already held their identity.
    pub present: u64,
    /// Lines skipped as malformed.
    pub malformed: u64,
}

impl Payload for ImportCompleted {
    const TYPE: &'static str = "core.import_completed";
    const VERSION: u32 = 1;
    const CLASS: Class = Class::Durable;
}

/// A line an import skipped as malformed.
#[derive(Debug)]
pub struct Problem {
    /// Its number in the journal, from 1.
    pub line: u64,
    /// Why it was skipped.
    pub reason: LineError,
}

/// What an import did.
#[derive(Debug, Default)]
#[must_use]
pub struct ImportReport {
    /// Lines read, blank ones and a torn last line left out.
    pub read: u64,
    /// Events appended.
    pub imported: u64,
    /// Lines skipped: the store already held their identity.
    pub present: u64,
    /// Lines skipped as malformed.
    pub malformed: u64,
    /// Whether the journal ended in a line with no `\n` that does not decode: a writer stopped
    /// mid-line (JR-03). It is passed over, not counted.
    pub incomplete: bool,
    /// The first [`MAX_PROBLEMS`] malformed lines.
    pub problems: Vec<Problem>,
}

impl ImportReport {
    fn skip(&mut self, line: u64, reason: LineError) {
        self.malformed = self.malformed.saturating_add(1);
        if self.problems.len() < MAX_PROBLEMS {
            self.problems.push(Problem { line, reason });
        }
    }
}

/// Why an import stopped. Batches stored before it stay, and are present to the next import.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    /// The journal could not be read.
    #[error("reading the journal")]
    Read(#[source] io::Error),
    /// The journal file could not be opened.
    #[error("opening the journal {}", path.display())]
    Open {
        /// The journal file.
        path: PathBuf,
        /// What went wrong.
        #[source]
        source: io::Error,
    },
    /// The store refused a batch, or the import's record.
    #[error("writing the store")]
    Store(#[source] StoreError),
    /// The import's record could not be built.
    #[error("recording the import")]
    Record(#[source] EnvelopeError),
}

/// Imports the journal at `path` into `store`, recording it as written at `ts` by `author`, with
/// the path as its source.
///
/// # Errors
/// [`ImportError::Open`] when the file cannot be opened; otherwise as [`import`].
pub fn import_file(
    store: &Store,
    path: &Path,
    ts: Timestamp,
    author: Author,
) -> Result<ImportReport, ImportError> {
    let file = File::open(path).map_err(|source| ImportError::Open {
        path: path.to_path_buf(),
        source,
    })?;
    let recording = Recording {
        source: ImportSource::Path(path.to_string_lossy().into_owned()),
        ts,
        author,
    };
    import(store, BufReader::new(file), recording)
}

/// Imports the journal `reader` yields into `store` (JR-10 to JR-12), then appends one
/// [`ImportCompleted`] as `recording` says (JR-13).
///
/// # Errors
/// [`ImportError::Read`] when `reader` fails, [`ImportError::Store`] when the store refuses a
/// batch or the record, and [`ImportError::Record`] when the record cannot be built.
pub fn import(
    store: &Store,
    mut reader: impl BufRead,
    recording: Recording,
) -> Result<ImportReport, ImportError> {
    let mut imported = ImportReport::default();
    let mut batch = Batch::default();
    let mut buf = Vec::new();
    let mut number = 0_u64;
    loop {
        let next = next_line(&mut reader, &mut buf).map_err(ImportError::Read)?;
        number = number.saturating_add(1);
        let complete = match next {
            Next::End => break,
            Next::TooLong { len } => {
                imported.read = imported.read.saturating_add(1);
                imported.skip(number, LineError::TooLong { len });
                continue;
            }
            Next::Line { complete } => complete,
        };
        let text = buf.strip_suffix(b"\r").unwrap_or(&buf);
        if text.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        match read_line(text).and_then(foreign) {
            Ok(event) => {
                imported.read = imported.read.saturating_add(1);
                batch.push(number, event);
            }
            Err(_) if !complete => imported.incomplete = true,
            Err(reason) => {
                imported.read = imported.read.saturating_add(1);
                imported.skip(number, reason);
            }
        }
        if batch.events.len() >= BATCH {
            batch.store(store, &mut imported)?;
        }
    }
    batch.store(store, &mut imported)?;
    record(store, recording, &imported)?;
    Ok(imported)
}

/// What [`next_line`] read into its buffer.
enum Next {
    /// A line, without its `\n`; `complete` when a `\n` ended it.
    Line { complete: bool },
    /// A line longer than [`MAX_LINE_BYTES`], read past and not kept.
    TooLong { len: usize },
    /// The end of the journal.
    End,
}

/// Reads the next line of `reader` into `buf`, holding at most [`MAX_LINE_BYTES`] and a `\r`.
fn next_line(reader: &mut impl BufRead, buf: &mut Vec<u8>) -> io::Result<Next> {
    buf.clear();
    let limit = u64::try_from(MAX_LINE_BYTES.saturating_add(2)).unwrap_or(u64::MAX);
    let read = Read::take(&mut *reader, limit).read_until(b'\n', buf)?;
    if read == 0 {
        return Ok(Next::End);
    }
    if buf.pop_if(|b| *b == b'\n').is_some() {
        return Ok(Next::Line { complete: true });
    }
    if buf.len() <= MAX_LINE_BYTES.saturating_add(1) {
        return Ok(Next::Line { complete: false });
    }
    let mut len = buf.len();
    buf.clear();
    loop {
        let available = reader.fill_buf()?;
        if available.is_empty() {
            return Ok(Next::TooLong { len });
        }
        let (used, ended) = match available.iter().position(|b| *b == b'\n') {
            Some(at) => (at.saturating_add(1), true),
            None => (available.len(), false),
        };
        reader.consume(used);
        len = len.saturating_add(used);
        if ended {
            return Ok(Next::TooLong {
                len: len.saturating_sub(1),
            });
        }
    }
}

/// A read line as the store takes it.
fn foreign(line: ReadLine) -> Result<Foreign, LineError> {
    let ReadLine {
        origin,
        event_type,
        data,
        tags,
        recorded_at_millis,
    } = line;
    let event = tags.into_iter().fold(
        NewEvent::new(event_type.as_str(), data).map_err(LineError::Refused)?,
        NewEvent::with_tag,
    );
    Ok(Foreign {
        id: origin,
        recorded_at_millis,
        event,
    })
}

/// Lines waiting to be stored, with their line numbers.
#[derive(Default)]
struct Batch {
    numbers: Vec<u64>,
    events: Vec<Foreign>,
}

impl Batch {
    fn push(&mut self, number: u64, event: Foreign) {
        self.numbers.push(number);
        self.events.push(event);
    }

    /// Stores the batch in one transaction. A line claiming the store's own identity at a
    /// position it does not hold makes happenstance refuse the whole batch; that line is
    /// skipped as malformed and the rest stored.
    fn store(&mut self, store: &Store, imported: &mut ImportReport) -> Result<(), ImportError> {
        while !self.events.is_empty() {
            match store.ingest(&self.events).map_err(ImportError::Store)? {
                Ingest::Done { appended, skipped } => {
                    imported.imported = imported.imported.saturating_add(count(appended));
                    imported.present = imported.present.saturating_add(count(skipped));
                    break;
                }
                Ingest::UnheldOwn(id) => {
                    let mut kept = Self::default();
                    for (number, event) in self.numbers.drain(..).zip(self.events.drain(..)) {
                        if event.id == id {
                            imported.skip(number, LineError::UnheldOwnIdentity);
                        } else {
                            kept.push(number, event);
                        }
                    }
                    *self = kept;
                }
            }
        }
        self.numbers.clear();
        self.events.clear();
        Ok(())
    }
}

fn count(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
}

/// Appends the import's record (JR-13).
fn record(store: &Store, recording: Recording, imported: &ImportReport) -> Result<(), ImportError> {
    let Recording { source, ts, author } = recording;
    let completed = ImportCompleted {
        source,
        read: imported.read,
        imported: imported.imported,
        present: imported.present,
        malformed: imported.malformed,
    };
    let event = Envelope::new(ts, author, completed)
        .to_event([])
        .map_err(ImportError::Record)?;
    store
        .append(vec![event], None)
        .map_err(ImportError::Store)?;
    Ok(())
}
