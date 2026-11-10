//! `anneal`: the deterministic layer of the anneal plugin. Everything but process plumbing
//! lives in the `bench` library.

use std::io::{self, BufWriter};
use std::process::ExitCode;

fn main() -> ExitCode {
    let stdout = io::stdout();
    let stderr = io::stderr();
    bench::cli::run(
        std::env::args_os(),
        BufWriter::new(stdout.lock()),
        stderr.lock(),
    )
    .into()
}
