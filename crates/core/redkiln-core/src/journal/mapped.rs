//! The mapped import (SPEC ST-15): records of an old plugin source that the plugin has mapped to
//! envelopes and identities, imported through the journal import's own path (JR-10 to JR-13).
//!
//! Core knows nothing of the old source: the plugin reads it, builds each envelope with its own
//! writer and chooses the identity (the old store's, ST-15, or its own rule, JR-07). Each event is
//! then encoded as the journal line it would be and read back exactly as a journal line is, so
//! trust becomes `asserted` with the writer's level in `claimed` (JR-12, TR-06), tags follow
//! JR-02 and every line limit holds. This entry gives a plugin no power that writing a journal
//! file does not already give it.

use std::time::Instant;

use happenstance_core::EventId;

use super::import::{ImportError, ImportReport, Importer, Recording, foreign};
use super::line::{LineError, encode_parts, read_line};
use crate::store::{Foreign, NewEvent, Store};

/// One old record as its plugin mapped it for the import (ST-15).
#[derive(Debug)]
pub enum Mapped {
    /// An envelope to store under `id`: the old store's identity (ST-15), or an identity the
    /// plugin's own rule gives (JR-07).
    Event {
        /// The identity it is stored under (ID-02).
        id: EventId,
        /// The envelope, built by the plugin's writer (`Envelope::to_event`).
        event: NewEvent,
    },
    /// A record the plugin could not map: read, and skipped as malformed (JR-10).
    Unmapped,
}

/// How a mapped import ended.
#[derive(Debug)]
#[must_use]
pub enum MappedImport {
    /// Every record was read and the import recorded (JR-13).
    Completed(ImportReport),
    /// `deadline` passed before a batch was stored: the batches stored stay, nothing is
    /// recorded, and the next import appends only the rest (JR-11).
    Stopped(ImportReport),
}

/// Imports `records` mapped from an old source into `store`, and records the import (ST-15).
///
/// Each record is numbered by its plugin (a ledger position, a journal line number, named in the
/// report's problems). Records are stored in batches of 128 (JR-10 to JR-12); then, unless
/// `deadline` passed before a batch, one [`ImportCompleted`](super::ImportCompleted) is appended
/// as `recording` says (JR-13).
///
/// Each [`Mapped::Event`] is encoded as the journal line it would be under its `id` and read
/// back exactly as a journal line is: trust becomes `asserted` with the writer's level in
/// `claimed` (JR-12, TR-06), tags follow JR-02, and the recorded time kept is the envelope's
/// `ts`. A record that fails there, or [`Mapped::Unmapped`], is read and skipped as malformed.
///
/// # Errors
/// As [`super::import`]: [`ImportError::Store`], [`ImportError::Record`] and
/// [`ImportError::UnheldOwnUnmatched`].
pub fn import_mapped(
    store: &Store,
    records: impl IntoIterator<Item = (u64, Mapped)>,
    recording: Recording,
    deadline: Option<Instant>,
) -> Result<MappedImport, ImportError> {
    let passed = || deadline.is_some_and(|deadline| Instant::now() >= deadline);
    let mut importer = Importer::default();
    for (number, record) in records {
        importer.take(number, as_foreign(record));
        if importer.full() {
            if passed() {
                return Ok(MappedImport::Stopped(importer.report));
            }
            importer.flush(store)?;
        }
    }
    if importer.pending() && passed() {
        return Ok(MappedImport::Stopped(importer.report));
    }
    importer
        .finish(store, recording)
        .map(MappedImport::Completed)
}

/// `record` as the store takes it, through the journal line it would be.
fn as_foreign(record: Mapped) -> Result<Foreign, LineError> {
    let Mapped::Event { id, event } = record else {
        return Err(LineError::Unmapped);
    };
    let (event_type, data, tags) = event.parts();
    let (_, line) = encode_parts(id, event_type, data, tags)?;
    let text = line.strip_suffix(b"\n").unwrap_or(&line);
    read_line(text).and_then(foreign)
}
