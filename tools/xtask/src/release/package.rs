//! Staging every plugin with the builds of its binaries, as the release ships it.

use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use super::{ReleaseError, checked_name, copy_file, copy_plugin, make_executable, read_manifest};
use crate::plugins;

/// The build a Cowork fetch script downloads (`libexec/<binary>-cowork-fetch.sh`, M1-D).
const COWORK_TARGET: &str = "x86_64-unknown-linux-musl";

/// The Windows builds that may become `libexec/<binary>.exe`, the one exec-form hooks start on
/// Windows (M1-E), in order of preference: msvc is what the release ships first.
const WINDOWS_TARGETS: [&str; 2] = ["x86_64-pc-windows-msvc", "x86_64-pc-windows-gnu"];

/// The largest build hashed for a stamp: claude.ai's per-plugin limit (H-13).
const MAX_BUILD: u64 = 200 * 1024 * 1024;

/// Whether a packaged Cowork fetch script may download its build from the release.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Download {
    /// Stamp the SHA-256 of the musl build, which the script downloads from the GitHub release:
    /// only for a package whose release carries that same build.
    Stamp,
    /// Stamp `none`: the script uses a copy in a connected folder and never downloads.
    Off,
}

/// One plugin as [`package`] staged it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packaged {
    /// The plugin's name from its manifest, which is also its folder in the output.
    pub name: String,
    /// Its version.
    pub version: String,
    /// How many builds it carries under `libexec/<target>/`.
    pub builds: usize,
}

/// Builds by binary, then by target: the file each `libexec/<target>/<binary>[.exe]` holds.
type Builds = BTreeMap<String, BTreeMap<String, PathBuf>>;

/// Stages every plugin under `plugins_dir` into `out/<name>/`, `<name>` from its manifest:
///
/// - every file of the plugin except its builds, a root `libexec/<binary>.exe` and `*.test.ts`;
/// - for each binary it ships a launcher copy for, every build of that binary found in any
///   plugin's `libexec/<target>/`, so a mod plugin carries the same builds as its skill plugin;
/// - the msvc build (else the gnu build) also at `libexec/<binary>.exe` (M1-E);
/// - `libexec/<binary>-cowork-fetch.sh` stamped with the version and, with [`Download::Stamp`],
///   the SHA-256 of the musl build.
///
/// Launchers and builds are made executable, and the staged tree is checked against the plugin
/// rules. `out` must not exist.
///
/// # Errors
///
/// [`ReleaseError::OutExists`], [`ReleaseError::NoBuild`] when a binary has no build at all (or no
/// musl build to stamp), [`ReleaseError::VersionsDiffer`] when two plugins that ship one binary
/// carry different versions, [`ReleaseError::Stamp`], [`ReleaseError::Rules`] when the staged tree
/// breaks the plugin rules, and the read and write errors.
pub fn package(
    plugins_dir: &Path,
    out: &Path,
    download: Download,
) -> Result<Vec<Packaged>, ReleaseError> {
    if fs::symlink_metadata(out).is_ok() {
        return Err(ReleaseError::OutExists {
            path: out.to_owned(),
        });
    }
    let all = plugins::read_plugins(plugins_dir)?;
    let builds = pool_builds(&all, plugins_dir);
    let mut versions: BTreeMap<&str, (&str, String)> = BTreeMap::new();
    let mut packaged = Vec::new();
    for plugin in all.iter().filter(|p| p.folder) {
        let from = plugins_dir.join(&plugin.name);
        let manifest = read_manifest(&from)?;
        let to = out.join(checked_name(&manifest.name)?);
        let binaries = plugins::launchers(plugin);
        for binary in &binaries {
            same_version(&mut versions, binary, &plugin.name, &manifest.version)?;
        }
        copy_plugin(plugin, &from, &to, |path| skipped(path, &binaries))?;
        let mut count = 0;
        for binary in &binaries {
            count += place_builds(&plugin.name, binary, &builds, &to)?;
            make_executable(&to.join("libexec").join(binary))?;
            let script = to.join(format!("libexec/{binary}-cowork-fetch.sh"));
            if script.is_file() {
                let sha = match download {
                    Download::Off => None,
                    Download::Stamp => Some(cowork_sha(&plugin.name, binary, &builds)?),
                };
                stamp_file(&script, &manifest.version, sha.as_deref())?;
            }
        }
        packaged.push(Packaged {
            name: manifest.name,
            version: manifest.version,
            builds: count,
        });
    }
    let violations: Vec<String> = plugins::check(&plugins::read_plugins(out)?)
        .iter()
        .map(ToString::to_string)
        .collect();
    if violations.is_empty() {
        Ok(packaged)
    } else {
        Err(ReleaseError::Rules { violations })
    }
}

