//! Opening a store file: one connection under TRUNCATE, the schema copied in from a template
//! with its stamp, and the refusals of ST-07 decided before anything is written.

use std::fs::{File, OpenOptions, TryLockError};
use std::io;
use std::path::Path;
use std::time::{Duration, Instant};

use happenstance_sqlite::event_store::SqliteEventStore;
use happenstance_sqlite::rusqlite::{self, Connection, ErrorCode, OpenFlags};

use super::aside::{set_aside, set_journal_aside, sibling};
use super::header::{self, Found, Header};
use super::schema::{StampOutOfRange, StoreSchema};
use super::{OpenError, Step};

/// The journal mode every store connection runs under, as `PRAGMA journal_mode` reports it.
pub(crate) const JOURNAL_MODE: &str = "truncate";

/// The store schema this build reads and writes: SPEC 1.0, `major * 1000 + minor` (ST-06).
pub const STORE_SCHEMA: i64 = 1000;

/// Core's own key-value table, part of schema 1.0. It holds the store's canonical path (ID-01).
const CORE_META: &str =
    "CREATE TABLE core_meta (k TEXT PRIMARY KEY, v BLOB NOT NULL) WITHOUT ROWID";

/// The `core_meta` key holding the path the store was created or last re-minted at.
const PATH_KEY: &str = "path";

/// The pause between attempts while another process holds what an open needs: the open lock
/// (`take_lock`), or a WAL store's connection while it is switched to TRUNCATE (`use_truncate`).
const RETRY_PAUSE: Duration = Duration::from_millis(20);

/// The suffix of the open lock, `<store>.lock`: a file created once beside a store and never
/// deleted (ST-05), whose lock every Core opener holds from reading the header to handing the
/// connection over. While it is held, a `-journal` or `-wal` beside the store is no Core
/// opener's own, so setting one aside never takes another creator's live journal.
pub(crate) const LOCK_SUFFIX: &str = ".lock";

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

/// A `map_err` for a SQLite call made at `step`.
const fn at(step: Step) -> impl FnOnce(rusqlite::Error) -> OpenError {
    move |source| OpenError::Sqlite { step, source }
}

/// Whether an open may create the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Mode {
    /// Creates the store, and its directory, when there is none: [`super::Store::open`].
    Create,
    /// Opens only a store that exists, refusing a missing or empty file, or a database never
    /// stamped, with [`OpenError::NoStore`] before anything is created or set aside: the
    /// telemetry connection, which must never stand up a new store where the opened one was,
    /// and [`super::Store::open_existing`], for reads.
    Existing,
}

/// Opens the store at `path` and hands its connection to happenstance; under [`Mode::Create`]
/// a missing or empty file becomes a new store.
///
/// Until the file is known to be a store this build opens, or an empty database it may create,
/// nothing is written to it or beside it, so a refused file is left exactly as it was. A file
/// stamped in Core's range (1000 to 1999) is taken to be a store: only Core writes that stamp
/// (ST-06).
pub(super) fn open(path: &Path, busy: Duration, mode: Mode) -> Result<SqliteEventStore, OpenError> {
    open_store(path, busy, mode).map(|(store, _schema)| store)
}

/// [`open`], with the schema the file's stamp gives, as the open read it.
pub(super) fn open_store(
    path: &Path,
    busy: Duration,
    mode: Mode,
) -> Result<(SqliteEventStore, StoreSchema), OpenError> {
    if mode == Mode::Create
        && let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty())
    {
        std::fs::create_dir_all(dir).map_err(OpenError::CreateDir)?;
    }
    // Refused first, so that no lock file is created beside a file this build will not open.
    refuse_found(&header::read(path)?, mode)?;
    let lock = take_lock(path, busy)?;
    let store = open_locked(path, busy, mode)?;
    // Closing the lock file releases its lock, and then lets Core spawn again.
    drop(lock);
    Ok(store)
}

/// The refusals the header decides, before anything is created beside the file: a file that is
/// not SQLite, ST-07's refusals, and under [`Mode::Existing`] a file holding no store yet.
fn refuse_found(found: &Found, mode: Mode) -> Result<(), OpenError> {
    match (found, mode) {
        (Found::NotSqlite, _) => Err(OpenError::NotAStore),
        (Found::Sqlite(header), Mode::Existing) if header.user_version == 0 => {
            refuse(header).and(Err(OpenError::NoStore))
        }
        (Found::Sqlite(header), _) => refuse(header),
        (Found::Missing | Found::Empty, Mode::Existing) => Err(OpenError::NoStore),
        (Found::Missing | Found::Empty, Mode::Create) => Ok(()),
    }
}

