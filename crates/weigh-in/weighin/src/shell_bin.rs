//! A copy of the running binary that the skill's shell can reach (#127).
//!
//! In the Windows desktop app, hooks start `libexec/weighin.exe` from the plugin folder, but the
//! shell the skill runs `W` in cannot see that folder. `SessionStart` can, so it copies itself
//! into the repository's git dir, which the shell working in that repository can see, and names
//! the copy.
//!
//! Nothing here deletes (B-01): the copy is written to a fixed `.part` name, overwritten each
//! time, and renamed into place. Copies of older versions stay where they are.

use anyhow::{Context as _, Result};
use std::path::{Path, PathBuf};

/// Why [`place`] could not put a fresh copy in place, and the copy already there, if any.
#[derive(Debug)]
pub(crate) struct Unplaced {
    /// What failed.
    pub(crate) error: anyhow::Error,
    /// A copy of this version already at the target, which the shell can still run: a rename
    /// fails on Windows while another session runs the target, or while a scanner holds the
    /// new file.
    pub(crate) existing: Option<PathBuf>,
}

/// Copy `exe` to `<dir>/bin/<version>/<exe's file name>`, unless a file of the same length is
/// already there, and return the copy's path.
///
/// Identity is by length: copies are kept per version, and two builds of one version differ in
/// length in practice. A same-length rebuild of one version happens only in development (B-26).
///
/// # Errors
/// Fails when `exe` has no file name or cannot be read, or the copy cannot be written; the error
/// carries the copy already at the target, if there is one.
pub(crate) fn place(exe: &Path, dir: &Path, version: &str) -> Result<PathBuf, Box<Unplaced>> {
    let fresh = |error| {
        Box::new(Unplaced {
            error,
            existing: None,
        })
    };
    let name = exe
        .file_name()
        .with_context(|| format!("{} has no file name", exe.display()))
        .map_err(fresh)?;
    let source_len = std::fs::metadata(exe)
        .with_context(|| format!("reading {}", exe.display()))
        .map_err(fresh)?
        .len();
    let bin = dir.join("bin").join(version);
    let target = bin.join(name);
    if std::fs::metadata(&target).is_ok_and(|m| m.len() == source_len) {
        return Ok(target);
    }
    match copy_into_place(exe, &bin, &target) {
        Ok(()) => Ok(target),
        Err(error) => {
            let existing = target.is_file().then_some(target);
            Err(Box::new(Unplaced { error, existing }))
        }
    }
}

/// Copy `exe` to `<bin>/<target's name>.part`, then rename it to `target`.
fn copy_into_place(exe: &Path, bin: &Path, target: &Path) -> Result<()> {
    std::fs::create_dir_all(bin).with_context(|| format!("creating {}", bin.display()))?;
    let mut part = target.as_os_str().to_os_string();
    part.push(".part");
    let part = PathBuf::from(part);
    std::fs::copy(exe, &part)
        .with_context(|| format!("copying {} to {}", exe.display(), part.display()))?;
    std::fs::rename(&part, target)
        .with_context(|| format!("renaming {} to {}", part.display(), target.display()))
}

#[cfg(test)]
mod tests {
    use super::place;
    use std::path::Path;

    fn source(dir: &Path, bytes: &[u8]) -> std::path::PathBuf {
        let exe = dir.join("weighin.exe");
        std::fs::write(&exe, bytes).expect("the temp dir is writable");
        exe
    }

    #[test]
    fn copies_into_the_versioned_bin_dir() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let exe = source(tmp.path(), b"binary");
        let state = tmp.path().join("state");

        let placed = place(&exe, &state, "1.2.3").expect("placed");

        assert_eq!(placed, state.join("bin").join("1.2.3").join("weighin.exe"));
        assert_eq!(std::fs::read(&placed).expect("copy readable"), b"binary");
    }

    #[test]
    fn leaves_a_copy_of_the_same_length_alone() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let exe = source(tmp.path(), b"binary");
        let state = tmp.path().join("state");
        let placed = place(&exe, &state, "1.2.3").expect("placed");
        std::fs::write(&placed, b"BINARY").expect("copy writable");

        let again = place(&exe, &state, "1.2.3").expect("placed again");

        assert_eq!(again, placed);
        assert_eq!(std::fs::read(&again).expect("copy readable"), b"BINARY");
    }

    #[test]
    fn replaces_a_copy_of_a_different_length() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let exe = source(tmp.path(), b"binary");
        let state = tmp.path().join("state");
        let placed = place(&exe, &state, "1.2.3").expect("placed");
        std::fs::write(&placed, b"old").expect("copy writable");

        place(&exe, &state, "1.2.3").expect("placed again");

        assert_eq!(std::fs::read(&placed).expect("copy readable"), b"binary");
    }

    #[test]
    fn a_missing_source_places_nothing() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let state = tmp.path().join("state");

        let unplaced =
            place(&tmp.path().join("missing.exe"), &state, "1.2.3").expect_err("nothing to copy");

        assert!(
            unplaced.error.to_string().contains("missing.exe"),
            "{:#}",
            unplaced.error
        );
        assert_eq!(unplaced.existing, None);
        assert!(!state.exists());
    }

    #[test]
    fn a_failed_copy_still_names_the_copy_already_there() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let exe = source(tmp.path(), b"binary");
        let state = tmp.path().join("state");
        let placed = place(&exe, &state, "1.2.3").expect("placed");
        std::fs::write(&placed, b"old").expect("copy writable");
        // A directory where the `.part` goes makes the copy fail, as a locked file would.
        std::fs::create_dir(placed.with_file_name("weighin.exe.part")).expect("dir creatable");

        let unplaced = place(&exe, &state, "1.2.3").expect_err("the copy fails");

        assert_eq!(unplaced.existing, Some(placed));
    }
}
