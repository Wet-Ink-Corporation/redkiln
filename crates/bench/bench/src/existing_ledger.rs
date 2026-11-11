//! Opening the store only when it already exists, within a caller's time limits: hooks and
//! read-only commands never create a store, and must not wait on git or a locked store past
//! their limit. Crate-private; the hooks and the read-only commands share it.

use std::ffi::OsString;
use std::io;
use std::path::Path;
use std::time::Duration;

use crate::paths::{self, PathError};
use crate::store::{Store, StoreError, Wait};

/// Why the store could not be opened.
#[derive(Debug, thiserror::Error)]
pub(crate) enum LedgerError {
    /// The working directory, needed to find it, cannot be read.
    #[error("reading the working directory")]
    CurrentDir(#[source] io::Error),
    /// git could not say where it is.
    #[error("finding the ledger")]
    Locate(#[source] PathError),
    /// The file exists but the store cannot open it.
    #[error("opening the ledger")]
    Open(#[source] StoreError),
}

/// The store seen from `cwd` (found as `paths::ledger_store_within` finds it, git given
/// `git_timeout`) when there is one, each write waiting at most `wait` for a lock; `None`,
/// with nothing created, when there is none (Core's `open_existing`: a missing, empty or
/// never-stamped file).
///
/// # Errors
/// The [`LedgerError`] when it cannot be found or opened: an anneal ledger or another
/// program's database at the path is refused, never read as "no store".
pub(crate) fn open_existing(
    cwd: &Path,
    lookup: impl Fn(&str) -> Option<OsString>,
    git_timeout: Duration,
    wait: Wait,
) -> Result<Option<Store>, LedgerError> {
    let location =
        paths::ledger_store_within(None, lookup, cwd, git_timeout).map_err(LedgerError::Locate)?;
    Store::open_existing(&location.path, wait).map_err(LedgerError::Open)
}
