//! Telemetry writes (SPEC EV-14, ST-09): never wait on a busy lock; drop and count instead.
//!
//! A durable append waits up to the open's busy timeout on happenstance's connection. A
//! telemetry event goes through a second connection to the same file, opened on the first
//! telemetry write by Core's own open path with no busy wait at all: the open lock, the journal
//! switch and the append each try once. Any busy answer on the way (the open lock held, SQLite's
//! `SQLITE_BUSY` or `SQLITE_LOCKED` when beginning, writing or committing) drops the event and
//! counts it; nothing retries. A busy `COMMIT` leaves SQLite's transaction open, and rusqlite
//! rolls it back when happenstance's transaction is dropped, so a dropped event is never
//! written.

use std::path::Path;
use std::time::Duration;

use happenstance_core::{AppendError, SendEventStore, StoreId};
use happenstance_sqlite::event_store::{SqliteEventStore, SqliteEventStoreError};
use happenstance_sqlite::rusqlite::{self, ErrorCode};

use super::{NewEvent, OpenError, Store, StoreError, open, refuse_inside_runtime};

/// A telemetry event, built by [`crate::envelope::Envelope::to_telemetry`] from a type declared
/// [`crate::envelope::Class::Telemetry`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TelemetryEvent(NewEvent);

impl TelemetryEvent {
    pub(crate) const fn new(event: NewEvent) -> Self {
        Self(event)
    }
}

/// The outcome of a telemetry write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recorded {
    /// Written at this position.
    At(u64),
    /// Not written: the lock was busy. Counted in [`Store::telemetry_dropped`], never retried.
    Dropped,
}

impl Store {
    /// Writes a telemetry event without waiting: when another writer holds the lock, the event
    /// is dropped and counted (ST-09).
    ///
    /// # Errors
    /// [`StoreError::InsideRuntime`]; [`StoreError::Open`] when the telemetry connection cannot
    /// be opened for a reason other than a busy lock; [`StoreError::IdentityChanged`] when the
    /// file at the store's path is no longer the store opened; [`StoreError::Append`] when
    /// happenstance refuses the append for a reason other than a busy lock.
    pub fn record_telemetry(&mut self, event: TelemetryEvent) -> Result<Recorded, StoreError> {
        refuse_inside_runtime()?;
        let handle = telemetry_handle(&mut self.telemetry, &self.path, self.inner.store_id())?;
        let outcome = match handle {
            None => None,
            Some(handle) => {
                let events = [event.0.into_event()];
                match self.runtime.block_on(handle.append(&events, None))? {
                    Ok(position) => Some(u64::from(position)),
                    Err(refused) if busy_append(&refused) => None,
                    Err(other) => return Err(StoreError::Append(other)),
                }
            }
        };
        if let Some(position) = outcome {
            return Ok(Recorded::At(position));
        }
        self.dropped = self.dropped.saturating_add(1);
        Ok(Recorded::Dropped)
    }

    /// How many telemetry events were dropped on a busy lock since this store was opened, for
    /// the process's report or the next durable write's context (ST-09).
    #[must_use]
    pub const fn telemetry_dropped(&self) -> u64 {
        self.dropped
    }
}

/// The telemetry connection in `slot`, opened at `path` now if it is not yet; `None` when the
/// open met a busy lock. A file whose identity is not `opened` is refused.
fn telemetry_handle<'a>(
    slot: &'a mut Option<SqliteEventStore>,
    path: &Path,
    opened: StoreId,
) -> Result<Option<&'a SqliteEventStore>, StoreError> {
    if slot.is_none() {
        match open::open(path, Duration::ZERO) {
            Ok(handle) if handle.store_id() == opened => *slot = Some(handle),
            Ok(handle) => {
                return Err(StoreError::IdentityChanged {
                    opened,
                    found: handle.store_id(),
                });
            }
            Err(refused) if busy_open(&refused) => return Ok(None),
            Err(source) => {
                return Err(StoreError::Open {
                    path: path.to_path_buf(),
                    source,
                });
            }
        }
    }
    Ok(slot.as_ref())
}

/// SQLite's two busy codes.
fn busy_code(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(failure, _)
            if matches!(failure.code, ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked)
    )
}

/// An append refused on a busy lock: happenstance's own `Busy` (from `BEGIN IMMEDIATE`), or a
/// busy code later in the transaction, which rolled it back.
fn busy_append(refused: &AppendError<SqliteEventStoreError>) -> bool {
    match refused {
        AppendError::Busy(_) => true,
        AppendError::Store(SqliteEventStoreError::Sqlite(error)) => busy_code(error),
        _ => false,
    }
}

/// An open refused on a busy lock: the open lock held, or a busy code from SQLite.
fn busy_open(refused: &OpenError) -> bool {
    match refused {
        OpenError::LockBusy => true,
        OpenError::Sqlite { source, .. } => busy_code(source),
        OpenError::Store(SqliteEventStoreError::Sqlite(error)) => busy_code(error),
        _ => false,
    }
}
