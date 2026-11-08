# Crate charter: `redkiln-core`

## Purpose
The code half of Redkiln Core: the one event store every plugin's binary opens (SPEC `spec/store.md`), and what the SPEC defines around it: the store's location (ST-01 to ST-03) and the store (ST-04 to ST-11), `suite.toml` (ST-12, ST-13), the envelope and Core's tag registry (EV-01 to EV-16), trust objects (TR-01), ids and digests (ID-01 to ID-08), subjects and their hashes (ID-09, ID-10), journal lines, the checkpoint into files and journal refs, the relay, and the import (JR-01 to JR-14), and a read-only reader for old weighin ledgers.

## Layer
Core, the hub (`cargo xtask layers`): other Core crates may depend only on it.

## Allowed dependencies
- Workspace crates: none.
- Third-party: `happenstance-core` and `happenstance-sqlite` at `=0.4.5` with `sync` (ST-04, 2026-11-01); `tokio` with `rt` only, to drive happenstance's async API on a private current-thread runtime; `tempfile` (the schema template and the old-ledger copy live on the local temporary disk); `thiserror`; `serde` and `serde_json` (envelope, canonical digest), `sha2` (ids, digests) and `time` (`ts`), pre-approved, added 2026-11-02. `toml` (parse only, with serde) reads `suite.toml` (approved).
- Dev only: `jsonschema` (no default features), for `spec/`'s schemas (2026-11-08).
- Never: `anyhow` (a library, H-09), a plugin's crates, or anything that prints.

## Public surface
- `store`: `Store`, `Wait`, `NewEvent`, `Tag`, `TagError`, `CoreKey`, `Filter`, `Page`, `Condition`, `Appended`, `Stored`, `TelemetryEvent`, `Recorded`, `OpenError`, `Step`, `StoreError`, `STORE_SCHEMA`, `TELEMETRY_WINDOW`.
- `location`: `locate`, `Search`, `Located`, `Source`, `Workspace`, `LocateError`, `GitError`, `STORE_ENV`, `STORE_FILE`, `GIT_TIMEOUT`.
- `suite`: `Suite`, `RepoClass`, `ClassSource`, `Owner`, `OwnerError`, `Key`, `Warning`, `Problem`, `SUITE_FILE`, `LEGACY_CONFIG`, `MAX_CONFIG_BYTES`.
- `envelope`: `Envelope`, `Author`, `Payload`, `Class`, `Namespace`, `TypeName`, `NameError`, `Timestamp`, `Actor`, `ActorName`, `Writer`, `Field`, `FieldError`, `EnvelopeError`, `decode`, `decode_all`, `Decoded`, `Skip`, `Unusable`, `SPEC_VERSION`, `MAX_ENVELOPE_BYTES`, `MAX_TYPE_BYTES`.
- `trust`: `Trust`, `Level`, `Via`, `TrustError`.
- `ids`: `parse_store_id`, `parse_event_ref`, `ContentId`, `IdPrefix`, `AtomKind`, `HexLen`, `content_input`, `SessionId`, `WorktreeId`, `Host`, `Digest`, `IdError`.
- `canonical`: `canonical_json`, `digest`, `digest_of`, `CanonicalError`, `MAX_DEPTH`.
- `subject`: `Subject`, `Method`, `SubjectError`, `sha256_file`, `git_commit`, `git_tree`, `patch_id`, `PatchId`, `NoPatchId`.
- `journal`: `checkpoint`, `Checkpoint`, `Exported`, `JournalError`, `import`, `import_file`, `import_mapped`, `Mapped`, `MappedImport`, `Recording`, `ImportSource`, `ImportCompleted`, `ImportReport`, `Problem`, `ImportError`, `Line`, `lines`, `encode_line`, `LineError`, `JournalName`, `NameError`, limits; `journal::refs`: `checkpoint_refs[_since]`, `sweep_refs_since`, `RefsScope`, `relay`, `import_refs`, outcomes, `RefsTarget`, `Breach`, `RefsError`, `RETRIES`, prefixes.
- `legacy`: `read_ledger`, `LegacyLedger`, `LegacyError`, `ATTEMPTS`, `MAX_LEDGER_BYTES`.
- At the root: happenstance's `EventId` and `StoreId`, re-exported, since they are the SPEC's identities (ID-01, ID-02).

Error variants carry `rusqlite::Error`, `SqliteEventStoreError` and `AppendError<SqliteEventStoreError>` as their sources, deliberately: Core is the store's adapter and pins both crates. Workspace-internal, never published.

