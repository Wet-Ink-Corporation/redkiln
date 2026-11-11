//! What `bench` says on stderr (SPEC CLI-08): one `bench: error: <message>` or
//! `bench: warning: <message>` line each, the message on one line, an error's holding its cause
//! chain joined by `": "`.

use std::error::Error;
use std::fmt::Display;
use std::io::{self, Write};

use bench_contract::cli::BINARY;

use crate::error_chain::reason;

/// Writes `bench: error: <reason>` and a newline to `err`: `error` and each of its sources,
/// joined by `": "`, on one line.
///
/// # Errors
/// The I/O error when `err` cannot be written.
pub(crate) fn error<W: Write>(err: W, error: &dyn Error) -> io::Result<()> {
    line(err, "error", &reason(error))
}

/// Writes `bench: error: <message>` and a newline to `err`, for a failure already told as text
/// (its cause chain joined), any line break in `message` folded into a space.
///
/// # Errors
/// The I/O error when `err` cannot be written.
pub(crate) fn error_message<W: Write>(err: W, message: &str) -> io::Result<()> {
    line(err, "error", message)
}

/// Writes `bench: warning: <message>` and a newline to `err`, any line break in `message`
/// folded into a space.
///
/// # Errors
/// The I/O error when `err` cannot be written.
pub(crate) fn warning<W: Write>(err: W, message: &dyn Display) -> io::Result<()> {
    line(err, "warning", &message.to_string())
}

/// Writes `bench: <level>: <message>` as one line.
fn line<W: Write>(mut err: W, level: &str, message: &str) -> io::Result<()> {
    let message = message.replace(['\r', '\n'], " ");
    writeln!(err, "{BINARY}: {level}: {message}")
}

#[cfg(test)]
mod tests {
    use super::{error, warning};
    use std::io;

    #[derive(Debug, thiserror::Error)]
    #[error("reading\nthe ledger")]
    struct Outer(#[source] io::Error);

    #[test]
    fn an_error_is_one_line_with_its_cause_chain() {
        let mut err = Vec::new();
        error(&mut err, &Outer(io::Error::other("disk full"))).expect("a Vec accepts writes");
        assert_eq!(
            String::from_utf8(err).expect("UTF-8"),
            "bench: error: reading the ledger: disk full\n"
        );
    }

    #[test]
    fn a_warning_is_one_line() {
        let mut err = Vec::new();
        warning(&mut err, &"the store\nmoved").expect("a Vec accepts writes");
        assert_eq!(
            String::from_utf8(err).expect("UTF-8"),
            "bench: warning: the store moved\n"
        );
    }
}
