# R-SQLITE: eight writers on one TRUNCATE store

The phase 0 spike the migration plan schedules before Move 2 (`references/docs/migration-plan.md:158`, `docs/plan/README.md`): eight concurrent writers, hooks included, on one store under SQLite's TRUNCATE journal, on Linux and Windows; this run adds macOS. Core 1.0 freezes ST-05 at Move 2, and Loop's store (Move 4) follows from the result (Blueprint call 1).

Findings and the ST-05 recommendation: [FINDINGS.md](FINDINGS.md).

## What it runs

A standalone package with its own `[workspace]`: never a member of the suite's workspace, never shipped, so it adds no crate, edge or dependency there. It links `redkiln-core` by path and calls Core's own `Store::open`, `append` and `record_telemetry`, unchanged. Every writer is its own process.

- **Cold start**, 20 rounds: eight hook processes open a store that does not exist yet, released at the same millisecond.
- **Mixed load**, each scenario on a fresh store for `--seconds` (30 by default), always eight writers:
  - a *hook lane* starts one short-lived hook process after another; each opens the store under the hook's 2 s wait, appends one durable event and records one telemetry event, then exits;
  - a *command* opens once under the 10 s wait and appends batches;
  - a *racer* makes conditioned appends (ST-10, `not_yet`), two of them claiming the same subjects in the same order.

| Scenario | Hooks | Commands | Racers |
|---|---|---|---|
| busy | 4, no pause | 2, batch 5 every 10 ms | 2, every 5 ms |
| import | 6, every 100 ms | 2, 256-event chunks back to back | 0 |
| import, yielding | 6, every 100 ms | 2, 256-event chunks every 25 ms | 0 |
| stress | 4, no pause | 2, batch 5, no pause | 2, no pause |
| hooks only | 8, no pause | 0 | 0 |
| paced | 4, every 100 ms | 2, batch 5 every 50 ms | 2, every 20 ms |

256 is happenstance's (0.4.5 and 0.6.0) `MAX_EVENTS_PER_BATCH`, so an import already writes in chunks.

After each scenario it checks, as correctness: every writer reported every operation; positions run 1..N; every append a writer reported is stored exactly once and nothing else is; each claimed subject has one claim, and the store holds exactly the claims reported won; afterwards an empty `-journal`, no `-wal` or `-shm`, a rollback header; a watcher polling every millisecond never saw the `-journal` disappear or a `-wal`/`-shm` appear; `PRAGMA integrity_check` is `ok`. A write refused on a busy lock past its timeout (happenstance's `AppendError::Busy`, `OpenError::LockBusy`, SQLite's busy or locked code while opening) is a load finding, reported as `busy:` with its latency, not a failed run; any other failure, a writer that exits non-zero, or one that prints too few reports fails the run.

## Running it

```sh
cd spikes/r-sqlite
cargo build --release --locked
./target/release/r-sqlite run --dir /tmp/r-sqlite --seconds 30 --rounds 20 --report report.md
```

`--dir` must not hold an earlier run. The exit code is 1 when a correctness check failed. CI runs it on Linux, Windows and macOS through `.github/workflows/r-sqlite.yml`, by hand or on a pull request that touches this folder or Core's store; it is advisory, outside `ci / ok`.
