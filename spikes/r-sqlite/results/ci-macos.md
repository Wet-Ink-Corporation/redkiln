# R-SQLITE run: macos aarch64

Correctness: **PASS** (no event lost, duplicated or invented; no file deleted beside the store; no WAL; integrity ok). Busy refusals are load findings, reported per scenario.

## Cold start: 8 hooks open a missing store at once

20 of 20 rounds passed every check.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| open (creates or waits) | 160 | 60.1 | 218.2 | 273.6 | 308.6 |
| append | 160 | 0.9 | 8.6 | 28.7 | 42.9 |

## busy: 4 hook lanes, 2 commands (batch 5) every 10 ms, 2 racers every 5 ms (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| no open or write failed other than on a busy lock | pass |  |
| no open or write refused on a busy lock past its timeout | pass |  |
| positions run 1..N with no gap | pass | N = 9524 |
| every reported append is stored exactly once, and nothing else | pass | 8561 reported, 8561 stored, 0 missing, 0 duplicated, 0 stored but reported failed, 0 unrecognised |
| conditioned appends: one winner per subject | pass | 963 won, 737 lost, 963 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 6382 polls: -journal present on 6382, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

3381 hook processes, 1036 command appends, 1700 claims; 9524 durable events (~317/s). Telemetry: 2856 written, 525 dropped on a busy lock.

Long-lived writers that opened the store and ran: 4 of 4.

Hook durable path (open + append) over 250 ms: 61, over 1 s: 2, over 2 s: 1.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 3381 | 0.4 | 128.9 | 253.9 | 598.4 |
| hook append | 3381 | 1.4 | 17.5 | 90.3 | 2035.9 |
| hook open + append | 3381 | 2.3 | 146.0 | 305.5 | 2036.3 |
| hook telemetry | 3381 | 1.5 | 3.4 | 5.6 | 63.9 |
| hook process, spawn to exit | 3381 | 8.3 | 152.4 | 309.4 | 2041.1 |
| command open | 4 | 6.9 | 48.7 | 48.7 | 48.7 |
| command append (batch) | 1036 | 7.2 | 110.3 | 326.4 | 828.6 |
| conditioned claim | 1700 | 4.9 | 76.5 | 218.0 | 2165.7 |

## import: 6 hook lanes every 100 ms, 2 commands appending 256-event chunks back to back (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| no open or write failed other than on a busy lock | pass |  |
| no open or write refused on a busy lock past its timeout | **busy** | 27 x busy:append__appending_to_the_store__the_store_was_busy_and_refused_the_append_before_writing_anything__SQLite_failed__database_is_locked__database_is_locked__Error_code_5__database_is_locked |
| positions run 1..N with no gap | pass | N = 1285004 |
| every reported append is stored exactly once, and nothing else | pass | 1285004 reported, 1285004 stored, 0 missing, 0 duplicated, 0 stored but reported failed, 0 unrecognised |
| conditioned appends: one winner per subject | pass | 0 won, 0 lost, 0 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 6445 polls: -journal present on 6445, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

167 hook processes, 5019 command appends, 0 claims; 1285004 durable events (~42833/s). Telemetry: 144 written, 23 dropped on a busy lock.

Long-lived writers that opened the store and ran: 2 of 2.

Hook durable path (open + append) over 250 ms: 94, over 1 s: 58, over 2 s: 39.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 167 | 0.4 | 119.4 | 209.6 | 260.0 |
| hook append | 167 | 328.1 | 2886.9 | 3031.9 | 3036.8 |
| hook open + append | 167 | 330.3 | 2897.6 | 3047.5 | 3164.4 |
| hook telemetry | 167 | 1.3 | 2.7 | 21.2 | 49.4 |
| hook process, spawn to exit | 167 | 334.1 | 2901.8 | 3050.9 | 3168.4 |
| command open | 2 | 1.4 | 1.8 | 1.8 | 1.8 |
| command append (batch) | 5019 | 5.1 | 8.3 | 21.3 | 4481.8 |
| conditioned claim | 0 | - | - | - | - |

## import, yielding: as import, with 25 ms between chunks (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| no open or write failed other than on a busy lock | pass |  |
| no open or write refused on a busy lock past its timeout | pass |  |
| positions run 1..N with no gap | pass | N = 135985 |
| every reported append is stored exactly once, and nothing else | pass | 135985 reported, 135985 stored, 0 missing, 0 duplicated, 0 stored but reported failed, 0 unrecognised |
| conditioned appends: one winner per subject | pass | 0 won, 0 lost, 0 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 3629 polls: -journal present on 3629, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

817 hook processes, 528 command appends, 0 claims; 135985 durable events (~4532/s). Telemetry: 752 written, 65 dropped on a busy lock.

Long-lived writers that opened the store and ran: 2 of 2.

Hook durable path (open + append) over 250 ms: 11, over 1 s: 0, over 2 s: 0.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 817 | 0.8 | 160.5 | 272.8 | 399.8 |
| hook append | 817 | 2.7 | 35.5 | 99.0 | 153.5 |
| hook open + append | 817 | 12.5 | 168.2 | 275.1 | 404.7 |
| hook telemetry | 817 | 1.5 | 4.0 | 5.9 | 42.8 |
| hook process, spawn to exit | 817 | 20.7 | 175.3 | 286.6 | 418.4 |
| command open | 2 | 0.4 | 0.6 | 0.6 | 0.6 |
| command append (batch) | 528 | 12.4 | 54.0 | 100.4 | 183.1 |
| conditioned claim | 0 | - | - | - | - |

