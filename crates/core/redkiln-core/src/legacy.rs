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
//! change counter, the `-wal`'s length and modification time are the same before and after,
//! and no `-journal` holds an unfinished commit. Otherwise it is taken again, at most
//! [`ATTEMPTS`] times.

use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use happenstance_core::{Query, ReadOptions, SendEventStore, StoreId, collect};
use happenstance_sqlite::event_store::{SqliteEventStore, SqliteEventStoreError};
use happenstance_sqlite::rusqlite::{self, Connection};

use crate::store::{Driver, OpenError, STORE_SCHEMA, StoreError, Stored, read_header, sibling};

/// How many times a copy is taken before the source is reported as changing under it.
pub const ATTEMPTS: usize = 3;

/// The largest ledger, file and `-wal` together, that is read (H-13).
pub const MAX_LEDGER_BYTES: u64 = 256 * 1024 * 1024;

/// The pause between copies of a ledger that changed while it was copied.
const RETRY_PAUSE: Duration = Duration::from_millis(50);

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
    /// The file is not a SQLite database.
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
    #[error("the ledger is {bytes} bytes, more than the {MAX_LEDGER_BYTES} read")]
    TooLarge {
        /// Its size, file and `-wal` together.
        bytes: u64,
    },
    /// The ledger changed during every one of [`ATTEMPTS`] copies.
    #[error("the ledger kept changing while it was copied ({ATTEMPTS} attempts)")]
    KeptChanging,
    /// Reading the source or writing the copy failed.
    #[error("copying the ledger")]
    Io(#[source] std::io::Error),
    /// The header could not be read.
    #[error("reading the ledger's header")]
    Header(#[source] OpenError),
    /// SQLite could not read the copy.
    #[error("SQLite failed on the copy")]
    Sqlite(#[from] rusqlite::Error),
    /// The copy is not a happenstance store, or its identity cannot be read.
    #[error("the ledger is not a happenstance store")]
    NotAStore(#[source] SqliteEventStoreError),
    /// Reading its events failed.
    #[error("reading the ledger's events")]
    Read(#[source] StoreError),
    /// The runtime that drives happenstance could not start.
    #[error("starting the async runtime")]
    Runtime(#[source] OpenError),
}

/// What says whether the source moved while it was copied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Fingerprint {
    change_counter: u32,
    wal: Option<(u64, SystemTime)>,
    journal_empty: bool,
}

/// Reads every event of the old ledger at `path` from a copy on the temporary disk. The file,
/// and every file beside it, are left as they were.
///
/// # Errors
/// [`LegacyError`]: a missing or non-SQLite file, a Core store, a ledger over
/// [`MAX_LEDGER_BYTES`], one that changed under every copy, or a copy SQLite or happenstance
/// cannot read.
pub fn read_ledger(path: &Path) -> Result<LegacyLedger, LegacyError> {
    let tmp = tempfile::tempdir().map_err(LegacyError::Io)?;
    let copy = tmp.path().join("ledger.sqlite3");
    let mut attempts = 0;
    loop {
        let before = fingerprint(path)?;
        if before.journal_empty {
            take_copy(path, &copy, before)?;
            if fingerprint(path)? == before {
                break;
            }
        }
        attempts += 1;
        if attempts >= ATTEMPTS {
            return Err(LegacyError::KeptChanging);
        }
        std::thread::sleep(RETRY_PAUSE);
    }
    let ledger = read_copy(&copy)?;
    tmp.close().map_err(LegacyError::Io)?;
    Ok(ledger)
}

fn fingerprint(path: &Path) -> Result<Fingerprint, LegacyError> {
    let header = read_header(path).map_err(LegacyError::Header)?;
    let Some(header) = header else {
        return Err(if path.exists() {
            LegacyError::NotADatabase {
                path: path.to_path_buf(),
            }
        } else {
            LegacyError::Missing {
                path: path.to_path_buf(),
            }
        });
    };
    if header.user_version >= STORE_SCHEMA {
        return Err(LegacyError::CoreStore {
            path: path.to_path_buf(),
            found: header.user_version,
        });
    }
    let wal = match fs::metadata(sibling(path, "-wal")) {
        Ok(meta) => Some((meta.len(), meta.modified().map_err(LegacyError::Io)?)),
        Err(missing) if missing.kind() == std::io::ErrorKind::NotFound => None,
        Err(other) => return Err(LegacyError::Io(other)),
    };
    let journal_empty = match fs::metadata(sibling(path, "-journal")) {
        Ok(meta) => meta.len() == 0,
        Err(missing) if missing.kind() == std::io::ErrorKind::NotFound => true,
        Err(other) => return Err(LegacyError::Io(other)),
    };
    Ok(Fingerprint {
        change_counter: header.change_counter,
        wal,
        journal_empty,
    })
}

/// Copies the ledger and its `-wal` (when there is one) beside `copy`, replacing an earlier
/// attempt's files.
fn take_copy(path: &Path, copy: &Path, before: Fingerprint) -> Result<(), LegacyError> {
    let size = fs::metadata(path).map_err(LegacyError::Io)?.len();
    let bytes = size.saturating_add(before.wal.map_or(0, |(len, _)| len));
    if bytes > MAX_LEDGER_BYTES {
        return Err(LegacyError::TooLarge { bytes });
    }
    fs::copy(path, copy).map_err(LegacyError::Io)?;
    let copied_wal = sibling(copy, "-wal");
    if before.wal.is_some() {
        fs::copy(sibling(path, "-wal"), &copied_wal).map_err(LegacyError::Io)?;
    } else if copied_wal.exists() {
        fs::remove_file(&copied_wal).map_err(LegacyError::Io)?;
    }
    Ok(())
}

fn read_copy(copy: &Path) -> Result<LegacyLedger, LegacyError> {
    let conn = Connection::open(copy)?;
    let user_version: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    let store = SqliteEventStore::new(conn).map_err(LegacyError::NotAStore)?;
    let driver = Driver::new().map_err(LegacyError::Runtime)?;
    let mut events = Vec::new();
    let page = NonZeroUsize::MIN.saturating_add(511);
    loop {
        let after = events.last().map_or(0, |e: &Stored| e.position);
        let Some(from) = after
            .checked_add(1)
            .and_then(happenstance_core::SequencePosition::new)
        else {
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
