# R-SQLITE run: linux x86_64

Correctness: **PASS** (no event lost, duplicated or invented; no file deleted beside the store; no WAL; integrity ok). Busy refusals are load findings, reported per scenario.

## Cold start: 8 hooks open a missing store at once

20 of 20 rounds passed every check.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| open (creates or waits) | 160 | 43.7 | 101.5 | 141.2 | 141.5 |
| append | 160 | 2.0 | 3.0 | 6.2 | 6.3 |

## busy: 4 hook lanes, 2 commands (batch 5) every 10 ms, 2 racers every 5 ms (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| no open or write failed other than on a busy lock | pass |  |
| no open or write refused on a busy lock past its timeout | pass |  |
| positions run 1..N with no gap | pass | N = 20075 |
| every reported append is stored exactly once, and nothing else | pass | 17946 reported, 17946 stored, 0 missing, 0 duplicated, 0 stored but reported failed, 0 unrecognised |
| conditioned appends: one winner per subject | pass | 2129 won, 1525 lost, 2129 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 27354 polls: -journal present on 27354, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

3141 hook processes, 2961 command appends, 3654 claims; 20075 durable events (~669/s). Telemetry: 1567 written, 1574 dropped on a busy lock.

Long-lived writers that opened the store and ran: 4 of 4.

Hook durable path (open + append) over 250 ms: 33, over 1 s: 4, over 2 s: 0.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 3141 | 8.6 | 107.3 | 201.5 | 1075.3 |
| hook append | 3141 | 2.1 | 20.7 | 105.6 | 833.3 |
| hook open + append | 3141 | 13.3 | 124.9 | 259.8 | 1108.8 |
| hook telemetry | 3141 | 1.3 | 4.2 | 5.2 | 15.3 |
| hook process, spawn to exit | 3141 | 17.8 | 127.9 | 265.0 | 1110.4 |
| command open | 4 | 20.5 | 60.7 | 60.7 | 60.7 |
| command append (batch) | 2961 | 3.3 | 36.5 | 107.5 | 633.3 |
| conditioned claim | 3654 | 2.8 | 53.7 | 130.7 | 930.5 |

## import: 6 hook lanes every 100 ms, 2 commands appending 256-event chunks back to back (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| no open or write failed other than on a busy lock | pass |  |
| no open or write refused on a busy lock past its timeout | **busy** | 31 x busy:append__appending_to_the_store__the_store_was_busy_and_refused_the_append_before_writing_anything__SQLite_failed__database_is_locked__database_is_locked__Error_code_5__database_is_locked |
| positions run 1..N with no gap | pass | N = 1071341 |
| every reported append is stored exactly once, and nothing else | pass | 1071341 reported, 1071341 stored, 0 missing, 0 duplicated, 0 stored but reported failed, 0 unrecognised |
| conditioned appends: one winner per subject | pass | 0 won, 0 lost, 0 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 27645 polls: -journal present on 27645, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

268 hook processes, 4184 command appends, 0 claims; 1071341 durable events (~35711/s). Telemetry: 233 written, 35 dropped on a busy lock.

Long-lived writers that opened the store and ran: 2 of 2.

Hook durable path (open + append) over 250 ms: 120, over 1 s: 61, over 2 s: 32.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 268 | 0.4 | 18.8 | 34.2 | 54.1 |
| hook append | 268 | 181.7 | 2003.9 | 2004.5 | 2006.7 |
| hook open + append | 268 | 185.9 | 2004.8 | 2007.5 | 2012.7 |
| hook telemetry | 268 | 2.6 | 4.0 | 6.8 | 12.6 |
| hook process, spawn to exit | 268 | 191.1 | 2009.9 | 2013.9 | 2017.8 |
| command open | 2 | 0.4 | 61.9 | 61.9 | 61.9 |
| command append (batch) | 4184 | 6.3 | 9.8 | 16.0 | 4142.6 |
| conditioned claim | 0 | - | - | - | - |

## import, yielding: as import, with 25 ms between chunks (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| no open or write failed other than on a busy lock | pass |  |
| no open or write refused on a busy lock past its timeout | pass |  |
| positions run 1..N with no gap | pass | N = 431602 |
| every reported append is stored exactly once, and nothing else | pass | 431602 reported, 431602 stored, 0 missing, 0 duplicated, 0 stored but reported failed, 0 unrecognised |
| conditioned appends: one winner per subject | pass | 0 won, 0 lost, 0 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 26124 polls: -journal present on 26124, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

1522 hook processes, 1680 command appends, 0 claims; 431602 durable events (~14386/s). Telemetry: 1351 written, 171 dropped on a busy lock.

Long-lived writers that opened the store and ran: 2 of 2.

Hook durable path (open + append) over 250 ms: 1, over 1 s: 0, over 2 s: 0.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 1522 | 0.3 | 20.3 | 60.7 | 185.1 |
| hook append | 1522 | 2.8 | 24.2 | 82.7 | 336.4 |
| hook open + append | 1522 | 5.3 | 45.4 | 108.0 | 336.7 |
| hook telemetry | 1522 | 3.2 | 6.9 | 9.4 | 84.9 |
| hook process, spawn to exit | 1522 | 11.1 | 51.8 | 115.5 | 338.4 |
| command open | 2 | 0.3 | 0.3 | 0.3 | 0.3 |
| command append (batch) | 1680 | 8.0 | 16.0 | 30.5 | 536.2 |
| conditioned claim | 0 | - | - | - | - |

