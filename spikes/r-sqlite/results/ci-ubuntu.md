# R-SQLITE run: linux x86_64

Correctness: **PASS** (no event lost, duplicated or invented; no file deleted beside the store; no WAL; integrity ok). Busy refusals are load findings, reported per scenario.

## Cold start: 8 hooks open a missing store at once

20 of 20 rounds passed every check.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| open (creates or waits) | 160 | 43.7 | 261.7 | 401.9 | 421.9 |
| append | 160 | 1.5 | 2.7 | 47.2 | 306.9 |

## busy: 4 hook lanes, 2 commands (batch 5) every 10 ms, 2 racers every 5 ms (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| no open or write failed other than on a busy lock | pass |  |
| no open or write refused on a busy lock past its timeout | pass |  |
| positions run 1..N with no gap | pass | N = 23215 |
| every reported append is stored exactly once, and nothing else | pass | 20894 reported, 20894 stored, 0 missing, 0 duplicated, 0 stored but reported failed, 0 unrecognised |
| conditioned appends: one winner per subject | pass | 2321 won, 1897 lost, 2321 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 28442 polls: -journal present on 28442, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

5269 hook processes, 3125 command appends, 4218 claims; 23215 durable events (~773/s). Telemetry: 2902 written, 2367 dropped on a busy lock.

Long-lived writers that opened the store and ran: 4 of 4.

Hook durable path (open + append) over 250 ms: 15, over 1 s: 0, over 2 s: 0.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 5269 | 1.5 | 60.6 | 101.9 | 462.2 |
| hook append | 5269 | 1.5 | 20.0 | 104.9 | 630.8 |
| hook open + append | 5269 | 5.5 | 76.6 | 146.8 | 651.3 |
| hook telemetry | 5269 | 2.2 | 3.1 | 4.3 | 9.6 |
| hook process, spawn to exit | 5269 | 9.0 | 80.5 | 150.2 | 652.5 |
| command open | 4 | 3.5 | 101.8 | 101.8 | 101.8 |
| command append (batch) | 3125 | 2.7 | 35.1 | 130.0 | 731.4 |
| conditioned claim | 4218 | 1.7 | 35.1 | 105.2 | 1229.6 |

## import: 6 hook lanes every 100 ms, 2 commands appending 256-event chunks back to back (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| no open or write failed other than on a busy lock | pass |  |
| no open or write refused on a busy lock past its timeout | **busy** | 14 x busy:append__appending_to_the_store__the_store_was_busy_and_refused_the_append_before_writing_anything__SQLite_failed__database_is_locked__database_is_locked__Error_code_5__database_is_locked |
| positions run 1..N with no gap | pass | N = 1129850 |
| every reported append is stored exactly once, and nothing else | pass | 1129850 reported, 1129850 stored, 0 missing, 0 duplicated, 0 stored but reported failed, 0 unrecognised |
| conditioned appends: one winner per subject | pass | 0 won, 0 lost, 0 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 28633 polls: -journal present on 28633, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

392 hook processes, 4412 command appends, 0 claims; 1129850 durable events (~37661/s). Telemetry: 352 written, 40 dropped on a busy lock.

Long-lived writers that opened the store and ran: 2 of 2.

Hook durable path (open + append) over 250 ms: 114, over 1 s: 60, over 2 s: 14.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 392 | 0.4 | 20.4 | 26.8 | 60.6 |
| hook append | 392 | 54.9 | 1831.6 | 2002.4 | 2002.5 |
| hook open + append | 392 | 55.6 | 1832.0 | 2005.9 | 2021.0 |
| hook telemetry | 392 | 2.1 | 3.0 | 5.9 | 7.3 |
| hook process, spawn to exit | 392 | 59.5 | 1835.8 | 2007.7 | 2025.2 |
| command open | 2 | 0.4 | 20.5 | 20.5 | 20.5 |
| command append (batch) | 4412 | 5.9 | 9.2 | 24.8 | 2737.5 |
| conditioned claim | 0 | - | - | - | - |

## import, yielding: as import, with 25 ms between chunks (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| no open or write failed other than on a busy lock | pass |  |
| no open or write refused on a busy lock past its timeout | pass |  |
| positions run 1..N with no gap | pass | N = 481671 |
| every reported append is stored exactly once, and nothing else | pass | 481671 reported, 481671 stored, 0 missing, 0 duplicated, 0 stored but reported failed, 0 unrecognised |
| conditioned appends: one winner per subject | pass | 0 won, 0 lost, 0 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 28589 polls: -journal present on 28589, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

1671 hook processes, 1875 command appends, 0 claims; 481671 durable events (~16055/s). Telemetry: 1624 written, 47 dropped on a busy lock.

Long-lived writers that opened the store and ran: 2 of 2.

Hook durable path (open + append) over 250 ms: 0, over 1 s: 0, over 2 s: 0.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 1671 | 0.3 | 3.5 | 20.4 | 40.5 |
| hook append | 1671 | 1.4 | 10.0 | 19.9 | 105.0 |
| hook open + append | 1671 | 2.0 | 19.9 | 21.9 | 105.3 |
| hook telemetry | 1671 | 2.1 | 2.9 | 4.9 | 8.2 |
| hook process, spawn to exit | 1671 | 5.7 | 21.4 | 25.3 | 108.9 |
| command open | 2 | 0.5 | 60.5 | 60.5 | 60.5 |
| command append (batch) | 1875 | 6.1 | 8.2 | 10.0 | 24.1 |
| conditioned claim | 0 | - | - | - | - |

