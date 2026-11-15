//! `skillcheck` as a library: the binary in `main.rs` only hands it the process's arguments and
//! streams. Skill QA for Redkiln plugins, moved out of Bench (Move 2 step 5, PR 5.1b); a dev
//! tool, never shipped.
//!
//! This library is internal to the binary and its tests (`publish = false`); its items are not
//! a stable API (API-14).
//!
//! - [`cli`]: the command tree and [`cli::run`], the entry point.
//! - [`lint`]: `lint skills`, the 21 skill design rules (Tier 0), with the finding model
//!   ([`lint::finding`]) and the checker catalogue ([`lint::checker`]).
//! - [`eval`]: `eval routing`, Tier 1 lexical routing and, with `--router claude`, Tier 2 under
//!   Claude Code's router with Wilson intervals; `eval uplift`, Tier 3, each entry skill's
//!   behaviour suites with the skill and without it, with Newcombe intervals.
//! - [`command`], [`envelope`], [`exit_codes`], [`error`]: the command layer: what a green
//!   result claims, the JSON envelope (`skillcheck.<command>/1`), the exit-code table and the
//!   errors that stop a command.
//! - [`paths`], [`baseline`], [`json_file`]: the anneal home and bounded git calls, the lint
//!   snapshot file, and capped JSON reads, copied from Bench.

pub mod baseline;
pub mod cli;
pub mod command;
pub mod envelope;
pub mod error;
mod error_chain;
pub mod eval;
pub mod exit_codes;
pub mod json_file;
pub mod lint;
mod path_text;
pub mod paths;
mod pipe;
mod process;
mod profile;
#[cfg(test)]
mod test_repo;
