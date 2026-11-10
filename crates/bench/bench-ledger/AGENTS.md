# Crate charter: `bench-ledger`

## Purpose
Bench's event ledger (anneal's `anneal-ledger`, renamed at the Move 2 door): the versioned registry of event types, their envelope, and happenstance 0.3.2's SQLite event store under a TRUNCATE journal. It dissolves into `redkiln-core` plus `bench.*` types in step 3 of `docs/plan/move-2-bench.md`.

The full charter as imported (public surface, invariants, errors, decisions) is [`docs/bench/crates/bench-ledger.md`](../../../docs/bench/crates/bench-ledger.md), moved there whole so this chain fits the 24 KiB charter budget. Its rules still hold.

## Layer
Plugin (bench): an adapter with its own small domain (`event`, `tag`, `envelope`); only `store` touches the file system.

## Allowed dependencies
- Workspace crates: none.
- Third-party: `happenstance-core` and `happenstance-sqlite` 0.3.2, declared in this crate's own manifest and nowhere else (the root pins `=0.6.0` for Core; no other crate may take 0.3.2), `tokio` (`rt`), `time`, `tempfile`, `serde`, `serde_json`, `thiserror`.
- Never: `clap`, `anyhow`, git or environment access.