/// The open lock, held until dropped. [`open`] holds it through [`open_locked`], SQLite's own busy
/// waits included, so Core spawns nothing for up to a few times `busy`; no Core path spawns while
/// it holds a SQLite lock, so that wait ends. Its fields drop in order: the file first, which releases
/// the lock, then the guard that kept Core from spawning while it was held.
struct Locked {
    _file: File,
    _no_forks: std::sync::RwLockWriteGuard<'static, ()>,
}

/// Takes the open lock, waiting at most `busy` for another opener to finish. A file system
/// without whole-file locks opens unlocked, as SQLite alone would: the lock only keeps
/// concurrent first creations from setting each other's journals aside.
///
/// Each attempt opens the lock file afresh under [`crate::git::no_forks`], and a held lock keeps
/// it: no child Core spawns can share the lock file's description, so none holds the lock after
/// this process lets it go. Between attempts, Core spawns as usual. A caller's own spawns on
/// other threads are not covered (`FORKS` in `crate::git` says why).
fn take_lock(path: &Path, busy: Duration) -> Result<Option<Locked>, OpenError> {
    let deadline = Instant::now().checked_add(busy);
    loop {
        let no_forks = crate::git::no_forks();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(sibling(path, LOCK_SUFFIX))
            .map_err(OpenError::Lock)?;
        match file.try_lock() {
            Ok(()) => {
                return Ok(Some(Locked {
                    _file: file,
                    _no_forks: no_forks,
                }));
            }
            Err(TryLockError::WouldBlock) if deadline.is_some_and(|d| Instant::now() < d) => {
                // The file closes first, so no later child can share it.
                drop(file);
                drop(no_forks);
            }
            Err(TryLockError::WouldBlock) => return Err(OpenError::LockBusy),
            Err(TryLockError::Error(e)) if e.kind() == io::ErrorKind::Unsupported => {
                return Ok(None);
            }
            Err(TryLockError::Error(e)) => return Err(OpenError::Lock(e)),
        }
        std::thread::sleep(RETRY_PAUSE);
    }
}

/// [`open`] under the open lock: the header is read again, since it may have changed while the
/// lock was awaited.
fn open_locked(
    path: &Path,
    busy: Duration,
    mode: Mode,
) -> Result<(SqliteEventStore, StoreSchema), OpenError> {
    let found = header::read(path)?;
    refuse_found(&found, mode)?;
    match found {
        Found::NotSqlite => return Err(OpenError::NotAStore),
        Found::Sqlite(found) => {
            // A `-wal` beside a file that is not in WAL is stale, and SQLite would delete it on
            // connect, which Cowork refuses. A stamped store in WAL keeps it until converted.
            if !found.wal {
                set_aside(path, "-wal")?;
            }
            if found.user_version == 0 {
                // An empty database: a journal beside it is from a creation that never
                // committed, unless another program's writer holds it; SQLite would delete a
                // stale one under its default mode.
                set_journal_aside(path)?;
            }
        }
        Found::Missing | Found::Empty => {
            // SQLite deletes a `-wal` or a hot `-journal` beside a file with no pages.
            set_aside(path, "-wal")?;
            set_journal_aside(path)?;
        }
    }
    let conn = connect(path, busy)?;
    // Before any write, so not even the schema's commit deletes a journal. On a file the header
    // let through this writes nothing to it: a rollback journal mode is the connection's, and
    // only a stamped store in WAL (converted here) changes its header.
    use_truncate(&conn, busy)?;
    match (inspect(&conn)?, mode) {
        (Contents::Empty, Mode::Create) => create_schema(&conn, path)?,
        (Contents::Empty, Mode::Existing) => return Err(OpenError::NoStore),
        (Contents::Store, _) => {}
        (Contents::Foreign, _) => return Err(OpenError::NotAStore),
    }
    let schema = schema_of(user_version(&conn)?)?;
    // TRUNCATE is in force, so no connection is using a WAL now; one left beside the store is
    // stale (its frames were checkpointed when the mode switched) and is never replayed.
    set_aside(path, "-wal")?;
    keep_identity_at(&conn, path)?;
    let store = SqliteEventStore::new(conn).map_err(OpenError::Store)?;
    Ok((store, schema))
}

/// ST-07's refusals, decided from the header alone so that no connection touches the file: a
/// newer major, a file stamped below 1000 (an old plugin ledger, or another program's
/// database), and an unstamped database that has held a schema or is in WAL (a weighin 0.x
/// ledger or another program's database; never a Core store, which is stamped as its schema
/// is created). An unstamped database with no schema is an empty file, which becomes a store.
fn refuse(found: &Header) -> Result<(), OpenError> {
    match found.user_version {
        0 if found.wal || found.schema_cookie != 0 => Err(OpenError::NotAStore),
        0 => Ok(()),
        stamp => check_stamp(stamp),
    }
}

