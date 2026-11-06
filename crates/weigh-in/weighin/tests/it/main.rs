//! Integration tests, compiled as one binary: the end-to-end CLI scenarios, the eval seed
//! scorer, the committed journals and the session journals the binary writes, `ci-check`, and
//! the store being redkiln-core's beside a 1.x ledger (`core_store`), over shared helpers in
//! `support` and `scratch`.

mod ageing;
mod annotate;
mod board_guard;
mod ci_check;
mod cli;
mod core_store;
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
