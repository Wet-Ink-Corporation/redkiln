# State and storage

The SPEC (`spec/store.md`, `events.md`, `journals.md`) defines the shared event log; the refs spike (OW-13, PR #9) settled where journals live. This chapter is about the behaviour behind those formats: what Weigh-In, anneal, Loop and the Node Redkiln learned about SQLite in synced folders, versioning, authority and locks.

## Guardrails

### SS-1 Nothing beside the store is ever deleted, and read-only commands create nothing

Cowork's connected folders refuse `unlink`, so the store runs under a TRUNCATE journal; a stale `-wal` is renamed aside, never deleted; offset files are overwritten, never removed; per-process lock files (flock) live in the temp directory, never beside the store, while locks and reservations every worktree must share live under `<git-common-dir>/` as store rows or refs, not deletable files ([SS-7](#ss-7-locks-and-id-reservations-live-under-git-common-dir)); writers create the store lazily on their first write; read-only commands never create one.

- **Why.** Creating a ledger under WAL or DELETE journals failed with `SQLITE_IOERR_DELETE` (2570), confirmed with strace and an unlink-refusing shim; `stats` once left an undeletable `.weighin/` in a user's folder.
- **Evidence.** weigh-in B-01, B-11, B-28, `crates/weighin/src/store.rs:200`, `@4b8f28d`; SPEC ST.

### SS-2 One owner of the file, one version stamp, and only a newer major is refused

Only the core opener touches `events.sqlite3`. The product's schema version lives in `PRAGMA user_version` (happenstance keeps its own counter elsewhere); migrations run step by step inside one `BEGIN IMMEDIATE`; a store of the same major and a newer minor opens, reads and appends without lowering the stamp, and only a newer major is refused with "upgrade", untouched (SPEC ST-07). weigh-in refused any newer stamp; the suite cannot, because one plugin's upgrade must not lock a sibling out. Tests build real old stores from copied payloads. Nothing else opens the file to sniff, hash or copy it: closing any descriptor drops SQLite's POSIX locks.

- **Evidence.** weigh-in B-10, `crates/weighin/src/store.rs:349-387`; SPEC ST-06 (OW-2), ST-07; prior-art/platform.md §1 (How To Corrupt An SQLite Database File).

### SS-3 Readers skip what they do not know, and only a newer major locks a reader out

A reader decodes its own event types and skips the rest; additive changes never lock a sibling out. This is a conformance test, not a convention.

- **Why.** anneal fails on the first foreign event and refuses a newer `user_version`, so one plugin's upgrade would lock out another (blueprint, What the audit changed).
- **Evidence.** blueprint, Data and state; SPEC EV (lenient reads); Fowler's Tolerant Reader (prior art below).

### SS-4 The deleted-database test decides what is authoritative

Ask of every field: "if the database were deleted, would the next run take a different action?" If yes, that state is written to git first (a trailer on a checkpoint commit), and a failed git write leaves no store row. Everything else is disposable and rebuildable. A rebuild command reproduces the next action from git alone, and a planner accepts only state reconciled in this invocation.

- **Why.** Loop's lane stalled across processes because the next stage was read from a record file that never moved; a second run reported "done" having done nothing. The fix put the stage on a `Story:` trailer and folded it from git.
- **Evidence.** redkiln-rs ADR-0109, ADR-0157, ADR-0161, ADR-0181, ADR-0183; blueprint design rule 5.

### SS-5 Every stage is re-entrant from durable state

A stage that mutates must be safe to run again after it stopped halfway: it reads what is already done and does the rest. Resume state that has to distinguish outcomes is recorded on enough axes to tell them apart.

- **Why.** In the Node Redkiln, `/redkiln:plan` threw on any re-run after its first mutation; closeout's five gated mutations had no recovery from partial completion; a slice rejected at review looked identical in history to one that passed, so repair took three launches and five human interventions until a `Slice-Verdict:` trailer was added.
- **Evidence.** redkiln-old B1-01, B1-03, B1-04, B1-12, `references/2026-07-22-slice-rereview-gap-and-gate-flake-budget.md`.

### SS-6 Nothing on the hot path waits, and writers start with `BEGIN IMMEDIATE`

Hook writes use a bounded busy timeout and take the write lock up front (`BEGIN IMMEDIATE`): a deferred transaction that reads then writes gets `SQLITE_BUSY` at once, and the busy timeout does not help. Best-effort events (telemetry) are dropped and counted when the lock is busy; guards decide first and record second.

- **Evidence.** blueprint, Data and state; anneal hook budgets ([HK-5](hooks.md#hk-5-budget-time-inside-the-hook-not-by-trusting-timeout)); [SQLite transactions](https://www.sqlite.org/lang_transaction.html).

### SS-7 Locks and id reservations live under `<git-common-dir>/`

Every worktree must resolve the same lock, so shared locks and id reservations resolve under `<git-common-dir>/`, held as store rows or refs rather than files that would have to be deleted ([SS-1](#ss-1-nothing-beside-the-store-is-ever-deleted-and-read-only-commands-create-nothing)). A lock holder is presumed alive until proven dead by more than a wall-clock step; fsync the parent directory as well as the file; never assume a cross-host lock can be cleared locally.

- **Why.** Ids collided across worktrees until the lock moved; the Node RFC says this area "has been wrong three times".
- **Evidence.** redkiln-old 0.12.0 CHANGELOG, A5-05, A5-06, A5-12.

### SS-8 One write path, and the read happens inside the lock

Validate, stamp, canonical key order, take the lock, read the current body inside it, serialise, fsync a temporary file, rename. Reading inside the lock is the fix; "the lock alone never was".

- **Evidence.** redkiln-old `CHANGELOG.md:2880-2885` (nine bare `writeFileSync` sites replaced).

### SS-9 A failed remote write is probed before it is retried

After a failed conditional write, the write may or may not have landed. Read back before resending; a same-content result is success, a different one is a conflict.

- **Why.** An assay state update could be applied twice because the adapter assumed a failed PUT wrote nothing. anneal's `plan approve` reads back a lost race: same hash is "already approved" (exit 0), another is a conflict (exit 3).
- **Evidence.** assay wi-67ab73, `crates/assay-forge/src/github/ledger_refs.rs:50-56`; anneal `crates/anneal-cli/AGENTS.md:187,194`.

### SS-10 A budget is re-checked after every wait

A check made before a long wait is stale after it. Admission, reservation and release are separate steps, and an unknown spend is charged at the reservation.

- **Evidence.** assay #297 (admit checked the budget before the slot wait and never again), R3-06, `crates/assay-app/src/release.rs`; redkiln-rs #9, #47 (admission not atomic).

## Patterns worth copying

- **Idempotent appends by content-hash id and conditional guards.** weigh-in ids are a prefix of `sha256(session, normalised question)`, so a hook and a skill can raise the same item twice safely; every append carries a guard (`NotYetRaised`, `ItemUnchangedSince`), and the projection folds by store position, never by timestamp (weigh-in `crates/weighin/src/store.rs:63-186`, `crates/weighin-core/src/id.rs`).
- **Build the library's schema on the temp disk and copy it into a TRUNCATE file.** happenstance's opener assumed WAL, so weigh-in builds a template in a temporary directory, attaches it, copies the DDL and identity rows inside one `BEGIN IMMEDIATE`, and stamps `user_version` in the same transaction, so concurrent creators end with one identity (weigh-in `crates/weighin/src/store.rs:215-317`).
- **Tags and the store's timestamp stay outside the canonical digest.** Loop digests events as `{kind, payload}`; selector keys ride beside it, so adding addressing never moves a golden log (redkiln-rs ADR-0131, ADR-0182).
- **Journals are append-only and deduplicated by line.** A complete line is never moved or removed; the only bytes ever dropped are an unterminated last line that does not decode, and its event is re-appended in the same write. `flock` is shared by the shell and the binary (weigh-in B-21, B-28, `crates/weighin/src/journal/append.rs:134-243`). OW-13 moves journals into per-session refs; the line discipline carries over.
- **One partition per writer, committed with `merge=union` on exactly the generated paths.** The index keeps the longest copy of a name, so one writer per name is the load-bearing invariant (redkiln-old `.gitattributes`, `test/telemetry-committed.test.ts`).
- **Bounded compare-and-set with no sleeps.** Read, change, conditional write; on conflict re-read and re-run a closure that derives everything from what it is shown; give up after five attempts as `Contended`. The port has no clock, so tests need none (assay `crates/assay-app/src/ledger/cas.rs`).
- **A memo key folds in exactly the inputs each check reads**, enumerated beside the checks; a flaked run is never memoized (redkiln-old `CHANGELOG.md:1671-1690`).

## Gotchas

- **WAL does not work over a network filesystem**, and separating a database from its `-wal` can lose transactions; sync clients move side files (SQLite WAL docs; weigh-in B-01).
- **Loop asserts WAL**, refuses other modes and refuses network filesystems; the load half of R-SQLITE has never run (redkiln-rs `crates/rk-store/src/conn.rs:186-200`). See [Tension T8](README.md#tensions-with-the-plan).
- **A tracked config file with a per-machine value points every clone at one target**: weigh-in's published board URL in `.kb/_ledger/config.json` would make every cloud container overwrite the same board (weigh-in B-20).
- **The board's single state document overflowed at about 700 items** (Artifact `db` documents hold 262,144 bytes) (weigh-in #19).
- **Same-second timestamps tie**; the Node writer moved to milliseconds after 2 of 9 runs went red (redkiln-old `CHANGELOG.md:2880-2885`). Order by store position, not time.
- **The `node:sqlite` builtin was experimental and loaded eagerly**, breaking every command including `--version` where unavailable (redkiln-old A6-04).
- **A hash chain that covers 9% of a ledger guarantees nothing** (redkiln-old A7-11).

## Open problems

- **Journals are never replayed into a new container's store**, so cloud sessions still settle everything before they end (weigh-in B-21, B-28).
- **R-64**: can a rebuild re-derive every event-store tag from git? It decides whether the shared store can be a rebuildable cache for Loop (redkiln-rs #76, #90).

## Prior art

- SQLite: TRUNCATE commits without changing the directory; PERSIST never deletes or truncates. **Agree; adopt** PERSIST as the documented fallback where truncation is also refused. [PRAGMA statements](https://www.sqlite.org/pragma.html)
- Stamp `application_id` beside `user_version` so tools and our own opener can recognise the file. **Adopt** (a SPEC change, so a decision for Ryan before SPEC 1.0). [PRAGMA statements](https://www.sqlite.org/pragma.html)
- Never close a descriptor SQLite has open; link one copy of SQLite. **Adopt** a test that nothing but the core opener opens the file, and watch for two `libsqlite3-sys` builds. [How To Corrupt An SQLite Database File](https://www.sqlite.org/howtocorrupt.html)
- Fowler's Tolerant Reader and Complete Rebuild. **Agree**; adopt a shared fixture corpus of every plugin's events that each reader's tests replay. [Tolerant Reader](https://www.martinfowler.com/bliki/TolerantReader.html), [Event Sourcing](https://martinfowler.com/eaaDev/EventSourcing.html)
- Greg Young: weak schema and upcasting; avoid strongly typed versions unless you can stop the system. **Agree**; adopt a named upcast step (raw JSON to current shape, a pure function) for the rare non-additive change. [Versioning in an Event Sourced System](https://leanpub.com/esversioning/read)
- An industry study of 19 event-sourced systems found schema evolution and projection rebuilds the main pains, and "never change events" cited often but rarely practised. **Adopt**: measure projection rebuild time on a large synthetic log in CI. [Overeem et al.](https://dspace.library.uu.nl/bitstream/1874/412217/1/1_s2.0_S0164121221000674_main.pdf)
- git-branchless keeps an event log in SQLite fed by git hooks and replays it at start-up; it never built checkpoints and slows down with long logs. **Adopt**: plan projection snapshots before logs grow. [git-branchless architecture](https://github-wiki-see.page/m/arxanas/git-branchless/wiki/Architecture)