/// A stamp this build opens: SPEC 1.x, any minor (ST-07). Below 1000 is not a Core store; an old
/// plugin ledger among those is imported, never opened (ST-15, ST-16).
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

/// The schema of a stamp this build opens ([`check_stamp`]). A stamp it lets through is 1000
/// to 1999, so always a schema; one that were not would be past any major this build reads.
fn schema_of(found: i64) -> Result<StoreSchema, OpenError> {
    check_stamp(found)?;
    StoreSchema::try_from(found).map_err(|StampOutOfRange(found)| OpenError::NewerMajor {
        found,
        supported: STORE_SCHEMA,
    })
}

/// A connection with no busy wait to a store [`open`] has opened, under TRUNCATE: for Core's own
/// statements on happenstance's tables, which happenstance's connection cannot run (the side
/// store's prune, EV-14).
///
/// It never creates the file: one that is gone since is an error, not a new empty database. The
/// header is read first: a file in WAL (another program opened it so) gets no connection
/// (`None`), and its `-wal`, live or not, stays where it is. Otherwise a stale `-wal` beside the
/// store is set aside, as [`open`] does, so SQLite never deletes it on connect (ST-05). The
/// caller checks, inside its own transaction, that the file is still the store its handle
/// opened.
pub(super) fn raw_connection(path: &Path) -> Result<Option<Connection>, OpenError> {
    if let Found::Sqlite(found) = header::read(path)?
        && found.wal
    {
        return Ok(None);
    }
    set_aside(path, "-wal")?;
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_URI
        | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(path, flags).map_err(at(Step::Connect))?;
    configure(&conn, Duration::ZERO)?;
    use_truncate(&conn, Duration::ZERO)?;
    Ok(Some(conn))
}

/// A connection that has written nothing: the busy timeout, durability and foreign keys are
/// per connection and never touch the file.
fn connect(path: &Path, busy: Duration) -> Result<Connection, OpenError> {
    let conn = Connection::open(path).map_err(at(Step::Connect))?;
    configure(&conn, busy)?;
    Ok(conn)
}

/// The busy timeout, durability and foreign keys of a store connection.
fn configure(conn: &Connection, busy: Duration) -> Result<(), OpenError> {
    conn.busy_timeout(busy).map_err(at(Step::Configure))?;
    // FULL, not NORMAL: under a rollback journal NORMAL can lose the last commit on power loss.
    conn.execute_batch("PRAGMA synchronous = FULL; PRAGMA foreign_keys = ON;")
        .map_err(at(Step::Configure))
}

fn inspect(conn: &Connection) -> Result<Contents, OpenError> {
    let (objects, store_meta): (i64, i64) = conn
        .query_row(
            "SELECT count(*), coalesce(sum(type = 'table' AND name = 'store_meta'), 0)
             FROM main.sqlite_master",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .map_err(at(Step::Inspect))?;
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
            Err(other) => return Err(at(Step::JournalMode)(other)),
        }
        std::thread::sleep(RETRY_PAUSE);
    }
}

pub(crate) fn user_version(conn: &Connection) -> Result<i64, OpenError> {
    conn.query_row("PRAGMA user_version", [], |row| row.get(0))
        .map_err(at(Step::ReadStamp))
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
    conn.execute("ATTACH DATABASE ?1 AS tpl", [name])
        .map_err(at(Step::AttachTemplate))?;
    let copied = copy_schema(conn, &canonical);
    let detached = conn.execute_batch("DETACH DATABASE tpl");
    match (copied, detached) {
        (Ok(()), Ok(())) => tmp.close().map_err(OpenError::TempDir),
        (Ok(()), Err(cleanup)) => Err(at(Step::DetachTemplate)(cleanup)),
        (Err(failure), Ok(())) => Err(failure),
        (Err(failure), Err(cleanup)) => Err(OpenError::Cleanup {
            failure: Box::new(failure),
            cleanup,
        }),
    }
}

fn copy_schema(conn: &Connection, canonical: &[u8]) -> Result<(), OpenError> {
    conn.execute_batch("BEGIN IMMEDIATE")
        .map_err(at(Step::Begin))?;
    let body = (|| {
        // Another process may have created the schema while the template was built.
        if inspect(conn)? != Contents::Empty {
            return Ok(());
        }
        let copy = at(Step::CopySchema);
        let objects = template_objects(conn).map_err(at(Step::CopySchema))?;
        if objects.is_empty() {
            return Err(OpenError::EmptyTemplate);
        }
        let result = (|| {
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
            conn.execute_batch(&format!("PRAGMA user_version = {STORE_SCHEMA}"))
        })();
        result.map_err(copy)
    })();
    finish_transaction(conn, body)
}

