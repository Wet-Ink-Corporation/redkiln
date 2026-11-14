//! Hook state files (`hook-errors.log`, `hook-errors.seen`, `hook-late.log`, `hook-late.seen`,
//! `journal-watermark-*`, the Windows shell copy) are read, appended or overwritten only when they are regular files or absent
//! (HK-6). A symlink, a directory or a FIFO where one should be is left alone, so a hook never
//! writes through a link to somewhere else, nor blocks opening a pipe.
//!
//! The check and the open are two calls: a symlink swapped in between is not caught, since std
//! has no `O_NOFOLLOW` open (`docs/weigh-in/hooks.md`).

use anyhow::{Context as _, Result, bail};
use std::io::ErrorKind;
use std::path::Path;

/// What is at a state file's path, when it may be used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Presence {
    /// Nothing: the file may be created.
    Absent,
    /// A regular file, not a link to one.
    Regular,
}

/// Whether `path` is a regular file or absent, without following a symlink.
///
/// # Errors
/// Anything else is there (a symlink, a directory, a FIFO), or its metadata cannot be read.
pub(crate) fn regular_or_absent(path: &Path) -> Result<Presence> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_file() => Ok(Presence::Regular),
        Ok(_) => bail!(
            "{} is not a regular file, so weighin leaves it alone",
            path.display()
        ),
        Err(e) if e.kind() == ErrorKind::NotFound => Ok(Presence::Absent),
        Err(e) => Err(anyhow::Error::new(e).context(format!("reading {}", path.display()))),
    }
}

/// Makes sure `path` is a directory and not a symlink to one, creating it (and its parents) when
/// it is absent.
///
/// # Errors
/// Something else is there, or the directory cannot be created or its metadata read.
pub(crate) fn real_dir(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(meta) if meta.file_type().is_dir() => Ok(()),
        Ok(_) => bail!(
            "{} is not a directory, so weighin leaves it alone",
            path.display()
        ),
        Err(e) if e.kind() == ErrorKind::NotFound => {
            std::fs::create_dir_all(path).with_context(|| format!("creating {}", path.display()))
        }
        Err(e) => Err(anyhow::Error::new(e).context(format!("reading {}", path.display()))),
    }
}

#[cfg(test)]
mod tests {
    use super::{Presence, real_dir, regular_or_absent};

    #[test]
    #[cfg_attr(miri, ignore = "reads file metadata through the file system")]
    fn absent_is_absent() {
        let tmp = tempfile::tempdir().expect("temp dir");

        let presence = regular_or_absent(&tmp.path().join("missing")).expect("absent is fine");

        assert_eq!(presence, Presence::Absent);
    }

    #[test]
    #[cfg_attr(miri, ignore = "reads file metadata through the file system")]
    fn a_file_is_regular() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let file = tmp.path().join("hook-errors.log");
        std::fs::write(&file, "line\n").expect("file written");

        let presence = regular_or_absent(&file).expect("a file is fine");

        assert_eq!(presence, Presence::Regular);
    }

    #[cfg(unix)]
    #[test]
    #[cfg_attr(miri, ignore = "makes a symlink through the file system")]
    fn a_symlink_is_refused() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let target = tmp.path().join("target");
        std::fs::write(&target, "kept\n").expect("target written");
        let link = tmp.path().join("hook-errors.log");
        std::os::unix::fs::symlink(&target, &link).expect("symlink");

        let refused = regular_or_absent(&link).expect_err("a symlink is refused");

        assert_eq!(
            refused.to_string(),
            format!(
                "{} is not a regular file, so weighin leaves it alone",
                link.display()
            )
        );
    }

    #[test]
    #[cfg_attr(miri, ignore = "makes a directory through the file system")]
    fn a_directory_is_refused() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let dir = tmp.path().join("hook-errors.seen");
        std::fs::create_dir(&dir).expect("dir made");

        let refused = regular_or_absent(&dir).expect_err("a directory is refused");

        assert!(
            refused
                .to_string()
                .ends_with("is not a regular file, so weighin leaves it alone"),
            "{refused}"
        );
    }

    #[test]
    #[cfg_attr(miri, ignore = "makes directories through the file system")]
    fn a_real_dir_is_made_when_absent_and_kept_when_present() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let dir = tmp.path().join("redkiln/weighin");

        real_dir(&dir).expect("made");
        real_dir(&dir).expect("kept");

        assert!(dir.is_dir());
    }

    #[cfg(unix)]
    #[test]
    #[cfg_attr(miri, ignore = "makes a symlink through the file system")]
    fn a_symlinked_dir_is_not_a_real_dir() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let elsewhere = tmp.path().join("elsewhere");
        std::fs::create_dir(&elsewhere).expect("dir made");
        let link = tmp.path().join("weighin");
        std::os::unix::fs::symlink(&elsewhere, &link).expect("symlink");

        let refused = real_dir(&link).expect_err("a symlinked dir is refused");

        assert_eq!(
            refused.to_string(),
            format!(
                "{} is not a directory, so weighin leaves it alone",
                link.display()
            )
        );
    }
}
