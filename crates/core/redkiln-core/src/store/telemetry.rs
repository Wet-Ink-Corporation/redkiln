//! Telemetry (SPEC EV-14, ST-09): written without waiting, dropped and counted on a busy lock,
//! kept in a side store for [`TELEMETRY_WINDOW`], and never read past it.
//!
//! Telemetry lives beside the log, in `<store>.telemetry`: a Core store of its own, opened and
//! created by the same open path (TRUNCATE, stamp 1000, its own open lock), so the durable log
//! never holds a telemetry row, exports read nothing to filter (JR-04), and telemetry writers
//! never take the durable writers' lock. Rows past the window are deleted there, never the file
//! (`retention`).
//!
//! A durable append waits up to the open's busy timeout on happenstance's connection. A
//! telemetry event goes through the side store's connection, opened on the first telemetry write
//! with no busy wait at all: the open locks, the journal switch and the append each try once.
//! Before it opens, the store's own file is opened in its existing-store-only mode and its
//! identity checked, so a store file that vanished or was replaced gets no telemetry beside it,
//! and nothing is created or set aside. Any busy answer on the way (an open lock held, SQLite's
//! `SQLITE_BUSY` or `SQLITE_LOCKED` when beginning, writing or committing) drops the event and
//! counts it; nothing retries. A busy `COMMIT` leaves SQLite's transaction open, and rusqlite
//! rolls it back when happenstance's transaction is dropped, so a dropped event is never
//! written.

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use happenstance_core::{AppendError, SendEventStore, StoreId};
use happenstance_sqlite::event_store::{SqliteEventStore, SqliteEventStoreError};
use happenstance_sqlite::rusqlite::{self, ErrorCode};

use super::{
    Filter, NewEvent, OpenError, Page, Store, StoreError, Stored, open, read_page,
    refuse_inside_runtime, retention, sibling,
};

/// How long telemetry is kept, and read (EV-14's default). SPEC 1.0 gives suite.toml no key for
/// it (ST-12), so it is fixed.
pub const TELEMETRY_WINDOW: Duration = Duration::from_hours(30 * 24);

