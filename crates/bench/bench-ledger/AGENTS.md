# Crate charter: `bench-ledger`

## Purpose
Bench's event ledger (anneal's `anneal-ledger`, renamed at the Move 2 door): the versioned registry of event types, their envelope, and happenstance's SQLite event store (`=0.6.0`, the workspace's pin; option A of the 2026-11-10 card, decided by Ryan) under a TRUNCATE journal, still its own store. It dissolves into `redkiln-core` plus `bench.*` types in step 3 of `docs/plan/move-2-bench.md`.

The full charter as imported (public surface, invariants, errors, decisions) is [`docs/bench/crates/bench-ledger.md`](../../../docs/bench/crates/bench-ledger.md), moved there whole so this chain fits the 24 KiB charter budget. Its rules still hold.

## Layer
Plugin (bench): an adapter with its own small domain (`tag`, `envelope`; `event` re-exports the contract's); only `store` touches the file system.

## Allowed dependencies
- Workspace crates: `bench-contract` (pure, no `core` feature), whose `event` types `event` re-exports at their old paths (Move 2 step 3, PR 3.2).
- Third-party: `happenstance-core` and `happenstance-sqlite` through the root's `=0.6.0` entries (a ledger anneal wrote on 0.3.2 is migrated in place on open, as Core does), `tokio` (`rt`), `time`, `tempfile`, `serde`, `serde_json`, `thiserror`.
- Never: `clap`, `anyhow`, git or environment access.