/// The template's schema objects, tables first: type, name and creating SQL.
fn template_objects(conn: &Connection) -> rusqlite::Result<Vec<(String, String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT type, name, sql FROM tpl.sqlite_master
         WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%'
         ORDER BY CASE type WHEN 'table' THEN 0 ELSE 1 END, rowid",
    )?;
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect()
}

/// Re-mints the store's identity when it is opened at another path than the one it recorded:
/// a copied `.git` or a restored backup must not issue the same event identities (ID-01).
///
/// A store opened where it was recorded is not written to. Otherwise the path is read again,
/// and the identity re-minted and the path recorded, in one `BEGIN IMMEDIATE`, so that of
/// several processes opening a copy at once exactly one re-mints and the rest read its identity.
/// The re-mint is happenstance's own statement on its documented `store_meta` row
/// (`remint_identity_on` opens a transaction of its own, so it cannot run inside this one).
fn keep_identity_at(conn: &Connection, path: &Path) -> Result<(), OpenError> {
    let canonical = canonical_bytes(path)?;
    if recorded_path(conn)?.as_deref() == Some(canonical.as_slice()) {
        return Ok(());
    }
    conn.execute_batch("BEGIN IMMEDIATE")
        .map_err(at(Step::Begin))?;
    let body = (|| {
        if recorded_path(conn)?.as_deref() == Some(canonical.as_slice()) {
            return Ok(());
        }
        let reminted = conn
            .execute(
                "UPDATE store_meta SET v = randomblob(16) WHERE k = 'store_id'",
                [],
            )
            .map_err(at(Step::Identity))?;
        if reminted != 1 {
            return Err(OpenError::MissingIdentity);
        }
        conn.execute(
            "INSERT OR REPLACE INTO core_meta (k, v) VALUES (?1, ?2)",
            rusqlite::params![PATH_KEY, canonical],
        )
        .map_err(at(Step::Identity))?;
        Ok(())
    })();
    finish_transaction(conn, body)
}

