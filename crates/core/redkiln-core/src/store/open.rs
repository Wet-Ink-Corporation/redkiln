//! Opening a store file: one connection under TRUNCATE, the schema copied in from a template
//! with its stamp, and the refusals of ST-07 decided before anything is written.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use happenstance_sqlite::event_store::SqliteEventStore;
use happenstance_sqlite::rusqlite::{self, Connection, ErrorCode};

use super::OpenError;
use super::header::{self, Header};

/// The journal mode every store connection runs under, as `PRAGMA journal_mode` reports it.
pub(crate) const JOURNAL_MODE: &str = "truncate";

/// The store schema this build reads and writes: SPEC 1.0, `major * 1000 + minor` (ST-06).
pub const STORE_SCHEMA: i64 = 1000;

/// Core's own key-value table, part of schema 1.0. It holds the store's canonical path (ID-01).
const CORE_META: &str =
    "CREATE TABLE core_meta (k TEXT PRIMARY KEY, v BLOB NOT NULL) WITHOUT ROWID";

/// The `core_meta` key holding the path the store was created or last re-minted at.
const PATH_KEY: &str = "path";

/// The pause between attempts to switch a WAL store to TRUNCATE while another connection holds it.
const RETRY_PAUSE: Duration = Duration::from_millis(20);

/// What a file holds, looked at through a connection that has written nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Contents {
    /// No schema objects: a new file or an empty database.
    Empty,
    /// happenstance's schema (its `store_meta` table).
    Store,
    /// Schema objects that are not happenstance's.
    Foreign,
}

/// Opens (creating when absent) the store at `path` and hands its connection to happenstance.
///
/// Until the file is known to be a store this build opens, or an empty database, nothing is
/// written to it or beside it, so a refused file is left exactly as it was.
pub(super) fn open(path: &Path, busy: Duration) -> Result<SqliteEventStore, OpenError> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(OpenError::CreateDir)?;
    }
    let found = header::read(path)?;
    if let Some(found) = &found {
        refuse(found)?;
    }
    // A `-wal` beside a file that is not in WAL is stale, and SQLite would delete it on connect,
    // which Cowork refuses. A stamped store in WAL keeps it until it is converted.
    if !found.is_some_and(|h| h.wal) {
        set_aside_wal(path)?;
    }
    let mut conn = connect(path, busy)?;
    // Before any write, so not even the schema's commit deletes a journal. On a file the header
    // let through this writes nothing to it: a rollback journal mode is the connection's, and
    // only a stamped store in WAL (converted here) changes its header.
    use_truncate(&conn, busy)?;
    match inspect(&conn)? {
        Contents::Empty => create_schema(&conn, path)?,
        Contents::Store => check_stamp(user_version(&conn)?)?,
        Contents::Foreign => return Err(OpenError::NotAStore),
    }
    check_stamp(user_version(&conn)?)?;
    // TRUNCATE is in force, so no connection is using a WAL now; one left beside the store is
    // stale (its frames were checkpointed when the mode switched) and is never replayed.
    set_aside_wal(path)?;
    keep_identity_at(&mut conn, path)?;
    SqliteEventStore::new(conn).map_err(OpenError::Store)
}

/// ST-07's refusals, decided from the header alone so that no connection touches the file: a
/// newer major, an old plugin ledger (stamped below 1000), and an unstamped database that has
/// held a schema or is in WAL (a weighin 0.x ledger or another program's database; never a Core
/// store, which is stamped as its schema is created). An unstamped database with no schema is
/// an empty file, which becomes a store.
fn refuse(found: &Header) -> Result<(), OpenError> {
    match found.user_version {
        0 if found.wal || found.schema_cookie != 0 => Err(OpenError::NotAStore),
        0 => Ok(()),
        stamp => check_stamp(stamp),
    }
}

/// A stamp this build opens: SPEC 1.x, any minor (ST-07). Below 1000 is an old plugin ledger,
/// which is imported, never opened (ST-15, ST-16).
fn check_stamp(found: i64) -> Result<(), OpenError> {
    let major = |v: i64| v.div_euclid(1000);
    if found < STORE_SCHEMA {
        Err(OpenError::OldLedger { found })
    } else if major(found) > major(STORE_SCHEMA) {
        Err(OpenError::NewerMajor {
            found,
            supported: STORE_SCHEMA,
        })
    } else {
        Ok(())
    }
}

/// A connection that has written nothing: the busy timeout, durability and foreign keys are
/// per connection and never touch the file.
fn connect(path: &Path, busy: Duration) -> Result<Connection, OpenError> {
    let conn = Connection::open(path)?;
    conn.busy_timeout(busy)?;
    // FULL, not NORMAL: under a rollback journal NORMAL can lose the last commit on power loss.
    conn.execute_batch("PRAGMA synchronous = FULL; PRAGMA foreign_keys = ON;")?;
    Ok(conn)
}

