//! EV-14's retention window in the telemetry side store: rows recorded more than
//! [`TELEMETRY_WINDOW`] before the newest are deleted, row by row, never the file (ST-05).
//!
//! The check runs before each telemetry append, on the telemetry connection: the oldest and the
//! newest rows' `recorded_at`, happenstance's own clock, so Core reads none. Only when the oldest
//! is past the window by more than [`SLACK`] are rows deleted, so the delete runs about once a
//! day of use rather than on every write; readers never see the rows in the slack
//! ([`super::Store::read_telemetry`] filters by the window).
//!
//! The delete is Core's own SQL on happenstance's tables (its schema is copied into every store
//! Core creates, and happenstance's `AUTOINCREMENT` keeps a deleted position from being reused),
//! on a connection of its own with no busy wait: `event_tag` rows first (they reference `event`),
//! `tag_cardinality` counts lowered with them, then the `event` rows, in one `BEGIN IMMEDIATE`.
//! A busy answer anywhere skips the prune; the next write tries again.

use std::path::Path;
use std::time::Duration;

use happenstance_core::{Query, ReadOptions, SendEventStore, collect};
use happenstance_sqlite::event_store::{SqliteEventStore, SqliteEventStoreError};
use happenstance_sqlite::rusqlite::{self, Connection};

use super::telemetry::{TELEMETRY_WINDOW, busy_code, busy_open};
use super::{Driver, OpenError, Step, StoreError, open};

/// How far past the window the oldest row may be before rows are deleted.
const SLACK: Duration = Duration::from_hours(24);

/// Lowers each tag's count by its rows recorded before the cut-off (`?1`, milliseconds), before
/// the tag rows that hold them go.
const LOWER_COUNTS: &str = "UPDATE tag_cardinality SET events = events - (
         SELECT count(*) FROM event_tag t JOIN event e ON e.position = t.position
         WHERE t.tag = tag_cardinality.tag AND e.recorded_at < ?1)
     WHERE tag IN (
         SELECT t.tag FROM event_tag t JOIN event e ON e.position = t.position
         WHERE e.recorded_at < ?1)";

/// A tag no row carries has no count: happenstance reads a missing count as matching nothing.
const DROP_EMPTY_COUNTS: &str = "DELETE FROM tag_cardinality WHERE events <= 0";

/// The tag rows of the rows recorded before the cut-off (`?1`).
const DELETE_TAGS: &str =
    "DELETE FROM event_tag WHERE position IN (SELECT position FROM event WHERE recorded_at < ?1)";

/// The rows recorded before the cut-off (`?1`).
const DELETE_EVENTS: &str = "DELETE FROM event WHERE recorded_at < ?1";

/// Deletes the side store's rows past the window when the oldest is past it by more than
/// [`SLACK`]; does nothing on a busy answer.
pub(super) fn prune(
    runtime: &Driver,
    handle: &SqliteEventStore,
    side: &Path,
) -> Result<(), StoreError> {
    let Some((oldest, newest)) = ends(runtime, handle)? else {
        return Ok(());
    };
    let window = millis(TELEMETRY_WINDOW);
    if oldest >= newest.saturating_sub(window).saturating_sub(millis(SLACK)) {
        return Ok(());
    }
    let cutoff = newest.saturating_sub(window);
    match delete_before(side, cutoff) {
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

/// Deletes every row recorded before `cutoff`, with its tag rows and counts.
fn delete_before(side: &Path, cutoff: i64) -> Result<(), OpenError> {
    let conn = open::raw_connection(side)?;
    conn.execute_batch("BEGIN IMMEDIATE")
        .map_err(|source| OpenError::Sqlite {
            step: Step::Begin,
            source,
        })?;
    let body = run(&conn, cutoff).map_err(|source| OpenError::Sqlite {
        step: Step::Prune,
        source,
    });
    open::finish_transaction(&conn, body)
}

fn run(conn: &Connection, cutoff: i64) -> rusqlite::Result<()> {
    conn.execute(LOWER_COUNTS, [cutoff])?;
    conn.execute(DROP_EMPTY_COUNTS, [])?;
    conn.execute(DELETE_TAGS, [cutoff])?;
    conn.execute(DELETE_EVENTS, [cutoff])?;
    Ok(())
}

/// `duration` in whole milliseconds, saturating; every duration here is a constant of days.
fn millis(duration: Duration) -> i64 {
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}
