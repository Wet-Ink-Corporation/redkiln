# Crate charter: `redkiln-guard`

## Purpose
The guard kit (blueprint, Kits): what a plugin's guard needs to read a Bash command before it runs. Part 1 (Move 2 step 5, PR 5.2a) holds Bench's shell lexer and its path globs, moved here with their history; the denylist engine and redaction follow in 5.2c, after 5.2b's rule-pack seam in Bench (`.temper/plans/move-2-step-5-cuts.md`). Bench is the only user today.

## Layer
Kit. No I/O, no process, no clock, no environment.

## Allowed dependencies
- Core crates only (none used yet), `thiserror`. Dev: none (the tests use only std). Any other is H-16.
- Never a plugin crate, a contract crate or `anyhow`.

## Public surface
Only what a guard calls is `pub` (API-14).
- `shell`: `parse`, `parse_without_extglob` (each `Result<Vec<SimpleCommand>, ParseError>`), `SimpleCommand` (fields private: `words`, `into_words`, `writes`, `stdin`, `defines_function`, `arithmetic`), `Word` (public fields, since Bench's denylist still builds words until 5.2c, and `has_pattern`, `outside_expansions`, `quotes_replacing`, `is_literal`, `is_process_substitution`, `all_quoted`), `Expansion`, `Shape`, `Globs`, `Stdin`, `ParseError`, `EXTGLOB_OPENERS`, `MAX_NESTING`.
- `glob`: `Glob` (`new`, `as_str`, `matches`), `GlobError`.

## Invariants
- No harness protocol: no `CLAUDE_*` name and no hook payload or response key, tests included (`cargo xtask plugins`, check (b); a kit has no adapter exemption).
- Every input is bounded (H-13): lexer nesting at `MAX_NESTING` (32), a glob at 256 bytes.
- The lexer never guesses: a text it cannot read the way bash would is a `ParseError`, which a guard denies; it never runs or expands anything.
- `ParseError` and `GlobError` are closed enums (workspace-internal, so not `#[non_exhaustive]`); Bench matches every variant.

## Test strategy
Unit tests beside the code (`src/shell/tests.rs`, `src/glob/tests.rs`), moved from Bench with their assertions unchanged. Bench's guard, merge-gate and Clef tests cover the lexer end to end. `cargo nextest run -p redkiln-guard`.

## Decisions
In `docs/decisions.md` (2026-11-15 row, PR 5.2a).
