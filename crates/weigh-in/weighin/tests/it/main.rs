//! Integration tests, compiled as one binary: the end-to-end CLI scenarios, the eval seed
//! scorer, the committed journals, the journal refs and the hooks that export to them, `ci-check`, and
//! the store being redkiln-core's beside a 1.x ledger (`core_store`), the repository class read
//! from suite.toml (`repo_class`), over shared helpers in `support` and `scratch`.

mod ageing;
mod annotate;
mod board_guard;
mod ci_check;
mod cli;
mod core_store;
mod eval_seed;
mod journal;
mod journal_hooks;
mod journal_refs;
mod owner;
mod pane;
mod recall;
mod repo_class;
#[cfg(test)]
mod scratch;
#[cfg(test)]
mod support;
