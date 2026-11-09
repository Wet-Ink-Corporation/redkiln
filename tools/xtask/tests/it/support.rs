//! Helpers that write plugin trees for the reader tests. They return `Result`, so they hold to
//! production lints (TST-14).

use std::fs;
use std::io;
use std::ops::Deref;
use std::path::Path;

/// A fresh, empty directory for one test, removed when dropped.
pub(crate) struct Scratch(tempfile::TempDir);

impl Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        self.0.path()
    }
}

impl AsRef<Path> for Scratch {
    fn as_ref(&self) -> &Path {
        self.0.path()
    }
}

/// A fresh, empty directory for one test under cargo's per-target scratch space, named
/// `<name>` plus a random suffix. Each call gets its own directory, so two runs of the suite at
/// once (two gates in one checkout) never share one, which the fixed `<name>` paths did.
pub(crate) fn scratch(name: &str) -> io::Result<Scratch> {
    let root = Path::new(env!("CARGO_TARGET_TMPDIR")).join("xtask-plugins");
    fs::create_dir_all(&root)?;
    tempfile::Builder::new()
        .prefix(&format!("{name}-"))
        .tempdir_in(root)
        .map(Scratch)
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