## stress: 4 hook lanes, 2 commands (batch 5), 2 racers, no pauses (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| no open or write failed other than on a busy lock | pass |  |
| no open or write refused on a busy lock past its timeout | **busy** | 44 x busy:append__appending_to_the_store__the_store_was_busy_and_refused_the_append_before_writing_anything__SQLite_failed__database_is_locked__database_is_locked__Error_code_5__database_is_locked; 3 x busy:claim__appending_to_the_store__the_store_was_busy_and_refused_the_append_before_writing_anything__SQLite_failed__database_is_locked__database_is_locked__Error_code_5__database_is_locked; 1 x busy:open__opening_the_store_/home/runner/work/_temp/r-sqlite/mix-3/events.sqlite3__another_process_is_opening_the_store;_timed_out_waiting_for_it |
| positions run 1..N with no gap | pass | N = 76348 |
| every reported append is stored exactly once, and nothing else | pass | 70818 reported, 70818 stored, 0 missing, 0 duplicated, 0 stored but reported failed, 0 unrecognised |
| conditioned appends: one winner per subject | pass | 5530 won, 1117 lost, 5530 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 28617 polls: -journal present on 28617, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

81 hook processes, 14158 command appends, 6650 claims; 76348 durable events (~2544/s). Telemetry: 35 written, 45 dropped on a busy lock.

Long-lived writers that opened the store and ran: 4 of 4.

Hook durable path (open + append) over 250 ms: 62, over 1 s: 54, over 2 s: 46.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 81 | 33.7 | 842.0 | 2088.8 | 2088.8 |
| hook append | 81 | 2002.1 | 2002.3 | 2003.5 | 2003.5 |
| hook open + append | 81 | 2003.6 | 2619.7 | 4091.0 | 4091.0 |
| hook telemetry | 81 | 0.3 | 2.4 | 8.3 | 8.3 |
| hook process, spawn to exit | 81 | 2005.5 | 2621.6 | 4092.9 | 4092.9 |
| command open | 4 | 180.2 | 308.9 | 308.9 | 308.9 |
| command append (batch) | 14158 | 1.5 | 1.9 | 4.2 | 10008.8 |
| conditioned claim | 6650 | 1.2 | 1.5 | 3.2 | 10008.6 |

## hooks only: 8 hook lanes, no pauses (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| no open or write failed other than on a busy lock | pass |  |
| no open or write refused on a busy lock past its timeout | pass |  |
| positions run 1..N with no gap | pass | N = 10663 |
| every reported append is stored exactly once, and nothing else | pass | 10663 reported, 10663 stored, 0 missing, 0 duplicated, 0 stored but reported failed, 0 unrecognised |
| conditioned appends: one winner per subject | pass | 0 won, 0 lost, 0 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 28398 polls: -journal present on 28398, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

10663 hook processes, 0 command appends, 0 claims; 10663 durable events (~355/s). Telemetry: 2490 written, 8173 dropped on a busy lock.

Long-lived writers that opened the store and ran: 0 of 0.

Hook durable path (open + append) over 250 ms: 13, over 1 s: 0, over 2 s: 0.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 10663 | 1.5 | 83.8 | 161.1 | 322.7 |
| hook append | 10663 | 1.4 | 3.0 | 10.7 | 330.4 |
| hook open + append | 10663 | 3.7 | 86.5 | 163.6 | 411.2 |
| hook telemetry | 10663 | 0.0 | 2.6 | 3.3 | 9.2 |
| hook process, spawn to exit | 10663 | 5.6 | 90.2 | 164.8 | 412.4 |
| command open | 0 | - | - | - | - |
| command append (batch) | 0 | - | - | - | - |
| conditioned claim | 0 | - | - | - | - |

## paced: 4 hook lanes every 100 ms, 2 commands every 50 ms, 2 racers every 20 ms (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| no open or write failed other than on a busy lock | pass |  |
| no open or write refused on a busy lock past its timeout | pass |  |
| positions run 1..N with no gap | pass | N = 8245 |
| every reported append is stored exactly once, and nothing else | pass | 6877 reported, 6877 stored, 0 missing, 0 duplicated, 0 stored but reported failed, 0 unrecognised |
| conditioned appends: one winner per subject | pass | 1368 won, 1368 lost, 1368 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 28651 polls: -journal present on 28651, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

1132 hook processes, 1149 command appends, 2736 claims; 8245 durable events (~274/s). Telemetry: 1121 written, 11 dropped on a busy lock.

Long-lived writers that opened the store and ran: 4 of 4.

Hook durable path (open + append) over 250 ms: 0, over 1 s: 0, over 2 s: 0.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 1132 | 0.3 | 3.4 | 8.6 | 60.6 |
| hook append | 1132 | 1.4 | 4.3 | 9.5 | 19.9 |
| hook open + append | 1132 | 1.8 | 5.1 | 20.2 | 62.0 |
| hook telemetry | 1132 | 2.1 | 2.8 | 4.4 | 9.7 |
| hook process, spawn to exit | 1132 | 5.3 | 8.8 | 23.6 | 66.4 |
| command open | 4 | 0.6 | 20.4 | 20.4 | 20.4 |
| command append (batch) | 1149 | 1.6 | 4.7 | 9.8 | 20.0 |
| conditioned claim | 2736 | 1.3 | 4.9 | 8.3 | 18.4 |

