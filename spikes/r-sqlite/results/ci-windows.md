# R-SQLITE run: windows x86_64

Correctness: **PASS** (no event lost, duplicated or invented; no file deleted beside the store; no WAL; integrity ok). Busy refusals are load findings, reported per scenario.

## Cold start: 8 hooks open a missing store at once

20 of 20 rounds passed every check.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| open (creates or waits) | 160 | 88.4 | 174.7 | 253.7 | 274.1 |
| append | 160 | 1.9 | 15.7 | 61.6 | 62.7 |

## busy: 4 hook lanes, 2 commands (batch 5) every 10 ms, 2 racers every 5 ms (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| no open or write failed other than on a busy lock | pass |  |
| no open or write refused on a busy lock past its timeout | **busy** | 1 x busy:append__appending_to_the_store__the_store_was_busy_and_refused_the_append_before_writing_anything__SQLite_failed__database_is_locked__database_is_locked__Error_code_5__database_is_locked |
| positions run 1..N with no gap | pass | N = 5533 |
| every reported append is stored exactly once, and nothing else | pass | 5082 reported, 5082 stored, 0 missing, 0 duplicated, 0 stored but reported failed, 0 unrecognised |
| conditioned appends: one winner per subject | pass | 451 won, 202 lost, 451 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 16981 polls: -journal present on 16981, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

773 hook processes, 862 command appends, 653 claims; 5533 durable events (~184/s). Telemetry: 551 written, 222 dropped on a busy lock.

Long-lived writers that opened the store and ran: 4 of 4.

Hook durable path (open + append) over 250 ms: 76, over 1 s: 19, over 2 s: 4.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 773 | 21.8 | 125.4 | 190.6 | 230.6 |
| hook append | 773 | 4.2 | 448.5 | 1578.5 | 2729.3 |
| hook open + append | 773 | 47.5 | 512.8 | 1608.7 | 2815.4 |
| hook telemetry | 773 | 4.9 | 63.3 | 78.9 | 112.0 |
| hook process, spawn to exit | 773 | 79.5 | 522.6 | 1676.4 | 2859.1 |
| command open | 4 | 8.2 | 210.4 | 210.4 | 210.4 |
| command append (batch) | 862 | 4.9 | 245.4 | 791.1 | 2025.1 |
| conditioned claim | 653 | 14.5 | 376.2 | 1357.4 | 2415.1 |

## import: 6 hook lanes every 100 ms, 2 commands appending 256-event chunks back to back (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| no open or write failed other than on a busy lock | pass |  |
| no open or write refused on a busy lock past its timeout | **busy** | 13 x busy:append__appending_to_the_store__the_store_was_busy_and_refused_the_append_before_writing_anything__SQLite_failed__database_is_locked__database_is_locked__Error_code_5__database_is_locked |
| positions run 1..N with no gap | pass | N = 530755 |
| every reported append is stored exactly once, and nothing else | pass | 530755 reported, 530755 stored, 0 missing, 0 duplicated, 0 stored but reported failed, 0 unrecognised |
| conditioned appends: one winner per subject | pass | 0 won, 0 lost, 0 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 18914 polls: -journal present on 18914, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

336 hook processes, 2072 command appends, 0 claims; 530755 durable events (~17691/s). Telemetry: 294 written, 42 dropped on a busy lock.

Long-lived writers that opened the store and ran: 2 of 2.

Hook durable path (open + append) over 250 ms: 115, over 1 s: 47, over 2 s: 23.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 336 | 1.7 | 57.9 | 96.8 | 183.2 |
| hook append | 336 | 110.7 | 2182.0 | 2417.4 | 2483.7 |
| hook open + append | 336 | 128.5 | 2195.6 | 2436.0 | 2485.0 |
| hook telemetry | 336 | 4.4 | 41.1 | 79.9 | 96.9 |
| hook process, spawn to exit | 336 | 152.3 | 2208.8 | 2475.0 | 2515.0 |
| command open | 2 | 1.5 | 82.9 | 82.9 | 82.9 |
| command append (batch) | 2072 | 10.8 | 69.1 | 407.0 | 3353.4 |
| conditioned claim | 0 | - | - | - | - |

## import, yielding: as import, with 25 ms between chunks (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| no open or write failed other than on a busy lock | pass |  |
| no open or write refused on a busy lock past its timeout | pass |  |
| positions run 1..N with no gap | pass | N = 234683 |
| every reported append is stored exactly once, and nothing else | pass | 234683 reported, 234683 stored, 0 missing, 0 duplicated, 0 stored but reported failed, 0 unrecognised |
| conditioned appends: one winner per subject | pass | 0 won, 0 lost, 0 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 17626 polls: -journal present on 17626, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

955 hook processes, 913 command appends, 0 claims; 234683 durable events (~7822/s). Telemetry: 691 written, 264 dropped on a busy lock.

Long-lived writers that opened the store and ran: 2 of 2.

Hook durable path (open + append) over 250 ms: 28, over 1 s: 2, over 2 s: 0.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 955 | 5.0 | 73.7 | 106.8 | 152.8 |
| hook append | 955 | 22.4 | 174.0 | 380.7 | 1869.5 |
| hook open + append | 955 | 47.0 | 206.0 | 440.8 | 1902.6 |
| hook telemetry | 955 | 4.6 | 45.8 | 64.5 | 97.2 |
| hook process, spawn to exit | 955 | 65.7 | 225.5 | 453.2 | 1914.9 |
| command open | 2 | 1.5 | 72.8 | 72.8 | 72.8 |
| command append (batch) | 913 | 16.8 | 126.5 | 191.5 | 600.1 |
| conditioned claim | 0 | - | - | - | - |

