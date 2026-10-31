//! The suite's layer rules (`AGENTS.md`, Layer rules) and a check of the workspace against them.
//!
//! Every member sits in one layer, found from its directory: Core under `crates/core/`, kits under
//! `crates/kits/`, a plugin's crates under `crates/<plugin>/` (its contract crate is the one named
//! `*-contract`), and dev tools under `tools/`.
//!
//! Core uses only Core; kits and contract crates use only Core; a plugin's crates use Core, kits,
//! any plugin's contract crate and their own plugin's crates; tools use Core, kits and contracts.
//! Dev and build dependencies are held to the same rules. Only binaries may use anyhow, and no
//! member is publishable.

use std::ffi::OsStr;
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Deserialize;

/// The plugin folders under `crates/`. Adding one is a crate-boundary change (H-16).
const PLUGINS: [&str; 7] = [
    "rust",
    "bench",
    "weigh-in",
    "knowledge",
    "review",
    "loop",
    "backlog",
];

/// A crate whose name ends with this is its plugin's contract crate.
const CONTRACT_SUFFIX: &str = "-contract";

/// The one dependency libraries may not use outside their tests (H-09).
const BINARY_ONLY: &str = "anyhow";

/// How a package uses a dependency.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `[dependencies]`.
    Normal,
    /// `[dev-dependencies]`.
    Dev,
    /// `[build-dependencies]`.
    Build,
}

/// One declared dependency of a member.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    /// The dependency's package name.
    pub name: String,
    /// The table that declares it.
    pub kind: Kind,
}

/// A workspace member as the check sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    /// The package name.
    pub name: String,
    /// The manifest's directory relative to the workspace root, `/`-separated.
    pub dir: String,
    /// Whether `cargo publish` would accept it.
    pub publishable: bool,
    /// Whether it builds a binary target.
    pub has_binary: bool,
    /// Its declared dependencies.
    pub dependencies: Vec<Dependency>,
}

/// A departure from the layer rules, borrowing its names from the checked packages.
#[derive(Debug, PartialEq, Eq)]
pub enum Violation<'a> {
    /// A member outside `crates/core/`, `crates/kits/`, `crates/<plugin>/` and `tools/`.
    Unplaced {
        /// The member.
        package: &'a str,
        /// Its directory.
        dir: &'a str,
    },
    /// An edge between members that the source's layer may not have.
    ForbiddenEdge {
        /// The depending member.
        from: &'a str,
        /// The member it depends on.
        to: &'a str,
    },
    /// A member without a binary target that uses anyhow outside its tests.
    AnyhowInLibrary {
        /// The member.
        package: &'a str,
    },
    /// A member `cargo publish` would accept.
    Publishable {
        /// The member.
        package: &'a str,
    },
}

impl fmt::Display for Violation<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unplaced { package, dir } => write!(
                f,
                "{package} is at {dir}, outside crates/core, crates/kits, crates/<plugin> and tools"
            ),
            Self::ForbiddenEdge { from, to } => {
                write!(f, "{from} depends on {to}, which its layer may not use")
            }
            Self::AnyhowInLibrary { package } => write!(
                f,
                "{package} has no binary target but depends on anyhow, which only binaries may use"
            ),
            Self::Publishable { package } => {
                write!(
                    f,
                    "{package} is publishable; turning publish on is an H-16 decision"
                )
            }
        }
    }
}

/// The layer a member belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Layer<'a> {
    Core,
    Kit,
    Contract,
    Plugin(&'a str),
    Tool,
}

impl<'a> Layer<'a> {
    fn of(package: &'a Package) -> Option<Self> {
        let mut parts = package.dir.split('/');
        let layer = match (parts.next(), parts.next(), parts.next()) {
            (Some("crates"), Some("core"), Some(_)) => Self::Core,
            (Some("crates"), Some("kits"), Some(_)) => Self::Kit,
            (Some("crates"), Some(plugin), Some(_)) if PLUGINS.contains(&plugin) => {
                if package.name.ends_with(CONTRACT_SUFFIX) {
                    Self::Contract
                } else {
                    Self::Plugin(plugin)
                }
            }
            (Some("tools"), Some(_), None) => Self::Tool,
            _ => return None,
        };
        parts.next().is_none().then_some(layer)
    }
}

/// Whether a member in layer `from` may depend on a member in layer `to`.
///
/// Core uses only Core; kits and contract crates use only Core; a plugin's crates use Core, kits,
/// any plugin's contract crate and their own plugin's crates; tools use Core, kits and contracts.
fn may_depend_on(from: Layer<'_>, to: Layer<'_>) -> bool {
    match (from, to) {
        (_, Layer::Core) | (Layer::Plugin(_) | Layer::Tool, Layer::Kit | Layer::Contract) => true,
        (Layer::Plugin(own), Layer::Plugin(other)) => own == other,
        _ => false,
    }
}

