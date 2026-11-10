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

use happenstance_core::{EventId, StoreId};
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
    /// An old plugin source (ST-15): its path, and the store id its events take (the old
    /// ledger's, or the plugin's rule for a source that is not a store).
    Legacy {
        /// The source's path, as the importer found it.
        path: String,
        /// The store id, written as ID-01 says (32 lowercase hex) and read only in that form.
        store_id: StoreId,
    },
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
    /// The store refused a batch for an identity of its own it does not hold, but no line of
    /// the batch names it; nothing of the batch was written.
    #[error("the store refused {id}, which no line of the batch names")]
    UnheldOwnUnmatched {
        /// The identity the store named.
        id: EventId,
    },
}

/// Imports the journal at `path` into `store`, recording it as written at `ts` by `author`, with
/// the path as its source.
///
/// # Errors
/// [`ImportError::Open`] when the file cannot be opened; otherwise as [`import`]. On Windows a
/// journal a checkpoint is writing is locked against reads, so importing it then fails with
/// [`ImportError::Read`]; the batches stored before stay, and an import after the checkpoint
/// appends only the rest (JR-11).
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
/// [`ImportError::UnheldOwnUnmatched`] when the store refuses an identity no line names. A
/// reader over a file a checkpoint holds fails on Windows, which locks it against reads while
/// it writes: [`ImportError::Read`], with the batches before kept.
pub fn import(
    store: &Store,
    mut reader: impl BufRead,
    recording: Recording,
) -> Result<ImportReport, ImportError> {
    let mut importer = Importer::default();
    let mut buf = Vec::new();
    let mut number = 0_u64;
    loop {
        let next = next_line(&mut reader, &mut buf).map_err(ImportError::Read)?;
        number = number.saturating_add(1);
        let complete = match next {
            Next::End => break,
            Next::TooLong { len } => {
                importer.take(number, Err(LineError::TooLong { len }));
                continue;
            }
            Next::Line { complete } => complete,
        };
        let text = buf.strip_suffix(b"\r").unwrap_or(&buf);
        if text.iter().all(u8::is_ascii_whitespace) {
            continue;
        }
        match read_line(text).and_then(foreign) {
            Err(_) if !complete => importer.report.incomplete = true,
            read => importer.take(number, read),
        }
        if importer.full() {
            importer.flush(store)?;
        }
    }
    importer.finish(store, recording)
}

/// An import under way: what it read so far, and the lines waiting to be stored. Both import
/// entries ([`import`] and [`super::import_mapped`]) drive it, so they batch, skip and record
/// alike.
#[derive(Default)]
pub(super) struct Importer {
    /// What the import did so far.
    pub(super) report: ImportReport,
    batch: Batch,
}

impl Importer {
    /// Takes record `number` as read: a line to store, or one skipped as malformed.
    pub(super) fn take(&mut self, number: u64, read: Result<Foreign, LineError>) {
        self.report.read = self.report.read.saturating_add(1);
        match read {
            Ok(event) => self.batch.push(number, event),
            Err(reason) => self.report.skip(number, reason),
        }
    }

    /// Whether a whole batch is waiting.
    pub(super) fn full(&self) -> bool {
        self.batch.events.len() >= BATCH
    }

    /// Whether any line is waiting.
    pub(super) fn pending(&self) -> bool {
        !self.batch.events.is_empty()
    }

    /// Stores the waiting lines (see [`Batch::store`]).
    pub(super) fn flush(&mut self, store: &Store) -> Result<(), ImportError> {
        self.batch.store(store, &mut self.report)
    }

    /// Stores what waits, then appends the import's record (JR-13).
    pub(super) fn finish(
        mut self,
        store: &Store,
        recording: Recording,
    ) -> Result<ImportReport, ImportError> {
        self.flush(store)?;
        record(store, recording, &self.report)?;
        Ok(self.report)
    }
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
pub(super) fn foreign(line: ReadLine) -> Result<Foreign, LineError> {
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
        self.store_with(imported, |events| store.ingest(events))
    }

