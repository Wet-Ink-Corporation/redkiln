//! Whether the `weighin` Bench found can take a park (decision D3): it must fold decision
//! requests (`requests: true` in `weighin status --json`, weighin.status/1) and keep the store
//! Bench writes. Otherwise a request would sit where no installed Weigh-In shows it, so the
//! park writes nothing and the orchestrator rules instead.

use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::Deserialize;
use weighin_contract::status::{SCHEMA, Status};

use super::{BridgeError, Call, Runner, run_ok};
use crate::error_chain;

/// How long `weighin status --json` may take.
pub(crate) const STATUS_TIMEOUT: Duration = Duration::from_secs(10);

/// Why weigh-in cannot take a park.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Unable {
    /// `weighin` is not there, could not run, failed or wrote too much.
    #[error(transparent)]
    Bridge(#[from] BridgeError),
    /// Its status is not `weighin.status/1`'s envelope.
    #[error("weighin status --json did not read: {0}")]
    Status(String),
    /// It is a build from before decision requests.
    #[error("this weighin ({version}) does not fold decision requests")]
    NoRequests {
        /// The version it reports.
        version: String,
    },
    /// It keeps another store, or cannot locate one.
    #[error("weighin keeps another store ({theirs}) than bench ({ours})")]
    OtherStore {
        /// The store it reports, or `none`.
        theirs: String,
        /// Bench's.
        ours: String,
    },
}

/// The envelope keys `able` reads; the rest (CLI-01's) are not its business.
#[derive(Debug, Deserialize)]
struct Envelope {
    schema: String,
    data: serde_json::Value,
}

/// Checks that the `weighin` at `bin`, run in `cwd` within [`STATUS_TIMEOUT`], folds decision
/// requests into the store at `ours`.
///
/// # Errors
/// [`Unable`] naming the first thing that is not so.
pub(crate) fn able(
    runner: &impl Runner,
    bin: &Path,
    cwd: &Path,
    ours: &Path,
) -> Result<(), Unable> {
    let call = Call {
        program: bin,
        args: &["status", "--json"],
        input: &[],
        cwd,
        timeout: STATUS_TIMEOUT,
    };
    let stdout = run_ok(runner, &call)?;
    let envelope: Envelope = serde_json::from_slice(&stdout)
        .map_err(|error| Unable::Status(error_chain::reason(&error)))?;
    if envelope.schema != SCHEMA {
        return Err(Unable::Status(format!(
            "schema {} is not {SCHEMA}",
            envelope.schema
        )));
    }
    let status = Status::from_envelope_data(&envelope.data)
        .map_err(|error| Unable::Status(error_chain::reason(&error)))?;
    if !status.requests {
        return Err(Unable::NoRequests {
            version: status.version,
        });
    }
    match status.store {
        Some(theirs) if same_file(Path::new(&theirs), ours) => Ok(()),
        theirs => Err(Unable::OtherStore {
            theirs: theirs.unwrap_or_else(|| "none".to_owned()),
            ours: ours.display().to_string(),
        }),
    }
}

/// Whether `a` and `b` name one file: their parent directories resolved, then the file name
/// compared (the store may not exist yet), else the paths as written.
fn same_file(a: &Path, b: &Path) -> bool {
    match (resolved(a), resolved(b)) {
        (Some(a), Some(b)) => a == b,
        _ => a == b,
    }
}

/// `path`'s parent resolved through the file system, joined with its file name.
fn resolved(path: &Path) -> Option<PathBuf> {
    let name = path.file_name()?;
    let parent = path.parent()?.canonicalize().ok()?;
    Some(parent.join(name))
}

#[cfg(test)]
mod tests;
