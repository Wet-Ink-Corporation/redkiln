# Events

Every fact in the store is one event: a JSON envelope, stored as the data of one happenstance event whose type is the envelope's `type`, with `key:value` tags for selection. Plugins meet in the shared log through namespaced types, and a reader decodes only the types it knows (Blueprint, How they compose).

## The envelope

**EV-01 (one-way, OW-3).** An envelope is one JSON object with these keys. The payload is nested under `data`, never flattened beside the envelope's keys.

| Key | Type | Meaning |
| --- | --- | --- |
| `type` | string | `<namespace>.<name>` (EV-03) |
| `version` | integer, from 1 | The payload version of this type (EV-06) |
| `ts` | string | When the writer wrote it: RFC 3339 in UTC with a `Z`, to the second or the millisecond |
| `actor` | string | Who the writer says acted: `human:<name>`, `agent`, `hook`, `cli`, `ci` or `app:<name>`. Asserted text; trust says whether to believe it |
| `trust` | object | The trust level and its evidence ([trust.md](trust.md)) |
| `writer` | object | The binary that wrote it: `binary`, `version` and `spec` (the SPEC version it conforms to), for example `{"binary": "weighin", "version": "2.0.0", "spec": "1.0.0"}` |
| `session` | string or absent | The agent session the write belongs to |
| `worktree` | string or absent | The writer's worktree id ([ids.md](ids.md), ID-06) |
| `branch` | string or absent | The checked-out branch; absent outside git or on a detached head |
| `head` | string or absent | The commit at `HEAD`; absent outside git or before the first commit |
| `origin` | string or absent | The event's identity, present once the event has left its first store (ID-02) |
| `cites` | array of strings, or absent | Events this one answers or depends on, as event refs (ID-03) |
| `data` | object | The payload, defined by the type's owner |

```json
{"type":"weighin.item_decided","version":1,"ts":"2026-10-31T04:20:11Z","actor":"human:ryan","trust":{"level":"witnessed","via":"chat-card"},"writer":{"binary":"weighin","version":"2.0.0","spec":"1.0.0"},"session":"8b6fb94b-0975-5275-b434-6c9e60544265","worktree":"wt-5d41402abc4b","branch":"main","head":"bd234009c1e5a7d4f1c3b2a0e9f8d7c6b5a49382","cites":["0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f/41"],"data":{"item":"wi-3fa9c01b2e","option":"a","applies":"once"}}
```

Optional keys are left out rather than written as `null`; a reader accepts either.

Why nested: both ledgers flatten today, and they already disagree about what may sit beside the payload. Nesting lets the envelope gain a key without colliding with any payload field, and lets the canonical digest of a payload (ID-08) be taken without stripping envelope keys, as Loop ADR-0131 requires of tags and store-minted values. The import at each move maps flattened events into `data` once.

**EV-02.** happenstance's store-minted values (position, `recorded_at`, origin) are not part of the envelope a writer builds. A reader that needs them takes them from the stored event.

## Names

**EV-03 (one-way, OW-4).** A type is `<namespace>.<name>`. The namespace is one of the closed registry below, and adding one is a SPEC minor. The name matches `[a-z][a-z0-9_]*(\.[a-z][a-z0-9_]*)*`, so anneal's dotted names (`migrate.wave.started`) keep their shape. The whole type is at most 64 bytes.

| Namespace | Owner | Written by |
| --- | --- | --- |
| `core` | Redkiln Core | Core libraries, for any plugin (imports, approvals, capacity, calibration) |
| `weighin` | Weigh-In | Weigh-In, and any plugin through `weighin-contract` |
| `bench` | Bench | Bench |
| `rust` | Redkiln Rust | Redkiln Rust |
| `review` | Review | Review in local mode; CI journals |
| `loop` | Loop | Loop |
| `knowledge` | Knowledge | Knowledge |
| `backlog` | Backlog | Backlog, or Loop through `backlog-contract` |