## Invariants
- The store runs under TRUNCATE, never WAL or DELETE. Nothing here deletes a file beside the store; files are set aside by renaming (ST-05). No SQLite connection Core opens can delete or play back a stale `-journal`: it is probed through a read-only read lock and renamed aside before any read-write connection sees the file, and renamed back when a writer holds it.
- `Store::open_existing` never creates: a missing, empty or unstamped file is `None` with no file, directory or lock made; other refusals are `open`'s (D6).
- Core's openers hold an advisory lock on `<store>.lock` (created on the first open, never deleted) from re-reading the header to handing the connection over, so a `-journal` or `-wal` set aside is never another opener's live one. A refused file gets no lock file; a file system without whole-file locks opens unlocked.
- Only Core writes `PRAGMA user_version`. A store is created with its schema and the stamp `1000` in one `BEGIN IMMEDIATE`, so a Core store never reads 0 (ST-06).
- A file this crate refuses (newer major, not a store, an old plugin ledger) is left byte for byte as it was: its header is read before SQLite opens it (ST-07).
- An old plugin ledger (stamp below 1000) is never opened in place and never stamped: a 1.x binary beside 2.0 would refuse a stamped file. It is read from a copy on the temporary disk (ST-15, ST-16).
- The store records its canonical path and re-mints its identity when opened at another (ID-01), checking, re-minting and recording in one `BEGIN IMMEDIATE`, so concurrent first opens of a copy re-mint once. The re-mint is happenstance's own statement on its `store_meta` row, because `remint_identity_on` opens its own transaction.
- A file stamped 1000 to 1999 is taken to be a store, since only Core writes those stamps.
- A journal ref only moves forward: a write merges by `origin` onto its file, pushed without force, local copies moved by compare-and-swap; a ref breaking JR-05 is never written. Inbox branches change, and are deleted, only under a lease. A network failure or timeout leaves the events in the store, reported (JR-07).
- Tags are never truncated (EV-11). An envelope's event carries `plugin` and `trust`, and `session` and `worktree` when it has them, set by Core; a caller's tag is a registered key or `<namespace>.<key>` (EV-12). A journal line carries the caller's tags in `tags` (at most 64, refused past that on write), never Core's, and an import restores them (JR-02).
- Writing an envelope is strict and builds nothing on a broken rule (EV-05), a NaN or infinite float in the payload among them (`serde_json` would write `null`); reading is lenient: another type is skipped, a known type this build cannot use is reported and counted, never an error for the whole read (EV-08 to EV-10). An envelope that names a key twice, at any depth, is unusable, never read last-wins.
- A telemetry write never waits: it goes through a second connection with no busy wait, opened lazily by the same open path in its existing-store-only mode (a vanished store is `OpenError::NoStore`, never created again), and a busy answer drops and counts it, never retried (ST-09). A file whose identity is not the opened store's gets no telemetry (`StoreError::IdentityChanged`). Durable appends keep the open's busy timeout.
- Telemetry lives in a side store, `<store>.telemetry` (a Core store with its own `.lock`), never in the log. The window is the caller's `now` minus 30 days (`TELEMETRY_WINDOW`) to `now` plus a day; rows outside it are deleted there, with their tag rows and counts, once a day's slack has built up or a row is dated ahead, on a connection with no busy wait that never creates the file and checks the side store's identity first; `read_telemetry` never returns a row outside the window (EV-14).
- Location and config read no harness protocol or process state: the working directory, `$REDKILN_STORE`'s value and the home directory are arguments. git runs only when the flag and the variable did not choose, under `LC_ALL=C` without the `GIT_DIR` family, with a timeout and output capped; only git's discovery finding no repository is the folder step. Locating creates nothing.
- `suite.toml` is read up to 64 KiB; anything it cannot give is `client` (ST-13). The legacy `config.json` `repo_class` is a fallback only when `suite.toml` sets none (M1-F). Problems and fallbacks are returned `Warning`s, never printed.
- The library reads no clock: a `Timestamp` comes from the caller's `SystemTime` or text.
- patch-id is ID-09's pinned recipe only: git 2.39 or later, the pinned flags and `-c` values, the diff variables unset, and no id for a shallow boundary, other than one merge base, or an empty change. A failure is a `NoPatchId` naming why, never a fallback id.
- Only an import (`import`, `import_file`, `import_refs`, `import_mapped`) writes under a foreign identity (the crate-internal `Store::ingest`), at `asserted` with the line's level in `claimed`, reading its source only; an origin past position 2^63 - 1 is a malformed line.
- The canonical digest is RFC 8785 over a JSON value; an integer beyond +-2^53 is refused, never rounded (I-JSON, which RFC 8785 requires).

## Errors
`OpenError` (why a file could not be opened), `StoreError` (an operation on an open store), `LocateError` and `GitError` (where the store is), `LegacyError` (reading an old ledger), `RefsError` (store or local git), `EnvelopeError` (an envelope not built), `TagError`, `TrustError`, `IdError`, `NameError`, `FieldError` and `CanonicalError`. Refusals are their own variants, so a caller can name them. `Skip` and `Unusable` are a lenient read's outcomes, not failures.

## Test strategy
Unit tests beside the code: the header, the transaction helper, tags and their registry, trust, ids, names, envelope fields, writing and reading, RFC 8785's vectors, `suite.toml`, git's deadline and cap. `tests/it/` (integration, no git): new, concurrent, WAL, refused and copied stores, conditioned appends, envelopes stored and read back amid foreign and unusable events, telemetry dropped under a held write lock, a read lock at commit and a held open lock, and kept to its window; the SPEC's goldens and schemas (`conformance`); the old-ledger fixtures in `tests/fixtures/` (by weighin 0.2.2 and 1.2.0 built from weigh-in's tags; 0.1.x's is 0.2.2's under WAL, as 0.1.x left it), opened only as copies, listing and bytes pinned by `tests/it/fixtures.rs`. `tests/repos/`: journals, refs and location on real repos (subdirectories, a linked worktree, a broken `.git`, none). `tests/working_dir.rs`, the one test that moves the working directory, is its own binary; `repos` and `subject` start git, so stay apart: a child forked while another thread holds `<store>.lock` keeps it.

## Do not
- Open the store through `SqliteEventStore::open` (it switches the file to WAL).
- Delete, truncate or stamp a file this crate did not create as a Core store.
- Retry a dropped telemetry write, or give one a busy wait.
- Read an environment variable or the process's working directory, or run git without a deadline.
