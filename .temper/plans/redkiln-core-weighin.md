# weighin writes and reads the Core store through redkiln-core (#17, part 5)

Base: `main` at 1ab3268 (parts 1 to 4 merged: store, envelope, ids, location, suite.toml, telemetry window, subjects, journal codec, checkpoint, import, legacy reader). Branch `claude/project-thread-kdpm1k`.

## Goal
`weighin` keeps every behaviour it has (one write path, DCB guards, hooks that never fail a session, 1.x committed journals), but its events live in the suite's store, `<git-common-dir>/redkiln/events.sqlite3`, opened, located and written only through `redkiln-core`, as SPEC envelopes of type `weighin.<type>`. `weighin` no longer depends on `happenstance-*` or `tokio`, which closes the "`=0.4.5` until it writes through `redkiln-core`" note in the root crate map.

## What lands here, and what waits for step 5

The runbook (Rename at the door) says the event, store and config renames "land with `redkiln-core` (step 2) and the contract crate (step 5)". Split by what each one is:

| Rename | Where | Why |
|---|---|---|
| Events `weighin.<type>`, nested in the EV-01 envelope, tagged `plugin:weighin` | **Here (5a)**, as crate-private names and payload impls in `weighin`. The public contract (the payload types, their versions and upcasters, the guards from `ops/`, golden fixtures) is **step 5** (`weighin-contract`) | Everything appended to the Core store is permanent (ID-02, OW-6) and is read by export and import as an envelope (JR-04, JR-08). Writing 1.x flat events under bare `item_raised` names into the shared log would create facts that never conform (EV-01, EV-03). So the moment weighin writes the Core store, it must write SPEC envelopes. What step 5 adds is sharing those types with siblings, which nothing needs yet |
| `WEIGHIN_STORE` fallback below `$REDKILN_STORE`, with a warning (ST-01) | **Here (5a)** | Store resolution moves onto Core's `locate` in this PR; the old variable's place in the new chain must be decided in the same change, or 2.0 silently ignores it |
| `.redkiln/suite.toml` `[suite]`: `store` | **Here (5a)**, free: Core's `locate` already reads it | Same reason |
| `[suite]` `repo_class` with M1-F's `config.json` fallback, `owner`, and ST-13's "a `client` repo never exports journals or publishes a board" | **Here, as 5b** (a second PR inside #17, after 5a) | Core already implements ST-12, ST-13 and M1-F (PR 3, decisions row 39), so #17 can close the rename; but it changes user-visible behaviour (a folder with no config becomes `client`), touches `init`, and is independent of the store move, so it reviews better alone |
| Weigh-In's own keys (`owner` beyond ST-12, board mode, caps, scan, journal) in a `[weigh-in]` table | **Step 5 or a PR beside it**, not #17 | Weighin-specific config migration; `config.json` stays their source meanwhile. Core reads only `[suite]` (row 39) |

## Constraints and non-goals
- No new third-party dependency. `weighin` gains the workspace edge `redkiln-core` (allowed by the layer rules: a plugin's crates depend on Core; already planned by the runbook and the 2026-11-02 row) and loses `happenstance-core`, `happenstance-sqlite` (normal and dev) and `tokio` (only `store.rs` used it; `cargo machete` would flag it).
- `weighin-core` is unchanged: `EventEnvelope`, `LedgerEvent` and `EVENT_TYPES` (the 1.x names) stay its in-memory model, because the projection, the 1.x journal lines, `stats` and `journal` all use them. The mapping to and from the Core envelope is the store adapter's.
- 1.x committed journals keep their exact line format (flat `EventEnvelope` JSON, `type: item_raised`) until step 7: assay's adapter reads them (runbook, Consumers).
- `ci-check --json` stays `weighin.ci-check/1` byte for byte (it never opens the store).
- Out of scope: the one-time import (step 8), journal refs and Core's checkpoint at Stop (step 7), `status --json` and the hook adapter module (step 6), telemetry (decision D9).
- No harness protocol moves (step 6). Process environment (`REDKILN_STORE`, `WEIGHIN_STORE`, `HOME`/`USERPROFILE`) is read once, in `env`, and passed to Core as arguments (Core's charter: Core reads no environment).

## Invariants (testable)
- weighin never opens SQLite itself: the only store calls are `redkiln_core::store::Store::{open, open_existing, append, read}`. `grep -r 'happenstance\|rusqlite' crates/weigh-in` finds nothing.
- An old weighin ledger (0.1.x, 0.2.2, 1.x) is never opened, stamped or written beside: its bytes and its directory listing are the same after any weighin 2.0 command or hook (ST-15, ST-16, decisions row 31).
- Every event weighin appends is an EV-01 envelope of one of the 13 `weighin.*` types, version 1, trust `asserted` (TR-05: CLI and hook writes), writer `weighin`, with caller tags `item:<id>` and, on a raise, `weighin.channel`, `weighin.kind`, `weighin.door`; Core adds `plugin`, `trust`, `session`, `worktree`. No tag value is truncated (EV-11): one that does not fit is an error and nothing is written.
- A stored weighin event this build cannot use is skipped and counted, never fatal (EV-10); events of other namespaces are skipped (EV-08) and never conflict a weighin write.
- A read never creates the store or its directory (B-01): read paths use `Store::open_existing`.
- A hook's durable write waits at most 2 s (`Wait::Hook`), a command's at most 10 s (`Wait::Command`) (ST-09).
- Round trip: for every `EventEnvelope` weighin writes, reading the store back yields an equal `EventEnvelope`, so the 1.x journal line of an event is the same whether built at write time or from the store (`stats` counts it once).

## Design (PR 5a)

### Placement (ARC-02, ARC-05, ARC-06)
- `crates/weigh-in/weighin/src/store.rs`, rewritten (872 lines to about 200): the adapter over `redkiln_core::store::Store`. Binary crate, `pub(crate)`.
- `crates/weigh-in/weighin/src/store/wire.rs`, new (about 230 lines): the 13 payload declarations and the `EventEnvelope` to and from `Envelope` mapping. Moves to `weighin-contract` at step 5.
- `crates/weigh-in/weighin/src/env/location.rs`, new (about 220 lines): store location over Core's `locate`, the 2.x fallbacks, old-ledger detection and the warnings. `env.rs` is 1,003 lines already; this keeps it from growing (ARC-06).
- `crates/core/redkiln-core/src/store.rs`: one new method, `Store::open_existing` (decision D6). No other Core change.

### Types and signatures

`store/wire.rs`:
```rust
/// A weighin-core payload as the `data` of a Core envelope. Local, so `Payload` (Core's trait)
/// can be implemented for `Wire<Raised>` etc. without the orphan rule (step 5 moves the payload
/// types into `weighin-contract`, which implements `Payload` on them directly).
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub(super) struct Wire<T>(pub(super) T);
```
- One `macro_rules!` table (ARC-15: 13 identical declarations, one source for three uses) of `(LedgerEvent variant, payload type, 1.x name)` generating: `impl Payload for Wire<$payload>` (`TYPE = concat!("weighin.", $name)`, `VERSION = 1`, `CLASS = Class::Durable`); the `TYPES: [&str; 13]` list of 2.0 names; the write dispatch; the read dispatch. A unit test pins the table to `weighin_core::event::EVENT_TYPES` (same 13 names, same order) and every `TYPES` entry to `TypeName::parse` with namespace `Weighin`.
- `pub(super) fn to_new_event(env: EventEnvelope, writer: &Writer) -> anyhow::Result<(NewEvent, EventEnvelope)>`: takes the envelope by value, moves the payload into `Envelope<Wire<T>>`, builds the event with `Envelope::to_event(tags)`, then moves the payload back out and returns the `EventEnvelope` for the ledger. No clone (OWN-08, H-08). Field mapping, each refusal an error with the field named (EV-05, nothing written):
  - `ts`: `Timestamp::parse(&env.ts)` (weighin writes RFC 3339 to the second, which Core accepts byte for byte).
  - `actor`: `Actor::parse(&env.actor)` (`agent`, `hook`, `cli`, `human:<owner>`).
  - `session_id`: `SessionId::parse` (the value was already checked at discovery, D10).
  - `worktree`: `WorktreeId::parse` (the value is already `wt-<12 hex>`, below).
  - `branch`, `head`: as they are; Core checks them.
  - `trust`: `Trust::asserted()`; `writer`: `Writer::new("weighin", env!("CARGO_PKG_VERSION"))`, built once per `Store`.
  - tags: `Tag::core(CoreKey::Item, id)`, and for `ItemRaised` `Tag::namespaced(Namespace::Weighin, "channel" | "kind" | "door", ..)`. 1.x's unprefixed `channel:`, `kind:`, `door:` become `weighin.channel:` etc. (EV-12's own example).
- `pub(super) fn from_stored(stored: &Stored) -> Result<EventEnvelope, Skip>`: dispatch on `stored.event_type` to `decode::<Wire<T>>`, then `EventEnvelope { event, actor: actor.to_string(), session_id: session.map(..as_str..), worktree, branch, head, ts }`. A type outside the table is `Skip::Foreign` (the read filter never returns one).
- `pub(super) fn filter() -> Result<Filter, StoreError>`: `Filter::all().of_type(..)` for the 13 types.

`store.rs`:
```rust
pub(crate) struct Store {
    inner: redkiln_core::store::Store,
    writer: Writer,
}
pub(crate) enum Guard { NotYetRaised(ItemId), ItemUnchangedSince(ItemId, u64) } // unchanged
pub(crate) enum Written {
    /// Appended; the events, handed back for the ledger, sit at `first..=last`.
    At { last: u64, events: Vec<EventEnvelope> },
    /// The guard matched: someone else wrote first.
    Conflict,
}
/// The ledger as read, and how many weighin events this build could not use (EV-10).
#[derive(Debug, Default)]
pub(crate) struct Loaded { pub(crate) ledger: Ledger, pub(crate) unusable: u64 }

impl Store {
    pub(crate) fn open(path: &Path, wait: Wait) -> anyhow::Result<Self>;
    pub(crate) fn open_existing(path: &Path, wait: Wait) -> anyhow::Result<Option<Self>>;
    pub(crate) fn read_all(&self) -> anyhow::Result<Events>; // Events { events: Vec<(u64, EventEnvelope)>, unusable: u64 }
    pub(crate) fn ledger(&self) -> anyhow::Result<Loaded>;
    pub(crate) fn append(&self, envs: Vec<EventEnvelope>, guard: &Guard) -> anyhow::Result<Written>;
}
```
- `append` takes the envelopes by value and hands them back in `Written::At` (OWN-02: the store consumes the bytes, the ledger keeps the values). `ops::Session::write` changes accordingly (it applies the returned envelopes at `first..=last`).
- Guards map to Core conditions, restricted to weighin's types:
  - `NotYetRaised(id)` to `Condition::not_yet("weighin.item_raised", item_tag)`.
  - `ItemUnchangedSince(id, p)` to `Condition::none_matching(wire::filter()?.tagged(item_tag)).after(p)`, **not** Core's `unchanged_since` (which matches any type). EV-12 lets any namespace tag `item:`, and the projection only advances an item's `last_position` on weighin events, so a typed-less guard would conflict forever on the first foreign event about the item. A weighin event this build cannot use still conflicts (fail closed); the conflict message then says the item has events a newer weighin wrote.
- `read_all` pages `inner.read(&filter, after, PAGE)` with `PAGE = 512` until a short page, in position order; `Skip::Unusable` increments `unusable`, nothing else fails the read (EV-10). This replaces today's "decoding event at position N" hard error.
- Errors: `weighin` is a binary, so `anyhow` with `.context()` throughout (H-09, charter). Before converting, `open` matches `StoreError::Open { source: OpenError::OldLedger { .. } | OpenError::NotAStore, path }` and adds the hint: "`<path>` is a weighin 1.x ledger or another program's database; weighin 2.0 imports a 1.x ledger rather than opening it, and left it unchanged".

`env/location.rs`:
```rust
/// The process values store location reads, taken once (Core reads none; tests pass their own,
/// since `set_var` is unsafe in edition 2024).
pub(crate) struct Vars { redkiln_store: Option<OsString>, weighin_store: Option<OsString>, home: Option<PathBuf> }
impl Vars { pub(crate) fn from_process() -> Self; }

pub(crate) struct StoreAt { pub(crate) path: PathBuf, pub(crate) source: Source }

/// Something store location took or found that the user should hear about (CLI-08 wording).
pub(crate) enum Notice {
    /// `$WEIGHIN_STORE` placed the store (2.x only, ST-01).
    WeighinStore { old: PathBuf, store: PathBuf },
    /// `config.json`'s `store` placed the store (2.x only).
    LegacyStoreKey { old: PathBuf, store: PathBuf },
    /// A weighin 1.x ledger is here and is not read until the import (Move 1 step 8).
    OldLedger { path: PathBuf },
    /// suite.toml could not be used, or a key was ignored (Core's warning, `Unusable`/`Value` only in 5a).
    Suite(redkiln_core::suite::Warning),
}

pub(crate) fn locate_store(cwd: &Path, flag: Option<&Path>, legacy_key: Option<&str>, vars: &Vars)
    -> Result<Located2, LocateError>;
// Located2 { store: StoreAt, workspace: Workspace, notices: Vec<Notice> }
```
- Chain (ST-01, with decision D2 option B):
  1. `--store` (Core `Search.flag`);
  2. `$REDKILN_STORE` (Core `Search.env`);
  3. else, when `$WEIGHIN_STORE` is set and not blank: the store is **beside the 1.x ledger it names**, `<parent>/<file stem>.events.sqlite3`, `Source::Env`, notice `WeighinStore`; the named file joins the old ledgers;
  4. `[suite] store` (Core);
  5. else, when `config.json` sets `store` (resolved against the worktree root, `~/` expanded): beside it the same way, `Source::Config`, notice `LegacyStoreKey`;
  6. `<git-common-dir>/redkiln/events.sqlite3`, or outside git `<folder>/.redkiln/events.sqlite3` (Core).
  Implemented as one `redkiln_core::location::locate` call; steps 3 and 5 are applied by weighin around it (step 3 by passing the derived path as `Search.env` when `REDKILN_STORE` is unset; step 5 by replacing a `GitCommonDir`/`Folder` answer). `locate` asks git only when steps 1 and 2 did not choose; otherwise weighin calls `Workspace::discover` itself, so `Env` always has a `Workspace` and git's discovery runs once (Env's own `rev-parse --show-toplevel` and `--git-common-dir` go).
- Old ledgers: `<common_dir>/weighin/ledger.sqlite3` (outside git `<folder>/.weighin/ledger.sqlite3`), plus the files steps 3 and 5 name. Existence only (`try_exists`), never opened, so a hook pays one `stat` each. Each existing one is an `OldLedger` notice. Step 8 replaces the notice with the import.

`env.rs` changes:
- `Env` gains `store: Result<StoreAt, LocateError>` (replaces `store_override`), `notices: Vec<Notice>`, `wait: Wait` (default `Command`; `hooks` sets `Hook` with `with_wait`), and `worktree_id: Option<WorktreeId>` from `WorktreeId::of_toplevel(root, Host::current())`. `worktree` (the directory name) stays for the board's repository label (`commands/board.rs:85`); `envelope()` writes `worktree_id` (ID-06: weigh-in 1.x wrote the directory name, which two clones can share).
- `store_path(&self) -> anyhow::Result<&Path>` and `state_dir(&self) -> anyhow::Result<PathBuf>` become fallible (an unlocated store is an error for commands; hooks fail open). State dir: `<store's directory>/weighin/` (decision D4).
- Session: `SessionId::from_surface` at discovery. A value it refuses: commands stop with an error naming the source; hooks run with no session and SessionStart says so (decision D10).
- `main::environment` bails on `Err(LocateError)` as it does on a broken config. Hooks: SessionStart puts the error in `systemMessage`; other hooks skip the store work, write nothing and exit 0 (CLI-09; there is no state dir to log to).

### Reporting (CLI-08)
- Commands: each notice is one stderr line after the command's output, `weighin: warning: <message>`, through the existing `run` warning path (`journal_after`'s), so `--json` stdout stays one object (CLI-01).
- `SessionStart`: notices and `Loaded::unusable > 0` join the `systemMessage`, once a session. Other hooks print nothing.
- `init` and `board export` name `Env::store_path()` as today.

