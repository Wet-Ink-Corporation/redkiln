# Crate charter: `bench-contract`

## Purpose
What other crates may name of Bench (M2-C, decided 2026-11-09): today `cli`, the `bench` binary's name, every subcommand path it accepts and its global value options, which `cargo xtask plugins` reads to check that every `bench …` a skill names exists. Later the `bench.*` event names and payloads that Loop's planning gate and the suite mod read (step 3 of `docs/plan/move-2-bench.md`).

## Layer
Contract (bench). Depends only on Core, and today on nothing at all; never on Bench's own internals (`bench`, `bench-ledger`).

## Allowed dependencies
- Workspace crates: `redkiln-core` when the event types arrive; none today.
- Third-party: none today.

## Public surface
`cli::{BINARY, COMMANDS, VALUE_OPTIONS}`. Workspace-internal; nothing is published.

## Invariants
- `COMMANDS` is sorted and unique, lowercase words separated by one space; a command with subcommands is listed itself only when it runs without one; clap's `help` is not listed. `VALUE_OPTIONS` is sorted long options. `bench`'s `cli_pin` test pins all three against clap's tree, so they change with the CLI or that test fails.

## Test strategy
Unit tests beside the lists (`cli::tests`); the pin lives in `crates/bench/bench/tests/it/cli_pin.rs`.
