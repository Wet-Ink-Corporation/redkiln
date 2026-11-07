//! Shared helpers.

use std::path::Path;

/// The names in `dir`, sorted, so a test can say nothing was added or removed beside a file.
pub(crate) fn listing(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("read the directory")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}
