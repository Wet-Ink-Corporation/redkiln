# R-SQLITE findings (2026-11-08)

**Result: TRUNCATE holds.** Eight concurrent writers, hooks included, on one Core store lost, duplicated or invented no event, on Linux, Windows and macOS. Nothing beside the store was deleted, and no WAL or shared-memory file ever appeared. **ST-05 can freeze as written at Move 2**, and Loop can give up WAL for the log (Blueprint call 1).

**What did not hold is fairness.** When one writer appends back to back, hooks miss their 2 s budget (ST-09) and their writes are refused, cleanly, because SQLite's rollback-journal locking lets a writer that never pauses take the lock again before a waiting process wakes. A 25 ms pause between that writer's transactions closes the gap on every OS (no hook refused, none over 2 s) while it still imports 4,500 to 16,000 events a second. The recommendation below makes that pause a Core rule, together with a fix to the opener that the run exposed.

Raw reports: [`results/`](results/) (`ci-ubuntu.md`, `ci-windows.md`, `ci-macos.md` from GitHub-hosted runners, run 42531881957 on PR #61; `local-linux.md` from a 4-core cloud container). Method: [README.md](README.md).

These are the second runs. The first (run 42518708347) had a harness fault the review caught: each long-lived writer's output went to a pipe that nothing read until the end, so a writer that printed enough stalled on a full pipe. It wrote fewer events than it should have, and it made the stress and import rows look worse than they are for hooks. The writers now log to files, and the run reports how many long-lived writers opened the store and ran (all of them, everywhere). Correctness passed in both runs; the load table below is from the second.

## Correctness: passed everywhere

| | Linux (CI) | Linux (local) | Windows | macOS |
|---|---|---|---|---|
| Cold start, 8 hooks create the store at once | 20/20 rounds | 20/20 | 20/20 | 20/20 |
| Mixed scenarios with every correctness check passing | 6/6 | 6/6 | 6/6 | 6/6 |

Over the four runs: about 5.6 million durable events, every one a writer reported as written found exactly once, and none that a writer reported failed. Positions ran 1..N with no gap. Every claimed subject had exactly one claim, with racers losing cleanly through `Appended::Conflict`. The watcher made about 468,000 one-millisecond polls and never saw the `-journal` missing once it existed, or a `-wal` or `-shm`. Afterwards the `-journal` was always empty (0 bytes), the header said rollback (1, 1), and `PRAGMA integrity_check` was `ok`. A busy refusal never wrote anything: no event of a refused append was found in any store.

## Load: where hooks miss 2 s

Hook durable path is `Store::open` under `Wait::Hook` plus one `append`, inside a fresh process. Counts are hooks whose open + append took over 2 s, out of all hooks in the 30 s scenario; p99 is the open + append time in ms. Open and append each have their own 2 s budget, so a hook can pass 2 s in total without being refused. Refused writes (`busy:`) in the failing rows, appends, claims and opens of every writer together: import 14, 13 and 27; stress 48, 26 and 18 (Linux, Windows, macOS). Windows also refused one append under busy.

| Scenario (8 writers) | Linux CI | Windows | macOS |
|---|---|---|---|
| paced: hooks every 100 ms, commands every 50 ms, racers every 20 ms | 0 of 1,132, p99 20 | 0 of 720, p99 235 | 0 of 571, p99 191 |
| hooks only: 8 hook lanes, no pause | 0 of 10,663, p99 164 | 0 of 1,859, p99 485 | 0 of 6,480, p99 608 |
| busy: commands every 10 ms, racers every 5 ms | 0 of 5,269, p99 147 | 4 of 773, p99 1,609 | 1 of 3,381, p99 306 |
| import, yielding: 256-event chunks every 25 ms | 0 of 1,671, p99 22 | 0 of 955, p99 441 | 0 of 817, p99 275 |
| **import: 256-event chunks back to back** | **14 of 392** | **23 of 336** | **39 of 167** |
| **stress: commands and racers with no pause** | **46 of 81** | **33 of 101** | **26 of 90** |

The Windows runner was the slowest of the three this run (it started 1,859 hooks in the hooks-only scenario, where Linux started 10,663), and its busy row is the only one outside the two failing shapes with a hook past 2 s. The two failing rows have one shape: a writer that commits and immediately begins again. Its own appends stay fast (p50 1 to 11 ms) while hooks queue behind it for seconds. That writer's own worst append also reached 2.7 to 11 s, at or past its 10 s budget under stress, because it waits like everyone else whenever it does lose the lock.

**Why.** Under a rollback journal SQLite has no queue: a process that finds the lock taken sleeps in its busy handler with growing back-off (up to 100 ms a sleep), then tries again. A writer that releases and re-takes the lock within microseconds wins almost every time against a sleeper. Raising the timeout does not help, since the sleeper is not behind in a queue; it simply keeps losing. WAL would not remove it either: WAL still has one writer lock with the same busy handler, and only readers stop waiting.

## A second finding: the opener can wait several budgets

Under the stress mix the hook's *open* alone reached 2.1 s (Linux), 1.6 s (Windows) and 2.1 s (macOS), past its 2 s budget, and some opens failed with `another process is opening the store; timed out waiting for it` (`OpenError::LockBusy`). A long-lived command's open took up to 3.2 s with a 10 s budget. Under hooks only on macOS, open p99 was 603 ms and its maximum 1.6 s while appends stayed small.

The cause is in `crates/core/redkiln-core/src/store/open.rs`: `open` takes the advisory `events.sqlite3.lock` and holds it through `open_locked`, which runs up to five statements in turn (`PRAGMA journal_mode`, `inspect`, `user_version`, `recorded_path`, and happenstance's setup in `SqliteEventStore::new`). Each statement gets the full busy timeout of its own, so one open may wait several times its budget. Every other opener waits on the lock file meanwhile and gives up with `LockBusy` after one budget, though no SQLite lock ever refused it. The lock exists so that two *first* opens never set each other's journal aside (ST-05), but it is taken on every open, including that of a stamped store in rollback mode at its recorded path, which needs no setting aside.

## Telemetry drops under concurrent hooks

With eight hooks and nothing else writing, the no-wait telemetry write (ST-09, EV-14) dropped 77 % (Linux CI), 74 % (Windows) and 8 % (macOS) of events; under paced load, 1 to 16 %. This is ST-09 working as designed, since the side store is one more single-writer file and telemetry never waits. But it means telemetry under load is a sample that leaves out the busiest moments, which matters to anything that reads latency from it.

## Recommendation for ST-05 and its neighbours

1. **ST-05: freeze as written.** TRUNCATE, the opener's rules and the no-delete rule held on every OS. Replace the paragraph that says R-SQLITE has not run with a pointer to this result.
2. **Blueprint call 1: Loop gives up WAL for the log**, under the rule in 3. Loop's projection file can stay WAL (it is only Loop's, ST-14).
3. **ST-09: add a give-way rule, enforced by Core rather than by each plugin.** A `Store` handle whose previous append wrote more than one event, or held the lock longer than 5 ms, waits 25 ms before its next append; the import paths (JR-10, ST-15) write in chunks of at most happenstance's 256 and give way between them. Measured: with the 25 ms pause no hook was refused or passed 2 s on any OS, at p99 22 ms (Linux), 275 ms (macOS) and 441 ms (Windows, the slow runner), while the importer still wrote 4,500 to 16,000 events a second. A hook makes one append and exits, so it is never slowed. The numbers are this spike's; Move 2 can tune them, and re-running this workflow on the Core change shows whether they hold.
4. **Opener (Core, Move 2): one deadline for the whole open, and no lock where none is needed.** Give the open a single deadline that every statement in it shares, so an open waits at most its budget. Take the open lock only when the header shows a missing, empty, unstamped or WAL file, or a `-wal`/`-journal` to set aside, and not for a stamped rollback store at its recorded path. Both are inside `store/open.rs`, need no API change, and keep ST-05's guarantees (the creation race is the case the lock is for).
5. **EV-14: say that telemetry is a sample.** Readers report the drop count beside any figure built from telemetry and never treat it as complete. No behaviour change.

Items 1, 2 and 5 are text. Items 3 and 4 are Core work for Move 2, before the freeze. Item 3 is a SPEC amendment that binds every plugin: Ryan chose it on 2026-11-08, and ST-09 now carries the rule (`docs/decisions.md`).

## Not covered

- Network file systems and Cowork's connected folders: the `libexec/` in Cowork spike covers Cowork; Core refuses nothing on a network file system today, and SQLite's locking there is unreliable under any journal mode.
- More than eight writers, or writers across machines (CI writes journals, never the store).
- Real plugin binaries: the hook here runs Core's own open, append and telemetry calls in a process of its own, which is the path every `rk-*` hook will take, but not their other work.
- WAL, which ST-05 forbids, was not run for comparison.
- Power loss and crash recovery mid-transaction.