## stress: 4 hook lanes, 2 commands (batch 5), 2 racers, no pauses (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| no open or write failed other than on a busy lock | pass |  |
| no open or write refused on a busy lock past its timeout | **busy** | 26 x busy:append__appending_to_the_store__the_store_was_busy_and_refused_the_append_before_writing_anything__SQLite_failed__database_is_locked__database_is_locked__Error_code_5__database_is_locked |
| positions run 1..N with no gap | pass | N = 27729 |
| every reported append is stored exactly once, and nothing else | pass | 24410 reported, 24410 stored, 0 missing, 0 duplicated, 0 stored but reported failed, 0 unrecognised |
| conditioned appends: one winner per subject | pass | 3319 won, 428 lost, 3319 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 18859 polls: -journal present on 18859, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

101 hook processes, 4867 command appends, 3747 claims; 27729 durable events (~924/s). Telemetry: 63 written, 38 dropped on a busy lock.

Long-lived writers that opened the store and ran: 4 of 4.

Hook durable path (open + append) over 250 ms: 54, over 1 s: 38, over 2 s: 33.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 101 | 42.3 | 635.0 | 1149.0 | 1637.8 |
| hook append | 101 | 162.9 | 2859.9 | 2918.8 | 3000.3 |
| hook open + append | 101 | 310.9 | 3348.6 | 3995.5 | 4556.7 |
| hook telemetry | 101 | 4.5 | 48.5 | 65.5 | 126.0 |
| hook process, spawn to exit | 101 | 349.4 | 3375.8 | 4056.5 | 4564.4 |
| command open | 4 | 324.4 | 1342.8 | 1342.8 | 1342.8 |
| command append (batch) | 4867 | 3.4 | 6.4 | 19.4 | 7855.0 |
| conditioned claim | 3747 | 2.5 | 3.9 | 18.3 | 9631.9 |

## hooks only: 8 hook lanes, no pauses (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| no open or write failed other than on a busy lock | pass |  |
| no open or write refused on a busy lock past its timeout | pass |  |
| positions run 1..N with no gap | pass | N = 1859 |
| every reported append is stored exactly once, and nothing else | pass | 1859 reported, 1859 stored, 0 missing, 0 duplicated, 0 stored but reported failed, 0 unrecognised |
| conditioned appends: one winner per subject | pass | 0 won, 0 lost, 0 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 16742 polls: -journal present on 16742, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

1859 hook processes, 0 command appends, 0 claims; 1859 durable events (~61/s). Telemetry: 487 written, 1372 dropped on a busy lock.

Long-lived writers that opened the store and ran: 0 of 0.

Hook durable path (open + append) over 250 ms: 192, over 1 s: 0, over 2 s: 0.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 1859 | 68.4 | 308.7 | 461.6 | 651.1 |
| hook append | 1859 | 3.5 | 58.5 | 98.1 | 712.1 |
| hook open + append | 1859 | 86.9 | 321.8 | 485.2 | 843.1 |
| hook telemetry | 1859 | 0.1 | 41.5 | 60.3 | 94.4 |
| hook process, spawn to exit | 1859 | 99.9 | 336.0 | 495.8 | 851.6 |
| command open | 0 | - | - | - | - |
| command append (batch) | 0 | - | - | - | - |
| conditioned claim | 0 | - | - | - | - |

## paced: 4 hook lanes every 100 ms, 2 commands every 50 ms, 2 racers every 20 ms (30 s)

| Check | Result | Detail |
|---|---|---|
| every writer process reported every operation (none crashed) | pass |  |
| no open or write failed other than on a busy lock | pass |  |
| no open or write refused on a busy lock past its timeout | pass |  |
| positions run 1..N with no gap | pass | N = 4653 |
| every reported append is stored exactly once, and nothing else | pass | 4020 reported, 4020 stored, 0 missing, 0 duplicated, 0 stored but reported failed, 0 unrecognised |
| conditioned appends: one winner per subject | pass | 633 won, 589 lost, 633 claims stored, 0 subjects claimed twice |
| TRUNCATE layout afterwards: empty -journal, no -wal or -shm, rollback header | pass | -journal 0 bytes, -wal false, -shm false, header versions Some([1, 1]) (1 = rollback, 2 = WAL) |
| nothing deleted, and no -wal or -shm created, while the writers ran | pass | 16952 polls: -journal present on 16952, missing after first seen on 0, -wal or -shm on 0 |
| PRAGMA integrity_check | pass | ok |

720 hook processes, 660 command appends, 1222 claims; 4653 durable events (~155/s). Telemetry: 605 written, 115 dropped on a busy lock.

Long-lived writers that opened the store and ran: 4 of 4.

Hook durable path (open + append) over 250 ms: 5, over 1 s: 0, over 2 s: 0.

| Step (ms) | n | p50 | p95 | p99 | max |
|---|---|---|---|---|---|
| hook open | 720 | 5.1 | 83.0 | 127.8 | 210.0 |
| hook append | 720 | 3.3 | 81.7 | 161.0 | 418.9 |
| hook open + append | 720 | 22.2 | 137.2 | 234.5 | 491.9 |
| hook telemetry | 720 | 4.6 | 60.5 | 77.5 | 92.6 |
| hook process, spawn to exit | 720 | 52.8 | 170.6 | 249.4 | 525.6 |
| command open | 4 | 22.0 | 91.8 | 91.8 | 91.8 |
| command append (batch) | 660 | 23.4 | 147.1 | 205.1 | 312.9 |
| conditioned claim | 1222 | 4.3 | 112.7 | 210.8 | 406.0 |