### Concurrency and resources
- No tasks or threads; Core drives happenstance on its own current-thread runtime. weighin has no runtime of its own after `tokio` leaves, so Core's `InsideRuntime` refusal cannot fire.
- Busy timeouts: `Wait::Hook` (2 s) for hooks, `Wait::Command` (10 s) for commands (was 5 s for both).
- Reads page at 512 events; the whole weighin history is still held in memory for the fold, as today (local store, H-13 bound is the store itself; unchanged).

### Alternatives considered
- Write 1.x flat bytes under `weighin.*` names with raw `NewEvent::new`: rejected, the shared log would hold non-envelopes forever (EV-01, ID-02).
- `weighin-core` depends on `redkiln-core` and implements `Payload` on its own structs: rejected for #17; it pulls SQLite and tokio into the no-I/O domain's build, and step 5's contract crate is where the impls are meant to live.
- Create `weighin-contract` now: a crate-boundary change ahead of its step (H-16), and step 5 also moves the guards and JSON types; not needed to move the store.
- Keep weighin's own `Env` git discovery beside Core's: two discoveries with different failure rules (weighin ignored every git error); ST-01 says only "not a repository" falls to the folder.

## Design (PR 5b, after 5a merges)
- `Env` takes the class from `Located.suite` (Core's `Suite::repo_class`, with `ClassSource`), not `Config::repo_class`; `Config` drops the field (serde ignores the key in old files). `Notice::Suite` then carries every Core warning, `LegacyRepoClass` included (M1-F).
- ST-13 at weighin's two export paths (decision D8): the journal writer skips with a new `Skip::Client`, and `board export` (artifact) refuses, unless the class is `own`.
- `init`: writes `.redkiln/suite.toml` with `[suite] repo_class = "<class>"` when the file is absent (two lines of fixed text: the class is an enum, nothing to escape), refuses with a message when one exists and sets another class, and stops writing `repo_class` to `config.json`. `--ledger` keeps writing `config.json`'s `store` until the `[weigh-in]` table PR. `init`'s receipt names where the class came from.
- Owner: `[suite] owner` (Core's `Owner`), else `config.json`'s `owner`, else git (B-31), as ST-12 lists.
- About 8 files, +350/-80. Tests in its own list below.

## Test list (drive in this order)

PR 5a. Unit tests beside the code; binary tests in `tests/it/`. Fixtures are read from `crates/core/redkiln-core/tests/fixtures/` (no copy).

Core:
- [x] 1. `open_existing` on a missing path is `Ok(None)` and creates nothing (no file, no directory, no `.lock`).
- [x] 2. `open_existing` on a stamped store opens it; on each old-ledger fixture and a non-store database it is the same refusal `open` gives (`OldLedger` or `NotAStore`), never `None`, file untouched.

`store/wire.rs`:
- [x] 3. The table's 13 names are `EVENT_TYPES` prefixed `weighin.`, each a valid `TypeName` in namespace `Weighin`, all version 1, all durable.
- [x] 4. A raise encodes to the expected envelope bytes (one literal golden: `type`, `version`, `ts`, `actor`, `trust`, `writer`, `session`, `worktree`, `data` holding `item_id` and no `type`), with tags `item:`, `weighin.channel:`, `weighin.kind:`, `weighin.door:` plus Core's `plugin:weighin`, `trust:asserted`, `session:`, `worktree:`.
- [x] 5. Every one of the 13 variants round-trips: `to_new_event` then `append` then `from_stored` gives an equal `EventEnvelope` (table-driven over one value per variant).
- [x] 6. An actor (`t`), a `ts` with nanoseconds, a worktree that is a directory name and a too-long tag value are each an error naming the field, and nothing is appended.

`store.rs`:
- [x] 7. Guards (rewritten from today's `guards_make_writes_idempotent_and_detect_races`): a second raise is `Conflict`; of two writers that read at the same position, the first wins and the second gets `Conflict`.
- [x] 8. A foreign event (`core.import_completed`, or a `bench.*` type tagged `item:<id>`) after the read position does not conflict a weighin write, and is skipped by the read with positions intact (EV-08).
- [x] 9. A `weighin.item_raised` stored at version 2, and one whose `data` does not decode, are skipped and counted in `Loaded::unusable`; the rest of the ledger folds (EV-10).
- [x] 10. A read of more than one page (513 events) returns them all in order.

`env/location.rs` (unit, with `Vars` passed in):
- [x] 11. Default: `<common>/redkiln/events.sqlite3` from the main checkout and from a linked worktree; outside git `<folder>/.redkiln/events.sqlite3`; `store_source` `git_common_dir` and `folder`.
- [x] 12. `--store` wins over both variables; `$REDKILN_STORE` wins over `$WEIGHIN_STORE` with no notice.
- [x] 13. `$WEIGHIN_STORE=/x/proj.sqlite3` alone puts the store at `/x/proj.events.sqlite3` with a `WeighinStore` notice; blank is unset.
- [x] 14. `[suite] store` wins over `config.json`'s `store`; `config.json`'s `store` alone places the store beside it with a `LegacyStoreKey` notice.
- [x] 15. Each existing old ledger (default path, the one `$WEIGHIN_STORE` names, the one `config.json` names) is an `OldLedger` notice; a missing one is none.
- [x] 16. A broken `.git` file is a `LocateError` (not the folder store): a command exits 1 with it, a hook exits 0 and writes nothing.

Binary, `tests/it/core_store.rs` (new), per fixture 0.1.x, 0.2.2 and 1.2.0 placed at `.git/weighin/ledger.sqlite3` of a scratch repository:
- [x] 17. A raise succeeds and lands in `.git/redkiln/events.sqlite3`; the fixture's bytes and its directory listing are unchanged (no `-journal`, `-wal`, `.lock` beside it); stderr has one `weighin: warning:` line naming it.
- [x] 18. `SessionStart`'s `systemMessage` names the old ledger; `open` shows none of its items (decision D1).
- [x] 19. `--store <fixture>` and `REDKILN_STORE=<fixture>` exit 1 with the "imports rather than opens" hint, fixture untouched; a hook with it exits 0.
- [x] 20. A read-only command in a repository with only an old ledger creates neither `.git/redkiln/` nor the store (B-01, through `open_existing`).
- [x] 21. A written event's envelope carries `worktree: wt-<12 hex>` of the toplevel, and its 1.x journal line carries the same value (journal and store agree; `stats` counts it once).
- [x] 22. A hook opens with `Wait::Hook` (unit test on the hook's `Env`).

Existing suite, re-pointed (decision D5):
- [x] 23. Every test in `tests/it/` green with the store at `.git/redkiln/events.sqlite3`, the state dir at `.git/redkiln/weighin/` and outside git `.redkiln/`; paths come from one helper in `support.rs`, assertions otherwise unchanged; tests that assert a read leaves no `.weighin/` also assert no `.redkiln/`.
- [x] 24. `ageing.rs` back-dates through Core (decision D5 iv), same assertions.

Manual (not CI):
- [x] 25. `tests/it/live_ledger.rs`, `#[ignore = "manual (#17): reads the 1.x ledger WEIGHIN_LIVE_LEDGER names"]`: `legacy::read_ledger` reads it; the file and its directory are unchanged; every event decodes as a 1.x `EventEnvelope`; every event maps through `wire` once its worktree is dropped (proves step 8's mapping covers real data). Run by Ryan on the redkiln-rs ledger, plus the end-to-end check below.

PR 5b:
- [ ] B1. `[suite] repo_class = "own"` is `own`, from suite.toml.
- [ ] B2. No suite.toml, `config.json` `repo_class: own`: `own`, with the `LegacyRepoClass` warning on stderr and in SessionStart.
- [ ] B3. Neither file: `client` (1.x said `own`).
- [ ] B4. A suite.toml that does not parse: `client`, no fallback, warning.
- [ ] B5. A `client` repository writes no journal (`Skip::Client`) and refuses `board export`; an `own` one does both as today.
- [ ] B6. `init --class own` writes `.redkiln/suite.toml` when absent, refuses a conflicting one untouched, and writes no `repo_class` to `config.json`.
- [ ] B7. Owner: `[suite] owner`, else `config.json`'s, else git's.

### Manual check on Ryan's live redkiln-rs ledger (5a)
Never in place: nothing may be created in that checkout's `.git/`.
1. `sha256sum <redkiln-rs>/.git/weighin/ledger.sqlite3*` and `ls -la <redkiln-rs>/.git/weighin/`; record both.
2. `WEIGHIN_LIVE_LEDGER=<redkiln-rs>/.git/weighin/ledger.sqlite3 cargo test -p weighin --test it live_ledger -- --ignored` (test 25).
3. `REDKILN_STORE=$(mktemp -d)/events.sqlite3 target/debug/weighin --dir <redkiln-rs> open` and the same for `hook SessionStart` with a payload: the old-ledger warning appears, nothing is listed, no `.git/redkiln/` is created.
4. Repeat step 1: identical hashes and listing (while a 1.x binary may be using it: the legacy reader tolerates that, `ATTEMPTS`).
Record the result in the PR body and in this plan's Progress.

## Decisions (H-16)

| # | Decision | Options | Recommendation | Decided by | Outcome |
|---|---|---|---|---|---|
| D1 | What weighin 2.0 does before step 8 when it finds only an old ledger | A) Use the Core store only; never read the old ledger; name it in a warning on every command and in SessionStart. B) Read-through: fold the old ledger (from a copy) with the Core store, write only to Core. C) Refuse to write until it is imported | **A.** 2.0.0 does not ship before step 8, so only dev builds meet this. B mixes two position spaces under one guard (`ItemUnchangedSince` positions would mean two stores) and would double-count once step 8 imports. C blocks the manual check and every dev use in a real repository. Under A nothing is lost: step 8 imports the old ledger into whatever the Core store holds by then (origins are unique) | Ryan, decision card 2026-11-06 | approved as recommended |
| D2 | `$WEIGHIN_STORE` (and `config.json`'s `store`, `init --ledger`) | A) Literal ST-01: the value is the store's path. Every existing value names a 1.x ledger, which Core refuses (row 31), so every user of it gets an error until they set `$REDKILN_STORE`. B) The value names the 1.x ledger; the store goes beside it, `<parent>/<stem>.events.sqlite3`, with a warning, and step 8 imports from the named file. C) Ignore both, warn | **B.** It keeps the reason the override exists (a ledger kept outside a synced tree, B-02) without opening an old file, and gives step 8 its source. It is a reading of ST-01's "MAY keep reading its old variable"; the SPEC text needs no change. A is what the runbook says word for word, and fails closed, if you prefer the letter | Ryan, decision card 2026-11-06 | approved as recommended |
| D3 | Split #17 part 5 into 5a (store) and 5b (`repo_class`, owner, ST-13 gating, `init` writes suite.toml); `[weigh-in]` table after #17 | A) 5a then 5b, both closing #17. B) One PR. C) 5a only; 5b moves to step 5 | **A.** 5a is about +1,100/-950 lines and atomic (the store cannot move half-way: Core refuses the old file). 5b is behaviour, not plumbing, and reviews better alone | Ryan, decision card 2026-11-06 | approved as recommended |
| D4 | weighin's state dir (`hook-errors.log`, `hook-errors.seen`, board exports) | A) `<store's directory>/weighin/` (`.git/redkiln/weighin/`). B) Keep `<common>/weighin/`, beside the 1.x ledger | **A.** It follows the store, as in 1.x; B would share board state and the error log with a 1.x binary writing its own ledger beside it | Ryan, decision card 2026-11-06 | approved as recommended |
| D5 | Test deletions and rewrites | (i) Delete `store.rs`'s 11 unit tests, whose behaviour Core owns and tests (mapping below); `guards_make_writes_idempotent_and_detect_races` is rewritten against the adapter (test 7). (ii) Integration tests' paths `.git/weighin/ledger.sqlite3` to `.git/redkiln/events.sqlite3`, `.git/weighin/<state>` to `.git/redkiln/weighin/<state>`, `.weighin/` to `.redkiln/` (cli.rs, board_guard.rs, recall.rs, journal_write.rs, pane.rs, ci_check.rs, support.rs, scratch.rs, eval_seed.rs; also `env_remove("REDKILN_STORE")` beside `WEIGHIN_STORE`), assertions unchanged. (iii) Unit tests' actor `"t"` becomes `"cli"` (Core refuses `t`), and `worktree: Some("wt")` a `wt-` id. (iv) `ageing.rs`'s raw-SQL back-dating: A) rebuild the store through Core (read every event, set the file aside, append each with its envelope's `ts` replaced and its tags kept, same positions), no happenstance edge; B) keep `happenstance-sqlite` as a dev-dependency for this one test. (v) `cli.rs:219`'s "not a database" message assertion takes Core's wording | **Approve (i) to (iii) and (v); (iv) A.** Each change moves a target, never loosens one; (iv) B would leave the crate map's happenstance note open | Ryan, decision card 2026-11-06 | approved as recommended |
| D6 | Core API addition `Store::open_existing(path, wait) -> Result<Option<Store>, StoreError>` (Core's existing `Mode::Existing`, `NoStore` as `None`) | A) Add it. B) weighin checks `try_exists` and then calls `open`, as 1.x did | **A.** B creates a store when the file vanishes between the check and the open, which is the B-01 case; A is a 20-line workspace-internal addition | Ryan, decision card 2026-11-06 | approved as recommended |
| D7 | Where the event mapping lives in #17 | A) `Wire<T>` newtype impls in `weighin` (`store/wire.rs`), moved to `weighin-contract` at step 5. B) `weighin-core` depends on `redkiln-core` and implements `Payload`. C) Create `weighin-contract` now | **A.** No crate-boundary change; B puts SQLite in the domain's build; C is step 5 early | Ryan, decision card 2026-11-06 | approved as recommended |
| D8 | (5b) ST-13 at weighin's export paths | A) A `client` repository writes no 1.x journal and exports no artifact board. B) Leave both to step 7, where Core's checkpoint asks the class | **A.** ST-13 is a MUST and Core already answers the class; the change is visible only to a `client` repository that had journals or an artifact board on | Ryan, decision card 2026-11-06 | approved as recommended |
| D9 | Telemetry from hooks (`Store::record_telemetry(&mut self, ..)`) | A) None in #17: Weigh-In's 13 types are durable (weigh-in.md, Events) and no telemetry type is defined. B) Add a `weighin.hook_timed` telemetry type now | **A.** A new type name is a one-way addition to the namespace (EV-03, EV-06) and belongs with the contract (step 5) and `status --json`, which reports drops (step 6, CLI-12). When it comes, `Session` already owns its `Store`, so `&mut` is available; read-only paths would open a store of their own | Ryan, decision card 2026-11-06 | approved as recommended |
| D10 | A session id Core refuses (control characters, over 247 bytes) | A) Commands stop with an error naming its source; hooks run with no session and SessionStart says so. B) Truncate, as 1.x's tags did | **A.** EV-11 forbids truncation; a hook still captures | Ryan, decision card 2026-11-06 | approved as recommended |

Changes the SPEC forces, recorded rather than decided: unusable events are skipped and counted (EV-10) where 1.x failed the read; a git failure other than "not a repository" stops a command (ST-01) where 1.x fell back to the folder; written events carry `wt-<hash>` worktree ids (ID-06), so 1.x journal lines written by 2.0 do too; a relative `--store` resolves against `--dir` or the hook's `cwd` (ST-01) rather than the process directory; a relative `$WEIGHIN_STORE` the same way (decision wi-8d37cf asked about it; Core's rule now decides).

Mapping for D5 (i), weighin test to the Core test that holds the behaviour (`crates/core/redkiln-core/tests/it/store.rs` unless noted):
- `ledger_runs_under_truncate_and_never_leaves_wal_files`, `a_new_ledger_is_stamped_and_reopening_writes_nothing`: `a_new_store_is_stamped_runs_under_truncate_and_reopening_writes_nothing`
- `empty_leftovers_from_a_failed_creation_are_recovered`: `an_empty_file_with_a_failed_wal_beside_it_becomes_a_store`, `a_journal_left_by_a_failed_creation_is_set_aside_not_deleted`
- `a_stale_wal_beside_a_truncate_ledger_is_set_aside`: `a_stale_wal_beside_a_truncate_store_is_set_aside_not_deleted`
- `a_wal_ledger_from_an_older_build_converts_and_keeps_its_events`: `a_wal_store_converts_to_truncate_and_keeps_its_events` (and 0.1.x ledgers are now refused untouched: `an_old_ledger_is_refused_untouched_whatever_its_journal`)
- `concurrent_creators_share_one_schema_and_identity`: same name
- `a_failed_commit_is_rolled_back_and_reported`, `a_failed_body_is_rolled_back_and_its_error_returned`: Core's `finish_transaction` unit tests in `src/store/open.rs`
- `a_ledger_from_a_newer_build_is_refused_untouched`: `a_newer_major_is_refused_and_a_newer_minor_opens_with_its_stamp_kept`
- the two 0.1.x and 0.2.2 migration tests (around `store.rs:717`, `:810`): `tests/it/legacy.rs` and `an_old_ledger_is_refused_untouched_whatever_its_journal`
The implementer lists the exact names from `store.rs` in the PR body against this table before deleting any.

## Documents that change with 5a
- Root `AGENTS.md` crate map, `weighin` row: depends on weighin-core, redkiln-core, clap, anyhow, time, tempfile, serde, serde_json (no happenstance, no tokio).
- `crates/weigh-in/weighin/AGENTS.md`: allowed dependencies; invariants ("No deletes beside the ledger" becomes "the store is Core's: weighin opens it only through `redkiln-core`; old ledgers are never opened"); test strategy (`core_store.rs`, `live_ledger.rs`); a decisions row.
- `docs/decisions.md`: one row per decision above as Ryan answers, and a row closing the crate-map note.
- `plugins/weigh-in/CHANGELOG.md` `## Unreleased`; `plugins/weigh-in/README.md` "Where the ledger lives"; `plugins/weigh-in/skills/weigh-in/SKILL.md`'s Cowork line (`F/.weighin/` to `F/.redkiln/`).
- No SPEC text change (D2 B is a reading of ST-01's MAY).

## Risks
- **The round trip loses a field.** The 1.x journal line is rebuilt from the store on later reads; a field the mapping drops would make `stats` count one event twice. Test 5 (all 13 variants) and test 21 catch it.
- **A guard that never clears.** A weighin event this build cannot use, tagged with an item, conflicts every later write to that item. Intended (fail closed); the conflict message names the cause. Test 9 pins the skip, test 8 that foreign events do not block.
- **Location now errors where 1.x carried on** (dubious ownership, broken `.git`). Hooks fail open (test 16); a user sees the error from commands, which ST-01 wants.
- **Latency.** Discovery is now one git call for location plus weighin's two (branch, head); the 1.x path made four. Old-ledger detection adds up to three `stat`s.
- **The fixtures are Core's.** If Core regenerates them, weighin's tests follow; they are referenced, not copied.
- **Hidden callers of `store_path()`** become fallible; the compiler finds them all.

## Size (lower bound)
- 5a: about 16 files. Production: `store.rs` (-872, +200), `store/wire.rs` (+230), `env/location.rs` (+220), `env.rs` (+60, -70), `ops.rs` (+25, -20), `hooks.rs` (+50), `main.rs` (+30), `commands/{init,board}.rs` (+6), `Cargo.toml` (+1, -5), Core `store.rs` (+25) and its tests (+40). Tests: `core_store.rs` (+220), `live_ledger.rs` (+60), unit tests in `wire.rs`/`store.rs`/`location.rs` (+300), path rewrites in nine `tests/it` files (about 80 lines changed), `ageing.rs` back-dating (+40, -12). Docs about +60. Roughly +1,550 / -1,000, of which new production code is about 820 lines.
- 5b: about 8 files, +350 / -80.

## Progress
- 2026-11-02: plan written (architect). Waiting on D1 to D10.
- 2026-11-06: Ryan approved every recommendation by decision card (H-16): D1 A, D2 B, D3 A, D4 A, D5 (i) to (iii) and (v), (iv) A, D6 A, D7 A, D8 A (5b), D9 A, D10 A. Only 5a is built here.
- 2026-11-06, first implementer (cut off by a container restart): 43a364f `Store::open_existing` (tests 1, 2), 5330d60 weighin on Core (tests 3 to 16, 22 to 24), 7414f28 `core_store.rs` (17 to 21) and the manual check (25), 5c67618 docs. **Red runs not observed**: those sessions' red runs were lost with the restarts and are not claimed here; the reverts below stand in for them.
- 2026-11-07, this session:
  - **The fixture siblings.** `weighin-0.1.x.sqlite3-shm` (32 KiB) and `-wal` (0 bytes) were not made by a test. The first implementer's transcript shows python `sqlite3.connect('file:<fixtures>/weighin-<v>.sqlite3?mode=ro', uri=True)` run on the fixtures in place at 2026-11-06 21:20:24 to 21:20:35, the files' mtime; a read-only connection to the WAL-mode 0.1.x ledger creates `-shm` and `-wal` and cannot remove them on close. `git add -A crates` at 21:26 committed them in 7414f28. The second implementer then removed them on purpose (`git rm --cached` and `rm`) and was cut off before committing. main never had them. Removed again (a70f242). The full workspace suite run afterwards left the directory with exactly the three ledgers, mtimes unchanged.
  - **Guard.** `crates/core/redkiln-core/tests/it/fixtures.rs::the_fixtures_directory_holds_only_the_three_ledgers_as_committed` pins the listing and each sha256 (7af6f3b). Red: with the two siblings put back, `left: [.., "weighin-0.1.x.sqlite3-shm", "weighin-0.1.x.sqlite3-wal", ..]`; green without.
  - **Test 6 completed** (f14e834): `an_item_id_core_refuses_as_a_tag_is_an_error_naming_the_tag_never_cut`. weighin's own tag values are bounded (item ids, enum names), so the plan's "too-long tag" is reachable only through Core's `session` tag, already a case of `a_field_core_refuses_is_an_error_naming_it`; the new test pins the reachable tag refusal (a 1.x item id is any string). First run red on the test's own expectation (Core says "not a valid item tag value", not the length message); after correcting it, red shown by reverting the tag's context: `left: "tagging the event"`.
  - **Reds shown now by temporarily reverting** (each restored, tree clean after):
    - test 1: `open_existing` with `Mode::Create`: `opening_only_an_existing_store_at_a_missing_path_is_none_and_creates_nothing` panics at `existing.rs:30`.
    - test 8: guard over `Filter::all()` (Core's any-type shape): `another_namespace_neither_conflicts_a_write_nor_reads_as_weighin`, `left: None, right: Some(4)`.
    - test 9: unusable not counted: `a_weighin_event_this_build_cannot_use_is_skipped_and_counted`, `left: 0, right: 2`.
    - tests 13, 14: store beside a named ledger as `<stem>.sqlite3`: `weighin_store_names_the_old_ledger_and_the_store_goes_beside_it` and `suite_store_wins_over_the_config_key_...` (`left: "/x/proj.sqlite3"`).
    - tests 15, 17, 18: old ledgers never noticed: `each_old_ledger_that_exists_is_a_notice_and_a_missing_one_is_none`, `a_raise_beside_an_old_ledger_...`, `session_start_names_the_old_ledger_...` fail.
    - test 19: no import hint: `a_store_path_naming_an_old_ledger_is_refused_with_the_import_hint` fails.
    - test 20: reads through `Store::open`: `a_read_beside_only_an_old_ledger_creates_no_store` panics at `core_store.rs:214`.
    - test 21: worktree as `wt-<dir name>`: `a_written_events_worktree_is_its_toplevels_id_in_the_store_and_the_journal` fails.
    - test 22: hook with `Wait::Command`: `left: (Command, Command), right: (Hook, Command)`.
    - Not shown by revert: 2, 3 to 5, 7, 10 to 12, 16, 23, 24 (each passes on this tree; their reds are not observed).
  - **Test 25** lives at `src/store/wire/tests.rs::a_live_ledger_reads_untouched_and_every_event_maps_through_the_wire`, not `tests/it/live_ledger.rs` (it needs the crate-private wire; `#[ignore = "manual (#17): ..."]`). Run on scratch copies of each fixture: passes for 0.1.x, 0.2.2, 1.2.0, nothing beside the copy. **Ryan's live redkiln-rs check not run** (no such checkout here).
  - **D5 (i) mapping, for the PR body.** Deleted from 1ab3268's `store.rs`, against the Core test holding each: `ledger_runs_under_truncate_and_never_leaves_wal_files`, `a_new_ledger_is_stamped_and_reopening_writes_nothing` -> `a_new_store_is_stamped_runs_under_truncate_and_reopening_writes_nothing`; `empty_leftovers_from_a_failed_creation_are_recovered` -> `an_empty_file_with_a_failed_wal_beside_it_becomes_a_store`, `a_journal_left_by_a_failed_creation_is_set_aside_not_deleted`; `a_stale_wal_beside_a_truncate_ledger_is_set_aside` -> `a_stale_wal_beside_a_truncate_store_is_set_aside_not_deleted`; `a_wal_ledger_from_an_older_build_converts_and_keeps_its_events` -> `a_wal_store_converts_to_truncate_and_keeps_its_events`; `concurrent_creators_share_one_schema_and_identity` -> same name; `a_failed_commit_is_rolled_back_and_reported`, `a_failed_body_is_rolled_back_and_its_error_returned` -> `finish_transaction` tests in `src/store/open.rs`; `a_ledger_from_a_newer_build_is_refused_untouched` -> `a_newer_major_is_refused_and_a_newer_minor_opens_with_its_stamp_kept`; `a_wal_ledger_from_0_1_is_stamped_and_reads_back`, `a_truncate_ledger_from_0_2_2_is_stamped_and_reads_back` -> `tests/it/legacy.rs` and `an_old_ledger_is_refused_untouched_whatever_its_journal`. `guards_make_writes_idempotent_and_detect_races` rewritten as test 7.
  - Charters: root crate map's weighin row (no happenstance, no tokio, `redkiln-core`), Core's (`open_existing` invariant, fixtures pin), weighin's (fixtures never opened in place). `cargo tree -p weighin --depth 1`: anyhow, clap, redkiln-core, serde, serde_json, tempfile, time, weighin-core.
  - Checks: temper gate GREEN (fingerprint bcb3e2c6def75a54; 772 tests, 1 skipped), `cargo xtask layers` ok, `cargo xtask plugins` ok, `cargo deny check` ok, `sh tools/charters/check.sh` ok (Core chain 24,177 bytes). Independent review: not yet requested.
