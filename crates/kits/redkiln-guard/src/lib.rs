//! The Redkiln guard kit: what a guard needs to read a Bash command before it runs, with no
//! I/O and no harness protocol.
//!
//! - [`shell`]: a hand-written POSIX-ish lexer ([`shell::parse`]) that lists every simple
//!   command a Bash text runs, nested ones included, each word with its quoting and its
//!   [`shell::Shape`]; anything it cannot read is a [`shell::ParseError`], which a guard denies.
//! - [`engine`]: the denylist engine ([`engine::check`]), which unwraps every simple command,
//!   reads the shell text wrappers run, follows variables and `cd`, and asks a rule pack
//!   ([`engine::Pack`]) what may not run. It denies or leaves undecided, never allows.
//! - [`protect`]: whether a path a command writes or removes is one of a pack's protected
//!   paths ([`protect::ProtectedPaths`]).
//! - [`redact`]: the command as an allowlist skeleton ([`redact::redact`]), every argument
//!   value hidden unless a fixed table names it, read through the engine's own walk.
//! - [`glob`]: repository path globs ([`glob::Glob`]: `**`, `*`, `?`, ASCII case-insensitive),
//!   the protected and sensitive path patterns.
//!
//! Every input is bounded: nesting by [`shell::MAX_NESTING`] and [`engine::MAX_DEPTH`], lexing
//! by [`engine::MAX_LEXED_MIB`], a pattern at 256 bytes.

pub mod engine;
pub mod glob;
pub mod protect;
pub mod redact;
pub mod shell;
