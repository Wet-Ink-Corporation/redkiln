//! The Redkiln guard kit: what a guard needs to read a Bash command before it runs, with no
//! I/O and no harness protocol.
//!
//! - [`shell`]: a hand-written POSIX-ish lexer ([`shell::parse`]) that lists every simple
//!   command a Bash text runs, nested ones included, each word with its quoting and its
//!   [`shell::Shape`]; anything it cannot read is a [`shell::ParseError`], which a guard denies.
//! - [`glob`]: repository path globs ([`glob::Glob`]: `**`, `*`, `?`, ASCII case-insensitive),
//!   the protected and sensitive path patterns.
//!
//! Every input is bounded: nesting by [`shell::MAX_NESTING`], a pattern at 256 bytes.

pub mod glob;
pub mod shell;
