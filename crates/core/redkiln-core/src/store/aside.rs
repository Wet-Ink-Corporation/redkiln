//! Files beside a store, set aside by renaming, never deleted (ST-05): a stale `-wal`, and the
//! `-journal` of a creation that never committed, unless a writer still holds it.

use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(not(windows))]
use happenstance_sqlite::rusqlite::ffi;
use happenstance_sqlite::rusqlite::{self, Connection, OpenFlags};

use super::telemetry::busy_code;
use super::{OpenError, Step};

/// How many names [`set_aside`] tries before giving up.
const ASIDE_NAMES: u32 = 100;

/// Windows' `ERROR_SHARING_VIOLATION`: another process holds the file open without sharing it,
/// as SQLite holds a live journal.
#[cfg(windows)]
const SHARING_VIOLATION: i32 = 32;

/// `<store><suffix>`: the journal and WAL files SQLite keeps beside a database.
pub(crate) fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut s = path.as_os_str().to_owned();
    s.push(suffix);
    PathBuf::from(s)
}

/// Renames `<store><suffix>` aside, to a name not taken yet; renaming is allowed where deleting
/// is not (ST-05). Nothing happens when there is no such file.
///
/// * Before connecting to a file not in WAL: SQLite deletes a `-wal` it finds beside such a
///   database, and a failed WAL creation in a Cowork folder leaves exactly that.
/// * After switching to TRUNCATE: converting a WAL store where deletes are refused leaves its
///   checkpointed `-wal` behind.
pub(super) fn set_aside(path: &Path, suffix: &str) -> Result<(), OpenError> {
    let file = sibling(path, suffix);
    if !file.try_exists().map_err(failed(path, suffix))? {
        return Ok(());
    }
    let aside = aside_name(path, suffix)?;
    std::fs::rename(&file, &aside).map_err(|e| busy_or(e, failed(path, suffix)))
}

/// Sets aside the `-journal` beside a file with no committed store yet (missing, empty, or a
/// database never stamped), unless a writer still holds it (F15). The journal is renamed, never
/// copied, deleted or played back (ST-05).
///
/// No Core opener can hold it, since the caller holds the open lock; another program creating a
/// database at the path can. An empty journal holds nothing to roll back, and one beside no file
/// belongs to no writer: both are set aside as they are. Otherwise only SQLite's locks tell a
/// live journal from a stale one, and any read-write connection that finds a journal with no
/// writer behind it deletes it or plays it back. So no read-write connection sees it:
///
/// 1. A read-only connection, with no busy wait, takes SQLite's read lock and keeps it. A
///    read-only connection cannot do either to a journal (SQLite refuses it the write lock a
///    delete takes, and answers a hot journal with `SQLITE_READONLY_ROLLBACK`). Busy means a
///    writer is committing: [`OpenError::WriterBusy`], and nothing moves. While the lock is
///    held, no writer can commit, change the file or end its transaction.
/// 2. The journal is renamed aside. On Windows a live journal is held open without sharing, so
///    the rename is refused (`ERROR_SHARING_VIOLATION`): [`OpenError::WriterBusy`].
/// 3. With no journal under its name, a read-write connection with no busy wait tries
///    `BEGIN IMMEDIATE`, then rolls back, having written nothing. Busy means a writer holds the
///    journal: it is renamed back, under the read lock, and the open is
///    [`OpenError::WriterBusy`], with nothing left aside.
///
/// Any other failure of step 3 puts the journal back too, so only a probe that found no writer
/// leaves it aside. A journal SQLite found hot in step 1 had no writer behind it, so it stays
/// aside whatever step 3 finds. On Windows step 1 is skipped (a read-only connection there can
/// take the write lock and delete a stale journal); step 2 is its test, and a journal it could
/// rename was held open by no writer, so it stays aside too.
pub(super) fn set_journal_aside(path: &Path) -> Result<(), OpenError> {
    let journal = sibling(path, "-journal");
    let len = match std::fs::metadata(&journal) {
        Ok(found) => found.len(),
        Err(missing) if missing.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(other) => return Err(failed(path, "-journal")(other)),
    };
    if len == 0 || !path.try_exists().map_err(failed(path, "-journal"))? {
        return set_aside(path, "-journal");
    }
    #[cfg(not(windows))]
    let reader = read_lock(path)?;
    // Windows skips step 1: a read-only connection there takes SQLite's write lock, and would
    // delete a stale journal beside an empty file. The rename is the test.
    #[cfg(windows)]
    let reader: Option<Connection> = None;
    let aside = aside_name(path, "-journal")?;
    match std::fs::rename(&journal, &aside) {
        Ok(()) => {}
        // Ended and cleared since: nothing to set aside.
        Err(gone) if gone.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(other) => return Err(busy_or(other, failed(path, "-journal"))),
    }
    let probed = probe_writer(path);
    if probed.is_err() && reader.is_some() {
        put_back(path, &aside, &journal)?;
    }
    // The read lock ends only now, after the journal is back under its name.
    drop(reader);
    probed
}

