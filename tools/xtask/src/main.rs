//! `cargo xtask <task>`: repository tasks for the redkiln workspace.
//!
//! - `ids`: check `.kb/` against the knowledge-base id rules (Knowledge call C): one id per
//!   atom, each atom in its kind's folder, one question per `wi-` id.
//! - `layers`: check the workspace against the layer rules in `AGENTS.md`.
//! - `plugins`: check `plugins/` against the plugin rules in `AGENTS.md` (CLI first, harness
//!   second), every subcommand a prompt names against its binary's command list, and `crates/`
//!   against the rule that harness protocol stays in one adapter module per binary.
//!
//! Each exits 0 when the repository follows its rules, 1 with one line per violation when it does
//! not, and 2 when it cannot check.
//!
//! The release tasks (`docs/plan/move-1-weigh-in.md`, Release and cutover) exit 0 when done and 2
//! when they cannot finish:
//!
//! - `build [--target <triple>]`: release builds of every plugin binary into the plugins'
//!   `libexec/<target>/` (the host's target by default).
//! - `package <out> [--download]`: every plugin staged into `<out>/<name>/` with its builds, the
//!   Windows build at `libexec/<binary>.exe` and its Cowork fetch script stamped (with the musl
//!   build's SHA-256 under `--download`), checked against the plugin rules.
//! - `marketplace <packages> <release-repository>`: the packaged plugins written into a checkout
//!   of the release repository and its `marketplace.json`.
//! - `release-check <plugin> <version>`: every version site of `plugins/<plugin>` is `version`;
//!   prints its changelog section, the release notes.

use std::env;
use std::ffi::{OsStr, OsString};
use std::fmt::Display;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use xtask::{harness, ids, layers, plugins, release};

const USAGE: &str = "usage: cargo xtask <ids|layers|plugins>\n       \
cargo xtask build [--target <triple>]\n       \
cargo xtask package <out> [--download]\n       \
cargo xtask marketplace <packages> <release-repository>\n       \
cargo xtask release-check <plugin> <version>";

fn main() -> ExitCode {
    let task: Vec<OsString> = env::args_os().skip(1).collect();
    let outcome = match task.as_slice() {
        [name] if name == "ids" => run_ids(),
        [name] if name == "layers" => run_layers(),
        [name] if name == "plugins" => run_plugins(),
        [name] if name == "build" => run_build(None),
        [name, flag, target] if name == "build" && flag == "--target" => run_build(Some(target)),
        [name, out] if name == "package" => run_package(out, release::Download::Off),
        [name, out, flag] if name == "package" && flag == "--download" => {
            run_package(out, release::Download::Stamp)
        }
        [name, packages, repository] if name == "marketplace" => {
            run_marketplace(packages, repository)
        }
        [name, plugin, version] if name == "release-check" => run_release_check(plugin, version),
        _ => Err(USAGE.to_owned()),
    };
    match outcome {
        Ok(code) => code,
        Err(message) => report(&message),
    }
}

/// Writes `message` to stderr and returns exit code 2, the code for "could not check".
fn report(message: &str) -> ExitCode {
    // A failed write of the error has nowhere else to go; the exit code reports it either way.
    if writeln!(io::stderr().lock(), "xtask: {message}").is_err() {
        return ExitCode::from(2);
    }
    ExitCode::from(2)
}

/// The repository root, two levels above this crate.
fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Checks this repository's `.kb/` against the knowledge-base id rules.
fn run_ids() -> Result<ExitCode, String> {
    let atoms = ids::read_kb(&repository().join(".kb")).map_err(|error| error_chain(&error))?;
    let violations = ids::check(&atoms);
    conclude("ids", &violations, "knowledge-base files", atoms.len())
}

/// Checks this workspace's members against the layer rules.
fn run_layers() -> Result<ExitCode, String> {
    let manifest = repository().join("Cargo.toml");
    let packages =
        layers::read_workspace(&cargo(), &manifest).map_err(|error| error_chain(&error))?;
    let violations = layers::check(&packages);
    conclude("layers", &violations, "workspace members", packages.len())
}

/// The command lists of the binaries plugins ship, from their contract crates.
const CLIS: [plugins::Cli<'static>; 2] = [
    plugins::Cli {
        binary: bench_contract::cli::BINARY,
        commands: bench_contract::cli::COMMANDS,
        value_options: bench_contract::cli::VALUE_OPTIONS,
    },
    plugins::Cli {
        binary: weighin_contract::cli::BINARY,
        commands: weighin_contract::cli::COMMANDS,
        value_options: weighin_contract::cli::VALUE_OPTIONS,
    },
];

