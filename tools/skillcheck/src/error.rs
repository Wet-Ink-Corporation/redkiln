//! Errors that stop a command before it produces a result, each with its exit code.
//!
//! A command's own outcomes (findings, a missed bound) are not errors: they are an
//! [`Envelope`](crate::envelope::Envelope) with a per-command code.

use std::io;

use crate::eval::routing::RoutingError;
use crate::eval::uplift::UpliftError;
use crate::exit_codes::{self, Code};
use crate::lint::LintError;

/// Why `skillcheck` could not produce a result.
#[derive(Debug, thiserror::Error)]
pub enum CliError {
    /// The command line was invalid. `reason` is one line.
    #[error("{reason}")]
    Usage {
        /// What was wrong with the arguments.
        reason: String,
    },
    /// `lint skills` could not check its inputs.
    #[error("lint")]
    Lint(#[from] LintError),
    /// `eval routing` could not read its inputs or run its first batch.
    #[error("eval")]
    Routing(#[from] RoutingError),
    /// `eval uplift` could not read its inputs or run its first batch.
    #[error("eval")]
    Uplift(#[from] UpliftError),
    /// stdout or stderr could not be written.
    #[error("writing output")]
    Output(#[source] io::Error),
}

impl CliError {
    /// The documented exit code for this error. Never 0.
    #[must_use]
    pub const fn code(&self) -> Code {
        match self {
            Self::Usage { .. } => exit_codes::USAGE,
            Self::Lint(error) => error.code(),
            Self::Routing(error) => error.code(),
            Self::Uplift(error) => error.code(),
            Self::Output(_) => exit_codes::OUTPUT,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::CliError;
    use std::io;

    #[test]
    fn usage_and_output_errors_map_to_their_global_codes() {
        let usage = CliError::Usage {
            reason: "unknown argument: x".to_owned(),
        };
        let output = CliError::Output(io::Error::other("closed"));

        assert_eq!(usage.code().value(), 2, "{usage:?}");
        assert_eq!(output.code().value(), 74, "{output:?}");
    }
}