/// Step 1 of [`set_journal_aside`]: SQLite's read lock on a read-only connection with no busy
/// wait, held on the connection returned until it is dropped. `None` when SQLite found the
/// journal hot, which means no writer held it.
///
/// # Errors
/// [`OpenError::WriterBusy`] when a writer is committing; [`OpenError::Sqlite`] at
/// [`Step::Probe`] otherwise.
#[cfg(not(windows))]
fn read_lock(path: &Path) -> Result<Option<Connection>, OpenError> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY
        | OpenFlags::SQLITE_OPEN_URI
        | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(path, flags).map_err(probe)?;
    conn.busy_timeout(Duration::ZERO).map_err(probe)?;
    // Reading the schema takes the read lock, and the open transaction keeps it.
    match conn.execute_batch("BEGIN; SELECT count(*) FROM sqlite_schema;") {
        Ok(()) => Ok(Some(conn)),
        Err(busy) if busy_code(&busy) => Err(OpenError::WriterBusy),
        Err(refused) if hot(&refused) => Ok(None),
        Err(other) => Err(probe(other)),
    }
}

/// Step 3 of [`set_journal_aside`]: [`OpenError::WriterBusy`] when another connection holds the
/// file's write lock. `BEGIN IMMEDIATE`, with no busy wait, on a connection that cannot create
/// the file, rolled back at once; it writes nothing, and with the journal renamed it finds no
/// journal to delete or play back.
fn probe_writer(path: &Path) -> Result<(), OpenError> {
    let flags = OpenFlags::SQLITE_OPEN_READ_WRITE
        | OpenFlags::SQLITE_OPEN_URI
        | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(path, flags).map_err(probe)?;
    conn.busy_timeout(Duration::ZERO).map_err(probe)?;
    match conn.execute_batch("BEGIN IMMEDIATE") {
        Ok(()) => conn.execute_batch("ROLLBACK").map_err(probe),
        Err(busy) if busy_code(&busy) => Err(OpenError::WriterBusy),
        Err(other) => Err(probe(other)),
    }
}

/// A hot journal, refused to a read-only connection: `SQLITE_READONLY_ROLLBACK`.
#[cfg(not(windows))]
fn hot(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(failure, _)
            if failure.extended_code == ffi::SQLITE_READONLY_ROLLBACK
    )
}

/// A [`map_err`](Result::map_err) for a probe's SQLite failure.
fn probe(source: rusqlite::Error) -> OpenError {
    OpenError::Sqlite {
        step: Step::Probe,
        source,
    }
}

