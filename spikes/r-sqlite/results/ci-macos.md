# R-SQLITE run: macos aarch64

Correctness: **PASS** (no event lost, duplicated or invented; no file deleted beside the store; no WAL; integrity ok). Busy refusals are load findings, reported per scenario.

## Cold start: 8 hooks open a missing store at once

20 of 20 rounds passed every check.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| open (creates or waits) | 160 | 77.0 | 244.6 | 312.5 | 316.7 |
| append | 160 | 1.1 | 25.1 | 49.2 | 72.2 |

## busy: 4 hook lanes, 2 commands (batch 5) every 10 ms, 2 racers every 5 ms (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| every open and write succeeded, or lost a claim cleanly | pass |  |
| positions run 1..N with no gap | pass | N = 8151 |
| every reported append is stored exactly once, and nothing else | pass | 7292 reported, 7292 stored, 0 missing, 0 duplicated, 0 stored but reported failed |
| conditioned appends: one winner per subject | pass | 859 won, 782 lost, 859 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 6212 polls: -journal present on 6212, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

2677 hook processes, 923 command appends, 1641 claims; 8151 durable events (~271/s). Telemetry: 2181 written, 496 dropped on a busy lock.

Hook durable path (open + append) over 250 ms: 79, over 1 s: 2, over 2 s: 0.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 2677 | 0.6 | 151.0 | 318.4 | 719.6 |
| hook append | 2677 | 1.8 | 25.9 | 203.0 | 1629.4 |
| hook open + append | 2677 | 4.9 | 183.2 | 433.3 | 1725.8 |
| hook telemetry | 2677 | 1.7 | 3.7 | 5.8 | 142.7 |
| hook process, spawn to exit | 2677 | 11.4 | 188.8 | 442.1 | 1728.8 |
| command open | 4 | 1.8 | 253.8 | 253.8 | 253.8 |
| command append (batch) | 923 | 7.4 | 122.7 | 513.4 | 2010.8 |
| conditioned claim | 1641 | 4.7 | 78.6 | 333.2 | 1004.2 |

## import: 6 hook lanes every 100 ms, 2 commands appending 256-event chunks back to back (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| every open and write succeeded, or lost a claim cleanly | **busy** | 33 x err:append__appending_to_the_store__the_store_was_busy_and_refused_the_append_before_writing_anything__SQLite_failed__database_is_locked__database_is_locked__Error_code_5__database_is_locked |
| positions run 1..N with no gap | pass | N = 784727 |
| every reported append is stored exactly once, and nothing else | pass | 784727 reported, 784727 stored, 0 missing, 0 duplicated, 0 stored but reported failed |
| conditioned appends: one winner per subject | pass | 0 won, 0 lost, 0 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 6577 polls: -journal present on 6577, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

120 hook processes, 3065 command appends, 0 claims; 784727 durable events (~26157/s). Telemetry: 94 written, 26 dropped on a busy lock.

Hook durable path (open + append) over 250 ms: 82, over 1 s: 62, over 2 s: 44.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 120 | 5.6 | 306.5 | 473.7 | 822.5 |
| hook append | 120 | 1046.6 | 3010.7 | 3155.1 | 3232.5 |
| hook open + append | 120 | 1047.1 | 3036.4 | 3374.3 | 3419.1 |
| hook telemetry | 120 | 1.7 | 4.5 | 9.7 | 12.8 |
| hook process, spawn to exit | 120 | 1053.2 | 3043.9 | 3379.7 | 3424.7 |
| command open | 2 | 0.7 | 125.8 | 125.8 | 125.8 |
| command append (batch) | 3065 | 7.3 | 15.8 | 50.4 | 7017.4 |
| conditioned claim | 0 | - | - | - | - |

## import, yielding: as import, with 25 ms between chunks (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| every open and write succeeded, or lost a claim cleanly | pass |  |
| positions run 1..N with no gap | pass | N = 168593 |
| every reported append is stored exactly once, and nothing else | pass | 168593 reported, 168593 stored, 0 missing, 0 duplicated, 0 stored but reported failed |
| conditioned appends: one winner per subject | pass | 0 won, 0 lost, 0 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 4998 polls: -journal present on 4998, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

913 hook processes, 655 command appends, 0 claims; 168593 durable events (~5619/s). Telemetry: 806 written, 107 dropped on a busy lock.

Hook durable path (open + append) over 250 ms: 19, over 1 s: 0, over 2 s: 0.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 913 | 0.8 | 146.0 | 268.8 | 453.0 |
| hook append | 913 | 3.0 | 45.0 | 132.4 | 319.9 |
| hook open + append | 913 | 11.9 | 169.7 | 299.4 | 521.5 |
| hook telemetry | 913 | 1.5 | 4.5 | 13.1 | 128.7 |
| hook process, spawn to exit | 913 | 19.3 | 179.0 | 308.3 | 529.1 |
| command open | 2 | 0.5 | 15.9 | 15.9 | 15.9 |
| command append (batch) | 655 | 10.4 | 50.3 | 125.4 | 357.3 |
| conditioned claim | 0 | - | - | - | - |

