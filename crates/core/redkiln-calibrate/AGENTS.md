# Crate charter: `redkiln-calibrate`

## Purpose
Core's calibration crate (blueprint, Core: `redkiln-calibrate`): how well a producer's probabilities match the outcomes later labelled, whether a decision point may leave shadow mode, and where each producer's per-user decision rows live. Moved from Bench in Move 2 step 5, PR 5.4: the scores, the v1 row reader and the rotating decision log with their history, the promotion bar out of Bench's report, and the user scope new. Bench is the only user today.

## Layer
Core leaf. Depends on no other Core crate (it may depend on `redkiln-core`, and on no other Core crate; it uses none today). I/O only on the log's and the rows' files; the environment only through the lookup a binary hands `UserScope::from_lookup` (ARC-11). No harness protocol.

## Allowed dependencies
- `serde`, `serde_json`, `thiserror`. Dev: `tempfile`. Any other is H-16.
- Never a kit, a plugin crate, a contract crate or `anyhow`.

## Public surface
Only what a producer calls is `pub` (API-14).
- `scope`: `UserScope` (`from_lookup`, `root`, `decisions`), `DecisionDirs { current, legacy }`, `ScopeError { NoHome, RelativeHome, Producer }`, and `HOME_VAR`; the folder names are crate-private.
- `log`: `DecisionLog` (`new`, `dir`, `path`, `append`, `files`), `LogError`, `LOG_FILE`, `MAX_ROW_BYTES`, `MAX_FILE_BYTES`, `ROTATED_FILES`.
- `rows`: `read::<P, Q>` over one or more logs, `Answered<P, Q>`, `Label`, `LogRows<P, Q>`, `ReadError`, `MAX_LOG_FILE_BYTES`, `VIA_MOD`; `Answered::probability` is a `Probability`. The point and question are the caller's types.
- `score`: `Probability` (`TryFrom<f64>`, `get`; `ProbabilityError`, opaque), `Sample`, `Bin`, `Costs`, `FalseAccept`, `Proposal`, `FALSE_ACCEPT_COST`, `NEEDLESS_COST`, `brier`, `bins`, `expected_cost`, `propose`, `count`.
- `promotion`: `Bar` (`DEFAULT`: 50 labelled, Brier 0.15, ask rate 0.20; `verdict`), `Verdict`, `Reason`.

## Invariants
- User scope (Ryan's D1, option A): rows live in `<root>/calibrate/<producer>/`, the root `$REDKILN_HOME`, else `$HOME/.redkiln`, else `%USERPROFILE%\.redkiln`; anneal's `<anneal home>/decisions/` (`$ANNEAL_HOME`, else `~/.anneal`) is named for reading alongside and is never written or deleted by anything here. Empty values are unset; relative ones are refused. A producer name is one plain lowercase folder name.
- The log is append-only and bounded (H-13): a row of at most `MAX_ROW_BYTES`, rotation at `MAX_FILE_BYTES`, `ROTATED_FILES` old files; the reader refuses a file over `MAX_LOG_FILE_BYTES` and a path that is not a regular file, and counts an unreadable line as skipped, never dropping it silently.
- `rows` reads anneal's v1 JSONL only. It is not Core's calibration record format, which step 6 defines; nothing here invents an event type.
- A verdict is only reported: promotion stays a human edit of the producer's configuration.
- Error enums are closed (workspace-internal, so not `#[non_exhaustive]`).

## Test strategy
Unit tests beside the code: `score` and `log` moved from Bench with their assertions unchanged (`log`'s `counts.json` tests stayed in Bench with the counts); new tests for `Probability`, `promotion`'s boundaries, `rows` over an old and a new log, and `scope` over injected lookups (no real `$HOME`, TST-10). Bench's `calibrate::read` and `report` tests exercise `rows` and `promotion` with Clef's types. `cargo nextest run -p redkiln-calibrate`.

## Decisions
In `docs/decisions.md` (2026-11-15 row, PR 5.4).
