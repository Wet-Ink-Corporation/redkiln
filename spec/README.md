# Redkiln Core SPEC

The SPEC is the part of Redkiln Core that is text: the formats every Redkiln binary reads and writes, so that the suite's shared data means the same thing whichever plugin wrote it. `redkiln-core` is the code that implements it, and the conformance kit is the code that checks it. Neither exists yet. Move 1 (Weigh-In) gives this text its first code.

**Version: 1.0.0-draft.1** (2026-10-31). The SPEC is versioned on its own (semver), and each plugin declares the SPEC version it conforms to in its `status --json` ([cli.md](cli.md)). It freezes as 1.0.0 at Move 2, when Bench's ledger conforms and Core has two real users (migration plan, Work sequence). Until then every section may change; the one-way items below are the ones that become expensive to change once events are written, so they are the ones to read first.

## Sections

| File | Covers | Ids |
| --- | --- | --- |
| [store.md](store.md) | Where the event log lives, how it is opened, its schema version, writers, projections, repo class, importing old ledgers | ST |
| [events.md](events.md) | The event envelope, `<plugin>.<type>` names, per-type versions, tags, lenient reads, order, durable facts and telemetry | EV |
| [ids.md](ids.md) | Store and event identity, content-hash ids, worktree ids, digests and the hash methods | ID |
| [trust.md](trust.md) | The three trust levels every event carries, and what each may satisfy | TR |
| [approvals.md](approvals.md) | The one approval record, bound to a subject hash | AP |
| [decision-requests.md](decision-requests.md) | The one shape for a plugin's question to the human, and its answer | DR |
| [journals.md](journals.md) | The JSONL export of durable facts, its checkpoint and idempotent import | JR |
| [cli.md](cli.md) | The CLI result envelope, exit codes, stderr, hooks and `status --json` | CLI |

Each requirement has an id (`ST-03`, `EV-11`) so that a conformance fixture, a test or a review comment can cite it. Ids are permanent: a dropped requirement keeps its id, marked withdrawn, and is never reused.

The key words MUST, MUST NOT, SHOULD, SHOULD NOT and MAY are used as in RFC 2119.

## Not in this draft

The Blueprint puts more in the SPEC than this draft covers. These follow, each with the move that first needs it:

| Part | Arrives with | Source |
| --- | --- | --- |
| End-of-turn grammar (`Status:` line, optional lines, fenced plugin blocks) | Move 2 (Bench reads Rust's `WIP`) | temper's Status grammar; weigh-in's marker block |
| Hook conventions (fail open, guards fail closed, Stop block caps, latency budget) | Move 2 | anneal `docs/exit-codes.md`, `docs/hooks.md`; Blueprint design rule 7; pressure test, Hooks from several plugins |
| `.redkiln/suite.toml` in full (pins, surfaces, per-plugin tables) | Move 1 for the keys here; the rest with the bundle | Blueprint, Data and state; pressure test, Version skew |
| Artifact rendering (Layout C) and spec artifact homes | Move 4 | Loop ADR-0005, ADR-0189; pressure-test call E |
| Process-catalog format | Move 4 | Loop's catalog; pressure-test recommendation 16 |
| Capacity leases and calibration records (`core.*` types) | Move 2 (calibration), Move 3 (capacity) | Blueprint, Redkiln Core |
| Schemas and golden fixtures | Move 1, beside the code | assay's schema and fixture discipline |

## One-way items

These freeze at Move 2 with SPEC 1.0. Each is cheap to change while no event carrying it has been written outside a test, and costly after: every stored event, committed journal and plugin that reads them would need a migration. Each needs Ryan's confirmation before Move 1 writes it to a real store; until then the draft carries the recommendation.

| # | Item | Draft recommendation | Section |
| --- | --- | --- | --- |
| OW-1 | Store location and file name | `<git-common-dir>/redkiln/events.sqlite3`; `<folder>/.redkiln/events.sqlite3` outside git | ST-01 |
| OW-2 | Schema version encoding in `user_version` | `major * 1000 + minor`; a reader refuses only a newer major | ST-06 |
| OW-3 | The envelope's shape | Payload nested under `data`, not flattened beside the envelope keys | EV-01 |
| OW-4 | Event type names | `<namespace>.<name>`, namespace from a closed registry, `snake_case` names | EV-03 |
| OW-5 | Per-type versions | An integer `version` per event; additive within a version | EV-06 |
| OW-6 | Event identity | happenstance's `EventId` (origin store id and position), kept across export and import | ID-02 |
| OW-7 | The worktree id | `wt-` and 12 hex of the normalised top-level path's sha256 | ID-06 |
| OW-8 | Content-hash id length | 10 hex for new ids; 6-hex `wi-` ids kept and read forever | ID-04 |
| OW-9 | Trust levels on every event | `asserted`, `witnessed`, `attested` from the first release (pressure-test call C) | TR-01 |
| OW-10 | The approval record | One `core.approval_recorded` type bound to a subject (kind, method, hash) | AP-01 |
| OW-11 | The decision request record | The fields in DR-02, as one shape for every plugin's human gate (Weigh-In call A) | DR-02 |
| OW-12 | Journal line format | One stored envelope per line, with its `origin` | JR-02 |
| OW-13 | Where journals live | Undecided: pressure-test call A waits on the refs spike; the draft writes committed files until then | JR-07 |
| OW-14 | Hash method names and digest forms | `sha256`, `git-commit`, `git-tree`, `patch-id`, `rust-fingerprint`, written `<method>:<hex>` | ID-09 |
| OW-15 | Content-hash id derivations | Each id kind's domain string and input fields; Weigh-In's captured-item input kept as today | ID-04, DR-03 |

One dependency call sits beside the table: identity across imports needs a public happenstance ingest call that 0.3.2 lacks (ST-04), to be settled before Move 1 as an H-16 decision.

Everything else in the draft is two-way: it can change in a SPEC minor or a Core release without touching stored data.

## Where this comes from

The SPEC is extracted, not invented (migration plan, principle 4). Its first version is weigh-in's and anneal's ledgers made one, which are near-copies today, plus the shapes the planning documents settled:

- `references/docs/blueprint.md`: Redkiln Core, How they compose, Data and state, design rules 1, 5, 7, 9 and 10.
- `references/docs/blueprint-pressure-test.md`: Which hash an approval binds to, Scenarios, Trust, Event schemas over time, Order and time, Volume, retention and privacy, calls A to C.
- `references/docs/weigh-in.md`: The decision model, What a request carries, Where an unattended question is answered.
- weigh-in at 1.1.1: `crates/weighin/src/store.rs`, `env.rs`, `git.rs`, `journal.rs`, `ci/report.rs`; `crates/weighin-core/src/event.rs`, `id.rs`.
- anneal at `1952536`: `docs/ledger.md`, `docs/exit-codes.md`, `crates/anneal-cli/src/envelope.rs`, `crates/anneal-ledger/`.
- happenstance 0.3.2: `StoreId`, `EventId`, `MAX_TAG_LEN`, `MAX_EVENT_TYPE_LEN`.
- redkiln-rs `build/phase-0-to-1`: ADR-0105 clause 4 (tree hash), ADR-0131 (tags outside the canonical digest), ADR-0009 (gate channel).

Where the two ledgers disagree, each section says which one the SPEC follows and why.
