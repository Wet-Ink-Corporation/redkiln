//! Reads `plugins/` from disk into [`Plugin`]s for [`check`](super::check).

use std::fs::{self, DirEntry, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use super::{FileKind, Plugin, PluginFile};

/// The most files one plugin may hold: claude.ai's per-plugin limit.
const MAX_FILES: usize = 5_000;

/// The deepest folder nesting read inside a plugin.
const MAX_DEPTH: usize = 16;

/// The largest text file read for the check, in bytes.
const MAX_TEXT: u64 = 1024 * 1024;

/// Why `plugins/` could not be read.
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
    /// A name that is not UTF-8.
    #[error("{} has a name that is not UTF-8", path.display())]
    NonUtf8 {
        /// The path with the name.
        path: PathBuf,
    },
    /// A plugin holds more files than claude.ai accepts.
    #[error("plugin {plugin} holds more than {limit} files")]
    TooMany {
        /// The plugin folder.
        plugin: String,
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
    /// A file the check reads is larger than it reads.
    #[error("{} is larger than {limit} bytes", path.display())]
    TooLarge {
        /// The file.
        path: PathBuf,
        /// The limit in bytes.
        limit: u64,
    },
}

/// Reads every folder under `root` (the repository's `plugins/`) as a plugin, in name order.
///
/// An absent `root` holds no plugins; a file directly under it is a plugin with no files.
/// Symlinks are reported, never followed.
///
/// # Errors
///
/// [`ReadError`] when a folder or a checked text file cannot be read, a name is not UTF-8, or a
/// plugin passes the file, depth or size limits.
pub fn read_plugins(root: &Path) -> Result<Vec<Plugin>, ReadError> {
    match fs::symlink_metadata(root) {
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(ReadError::Io {
                path: root.to_owned(),
                source,
            });
        }
        Ok(_) => {}
    }
    let mut plugins = Vec::new();
    for entry in sorted_entries(root)? {
        let name = utf8_name(&entry)?;
        let mut plugin = Plugin {
            name,
            files: Vec::new(),
        };
        if file_type(&entry)?.is_dir() {
            walk(&mut plugin, &entry.path(), "", 0)?;
        }
        plugins.push(plugin);
    }
    Ok(plugins)
}

/// Adds every file under `dir`, whose path in the plugin starts with `prefix`, to `plugin`.
fn walk(plugin: &mut Plugin, dir: &Path, prefix: &str, depth: usize) -> Result<(), ReadError> {
    if depth > MAX_DEPTH {
        return Err(ReadError::TooDeep {
            path: dir.to_owned(),
            limit: MAX_DEPTH,
        });
    }
    for entry in sorted_entries(dir)? {
        let path = format!("{prefix}{}", utf8_name(&entry)?);
        let kind = file_type(&entry)?;
        if kind.is_dir() {
            walk(plugin, &entry.path(), &format!("{path}/"), depth + 1)?;
            continue;
        }
        if plugin.files.len() >= MAX_FILES {
            return Err(ReadError::TooMany {
                plugin: plugin.name.clone(),
                limit: MAX_FILES,
            });
        }
        let file = if kind.is_file() {
            let text = if wants_text(&path) {
                Some(read_text(&entry.path())?)
            } else {
                None
            };
            PluginFile {
                path,
                kind: FileKind::Regular,
                text,
            }
        } else {
            PluginFile {
                path,
                kind: FileKind::Other,
                text: None,
            }
        };
        plugin.files.push(file);
    }
    Ok(())
}

/// The files whose contents the check reads.
fn wants_text(path: &str) -> bool {
    path == "hooks/hooks.json"
        || (super::is_prompt(path)
            && Path::new(path)
                .extension()
                .is_some_and(|extension| extension == "md"))
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
                plugin: dir.display().to_string(),
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

fn file_type(entry: &DirEntry) -> Result<fs::FileType, ReadError> {
    entry.file_type().map_err(|source| ReadError::Io {
        path: entry.path(),
        source,
    })
}

fn read_text(path: &Path) -> Result<String, ReadError> {
    let io_error = |source| ReadError::Io {
        path: path.to_owned(),
        source,
    };
    let mut text = String::new();
    File::open(path)
        .map_err(io_error)?
        .take(MAX_TEXT + 1)
        .read_to_string(&mut text)
        .map_err(io_error)?;
    if u64::try_from(text.len()).map_or(true, |len| len > MAX_TEXT) {
        return Err(ReadError::TooLarge {
            path: path.to_owned(),
            limit: MAX_TEXT,
        });
    }
    Ok(text)
}