fn inspect(conn: &Connection) -> Result<Contents, OpenError> {
    let (objects, store_meta): (i64, i64) = conn.query_row(
        "SELECT count(*), coalesce(sum(type = 'table' AND name = 'store_meta'), 0)
         FROM main.sqlite_master",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    Ok(match (objects, store_meta) {
        (0, _) => Contents::Empty,
        (_, 0) => Contents::Foreign,
        _ => Contents::Store,
    })
}

/// Switches the connection to TRUNCATE. A store in WAL converts here; the conversion needs every
/// other connection gone, and SQLite reports that as busy without calling the busy handler, so
/// it is retried until `busy` has passed.
fn use_truncate(conn: &Connection, busy: Duration) -> Result<(), OpenError> {
    let deadline = Instant::now().checked_add(busy);
    let sql = format!("PRAGMA journal_mode = {JOURNAL_MODE}");
    loop {
        let result = conn.query_row(&sql, [], |row| row.get::<_, String>(0));
        let retry = deadline.is_some_and(|d| Instant::now() < d);
        match result {
            Ok(mode) if mode.eq_ignore_ascii_case(JOURNAL_MODE) => return Ok(()),
            Ok(_) if retry => {}
            Err(rusqlite::Error::SqliteFailure(f, _))
                if retry
                    && matches!(f.code, ErrorCode::DatabaseBusy | ErrorCode::DatabaseLocked) => {}
            Ok(found) => return Err(OpenError::JournalMode { found }),
            Err(other) => return Err(OpenError::Sqlite(other)),
        }
        std::thread::sleep(RETRY_PAUSE);
    }
}

/// `<store><suffix>`: the journal and WAL files SQLite keeps beside a database.
pub(crate) fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

/// Renames a `-wal` beside the store aside; renaming is allowed where deleting is not (ST-05).
///
/// * Before connecting to a file not in WAL: SQLite deletes a `-wal` it finds beside such a
///   database, and a failed WAL creation in a Cowork folder leaves exactly that.
/// * After switching to TRUNCATE: converting a WAL store where deletes are refused leaves its
///   checkpointed `-wal` behind.
fn set_aside_wal(path: &Path) -> Result<(), OpenError> {
    let wal = sibling(path, "-wal");
    if wal.exists() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let aside = sibling(path, &format!("-wal.orphaned-{stamp}"));
        std::fs::rename(&wal, &aside)
            .map_err(|source| OpenError::SetAside { path: wal, source })?;
    }
    Ok(())
}

pub(crate) fn user_version(conn: &Connection) -> Result<i64, OpenError> {
    Ok(conn.query_row("PRAGMA user_version", [], |row| row.get(0))?)
}

/// Gives an empty file happenstance's schema, Core's table and the stamp, in one
/// `BEGIN IMMEDIATE`, without letting happenstance open it: its open switches a file to WAL.
fn create_schema(conn: &Connection, path: &Path) -> Result<(), OpenError> {
    let canonical = canonical_bytes(path)?;
    let tmp = tempfile::tempdir().map_err(OpenError::TempDir)?;
    let template = tmp.path().join("template.sqlite3");
    drop(SqliteEventStore::open(&template).map_err(OpenError::Template)?);
    let Some(name) = template.to_str() else {
        return Err(OpenError::TemplatePath { path: template });
    };
    conn.execute("ATTACH DATABASE ?1 AS tpl", [name])?;
    let copied = copy_schema(conn, &canonical);
    let detached = conn.execute_batch("DETACH DATABASE tpl");
    match (copied, detached) {
        (Ok(()), Ok(())) => tmp.close().map_err(OpenError::TempDir),
        (Ok(()), Err(cleanup)) => Err(OpenError::Sqlite(cleanup)),
        (Err(failure), Ok(())) => Err(failure),
        (Err(failure), Err(cleanup)) => Err(OpenError::Cleanup {
            failure: Box::new(failure),
            cleanup,
        }),
    }
}

fn copy_schema(conn: &Connection, canonical: &[u8]) -> Result<(), OpenError> {
    conn.execute_batch("BEGIN IMMEDIATE")?;
    let body = (|| {
        // Another process may have created the schema while the template was built.
        if inspect(conn)? != Contents::Empty {
            return Ok(());
        }
        let objects = {
            let mut stmt = conn.prepare(
                "SELECT type, name, sql FROM tpl.sqlite_master
                 WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%'
                 ORDER BY CASE type WHEN 'table' THEN 0 ELSE 1 END, rowid",
            )?;
            stmt.query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?
        };
        if objects.is_empty() {
            return Err(OpenError::EmptyTemplate);
        }
        // Unqualified DDL creates its object in `main`.
        for (_, _, sql) in &objects {
            conn.execute_batch(sql)?;
        }
        for (_, name, _) in objects.iter().filter(|(ty, _, _)| ty == "table") {
            let name = name.replace('"', "\"\"");
            conn.execute(
                &format!("INSERT INTO main.\"{name}\" SELECT * FROM tpl.\"{name}\""),
                [],
            )?;
        }
        conn.execute_batch(CORE_META)?;
        conn.execute(
            "INSERT INTO core_meta (k, v) VALUES (?1, ?2)",
            rusqlite::params![PATH_KEY, canonical],
        )?;
        conn.execute_batch(&format!("PRAGMA user_version = {STORE_SCHEMA}"))?;
        Ok(())
    })();
    finish_transaction(conn, body)
}