/// The side store's suffix: `<store>.telemetry`.
const SIDE_SUFFIX: &str = ".telemetry";

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
    /// Writes a telemetry event to the side store without waiting: when another writer holds
    /// the lock, the event is dropped and counted (ST-09). Rows past [`TELEMETRY_WINDOW`] are
    /// deleted first, when there are a day's worth of them.
    ///
    /// # Errors
    /// [`StoreError::InsideRuntime`]; [`StoreError::Open`] when the store's file or the side
    /// store cannot be opened for a reason other than a busy lock, [`OpenError::NoStore`] among
    /// them when the store's file is gone; [`StoreError::IdentityChanged`] when the file at the
    /// store's path is no longer the store opened; [`StoreError::Read`] or
    /// [`StoreError::Prune`] when the window cannot be kept; [`StoreError::Append`] when
    /// happenstance refuses the append for a reason other than a busy lock.
    pub fn record_telemetry(&mut self, event: TelemetryEvent) -> Result<Recorded, StoreError> {
        refuse_inside_runtime()?;
        let side = sibling(&self.path, SIDE_SUFFIX);
        let handle = telemetry_handle(
            &mut self.telemetry,
            &self.path,
            &side,
            self.inner.store_id(),
        )?;
        let outcome = match handle {
            None => None,
            Some(handle) => {
                retention::prune(&self.runtime, handle, &side)?;
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

    /// At most `page.limit` telemetry events matching `filter` after `page.after`, in the side
    /// store's order, recorded no earlier than [`TELEMETRY_WINDOW`] before `now` (EV-14):
    /// older rows not yet deleted are passed over, never returned. `now` is the caller's clock
    /// (`SystemTime::now()`); the library reads none. A store with no side store yet reads none,
    /// and none is created. The read waits up to the open's busy timeout.
    ///
    /// # Errors
    /// [`StoreError::InsideRuntime`]; [`StoreError::Open`] when the side store cannot be opened;
    /// [`StoreError::Query`] for a filter happenstance cannot query; [`StoreError::Read`] when
    /// happenstance cannot read it.
    pub fn read_telemetry(
        &self,
        filter: &Filter,
        page: Page,
        now: SystemTime,
    ) -> Result<Vec<Stored>, StoreError> {
        refuse_inside_runtime()?;
        let side = sibling(&self.path, SIDE_SUFFIX);
        let handle = match open::open(&side, self.wait, open::Mode::Existing) {
            Ok(handle) => handle,
            Err(OpenError::NoStore) => return Ok(Vec::new()),
            Err(source) => return Err(StoreError::Open { path: side, source }),
        };
        let cutoff = window_start(now);
        let limit = page.limit.get();
        let mut kept = Vec::new();
        let mut next = page;
        // Each page moves past its last row, so this ends at the side store's head.
        loop {
            let rows = read_page(&self.runtime, &handle, filter, next)?;
            let full = rows.len() == limit;
            let Some(last) = rows.last().map(|stored| stored.position) else {
                break;
            };
            kept.extend(
                rows.into_iter()
                    .filter(|stored| stored.recorded_at_millis >= cutoff),
            );
            if kept.len() >= limit || !full {
                break;
            }
            next.after = last;
        }
        kept.truncate(limit);
        Ok(kept)
    }
}

/// The earliest `recorded_at` a reader at `now` sees, in milliseconds since the Unix epoch,
/// saturating at the ends of `i64`.
fn window_start(now: SystemTime) -> i64 {
    let Some(start) = now.checked_sub(TELEMETRY_WINDOW) else {
        return i64::MIN;
    };
    match start.duration_since(UNIX_EPOCH) {
        Ok(after) => i64::try_from(after.as_millis()).unwrap_or(i64::MAX),
        Err(before) => i64::try_from(before.duration().as_millis())
            .map_or(i64::MIN, |millis| 0_i64.saturating_sub(millis)),
    }
}

/// The side store's telemetry connection in `slot`, opened at `side` now if it is not yet;
/// `None` when an open met a busy lock. Before the first open, the store's own file at `main`
/// must still be the store opened (`opened`): one that vanished is [`OpenError::NoStore`], and
/// one with another identity is refused, with nothing created beside either.
fn telemetry_handle<'a>(
    slot: &'a mut Option<SqliteEventStore>,
    main: &Path,
    side: &Path,
    opened: StoreId,
) -> Result<Option<&'a SqliteEventStore>, StoreError> {
    if slot.is_some() {
        return Ok(slot.as_ref());
    }
    match open::open(main, Duration::ZERO, open::Mode::Existing) {
        Ok(handle) if handle.store_id() == opened => {}
        Ok(handle) => {
            return Err(StoreError::IdentityChanged {
                opened,
                found: handle.store_id(),
            });
        }
        Err(refused) if busy_open(&refused) => return Ok(None),
        Err(source) => {
            return Err(StoreError::Open {
                path: main.to_path_buf(),
                source,
            });
        }
    }
    match open::open(side, Duration::ZERO, open::Mode::Create) {
        Ok(handle) => *slot = Some(handle),
        Err(refused) if busy_open(&refused) => return Ok(None),
        Err(source) => {
            return Err(StoreError::Open {
                path: side.to_path_buf(),
                source,
            });
        }
    }
    Ok(slot.as_ref())
}

/// SQLite's two busy codes.
pub(super) fn busy_code(error: &rusqlite::Error) -> bool {
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
pub(super) fn busy_open(refused: &OpenError) -> bool {
    match refused {
        OpenError::LockBusy => true,
        OpenError::Sqlite { source, .. } => busy_code(source),
        OpenError::Store(SqliteEventStoreError::Sqlite(error)) => busy_code(error),
        _ => false,
    }
}