**EV-04.** A namespace's owner defines its types in its contract crate, or in its own crates for types no sibling writes. Each type declares its versions, its class (EV-14) and its subject tag, if any. A sibling writes another namespace's type only through that namespace's contract crate (Blueprint design rule 4).

**EV-05.** Writing is strict. An unregistered type, a missing required field, a key the type's version does not define, a `ts` that is not RFC 3339, or an invalid tag is an error, and nothing is written (anneal `docs/ledger.md`).

## Versions

**EV-06 (one-way, OW-5).** Each event carries its type's payload `version`. Within a version, changes are additive only: a new optional field, read back from a missing key. A new required field, a rename, a removed field or a changed meaning is a new version. Every payload field name is a persisted key; renaming one is a migration.

**EV-07.** A contract crate keeps an upcaster from each older version to its newest, and a golden fixture for every version ever written (pressure test, Event schemas over time). A writer writes the newest version its contract knows.

## Lenient reads

**EV-08.** A reader decodes only the types it knows and skips every other type without error. This is a conformance test (the foreign-events-are-skipped fixture), and it is the change anneal most needs: it fails today on the first foreign event.

**EV-09.** A reader ignores keys it does not know, in the envelope and in `data`.

**EV-10.** An event of a known type that a reader cannot use (a newer `version` than its contract knows, or a payload that does not decode) is skipped and counted, never fatal. The count is reported in the plugin's `status --json` ([cli.md](cli.md), CLI-12). A gate that needed such an event treats it as absent, so a guard that waits on an approval it cannot read fails closed.

## Tags

**EV-11.** Tags are happenstance `key:value` tags. A value is non-empty, holds no control characters, and with its key and the colon is at most 255 bytes (happenstance `MAX_TAG_LEN`). A writer never truncates a value to fit: one that does not fit is an error (EV-05), and long content goes in `data` with its digest as the tag. weigh-in truncates today, which can make two items share a tag; the SPEC does not keep that.

**EV-12.** Unprefixed tag keys are Core's registry, and each means one thing for every writer:

| Key | Value | Carried by |
| --- | --- | --- |
| `plugin` | The type's namespace | Every event |
| `trust` | `asserted`, `witnessed` or `attested` | Every event |
| `session` | The envelope's `session` | Every event that has one |
| `worktree` | The envelope's `worktree` | Every event that has one |
| `origin` | The event's identity (ID-02) | Every imported event |
| `item` | A Weigh-In item id (`wi-…`) | Events about an item, in any namespace |
| `subject` | `<method>:<hash>` of a subject (ID-09) | Approvals, decision requests, verdicts and gate results |
| `pr` | A pull request number | Events about a pull request |
| `run` | A Loop run id | Events of a run |
| `task` | A task or ticket key | Events about a task |

Any other key is prefixed with its namespace and a dot (`weighin.channel`, `bench.point`), so plugins never collide on a key. Adding an unprefixed key is a SPEC minor.

**EV-13.** Tags, like `origin` and `recorded_at`, sit outside a payload's canonical digest (Loop ADR-0131). Adding a tag to a type never changes the digest of its payloads.

## Durable facts and telemetry

**EV-14.** Each type is one of two classes, declared with the type:

- **Durable:** a fact a decision, gate or record depends on. Always written under the busy timeout, exported to journals (JR-04, JR-08), never pruned.
- **Telemetry:** measurement of every tool call, hook timing, counters. Dropped when the lock is busy (ST-09), never exported or committed, leaves the machine only as aggregates, and kept for a retention window of 30 days by default. Whether Core prunes telemetry rows or keeps them in a disposable side file is Core's choice; readers see neither beyond the window.

## Order and time

**EV-15.** Within one store, events are ordered by position. Across stores there is no total order: `ts` comes from different clocks and is advisory. An event that answers or depends on another names it in `cites` (a decision cites its request, a verdict cites the gate run), and readers order by citation where it matters (pressure test, Order and time).

## Size

**EV-16.** An envelope serialises to at most 64 KiB. Larger content (a plan, a diff, a transcript) is stored where it lives, in git or a file, and the event carries its digest and a locator.
