//! Integration tests, compiled as one binary: the end-to-end CLI scenarios, the eval seed
//! scorer, the committed journals, the journal refs and the hooks that export and import them,
//! `ci-check`, and the store being redkiln-core's beside a 1.x ledger (`core_store`), the
//! repository class read from suite.toml (`repo_class`), a failing hook's note and its state
//! files (`hook_notes`), the relay workflow (`relay_workflow`)
//! and the `--json` envelope against the SPEC's schema (`cli_conformance`), Knowledge's hashed
//! atom ids against Core's ID-04 (`atom_ids`), over shared helpers in `support` and `scratch`.

mod ageing;
mod annotate;
mod atom_ids;
mod board_guard;
mod ci_check;
mod cli;
mod cli_conformance;
mod core_store;
mod decision_request;
mod eval_seed;
mod heartbeat;
mod hook_notes;
mod journal;
mod journal_hooks;
mod journal_refs;
mod journal_session_start;
mod legacy_import;
mod owner;
mod pane;
mod recall;
mod relay_workflow;
mod repo_class;
#[cfg(test)]
mod scratch;
#[cfg(test)]
mod support;
