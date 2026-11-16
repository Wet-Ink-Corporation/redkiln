//! The program a simple command runs, as the walk names it to packs.

use crate::shell::Word;

/// The program of a simple command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Program<'a> {
    /// Its name, without a path or `.exe`.
    Named(&'a str),
    /// A word that is not a literal name (`$(echo rm)`, `"$GIT"`, a glob or a brace).
    Unknown(&'a Word),
}
