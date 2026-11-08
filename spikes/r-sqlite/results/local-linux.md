# R-SQLITE run: linux x86_64

Correctness: **PASS** (no event lost, duplicated or invented; no file deleted beside the store; no WAL; integrity ok). Busy refusals are load findings, reported per scenario.

## Cold start: 8 hooks open a missing store at once

20 of 20 rounds passed every check.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| open (creates or waits) | 160 | 49.7 | 141.6 | 285.2 | 303.7 |
| append | 160 | 2.1 | 3.5 | 6.4 | 126.3 |

## busy: 4 hook lanes, 2 commands (batch 5) every 10 ms, 2 racers every 5 ms (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| every open and write succeeded, or lost a claim cleanly | pass |  |
| positions run 1..N with no gap | pass | N = 17134 |
| every reported append is stored exactly once, and nothing else | pass | 15430 reported, 15430 stored, 0 missing, 0 duplicated, 0 stored but reported failed |
| conditioned appends: one winner per subject | pass | 1704 won, 1078 lost, 1704 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 27052 polls: -journal present on 27052, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

2560 hook processes, 2574 command appends, 2782 claims; 17134 durable events (~571/s). Telemetry: 1063 written, 1497 dropped on a busy lock.

Hook durable path (open + append) over 250 ms: 47, over 1 s: 0, over 2 s: 0.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 2560 | 19.0 | 141.4 | 265.5 | 573.1 |
| hook append | 2560 | 2.7 | 21.1 | 108.1 | 932.9 |
| hook open + append | 2560 | 23.0 | 156.1 | 294.5 | 934.4 |
| hook telemetry | 2560 | 0.0 | 4.9 | 6.1 | 12.7 |
| hook process, spawn to exit | 2560 | 25.7 | 162.2 | 302.7 | 936.4 |
| command open | 4 | 20.5 | 418.6 | 418.6 | 418.6 |
| command append (batch) | 2574 | 4.3 | 56.7 | 182.2 | 633.4 |
| conditioned claim | 2782 | 3.4 | 78.9 | 229.5 | 1134.5 |

## import: 6 hook lanes every 100 ms, 2 commands appending 256-event chunks back to back (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| every open and write succeeded, or lost a claim cleanly | **busy** | 29 x err:append__appending_to_the_store__the_store_was_busy_and_refused_the_append_before_writing_anything__SQLite_failed__database_is_locked__database_is_locked__Error_code_5__database_is_locked |
| positions run 1..N with no gap | pass | N = 892391 |
| every reported append is stored exactly once, and nothing else | pass | 892391 reported, 892391 stored, 0 missing, 0 duplicated, 0 stored but reported failed |
| conditioned appends: one winner per subject | pass | 0 won, 0 lost, 0 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 27180 polls: -journal present on 27180, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

260 hook processes, 3485 command appends, 0 claims; 892391 durable events (~29746/s). Telemetry: 221 written, 39 dropped on a busy lock.

Hook durable path (open + append) over 250 ms: 113, over 1 s: 68, over 2 s: 33.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 260 | 1.4 | 27.1 | 105.1 | 243.1 |
| hook append | 260 | 181.5 | 2004.6 | 2006.8 | 2008.2 |
| hook open + append | 260 | 182.6 | 2007.7 | 2028.9 | 2234.8 |
| hook telemetry | 260 | 3.2 | 5.3 | 10.3 | 13.6 |
| hook process, spawn to exit | 260 | 187.5 | 2013.3 | 2037.3 | 2237.6 |
| command open | 2 | 0.4 | 0.7 | 0.7 | 0.7 |
| command append (batch) | 3485 | 7.6 | 12.0 | 44.0 | 4549.1 |
| conditioned claim | 0 | - | - | - | - |

## import, yielding: as import, with 25 ms between chunks (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| every open and write succeeded, or lost a claim cleanly | pass |  |
| positions run 1..N with no gap | pass | N = 453674 |
| every reported append is stored exactly once, and nothing else | pass | 453674 reported, 453674 stored, 0 missing, 0 duplicated, 0 stored but reported failed |
| conditioned appends: one winner per subject | pass | 0 won, 0 lost, 0 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 27277 polls: -journal present on 27277, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

1578 hook processes, 1766 command appends, 0 claims; 453674 durable events (~15122/s). Telemetry: 1478 written, 100 dropped on a busy lock.

Hook durable path (open + append) over 250 ms: 0, over 1 s: 0, over 2 s: 0.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 1578 | 0.3 | 8.9 | 23.7 | 81.0 |
| hook append | 1578 | 2.6 | 21.1 | 56.7 | 107.6 |
| hook open + append | 1578 | 4.2 | 24.2 | 58.1 | 108.2 |
| hook telemetry | 1578 | 2.9 | 4.4 | 5.8 | 9.2 |
| hook process, spawn to exit | 1578 | 9.9 | 29.8 | 67.3 | 114.4 |
| command open | 2 | 0.4 | 60.9 | 60.9 | 60.9 |
| command append (batch) | 1766 | 7.3 | 12.1 | 19.5 | 105.2 |
| conditioned claim | 0 | - | - | - | - |