    /// [`Batch::store`] through `ingest`. Each refusal drops at least one line, so the loop
    /// ends; a refusal naming no line of the batch is [`ImportError::UnheldOwnUnmatched`].
    fn store_with(
        &mut self,
        imported: &mut ImportReport,
        mut ingest: impl FnMut(&[Foreign]) -> Result<Ingest, StoreError>,
    ) -> Result<(), ImportError> {
        while !self.events.is_empty() {
            match ingest(&self.events).map_err(ImportError::Store)? {
                Ingest::Done { appended, skipped } => {
                    imported.imported = imported.imported.saturating_add(count(appended));
                    imported.present = imported.present.saturating_add(count(skipped));
                    break;
                }
                Ingest::UnheldOwn(id) => {
                    let before = self.events.len();
                    let mut kept = Self::default();
                    for (number, event) in self.numbers.drain(..).zip(self.events.drain(..)) {
                        if event.id == id {
                            imported.skip(number, LineError::UnheldOwnIdentity);
                        } else {
                            kept.push(number, event);
                        }
                    }
                    *self = kept;
                    if self.events.len() == before {
                        return Err(ImportError::UnheldOwnUnmatched { id });
                    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::parse_event_ref;

    fn foreign(id: &str) -> Foreign {
        Foreign {
            id: parse_event_ref(id).expect("an event ref"),
            recorded_at_millis: 0,
            event: NewEvent::new("weighin.item_raised", b"{}".to_vec()).expect("an event"),
        }
    }

    #[test]
    fn a_refused_identity_no_line_names_is_an_error_not_a_retry() {
        let mut batch = Batch::default();
        batch.push(1, foreign("0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:1"));
        let elsewhere = parse_event_ref("0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:9").expect("a ref");
        let mut report = ImportReport::default();
        let mut calls = 0_u32;
        let outcome = batch.store_with(&mut report, |_| {
            calls += 1;
            if calls > 3 {
                return Ok(Ingest::Done {
                    appended: 0,
                    skipped: 0,
                });
            }
            Ok(Ingest::UnheldOwn(elsewhere))
        });
        assert!(
            matches!(outcome, Err(ImportError::UnheldOwnUnmatched { id }) if id == elsewhere),
            "{outcome:?}"
        );
        assert_eq!(calls, 1, "not retried");
    }

    #[test]
    fn a_refused_line_is_dropped_and_the_rest_stored() {
        let mut batch = Batch::default();
        batch.push(1, foreign("0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:1"));
        batch.push(2, foreign("0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:2"));
        let refused = parse_event_ref("0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:2").expect("a ref");
        let mut report = ImportReport::default();
        let mut sizes = Vec::new();
        batch
            .store_with(&mut report, |events| {
                sizes.push(events.len());
                Ok(if events.iter().any(|e| e.id == refused) {
                    Ingest::UnheldOwn(refused)
                } else {
                    Ingest::Done {
                        appended: events.len(),
                        skipped: 0,
                    }
                })
            })
            .expect("stored");
        assert_eq!(sizes, [2, 1]);
        assert_eq!((report.imported, report.malformed), (1, 1));
        assert_eq!(report.problems.first().map(|p| p.line), Some(2));
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    struct Raised {
        item: String,
    }

    impl Payload for Raised {
        const TYPE: &'static str = "weighin.item_raised";
        const VERSION: u32 = 1;
        const CLASS: Class = Class::Durable;
    }

    fn author() -> Author {
        Author {
            actor: crate::envelope::Actor::Agent,
            trust: crate::trust::Trust::asserted(),
            writer: crate::envelope::Writer::new("weighin", "2.0.0").expect("valid"),
        }
    }

    fn ts() -> Timestamp {
        Timestamp::parse("2026-11-10T10:00:00Z").expect("valid")
    }

    /// A store at `path`, giving way on `clock` rather than sleeping (TST-10).
    fn store_on(path: &Path, clock: &std::sync::Arc<crate::store::FakeClock>) -> Store {
        Store::open(path, crate::store::Wait::Command)
            .expect("opens")
            .giving_way_on(std::sync::Arc::clone(clock))
    }

    /// A journal of `count` lines, exported from a store of its own in `dir`.
    fn journal_of(dir: &Path, count: usize) -> Vec<u8> {
        let clock = std::sync::Arc::new(crate::store::FakeClock::default());
        let source = store_on(&dir.join("source.sqlite3"), &clock);
        let events: Vec<NewEvent> = (0..count)
            .map(|n| {
                let item = format!("wi-{n:06}");
                Envelope::new(ts(), author(), Raised { item })
                    .to_event([])
                    .expect("an event")
            })
            .collect();
        for chunk in events.chunks(150) {
            source.append(chunk.to_vec(), None).expect("appended");
        }
        let limit = std::num::NonZeroUsize::new(count).expect("non-zero");
        source
            .read(&crate::store::Filter::all(), 0, limit)
            .expect("read")
            .iter()
            .map(|stored| super::super::encode_line(stored).expect("a line"))
            .collect::<Vec<_>>()
            .concat()
    }

    // A batch stays within happenstance's limit, checked at compile time.
    const _: () =
        assert!(BATCH <= happenstance_sqlite::event_store::SqliteEventStore::MAX_EVENTS_PER_BATCH);

    #[test]
    fn an_import_gives_way_between_its_batches() {
        let dir = tempfile::tempdir().expect("tempdir");
        let journal = journal_of(dir.path(), 300);
        let clock = std::sync::Arc::new(crate::store::FakeClock::default());
        let target = store_on(&dir.path().join("events.sqlite3"), &clock);
        let recording = Recording {
            source: ImportSource::Path("journal.jsonl".to_owned()),
            ts: ts(),
            author: author(),
        };
        let report = import(&target, io::Cursor::new(journal), recording).expect("imported");
        assert_eq!((report.read, report.imported), (300, 300));
        // Batches of 128, 128 and 44, then the record: a pause before each write after the first.
        assert_eq!(clock.slept(), [crate::store::PAUSE; 3]);
    }
}
