# Crate charter: `redkiln-core`

## Purpose
The code half of Redkiln Core: the one event store every plugin's binary opens (SPEC `spec/store.md`), and, as Move 1 step 2 continues (#17), the envelope, ids, location, config and journal codec the SPEC defines. Today it holds the store (ST-04 to ST-11), the envelope and Core's tag registry (EV-01 to EV-16), trust objects (TR-01), ids and digests (ID-01 to ID-08), and a read-only reader for old weighin ledgers.

## Layer
Core, the hub (`cargo xtask layers`): it depends on no other workspace crate, and other Core crates may depend only on it.

## Allowed dependencies
- Workspace crates: none.
- Third-party: `happenstance-core` and `happenstance-sqlite` at `=0.4.5` with `sync` (ST-04, decided 2026-11-01); `tokio` with `rt` only, to drive happenstance's async API on a private current-thread runtime; `tempfile` (the schema template and the old-ledger copy live on the local temporary disk); `thiserror`; `serde` and `serde_json` (the envelope and the canonical JSON digest), `sha2` (content ids, worktree ids, digests) and `time` (the envelope's `ts`), pre-approved, added 2026-11-02. `toml` arrives with `suite.toml` (approved 2026-11-02).
- Never: `anyhow` (a library, H-09), a plugin's crates, or anything that prints.

## Public surface
- `store`: `Store`, `Wait`, `NewEvent`, `Tag`, `TagError`, `CoreKey`, `Filter`, `Condition`, `Appended`, `Stored`, `TelemetryEvent`, `Recorded`, `OpenError`, `Step`, `StoreError`, `STORE_SCHEMA`.
- `envelope`: `Envelope`, `Author`, `Payload`, `Class`, `Namespace`, `TypeName`, `NameError`, `Timestamp`, `Actor`, `ActorName`, `Writer`, `Field`, `FieldError`, `EnvelopeError`, `decode`, `decode_all`, `Decoded`, `Skip`, `Unusable`, `SPEC_VERSION`, `MAX_ENVELOPE_BYTES`, `MAX_TYPE_BYTES`.
- `trust`: `Trust`, `Level`, `Via`, `TrustError`.
- `ids`: `parse_store_id`, `parse_event_ref`, `ContentId`, `IdPrefix`, `AtomKind`, `HexLen`, `content_input`, `SessionId`, `WorktreeId`, `Host`, `Digest`, `IdError`.
- `canonical`: `canonical_json`, `digest`, `digest_of`, `CanonicalError`, `MAX_DEPTH`.
- `legacy`: `read_ledger`, `LegacyLedger`, `LegacyError`, `ATTEMPTS`, `MAX_LEDGER_BYTES`.
- At the root: happenstance's `EventId` and `StoreId`, re-exported, since they are the SPEC's identities (ID-01, ID-02).

Error variants carry `rusqlite::Error`, `SqliteEventStoreError` and `AppendError<SqliteEventStoreError>` as their sources, deliberately: Core is the store's adapter and pins both crates. Workspace-internal, never published.

## Invariants
- The store runs under TRUNCATE, never WAL or DELETE. Nothing here deletes a file beside the store; files are set aside by renaming (ST-05).
- Core's openers hold an advisory lock on `<store>.lock` (created on the first open, never deleted) from re-reading the header to handing the connection over, so a `-journal` or `-wal` set aside is never another opener's live one. A refused file gets no lock file; a file system without whole-file locks opens unlocked.
- Only Core writes `PRAGMA user_version`. A store is created with its schema and the stamp `1000` in one `BEGIN IMMEDIATE`, so a Core store never reads 0 (ST-06).
- A file this crate refuses (newer major, not a store, an old plugin ledger) is left byte for byte as it was: its header is read before SQLite opens it (ST-07).
- An old plugin ledger (stamp below 1000) is never opened in place and never stamped: a 1.x binary beside 2.0 would refuse a stamped file. It is read from a copy on the temporary disk (ST-15, ST-16).
- The store records its canonical path and re-mints its identity when opened at another (ID-01), checking, re-minting and recording in one `BEGIN IMMEDIATE`, so concurrent first opens of a copy re-mint once. The re-mint is happenstance's own statement on its `store_meta` row, because `remint_identity_on` opens its own transaction.
- A file stamped 1000 to 1999 is taken to be a store, since only Core writes those stamps.
- Tags are never truncated (EV-11). An envelope's event carries `plugin` and `trust`, and `session` and `worktree` when it has them, set by Core; a caller's tag is a registered key or `<namespace>.<key>` (EV-12).
- Writing an envelope is strict and builds nothing on a broken rule (EV-05); reading is lenient: another type is skipped, a known type this build cannot use is reported and counted, never an error for the whole read (EV-08 to EV-10).
- A telemetry write never waits: it goes through a second connection with no busy wait, opened lazily by the same open path, and a busy answer drops and counts it, never retried (ST-09). Durable appends keep the open's busy timeout.
- The library reads no clock: a `Timestamp` comes from the caller's `SystemTime` or text.
- The canonical digest is RFC 8785 over a JSON value; an integer beyond +-2^53 is refused, never rounded (I-JSON, which RFC 8785 requires).

## Errors
`OpenError` (why a file could not be opened), `StoreError` (an operation on an open store), `LegacyError` (reading an old ledger), `EnvelopeError` (an envelope not built), `TagError`, `TrustError`, `IdError`, `NameError`, `FieldError` and `CanonicalError`. Refusals are their own variants, so a caller can name them. `Skip` and `Unusable` are a lenient read's outcomes, not failures.

## Test strategy
Unit tests beside the code for the header, the transaction helper, tags and their registry, trust, ids, names, envelope fields, writing and reading (the SPEC's EV-01 example byte for byte), and RFC 8785's own vectors; integration tests in one binary, `tests/it/`, against real files: new, concurrent, WAL, refused and copied stores, conditioned appends, envelopes stored and read back with foreign and unusable events among them, telemetry dropped under a held write lock, a read lock at commit and a held open lock, and the old-ledger fixtures in `tests/fixtures/` (written by weighin 0.2.2 and 1.2.0 binaries built from weigh-in's tags; the 0.1.x one is the 0.2.2 ledger under WAL, as happenstance's own open left 0.1.x ledgers, since 0.1.x predates weigh-in's history).

## Do not
- Open the store through `SqliteEventStore::open` (it switches the file to WAL).
- Delete, truncate or stamp a file this crate did not create as a Core store.
- Retry a dropped telemetry write, or give one a busy wait.
