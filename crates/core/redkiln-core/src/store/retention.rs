//! EV-14's retention window in the telemetry side store: rows recorded outside it are deleted,
//! row by row, never the file (ST-05).
//!
//! The window is the caller's clock: from [`TELEMETRY_WINDOW`] before `now` to [`SLACK`] after
//! it ([`Window::at`]). A row dated past its end (a clock that ran ahead) is outside the window
//! too, so no row's own date can stop the prune or move the window. The library reads no clock:
//! `now` is the caller's, on writes as on reads.
//!
//! The check runs before each telemetry append, on the telemetry connection: the first and the
//! last rows' `recorded_at`. Rows are deleted when the first is older than the window's start by
//! more than [`SLACK`], or either is past its end, so the delete runs about once a day of use
//! rather than on every write. A stray row elsewhere goes at the next delete; readers never see
//! a row outside the window ([`super::Store::read_telemetry`] filters by it).
//!
//! The delete is Core's own SQL on happenstance's tables (its schema is copied into every store
//! Core creates, and happenstance's `AUTOINCREMENT` keeps a deleted position from being reused),
//! on a connection of its own with no busy wait that never creates the file: `tag_cardinality`
//! counts lowered, the `event_tag` rows (they reference `event`), then the `event` rows, in one
//! `BEGIN IMMEDIATE` that first checks the file is still the side store the handle opened. A busy
//! answer anywhere skips the prune; the next write tries again.

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use happenstance_core::{Query, ReadOptions, SendEventStore, StoreId, collect};
use happenstance_sqlite::event_store::{SqliteEventStore, SqliteEventStoreError};
use happenstance_sqlite::rusqlite::{self, Connection};

use super::telemetry::{TELEMETRY_WINDOW, busy_code, busy_open};
use super::{Driver, OpenError, Step, StoreError, open};

/// How far past the window's start the first row may be before rows are deleted, and how far
/// past `now` a row may be dated (clock skew between processes) and still be read.
const SLACK: Duration = Duration::from_hours(24);

/// Rows outside the window: `recorded_at` before `?1` or after `?2` (milliseconds).
const OUTSIDE: &str = "(recorded_at < ?1 OR recorded_at > ?2)";

/// The side store's identity, as happenstance keeps it.
const STORE_ID: &str = "SELECT v FROM store_meta WHERE k = 'store_id'";

/// The `recorded_at` range a writer or reader at some `now` keeps, in milliseconds since the
/// Unix epoch, both ends included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Window {
    /// [`TELEMETRY_WINDOW`] before `now`.
    pub(super) start: i64,
    /// [`SLACK`] after `now`.
    pub(super) end: i64,
}

impl Window {
    /// The window of a writer or reader whose clock says `now`.
    pub(super) fn at(now: SystemTime) -> Self {
        let now = millis_since_epoch(now);
        Self {
            start: now.saturating_sub(millis(TELEMETRY_WINDOW)),
            end: now.saturating_add(millis(SLACK)),
        }
    }

    /// Whether a row recorded at `recorded_at` is in the window.
    pub(super) const fn holds(self, recorded_at: i64) -> bool {
        self.start <= recorded_at && recorded_at <= self.end
    }
}

/// Deletes the side store's rows outside `window` when its first row is older than the window by
/// more than [`SLACK`], or its first or last row is dated past it; does nothing on a busy answer.
pub(super) fn prune(
    runtime: &Driver,
    handle: &SqliteEventStore,
    side: &Path,
    window: Window,
) -> Result<(), StoreError> {
    let Some((first, last)) = ends(runtime, handle)? else {
        return Ok(());
    };
    let stale = first < window.start.saturating_sub(millis(SLACK));
    let ahead = first > window.end || last > window.end;
    if !stale && !ahead {
        return Ok(());
    }
    match delete_outside(side, handle.store_id(), window) {
        Ok(()) => Ok(()),
        Err(busy) if busy_open(&busy) => Ok(()),
        Err(source) => Err(StoreError::Prune {
            path: side.to_path_buf(),
            source,
        }),
    }
}

