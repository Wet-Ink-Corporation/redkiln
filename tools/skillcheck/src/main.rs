//! `skillcheck`: skill QA for Redkiln plugins. Everything but process plumbing lives in the
//! `skillcheck` library.

use std::io::{self, BufWriter};
use std::process::ExitCode;

fn main() -> ExitCode {
    let stdout = io::stdout();
    let stderr = io::stderr();
    skillcheck::cli::run(
        std::env::args_os(),
        BufWriter::new(stdout.lock()),
        stderr.lock(),
    )
    .into()
}
