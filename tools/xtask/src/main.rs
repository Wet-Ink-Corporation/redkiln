//! `cargo xtask <task>`: repository tasks for the redkiln workspace.
//!
//! - `ids`: check `.kb/` against the knowledge-base id rules (Knowledge call C): one id per
//!   atom, each atom in its kind's folder, one question per `wi-` id.
//! - `layers`: check the workspace against the layer rules in `AGENTS.md`.
//! - `plugins`: check `plugins/` against the plugin rules in `AGENTS.md` (CLI first, harness
//!   second), and `crates/` against its rule that harness protocol stays in one adapter module
//!   per binary.
//!
//! Each exits 0 when the repository follows its rules, 1 with one line per violation when it does
//! not, and 2 when it cannot check.

use std::env;
use std::ffi::OsString;
use std::fmt::Display;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use xtask::{harness, ids, layers, plugins};

const USAGE: &str = "usage: cargo xtask <ids|layers|plugins>";

fn main() -> ExitCode {
    let task: Vec<OsString> = env::args_os().skip(1).collect();
    let outcome = match task.as_slice() {
        [name] if name == "ids" => run_ids(),
        [name] if name == "layers" => run_layers(),
        [name] if name == "plugins" => run_plugins(),
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
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let packages =
        layers::read_workspace(&cargo, &manifest).map_err(|error| error_chain(&error))?;
    let violations = layers::check(&packages);
    conclude("layers", &violations, "workspace members", packages.len())
}

/// Checks this repository's `plugins/` against the plugin rules, then its `crates/` against the
/// rule that harness protocol stays in an adapter module (`harness`).
fn run_plugins() -> Result<ExitCode, String> {
    let plugins = plugins::read_plugins(&repository().join("plugins"))
        .map_err(|error| error_chain(&error))?;
    let sources =
        harness::read_sources(&repository().join("crates")).map_err(|error| error_chain(&error))?;
    let violations: Vec<String> = plugins::check(&plugins)
        .iter()
        .map(ToString::to_string)
        .chain(harness::check(&sources).iter().map(ToString::to_string))
        .collect();
    let checked = format!("{}, Rust files checked: {}", plugins.len(), sources.len());
    conclude("plugins", &violations, "plugins", checked)
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
