//! Files beside a store, set aside by renaming, never deleted (ST-05): a stale `-wal`, and the
//! `-journal` of a creation that never committed, unless a writer still holds it.

use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use happenstance_sqlite::rusqlite::{Connection, OpenFlags};

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
/// database never stamped), unless a writer still holds it (F15).
///
/// No Core opener can hold it, since the caller holds the open lock; another program creating a
/// database at the path can. An empty journal holds nothing to roll back and is set aside as it
/// is. One with content is copied aside first, then a connection that never creates the file
/// and never waits tries SQLite's write lock: a writer holding it is
/// [`OpenError::WriterBusy`], and its journal keeps its name. Without one, SQLite may delete the
/// stale journal or roll it back while the probe takes its lock (it does so for any reader),
/// which the copy makes harmless; a journal still there is then renamed aside.
pub(super) fn set_journal_aside(path: &Path) -> Result<(), OpenError> {
    let journal = sibling(path, "-journal");
    let len = match std::fs::metadata(&journal) {
        Ok(found) => found.len(),
        Err(missing) if missing.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(other) => return Err(failed(path, "-journal")(other)),
    };
    if len > 0 && path.try_exists().map_err(failed(path, "-journal"))? {
        let kept = aside_name(path, "-journal")?;
        std::fs::copy(&journal, &kept).map_err(|e| busy_or(e, failed(path, "-journal")))?;
        probe_writer(path)?;
    }
    set_aside(path, "-journal")
}

/// Fails with [`OpenError::WriterBusy`] when another connection holds the file's write lock:
/// `BEGIN IMMEDIATE`, with no busy wait, on a connection that cannot create the file, rolled back
/// at once. It writes nothing: no page changes.
fn probe_writer(path: &Path) -> Result<(), OpenError> {
    let probe = |source| OpenError::Sqlite {
        step: Step::Probe,
        source,
    };
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

/// A rename or copy Windows refused because another process holds the file (a live journal) is
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
