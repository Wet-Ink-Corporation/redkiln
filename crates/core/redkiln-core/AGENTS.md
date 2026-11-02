# Crate charter: `redkiln-core`

## Purpose
The code half of Redkiln Core: the one event store every plugin's binary opens (SPEC `spec/store.md`), and, as Move 1 step 2 continues (#17), the envelope, ids, location, config and journal codec the SPEC defines. Today it holds the store (ST-04 to ST-11) and a read-only reader for old weighin ledgers.

## Layer
Core, the hub (`cargo xtask layers`): it depends on no other workspace crate, and other Core crates may depend only on it.

## Allowed dependencies
- Workspace crates: none.
- Third-party: `happenstance-core` and `happenstance-sqlite` at `=0.4.5` with `sync` (ST-04, decided 2026-11-01); `tokio` with `rt` only, to drive happenstance's async API on a private current-thread runtime; `tempfile` (the schema template and the old-ledger copy live on the local temporary disk); `thiserror`. `toml` arrives with `suite.toml` (approved 2026-11-02).
- Never: `anyhow` (a library, H-09), a plugin's crates, or anything that prints.

## Public surface
- `store`: `Store`, `Wait`, `NewEvent`, `Tag`, `TagError`, `Filter`, `Condition`, `Appended`, `Stored`, `OpenError`, `Step`, `StoreError`, `STORE_SCHEMA`.
- `legacy`: `read_ledger`, `LegacyLedger`, `LegacyError`, `ATTEMPTS`, `MAX_LEDGER_BYTES`.
- At the root: happenstance's `EventId` and `StoreId`, re-exported, since they are the SPEC's identities (ID-01, ID-02).

Error variants carry `rusqlite::Error`, `SqliteEventStoreError` and `AppendError<SqliteEventStoreError>` as their sources, deliberately: Core is the store's adapter and pins both crates. Workspace-internal, never published.

## Invariants
- The store runs under TRUNCATE, never WAL or DELETE. Nothing here deletes a file beside the store; files are set aside by renaming (ST-05).
- Only Core writes `PRAGMA user_version`. A store is created with its schema and the stamp `1000` in one `BEGIN IMMEDIATE`, so a Core store never reads 0 (ST-06).
- A file this crate refuses (newer major, not a store, an old plugin ledger) is left byte for byte as it was: its header is read before SQLite opens it (ST-07).
- An old plugin ledger (stamp below 1000) is never opened in place and never stamped: a 1.x binary beside 2.0 would refuse a stamped file. It is read from a copy on the temporary disk (ST-15, ST-16).
- The store records its canonical path and re-mints its identity when opened at another (ID-01), checking, re-minting and recording in one `BEGIN IMMEDIATE`, so concurrent first opens of a copy re-mint once. The re-mint is happenstance's own statement on its `store_meta` row, because `remint_identity_on` opens its own transaction.
- A file stamped 1000 to 1999 is taken to be a store, since only Core writes those stamps.
- Tags are never truncated (EV-11).

## Errors
`OpenError` (why a file could not be opened), `StoreError` (an operation on an open store) and `LegacyError` (reading an old ledger). Refusals are their own variants, so a caller can name them.

## Test strategy
Unit tests beside the code for the header, the transaction helper and tags; integration tests in one binary, `tests/it/`, against real files: new, concurrent, WAL, refused and copied stores, conditioned appends, and the old-ledger fixtures in `tests/fixtures/` (written by weighin 0.2.2 and 1.2.0 binaries built from weigh-in's tags; the 0.1.x one is the 0.2.2 ledger under WAL, as happenstance's own open left 0.1.x ledgers, since 0.1.x predates weigh-in's history).

## Do not
- Open the store through `SqliteEventStore::open` (it switches the file to WAL).
- Delete, truncate or stamp a file this crate did not create as a Core store.
