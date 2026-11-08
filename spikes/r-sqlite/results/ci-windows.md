# R-SQLITE run: windows x86_64

Correctness: **PASS** (no event lost, duplicated or invented; no file deleted beside the store; no WAL; integrity ok). Busy refusals are load findings, reported per scenario.

## Cold start: 8 hooks open a missing store at once

20 of 20 rounds passed every check.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| open (creates or waits) | 160 | 106.3 | 187.9 | 209.4 | 209.6 |
| append | 160 | 0.8 | 1.0 | 8.1 | 8.5 |

## busy: 4 hook lanes, 2 commands (batch 5) every 10 ms, 2 racers every 5 ms (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| every open and write succeeded, or lost a claim cleanly | pass |  |
| positions run 1..N with no gap | pass | N = 10362 |
| every reported append is stored exactly once, and nothing else | pass | 9318 reported, 9318 stored, 0 missing, 0 duplicated, 0 stored but reported failed |
| conditioned appends: one winner per subject | pass | 1044 won, 585 lost, 1044 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 17685 polls: -journal present on 17685, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

1923 hook processes, 1479 command appends, 1629 claims; 10362 durable events (~345/s). Telemetry: 1425 written, 498 dropped on a busy lock.

Hook durable path (open + append) over 250 ms: 42, over 1 s: 6, over 2 s: 0.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 1923 | 2.7 | 99.0 | 136.8 | 260.3 |
| hook append | 1923 | 1.4 | 123.4 | 367.3 | 1784.9 |
| hook open + append | 1923 | 22.4 | 174.2 | 397.3 | 1807.8 |
| hook telemetry | 1923 | 2.3 | 28.3 | 44.9 | 77.0 |
| hook process, spawn to exit | 1923 | 32.8 | 187.2 | 420.2 | 1817.8 |
| command open | 4 | 57.9 | 205.3 | 205.3 | 205.3 |
| command append (batch) | 1479 | 1.8 | 118.1 | 295.3 | 1265.9 |
| conditioned claim | 1629 | 1.2 | 139.7 | 430.0 | 1208.7 |

## import: 6 hook lanes every 100 ms, 2 commands appending 256-event chunks back to back (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| every open and write succeeded, or lost a claim cleanly | **busy** | 7 x err:append__appending_to_the_store__the_store_was_busy_and_refused_the_append_before_writing_anything__SQLite_failed__database_is_locked__database_is_locked__Error_code_5__database_is_locked |
| positions run 1..N with no gap | pass | N = 1125122 |
| every reported append is stored exactly once, and nothing else | pass | 1125122 reported, 1125122 stored, 0 missing, 0 duplicated, 0 stored but reported failed |
| conditioned appends: one winner per subject | pass | 0 won, 0 lost, 0 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 19540 polls: -journal present on 19540, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

521 hook processes, 4393 command appends, 0 claims; 1125122 durable events (~37504/s). Telemetry: 478 written, 43 dropped on a busy lock.

Hook durable path (open + append) over 250 ms: 117, over 1 s: 32, over 2 s: 9.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 521 | 0.9 | 30.1 | 43.5 | 64.4 |
| hook append | 521 | 45.5 | 1172.1 | 2308.9 | 2393.6 |
| hook open + append | 521 | 48.6 | 1176.8 | 2309.6 | 2415.0 |
| hook telemetry | 521 | 2.3 | 4.7 | 29.8 | 50.3 |
| hook process, spawn to exit | 521 | 58.7 | 1189.3 | 2318.6 | 2424.3 |
| command open | 2 | 1.2 | 21.2 | 21.2 | 21.2 |
| command append (batch) | 4393 | 5.2 | 11.4 | 117.0 | 3178.9 |
| conditioned claim | 0 | - | - | - | - |

## import, yielding: as import, with 25 ms between chunks (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| every open and write succeeded, or lost a claim cleanly | pass |  |
| positions run 1..N with no gap | pass | N = 427188 |
| every reported append is stored exactly once, and nothing else | pass | 427188 reported, 427188 stored, 0 missing, 0 duplicated, 0 stored but reported failed |
| conditioned appends: one winner per subject | pass | 0 won, 0 lost, 0 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 19254 polls: -journal present on 19254, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

1460 hook processes, 1663 command appends, 0 claims; 427188 durable events (~14239/s). Telemetry: 1335 written, 125 dropped on a busy lock.

Hook durable path (open + append) over 250 ms: 4, over 1 s: 0, over 2 s: 0.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 1460 | 0.7 | 21.4 | 58.7 | 82.6 |
| hook append | 1460 | 1.4 | 44.7 | 91.5 | 268.5 |
| hook open + append | 1460 | 2.5 | 56.1 | 115.4 | 282.0 |
| hook telemetry | 1460 | 2.2 | 3.3 | 30.3 | 88.6 |
| hook process, spawn to exit | 1460 | 11.5 | 68.4 | 130.4 | 287.7 |
| command open | 2 | 1.0 | 41.3 | 41.3 | 41.3 |
| command append (batch) | 1663 | 6.3 | 35.3 | 68.3 | 137.0 |
| conditioned claim | 0 | - | - | - | - |