/// Every violation in `packages`, in member order.
#[must_use]
pub fn check(packages: &[Package]) -> Vec<Violation<'_>> {
    let mut found = Vec::new();
    for package in packages {
        let Some(layer) = Layer::of(package) else {
            found.push(Violation::Unplaced {
                package: &package.name,
                dir: &package.dir,
            });
            continue;
        };
        if package.publishable {
            found.push(Violation::Publishable {
                package: &package.name,
            });
        }
        for dependency in &package.dependencies {
            let target = packages
                .iter()
                .find(|member| member.name == dependency.name);
            if let Some(target) = target {
                if !Layer::of(target).is_some_and(|to| may_depend_on(layer, to)) {
                    found.push(Violation::ForbiddenEdge {
                        from: &package.name,
                        to: &target.name,
                    });
                }
            } else if dependency.name == BINARY_ONLY
                && dependency.kind != Kind::Dev
                && !package.has_binary
            {
                found.push(Violation::AnyhowInLibrary {
                    package: &package.name,
                });
            }
        }
    }
    found
}

/// Reading the workspace through `cargo metadata` failed.
#[derive(Debug, thiserror::Error)]
pub enum ReadError {
    /// cargo could not be started.
    #[error("could not run cargo metadata")]
    Spawn(#[source] std::io::Error),
    /// cargo ran and failed.
    #[error("cargo metadata failed ({status}): {stderr}")]
    Cargo {
        /// cargo's exit status.
        status: std::process::ExitStatus,
        /// What cargo wrote to stderr.
        stderr: String,
    },
    /// cargo's output was not the metadata this check reads.
    #[error("cargo metadata printed output this check cannot read")]
    Parse(#[source] serde_json::Error),
    /// A member's manifest is outside the workspace root.
    #[error("{manifest} is outside the workspace root")]
    OutsideWorkspace {
        /// The member's manifest.
        manifest: PathBuf,
    },
    /// A member's directory is not UTF-8.
    #[error("{manifest} has a directory that is not UTF-8")]
    NonUtf8 {
        /// The member's manifest.
        manifest: PathBuf,
    },
}

/// The members of the workspace whose root manifest is `manifest`, from
/// `cargo metadata --no-deps --locked` run with the program `cargo`.
///
/// # Errors
///
/// [`ReadError`] when cargo cannot run or fails, prints something else than metadata, or names a
/// member outside the workspace root or in a directory that is not UTF-8.
pub fn read_workspace(cargo: &OsStr, manifest: &Path) -> Result<Vec<Package>, ReadError> {
    // Unbounded and untimed on purpose: the pinned toolchain reporting on this repository's own
    // manifests, whose authors can already run code through build.rs (H-13 trust boundary).
    let output = Command::new(cargo)
        .args(["metadata", "--no-deps", "--locked", "--format-version", "1"])
        .arg("--manifest-path")
        .arg(manifest)
        .output()
        .map_err(ReadError::Spawn)?;
    if !output.status.success() {
        return Err(ReadError::Cargo {
            status: output.status,
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    let metadata: Metadata = serde_json::from_slice(&output.stdout).map_err(ReadError::Parse)?;
    metadata
        .packages
        .into_iter()
        .map(|package| package.into_package(&metadata.workspace_root))
        .collect()
}

/// The part of `cargo metadata --format-version 1` this check reads.
#[derive(Deserialize)]
struct Metadata {
    packages: Vec<PackageDto>,
    workspace_root: PathBuf,
}

#[derive(Deserialize)]
struct PackageDto {
    name: String,
    manifest_path: PathBuf,
    /// `null` when any registry may publish it, else the registries that may.
    publish: Option<Vec<String>>,
    dependencies: Vec<DependencyDto>,
    targets: Vec<TargetDto>,
}

#[derive(Deserialize)]
struct DependencyDto {
    name: String,
    kind: Option<KindDto>,
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
enum KindDto {
    Dev,
    Build,
}

#[derive(Deserialize)]
struct TargetDto {
    kind: Vec<String>,
}

impl PackageDto {
    fn into_package(self, root: &Path) -> Result<Package, ReadError> {
        let relative = self
            .manifest_path
            .parent()
            .and_then(|dir| dir.strip_prefix(root).ok());
        let Some(relative) = relative else {
            return Err(ReadError::OutsideWorkspace {
                manifest: self.manifest_path,
            });
        };
        let parts: Option<Vec<&str>> = relative
            .components()
            .map(|component| component.as_os_str().to_str())
            .collect();
        let Some(parts) = parts else {
            return Err(ReadError::NonUtf8 {
                manifest: self.manifest_path,
            });
        };
        Ok(Package {
            dir: parts.join("/"),
            publishable: self.publish.is_none_or(|registries| !registries.is_empty()),
            has_binary: self
                .targets
                .iter()
                .any(|target| target.kind.iter().any(|kind| kind == "bin")),
            dependencies: self
                .dependencies
                .into_iter()
                .map(|dependency| Dependency {
                    name: dependency.name,
                    kind: match dependency.kind {
                        None => Kind::Normal,
                        Some(KindDto::Dev) => Kind::Dev,
                        Some(KindDto::Build) => Kind::Build,
                    },
                })
                .collect(),
            name: self.name,
        })
    }
}
