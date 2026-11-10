//! happenstance's own `store_meta` rows, read before Core writes to a store: its schema version
//! (ST-08) and its identity (ID-01), so that a store Core will refuse is never re-minted or
//! migrated first.

use happenstance_sqlite::event_store::{SCHEMA_VERSION, SqliteEventStoreError};
use happenstance_sqlite::rusqlite::{self, Connection, OptionalExtension};

use super::{OpenError, Step};

/// The `store_meta` key of happenstance's own schema version (ST-08), never `user_version`.
const HAPPENSTANCE_SCHEMA_KEY: &str = "schema_version";

/// The `store_meta` key of happenstance's store identity (ID-01).
pub(super) const STORE_ID_KEY: &str = "store_id";

/// The identity's length in bytes: a `StoreId` is sixteen.
const IDENTITY_LEN: i64 = 16;

/// What happenstance's `store_meta` holds: its schema version and the length of its identity,
/// each `None` when the row is missing.
struct Meta {
    version: Option<i64>,
    identity_len: Option<i64>,
}

impl Meta {
    /// At a happenstance schema older than this build's, with a sixteen-byte identity to keep:
    /// `open_on` would mint one silently for a store that has none.
    fn needs_migration(&self) -> bool {
        self.identity_len == Some(IDENTITY_LEN)
            && self
                .version
                .is_some_and(|version| (1..i64::from(SCHEMA_VERSION)).contains(&version))
    }
}

/// `store_meta`'s rows, or `None` when the file has no `store_meta` table.
fn read_meta(conn: &Connection) -> rusqlite::Result<Option<Meta>> {
    let has_meta: bool = conn.query_row(
        "SELECT count(*) > 0 FROM main.sqlite_master WHERE type = 'table' AND name = 'store_meta'",
        [],
        |row| row.get(0),
    )?;
    if !has_meta {
        return Ok(None);
    }
    let version = conn
        .query_row(
            "SELECT CAST(v AS INTEGER) FROM store_meta WHERE k = ?1",
            [HAPPENSTANCE_SCHEMA_KEY],
            |row| row.get(0),
        )
        .optional()?;
    let identity_len = conn
        .query_row(
            "SELECT length(CAST(v AS BLOB)) FROM store_meta WHERE k = ?1",
            [STORE_ID_KEY],
            |row| row.get(0),
        )
        .optional()?;
    Ok(Some(Meta {
        version,
        identity_len,
    }))
}

/// Whether the file holds happenstance's schema at a version older than this build's (ST-08),
/// with its identity, which only `open_on` migrates: one write transaction (`BEGIN IMMEDIATE`),
/// keeping the journal mode and `user_version`. Every other file goes to `new`, which only
/// reads, and whose refusals (no schema, no version row, no identity, a newer version) stand:
/// `open_on` would give a file with no schema happenstance's tables, and one with no identity
/// a new one.
pub(crate) fn needs_migration(conn: &Connection) -> rusqlite::Result<bool> {
    Ok(read_meta(conn)?.is_some_and(|meta| meta.needs_migration()))
}

/// The refusals Core decides from `store_meta` before it writes to a store: a newer
/// happenstance schema, no identity row (never minted in its place: that would re-issue the
/// identities of events already written, ID-01) and an identity that is not sixteen bytes.
/// `Ok(true)` when the store needs happenstance's migration. A file with no `store_meta` is
/// left to happenstance's own refusal.
pub(super) fn check_happenstance(conn: &Connection) -> Result<bool, OpenError> {
    let read = read_meta(conn).map_err(|source| OpenError::Sqlite {
        step: Step::Inspect,
        source,
    })?;
    let Some(meta) = read else {
        return Ok(false);
    };
    if let Some(found) = meta.version.filter(|v| *v > i64::from(SCHEMA_VERSION)) {
        // A version past what a `u32` holds is past any happenstance's; shown as the largest.
        return Err(OpenError::Store(
            SqliteEventStoreError::UnsupportedSchemaVersion {
                found: u32::try_from(found).unwrap_or(u32::MAX),
                supported: SCHEMA_VERSION,
            },
        ));
    }
    match meta.identity_len {
        None => Err(OpenError::MissingIdentity),
        Some(IDENTITY_LEN) => Ok(meta.needs_migration()),
        Some(len) => Err(OpenError::Store(SqliteEventStoreError::MalformedIdentity {
            // SQLite's `length` is never negative; a blob past `usize` cannot be in memory.
            len: usize::try_from(len).unwrap_or(usize::MAX),
        })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_store_needs_migration_only_below_this_happenstance_and_with_its_identity() {
        let conn = Connection::open_in_memory().expect("memory database");
        conn.execute_batch(
            "CREATE TABLE store_meta (k TEXT PRIMARY KEY, v BLOB NOT NULL);
             INSERT INTO store_meta VALUES ('schema_version', 1);",
        )
        .expect("a schema-1 store_meta with no identity");
        assert!(
            !needs_migration(&conn).expect("read"),
            "no identity, no migration"
        );
        conn.execute(
            "INSERT INTO store_meta VALUES ('store_id', randomblob(15))",
            [],
        )
        .expect("a short identity");
        assert!(
            !needs_migration(&conn).expect("read"),
            "a short identity is not kept"
        );
        conn.execute(
            "UPDATE store_meta SET v = randomblob(16) WHERE k = 'store_id'",
            [],
        )
        .expect("an identity");
        assert!(needs_migration(&conn).expect("read"));
        conn.execute(
            "UPDATE store_meta SET v = ?1 WHERE k = 'schema_version'",
            [SCHEMA_VERSION],
        )
        .expect("this happenstance");
        assert!(!needs_migration(&conn).expect("read"));
    }

    #[test]
    fn a_short_identity_is_refused_as_malformed() {
        let conn = Connection::open_in_memory().expect("memory database");
        conn.execute_batch(
            "CREATE TABLE store_meta (k TEXT PRIMARY KEY, v BLOB NOT NULL);
             INSERT INTO store_meta VALUES ('schema_version', 2);
             INSERT INTO store_meta VALUES ('store_id', randomblob(15));",
        )
        .expect("a store_meta with a short identity");
        let err = check_happenstance(&conn).expect_err("refused");
        assert!(
            matches!(
                err,
                OpenError::Store(SqliteEventStoreError::MalformedIdentity { len: 15 })
            ),
            "{err:?}"
        );
    }
}
