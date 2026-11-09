//! Release builds of every plugin binary, into the plugins' `libexec/<target>/`.

use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::notes::read_metadata;
use super::{ReleaseError, checked_name, copy_file, make_executable};
use crate::plugins;

/// Builds every plugin binary for one target into the plugins' `libexec/<target>/`.
///
/// Each binary a plugin under `<repository>/plugins/` ships a launcher copy for is built with
/// `cargo build --locked --release --bin <binary> --target <target>` (`target` defaults to the
/// host `rustc -vV` names) and copied into `libexec/<target>/` of every plugin that ships that
/// binary's launcher. Cross-compiling needs the linker environment set by the caller, as the
/// release workflow does. Returns the files written.
///
/// # Errors
///
/// [`ReleaseError::NoHost`] when no target is given and `rustc -vV` names none,
/// [`ReleaseError::BadName`] for a target that is not one path component,
/// [`ReleaseError::Failed`] when a build fails, and the read, run and write errors.
pub fn build(
    cargo: &OsStr,
    rustc: &OsStr,
    repository: &Path,
    target: Option<&str>,
) -> Result<Vec<PathBuf>, ReleaseError> {
    let target = match target {
        Some(target) => checked_name(target)?.to_owned(),
        None => host(rustc)?,
    };
    let exe = if target.contains("-windows-") {
        ".exe"
    } else {
        ""
    };
    let all = plugins::read_plugins(&repository.join("plugins"))?;
    let binaries: BTreeSet<&str> = all
        .iter()
        .filter(|p| p.folder)
        .flat_map(plugins::launchers)
        .collect();
    let target_dir = read_metadata(repository, cargo)?.target_directory;
    let mut written = Vec::new();
    for binary in binaries {
        run(Command::new(cargo)
            .args([
                "build",
                "--locked",
                "--release",
                "--bin",
                binary,
                "--target",
            ])
            .arg(&target)
            .arg("--manifest-path")
            .arg(repository.join("Cargo.toml")))?;
        let built = target_dir
            .join(&target)
            .join("release")
            .join(format!("{binary}{exe}"));
        for plugin in all
            .iter()
            .filter(|p| p.folder && plugins::launchers(p).contains(&binary))
        {
            let placed = repository
                .join("plugins")
                .join(&plugin.name)
                .join("libexec")
                .join(&target)
                .join(format!("{binary}{exe}"));
            copy_file(&built, &placed)?;
            make_executable(&placed)?;
            written.push(placed);
        }
    }
    Ok(written)
}

/// The host target `rustc -vV` names on its `host: ` line.
fn host(rustc: &OsStr) -> Result<String, ReleaseError> {
    let output = Command::new(rustc)
        .arg("-vV")
        .output()
        .map_err(|source| ReleaseError::Spawn {
            program: "rustc -vV".to_owned(),
            source,
        })?;
    if !output.status.success() {
        return Err(ReleaseError::Failed {
            program: "rustc -vV".to_owned(),
            status: output.status,
            stderr: format!(": {}", String::from_utf8_lossy(&output.stderr).trim()),
        });
    }
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .map(|host| checked_name(host.trim()).map(str::to_owned))
        .ok_or(ReleaseError::NoHost)?
}

/// Runs a build with its output shown, as `cargo build` would be by hand.
fn run(command: &mut Command) -> Result<(), ReleaseError> {
    // Untimed on purpose: a release build of this repository's own code (H-13 trust boundary, as
    // `read_metadata`).
    let status = command.status().map_err(|source| ReleaseError::Spawn {
        program: "cargo build".to_owned(),
        source,
    })?;
    if status.success() {
        Ok(())
    } else {
        Err(ReleaseError::Failed {
            program: "cargo build".to_owned(),
            status,
            stderr: String::new(),
        })
    }
}