/// Puts the journal set aside at `aside` back under its name: a writer held it after all, or
/// the probe could not tell.
/// Under the read lock no other writer can have started one; a journal back under the name
/// anyway is left where it is, and the one aside stays aside.
fn put_back(path: &Path, aside: &Path, journal: &Path) -> Result<(), OpenError> {
    if journal.try_exists().map_err(failed(path, "-journal"))? {
        return Ok(());
    }
    std::fs::rename(aside, journal).map_err(failed(path, "-journal"))
}

/// A name `<store><suffix>.orphaned-<stamp>-<n>` not taken yet.
fn aside_name(path: &Path, suffix: &str) -> Result<PathBuf, OpenError> {
    // The clock only makes names readable; uniqueness comes from checking each candidate.
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    for n in 0..ASIDE_NAMES {
        let aside = sibling(path, &format!("{suffix}.orphaned-{stamp}-{n}"));
        if !aside.try_exists().map_err(failed(path, suffix))? {
            return Ok(aside);
        }
    }
    Err(OpenError::SetAside {
        path: sibling(path, suffix),
        source: io::Error::from(io::ErrorKind::AlreadyExists),
    })
}

/// A `map_err` naming the file that stayed: `<store><suffix>`.
fn failed<'a>(path: &'a Path, suffix: &'a str) -> impl Fn(io::Error) -> OpenError + 'a {
    // The name is built only on failure.
    move |source| OpenError::SetAside {
        path: sibling(path, suffix),
        source,
    }
}

