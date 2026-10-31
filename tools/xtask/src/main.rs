//! `cargo xtask <task>`: repository tasks for the redkiln workspace.
//!
//! - `layers`: check the workspace against the layer rules in `AGENTS.md`. Exits 0 when it
//!   follows them, 1 with one line per violation when it does not, and 2 when it cannot check.

use std::env;
use std::ffi::OsString;
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use xtask::layers;

const USAGE: &str = "usage: cargo xtask layers";

fn main() -> ExitCode {
    let task: Vec<OsString> = env::args_os().skip(1).collect();
    let outcome = match task.as_slice() {
        [name] if name == "layers" => run_layers(),
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

/// Checks this workspace, whose root manifest is two levels above this crate.
fn run_layers() -> Result<ExitCode, String> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
    let cargo = env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let packages =
        layers::read_workspace(&cargo, &manifest).map_err(|error| error_chain(&error))?;
    let violations = layers::check(&packages);
    let mut out = io::stderr().lock();
    for violation in &violations {
        writeln!(out, "layers: {violation}").map_err(|error| error.to_string())?;
    }
    if violations.is_empty() {
        writeln!(
            io::stdout().lock(),
            "layers: ok, workspace members checked: {}",
            packages.len()
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