## stress: 4 hook lanes, 2 commands (batch 5), 2 racers, no pauses (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| no open or write failed other than on a busy lock | pass |  |
| no open or write refused on a busy lock past its timeout | **busy** | 17 x busy:append__appending_to_the_store__the_store_was_busy_and_refused_the_append_before_writing_anything__SQLite_failed__database_is_locked__database_is_locked__Error_code_5__database_is_locked; 1 x busy:open__opening_the_store_/Users/runner/work/_temp/r-sqlite/mix-3/events.sqlite3__another_process_is_opening_the_store;_timed_out_waiting_for_it |
| positions run 1..N with no gap | pass | N = 96681 |
| every reported append is stored exactly once, and nothing else | pass | 89882 reported, 89882 stored, 0 missing, 0 duplicated, 0 stored but reported failed, 0 unrecognised |
| conditioned appends: one winner per subject | pass | 6799 won, 1382 lost, 6799 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 3976 polls: -journal present on 3976, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

90 hook processes, 17962 command appends, 8181 claims; 96681 durable events (~3222/s). Telemetry: 65 written, 24 dropped on a busy lock.

Long-lived writers that opened the store and ran: 4 of 4.

Hook durable path (open + append) over 250 ms: 39, over 1 s: 33, over 2 s: 26.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 90 | 0.5 | 1208.4 | 2093.7 | 2093.7 |
| hook append | 90 | 2.6 | 4374.5 | 4544.6 | 4544.6 |
| hook open + append | 90 | 31.4 | 4912.5 | 5536.3 | 5536.3 |
| hook telemetry | 90 | 1.3 | 3.6 | 5.7 | 5.7 |
| hook process, spawn to exit | 90 | 35.6 | 4919.0 | 5539.9 | 5539.9 |
| command open | 4 | 0.6 | 3217.6 | 3217.6 | 3217.6 |
| command append (batch) | 17962 | 1.1 | 1.5 | 2.8 | 10942.8 |
| conditioned claim | 8181 | 0.7 | 1.2 | 2.6 | 10094.0 |

## hooks only: 8 hook lanes, no pauses (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| no open or write failed other than on a busy lock | pass |  |
| no open or write refused on a busy lock past its timeout | pass |  |
| positions run 1..N with no gap | pass | N = 6480 |
| every reported append is stored exactly once, and nothing else | pass | 6480 reported, 6480 stored, 0 missing, 0 duplicated, 0 stored but reported failed, 0 unrecognised |
| conditioned appends: one winner per subject | pass | 0 won, 0 lost, 0 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 4012 polls: -journal present on 4012, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

6480 hook processes, 0 command appends, 0 claims; 6480 durable events (~216/s). Telemetry: 5945 written, 535 dropped on a busy lock.

Long-lived writers that opened the store and ran: 0 of 0.

Hook durable path (open + append) over 250 ms: 282, over 1 s: 10, over 2 s: 0.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 6480 | 0.3 | 214.9 | 603.0 | 1645.4 |
| hook append | 6480 | 1.0 | 1.5 | 10.1 | 128.4 |
| hook open + append | 6480 | 1.4 | 218.3 | 607.7 | 1646.6 |
| hook telemetry | 6480 | 1.5 | 2.0 | 2.4 | 35.6 |
| hook process, spawn to exit | 6480 | 5.7 | 223.1 | 612.6 | 1650.9 |
| command open | 0 | - | - | - | - |
| command append (batch) | 0 | - | - | - | - |
| conditioned claim | 0 | - | - | - | - |

## paced: 4 hook lanes every 100 ms, 2 commands every 50 ms, 2 racers every 20 ms (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| no open or write failed other than on a busy lock | pass |  |
| no open or write refused on a busy lock past its timeout | pass |  |
| positions run 1..N with no gap | pass | N = 3116 |
| every reported append is stored exactly once, and nothing else | pass | 2806 reported, 2806 stored, 0 missing, 0 duplicated, 0 stored but reported failed, 0 unrecognised |
| conditioned appends: one winner per subject | pass | 310 won, 308 lost, 310 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 3570 polls: -journal present on 3570, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

571 hook processes, 447 command appends, 618 claims; 3116 durable events (~103/s). Telemetry: 541 written, 30 dropped on a busy lock.

Long-lived writers that opened the store and ran: 4 of 4.

Hook durable path (open + append) over 250 ms: 1, over 1 s: 0, over 2 s: 0.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 571 | 0.9 | 142.9 | 177.0 | 272.6 |
| hook append | 571 | 2.0 | 20.1 | 37.5 | 101.3 |
| hook open + append | 571 | 3.3 | 151.1 | 190.8 | 274.4 |
| hook telemetry | 571 | 2.8 | 4.4 | 23.3 | 36.9 |
| hook process, spawn to exit | 571 | 13.3 | 160.5 | 207.5 | 290.4 |
| command open | 4 | 0.5 | 92.8 | 92.8 | 92.8 |
| command append (batch) | 447 | 9.2 | 51.8 | 84.3 | 357.0 |
| conditioned claim | 618 | 2.6 | 37.8 | 90.4 | 321.5 |

