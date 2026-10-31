# The store

One event log per repository holds the suite's facts. Every plugin's binary opens it through `redkiln-core` and appends namespaced events to it ([events.md](events.md)). Anything derived from it is a projection and can be rebuilt (Blueprint design rule 5).

## Location

**ST-01 (one-way, OW-1).** A binary resolves the store from the hook input's `cwd` or the process working directory, first match wins:

1. `--store <file>`, relative to the working directory;
2. `$REDKILN_STORE`, with a leading `~/` expanded; empty or blank counts as unset;
3. `store` in `.redkiln/suite.toml` (ST-12), relative to the worktree root;
4. `<git-common-dir>/redkiln/events.sqlite3`, the same file from the main checkout and every linked worktree, never committed;
5. outside git, `<folder>/.redkiln/events.sqlite3`.

Step 5 applies only when git says the directory is not in a repository. Any other git failure (dubious ownership, a broken `.git` file, git missing) stops the command with an error, so a repository git refuses never gets a second, per-folder store (anneal `docs/ledger.md`).

**ST-02.** Resolution starts from the hook input's `cwd`, never `CLAUDE_PROJECT_DIR`. `CLAUDE_PROJECT_DIR` stays at the original root when a session moves into a worktree; `cwd` follows it (pressure test, Hooks from several plugins).

**ST-03.** A command that reports on the store names the file in `store` and the step that chose it in `store_source`: `flag`, `env`, `config`, `git_common_dir` or `folder`.

The per-plugin overrides (`WEIGHIN_STORE`, `ANNEAL_STORE`, `--store` on each CLI) become this one chain at the rename at the door. A plugin MAY keep reading its old variable for one major version, as a fallback below `$REDKILN_STORE`, and SHOULD warn when it does.

## Opening

**ST-04.** The store is a happenstance SQLite event store. Core pins one happenstance version for every binary; the move from 0.3.2 to 0.4.1 happens once, in Core, with fixtures from 0.1.x and 0.2.2 ledgers (migration plan, risks). The pinned version must offer a public ingest call that stores an event under its existing `EventId` (ID-02, JR-11). 0.3.2 has the unique origin columns but no public ingest call. Neither has 0.4.3, the newest release on 2026-10-31: `happenstance-sqlite` implements ingest only under `cfg(test)`, against the unpublished `happenstance-sync` port, which its own notes say a later phase publishes as a feature. Ryan decided on 2026-10-31 that the call comes from a happenstance release, not a `[patch]`, so Core pins the first release that publishes the ingest port, and Move 1's journal import waits for it.

**ST-05.** The store runs under SQLite's TRUNCATE journal mode, never WAL and never DELETE. WAL and DELETE both delete files beside the database, and Cowork's connected folders refuse deletes (`SQLITE_IOERR_DELETE`) and WAL's shared memory. TRUNCATE keeps one `events.sqlite3-journal` beside the store, emptied at each commit and never deleted. Concretely:

- A binary never opens the store through `SqliteEventStore::open`, which switches the file to WAL. Core opens its own connection, sets the journal mode, and, for a new file, copies happenstance's schema from a template built on the local temporary disk, inside one `BEGIN IMMEDIATE` so that two processes creating the store cannot both write a schema (weigh-in `store.rs`).
- A file found in WAL is converted to TRUNCATE on open, retrying until the busy timeout while another connection holds it. A stale `-wal` file beside a TRUNCATE store is renamed aside, never deleted and never replayed (weigh-in's rule; anneal refuses such files instead, which the SPEC does not keep, because Loop's store and weigh-in 0.1.x ledgers are WAL today).
- Nothing in Core deletes a file beside the store. Files are set aside by renaming.

R-SQLITE (eight concurrent writers, hooks included, on Linux and Windows) has not run yet. If it fails under TRUNCATE, Loop keeps its tier-2 events in its own WAL file and exports a journal into the log instead (Blueprint call 1); the rules here do not change.

## Schema version

**ST-06 (one-way, OW-2).** Only Core writes `PRAGMA user_version`. It holds the store schema as `major * 1000 + minor`; SPEC 1.0's store schema is 1.0, stamped `1000`. A minor change is additive: a new table, index or column that a binary built on an older Core can ignore while it reads and appends. A major change is anything else.

