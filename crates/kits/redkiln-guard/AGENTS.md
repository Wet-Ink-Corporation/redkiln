# Crate charter: `redkiln-guard`

## Purpose
The guard kit (blueprint, Kits): what a plugin's guard needs to read a Bash command before it runs. Moved from Bench in Move 2 step 5 with its history: the shell lexer and path globs (PR 5.2a), then the denylist engine, protected-path matching and redaction (PR 5.2c, over 5.2b's rule-pack seam). What may not run is each plugin's rule pack; the kit ships no rule. Bench is the only user today.

## Layer
Kit. No I/O, no process, no clock, no environment.

## Allowed dependencies
- Core crates only (none used yet), `thiserror`. Dev: none (the tests use only std). Any other is H-16.
- Never a plugin crate, a contract crate or `anyhow`.

## Public surface
Only what a guard calls is `pub` (API-14); the rest is `pub(crate)` or narrower.
- `shell`: `parse`, `parse_without_extglob`, `SimpleCommand` (accessors only; built by the lexer), `Word` (fields crate-private; read through `text`, `shape`, `quoted`, `expansions`, `has_pattern`, `outside_expansions`, `is_literal`, `is_process_substitution`), `Expansion`, `Shape`, `Globs`, `Stdin`, `ParseError`, `EXTGLOB_OPENERS`, `MAX_NESTING`.
- `engine`: `check<P: Pack>(command, &mut P) -> Option<Denial<P::Rule>>`, the seam (`Pack`, `Call`, `Denial`, `Program`, `Cwd`), `MAX_DEPTH`, `MAX_LEXED_MIB`; option helpers for packs: `wrappers::{Long, is_long, is_option, may_be_option, skip_one, skip_options, takes_value, defines}`, `braces::{reads_as_written, expand, TooMany}`, `readings::may_start_option_cluster`, `git_settings::{settings, Settings, GLOBAL_VALUES}`.
- `protect`: `Protected` (`file`, `exact`, `affix`), `ProtectedPaths` (`new`, `protected`, `contains_protected`), `last_component`. The table is the pack's.
- `redact`: `redact`, `cut_chars`, `REDACTED`, `UNPARSED`, `MAX_COMMAND_CHARS`.
- `glob`: `Glob` (`new`, `as_str`, `matches`), `GlobError`.

## Invariants
- No harness protocol: no `CLAUDE_*` name and no hook payload or response key, tests included (`cargo xtask plugins`, check (b); a kit has no adapter exemption).
- The engine never allows: `check` returns a pack's denial, its own (under `Pack::UNREADABLE`), or `None` (undecided). What it cannot read the way bash would, it denies; it never runs or expands anything.
- Every input is bounded (H-13): lexer nesting at `MAX_NESTING` (32), engine nesting at `MAX_DEPTH` (8), `MAX_LEXED_MIB` (4) lexed per check, brace expansion at 256 words, a glob at 256 bytes.
- Redaction reads commands through the engine's own walk (`wrappers::layout`), so it never reads bash differently from the denylist; no argument text survives unless a fixed table names it.
- Error enums are closed (workspace-internal, so not `#[non_exhaustive]`).

## Test strategy
Unit tests beside the code (`src/shell/tests.rs`, `src/glob/tests.rs`, `src/redact/tests*`, `src/engine/tests/wrapper_probes.rs` against `tests/fixtures/wrappers/probes.txt`), moved from Bench with their assertions unchanged. `tests/it/` drives small packs of its own through the public API only (`seam.rs`, `pack.rs`: wrappers, `bash -c`, `eval`, variables, here-strings, redirects, the depth and lexing budgets). Bench's denylist, merge-gate and Clef tests cover the engine with its real packs end to end. `cargo nextest run -p redkiln-guard`.

## Decisions
In `docs/decisions.md` (2026-11-15 rows, PRs 5.2a, 5.2b and 5.2c).
