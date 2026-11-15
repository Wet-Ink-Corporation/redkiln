//! Where relative paths start once a `cd` has run: the engine follows `cd`, `pushd` and
//! `popd`, and hands packs the result.

use super::wrappers::skip_options;
use redkiln_guard::shell::{Shape, Word};

/// Where relative paths start, as far as the `cd` commands seen so far say.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Cwd {
    /// No `cd` yet: the hook's working directory.
    Start,
    /// A directory a `cd` named (relative to the start, or absolute), and its shape.
    Dir(String, Shape),
    /// A `cd` to a variable or to `-` (the previous directory): anywhere.
    Unknown,
}

impl Cwd {
    /// Follows `cd <args>` (or `pushd`); with no directory it goes home, `~`. The directory is
    /// copied: it outlives the command.
    pub(crate) fn change(&mut self, args: &[Word]) {
        let Some(dir) = skip_options(args, &[]).first() else {
            *self = Self::Dir("~".to_owned(), Shape::Pattern);
            return;
        };
        // An unquoted leading `~` is the home directory, which is absolute.
        let rooted = is_absolute(&dir.text) || (dir.text.starts_with('~') && !dir.is_literal());
        *self = match &*self {
            _ if dir.shape == Shape::Dynamic || dir.text == "-" => Self::Unknown,
            Self::Dir(base, shape) if !rooted => {
                Self::Dir(format!("{base}/{}", dir.text), dir.shape.max(*shape))
            }
            Self::Unknown if !rooted => Self::Unknown,
            Self::Start | Self::Dir(..) | Self::Unknown => Self::Dir(dir.text.clone(), dir.shape),
        };
    }
}

/// Whether `path` is absolute: rooted at `/` or `\`, or led by a drive letter.
pub(crate) fn is_absolute(path: &str) -> bool {
    path.starts_with(['/', '\\']) || path.get(1..2) == Some(":")
}
