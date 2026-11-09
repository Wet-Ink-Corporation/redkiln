//! Building, packaging and assembling plugin releases (`docs/plan/move-1-weigh-in.md`, Release and
//! cutover).
//!
//! - [`build`]: `cargo build --release` of every binary a plugin ships a launcher copy for, copied
//!   into `plugins/<plugin>/libexec/<target>/`.
//! - [`package`]: a staged copy of every plugin with the builds of its binaries, the Windows build
//!   at `libexec/<binary>.exe` (M1-E) and its Cowork fetch script stamped, checked against the
//!   plugin rules.
//! - [`assemble`]: the packaged plugins written into the release repository's `plugins/` and its
//!   `marketplace.json`, leaving every other entry as it is.
//! - [`release_check`]: every version site of a plugin agrees with the release, and its changelog
//!   has the release's section, which it returns as the release notes.

mod build;
mod marketplace;
mod notes;
mod package;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::plugins;

pub use build::build;
pub use marketplace::assemble;
pub use notes::{changelog_section, release_check};
pub use package::{Download, Packaged, package, stamp};

/// Why a release task could not finish.
#[derive(Debug, thiserror::Error)]
pub enum ReleaseError {
    /// A plugin tree could not be read.
    #[error("cannot read the plugins")]
    Read(#[from] plugins::ReadError),
    /// A file or folder could not be read or written.
    #[error("cannot read or write {}", path.display())]
    Io {
        /// The path that failed.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: io::Error,
    },
    /// A JSON file was not what the task reads.
    #[error("{} is not the JSON this task reads", path.display())]
    Json {
        /// The file.
        path: PathBuf,
        /// The parse error.
        #[source]
        source: serde_json::Error,
    },
    /// A program could not be started.
    #[error("could not run {program}")]
    Spawn {
        /// The program and its first argument.
        program: String,
        /// The underlying error.
        #[source]
        source: io::Error,
    },
    /// A program ran and failed.
    #[error("{program} failed ({status}){stderr}")]
    Failed {
        /// The program and its first argument.
        program: String,
        /// Its exit status.
        status: std::process::ExitStatus,
        /// What it wrote to stderr, when captured, after `: `.
        stderr: String,
    },
    /// `rustc -vV` named no host target.
    #[error("rustc -vV names no host target")]
    NoHost,
    /// The output folder already exists.
    #[error("{} already exists; packaging writes only into a new folder", path.display())]
    OutExists {
        /// The folder.
        path: PathBuf,
    },
    /// A name that cannot be a folder name in a plugin or release repository.
    #[error("`{name}` is not a plugin name")]
    BadName {
        /// The name.
        name: String,
    },
    /// A binary a plugin ships a launcher copy for has no build to package.
    #[error("plugin {plugin}: no {binary} build{for_target}; run `cargo xtask build` first")]
    NoBuild {
        /// The plugin folder.
        plugin: String,
        /// The binary.
        binary: String,
        /// ` for <target>` when one target's build is missing, else empty.
        for_target: String,
    },
    /// Two plugins that ship one binary carry different versions.
    #[error(
        "{binary}: plugin {first} is at {first_version}, {second} at {second_version}; versions move together"
    )]
    VersionsDiffer {
        /// The shared binary.
        binary: String,
        /// The first plugin folder.
        first: String,
        /// Its version.
        first_version: String,
        /// The second plugin folder.
        second: String,
        /// Its version.
        second_version: String,
    },
    /// A version site disagrees with the release.
    #[error("{site} is at {found}, not {version}")]
    VersionMismatch {
        /// What carries the version.
        site: String,
        /// The version it carries.
        found: String,
        /// The release's version.
        version: String,
    },
    /// The changelog has no section for the release.
    #[error("{} has no section for {version}", path.display())]
    NoChangelogSection {
        /// The changelog.
        path: PathBuf,
        /// The release's version.
        version: String,
    },
    /// The plugin to release does not exist or ships no binary.
    #[error("plugin {plugin}: {problem}")]
    Plugin {
        /// The plugin folder.
        plugin: String,
        /// What is wrong.
        problem: String,
    },
    /// A Cowork fetch script without exactly one `V=...; S=...;` stamp.
    #[error("{script}: {found} `V=...; S=...;` stamps, where packaging needs exactly one")]
    Stamp {
        /// The script.
        script: String,
        /// How many stamps it holds.
        found: usize,
    },
    /// The release repository's `marketplace.json` has no `plugins` list of objects.
    #[error("{}: {problem}", path.display())]
    Marketplace {
        /// The file.
        path: PathBuf,
        /// What is wrong.
        problem: String,
    },
    /// The packaged tree breaks the plugin rules.
    #[error("the packaged plugins break the plugin rules:\n{}", violations.join("\n"))]
    Rules {
        /// One line per violation.
        violations: Vec<String>,
    },
}

