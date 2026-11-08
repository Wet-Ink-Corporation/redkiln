//! Reads `crates/` from disk into [`SourceFile`]s for [`check`](super::check).

use std::fs::{self, DirEntry, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use super::SourceFile;

/// The most Rust files read from `crates/`, and the most entries read from one folder.
pub const MAX_FILES: usize = 10_000;

/// The deepest folder nesting read: a file may sit this many folders below `crates/`.
pub const MAX_DEPTH: usize = 16;

/// The largest Rust file read, in bytes.
pub const MAX_TEXT: u64 = 1024 * 1024;

/// A build folder, which holds generated code, never sources; passed over wherever it is.
const TARGET: &str = "target";

/// Why `crates/` could not be read.
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
    /// `crates` itself is a symlink or a file; the reader never follows a symlink.
    #[error("{} is not a folder", path.display())]
    RootNotFolder {
        /// The path given as `crates/`.
        path: PathBuf,
    },
    /// A name that is not UTF-8.
    #[error("{} has a name that is not UTF-8", path.display())]
    NonUtf8 {
        /// The path with the name.
        path: PathBuf,
    },
    /// A Rust file that is not UTF-8.
    #[error("{} is not UTF-8 text after byte {valid_up_to}", path.display())]
    NonUtf8Text {
        /// The file.
        path: PathBuf,
        /// How many leading bytes are valid UTF-8.
        valid_up_to: usize,
    },
    /// More Rust files in all than the reader reads.
    #[error("{} holds more than {limit} Rust files", path.display())]
    TooMany {
        /// The `crates/` folder.
        path: PathBuf,
        /// The limit.
        limit: usize,
    },
    /// One folder holds more entries than the reader lists.
    #[error("{} holds more than {limit} entries", path.display())]
    TooManyEntries {
        /// The folder.
        path: PathBuf,
        /// The limit.
        limit: usize,
    },
    /// A symlink, or anything else that is neither a folder nor a regular file, inside
    /// `crates/`. A link could carry code the check would otherwise not see.
    #[error("{} is not a regular file or folder", path.display())]
    NotRegular {
        /// The entry.
        path: PathBuf,
    },
    /// Folders nested deeper than the reader goes.
    #[error("{} is nested more than {limit} folders deep", path.display())]
    TooDeep {
        /// The folder.
        path: PathBuf,
        /// The limit.
        limit: usize,
    },
    /// A Rust file larger than the reader reads.
    #[error("{} is larger than {limit} bytes", path.display())]
    TooLarge {
        /// The file.
        path: PathBuf,
        /// The limit in bytes.
        limit: u64,
    },
}

/// Reads every regular `*.rs` file under `crates` (the repository's `crates/`), in path order,
/// passing over folders named `target`. Each path starts `crates/`.
///
/// An absent `crates` holds no sources. Symlinks are never followed: a symlink anywhere it reads
/// is refused, `crates` included.
///
/// # Errors
///
/// [`ReadError`] when `crates` is not a folder, a folder or Rust file cannot be read, an entry
/// is a symlink or another non-regular file, a name or file is not UTF-8, or `crates/` passes
/// the file, entry, depth or size limits.
pub fn read_sources(crates: &Path) -> Result<Vec<SourceFile>, ReadError> {
    match fs::symlink_metadata(crates) {
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(ReadError::Io {
                path: crates.to_owned(),
                source,
            });
        }
        Ok(metadata) if !metadata.is_dir() => {
            return Err(ReadError::RootNotFolder {
                path: crates.to_owned(),
            });
        }
        Ok(_) => {}
    }
    let mut sources = Vec::new();
    walk(&mut sources, crates, crates, "")?;
    sources.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(sources)
}

/// Adds every Rust file under `dir`, whose path in `crates/` (at `root`) starts with `prefix`,
/// to `sources`. `prefix` is empty or ends in `/`, so its `/`s count the folders below `crates/`.
fn walk(
    sources: &mut Vec<SourceFile>,
    root: &Path,
    dir: &Path,
    prefix: &str,
) -> Result<(), ReadError> {
    let depth = prefix.matches('/').count();
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
            if name != TARGET {
                let inner = format!("{prefix}{name}/");
                walk(sources, root, &entry.path(), &inner)?;
            }
            continue;
        }
        if !kind.is_file() {
            return Err(ReadError::NotRegular { path: entry.path() });
        }
        if Path::new(&name).extension().is_none_or(|e| e != "rs") {
            continue;
        }
        if sources.len() >= MAX_FILES {
            return Err(ReadError::TooMany {
                path: root.to_owned(),
                limit: MAX_FILES,
            });
        }
        sources.push(SourceFile {
            text: read_text(&entry.path())?,
            path: format!("crates/{prefix}{name}"),
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
            return Err(ReadError::TooManyEntries {
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
