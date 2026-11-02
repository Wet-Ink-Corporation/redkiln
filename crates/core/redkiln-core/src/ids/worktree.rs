//! Worktree ids (SPEC ID-06).

use std::fmt;
use std::path::Path;

use sha2::{Digest as _, Sha256};

use super::{IdError, is_lower_hex, to_hex};

/// The hex characters of a worktree id.
const WORKTREE_HEX: usize = 12;

/// The platform whose path rules normalise a top-level path (ID-06).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    /// Linux and other Unix: the path as git printed it.
    Unix,
    /// macOS: ASCII-lowercased.
    MacOs,
    /// Windows: `\` written `/`, and ASCII-lowercased.
    Windows,
}

impl Host {
    /// The platform this build runs on.
    #[must_use]
    pub const fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Unix
        }
    }

    const fn folds_case(self) -> bool {
        matches!(self, Self::MacOs | Self::Windows)
    }
}

/// A worktree id: `wt-` and the first 12 hex characters of the sha256 of the worktree's
/// normalised top-level path (ID-06), so a journal never carries a local absolute path.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct WorktreeId(String);

impl WorktreeId {
    /// The id of the worktree whose top level is `toplevel`, the absolute path
    /// `git rev-parse --show-toplevel` prints (the caller runs git), normalised by `host`'s
    /// rules: `/` separators, no trailing slash, ASCII-lowercased on Windows and macOS.
    ///
    /// # Errors
    /// [`IdError::NotUtf8`] for a path that is not UTF-8, [`IdError::NotAbsolute`] for a
    /// relative one.
    pub fn of_toplevel(toplevel: &Path, host: Host) -> Result<Self, IdError> {
        let normalised = normalise(toplevel, host)?;
        let hex = to_hex(&Sha256::digest(normalised.as_bytes()));
        Ok(Self(format!(
            "wt-{}",
            hex.chars().take(WORKTREE_HEX).collect::<String>()
        )))
    }

    /// Reads a worktree id: `wt-` and 12 lowercase hex characters.
    ///
    /// # Errors
    /// [`IdError::Worktree`] for any other text.
    pub fn parse(text: &str) -> Result<Self, IdError> {
        match text.strip_prefix("wt-") {
            Some(hex) if hex.len() == WORKTREE_HEX && is_lower_hex(hex) => {
                Ok(Self(text.to_owned()))
            }
            _ => Err(IdError::Worktree),
        }
    }

    /// The id as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WorktreeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// ID-06's normal form of a top-level path.
fn normalise(toplevel: &Path, host: Host) -> Result<String, IdError> {
    let text = toplevel.to_str().ok_or(IdError::NotUtf8)?;
    let mut path = if host == Host::Windows {
        text.replace('\\', "/")
    } else {
        text.to_owned()
    };
    if host.folds_case() {
        path.make_ascii_lowercase();
    }
    let absolute = path.starts_with('/') || (host == Host::Windows && is_drive_rooted(&path));
    if !absolute {
        return Err(IdError::NotAbsolute);
    }
    // A root (`/`, `c:/`) keeps its slash; anything longer drops every trailing one.
    while path.len() > root_len(&path) && path.ends_with('/') {
        path.pop();
    }
    Ok(path)
}

/// `x:/…`, a Windows drive root.
fn is_drive_rooted(path: &str) -> bool {
    matches!(path.as_bytes(), [drive, b':', b'/', ..] if drive.is_ascii_alphabetic())
}

/// The length of the path's root: 3 for a drive root, else 1.
fn root_len(path: &str) -> usize {
    if is_drive_rooted(path) { 3 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expected(normalised: &str) -> String {
        let hex = to_hex(&Sha256::digest(normalised.as_bytes()));
        format!("wt-{}", hex.get(..12).expect("64 hex"))
    }

    #[test]
    fn a_worktree_id_is_wt_and_twelve_hex_of_the_path() {
        let id =
            WorktreeId::of_toplevel(Path::new("/home/user/redkiln"), Host::Unix).expect("valid");
        assert_eq!(id.as_str(), expected("/home/user/redkiln"));
        assert_eq!(WorktreeId::parse(id.as_str()), Ok(id));
    }

    #[test]
    fn a_trailing_slash_is_dropped() {
        let id =
            WorktreeId::of_toplevel(Path::new("/home/user/redkiln//"), Host::Unix).expect("valid");
        assert_eq!(id.as_str(), expected("/home/user/redkiln"));
    }

    #[test]
    fn unix_keeps_case_and_backslashes() {
        let id = WorktreeId::of_toplevel(Path::new("/Home/a\\b"), Host::Unix).expect("valid");
        assert_eq!(id.as_str(), expected("/Home/a\\b"));
    }

    #[test]
    fn macos_lowercases_ascii() {
        let id =
            WorktreeId::of_toplevel(Path::new("/Users/Ryan/Redkiln"), Host::MacOs).expect("valid");
        assert_eq!(id.as_str(), expected("/users/ryan/redkiln"));
    }

    #[test]
    fn windows_uses_forward_slashes_and_lowercases_ascii() {
        let id = WorktreeId::of_toplevel(Path::new("C:\\Users\\Ryan\\Redkiln\\"), Host::Windows)
            .expect("valid");
        assert_eq!(id.as_str(), expected("c:/users/ryan/redkiln"));
        let git_form = WorktreeId::of_toplevel(Path::new("C:/Users/Ryan/Redkiln"), Host::Windows)
            .expect("valid");
        assert_eq!(git_form, id, "git's own spelling gives the same id");
    }

    #[test]
    fn only_ascii_is_folded() {
        let id = WorktreeId::of_toplevel(Path::new("/\u{c9}t\u{c9}"), Host::MacOs).expect("valid");
        assert_eq!(id.as_str(), expected("/\u{c9}t\u{c9}"));
    }

    #[test]
    fn a_relative_path_is_refused() {
        assert_eq!(
            WorktreeId::of_toplevel(Path::new("redkiln"), Host::Unix),
            Err(IdError::NotAbsolute)
        );
        assert_eq!(
            WorktreeId::of_toplevel(Path::new("Users\\x"), Host::Windows),
            Err(IdError::NotAbsolute)
        );
    }

    #[test]
    fn a_worktree_id_reads_only_its_own_form() {
        for text in [
            "wt-5d41402abc4",
            "wt-5D41402ABC4B",
            "wi-5d41402abc4b",
            "wt-5d41402abc4b0",
        ] {
            assert_eq!(WorktreeId::parse(text), Err(IdError::Worktree), "{text:?}");
        }
        let example = WorktreeId::parse("wt-5d41402abc4b").expect("the SPEC's example");
        assert_eq!(example.as_str(), "wt-5d41402abc4b");
    }
}
