//! Integration tests, compiled as one binary: the end-to-end CLI scenarios, the eval seed
//! scorer, the committed journals and the session journals the binary writes, and `ci-check`,
//! over shared helpers in `support` and `scratch`.

mod ageing;
mod annotate;
mod board_guard;
mod ci_check;
mod cli;
mod eval_seed;
mod journal;
mod journal_write;
mod owner;
mod pane;
mod recall;
#[cfg(test)]
mod scratch;
#[cfg(test)]
mod support;