## stress: 4 hook lanes, 2 commands (batch 5), 2 racers, no pauses (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| every open and write succeeded, or lost a claim cleanly | **busy** | 16 x err:append__appending_to_the_store__the_store_was_busy_and_refused_the_append_before_writing_anything__SQLite_failed__database_is_locked__database_is_locked__Error_code_5__database_is_locked; 3 x err:open__opening_the_store_$RUNNER_TEMP/r-sqlite/mix-3/events.sqlite3__another_process_is_opening_the_store;_timed_out_waiting_for_it |
| positions run 1..N with no gap | pass | N = 62252 |
| every reported append is stored exactly once, and nothing else | pass | 59163 reported, 59163 stored, 0 missing, 0 duplicated, 0 stored but reported failed |
| conditioned appends: one winner per subject | pass | 3089 won, 2268 lost, 3089 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 4045 polls: -journal present on 4045, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

72 hook processes, 11822 command appends, 5357 claims; 62252 durable events (~2075/s). Telemetry: 44 written, 25 dropped on a busy lock.

Hook durable path (open + append) over 250 ms: 35, over 1 s: 31, over 2 s: 28.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 72 | 10.1 | 2136.1 | 2658.1 | 2658.1 |
| hook append | 72 | 7.6 | 4361.8 | 4543.8 | 4543.8 |
| hook open + append | 72 | 66.0 | 5169.4 | 6671.0 | 6671.0 |
| hook telemetry | 72 | 1.6 | 9.7 | 27.8 | 27.8 |
| hook process, spawn to exit | 72 | 77.2 | 5176.1 | 6690.1 | 6690.1 |
| command open | 4 | 174.2 | 352.8 | 352.8 | 352.8 |
| command append (batch) | 11822 | 1.4 | 3.2 | 13.8 | 8991.5 |
| conditioned claim | 5357 | 0.8 | 1.9 | 4.9 | 12893.1 |

## hooks only: 8 hook lanes, no pauses (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| every open and write succeeded, or lost a claim cleanly | pass |  |
| positions run 1..N with no gap | pass | N = 4826 |
| every reported append is stored exactly once, and nothing else | pass | 4826 reported, 4826 stored, 0 missing, 0 duplicated, 0 stored but reported failed |
| conditioned appends: one winner per subject | pass | 0 won, 0 lost, 0 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 4062 polls: -journal present on 4062, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

4826 hook processes, 0 command appends, 0 claims; 4826 durable events (~160/s). Telemetry: 4248 written, 578 dropped on a busy lock.

Hook durable path (open + append) over 250 ms: 312, over 1 s: 9, over 2 s: 0.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 4826 | 0.4 | 314.4 | 703.5 | 1778.7 |
| hook append | 4826 | 1.2 | 2.6 | 25.1 | 122.3 |
| hook open + append | 4826 | 1.7 | 320.2 | 705.0 | 1780.1 |
| hook telemetry | 4826 | 1.6 | 2.8 | 4.7 | 83.6 |
| hook process, spawn to exit | 4826 | 6.6 | 325.8 | 714.4 | 1785.7 |
| command open | 0 | - | - | - | - |
| command append (batch) | 0 | - | - | - | - |
| conditioned claim | 0 | - | - | - | - |

## paced: 4 hook lanes every 100 ms, 2 commands every 50 ms, 2 racers every 20 ms (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| every open and write succeeded, or lost a claim cleanly | pass |  |
| positions run 1..N with no gap | pass | N = 2995 |
| every reported append is stored exactly once, and nothing else | pass | 2715 reported, 2715 stored, 0 missing, 0 duplicated, 0 stored but reported failed |
| conditioned appends: one winner per subject | pass | 280 won, 274 lost, 280 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 3711 polls: -journal present on 3711, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

540 hook processes, 435 command appends, 554 claims; 2995 durable events (~99/s). Telemetry: 508 written, 32 dropped on a busy lock.

Hook durable path (open + append) over 250 ms: 12, over 1 s: 1, over 2 s: 0.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 540 | 0.9 | 164.0 | 262.5 | 1198.4 |
| hook append | 540 | 1.8 | 24.6 | 45.5 | 96.7 |
| hook open + append | 540 | 3.0 | 167.7 | 304.0 | 1223.0 |
| hook telemetry | 540 | 1.7 | 4.6 | 50.7 | 298.7 |
| hook process, spawn to exit | 540 | 12.5 | 175.2 | 402.3 | 1284.8 |
| command open | 4 | 0.6 | 159.5 | 159.5 | 159.5 |
| command append (batch) | 435 | 7.4 | 54.6 | 211.4 | 725.4 |
| conditioned claim | 554 | 2.7 | 55.3 | 242.2 | 558.3 |

