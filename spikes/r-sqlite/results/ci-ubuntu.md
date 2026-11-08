# R-SQLITE run: linux x86_64

Correctness: **PASS** (no event lost, duplicated or invented; no file deleted beside the store; no WAL; integrity ok). Busy refusals are load findings, reported per scenario.

## Cold start: 8 hooks open a missing store at once

20 of 20 rounds passed every check.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| open (creates or waits) | 160 | 41.6 | 120.9 | 140.9 | 141.1 |
| append | 160 | 1.4 | 2.4 | 2.9 | 5.7 |

## busy: 4 hook lanes, 2 commands (batch 5) every 10 ms, 2 racers every 5 ms (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| every open and write succeeded, or lost a claim cleanly | pass |  |
| positions run 1..N with no gap | pass | N = 24108 |
| every reported append is stored exactly once, and nothing else | pass | 21567 reported, 21567 stored, 0 missing, 0 duplicated, 0 stored but reported failed |
| conditioned appends: one winner per subject | pass | 2541 won, 1930 lost, 2541 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 28439 polls: -journal present on 28439, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

5427 hook processes, 3228 command appends, 4471 claims; 24108 durable events (~803/s). Telemetry: 3040 written, 2387 dropped on a busy lock.

Hook durable path (open + append) over 250 ms: 13, over 1 s: 0, over 2 s: 0.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 5427 | 1.5 | 61.7 | 100.8 | 222.3 |
| hook append | 5427 | 1.4 | 19.8 | 105.0 | 831.0 |
| hook open + append | 5427 | 5.1 | 71.6 | 130.4 | 872.3 |
| hook telemetry | 5427 | 2.2 | 3.1 | 3.9 | 14.0 |
| hook process, spawn to exit | 5427 | 8.7 | 75.3 | 132.0 | 873.6 |
| command open | 4 | 3.6 | 20.4 | 20.4 | 20.4 |
| command append (batch) | 3228 | 2.7 | 34.9 | 105.4 | 631.4 |
| conditioned claim | 4471 | 1.6 | 34.6 | 105.2 | 1429.9 |

## import: 6 hook lanes every 100 ms, 2 commands appending 256-event chunks back to back (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| every open and write succeeded, or lost a claim cleanly | **busy** | 19 x err:append__appending_to_the_store__the_store_was_busy_and_refused_the_append_before_writing_anything__SQLite_failed__database_is_locked__database_is_locked__Error_code_5__database_is_locked |
| positions run 1..N with no gap | pass | N = 1149797 |
| every reported append is stored exactly once, and nothing else | pass | 1149797 reported, 1149797 stored, 0 missing, 0 duplicated, 0 stored but reported failed |
| conditioned appends: one winner per subject | pass | 0 won, 0 lost, 0 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 28635 polls: -journal present on 28635, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

376 hook processes, 4490 command appends, 0 claims; 1149797 durable events (~38326/s). Telemetry: 337 written, 39 dropped on a busy lock.

Hook durable path (open + append) over 250 ms: 124, over 1 s: 50, over 2 s: 22.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 376 | 0.4 | 20.5 | 40.6 | 61.8 |
| hook append | 376 | 80.1 | 2002.3 | 2002.6 | 2003.9 |
| hook open + append | 376 | 81.5 | 2002.8 | 2022.9 | 2026.0 |
| hook telemetry | 376 | 2.1 | 2.7 | 3.8 | 6.4 |
| hook process, spawn to exit | 376 | 84.9 | 2006.6 | 2024.3 | 2031.1 |
| command open | 2 | 0.5 | 20.4 | 20.4 | 20.4 |
| command append (batch) | 4490 | 5.9 | 9.0 | 23.9 | 3438.1 |
| conditioned claim | 0 | - | - | - | - |

## import, yielding: as import, with 25 ms between chunks (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| every open and write succeeded, or lost a claim cleanly | pass |  |
| positions run 1..N with no gap | pass | N = 482702 |
| every reported append is stored exactly once, and nothing else | pass | 482702 reported, 482702 stored, 0 missing, 0 duplicated, 0 stored but reported failed |
| conditioned appends: one winner per subject | pass | 0 won, 0 lost, 0 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 28577 polls: -journal present on 28577, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

1678 hook processes, 1879 command appends, 0 claims; 482702 durable events (~16090/s). Telemetry: 1657 written, 21 dropped on a busy lock.

Hook durable path (open + append) over 250 ms: 0, over 1 s: 0, over 2 s: 0.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 1678 | 0.3 | 3.5 | 20.3 | 43.6 |
| hook append | 1678 | 1.4 | 9.8 | 19.8 | 21.1 |
| hook open + append | 1678 | 1.9 | 13.2 | 21.9 | 45.0 |
| hook telemetry | 1678 | 2.1 | 2.8 | 4.1 | 7.4 |
| hook process, spawn to exit | 1678 | 5.5 | 16.8 | 24.6 | 48.2 |
| command open | 2 | 0.6 | 20.4 | 20.4 | 20.4 |
| command append (batch) | 1879 | 6.0 | 7.9 | 9.8 | 18.2 |
| conditioned claim | 0 | - | - | - | - |