/// A rename Windows refused because another process holds the file (a live journal) is
/// [`OpenError::WriterBusy`]; anything else is `other`'s.
fn busy_or(error: io::Error, other: impl Fn(io::Error) -> OpenError) -> OpenError {
    #[cfg(windows)]
    if error.raw_os_error() == Some(SHARING_VIOLATION) {
        return OpenError::WriterBusy;
    }
    other(error)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The names in `dir` with `-journal.orphaned-` in them, and the bytes of each.
    fn orphans(dir: &Path) -> Vec<Vec<u8>> {
        std::fs::read_dir(dir)
            .expect("list")
            .map(|e| e.expect("entry").path())
            .filter(|p| p.to_string_lossy().contains("-journal.orphaned-"))
            .map(|p| std::fs::read(p).expect("read"))
            .collect()
    }

    /// The file's identity on disk: the same file under another name has the same one, a copy
    /// does not.
    #[cfg(unix)]
    fn inode(path: &Path) -> u64 {
        std::os::unix::fs::MetadataExt::ino(&std::fs::metadata(path).expect("stat"))
    }

    /// The one orphaned journal in `dir`.
    #[cfg(unix)]
    fn orphan_path(dir: &Path) -> PathBuf {
        let found: Vec<PathBuf> = std::fs::read_dir(dir)
            .expect("list")
            .map(|e| e.expect("entry").path())
            .filter(|p| p.to_string_lossy().contains("-journal.orphaned-"))
            .collect();
        let [one] = <[PathBuf; 1]>::try_from(found).expect("one orphan");
        one
    }

    /// A database with pages and the hot journal of a transaction that spilled into it, as a
    /// writer killed mid-transaction leaves them: SQLite would roll the file back and delete
    /// the journal. Returns the file's and the journal's bytes.
    fn hot_journal_over_pages(path: &Path) -> (Vec<u8>, Vec<u8>) {
        let setup = Connection::open(path).expect("connect");
        setup
            .execute_batch("PRAGMA page_size = 4096; VACUUM;")
            .expect("a database with a page");
        drop(setup);
        let writer = Connection::open(path).expect("connect");
        writer
            .execute_batch(
                "PRAGMA journal_mode = persist; PRAGMA cache_size = 1; BEGIN IMMEDIATE;
                 CREATE TABLE t (x);
                 WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 50)
                 INSERT INTO t SELECT randomblob(3000) FROM n;",
            )
            .expect("a transaction that spills pages into the file");
        let journal = std::fs::read(sibling(path, "-journal")).expect("its journal");
        let file = std::fs::read(path).expect("the file mid-transaction");
        writer.execute_batch("ROLLBACK").expect("roll back");
        drop(writer);
        std::fs::write(path, &file).expect("the file as the writer left it");
        std::fs::write(sibling(path, "-journal"), &journal).expect("its journal");
        assert_ne!(journal.first(), Some(&0), "the journal is hot");
        (file, journal)
    }

    #[test]
    fn a_hot_journal_beside_a_database_is_renamed_aside_never_played_back() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("events.sqlite3");
        let (file, journal) = hot_journal_over_pages(&path);
        #[cfg(unix)]
        let before = inode(&sibling(&path, "-journal"));

        set_journal_aside(&path).expect("set aside");

        assert_eq!(std::fs::read(&path).expect("read"), file, "not rolled back");
        assert!(!sibling(&path, "-journal").exists(), "no journal by name");
        assert_eq!(orphans(dir.path()), [journal], "kept aside, once");
        #[cfg(unix)]
        assert_eq!(
            inode(&orphan_path(dir.path())),
            before,
            "renamed, not copied"
        );
    }

    #[test]
    fn a_stale_journal_beside_an_empty_file_is_renamed_aside_never_deleted() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("events.sqlite3");
        let writer = Connection::open(&path).expect("connect");
        writer
            .execute_batch("BEGIN IMMEDIATE; CREATE TABLE t (x);")
            .expect("a creation in progress");
        let stale = std::fs::read(sibling(&path, "-journal")).expect("its journal");
        writer.execute_batch("ROLLBACK").expect("roll back");
        drop(writer);
        std::fs::write(&path, b"").expect("an empty file");
        std::fs::write(sibling(&path, "-journal"), &stale).expect("a stale journal");
        #[cfg(unix)]
        let before = inode(&sibling(&path, "-journal"));

        set_journal_aside(&path).expect("set aside");

        assert_eq!(orphans(dir.path()), [stale], "kept aside, once");
        #[cfg(unix)]
        assert_eq!(
            inode(&orphan_path(dir.path())),
            before,
            "renamed, not copied"
        );
    }

    #[test]
    fn a_live_writers_journal_is_busy_and_nothing_is_left_aside() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("events.sqlite3");
        let writer = Connection::open(&path).expect("connect");
        writer
            .execute_batch("BEGIN IMMEDIATE; CREATE TABLE t (x);")
            .expect("a creation in progress");
        let live = std::fs::read(sibling(&path, "-journal")).expect("its journal");
        #[cfg(unix)]
        let before = inode(&sibling(&path, "-journal"));

        assert!(
            matches!(set_journal_aside(&path), Err(OpenError::WriterBusy)),
            "busy"
        );

        assert!(orphans(dir.path()).is_empty(), "no copy, no orphan");
        assert_eq!(
            std::fs::read(sibling(&path, "-journal")).expect("still there"),
            live
        );
        #[cfg(unix)]
        assert_eq!(inode(&sibling(&path, "-journal")), before, "the same file");
        writer.execute_batch("COMMIT").expect("the writer commits");
    }

    #[test]
    fn setting_aside_never_overwrites_an_earlier_orphan() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = dir.path().join("events.sqlite3");
        for content in [&b"first"[..], b"second", b"third"] {
            std::fs::write(sibling(&store, "-wal"), content).expect("write");
            set_aside(&store, "-wal").expect("set aside");
        }
        let mut kept: Vec<Vec<u8>> = std::fs::read_dir(dir.path())
            .expect("list")
            .map(|e| std::fs::read(e.expect("entry").path()).expect("read"))
            .collect();
        kept.sort();
        assert_eq!(
            kept,
            vec![b"first".to_vec(), b"second".to_vec(), b"third".to_vec()]
        );
        assert!(
            matches!(set_aside(&store, "-wal"), Ok(())),
            "nothing to set aside"
        );
    }
}
