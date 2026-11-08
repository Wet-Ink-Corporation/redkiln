//! R-SQLITE: eight concurrent writers on one store under Core's TRUNCATE journal (SPEC ST-05,
//! ST-09, ST-10), hooks included, run as separate processes as a real session runs them.
//!
//! `r-sqlite run --dir <empty dir>` drives every scenario and prints a Markdown report; it exits
//! 1 when a correctness check fails (a lost or duplicated event, a deleted journal, a WAL or
//! shared-memory file, a failed integrity check). Latency is reported, never judged.
//!
//! The other subcommands are the writers `run` starts: `hook-once`, `command` and `racer`.

mod args;
mod events;
mod report;
mod run;
mod verify;
mod worker;

use std::io::Write as _;
use std::process::ExitCode;

fn main() -> ExitCode {
    match dispatch() {
        Ok(code) => code,
        Err(error) => {
            // The last resort: stderr is the only channel left, and a failed write there has no
            // other place to go.
            let mut err = std::io::stderr().lock();
            if writeln!(err, "r-sqlite: {error:#}").is_err() {
                return ExitCode::from(3);
            }
            ExitCode::from(2)
        }
    }
}

fn dispatch() -> anyhow::Result<ExitCode> {
    let args = args::Args::from_env()?;
    match args.command.as_str() {
        "run" => run::run(&args),
        "hook-once" => worker::hook_once(&args).map(|()| ExitCode::SUCCESS),
        "command" => worker::command(&args).map(|()| ExitCode::SUCCESS),
        "racer" => worker::racer(&args).map(|()| ExitCode::SUCCESS),
        other => anyhow::bail!("unknown subcommand {other:?}: run, hook-once, command or racer"),
    }
}