fn recorded_path(conn: &Connection) -> Result<Option<Vec<u8>>, OpenError> {
    match conn.query_row("SELECT v FROM core_meta WHERE k = ?1", [PATH_KEY], |r| {
        r.get(0)
    }) {
        Ok(path) => Ok(Some(path)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(other) => Err(at(Step::Identity)(other)),
    }
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
            Err(e) => at(Step::Commit)(e),
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

    /// A second handle on `path`'s lock file, holding its lock.
    fn hold_lock(path: &Path) -> File {
        let file = File::create(sibling(path, LOCK_SUFFIX)).expect("lock file");
        file.try_lock().expect("the lock is free");
        file
    }

    #[test]
    fn a_held_open_lock_keeps_core_from_spawning_until_it_is_let_go() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("store.db");
        let locked = take_lock(&path, Duration::ZERO)
            .expect("locks")
            .expect("whole-file locks");
        assert!(!crate::git::forks_free(), "no spawn while the lock is held");
        let second = File::open(sibling(&path, LOCK_SUFFIX)).expect("lock file");
        assert!(second.try_lock().is_err(), "the lock is held");
        drop(locked);
        drop(hold_lock(&path));
    }

    #[test]
    fn a_waiting_opener_lets_core_spawn_between_attempts() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("store.db");
        let other = hold_lock(&path);
        std::thread::scope(|s| {
            let waiter = std::thread::Builder::new()
                .name("waiting-opener".into())
                .spawn_scoped(s, || {
                    take_lock(&path, Duration::from_secs(30)).map(|l| l.is_some())
                })
                .expect("the thread starts");
            // Blocks until LockBusy if the waiter held the guard across its wait.
            drop(crate::git::forking());
            drop(other);
            let taken = waiter.join().expect("the waiter returns");
            assert!(matches!(taken, Ok(true)), "{taken:?}");
        });
    }

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
        assert!(
            matches!(
                &err,
                OpenError::Sqlite {
                    step: Step::Commit,
                    ..
                }
            ),
            "{err:?}"
        );
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

    fn names(dir: &Path) -> Vec<std::ffi::OsString> {
        let mut names: Vec<_> = std::fs::read_dir(dir)
            .expect("list")
            .map(|e| e.expect("entry").file_name())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn an_existing_only_open_of_a_missing_file_creates_nothing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("gone").join("events.sqlite3");
        let err = open(&path, Duration::ZERO, Mode::Existing).expect_err("no store");
        assert!(matches!(err, OpenError::NoStore), "{err:?}");
        assert!(names(dir.path()).is_empty(), "not even its directory");
    }

    #[test]
    fn an_existing_only_open_of_an_empty_file_or_database_leaves_it_as_it_was() {
        let dir = tempfile::tempdir().expect("tempdir");
        let empty = dir.path().join("empty.sqlite3");
        std::fs::write(&empty, b"").expect("an empty file");
        let unstamped = dir.path().join("unstamped.sqlite3");
        let conn = Connection::open(&unstamped).expect("open");
        // Writes a header: a database with no schema and no stamp.
        conn.execute_batch("PRAGMA application_id = 7")
            .expect("write the header");
        drop(conn);
        assert!(
            std::fs::metadata(&unstamped).expect("stat").len() > 0,
            "a header was written"
        );
        let before: Vec<_> = [&empty, &unstamped]
            .map(|p| std::fs::read(p).expect("read"))
            .into();
        for path in [&empty, &unstamped] {
            let err = open(path, Duration::ZERO, Mode::Existing).expect_err("no store");
            assert!(matches!(err, OpenError::NoStore), "{path:?}: {err:?}");
        }
        let after: Vec<_> = [&empty, &unstamped]
            .map(|p| std::fs::read(p).expect("read"))
            .into();
        assert_eq!(after, before, "neither file was written");
        assert_eq!(
            names(dir.path()),
            ["empty.sqlite3", "unstamped.sqlite3"],
            "no lock or journal beside them"
        );
    }

    #[test]
    fn an_existing_only_open_opens_an_existing_store() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("events.sqlite3");
        let created = open(&path, Duration::ZERO, Mode::Create).expect("created");
        let reopened = open(&path, Duration::ZERO, Mode::Existing).expect("opened");
        assert_eq!(reopened.store_id(), created.store_id());
    }

    #[test]
    fn the_schema_is_the_stamp_read_at_open() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("events.sqlite3");
        let (_, created) = open_store(&path, Duration::ZERO, Mode::Create).expect("created");
        assert_eq!(created.to_string(), "1.0", "a new store is STORE_SCHEMA");
        let conn = Connection::open(&path).expect("open");
        conn.execute_batch("PRAGMA user_version = 1003")
            .expect("a newer minor");
        drop(conn);
        let (_, reopened) = open_store(&path, Duration::ZERO, Mode::Existing).expect("opened");
        assert_eq!(reopened.to_string(), "1.3", "the file's own minor");
    }

    #[test]
    fn a_raw_connection_never_creates_its_file() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("events.sqlite3.telemetry");
        let err = raw_connection(&path).expect_err("no file");
        assert!(
            matches!(
                err,
                OpenError::Sqlite {
                    step: Step::Connect,
                    ..
                }
            ),
            "{err:?}"
        );
        assert!(names(dir.path()).is_empty(), "nothing created");
    }

    #[test]
    fn a_raw_connection_sets_a_stale_wal_aside_rather_than_let_sqlite_delete_it() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("events.sqlite3");
        drop(open(&path, Duration::ZERO, Mode::Create).expect("created"));
        std::fs::write(sibling(&path, "-wal"), b"stale").expect("a stale wal");
        let conn = raw_connection(&path).expect("connected");
        drop(conn);
        let kept: Vec<Vec<u8>> = std::fs::read_dir(dir.path())
            .expect("list")
            .map(|e| e.expect("entry"))
            .filter(|e| e.file_name().to_string_lossy().contains("-wal.orphaned-"))
            .map(|e| std::fs::read(e.path()).expect("read"))
            .collect();
        assert_eq!(kept, [b"stale".to_vec()], "set aside, not deleted");
    }

    #[test]
    fn a_raw_connection_to_a_file_in_wal_is_none_and_leaves_its_wal_in_place() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("events.sqlite3.telemetry");
        let conn = Connection::open(&path).expect("open");
        conn.execute_batch("PRAGMA journal_mode = WAL; CREATE TABLE t (x);")
            .expect("a database in WAL");
        drop(conn);
        std::fs::write(sibling(&path, "-wal"), b"live").expect("a wal");
        let conn = raw_connection(&path).expect("no failure");
        assert!(conn.is_none(), "no connection to a file in WAL");
        assert_eq!(
            std::fs::read(sibling(&path, "-wal")).expect("still there"),
            b"live",
            "its WAL is not set aside"
        );
    }
}
