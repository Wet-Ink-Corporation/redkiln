//! The `lint skills --cache` snapshot file: read within a cap, written atomically.
//!
//! A copy of the two snapshot functions of Bench's `baseline` module and what they need, under
//! the same name so the cache code reads like Bench's.

use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde::Serialize;

/// Snapshot files larger than this are refused rather than read into memory (H-13).
pub const MAX_SNAPSHOT_BYTES: u64 = 16 * 1024 * 1024;

/// Why a snapshot could not be loaded or saved.
#[derive(Debug, thiserror::Error)]
pub enum BaselineError {
    /// The file could not be read or written.
    #[error("{op} {path}", path = path.display())]
    Io {
        /// `"reading"` or `"writing"`.
        op: &'static str,
        /// The snapshot path.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: io::Error,
    },
    /// The file is larger than [`MAX_SNAPSHOT_BYTES`].
    #[error("{path} is larger than {MAX_SNAPSHOT_BYTES} bytes", path = path.display())]
    TooLarge {
        /// The snapshot path.
        path: PathBuf,
    },
    /// The file is not a snapshot this version understands.
    #[error("{path} is not a valid baseline snapshot", path = path.display())]
    Format {
        /// The snapshot path.
        path: PathBuf,
        /// The parse error.
        #[source]
        source: serde_json::Error,
    },
}

/// Reads the snapshot at `path` whole: `Ok(None)` when it does not exist.
///
/// # Errors
/// [`BaselineError::Io`] when it cannot be read; [`BaselineError::TooLarge`] when it is larger
/// than [`MAX_SNAPSHOT_BYTES`].
pub(crate) fn read_snapshot_bytes(path: &Path) -> Result<Option<Vec<u8>>, BaselineError> {
    let io_error = |source| BaselineError::Io {
        op: "reading",
        path: path.to_path_buf(),
        source,
    };
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_error(error)),
    };
    let mut bytes = Vec::new();
    file.take(MAX_SNAPSHOT_BYTES.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if !u64::try_from(bytes.len()).is_ok_and(|len| len <= MAX_SNAPSHOT_BYTES) {
        return Err(BaselineError::TooLarge {
            path: path.to_path_buf(),
        });
    }
    Ok(Some(bytes))
}

/// Writes `value` as pretty JSON with a final newline to `path`, creating its parent
/// directory, through [`write_atomically`], so a reader never sees a half-written snapshot.
///
/// # Errors
/// [`BaselineError::Io`] when the directory or file cannot be written.
pub(crate) fn write_snapshot_json<T: Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), BaselineError> {
    let io_error = |source| BaselineError::Io {
        op: "writing",
        path: path.to_path_buf(),
        source,
    };
    let mut json = serde_json::to_vec_pretty(value).map_err(|error| io_error(error.into()))?;
    json.push(b'\n');
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        fs::create_dir_all(parent).map_err(io_error)?;
    }
    write_atomically(path, &json).map_err(io_error)
}

/// Distinguishes concurrent writers' temporary files within one process.
static NEXT_TEMPORARY: AtomicU64 = AtomicU64::new(0);

/// How many fresh temporary names a save tries before giving up.
const MAX_TEMPORARY_ATTEMPTS: u32 = 8;

/// `<path>.<pid>.<n>.tmp`, beside `path`, with `n` new on every call in this process.
fn temporary_path(path: &Path) -> PathBuf {
    // Relaxed: the counter only has to hand out distinct numbers; it orders no other memory.
    let n = NEXT_TEMPORARY.fetch_add(1, Ordering::Relaxed);
    let mut temporary = path.as_os_str().to_owned();
    temporary.push(format!(".{}.{n}.tmp", std::process::id()));
    PathBuf::from(temporary)
}

/// Creates a temporary file that did not exist before (`create_new`), so two writers can never
/// share one; a name that is taken (say, left by a crashed process with a recycled pid) is
/// skipped for the next.
fn create_temporary(path: &Path) -> io::Result<(PathBuf, File)> {
    for _ in 0..MAX_TEMPORARY_ATTEMPTS {
        let temporary = temporary_path(path);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => return Ok((temporary, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!(
            "no free temporary name after {MAX_TEMPORARY_ATTEMPTS} attempts; \
             remove stale temporary files (*.tmp) beside the snapshot"
        ),
    ))
}

/// Writes `bytes` to a temporary file unique to this writer (`<path>.<pid>.<n>.tmp`), syncs it,
/// and renames it over `path`, so readers see the old file or the new one, never a mix. On
/// failure the temporary file is removed.
fn write_atomically(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let (temporary, mut file) = create_temporary(path)?;
    // The file is closed (dropped) before any rename, which Windows needs.
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    written
        .and_then(|()| fs::rename(&temporary, path))
        .map_err(|error| discard(&temporary, error))
}

/// Removes the staged `temporary` after `error`, returning `error` (with the removal's own
/// failure added, if any).
fn discard(temporary: &Path, error: io::Error) -> io::Error {
    match fs::remove_file(temporary) {
        Ok(()) => error,
        Err(cleanup) if cleanup.kind() == io::ErrorKind::NotFound => error,
        Err(cleanup) => io::Error::new(
            error.kind(),
            format!(
                "{error}; removing {} also failed: {cleanup}",
                temporary.display()
            ),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::{BaselineError, MAX_SNAPSHOT_BYTES, read_snapshot_bytes, write_snapshot_json};

    #[test]
    fn a_written_snapshot_reads_back_and_a_missing_one_is_none() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("nested/snapshot.json");

        let missing = read_snapshot_bytes(&path).expect("missing is no error");
        write_snapshot_json(&path, &serde_json::json!({"a": 1})).expect("writes");
        let bytes = read_snapshot_bytes(&path).expect("reads");

        assert!(missing.is_none(), "missing");
        assert_eq!(
            bytes.as_deref(),
            Some(&b"{\n  \"a\": 1\n}\n"[..]),
            "pretty JSON, final newline"
        );
        let left: Vec<_> = std::fs::read_dir(path.parent().expect("a parent"))
            .expect("listable")
            .collect();
        assert_eq!(left.len(), 1, "no temporary file left beside it");
    }

    #[test]
    fn a_snapshot_over_the_cap_is_refused() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("big.json");
        let file = std::fs::File::create(&path).expect("creates");
        file.set_len(MAX_SNAPSHOT_BYTES + 1).expect("grows");

        let result = read_snapshot_bytes(&path);

        assert!(
            matches!(result, Err(BaselineError::TooLarge { .. })),
            "{result:?}"
        );
    }
}
