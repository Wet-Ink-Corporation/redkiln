# `redkiln-core`: public surface

Moved from `crates/core/redkiln-core/AGENTS.md` on 2026-11-14 (Move 2 step 5.0), so the charter chains fit their 24 KiB budget (`tools/charters/check.sh`) as the step 5 crates join the root's crate map. The charter points here; keep this list current when the surface changes.

## Modules and items
- `store`: `Store`, `Wait`, `NewEvent`, `Tag`, `TagError`, `CoreKey`, `Filter`, `Page`, `Condition`, `Appended`, `Stored`, `TelemetryEvent`, `Recorded`, `OpenError`, `Step`, `StoreError`, `STORE_SCHEMA`, `StoreSchema`, `StampOutOfRange`, `TELEMETRY_WINDOW`.
- `location`: `locate`, `Search`, `Located`, `Source`, `Workspace`, `LocateError`, `GitError`, `STORE_ENV`, `STORE_FILE`, `GIT_TIMEOUT`.
- `suite`: `Suite`, `RepoClass`, `ClassSource`, `Owner`, `OwnerError`, `Key`, `Warning`, `Problem`, `SUITE_FILE`, `LEGACY_CONFIG`, `MAX_CONFIG_BYTES`.
- `envelope`: `Envelope`, `Author`, `Payload`, `Class`, `Namespace`, `TypeName`, `NameError`, `Timestamp`, `Actor`, `ActorName`, `Writer`, `Field`, `FieldError`, `EnvelopeError`, `decode`, `decode_all`, `Decoded`, `Skip`, `Unusable`, `SPEC_VERSION`, `MAX_ENVELOPE_BYTES`, `MAX_TYPE_BYTES`.
- `trust`: `Trust`, `Level`, `Via`, `TrustError`.
- `approval`: `ApprovalRecorded` (AP-01), `ApprovalSubject`, `Verdict`, `Scope`.
- `ids`: `parse_store_id`, `parse_event_ref`, `ContentId`, `IdPrefix`, `AtomKind`, `HexLen`, `content_input`, `SessionId`, `WorktreeId`, `Host`, `Digest`, `IdError`.
- `canonical`: `canonical_json`, `digest`, `digest_of`, `CanonicalError`, `MAX_DEPTH`.
- `subject`: `Subject`, `Method`, `SubjectError`, `sha256_file`, `git_commit`, `git_tree`, `patch_id`, `PatchId`, `NoPatchId`.
- `journal`: `checkpoint`, `Checkpoint`, `Exported`, `JournalError`, `import`, `import_file`, `import_mapped`, `Mapped`, `MappedImport`, `Recording`, `ImportSource`, `ImportCompleted`, `ImportReport`, `Problem`, `ImportError`, `Line`, `lines`, `encode_line`, `read_line`, `ReadLine`, `LineError`, `JournalName`, `NameError`, limits; `journal::refs`: `checkpoint_refs[_since]`, `sweep_refs_since`, `RefsScope`, `relay`, `import_refs`, `read_local`, outcomes, `RefsTarget`, `Breach`, `RefsError`, `RETRIES`, prefixes.
- `legacy`: `read_ledger`, `LegacyLedger`, `LegacyError`, `ATTEMPTS`, `MAX_LEDGER_BYTES`.
- At the root: happenstance's `EventId` and `StoreId`, re-exported, since they are the SPEC's identities (ID-01, ID-02).

Error variants carry rusqlite's and happenstance's errors as sources: Core is their adapter and pins both. Workspace-internal, never published.