/// The part of `.claude-plugin/plugin.json` the release tasks read.
#[derive(Debug, Deserialize)]
struct Manifest {
    name: String,
    version: String,
    #[serde(default)]
    description: Option<String>,
}

/// Reads `<plugin>/.claude-plugin/plugin.json`.
fn read_manifest(plugin: &Path) -> Result<Manifest, ReleaseError> {
    let path = plugin.join(".claude-plugin/plugin.json");
    let text = fs::read_to_string(&path).map_err(|source| ReleaseError::Io {
        path: path.clone(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| ReleaseError::Json { path, source })
}

/// A plugin, binary or target name: ASCII letters, digits, `-` and `_`, not empty, so it is one
/// path component everywhere.
fn checked_name(name: &str) -> Result<&str, ReleaseError> {
    let plain = !name.is_empty()
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    if plain {
        Ok(name)
    } else {
        Err(ReleaseError::BadName {
            name: name.to_owned(),
        })
    }
}

/// Copies `from` to `to`, creating `to`'s folder.
fn copy_file(from: &Path, to: &Path) -> Result<(), ReleaseError> {
    if let Some(parent) = to.parent() {
        fs::create_dir_all(parent).map_err(|source| ReleaseError::Io {
            path: parent.to_owned(),
            source,
        })?;
    }
    fs::copy(from, to).map_err(|source| ReleaseError::Io {
        path: from.to_owned(),
        source,
    })?;
    Ok(())
}

/// Copies every file of `plugin` (as `read_plugins` listed it from `from`) to `to`, refusing a
/// symlink or other non-regular file, and skipping the paths `skip` names.
fn copy_plugin(
    plugin: &plugins::Plugin,
    from: &Path,
    to: &Path,
    skip: impl Fn(&str) -> bool,
) -> Result<(), ReleaseError> {
    for file in &plugin.files {
        if skip(&file.path) {
            continue;
        }
        if file.kind != plugins::FileKind::Regular {
            return Err(ReleaseError::Plugin {
                plugin: plugin.name.clone(),
                problem: format!("{} is not a regular file", file.path),
            });
        }
        copy_file(&from.join(&file.path), &to.join(&file.path))?;
    }
    Ok(())
}

/// Marks `path` executable for everyone who can read it: hooks start launchers in exec form, and
/// launchers exec the builds, whatever mode an artifact download left them in.
#[cfg(unix)]
fn make_executable(path: &Path) -> Result<(), ReleaseError> {
    use std::os::unix::fs::PermissionsExt;
    let io_error = |source| ReleaseError::Io {
        path: path.to_owned(),
        source,
    };
    let mut permissions = fs::metadata(path).map_err(io_error)?.permissions();
    permissions.set_mode(permissions.mode() | 0o755);
    fs::set_permissions(path, permissions).map_err(io_error)
}

/// Windows has no executable bit; the release repository is assembled on Linux.
#[cfg(not(unix))]
fn make_executable(_path: &Path) -> Result<(), ReleaseError> {
    Ok(())
}
