//! Helpers that write plugin trees for the reader tests. They return `Result`, so they hold to
//! production lints (TST-14).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// A fresh, empty directory for one test, under cargo's per-target scratch space.
pub(crate) fn scratch(name: &str) -> io::Result<PathBuf> {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR"))
        .join("xtask-plugins")
        .join(name);
    if dir.exists() {
        fs::remove_dir_all(&dir)?;
    }
    fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Writes each `(path, contents)` under `root`, creating folders as needed.
pub(crate) fn tree(root: &Path, files: &[(&str, &str)]) -> io::Result<()> {
    for &(path, contents) in files {
        let full = root.join(path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(full, contents)?;
    }
    Ok(())
}

/// Writes `count` empty files named `0` to `count - 1` into `dir`.
pub(crate) fn many(dir: &Path, count: usize) -> io::Result<()> {
    fs::create_dir_all(dir)?;
    for n in 0..count {
        fs::write(dir.join(n.to_string()), b"")?;
    }
    Ok(())
}
