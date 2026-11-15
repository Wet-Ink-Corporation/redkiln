//! A `bench` binary, placed in each plugin copy where the plugin's launcher
//! (`plugins/bench/libexec/bench`) looks first, so the hooks and skills of a run find a binary:
//! the launcher's release download fails inside the eval sandbox.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::scaffold::make_executable;

/// The file name of `bench` on this platform: `bench` plus its executable suffix.
pub(crate) fn bench_file() -> String {
    format!("bench{}", std::env::consts::EXE_SUFFIX)
}

/// Why no `bench` binary could be found to place in the plugin copies.
#[derive(Debug, thiserror::Error)]
pub enum BenchBinaryError {
    /// `--bench-binary` names no file.
    #[error("--bench-binary {} is not a file", given.display())]
    NotAFile {
        /// The path given.
        given: PathBuf,
    },
    /// Without `--bench-binary`, no `bench` file sits beside the running `skillcheck`.
    #[error(
        "no bench binary at {}, beside skillcheck; build it (`cargo build -p bench`) or pass \
         --bench-binary <PATH>",
        looked.display()
    )]
    NotBeside {
        /// Where it was looked for.
        looked: PathBuf,
    },
    /// The running `skillcheck`'s own path could not be found.
    #[error("finding skillcheck's own path, beside which bench is looked for")]
    OwnPath(#[source] io::Error),
}

/// The `bench` binary to place: `given` (`--bench-binary`) when it is a file, else the
/// [`bench_file`] beside `skillcheck`, the running executable, when that is a file.
///
/// # Errors
/// [`BenchBinaryError::NotAFile`] when `given` is not a file (a `bench` beside `skillcheck`
/// is not tried then); [`BenchBinaryError::NotBeside`] when nothing is given and no file
/// is there.
pub(crate) fn locate_bench(
    given: Option<&Path>,
    skillcheck: &Path,
) -> Result<PathBuf, BenchBinaryError> {
    if let Some(given) = given {
        return if given.is_file() {
            Ok(given.to_path_buf())
        } else {
            Err(BenchBinaryError::NotAFile {
                given: given.to_path_buf(),
            })
        };
    }
    let looked = skillcheck
        .parent()
        .unwrap_or_else(|| Path::new(""))
        .join(bench_file());
    if looked.is_file() {
        Ok(looked)
    } else {
        Err(BenchBinaryError::NotBeside { looked })
    }
}

/// A target directory of `libexec/` that the launcher tries first on some host, and the
/// binary's file name there.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LauncherTarget {
    triple: &'static str,
    file: &'static str,
}

impl LauncherTarget {
    /// The target the launcher tries first on a host whose `std::env::consts` `OS` and `ARCH`
    /// are `os` and `arch`, or `None` for a host it does not know (it lists Linux and macOS on
    /// x86-64 and ARM64, and Windows on x86-64).
    pub(crate) fn of(os: &str, arch: &str) -> Option<Self> {
        let (triple, file) = match (os, arch) {
            ("linux", "x86_64") => ("x86_64-unknown-linux-musl", "bench"),
            ("linux", "aarch64") => ("aarch64-unknown-linux-musl", "bench"),
            ("macos", "aarch64") => ("aarch64-apple-darwin", "bench"),
            ("macos", "x86_64") => ("x86_64-apple-darwin", "bench"),
            ("windows", "x86_64") => ("x86_64-pc-windows-msvc", "bench.exe"),
            _ => return None,
        };
        Some(Self { triple, file })
    }

    /// The target of this host, if the launcher knows it.
    pub(crate) fn host() -> Option<Self> {
        Self::of(std::env::consts::OS, std::env::consts::ARCH)
    }

    /// Its target triple, the directory under `libexec/`.
    pub(crate) const fn triple(self) -> &'static str {
        self.triple
    }

    /// Where the launcher looks, relative to a plugin's root.
    fn relative(self) -> PathBuf {
        Path::new("libexec").join(self.triple).join(self.file)
    }
}

/// The binary to place in each plugin copy, and the launcher target to place it under.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Placement<'a> {
    /// The `bench` binary ([`locate_bench`]).
    pub(crate) binary: &'a Path,
    /// The host's launcher target.
    pub(crate) target: LauncherTarget,
}

impl Placement<'_> {
    /// Places the binary in the plugin copy at `copy` ([`install`]).
    ///
    /// # Errors
    /// As [`install`].
    pub(crate) fn place_in(self, copy: &Path) -> io::Result<()> {
        install(self.binary, copy, self.target)
    }
}

/// Copies the binary at `binary` into the plugin copy at `copy`, where the launcher tries
/// first on `target`'s host, replacing any binary there, executable on unix.
///
/// # Errors
/// The I/O error of making the directory, removing the old binary or copying the new one.
pub(crate) fn install(binary: &Path, copy: &Path, target: LauncherTarget) -> io::Result<()> {
    let placed = copy.join(target.relative());
    if let Some(dir) = placed.parent() {
        fs::create_dir_all(dir)?;
    }
    // A read-only binary the source plugin carried cannot be opened for writing: remove it.
    if let Err(error) = fs::remove_file(&placed)
        && error.kind() != io::ErrorKind::NotFound
    {
        return Err(error);
    }
    fs::copy(binary, &placed)?;
    make_executable(&placed)
}

#[cfg(test)]
mod tests;
