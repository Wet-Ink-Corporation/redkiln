//! Whether the ratchet file `floor-guard` judges is the worktree's own (wi-3e8eb7a535 A).
//!
//! A symlinked `.redkiln`, `.redkiln/bench` or `.anneal` (or a symlinked ratchet file) hides
//! the file from the diff's paths and from the merge base's tree, so its floors would never
//! be judged. As the config reader refuses such a file (`repo_config::FileError::NotOwn`),
//! floor-guard fails closed: a ratchet file that is not the worktree's own is one
//! `ratchet_loosened` finding, as one it cannot read as a ratchet file already is.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::detect::{Category, FloorFinding};
use super::{FloorError, git};
use crate::repo_file::{self, LEGACY_DIR, RATCHETS};

/// The folders on the way to the ratchet file in `.redkiln/bench/`, and the file.
const CURRENT_CHAIN: [&str; 3] = [".redkiln", ".redkiln/bench", RATCHETS.path];

/// The folder on the way to anneal's ratchet file, and the file.
const LEGACY_CHAIN: [&str; 2] = [LEGACY_DIR, RATCHETS.legacy];

/// Every path whose change in the diff makes floor-guard judge the ratchet file: both copies,
/// and the folders on their way, which a diff lists only when one is not a folder (a symlink).
pub(super) const TOUCHING: [&str; 5] = [
    CURRENT_CHAIN[0],
    CURRENT_CHAIN[1],
    CURRENT_CHAIN[2],
    LEGACY_CHAIN[0],
    LEGACY_CHAIN[1],
];

/// A finding on the whole ratchet file `file`.
fn finding(file: &str, message: String) -> FloorFinding {
    FloorFinding {
        category: Category::RatchetLoosened,
        file: file.to_owned(),
        line: None,
        message,
    }
}

/// The finding when the working tree's ratchet file in `root` (the copy readers choose,
/// `RepoFile::choose`) does not resolve to the worktree's own path: through a symlinked
/// folder, as a symlink itself, or as a symlink that resolves nowhere. Checked on every run,
/// whatever the diff's paths, since such a file hides its edits from them.
///
/// # Errors
/// [`FloorError::Read`] when the path or the root cannot be examined for another reason than
/// its absence.
pub(super) fn tree(root: &Path) -> Result<Option<FloorFinding>, FloorError> {
    let chosen = RATCHETS.choose(root).path;
    let path = repo_file::under(root, chosen);
    let failed = |source: io::Error| FloorError::Read {
        // The error owns the path it names.
        path: PathBuf::from(chosen),
        source,
    };
    match fs::symlink_metadata(&path) {
        Ok(_) => {}
        // Nothing there, or a file where a folder on its way should be: no ratchet file to
        // judge here, and the diff's own read reports the latter when it touches the file.
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
            ) =>
        {
            return Ok(None);
        }
        Err(error) => return Err(failed(error)),
    }
    let resolved = match fs::canonicalize(&path) {
        Ok(resolved) => resolved,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let message = format!(
                "{chosen} is a symlink that resolves nowhere, so its floors cannot be judged; \
                 make it a regular file in plain directories"
            );
            return Ok(Some(finding(chosen, message)));
        }
        Err(error) => return Err(failed(error)),
    };
    let own = repo_file::under(&fs::canonicalize(root).map_err(failed)?, chosen);
    if resolved == own {
        return Ok(None);
    }
    let message = format!(
        "{chosen} resolves to {}, not this worktree's own {chosen}, so its floors cannot be \
         judged; make it a regular file in plain directories",
        resolved.display()
    );
    Ok(Some(finding(chosen, message)))
}

/// The finding when the merge base reaches its ratchet file through a symlink, so its floors
/// cannot be compared: one on the way to `.redkiln/bench/ratchets.json`, or, unless the merge
/// base has that file (`base_current`), one on the way to `.anneal/ratchets.json`.
///
/// # Errors
/// The [`FloorError`] of `git ls-tree`.
pub(super) fn base(
    root: &Path,
    merge_base: &str,
    base_current: bool,
) -> Result<Option<FloorFinding>, FloorError> {
    let linked = git::symlinks(root, merge_base, &TOUCHING)?;
    let current = CURRENT_CHAIN.iter().find(|path| linked.contains(path));
    let legacy = LEGACY_CHAIN
        .iter()
        .find(|path| !base_current && linked.contains(path));
    let found = current
        .map(|link| (RATCHETS.path, link))
        .or_else(|| legacy.map(|link| (RATCHETS.legacy, link)));
    Ok(found.map(|(file, link)| {
        let message = format!(
            "the merge base reaches {file} through the symlink {link}, so its floors cannot be \
             compared"
        );
        finding(file, message)
    }))
}