/// Checks this repository's `plugins/` against the plugin rules and every subcommand its prompts
/// name against the binaries' command lists, then its `crates/` against the rule that harness
/// protocol stays in an adapter module (`harness`).
fn run_plugins() -> Result<ExitCode, String> {
    let plugins = plugins::read_plugins(&repository().join("plugins"))
        .map_err(|error| error_chain(&error))?;
    let sources =
        harness::read_sources(&repository().join("crates")).map_err(|error| error_chain(&error))?;
    let violations: Vec<String> = plugins::check(&plugins)
        .iter()
        .chain(&plugins::check_commands(&plugins, &CLIS))
        .map(ToString::to_string)
        .chain(harness::check(&sources).iter().map(ToString::to_string))
        .collect();
    let checked = format!("{}, Rust files checked: {}", plugins.len(), sources.len());
    conclude("plugins", &violations, "plugins", checked)
}

/// The program cargo runs this task with, or `cargo` on `PATH`.
fn cargo() -> OsString {
    env::var_os("CARGO").unwrap_or_else(|| "cargo".into())
}

/// Builds every plugin binary for `target` (the host's when `None`).
fn run_build(target: Option<&OsStr>) -> Result<ExitCode, String> {
    let target = target
        .map(|t| t.to_str().ok_or("--target is not UTF-8"))
        .transpose()?;
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let written = release::build(&cargo(), &rustc, &repository(), target)
        .map_err(|error| error_chain(&error))?;
    let mut out = io::stdout().lock();
    for path in &written {
        writeln!(out, "build: {}", path.display()).map_err(|error| error.to_string())?;
    }
    Ok(ExitCode::SUCCESS)
}

/// Stages every plugin into `out`.
fn run_package(out: &OsStr, download: release::Download) -> Result<ExitCode, String> {
    let packaged = release::package(&repository().join("plugins"), Path::new(out), download)
        .map_err(|error| error_chain(&error))?;
    let mut stdout = io::stdout().lock();
    for plugin in &packaged {
        writeln!(
            stdout,
            "package: {} {} with {} build(s)",
            plugin.name, plugin.version, plugin.builds
        )
        .map_err(|error| error.to_string())?;
    }
    Ok(ExitCode::SUCCESS)
}

/// Writes the packaged plugins into a release repository checkout.
fn run_marketplace(packages: &OsStr, repository: &OsStr) -> Result<ExitCode, String> {
    let written = release::assemble(Path::new(packages), Path::new(repository))
        .map_err(|error| error_chain(&error))?;
    writeln!(io::stdout().lock(), "marketplace: {}", written.join(", "))
        .map_err(|error| error.to_string())?;
    Ok(ExitCode::SUCCESS)
}

/// Checks a plugin's version sites and prints its release notes.
fn run_release_check(plugin: &OsStr, version: &OsStr) -> Result<ExitCode, String> {
    let (Some(plugin), Some(version)) = (plugin.to_str(), version.to_str()) else {
        return Err("the plugin and version must be UTF-8".to_owned());
    };
    let notes = release::release_check(&repository(), &cargo(), plugin, version)
        .map_err(|error| error_chain(&error))?;
    writeln!(io::stdout().lock(), "{notes}").map_err(|error| error.to_string())?;
    Ok(ExitCode::SUCCESS)
}

/// Reports `violations` on stderr, or an ok line on stdout naming how many `things` were checked.
fn conclude(
    task: &str,
    violations: &[impl Display],
    things: &str,
    checked: impl Display,
) -> Result<ExitCode, String> {
    let mut out = io::stderr().lock();
    for violation in violations {
        writeln!(out, "{task}: {violation}").map_err(|error| error.to_string())?;
    }
    if violations.is_empty() {
        writeln!(
            io::stdout().lock(),
            "{task}: ok, {things} checked: {checked}"
        )
        .map_err(|error| error.to_string())?;
        Ok(ExitCode::SUCCESS)
    } else {
        Ok(ExitCode::FAILURE)
    }
}

/// `error` and each of its sources, joined with `: `.
fn error_chain(error: &dyn std::error::Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        message.push_str(": ");
        message.push_str(&cause.to_string());
        source = cause.source();
    }
    message
}