## stress: 4 hook lanes, 2 commands (batch 5), 2 racers, no pauses (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| every open and write succeeded, or lost a claim cleanly | **busy** | 27 x err:append__appending_to_the_store__the_store_was_busy_and_refused_the_append_before_writing_anything__SQLite_failed__database_is_locked__database_is_locked__Error_code_5__database_is_locked |
| positions run 1..N with no gap | pass | N = 94903 |
| every reported append is stored exactly once, and nothing else | pass | 91734 reported, 91734 stored, 0 missing, 0 duplicated, 0 stored but reported failed |
| conditioned appends: one winner per subject | pass | 3169 won, 3129 lost, 3169 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 19206 polls: -journal present on 19206, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

191 hook processes, 18314 command appends, 6298 claims; 94903 durable events (~3163/s). Telemetry: 146 written, 45 dropped on a busy lock.

Hook durable path (open + append) over 250 ms: 67, over 1 s: 47, over 2 s: 32.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 191 | 1.1 | 175.2 | 543.2 | 929.1 |
| hook append | 191 | 8.7 | 2615.7 | 2720.9 | 2781.7 |
| hook open + append | 191 | 23.0 | 2668.4 | 2856.7 | 2926.0 |
| hook telemetry | 191 | 2.2 | 32.3 | 62.4 | 100.8 |
| hook process, spawn to exit | 191 | 36.0 | 2687.0 | 2907.0 | 2961.8 |
| command open | 4 | 428.1 | 809.4 | 809.4 | 809.4 |
| command append (batch) | 18314 | 1.2 | 1.5 | 2.8 | 7440.2 |
| conditioned claim | 6298 | 0.5 | 1.1 | 1.4 | 6822.1 |

## hooks only: 8 hook lanes, no pauses (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| every open and write succeeded, or lost a claim cleanly | pass |  |
| positions run 1..N with no gap | pass | N = 3433 |
| every reported append is stored exactly once, and nothing else | pass | 3433 reported, 3433 stored, 0 missing, 0 duplicated, 0 stored but reported failed |
| conditioned appends: one winner per subject | pass | 0 won, 0 lost, 0 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 16888 polls: -journal present on 16888, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

3433 hook processes, 0 command appends, 0 claims; 3433 durable events (~114/s). Telemetry: 1548 written, 1885 dropped on a busy lock.

Hook durable path (open + append) over 250 ms: 88, over 1 s: 0, over 2 s: 0.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 3433 | 30.1 | 186.6 | 326.8 | 615.3 |
| hook append | 3433 | 1.3 | 32.9 | 65.6 | 363.9 |
| hook open + append | 3433 | 41.5 | 201.3 | 329.3 | 616.7 |
| hook telemetry | 3433 | 0.1 | 24.5 | 42.0 | 76.6 |
| hook process, spawn to exit | 3433 | 49.4 | 213.1 | 337.9 | 640.8 |
| command open | 0 | - | - | - | - |
| command append (batch) | 0 | - | - | - | - |
| conditioned claim | 0 | - | - | - | - |

## paced: 4 hook lanes every 100 ms, 2 commands every 50 ms, 2 racers every 20 ms (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| every open and write succeeded, or lost a claim cleanly | pass |  |
| positions run 1..N with no gap | pass | N = 7283 |
| every reported append is stored exactly once, and nothing else | pass | 6120 reported, 6120 stored, 0 missing, 0 duplicated, 0 stored but reported failed |
| conditioned appends: one winner per subject | pass | 1163 won, 1163 lost, 1163 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 18948 polls: -journal present on 18948, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

1000 hook processes, 1024 command appends, 2326 claims; 7283 durable events (~242/s). Telemetry: 949 written, 51 dropped on a busy lock.

Hook durable path (open + append) over 250 ms: 0, over 1 s: 0, over 2 s: 0.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 1000 | 0.6 | 32.5 | 62.0 | 102.8 |
| hook append | 1000 | 1.2 | 16.1 | 46.3 | 92.9 |
| hook open + append | 1000 | 1.9 | 42.9 | 93.6 | 126.4 |
| hook telemetry | 1000 | 2.1 | 27.0 | 44.8 | 59.4 |
| hook process, spawn to exit | 1000 | 9.8 | 59.0 | 102.4 | 147.1 |
| command open | 4 | 21.7 | 62.3 | 62.3 | 62.3 |
| command append (batch) | 1024 | 1.4 | 45.6 | 76.0 | 107.9 |
| conditioned claim | 2326 | 1.0 | 34.9 | 76.0 | 152.3 |

