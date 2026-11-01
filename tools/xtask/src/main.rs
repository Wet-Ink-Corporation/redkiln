//! `cargo xtask <task>`: repository tasks for the redkiln workspace.
//!
//! - `layers`: check the workspace against the layer rules in `AGENTS.md`.
//! - `plugins`: check `plugins/` against the plugin rules in `AGENTS.md` (CLI first, harness
//!   second).
//!
//! Each exits 0 when the repository follows its rules, 1 with one line per violation when it does
//! not, and 2 when it cannot check.

use std::env;
use std::ffi::OsString;
use std::fmt::Display;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use xtask::{layers, plugins};

const USAGE: &str = "usage: cargo xtask <layers|plugins>";

fn main() -> ExitCode {
    let task: Vec<OsString> = env::args_os().skip(1).collect();
    let outcome = match task.as_slice() {
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

/// Checks this workspace's members against the layer rules.
fn run_layers() -> Result<ExitCode, String> {
    let manifest = repository().join("Cargo.toml");
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let packages =
        layers::read_workspace(&cargo, &manifest).map_err(|error| error_chain(&error))?;
    let violations = layers::check(&packages);
    conclude("layers", &violations, "workspace members", packages.len())
}

/// Checks this repository's `plugins/` against the plugin rules.
fn run_plugins() -> Result<ExitCode, String> {
    let plugins = plugins::read_plugins(&repository().join("plugins"))
        .map_err(|error| error_chain(&error))?;
    let violations = plugins::check(&plugins);
    conclude("plugins", &violations, "plugins", plugins.len())
}

/// Reports `violations` on stderr, or an ok line on stdout naming how many `things` were checked.
fn conclude(
    task: &str,
    violations: &[impl Display],
    things: &str,
    checked: usize,
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
