//! Reading an old plugin ledger without touching it (SPEC ST-15, ST-16).
//!
//! weighin 0.1.x to 1.x kept their ledger at `<git-common-dir>/weighin/ledger.sqlite3`: a
//! happenstance store stamped 0 (0.1.x in WAL, 0.2.x under TRUNCATE) or 1 (later releases).
//! The import reads it while a 1.x binary may still use it, so the file is never opened in
//! place: opening it could convert its journal mode, replay or delete a `-wal`, or leave a
//! journal beside it, and a stamp would make 1.x refuse it as newer. [`read_ledger`] copies the
//! file (and its `-wal`, if any) to the local temporary disk and reads the copy.
//!
//! The copy is consistent when the source did not change while it was taken: the header's
//! change counter, and the `-wal`'s length, modification time and header (checkpoint sequence
//! and salts) are the same before and after, and no `-journal` holds an unfinished commit.
//! Otherwise it is taken again, at most [`ATTEMPTS`] times.

use std::fs::{self, File};
use std::io::{self, Read};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use happenstance_core::{Query, ReadOptions, SendEventStore, SequencePosition, StoreId, collect};
use happenstance_sqlite::event_store::{SqliteEventStore, SqliteEventStoreError};
use happenstance_sqlite::rusqlite::{self, Connection};

use crate::store::{
    Driver, Found, OpenError, STORE_SCHEMA, StoreError, Stored, read_header, sibling,
};

/// How many times a copy is taken before the source is reported as changing under it.
pub const ATTEMPTS: usize = 3;

/// The largest ledger, file and `-wal` together, that is read (H-13).
pub const MAX_LEDGER_BYTES: u64 = 256 * 1024 * 1024;

/// The pause between copies of a ledger that changed while it was copied.
const RETRY_PAUSE: Duration = Duration::from_millis(50);

/// How many events one read of the copy returns.
const PAGE: usize = 512;

/// The WAL header's first 24 bytes, which the fingerprint keeps: magic, format version, page
/// size, then the checkpoint sequence and both salts (offsets 12 to 24), which a checkpoint or
/// a WAL reset changes.
const WAL_HEADER_LEN: usize = 24;

/// An old ledger as read: its identity, its stamp and every event, in position order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LegacyLedger {
    /// The store id the ledger minted its events under.
    pub store_id: StoreId,
    /// Its `user_version` (0 or 1 for weighin).
    pub user_version: i64,
    /// Its events, from position 1.
    pub events: Vec<Stored>,
}

