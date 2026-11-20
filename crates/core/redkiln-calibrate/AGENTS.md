# Crate charter: `redkiln-calibrate`

## Purpose
Core's calibration crate (blueprint, Core: `redkiln-calibrate`): how well a producer's probabilities match the outcomes later labelled, whether a decision point may leave shadow mode, and where each producer's per-user decision rows live. Moved from Bench in Move 2 step 5, PR 5.4: the scores, the v1 row reader and the rotating decision log with their history, the promotion bar out of Bench's report, and the user scope new. Step 6, PR 6.4, added the calibration records of SPEC CA-02 and their reader; nothing writes them at Move 2 (CAL-D1 A). Bench is the only user today.

## Layer
Core leaf. Depends on no other Core crate (it may depend on `redkiln-core`, and on no other Core crate; it uses none today). I/O only on the log's and the rows' files; the environment only through the lookup a binary hands `UserScope::from_lookup` (ARC-11). No harness protocol.

## Allowed dependencies
- `serde`, `serde_json`, `thiserror`. Dev: `tempfile`, `proptest` (`std` only; CAL-D1..D4 A). Any other is H-16.
- Never a kit, a plugin crate, a contract crate or `anyhow`.

## Public surface
Only what a producer calls is `pub` (API-14). Version 1.0.0 since the freeze (M2-G, 2026-11-20), with CA-02's record types frozen as OW-17 and the user scope as OW-16.
- `scope`: `UserScope` (`from_lookup`, `root`, `legacy_home`, `decisions`), `user_root` (the root alone, never reading anneal's home; wi-ad764d8531 A), `DecisionDirs { current, legacy }`, `ScopeError { NoHome, RelativeHome, Producer }`, and `HOME_VAR`; the folder names are crate-private.
- `log`: `DecisionLog` (`new`, `dir`, `path`, `append`, `files`), `LogError`, `LOG_FILE`, `MAX_ROW_BYTES`, `MAX_FILE_BYTES`, `ROTATED_FILES`.
- `record`: `JudgementRecorded`, `JudgementLabelled`, `Answer`, `Mode`, `LabelBy`, `LabelActor`, the words `JudgementId`, `Outcome`, `Vocab` (`TryFrom<String>`, `FromStr`, `as_str`, `into_string`) with `WordError`, which hands the text back, `Producer` (ST-17's plain name, through `scope`'s rule) and `Subject` (ID-09's `<method>:<hex>`) built the same way with `ProducerError` and `SubjectError`, which hand the text back, `RecordError` (`field`), and `JUDGEMENT_RECORDED`, `JUDGEMENT_LABELLED`, `VERSION`. Read and written through private wire shapes (OUT-05); no `Payload` impl and no `core` feature while nothing writes an envelope.
- `rows`: `read::<P, Q>` over one or more logs, `Answered<P, Q>`, `Label`, `LogRows<P, Q>`, `ReadError`, `MAX_LOG_FILE_BYTES`, `VIA_MOD`; `Answered::probability` is a `Probability`. The point and question are the caller's types.
- `score`: `Probability` (`TryFrom<f64>`, `get`; `ProbabilityError`, opaque), `Sample`, `Bin`, `Costs`, `FalseAccept`, `Proposal`, `FALSE_ACCEPT_COST`, `NEEDLESS_COST`, `brier`, `bins`, `expected_cost`, `propose`, `count`.
- `promotion`: `Bar` (`DEFAULT`: 50 labelled, Brier 0.15, ask rate 0.20; `verdict`), `Verdict`, `Reason`.

## Invariants
- User scope (Ryan's D1, option A): rows live in `<root>/calibrate/<producer>/`, the root `$REDKILN_HOME`, else `$HOME/.redkiln`, else `%USERPROFILE%\.redkiln`; anneal's `<anneal home>/decisions/` (`$ANNEAL_HOME`, else `~/.anneal`) is named for reading alongside and is never written or deleted by anything here. Empty values are unset; relative ones are refused. A producer name is one plain lowercase folder name.
- The log is append-only and bounded (H-13): a row of at most `MAX_ROW_BYTES`, rotation at `MAX_FILE_BYTES`, `ROTATED_FILES` old files; the reader refuses a file over `MAX_LOG_FILE_BYTES` and a path that is not a regular file, and counts an unreadable line as skipped, never dropping it silently.
- `rows` reads v1 rows and CA-02 records alike (CA-04): a v1 row is upcast onto the record types (producer `bench`, every key that has no typed field kept in `detail`), and a record line is an EV-01 envelope of at most the row cap. A record of another producer, an unknown type or a version other than `VERSION` is skipped and counted. The types are SPEC CA-02 (`spec/calibration.md`); nothing here invents another.
- A verdict is only reported: promotion stays a human edit of the producer's configuration.
- Error enums are closed (workspace-internal, so not `#[non_exhaustive]`).

## Test strategy
Unit tests beside the code: `score` and `log` moved from Bench with their assertions unchanged (`log`'s `counts.json` tests stayed in Bench with the counts); new tests for `Probability`, `promotion`'s boundaries, `rows` over an old and a new log, `record` over the SPEC's goldens (valid ones written back byte for byte, `invalid-data` ones refused naming their key), `rows` over the mixed golden and every Bench v1 fixture row, a proptest round trip (fixed seed, no regression files) from v1 row through record and back, and `scope` over injected lookups (no real `$HOME`, TST-10). Bench's `calibrate::read` and `report` tests exercise `rows` and `promotion` with Clef's types. `cargo nextest run -p redkiln-calibrate`.

## Decisions
In `docs/decisions.md` (2026-11-15 row, PR 5.4; 2026-11-17 row, PR 6.4).
