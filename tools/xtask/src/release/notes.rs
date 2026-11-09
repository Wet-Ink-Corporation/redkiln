//! A plugin's release notes and the agreement of its version sites.

use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::process::Command;

use serde::Deserialize;

use super::{ReleaseError, checked_name, read_manifest};
use crate::plugins;

/// The body of a changelog's `## <version>` section.
///
/// That is the lines under the heading up to the next `## ` heading, without the blank lines
/// around them. The heading's first word after `## ` is the
/// version, so `## 2.0.0 (2026-11-10)` is 2.0.0's section. `None` when there is no such section
/// or it is empty.
#[must_use]
pub fn changelog_section<'a>(changelog: &'a str, version: &str) -> Option<&'a str> {
    let mut start = None;
    let mut end = changelog.len();
    let mut at = 0;
    for line in changelog.split_inclusive('\n') {
        let next = at + line.len();
        if let Some(title) = line.strip_prefix("## ") {
            if start.is_some() {
                end = at;
                break;
            }
            if title.split_whitespace().next() == Some(version) {
                start = Some(next);
            }
        }
        at = next;
    }
    let body = changelog.get(start?..end)?.trim();
    (!body.is_empty()).then_some(body)
}

/// Checks a release's version sites and returns its notes.
///
/// Releasing plugin folder `plugin` (under `<repository>/plugins/`) at `version` must agree with
/// every version site: its manifest, the manifest of every plugin that ships one of
/// its binaries, and the version of each binary's package (from `cargo metadata`, run with the
/// program `cargo`). Returns the plugin changelog's section for `version`: the release notes.
///
/// # Errors
///
/// [`ReleaseError::VersionMismatch`] for the first site that disagrees,
/// [`ReleaseError::NoChangelogSection`] when `CHANGELOG.md` has no section for `version`,
/// [`ReleaseError::Plugin`] when the plugin does not exist or ships no binary, and the read and
/// run errors otherwise.
pub fn release_check(
    repository: &Path,
    cargo: &OsStr,
    plugin: &str,
    version: &str,
) -> Result<String, ReleaseError> {
    let plugin = checked_name(plugin)?;
    let root = repository.join("plugins");
    let all = plugins::read_plugins(&root)?;
    let target = all
        .iter()
        .find(|p| p.folder && p.name == plugin)
        .ok_or_else(|| ReleaseError::Plugin {
            plugin: plugin.to_owned(),
            problem: format!("no folder under {}", root.display()),
        })?;
    let binaries = plugins::launchers(target);
    if binaries.is_empty() {
        return Err(ReleaseError::Plugin {
            plugin: plugin.to_owned(),
            problem: "ships no launcher copy, so there is no binary to release".to_owned(),
        });
    }
    for other in all.iter().filter(|p| p.folder) {
        if other.name != plugin
            && !plugins::launchers(other)
                .iter()
                .any(|b| binaries.contains(b))
        {
            continue;
        }
        let manifest = read_manifest(&root.join(&other.name))?;
        agree(
            format!("plugins/{}/.claude-plugin/plugin.json", other.name),
            &manifest.version,
            version,
        )?;
    }
    let packages = read_bin_packages(repository, cargo)?;
    for binary in &binaries {
        let package = packages
            .iter()
            .find(|p| p.bins().any(|b| b == *binary))
            .ok_or_else(|| ReleaseError::Plugin {
                plugin: plugin.to_owned(),
                problem: format!("no workspace package builds the binary {binary}"),
            })?;
        agree(
            format!("package {} (binary {binary})", package.name),
            &package.version,
            version,
        )?;
    }
    let path = root.join(plugin).join("CHANGELOG.md");
    let changelog = fs::read_to_string(&path).map_err(|source| ReleaseError::Io {
        path: path.clone(),
        source,
    })?;
    changelog_section(&changelog, version)
        .map(str::to_owned)
        .ok_or_else(|| ReleaseError::NoChangelogSection {
            path,
            version: version.to_owned(),
        })
}

fn agree(site: String, found: &str, version: &str) -> Result<(), ReleaseError> {
    if found == version {
        Ok(())
    } else {
        Err(ReleaseError::VersionMismatch {
            site,
            found: found.to_owned(),
            version: version.to_owned(),
        })
    }
}

/// The part of `cargo metadata --format-version 1` the release tasks read.
#[derive(Debug, Deserialize)]
pub(super) struct Metadata {
    pub(super) packages: Vec<PackageDto>,
    pub(super) target_directory: std::path::PathBuf,
}

#[derive(Debug, Deserialize)]
pub(super) struct PackageDto {
    name: String,
    version: String,
    targets: Vec<TargetDto>,
}

impl PackageDto {
    /// The names of the package's binary targets.
    fn bins(&self) -> impl Iterator<Item = &str> {
        self.targets
            .iter()
            .filter(|t| t.kind.iter().any(|k| k == "bin"))
            .map(|t| t.name.as_str())
    }
}

#[derive(Debug, Deserialize)]
struct TargetDto {
    name: String,
    kind: Vec<String>,
}

fn read_bin_packages(repository: &Path, cargo: &OsStr) -> Result<Vec<PackageDto>, ReleaseError> {
    read_metadata(repository, cargo).map(|m| m.packages)
}

/// `cargo metadata --no-deps --locked` of the workspace at `repository`.
pub(super) fn read_metadata(repository: &Path, cargo: &OsStr) -> Result<Metadata, ReleaseError> {
    // Unbounded and untimed on purpose, as in `layers::read_workspace`: the pinned toolchain
    // reporting on this repository's own manifests (H-13 trust boundary).
    let program = "cargo metadata".to_owned();
    let output = Command::new(cargo)
        .args(["metadata", "--no-deps", "--locked", "--format-version", "1"])
        .arg("--manifest-path")
        .arg(repository.join("Cargo.toml"))
        .output()
        .map_err(|source| ReleaseError::Spawn {
            program: program.clone(),
            source,
        })?;
    if !output.status.success() {
        return Err(ReleaseError::Failed {
            program,
            status: output.status,
            stderr: format!(": {}", String::from_utf8_lossy(&output.stderr).trim()),
        });
    }
    serde_json::from_slice(&output.stdout).map_err(|source| ReleaseError::Json {
        path: repository.join("Cargo.toml"),
        source,
    })
}