/// Why an old ledger could not be read. The ledger itself is never changed.
#[derive(Debug, thiserror::Error)]
pub enum LegacyError {
    /// There is no file at the path.
    #[error("no ledger at {}", path.display())]
    Missing {
        /// The path that was looked at.
        path: PathBuf,
    },
    /// The file is not a SQLite database (empty, short, or without SQLite's magic).
    #[error("{} is not a SQLite database", path.display())]
    NotADatabase {
        /// The file.
        path: PathBuf,
    },
    /// The file is a Redkiln store (stamped 1000 or more), not an old ledger.
    #[error("{} is a Redkiln store (schema {found}), not an old ledger", path.display())]
    CoreStore {
        /// The file.
        path: PathBuf,
        /// Its stamp.
        found: i64,
    },
    /// The ledger and its `-wal` are larger than [`MAX_LEDGER_BYTES`].
    #[error("the ledger is more than the {MAX_LEDGER_BYTES} bytes read")]
    TooLarge,
    /// Every attempt found a non-empty `-journal` beside the ledger: a commit is in progress, or
    /// a crashed 1.x binary left one that its next open will roll back.
    #[error("the ledger has a hot journal beside it; run weighin 1.x once, or wait, and retry")]
    HotJournal,
    /// The ledger changed during every one of [`ATTEMPTS`] copies.
    #[error("the ledger kept changing while it was copied ({ATTEMPTS} attempts)")]
    KeptChanging,
    /// Reading the source or writing the copy failed.
    #[error("copying the ledger")]
    Io(#[source] io::Error),
    /// The header could not be read.
    #[error("reading the ledger's header")]
    Header(#[source] OpenError),
    /// SQLite could not open the copy or read its stamp.
    #[error("SQLite could not open the copy")]
    OpenCopy(#[source] rusqlite::Error),
    /// The copy is not a happenstance store, or its identity cannot be read.
    #[error("the ledger is not a happenstance store")]
    NotAStore(#[source] SqliteEventStoreError),
    /// Reading its events failed.
    #[error("reading the ledger's events")]
    Read(#[source] StoreError),
    /// Called inside a tokio runtime context, where the reader's `block_on` cannot run. It is
    /// refused before anything is copied.
    #[error("the ledger reader cannot run inside an async runtime; call it from a blocking thread")]
    InsideRuntime,
    /// The runtime that drives happenstance could not start.
    #[error("starting the async runtime")]
    Runtime(#[source] io::Error),
}

/// What says whether the source moved while it was copied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Fingerprint {
    change_counter: u32,
    wal: Option<WalFingerprint>,
    journal_empty: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct WalFingerprint {
    len: u64,
    modified: SystemTime,
    header: [u8; WAL_HEADER_LEN],
}

/// Reads every event of the old ledger at `path` from a copy on the temporary disk. The file,
/// and every file beside it, are left as they were.
///
/// # Errors
/// [`LegacyError`]: a call inside a tokio runtime context ([`LegacyError::InsideRuntime`],
/// before anything is copied), a missing or non-SQLite file, a Core store, a ledger over
/// [`MAX_LEDGER_BYTES`], a hot journal, one that changed under every copy, or a copy SQLite or
/// happenstance cannot read.
pub fn read_ledger(path: &Path) -> Result<LegacyLedger, LegacyError> {
    read_ledger_with(path, || {})
}

/// [`read_ledger`], calling `between` after each copy and before the source is looked at again,
/// so a test can change the source while it is being copied.
fn read_ledger_with(path: &Path, mut between: impl FnMut()) -> Result<LegacyLedger, LegacyError> {
    if crate::store::inside_runtime() {
        return Err(LegacyError::InsideRuntime);
    }
    let tmp = tempfile::tempdir().map_err(LegacyError::Io)?;
    let copy = tmp.path().join("ledger.sqlite3");
    let mut hot = 0;
    for attempt in 1..=ATTEMPTS {
        let before = fingerprint(path)?;
        if before.journal_empty {
            let copied = take_copy(path, &copy, before)?;
            between();
            if copied == Copied::Taken && fingerprint(path)? == before {
                let ledger = read_copy(&copy)?;
                tmp.close().map_err(LegacyError::Io)?;
                return Ok(ledger);
            }
        } else {
            hot += 1;
        }
        if attempt < ATTEMPTS {
            std::thread::sleep(RETRY_PAUSE);
        }
    }
    Err(if hot == ATTEMPTS {
        LegacyError::HotJournal
    } else {
        LegacyError::KeptChanging
    })
}

fn fingerprint(path: &Path) -> Result<Fingerprint, LegacyError> {
    let header = match read_header(path).map_err(LegacyError::Header)? {
        Found::Sqlite(header) => header,
        Found::Missing => {
            return Err(LegacyError::Missing {
                path: path.to_path_buf(),
            });
        }
        Found::Empty | Found::NotSqlite => {
            return Err(LegacyError::NotADatabase {
                path: path.to_path_buf(),
            });
        }
    };
    if header.user_version >= STORE_SCHEMA {
        return Err(LegacyError::CoreStore {
            path: path.to_path_buf(),
            found: header.user_version,
        });
    }
    let journal_empty = match fs::metadata(sibling(path, "-journal")) {
        Ok(meta) => meta.len() == 0,
        Err(missing) if missing.kind() == io::ErrorKind::NotFound => true,
        Err(other) => return Err(LegacyError::Io(other)),
    };
    Ok(Fingerprint {
        change_counter: header.change_counter,
        wal: wal_fingerprint(&sibling(path, "-wal"))?,
        journal_empty,
    })
}

fn wal_fingerprint(wal: &Path) -> Result<Option<WalFingerprint>, LegacyError> {
    let mut file = match File::open(wal) {
        Ok(file) => file,
        Err(missing) if missing.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(other) => return Err(LegacyError::Io(other)),
    };
    let meta = file.metadata().map_err(LegacyError::Io)?;
    let mut header = [0_u8; WAL_HEADER_LEN];
    // A WAL shorter than its header (empty after a reset) fingerprints as zeros past its end.
    let mut filled = 0;
    while let Some(rest) = header.get_mut(filled..).filter(|r| !r.is_empty()) {
        match file.read(rest).map_err(LegacyError::Io)? {
            0 => break,
            n => filled = filled.saturating_add(n),
        }
    }
    Ok(Some(WalFingerprint {
        len: meta.len(),
        modified: meta.modified().map_err(LegacyError::Io)?,
        header,
    }))
}

/// Copies the ledger and its `-wal` (when there is one) beside `copy`, replacing an earlier
/// attempt's files, reading at most [`MAX_LEDGER_BYTES`] in all.
fn take_copy(path: &Path, copy: &Path, before: Fingerprint) -> Result<Copied, LegacyError> {
    let mut budget = MAX_LEDGER_BYTES;
    copy_capped(path, copy, &mut budget)?;
    let copied_wal = sibling(copy, "-wal");
    if before.wal.is_some() {
        match copy_capped(&sibling(path, "-wal"), &copied_wal, &mut budget) {
            Ok(()) => {}
            // A WAL ledger's last connection removes its `-wal` as it closes.
            Err(LegacyError::Io(gone)) if gone.kind() == io::ErrorKind::NotFound => {
                return Ok(Copied::SourceChanged);
            }
            Err(other) => return Err(other),
        }
    } else if copied_wal.try_exists().map_err(LegacyError::Io)? {
        // Our own temporary directory, never beside the ledger.
        fs::remove_file(&copied_wal).map_err(LegacyError::Io)?;
    }
    Ok(Copied::Taken)
}

/// What one copy of the ledger came to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Copied {
    /// The files fingerprinted were copied; the fingerprint, taken again, says if they held still.
    Taken,
    /// A file fingerprinted was gone by the time it was copied: the source changed.
    SourceChanged,
}

/// Copies `from` to `to`, spending its length from `budget`, and refuses a file that would
/// overspend it (it may have grown since it was measured).
fn copy_capped(from: &Path, to: &Path, budget: &mut u64) -> Result<(), LegacyError> {
    let source = File::open(from).map_err(LegacyError::Io)?;
    let mut target = File::create(to).map_err(LegacyError::Io)?;
    let copied = io::copy(&mut source.take(budget.saturating_add(1)), &mut target)
        .map_err(LegacyError::Io)?;
    *budget = budget.checked_sub(copied).ok_or(LegacyError::TooLarge)?;
    Ok(())
}

fn read_copy(copy: &Path) -> Result<LegacyLedger, LegacyError> {
    let conn = Connection::open(copy).map_err(LegacyError::OpenCopy)?;
    let user_version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(LegacyError::OpenCopy)?;
    let store = SqliteEventStore::new(conn).map_err(LegacyError::NotAStore)?;
    let driver = Driver::new().map_err(LegacyError::Runtime)?;
    let page = NonZeroUsize::new(PAGE).unwrap_or(NonZeroUsize::MIN);
    let mut events: Vec<Stored> = Vec::new();
    loop {
        let after = events.last().map_or(0, |e| e.position);
        let Some(from) = after.checked_add(1).and_then(SequencePosition::new) else {
            break;
        };
        let options = ReadOptions::new().from(from).limit(page.get());
        let read = driver
            .block_on(collect(store.read(&Query::all(), options)))
            .map_err(LegacyError::Read)?
            .map_err(|e| LegacyError::Read(StoreError::Read(e)))?;
        let full = read.len() == page.get();
        events.extend(read.into_iter().map(Stored::from_sequenced));
        if !full {
            break;
        }
    }
    Ok(LegacyLedger {
        store_id: store.store_id(),
        user_version,
        events,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(name: &str, dir: &Path) -> PathBuf {
        let from = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name);
        let to = dir.join(name);
        fs::copy(from, &to).expect("copy the fixture");
        to
    }

    /// Commits one more event to the ledger at `path` through a plain connection.
    fn commit_one(conn: &Connection) {
        conn.execute(
            "INSERT INTO event (event_type, data, tags, origin_store, origin_position, recorded_at)
             SELECT event_type, data, tags, origin_store,
                    (SELECT max(position) + 1 FROM event), recorded_at
             FROM event WHERE position = 1",
            [],
        )
        .expect("commit an event");
    }

    #[test]
    fn a_commit_to_a_rollback_ledger_moves_its_fingerprint() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = fixture("weighin-0.2.2.sqlite3", dir.path());
        let before = fingerprint(&path).expect("fingerprint");
        let conn = Connection::open(&path).expect("open");
        conn.query_row("PRAGMA journal_mode = truncate", [], |r| {
            r.get::<_, String>(0)
        })
        .expect("truncate");
        commit_one(&conn);
        let after = fingerprint(&path).expect("fingerprint");
        assert_ne!(after.change_counter, before.change_counter);
        assert_ne!(after, before);
    }

    #[test]
    fn a_frame_in_the_wal_moves_its_fingerprint() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = fixture("weighin-0.1.x.sqlite3", dir.path());
        let conn = Connection::open(&path).expect("open");
        conn.execute_batch("PRAGMA wal_autocheckpoint = 0;")
            .expect("no checkpoint");
        commit_one(&conn);
        let before = fingerprint(&path).expect("fingerprint");
        assert!(before.wal.is_some(), "the commit is in the WAL");
        commit_one(&conn);
        let after = fingerprint(&path).expect("fingerprint");
        assert_eq!(
            after.change_counter, before.change_counter,
            "a WAL commit leaves the header"
        );
        assert_ne!(after.wal.map(|w| w.len), before.wal.map(|w| w.len));
    }

    #[test]
    fn a_change_during_one_copy_takes_another() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = fixture("weighin-0.2.2.sqlite3", dir.path());
        let conn = Connection::open(&path).expect("open");
        conn.query_row("PRAGMA journal_mode = truncate", [], |r| {
            r.get::<_, String>(0)
        })
        .expect("truncate");
        let mut copies = 0;
        let ledger = read_ledger_with(&path, || {
            copies += 1;
            if copies == 1 {
                commit_one(&conn);
            }
        })
        .expect("the second copy is consistent");
        assert_eq!(copies, 2, "the first copy was taken again");
        assert_eq!(ledger.events.len(), 4, "the second copy holds the commit");
    }

    #[test]
    fn a_ledger_changing_under_every_copy_is_reported() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = fixture("weighin-0.2.2.sqlite3", dir.path());
        let conn = Connection::open(&path).expect("open");
        conn.query_row("PRAGMA journal_mode = truncate", [], |r| {
            r.get::<_, String>(0)
        })
        .expect("truncate");
        let mut copies = 0;
        let result = read_ledger_with(&path, || {
            copies += 1;
            commit_one(&conn);
        });
        assert!(
            matches!(result, Err(LegacyError::KeptChanging)),
            "{result:?}"
        );
        assert_eq!(copies, ATTEMPTS);
    }

    #[test]
    fn a_hot_journal_is_named() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = fixture("weighin-0.2.2.sqlite3", dir.path());
        fs::write(sibling(&path, "-journal"), b"a hot journal").expect("write");
        let mut copies = 0;
        let result = read_ledger_with(&path, || copies += 1);
        assert!(matches!(result, Err(LegacyError::HotJournal)), "{result:?}");
        assert_eq!(copies, 0, "no copy is taken beside a hot journal");
    }

    #[test]
    fn a_wal_that_vanishes_before_the_copy_is_a_change_not_an_error() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = fixture("weighin-0.1.x.sqlite3", dir.path());
        let conn = Connection::open(&path).expect("open");
        conn.execute_batch("PRAGMA wal_autocheckpoint = 0;")
            .expect("no checkpoint");
        commit_one(&conn);
        let before = fingerprint(&path).expect("fingerprint");
        assert!(before.wal.is_some(), "the commit is in the WAL");
        // The last connection of a WAL ledger closing checkpoints and removes its `-wal`.
        drop(conn);
        assert!(
            !sibling(&path, "-wal").exists(),
            "the WAL went with its connection"
        );
        let copy = tempfile::tempdir().expect("tempdir");
        let taken = take_copy(&path, &copy.path().join("copy.sqlite3"), before);
        assert!(matches!(taken, Ok(Copied::SourceChanged)), "{taken:?}");
    }

    #[test]
    fn a_read_inside_an_async_runtime_is_refused_before_any_copy() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = fixture("weighin-0.2.2.sqlite3", dir.path());
        let runtime = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("runtime");
        let mut copies = 0;
        let result = runtime.block_on(async { read_ledger_with(&path, || copies += 1) });
        assert!(
            matches!(result, Err(LegacyError::InsideRuntime)),
            "{result:?}"
        );
        assert_eq!(copies, 0, "nothing is copied inside a runtime");
    }

    #[test]
    fn a_file_over_the_budget_is_refused() {
        let dir = tempfile::tempdir().expect("tempdir");
        let from = dir.path().join("from");
        fs::write(&from, [0_u8; 10]).expect("write");
        let mut budget = 9;
        let result = copy_capped(&from, &dir.path().join("to"), &mut budget);
        assert!(matches!(result, Err(LegacyError::TooLarge)), "{result:?}");
        let mut budget = 10;
        assert!(matches!(
            copy_capped(&from, &dir.path().join("to"), &mut budget),
            Ok(())
        ));
        assert_eq!(budget, 0);
    }
}
