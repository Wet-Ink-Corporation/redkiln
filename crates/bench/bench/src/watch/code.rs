//! The exit code of a target `watch` cannot read: the kit's [`WatchError`] carries no code, so
//! Bench gives it the one it had (7, `exit_codes::WATCH_QUERY`).

use redkiln_github::watch::WatchError;

use crate::exit_codes::{self, Code};

/// The exit code of `error`: 7 for every query error. Exhaustive, so a new variant has to
/// choose. A `const fn`, so `CliError::code` stays one.
pub(crate) const fn exit_code(error: &WatchError) -> Code {
    match error {
        WatchError::Gh(_)
        | WatchError::Threads { .. }
        | WatchError::Json { .. }
        | WatchError::Closed { .. }
        | WatchError::Missing { .. }
        | WatchError::BadRef { .. }
        | WatchError::TooMany { .. } => exit_codes::WATCH_QUERY,
    }
}

/// [`WatchError`]'s exit code as a method, as it was before the error moved to the kit, so the
/// tests that assert it read unchanged.
#[cfg(test)]
pub(crate) trait WatchCode {
    /// The exit code ([`exit_code`]).
    fn code(&self) -> Code;
}

#[cfg(test)]
impl WatchCode for WatchError {
    fn code(&self) -> Code {
        exit_code(self)
    }
}

#[cfg(test)]
mod tests;