**ST-07.** On open, Core compares the stamp with its own schema:

- **Same major, same or older minor:** it opens; an older minor is migrated forward one step at a time inside one `BEGIN IMMEDIATE`.
- **Same major, newer minor:** it opens, reads and appends, and never lowers the stamp. One plugin's upgrade never locks a sibling out (Blueprint, Data and state).
- **Newer major:** it refuses, names both versions, and leaves the file untouched.
- **A SQLite database with tables that is not a store** (a mistyped `--store`): it refuses and leaves the file untouched.

The version is read from the file's 100-byte header before SQLite opens it, so a refusal never creates a journal or WAL file beside it (anneal `docs/ledger.md`).

**ST-08.** happenstance keeps its own migration counter in `store_meta`. Core does not reuse `user_version` for it, and a happenstance migration is a store schema step like any other.

## Writers

**ST-09.** Every append is its own `BEGIN IMMEDIATE` transaction. Writers wait for a lock up to a bounded busy timeout: at most 10 seconds for a command, at most 2 seconds for a hook's durable write. A telemetry event (EV-14) never waits: when the lock is busy it is dropped, and the drop is counted in the process's own report or the next durable write's context, never retried in a loop (Blueprint, Data and state: nothing on the hot path waits).

**ST-10.** A write that must not race is a conditioned append (DCB): happenstance's `AppendCondition`, a query that must match no event, optionally after a position the writer read. Of two writers racing on one condition exactly one wins; the other gets a conflict, re-reads and decides again. Core offers the guards both ledgers use today: *not yet raised* (no event of a type with this subject tag exists) and *unchanged since* (no event with this subject tag after position *p*).

**ST-11.** A guard decides first and records second. A hook that must answer allow or deny never waits on the store to answer, and a failed record never turns an allow into a deny.

## Repo class and config

**ST-12.** `.redkiln/suite.toml` is committed and reviewed. SPEC 1.0 defines three keys in its `[suite]` table; the rest of the file arrives later (README, Not in this draft):

| Key | Values | Default |
| --- | --- | --- |
| `repo_class` | `own` or `client` | `client` |
| `store` | a path, relative to the worktree root | none: ST-01 step 4 or 5 |
| `owner` | the owner's name, lower-case ASCII words joined by dashes | git's `user.name`, else the local part of `user.email`, in that form (weigh-in B-31) |

**ST-13.** A missing file, a missing key, or a file that does not parse means `client`, so a forgotten config never publishes (pressure-test recommendation 11). A `client` repository never exports journals, pushes a ref, publishes a board, or projects items to GitHub (Blueprint design rule 10). Core enforces this: the export and publish paths ask Core for the class and get `client` unless the file says `own`.

weigh-in's `.kb/_ledger/config.json` `repo_class` becomes this key at the rename at the door. weigh-in defaults a missing config to `own`; the suite default is the stricter one.

## Projections

**ST-14.** A projection is disposable. It lives in the store or in `<git-common-dir>/redkiln/<namespace>.projections.sqlite3`, is rebuilt from the log (and from git, for Loop), and is never authoritative. Deleting a projection file loses nothing (Loop ADR-0109, the deleted-database test). A projection file follows ST-05 and keeps its own version; it is never stamped with the store's.

## Old ledgers

**ST-15.** The first run of a binary that finds its plugin's old ledger (`<git-common-dir>/weighin/ledger.sqlite3`, `<git-common-dir>/anneal/ledger.sqlite3`, `.git/temper/`) imports it into the store once, through the journal import path (JR-10, JR-11): each old event becomes an envelope of the renamed type, its `origin` is the old store's id and position (ID-02), and its trust is `asserted` (TR-06). A source that is not a happenstance store (temper's files under `.git/temper/`) takes as its store id the first 32 hex characters of the sha256 of its canonical path, and its line number as the position. The import is recorded as a `core.import_completed` event naming the source path, its store id and the count. A second run finds the source's origins already present and appends nothing.

**ST-16.** An import never deletes, truncates or rewrites its source. Old files stay where they are, because Cowork refuses deletes and because a user may still run the 1.x binary beside the 2.0 one for a while.