## stress: 4 hook lanes, 2 commands (batch 5), 2 racers, no pauses (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| no open or write failed other than on a busy lock | pass |  |
| no open or write refused on a busy lock past its timeout | **busy** | 28 x busy:append__appending_to_the_store__the_store_was_busy_and_refused_the_append_before_writing_anything__SQLite_failed__database_is_locked__database_is_locked__Error_code_5__database_is_locked; 2 x busy:claim__appending_to_the_store__the_store_was_busy_and_refused_the_append_before_writing_anything__SQLite_failed__database_is_locked__database_is_locked__Error_code_5__database_is_locked; 1 x busy:open__opening_the_store_<dir>/mix-3/events.sqlite3__SQLite_failed_while_configuring_the_connection__database_is_locked__Error_code_5__database_is_locked; 1 x busy:open__opening_the_store_<dir>/mix-3/events.sqlite3__SQLite_failed_while_keeping_the_store's_identity__database_is_locked__Error_code_5__database_is_locked; 12 x busy:open__opening_the_store_<dir>/mix-3/events.sqlite3__another_process_is_opening_the_store;_timed_out_waiting_for_it |
| positions run 1..N with no gap | pass | N = 28554 |
| every reported append is stored exactly once, and nothing else | pass | 21827 reported, 21827 stored, 0 missing, 0 duplicated, 0 stored but reported failed, 0 unrecognised |
| conditioned appends: one winner per subject | pass | 6727 won, 306 lost, 6727 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 26992 polls: -journal present on 26992, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

55 hook processes, 4366 command appends, 7035 claims; 28554 durable events (~951/s). Telemetry: 10 written, 31 dropped on a busy lock.

Long-lived writers that opened the store and ran: 4 of 4.

Hook durable path (open + append) over 250 ms: 50, over 1 s: 47, over 2 s: 41.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 55 | 1111.2 | 2365.1 | 3241.4 | 3241.4 |
| hook append | 55 | 631.4 | 2006.9 | 2008.7 | 2008.7 |
| hook open + append | 55 | 2026.9 | 4017.3 | 5124.5 | 5124.5 |
| hook telemetry | 55 | 0.0 | 4.3 | 9.0 | 9.0 |
| hook process, spawn to exit | 55 | 2029.1 | 4019.2 | 5127.3 | 5127.3 |
| command open | 4 | 2.3 | 2776.4 | 2776.4 | 2776.4 |
| command append (batch) | 4366 | 2.3 | 3.5 | 5.9 | 10026.7 |
| conditioned claim | 7035 | 2.0 | 4.9 | 20.3 | 10014.0 |

## hooks only: 8 hook lanes, no pauses (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| no open or write failed other than on a busy lock | pass |  |
| no open or write refused on a busy lock past its timeout | pass |  |
| positions run 1..N with no gap | pass | N = 7286 |
| every reported append is stored exactly once, and nothing else | pass | 7286 reported, 7286 stored, 0 missing, 0 duplicated, 0 stored but reported failed, 0 unrecognised |
| conditioned appends: one winner per subject | pass | 0 won, 0 lost, 0 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 27215 polls: -journal present on 27215, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

7286 hook processes, 0 command appends, 0 claims; 7286 durable events (~242/s). Telemetry: 1636 written, 5650 dropped on a busy lock.

Long-lived writers that opened the store and ran: 0 of 0.

Hook durable path (open + append) over 250 ms: 6, over 1 s: 0, over 2 s: 0.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 7286 | 3.8 | 121.0 | 183.8 | 285.7 |
| hook append | 7286 | 2.2 | 4.0 | 10.4 | 231.5 |
| hook open + append | 7286 | 7.3 | 123.3 | 186.6 | 287.7 |
| hook telemetry | 7286 | 0.0 | 3.8 | 4.8 | 62.5 |
| hook process, spawn to exit | 7286 | 12.1 | 125.2 | 188.3 | 289.2 |
| command open | 0 | - | - | - | - |
| command append (batch) | 0 | - | - | - | - |
| conditioned claim | 0 | - | - | - | - |

## paced: 4 hook lanes every 100 ms, 2 commands every 50 ms, 2 racers every 20 ms (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| no open or write failed other than on a busy lock | pass |  |
| no open or write refused on a busy lock past its timeout | pass |  |
| positions run 1..N with no gap | pass | N = 7949 |
| every reported append is stored exactly once, and nothing else | pass | 6665 reported, 6665 stored, 0 missing, 0 duplicated, 0 stored but reported failed, 0 unrecognised |
| conditioned appends: one winner per subject | pass | 1284 won, 1284 lost, 1284 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 27346 polls: -journal present on 27346, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

1100 hook processes, 1113 command appends, 2568 claims; 7949 durable events (~264/s). Telemetry: 1057 written, 43 dropped on a busy lock.

Long-lived writers that opened the store and ran: 4 of 4.

Hook durable path (open + append) over 250 ms: 0, over 1 s: 0, over 2 s: 0.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 1100 | 0.3 | 3.7 | 20.5 | 66.1 |
| hook append | 1100 | 2.3 | 3.9 | 10.3 | 36.0 |
| hook open + append | 1100 | 2.8 | 7.5 | 23.3 | 67.7 |
| hook telemetry | 1100 | 3.2 | 4.6 | 5.6 | 9.5 |
| hook process, spawn to exit | 1100 | 8.2 | 13.7 | 28.8 | 71.7 |
| command open | 4 | 3.7 | 29.0 | 29.0 | 29.0 |
| command append (batch) | 1113 | 2.9 | 7.6 | 12.2 | 37.4 |
| conditioned claim | 2568 | 2.5 | 8.5 | 18.7 | 79.0 |

