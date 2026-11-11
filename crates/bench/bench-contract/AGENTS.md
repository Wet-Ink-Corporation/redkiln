# Crate charter: `bench-contract`

## Purpose
What other crates may name of Bench (M2-C, decided 2026-11-09): `event`, the 17 `bench.*` event types (EV-04: the namespace owner declares them) with their payloads, which Loop's planning gate and the suite mod read; `cli`, the `bench` binary's name, every subcommand path it accepts and its global value options, which `cargo xtask plugins` reads to check that every `bench …` a skill names exists; `plugin`, the `rk-bench` plugin name; and `status`, the `data` of `bench status --json` (`bench.status/1`, SPEC CLI-12), which the cockpit mod and the suite's status view read.

## Layer
Contract (bench). Depends only on Core, and on Core only behind the `core` feature; never on Bench's own internals (`bench`). No file, clock, process or environment access.

## Allowed dependencies
- Workspace crates: `redkiln-core`, only behind the `core` feature (`Payload`, `Tag`, `Stored`, `decode`). Without it the crate is pure, so `tools/xtask` links it without Core or SQLite.
- Third-party: `serde`, `thiserror`. Dev: `serde_json`.

## Public surface
`event::{EventKind, SubjectKey, BenchEvent, UnknownEventKind, REGISTRY_VERSION}`, the payloads and their enums (`TestRun` … `OrchestrateRuled`, `By`, `Actor`, `Via`, `EmptyBy`, `RoundOfNothing`); with `core`, the `Payload` impls, `event::{TYPES, decode, MigrateWaveStarted, MigrateWaveDone}` and `BenchEvent::subject_tag`. `BenchEvent` and `EventKind` at the root. `cli::{BINARY, COMMANDS, VALUE_OPTIONS, OUTPUTS, OUTPUT_VERSION, output_id}`; `plugin::{NAME, skill_of}`, the plugin name `rk-bench` that Claude Code namespaces its skills and commands under (pinned against the manifest by `bench`'s `tests/it/plugin_name.rs`). `status::{SCHEMA, Status, StoreSource, RepoClass, Health}` and its section types (`Phase`, `Playbook`, `Plan`, `PlanStatus`, `Approved`, `Tasks`, `NextTask`, `Gate`, `HeadVerdict`, `Outcome`, `FreshVerdict`, `Fresh`, `Clef`, `PointStatus`, `DecisionPoint`, `ClefMode`, `Context`, `WindowSource`, `Level`, `TailEvent`). Workspace-internal; nothing is published.

## Invariants
- Every type is `bench.<short name>` (`EventKind::stored`), version 1, durable, with no upcaster; `TYPES` is `EventKind::ALL` in order. One `macro_rules!` table in `event/core.rs` generates the `Payload` impls, `TYPES` and `decode`'s dispatch (ARC-15).
- Changing a payload's bytes needs a new version (EV-06); `event/core/tests.rs` holds each type's version 1 golden.
- `decode` never fails a read: another namespace's event, or an undeclared `bench.*` type, is `Skip::Foreign` (EV-08); one of the 17 at a newer version or with undecodable data is `Skip::Unusable` (EV-10).
- Subject tags: `task` and `pr` are Core's keys, `plan`, `point`, `agent`, `migration` are `bench.<key>` (EV-12); a value Core refuses is an error, never shortened (EV-11).
- `BenchEvent` is the 0.x flat form (`type` beside the payload, short names): anneal's records, which the one-time import reads, and Bench's in-memory event; since step 4 `bench ledger` and the commands that record an event read and print Core's EV-01 envelope instead. `REGISTRY_VERSION` is frozen at 10.
- `COMMANDS` is sorted and unique, lowercase words separated by one space; a command with subcommands is listed itself only when it runs without one; clap's `help` is not listed. `VALUE_OPTIONS` is sorted long options. `OUTPUTS` is sorted, the CLI-01 schema id (`output_id`: `bench.<path words joined by ->/1`) of every command but `hook *`, `statusline` and `ledger journal`, which write no envelope. `bench`'s `cli_pin` test pins all four against clap's tree, so they change with the CLI or that test fails.
- `status::Status` is CLI-12's common keys, then Bench's sections, field order the wire order; every `Option` is written `null`, and a reader accepts keys it does not know (no `deny_unknown_fields`, CLI-04). A reader deserialises the envelope's `data` itself, so the crate takes no `serde_json`; `spec/fixtures/status/valid/bench-ok.json` is the golden, read and written back byte for byte by `status::tests`.

## Test strategy
Unit tests beside the code (`event::tests`, `event::core::tests`, `cli::tests`, `status::tests`); the pin lives in `crates/bench/bench/tests/it/cli_pin.rs`. Run `cargo test -p bench-contract --all-features`, and without features for the pure half.