/// Every `libexec/<target>/<binary>[.exe]` of every plugin, for a binary that plugin ships a
/// launcher copy for. The first plugin (in name order) that has a target's build provides it.
fn pool_builds(all: &[plugins::Plugin], plugins_dir: &Path) -> Builds {
    let mut builds = Builds::new();
    for plugin in all.iter().filter(|p| p.folder) {
        let binaries = plugins::launchers(plugin);
        for file in plugin
            .files
            .iter()
            .filter(|f| f.kind == plugins::FileKind::Regular)
        {
            let Some((target, build)) = build_of(&file.path, &binaries) else {
                continue;
            };
            builds
                .entry(build.to_owned())
                .or_default()
                .entry(target.to_owned())
                .or_insert_with(|| plugins_dir.join(&plugin.name).join(&file.path));
        }
    }
    builds
}

/// `(target, binary)` when `path` is `libexec/<target>/<binary>[.exe]` for one of `binaries`.
fn build_of<'a>(path: &'a str, binaries: &[&str]) -> Option<(&'a str, &'a str)> {
    let mut parts = path.split('/');
    let (Some("libexec"), Some(target), Some(file), None) =
        (parts.next(), parts.next(), parts.next(), parts.next())
    else {
        return None;
    };
    let binary = file.strip_suffix(".exe").unwrap_or(file);
    (target.contains('-') && checked_name(target).is_ok() && binaries.contains(&binary))
        .then_some((target, binary))
}

/// The paths a package never copies from its source: builds (re-added from the pool), a root
/// `libexec/<binary>.exe` (rebuilt from the pool, so a stale one never ships) and the mod's tests.
fn skipped(path: &str, binaries: &[&str]) -> bool {
    let build_folder = path
        .strip_prefix("libexec/")
        .is_some_and(|rest| rest.contains('/'));
    let root_exe = path
        .strip_prefix("libexec/")
        .and_then(|name| name.strip_suffix(".exe"))
        .is_some_and(|binary| binaries.contains(&binary));
    build_folder || root_exe || path.ends_with(".test.ts")
}

fn same_version<'a>(
    versions: &mut BTreeMap<&'a str, (&'a str, String)>,
    binary: &'a str,
    plugin: &'a str,
    version: &str,
) -> Result<(), ReleaseError> {
    match versions.get(binary) {
        Some((first, first_version)) if first_version != version => {
            Err(ReleaseError::VersionsDiffer {
                binary: binary.to_owned(),
                first: (*first).to_owned(),
                first_version: first_version.clone(),
                second: plugin.to_owned(),
                second_version: version.to_owned(),
            })
        }
        Some(_) => Ok(()),
        None => {
            versions.insert(binary, (plugin, version.to_owned()));
            Ok(())
        }
    }
}

/// Copies every build of `binary` into `to/libexec/<target>/`, and the preferred Windows build to
/// `to/libexec/<binary>.exe`. Returns how many builds it placed.
fn place_builds(
    plugin: &str,
    binary: &str,
    builds: &Builds,
    to: &Path,
) -> Result<usize, ReleaseError> {
    let found = builds
        .get(binary)
        .filter(|targets| !targets.is_empty())
        .ok_or_else(|| ReleaseError::NoBuild {
            plugin: plugin.to_owned(),
            binary: binary.to_owned(),
            for_target: String::new(),
        })?;
    for (target, from) in found {
        let name = from
            .file_name()
            .map_or_else(|| binary.into(), std::ffi::OsStr::to_os_string);
        let placed = to.join("libexec").join(target).join(name);
        copy_file(from, &placed)?;
        make_executable(&placed)?;
    }
    if let Some(windows) = WINDOWS_TARGETS.iter().find_map(|target| {
        found
            .get(*target)
            .filter(|p| p.extension().is_some_and(|e| e == "exe"))
    }) {
        let placed = to.join("libexec").join(format!("{binary}.exe"));
        copy_file(windows, &placed)?;
        make_executable(&placed)?;
    }
    Ok(found.len())
}