## stress: 4 hook lanes, 2 commands (batch 5), 2 racers, no pauses (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| every open and write succeeded, or lost a claim cleanly | **busy** | 38 x err:append__appending_to_the_store__the_store_was_busy_and_refused_the_append_before_writing_anything__SQLite_failed__database_is_locked__database_is_locked__Error_code_5__database_is_locked; 3 x err:appending_to_the_store__the_store_was_busy_and_refused_the_append_before_writing_anything__SQLite_failed__database_is_locked__database_is_locked__Error_code_5__database_is_locked |
| positions run 1..N with no gap | pass | N = 85606 |
| every reported append is stored exactly once, and nothing else | pass | 82587 reported, 82587 stored, 0 missing, 0 duplicated, 0 stored but reported failed |
| conditioned appends: one winner per subject | pass | 3019 won, 230 lost, 3019 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 28500 polls: -journal present on 28500, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

75 hook processes, 16510 command appends, 3252 claims; 85606 durable events (~2853/s). Telemetry: 32 written, 43 dropped on a busy lock.

Hook durable path (open + append) over 250 ms: 62, over 1 s: 52, over 2 s: 41.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 75 | 45.1 | 1670.1 | 1831.5 | 1831.5 |
| hook append | 75 | 2002.1 | 2002.4 | 2002.7 | 2002.7 |
| hook open + append | 75 | 2002.7 | 3457.8 | 3833.8 | 3833.8 |
| hook telemetry | 75 | 0.2 | 2.4 | 7.4 | 7.4 |
| hook process, spawn to exit | 75 | 2004.4 | 3459.5 | 3835.6 | 3835.6 |
| command open | 4 | 911.5 | 1026.0 | 1026.0 | 1026.0 |
| command append (batch) | 16510 | 1.4 | 1.9 | 3.2 | 9337.4 |
| conditioned claim | 3252 | 1.1 | 1.5 | 2.7 | 10009.0 |

## hooks only: 8 hook lanes, no pauses (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| every open and write succeeded, or lost a claim cleanly | pass |  |
| positions run 1..N with no gap | pass | N = 11014 |
| every reported append is stored exactly once, and nothing else | pass | 11014 reported, 11014 stored, 0 missing, 0 duplicated, 0 stored but reported failed |
| conditioned appends: one winner per subject | pass | 0 won, 0 lost, 0 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 28395 polls: -journal present on 28395, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

11014 hook processes, 0 command appends, 0 claims; 11014 durable events (~367/s). Telemetry: 2554 written, 8460 dropped on a busy lock.

Hook durable path (open + append) over 250 ms: 9, over 1 s: 0, over 2 s: 0.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 11014 | 1.5 | 81.9 | 144.3 | 381.9 |
| hook append | 11014 | 1.4 | 2.7 | 9.8 | 430.3 |
| hook open + append | 11014 | 3.3 | 85.0 | 162.2 | 430.6 |
| hook telemetry | 11014 | 0.0 | 2.6 | 3.2 | 7.0 |
| hook process, spawn to exit | 11014 | 5.5 | 86.6 | 163.8 | 431.8 |
| command open | 0 | - | - | - | - |
| command append (batch) | 0 | - | - | - | - |
| conditioned claim | 0 | - | - | - | - |

## paced: 4 hook lanes every 100 ms, 2 commands every 50 ms, 2 racers every 20 ms (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| every open and write succeeded, or lost a claim cleanly | pass |  |
| positions run 1..N with no gap | pass | N = 8239 |
| every reported append is stored exactly once, and nothing else | pass | 6872 reported, 6872 stored, 0 missing, 0 duplicated, 0 stored but reported failed |
| conditioned appends: one winner per subject | pass | 1367 won, 1367 lost, 1367 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 28618 polls: -journal present on 28618, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

1127 hook processes, 1149 command appends, 2734 claims; 8239 durable events (~274/s). Telemetry: 1112 written, 15 dropped on a busy lock.

Hook durable path (open + append) over 250 ms: 0, over 1 s: 0, over 2 s: 0.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 1127 | 0.3 | 3.5 | 20.4 | 80.7 |
| hook append | 1127 | 1.4 | 4.4 | 9.7 | 20.0 |
| hook open + append | 1127 | 1.8 | 5.2 | 21.8 | 81.9 |
| hook telemetry | 1127 | 2.1 | 2.8 | 3.7 | 12.6 |
| hook process, spawn to exit | 1127 | 5.6 | 9.2 | 25.6 | 85.3 |
| command open | 4 | 3.6 | 40.5 | 40.5 | 40.5 |
| command append (batch) | 1149 | 1.7 | 4.6 | 9.6 | 35.0 |
| conditioned claim | 2734 | 1.4 | 4.6 | 8.3 | 33.5 |

