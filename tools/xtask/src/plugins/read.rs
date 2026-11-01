//! Reads `plugins/` from disk into [`Plugin`]s for [`check`](super::check).

use std::fs::{self, DirEntry, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use super::{FileKind, Plugin, PluginFile, SKILL_EXTENSIONS};

/// The most files one plugin may hold: claude.ai's per-plugin limit.
pub const MAX_FILES: usize = 5_000;

/// The deepest folder nesting read inside a plugin: a file may sit this many folders below it.
pub const MAX_DEPTH: usize = 16;

/// The largest text file read for the check, in bytes.
pub const MAX_TEXT: u64 = 1024 * 1024;

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
    /// `plugins/` itself is a symlink or a file; the reader never follows a symlink.
    #[error("{} is not a folder", path.display())]
    RootNotFolder {
        /// The path given as `plugins/`.
        path: PathBuf,
    },
    /// A name that is not UTF-8.
    #[error("{} has a name that is not UTF-8", path.display())]
    NonUtf8 {
        /// The path with the name.
        path: PathBuf,
    },
    /// A text file the check reads that is not UTF-8.
    #[error("{} is not UTF-8 text after byte {valid_up_to}", path.display())]
    NonUtf8Text {
        /// The file.
        path: PathBuf,
        /// How many leading bytes are valid UTF-8.
        valid_up_to: usize,
    },
    /// A plugin holds more files than claude.ai accepts.
    #[error("plugin {plugin} holds more than {limit} files")]
    TooMany {
        /// The plugin folder.
        plugin: String,
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

/// Reads every entry under `root` (the repository's `plugins/`) as a plugin, in name order.
///
/// An absent `root` holds no plugins. An entry that is not a folder is a plugin with
/// `folder: false`. Symlinks are reported, never followed, `root` included.
///
/// # Errors
///
/// [`ReadError`] when `root` is not a folder, a folder or a checked text file cannot be read, a
/// name or text is not UTF-8, or a plugin passes the file, depth or size limits.
pub fn read_plugins(root: &Path) -> Result<Vec<Plugin>, ReadError> {
    match fs::symlink_metadata(root) {
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => {
            return Err(ReadError::Io {
                path: root.to_owned(),
                source,
            });
        }
        Ok(metadata) if !metadata.is_dir() => {
            return Err(ReadError::RootNotFolder {
                path: root.to_owned(),
            });
        }
        Ok(_) => {}
    }
    let mut plugins = Vec::new();
    for entry in sorted_entries(root)? {
        let folder = file_type(&entry)?.is_dir();
        let mut plugin = Plugin {
            name: utf8_name(&entry)?,
            folder,
            files: Vec::new(),
        };
        if folder {
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

/// The files whose contents the check reads: `hooks.json`, every text file a skill may carry, and
/// agent and command Markdown.
fn wants_text(path: &str) -> bool {
    let extension = Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str());
    path == "hooks/hooks.json"
        || (path.starts_with("skills/") && extension.is_some_and(|e| SKILL_EXTENSIONS.contains(&e)))
        || (super::is_prompt(path) && extension == Some("md"))
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

fn file_type(entry: &DirEntry) -> Result<fs::FileType, ReadError> {
    entry.file_type().map_err(|source| ReadError::Io {
        path: entry.path(),
        source,
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
