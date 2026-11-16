//! `counts.json`: how many answered decisions each Clef point has had, beside the decision log
//! (`{"kind", "version", "answered": {"D1": n, ...}}`). Clef's own state, keyed by its
//! [`Point`], so it stays Bench's while the log itself is `redkiln_calibrate`'s.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::baseline;
use crate::decide::log::LogError;
use crate::decide::point::Point;

/// The counts file inside the log's directory.
const COUNTS_FILE: &str = "counts.json";

/// The counts file's `kind`, so another file of the same name is not overwritten.
pub(crate) const COUNTS_KIND: &str = "anneal-decision-counts";

/// The counts file's format version.
pub(crate) const COUNTS_VERSION: u32 = 1;

/// The file as written.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct CountsFile {
    kind: String,
    version: u32,
    answered: BTreeMap<Point, u64>,
}

/// The counts file in the log directory `dir`.
pub(crate) fn path(dir: &Path) -> PathBuf {
    dir.join(COUNTS_FILE)
}

/// The counts at `path`, or `None` when there is no file.
///
/// # Errors
/// [`LogError::Counts`] when the file cannot be read or is not a regular file;
/// [`LogError::CountsFormat`] or [`LogError::CountsForeign`] when it is not ours.
pub(crate) fn read(path: &Path) -> Result<Option<BTreeMap<Point, u64>>, LogError> {
    let Some(bytes) = baseline::read_state_bytes(path).map_err(LogError::Counts)? else {
        return Ok(None);
    };
    let file: CountsFile = match serde_json::from_slice(&bytes) {
        Ok(file) => file,
        Err(source) => {
            return Err(LogError::CountsFormat {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    if file.kind != COUNTS_KIND || file.version != COUNTS_VERSION {
        return Err(LogError::CountsForeign {
            path: path.to_path_buf(),
        });
    }
    Ok(Some(file.answered))
}

/// Writes `answered` to `path`.
///
/// # Errors
/// [`LogError::Counts`] when the file cannot be written.
pub(crate) fn write(path: &Path, answered: BTreeMap<Point, u64>) -> Result<(), LogError> {
    let file = CountsFile {
        kind: COUNTS_KIND.to_owned(),
        version: COUNTS_VERSION,
        answered,
    };
    baseline::write_snapshot_json(path, &file).map_err(LogError::Counts)
}