/// The lowercase hex SHA-256 of the musl build of `binary`.
fn cowork_sha(plugin: &str, binary: &str, builds: &Builds) -> Result<String, ReleaseError> {
    let path = builds
        .get(binary)
        .and_then(|targets| targets.get(COWORK_TARGET))
        .ok_or_else(|| ReleaseError::NoBuild {
            plugin: plugin.to_owned(),
            binary: binary.to_owned(),
            for_target: format!(" for {COWORK_TARGET}, which its Cowork fetch script downloads"),
        })?;
    let io_error = |source| ReleaseError::Io {
        path: path.clone(),
        source,
    };
    let mut hasher = Sha256::new();
    let copied = io::copy(
        &mut File::open(path).map_err(io_error)?.take(MAX_BUILD + 1),
        &mut hasher,
    )
    .map_err(io_error)?;
    if copied > MAX_BUILD {
        return Err(io_error(io::Error::other(format!(
            "larger than {MAX_BUILD} bytes"
        ))));
    }
    // Each nibble is below 16, so `from_digit` gives a digit for every one.
    Ok(hasher
        .finalize()
        .iter()
        .flat_map(|byte| [byte >> 4, byte & 0x0f])
        .filter_map(|nibble| char::from_digit(u32::from(nibble), 16))
        .collect())
}

fn stamp_file(script: &Path, version: &str, sha: Option<&str>) -> Result<(), ReleaseError> {
    let io_error = |source| ReleaseError::Io {
        path: script.to_owned(),
        source,
    };
    let text = fs::read_to_string(script).map_err(io_error)?;
    let stamped = stamp(&text, version, sha).map_err(|error| match error {
        ReleaseError::Stamp { found, .. } => ReleaseError::Stamp {
            script: script.display().to_string(),
            found,
        },
        other => other,
    })?;
    fs::write(script, stamped).map_err(io_error)
}

/// `script` with its one `V=<version>; S=<sha256>;` stamp (each value on one line, without `;`)
/// set to `version` and `sha`, or `none` without one.
///
/// # Errors
///
/// [`ReleaseError::Stamp`] when the script holds no stamp or more than one.
pub fn stamp(script: &str, version: &str, sha: Option<&str>) -> Result<String, ReleaseError> {
    let found: Vec<(usize, usize)> = script
        .match_indices("V=")
        .filter_map(|(at, _)| stamp_end(script, at).map(|end| (at, end)))
        .collect();
    // Both ends come from `match_indices` and `find` on `script`, so both slices exist.
    let one = match found.as_slice() {
        [(start, end)] => script.get(..*start).zip(script.get(*end..)),
        _ => None,
    };
    let (head, tail) = one.ok_or_else(|| ReleaseError::Stamp {
        script: "the script".to_owned(),
        found: found.len(),
    })?;
    Ok(format!(
        "{head}V={version}; S={};{tail}",
        sha.unwrap_or("none")
    ))
}

/// The end (exclusive) of a stamp starting at `at` (`V=...; S=...;`), or `None` when none does.
fn stamp_end(script: &str, at: usize) -> Option<usize> {
    let value = |text: &str| {
        let end = text.find(';')?;
        (!text.get(..end)?.contains('\n')).then_some(end)
    };
    let after_v = script.get(at + "V=".len()..)?;
    let v_end = value(after_v)?;
    let after_s = after_v.get(v_end + 1..)?.strip_prefix(" S=")?;
    let s_end = value(after_s)?;
    Some(at + "V=".len() + v_end + 1 + " S=".len() + s_end + 1)
}