## stress: 4 hook lanes, 2 commands (batch 5), 2 racers, no pauses (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| every open and write succeeded, or lost a claim cleanly | **busy** | 25 x err:append__appending_to_the_store__the_store_was_busy_and_refused_the_append_before_writing_anything__SQLite_failed__database_is_locked__database_is_locked__Error_code_5__database_is_locked; 8 x err:open__opening_the_store_$DIR/mix-3/events.sqlite3__another_process_is_opening_the_store;_timed_out_waiting_for_it |
| positions run 1..N with no gap | pass | N = 51902 |
| every reported append is stored exactly once, and nothing else | pass | 48758 reported, 48758 stored, 0 missing, 0 duplicated, 0 stored but reported failed |
| conditioned appends: one winner per subject | pass | 3144 won, 3039 lost, 3144 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 27322 polls: -journal present on 27322, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

86 hook processes, 9741 command appends, 6183 claims; 51902 durable events (~1730/s). Telemetry: 34 written, 44 dropped on a busy lock.

Hook durable path (open + append) over 250 ms: 59, over 1 s: 49, over 2 s: 39.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 86 | 137.6 | 2095.8 | 3427.7 | 3427.7 |
| hook append | 86 | 131.2 | 2004.6 | 2006.4 | 2006.4 |
| hook open + append | 86 | 1591.1 | 3279.1 | 5431.2 | 5431.2 |
| hook telemetry | 86 | 0.1 | 4.5 | 9.2 | 9.2 |
| hook process, spawn to exit | 86 | 1593.2 | 3283.6 | 5433.2 | 5433.2 |
| command open | 4 | 3585.4 | 6063.6 | 6063.6 | 6063.6 |
| command append (batch) | 9741 | 2.2 | 3.5 | 4.8 | 6339.7 |
| conditioned claim | 6183 | 1.1 | 2.6 | 3.4 | 8239.4 |

## hooks only: 8 hook lanes, no pauses (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| every open and write succeeded, or lost a claim cleanly | pass |  |
| positions run 1..N with no gap | pass | N = 7474 |
| every reported append is stored exactly once, and nothing else | pass | 7474 reported, 7474 stored, 0 missing, 0 duplicated, 0 stored but reported failed |
| conditioned appends: one winner per subject | pass | 0 won, 0 lost, 0 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 27223 polls: -journal present on 27223, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

7474 hook processes, 0 command appends, 0 claims; 7474 durable events (~249/s). Telemetry: 1782 written, 5692 dropped on a busy lock.

Hook durable path (open + append) over 250 ms: 9, over 1 s: 0, over 2 s: 0.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 7474 | 3.7 | 121.0 | 181.2 | 323.1 |
| hook append | 7474 | 2.1 | 3.7 | 6.3 | 433.1 |
| hook open + append | 7474 | 6.9 | 123.2 | 183.2 | 494.0 |
| hook telemetry | 7474 | 0.0 | 3.6 | 4.6 | 11.2 |
| hook process, spawn to exit | 7474 | 11.5 | 125.0 | 185.0 | 495.8 |
| command open | 0 | - | - | - | - |
| command append (batch) | 0 | - | - | - | - |
| conditioned claim | 0 | - | - | - | - |

## paced: 4 hook lanes every 100 ms, 2 commands every 50 ms, 2 racers every 20 ms (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| every open and write succeeded, or lost a claim cleanly | pass |  |
| positions run 1..N with no gap | pass | N = 7928 |
| every reported append is stored exactly once, and nothing else | pass | 6646 reported, 6646 stored, 0 missing, 0 duplicated, 0 stored but reported failed |
| conditioned appends: one winner per subject | pass | 1282 won, 1282 lost, 1282 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 27319 polls: -journal present on 27319, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

1096 hook processes, 1110 command appends, 2564 claims; 7928 durable events (~264/s). Telemetry: 1059 written, 37 dropped on a busy lock.

Hook durable path (open + append) over 250 ms: 0, over 1 s: 0, over 2 s: 0.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 1096 | 0.3 | 4.7 | 21.6 | 42.1 |
| hook append | 1096 | 2.3 | 4.8 | 8.7 | 20.9 |
| hook open + append | 1096 | 2.9 | 10.9 | 24.6 | 44.1 |
| hook telemetry | 1096 | 3.1 | 4.9 | 6.2 | 8.8 |
| hook process, spawn to exit | 1096 | 8.3 | 15.9 | 31.5 | 49.3 |
| command open | 4 | 20.5 | 63.0 | 63.0 | 63.0 |
| command append (batch) | 1110 | 3.0 | 7.2 | 20.9 | 36.6 |
| conditioned claim | 2564 | 2.5 | 8.5 | 18.8 | 53.9 |