/// Re-mints the store's identity when it is opened at another path than the one it recorded:
/// a copied `.git` or a restored backup must not issue the same event identities (ID-01).
/// A store opened where it was recorded is not written to.
fn keep_identity_at(conn: &mut Connection, path: &Path) -> Result<(), OpenError> {
    let canonical = canonical_bytes(path)?;
    let recorded: Option<Vec<u8>> = conn
        .query_row("SELECT v FROM core_meta WHERE k = ?1", [PATH_KEY], |r| {
            r.get(0)
        })
        .map(Some)
        .or_else(|e| match e {
            rusqlite::Error::QueryReturnedNoRows => Ok(None),
            other => Err(other),
        })?;
    if recorded.as_deref() == Some(canonical.as_slice()) {
        return Ok(());
    }
    SqliteEventStore::remint_identity_on(conn).map_err(OpenError::Store)?;
    conn.execute(
        "INSERT OR REPLACE INTO core_meta (k, v) VALUES (?1, ?2)",
        rusqlite::params![PATH_KEY, canonical],
    )?;
    Ok(())
}

/// The store's canonical path, as the platform encodes it; compared, never displayed.
fn canonical_bytes(path: &Path) -> Result<Vec<u8>, OpenError> {
    let canonical = std::fs::canonicalize(path).map_err(OpenError::Canonicalize)?;
    Ok(canonical.into_os_string().into_encoded_bytes())
}

/// Ends the transaction a `BEGIN IMMEDIATE` opened: commits when `body` succeeded, and rolls back
/// when it failed or when the commit itself failed, since a failed `COMMIT` (a deferred
/// constraint, a busy lock) leaves the transaction open on the connection.
pub(crate) fn finish_transaction(
    conn: &Connection,
    body: Result<(), OpenError>,
) -> Result<(), OpenError> {
    let failure = match body {
        Ok(()) => match conn.execute_batch("COMMIT") {
            Ok(()) => return Ok(()),
            Err(e) => OpenError::Sqlite(e),
        },
        Err(e) => e,
    };
    // SQLite ends the transaction itself on some errors (SQLITE_FULL, IOERR, NOMEM); a ROLLBACK
    // then would only add a misleading "no transaction is active".
    if conn.is_autocommit() {
        return Err(failure);
    }
    match conn.execute_batch("ROLLBACK") {
        Ok(()) => Err(failure),
        Err(cleanup) => Err(OpenError::Cleanup {
            failure: Box::new(failure),
            cleanup,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A connection inside a transaction holding a row that breaks a deferred foreign key, so its
    /// `COMMIT` fails and leaves the transaction open.
    fn transaction_whose_commit_fails() -> Connection {
        let conn = Connection::open_in_memory().expect("memory database");
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE parent (id INTEGER PRIMARY KEY);
             CREATE TABLE child (parent INTEGER REFERENCES parent (id) DEFERRABLE INITIALLY DEFERRED);
             BEGIN IMMEDIATE;
             INSERT INTO child (parent) VALUES (1);",
        )
        .expect("fixture");
        conn
    }

    #[test]
    fn a_failed_commit_is_rolled_back_and_reported() {
        let conn = transaction_whose_commit_fails();
        let err = finish_transaction(&conn, Ok(())).expect_err("the commit fails");
        assert!(format!("{err:?}").contains("FOREIGN KEY"), "{err:?}");
        assert!(conn.is_autocommit(), "the transaction is rolled back");
        let children: i64 = conn
            .query_row("SELECT count(*) FROM child", [], |r| r.get(0))
            .expect("count");
        assert_eq!(children, 0, "the rolled-back row is gone");
    }

    #[test]
    fn a_failed_body_is_rolled_back_and_its_error_returned() {
        let conn = transaction_whose_commit_fails();
        let err = finish_transaction(&conn, Err(OpenError::NotAStore)).expect_err("body failed");
        assert!(matches!(err, OpenError::NotAStore), "{err:?}");
        assert!(conn.is_autocommit(), "the transaction is rolled back");
    }

    #[test]
    fn stamps_are_judged_by_major() {
        assert!(matches!(check_stamp(1000), Ok(())));
        assert!(matches!(check_stamp(1007), Ok(())), "a newer minor opens");
        assert!(matches!(
            check_stamp(2000),
            Err(OpenError::NewerMajor { found: 2000, .. })
        ));
        assert!(matches!(
            check_stamp(1),
            Err(OpenError::OldLedger { found: 1 })
        ));
        assert!(matches!(
            check_stamp(0),
            Err(OpenError::OldLedger { found: 0 })
        ));
        assert!(matches!(
            check_stamp(-1),
            Err(OpenError::OldLedger { found: -1 })
        ));
    }
}
