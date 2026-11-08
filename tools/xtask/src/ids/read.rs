//! Reads `.kb/` from disk into [`AtomFile`]s for [`check`](super::check).

use std::fs::{self, DirEntry, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use super::AtomFile;

/// The most Markdown files read from `.kb/`, and the most entries read from one folder.
pub const MAX_FILES: usize = 50_000;

/// The deepest folder nesting read: a file may sit this many folders below `.kb/`.
pub const MAX_DEPTH: usize = 8;

/// The largest Markdown file read, in bytes.
pub const MAX_TEXT: u64 = 1024 * 1024;

/// The ledger folder, which holds journals and configuration, never atoms.
const LEDGER: &str = "_ledger";

/// Why `.kb/` could not be read.
#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    /// A folder or file could not be read.
    #[error("cannot read {}", path.display())]
    Io {
        /// The path that failed.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: io::Error,
    },
    /// `.kb` itself is a symlink or a file; the reader never follows a symlink.
    #[error("{} is not a folder", path.display())]
    RootNotFolder {
        /// The path given as `.kb/`.
        path: PathBuf,
    },
    /// A name that is not UTF-8.
    #[error("{} has a name that is not UTF-8", path.display())]
    NonUtf8 {
        /// The path with the name.
        path: PathBuf,
    },
    /// A Markdown file that is not UTF-8.
    #[error("{} is not UTF-8 text after byte {valid_up_to}", path.display())]
    NonUtf8Text {
        /// The file.
        path: PathBuf,
        /// How many leading bytes are valid UTF-8.
        valid_up_to: usize,
    },
    /// More Markdown files, or more entries in one folder, than the reader reads.
    #[error("{} holds more than {limit} files", path.display())]
    TooMany {
        /// The folder.
        path: PathBuf,
        /// The limit.
        limit: usize,
    },
    /// Folders nested deeper than the reader goes.
    #[error("{} is nested more than {limit} folders deep", path.display())]
    TooDeep {
        /// The folder.
        path: PathBuf,
        /// The limit.
        limit: usize,
    },
    /// A Markdown file larger than the reader reads.
    #[error("{} is larger than {limit} bytes", path.display())]
    TooLarge {
        /// The file.
        path: PathBuf,
        /// The limit in bytes.
        limit: u64,
    },
}

/// Reads every regular `*.md` file under `kb` (the repository's `.kb/`), in path order, passing
/// over `.kb/_ledger/`.
///
/// An absent `kb` holds no atoms. Symlinks are never followed; a symlinked file or folder is
/// passed over.
///
/// # Errors
///
/// [`ReadError`] when `kb` is not a folder, a folder or Markdown file cannot be read, a name or
/// file is not UTF-8, or `.kb/` passes the file, depth or size limits.
pub fn read_kb(kb: &Path) -> Result<Vec<AtomFile>, ReadError> {
    match fs::symlink_metadata(kb) {
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(ReadError::Io {
                path: kb.to_owned(),
                source,
            });
        }
        Ok(metadata) if !metadata.is_dir() => {
            return Err(ReadError::RootNotFolder {
                path: kb.to_owned(),
            });
        }
        Ok(_) => {}
    }
    let mut atoms = Vec::new();
    walk(&mut atoms, kb, "", 0)?;
    Ok(atoms)
}

/// Adds every Markdown file under `dir`, whose path in `.kb/` starts with `prefix`, to `atoms`.
fn walk(
    atoms: &mut Vec<AtomFile>,
    dir: &Path,
    prefix: &str,
    depth: usize,
) -> Result<(), ReadError> {
    if depth > MAX_DEPTH {
        return Err(ReadError::TooDeep {
            path: dir.to_owned(),
            limit: MAX_DEPTH,
        });
    }
    for entry in sorted_entries(dir)? {
        let name = utf8_name(&entry)?;
        let kind = entry.file_type().map_err(|source| ReadError::Io {
            path: entry.path(),
            source,
        })?;
        if kind.is_dir() {
            if !(depth == 0 && name == LEDGER) {
                walk(atoms, &entry.path(), &format!("{prefix}{name}/"), depth + 1)?;
            }
            continue;
        }
        if !kind.is_file() || Path::new(&name).extension().is_none_or(|e| e != "md") {
            continue;
        }
        if atoms.len() >= MAX_FILES {
            return Err(ReadError::TooMany {
                path: dir.to_owned(),
                limit: MAX_FILES,
            });
        }
        atoms.push(AtomFile {
            text: read_text(&entry.path())?,
            path: format!("{prefix}{name}"),
        });
    }
    Ok(())
}

fn sorted_entries(dir: &Path) -> Result<Vec<DirEntry>, ReadError> {
    let io_error = |source| ReadError::Io {
        path: dir.to_owned(),
        source,
    };
    let mut entries = Vec::new();
    for entry in fs::read_dir(dir).map_err(io_error)? {
        if entries.len() >= MAX_FILES {
            return Err(ReadError::TooMany {
                path: dir.to_owned(),
                limit: MAX_FILES,
            });
        }
        entries.push(entry.map_err(io_error)?);
    }
    entries.sort_by_key(DirEntry::file_name);
    Ok(entries)
}

fn utf8_name(entry: &DirEntry) -> Result<String, ReadError> {
    entry
        .file_name()
        .into_string()
        .map_err(|name| ReadError::NonUtf8 {
            path: entry.path().with_file_name(name),
        })
}

/// Reads at most one byte past [`MAX_TEXT`], so a larger file is refused by its size before its
/// encoding is judged.
fn read_text(path: &Path) -> Result<String, ReadError> {
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|file| file.take(MAX_TEXT + 1).read_to_end(&mut bytes))
        .map_err(|source| ReadError::Io {
            path: path.to_owned(),
            source,
        })?;
    if u64::try_from(bytes.len()).map_or(true, |len| len > MAX_TEXT) {
        return Err(ReadError::TooLarge {
            path: path.to_owned(),
            limit: MAX_TEXT,
        });
    }
    String::from_utf8(bytes).map_err(|error| ReadError::NonUtf8Text {
        path: path.to_owned(),
        valid_up_to: error.utf8_error().valid_up_to(),
    })
}