/// The `recorded_at` of the side store's first and last rows; `None` when it is empty or a read
/// met a busy lock.
fn ends(runtime: &Driver, handle: &SqliteEventStore) -> Result<Option<(i64, i64)>, StoreError> {
    match runtime.block_on(read_ends(handle))? {
        Ok(ends) => Ok(ends),
        Err(SqliteEventStoreError::Sqlite(error)) if busy_code(&error) => Ok(None),
        Err(other) => Err(StoreError::Read(other)),
    }
}

/// [`ends`]: the first row, and the row at the head.
async fn read_ends(handle: &SqliteEventStore) -> Result<Option<(i64, i64)>, SqliteEventStoreError> {
    let Some(head) = handle.head().await? else {
        return Ok(None);
    };
    let first = collect(handle.read(&Query::all(), ReadOptions::new().limit(1))).await?;
    let last = collect(handle.read(&Query::all(), ReadOptions::new().from(head).limit(1))).await?;
    Ok(match (first.first(), last.first()) {
        (Some(first), Some(last)) => {
            Some((first.recorded_at.as_millis(), last.recorded_at.as_millis()))
        }
        _ => None,
    })
}

/// Deletes every row outside `window`, with its tag rows and counts, when the file is still the
/// store `opened`; a file with another identity is left as it is.
fn delete_outside(side: &Path, opened: StoreId, window: Window) -> Result<(), OpenError> {
    let conn = open::raw_connection(side)?;
    conn.execute_batch("BEGIN IMMEDIATE")
        .map_err(|source| OpenError::Sqlite {
            step: Step::Begin,
            source,
        })?;
    let body = run(&conn, opened, window).map_err(|source| OpenError::Sqlite {
        step: Step::Prune,
        source,
    });
    open::finish_transaction(&conn, body)
}

fn run(conn: &Connection, opened: StoreId, window: Window) -> rusqlite::Result<()> {
    let found: Vec<u8> = conn.query_row(STORE_ID, [], |row| row.get(0))?;
    if found.as_slice() != opened.to_bytes().as_slice() {
        return Ok(());
    }
    let bounds = [window.start, window.end];
    conn.execute(
        &format!(
            "UPDATE tag_cardinality SET events = events - (
                 SELECT count(*) FROM event_tag t
                 WHERE t.tag = tag_cardinality.tag
                   AND t.position IN (SELECT position FROM event WHERE {OUTSIDE}))
             WHERE tag IN (
                 SELECT t.tag FROM event_tag t
                 WHERE t.position IN (SELECT position FROM event WHERE {OUTSIDE}))"
        ),
        bounds,
    )?;
    // A tag no row carries has no count: happenstance reads a missing count as matching nothing.
    conn.execute("DELETE FROM tag_cardinality WHERE events <= 0", [])?;
    conn.execute(
        &format!(
            "DELETE FROM event_tag WHERE position IN (SELECT position FROM event WHERE {OUTSIDE})"
        ),
        bounds,
    )?;
    conn.execute(&format!("DELETE FROM event WHERE {OUTSIDE}"), bounds)?;
    Ok(())
}

/// `time` in milliseconds since the Unix epoch, negative before it, saturating at `i64`'s ends.
fn millis_since_epoch(time: SystemTime) -> i64 {
    match time.duration_since(UNIX_EPOCH) {
        Ok(after) => millis(after),
        Err(before) => 0_i64.saturating_sub(millis(before.duration())),
    }
}

/// `duration` in whole milliseconds, saturating at `i64::MAX` (some 292 million years).
fn millis(duration: Duration) -> i64 {
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_window_is_thirty_days_back_and_a_day_ahead_of_now() {
        let now = UNIX_EPOCH + Duration::from_hours(100 * 24);
        let window = Window::at(now);
        let day = 24 * 60 * 60 * 1000;
        assert_eq!(window.start, 70 * day);
        assert_eq!(window.end, 101 * day);
        assert!(
            window.holds(70 * day) && window.holds(101 * day),
            "ends included"
        );
        assert!(!window.holds(70 * day - 1) && !window.holds(101 * day + 1));
    }

    #[test]
    fn times_before_the_epoch_and_far_ahead_saturate() {
        let before = UNIX_EPOCH
            .checked_sub(Duration::from_secs(1))
            .expect("a second before");
        assert_eq!(millis_since_epoch(before), -1000);
        assert_eq!(millis(Duration::MAX), i64::MAX);
    }
}
