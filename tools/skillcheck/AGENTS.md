# Crate charter: `skillcheck`

## Purpose
Skill QA for Redkiln plugins, run as `cargo run -p skillcheck -- <command>`: `lint skills` (Tier 0, the 21 skill design rules, `docs/skill-rules.md`), `eval routing` (Tier 1 lexical routing; Tier 2 with `--router claude`) and `eval uplift` (Tier 3, behaviour with and without each skill). Moved out of Bench at Move 2 step 5 (PR 5.1b) with the same arguments, exit codes and `data`. A dev tool, never shipped in a plugin. `docs/write-skill/` is the guide for writing a skill test-first through the tiers.

## Layer
Tool (`tools/`). A library (`src/lib.rs`, for the tests) plus a thin binary (`src/main.rs`, which passes the process's arguments and locked streams to `cli::run`).

## Allowed dependencies
- Workspace crates: contract crates only, read through a plugin's `Profile` (`src/profile.rs`). Today `bench-contract`, without features (its `cli` and `plugin` lists). Never a plugin's internal crates.
- Third-party: `clap`, `serde`, `serde_json`, `sha2` (the lint snapshot's content hash), `thiserror`. Dev: `jsonschema` (envelopes against `spec/schemas/cli-envelope-1.schema.json`), `tempfile`.

## Public surface
Workspace-internal; nothing is published (API-14). `cli::run`; `lint::{SkillsArgs, Outcome, LintError, lint}`, `lint::finding`, `lint::checker`; `profile::{Profile, UnknownPlugin, profile, DEFAULT_PLUGIN}`; `eval::{EvalArgs, routing, uplift}`; `envelope::{BINARY, OUTPUTS, Envelope, Finding}`, `exit_codes`, `error::CliError`, and the helpers copied from Bench (`paths`, `baseline`, `json_file`).

## Invariants
- Every envelope is one JSON line whose `schema` is in `envelope::OUTPUTS` (`skillcheck.lint-skills/1`, `skillcheck.eval-routing/1`, `skillcheck.eval-uplift/1`); `command` stays `lint skills`, `eval routing`, `eval uplift`. Failures are one `skillcheck: error: <cause chain>` line and a documented non-zero code.
- The per-skill checks read only the skill, its surroundings, the hook ids and the plugin's `Profile`, and the snapshot hash covers exactly those and the checks' own source (`lint::cache`).
- `--plugin` names a plugin with a profile (`bench`, the default); any other is a usage error (2) before any path is read.

## Test strategy
Unit tests beside the code; integration tests in `tests/it/` run the built binary and go through `tests/it/support/conform.rs` (the SPEC schema, an id in `OUTPUTS`, one golden per id in `tests/fixtures/cli/`, `SKILLCHECK_BLESS=1` rewrites them). `tests/it/repo_gates.rs` holds the Tier 0, 1 and 3 `--check` bars over `plugins/bench/skills` and `evals/bench/`, and `bootstrap_index.rs` Bench's committed index. Run with `cargo nextest run -p skillcheck`.

## Errors
`CliError` maps each failure to `exit_codes`. Usage errors are 2: bad arguments, an unknown `--plugin` or `--case`, a missing or wrong-kind path, a `--changed-since` base that names no commit, a missing suites or skills directory, a `--skill` with no suites, an unreadable `--baseline` manifest, gated tools without `--allow-gated-tools`, fixtures without `--allow-scaffolds`, and no `bench` to place for `eval uplift` (none beside the executable, a `--bench-binary` naming no file, or `--bench-binary` with `--check`). Internal errors are 70: a file that cannot be read, a `--cache` snapshot that cannot be read or written, git failing under `--changed-since`, an unusable anneal home, the evaluation workspace and a `--baseline` that cannot be copied. A write failure on stdout or stderr is 74, and `eval` exits 4 when the real router cannot run its first batch. Findings or a result below its bound (1), a run stopped short (3) and invalid fixtures, suites or skills (5) are outcomes with an envelope, not errors.

## Do not
- Depend on a plugin's internal crates; what skillcheck knows of a plugin comes from its contract crate through a `Profile`.
- Ship it: no launcher copy, no build under `plugins/`, and no skill, hook or prompt names it.
- Change an envelope's `data` or a code's meaning without a decisions row; the goldens and `conform.rs` pin both.

## Decisions
Rows in `docs/decisions.md`: the move out of Bench and its test rewrites as a class (Move 2 step 5, D3), the `skillcheck.*` SPEC namespace, `--bench-binary` with the sibling lookup for `eval uplift`, and the `skillcheck` CI job (D4).
