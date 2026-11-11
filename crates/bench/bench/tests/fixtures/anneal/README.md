# Old-ledger fixtures (Move 2 step 3)

anneal and dev `bench` ledgers that step 3's one-time import (PR 3.5) reads, each only ever from a
copy (`redkiln_core::legacy::read_ledger` copies it itself). `tests/it/fixtures.rs` pins this
listing, every file's sha256, each file's journal mode, stamp and happenstance schema, and the
event types Core's reader finds. Nothing here may be opened in place: SQLite on the WAL fixture,
even read-only, checkpoints it or leaves a `-shm`. Generated on 2026-11-11 (PR 3.1) by the
scripts below, outside the workspace, in throwaway directories: no workspace dependency or
lockfile changed.

| File | Written by | Ledger | Events |
|---|---|---|---|
| `anneal-0.0.5.sqlite3` | anneal at the imported tip `664aaa5` (reports `anneal 0.0.5`, registry 10, happenstance 0.3.2) | rollback header (TRUNCATE), `user_version` 1, happenstance schema 1 | 23: all 17 types; `plan_approved` by `plan approve` with `--actor human --via mod`, `--actor model --via bash`, none, and `--via terminal` alone; `verdict_set` by `verdict set` with a human and a model `by`; the rest by `ledger append`, with a session, a worktree path, a branch and a head, a `now_millis` stamp whose trailing zero anneal dropped (`10:04:00.12Z`, also `.5Z`), and a `task_done` whose actor `test` Core refuses |
| `anneal-older.sqlite3` | anneal at tag `v0.0.1` (`2ffa7a2`, registry 5, happenstance 0.3.2) | as above | 7 by `ledger append`: shapes the lenient read must still take: `round_started` with `pr` only (before registry 8's `ticket`), `grill_confirmed` with no `hash` or `quote`, `verdict_set` with no `source`, `sha` or `by` |
| `anneal-wal.sqlite3`, `anneal-wal.sqlite3-wal` | `anneal-0.0.5.sqlite3`'s run, copied after its 16th event, switched to WAL by Python's `sqlite3` with `wal_autocheckpoint = 0`; the full ledger's later rows (`event`, `event_tag`, `tag_cardinality`, `sqlite_sequence`), byte for byte as anneal wrote them, then written through that connection, left without a checkpoint | WAL header, `user_version` 1, happenstance schema 1; the main file alone holds 16 events, with its `-wal` 23 | the same events and store id as `anneal-0.0.5.sqlite3` (anneal refused WAL; this stands for a ledger another tool switched, as Core's `weighin-0.1.x` fixture does). Import it into another store than `anneal-0.0.5.sqlite3`'s: their origins are the same |
| `bench-ledger-hs060.sqlite3` | the workspace's `bench` at `5b09037` (`bench-ledger` on happenstance 0.6.0, registry 10), as a dev build since #80 wrote | rollback header, `user_version` 1, happenstance schema 2 | 5: `plan approve --actor human --via mod`, `verdict set` with no `by`, and three `ledger append`s, one with a `.25Z` stamp |
| `bench-ledger-hs032.sqlite3` | copied unchanged from `crates/bench/bench-ledger/tests/fixtures/` (that crate's in-place migration test keeps its own copy until PR 3.4 deletes the crate) | rollback header, `user_version` 1, happenstance schema 1 | 3: `task_done`, `test_run`, `task_done`; its actor `test` is the refused-actor case |

## How they were made

anneal's trees come from this repository's history (the Move 2 import keeps anneal's commits):

```sh
mkdir -p anneal-tip anneal-0.0.1                 # tar -C needs the directories
git archive 664aaa5 | tar -x -C anneal-tip      # imported anneal tip
git archive 2ffa7a2 | tar -x -C anneal-0.0.1      # anneal's v0.0.1 tag
(cd anneal-tip && CARGO_TARGET_DIR=../target-tip cargo build --locked -p anneal-cli)
(cd anneal-0.0.1 && CARGO_TARGET_DIR=../target-001 cargo build --locked -p anneal-cli)
cargo build -p bench                            # in this workspace, at 5b09037
sh gen-anneal.sh target-tip/debug/anneal target-001/debug/anneal out
python3 -I make-wal.py out/prefix.sqlite3 out/anneal-0.0.5.sqlite3 out/anneal-wal.sqlite3
rm out/prefix.sqlite3 out/anneal-wal.sqlite3-shm  # the -shm is rebuilt from the -wal
sh gen-hs060.sh target/debug/bench out
```

Each ledger was checked on a copy with Python's `sqlite3` before it was committed. Timestamps from
the commands are the clock's, so a rerun gives other bytes: regenerate all of them together and
update the hashes in `tests/it/fixtures.rs`.

`gen-anneal.sh`:

```sh
#!/bin/sh
# Generates anneal-0.0.5.sqlite3 (frozen anneal tip 664aaa5, reports 0.0.5, registry 10) and
# anneal-older.sqlite3 (tag v0.0.1, 2ffa7a2, registry 5). Usage: gen-anneal.sh <tip-bin> <001-bin> <out-dir>
set -eu
# The script cd's into its scratch repository, so every argument is made absolute first.
abs_bin() { printf '%s/%s\n' "$(cd "$(dirname "$1")" && pwd)" "$(basename "$1")"; }
TIP=$(abs_bin "$1")
OLD=$(abs_bin "$2")
mkdir -p "$3"
OUT=$(cd "$3" && pwd)
WORK=$(mktemp -d "${TMPDIR:-/tmp}/anneal-gen.XXXXXX")
unset ANNEAL_STORE REDKILN_STORE GITHUB_TOKEN GH_TOKEN || true
export HOME="$WORK/home"
mkdir -p "$HOME" "$OUT"
S=4c1d8f2e-6b3a-4f7c-9d2e-1a5b7c3d9e0f
H=bd234009c1e5a7d4f1c3b2a0e9f8d7c6b5a49382
WT=/home/user/anneal-fixture

# ---- anneal 0.0.5 (tip) ----
R="$WORK/repo"
mkdir -p "$R/docs/specs" "$R/docs/plans"
cd "$R"
git init -q -b main
git -c user.name=fixture -c user.email=fixture@example.com commit -q --allow-empty -m init
printf '# Spec: export\n\n## Problem\n\nAll agreed.\n' > docs/specs/export.md
for p in human model plain via; do
  cat > "docs/plans/$p.md" <<EOF
# Plan: export ($p)

- Spec: [export](../specs/export.md)

## Goal

Analysts export a report as CSV.

## Tasks

### T1: CSV writer

- Files: \`src/csv.rs\`
- Test: \`cargo test -p report csv\`
- Blocked by: none

## Review focus

- Quoting.
EOF
done
L=.git/anneal/ledger.sqlite3
"$TIP" plan approve docs/plans/human.md --actor human --via mod --session "$S" >/dev/null
"$TIP" plan approve docs/plans/model.md --actor model --via bash --session "$S" >/dev/null
"$TIP" plan approve docs/plans/plain.md >/dev/null
"$TIP" plan approve docs/plans/via.md --via terminal >/dev/null
"$TIP" verdict set --pr 7 --sha HEAD --level approved --source human --actor human --via terminal --no-status --session "$S" >/dev/null
"$TIP" verdict set --pr 8 --sha HEAD --level reviewed --source local --actor model --via bash --no-status >/dev/null
append() { "$TIP" ledger --store "$L" append "$1" >/dev/null; }
ctx="\"session_id\":\"$S\",\"worktree\":\"$WT\",\"branch\":\"main\",\"head\":\"$H\""
append "{\"type\":\"test_run\",\"task\":\"T1\",\"command\":\"cargo test\",\"outcome\":\"red\",\"exit\":101,\"duration_ms\":1200,\"timeout_secs\":600,\"actor\":\"hook\",$ctx,\"ts\":\"2026-11-02T10:04:00.12Z\"}"
append "{\"type\":\"task_done\",\"task\":\"T1\",\"actor\":\"cli\",$ctx,\"ts\":\"2026-11-02T10:05:00Z\"}"
append "{\"type\":\"grill_confirmed\",\"plan\":\"docs/design/export.json\",\"hash\":\"sha256:2c26b46b68ffc68ff99b453c1d30413413422d706483bfa0f98a5e886266e7ae\",\"quote\":\"yes, go\",\"actor\":\"cli\",$ctx,\"ts\":\"2026-11-02T10:06:00Z\"}"
append "{\"type\":\"repro_red\",\"task\":\"T2\",\"test\":\"csv::quotes\",\"actor\":\"cli\",$ctx,\"ts\":\"2026-11-02T10:07:00Z\"}"
append "{\"type\":\"round_started\",\"pr\":7,\"round\":1,\"actor\":\"cli\",$ctx,\"ts\":\"2026-11-02T10:08:00Z\"}"
append "{\"type\":\"round_started\",\"ticket\":\"T2\",\"round\":2,\"actor\":\"cli\",$ctx,\"ts\":\"2026-11-02T10:08:30Z\"}"
append "{\"type\":\"gate_blocked\",\"point\":\"preconditions\",\"reason\":\"no approved plan\",\"actor\":\"hook\",$ctx,\"ts\":\"2026-11-02T10:09:00Z\"}"
append "{\"type\":\"stop_blocked\",\"point\":\"completeness\",\"reason\":\"tests not run\",\"actor\":\"hook\",$ctx,\"ts\":\"2026-11-02T10:10:00Z\"}"
append "{\"type\":\"mode_set\",\"mode\":\"build\",\"clean\":true,\"plan\":\"docs/plans/human.md\",\"tree\":null,\"actor\":\"cli\",\"session_id\":\"$S\",\"worktree\":\"$WT\",\"branch\":null,\"head\":null,\"ts\":\"2026-11-02T10:11:00Z\"}"
append "{\"type\":\"subagent_status\",\"agent\":\"rust-implementer\",\"agent_id\":\"a1\",\"status\":\"wip\",\"violation\":null,\"missing\":[],\"duration_ms\":5000,\"tokens\":{\"input\":10,\"output\":20,\"cache_read\":30,\"cache_creation\":40},\"actor\":\"hook\",$ctx,\"ts\":\"2026-11-02T10:12:00.5Z\"}"
cp "$L" "$WORK/prefix.sqlite3"
append "{\"type\":\"migrate.wave.started\",\"migration\":\"m1\",\"wave\":1,\"actor\":\"cli\",$ctx,\"ts\":\"2026-11-02T10:13:00Z\"}"
append "{\"type\":\"migrate.wave.done\",\"migration\":\"m1\",\"wave\":1,\"actor\":\"cli\",$ctx,\"ts\":\"2026-11-02T10:14:00Z\"}"
append "{\"type\":\"migrate.contract\",\"migration\":\"m1\",\"actor\":\"cli\",$ctx,\"ts\":\"2026-11-02T10:15:00Z\"}"
append "{\"type\":\"weighin_parked\",\"item_id\":\"wi-3fa9c01b2e\",\"ticket\":\"T3\",\"decision\":\"delimiter\",\"actor\":\"cli\",$ctx,\"ts\":\"2026-11-02T10:16:00Z\"}"
append "{\"type\":\"bridge.unavailable\",\"ticket\":\"T3\",\"reason\":\"weighin not found\",\"actor\":\"cli\",$ctx,\"ts\":\"2026-11-02T10:17:00Z\"}"
append "{\"type\":\"orchestrate.ruled\",\"ticket\":\"T3\",\"what\":\"comma\",\"why\":\"most tools\",\"cost_if_wrong\":\"low\",\"decision\":\"delimiter\",\"actor\":\"agent\",$ctx,\"ts\":\"2026-11-02T10:18:00Z\"}"
# An actor Core refuses (Actor::parse): anneal accepted any text.
append "{\"type\":\"task_done\",\"task\":\"T9\",\"actor\":\"test\",\"session_id\":null,\"worktree\":null,\"branch\":null,\"head\":null,\"ts\":\"2026-11-02T10:19:00Z\"}"
"$TIP" ledger --store "$L" query > "$WORK/anneal-0.0.5.json"
cp "$L" "$OUT/anneal-0.0.5.sqlite3"

# ---- anneal 0.0.1 (registry 5) ----
O="$WORK/older"
mkdir -p "$O"
OL="$O/ledger.sqlite3"
old() { "$OLD" ledger --store "$OL" append "$1" >/dev/null; }
octx="\"session_id\":\"$S\",\"worktree\":\"$WT\",\"branch\":\"main\",\"head\":\"$H\""
old "{\"type\":\"test_run\",\"task\":null,\"command\":\"cargo test\",\"outcome\":\"green\",\"exit\":0,\"duration_ms\":900,\"timeout_secs\":null,\"actor\":\"hook\",$octx,\"ts\":\"2026-10-12T09:00:00Z\"}"
old "{\"type\":\"plan_approved\",\"plan\":\"docs/plans/export.md\",\"hash\":\"sha256:9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08\",\"actor\":\"cli\",$octx,\"ts\":\"2026-10-12T09:01:00Z\"}"
old "{\"type\":\"grill_confirmed\",\"plan\":\"docs/design/export.json\",\"hash\":null,\"quote\":null,\"actor\":\"cli\",$octx,\"ts\":\"2026-10-12T09:02:00Z\"}"
old "{\"type\":\"verdict_set\",\"pr\":7,\"verdict\":\"changes_requested\",\"actor\":\"cli\",$octx,\"ts\":\"2026-10-12T09:03:00Z\"}"
old "{\"type\":\"round_started\",\"pr\":7,\"round\":1,\"actor\":\"cli\",$octx,\"ts\":\"2026-10-12T09:04:00Z\"}"
old "{\"type\":\"mode_set\",\"mode\":\"diagnose\",\"clean\":false,\"plan\":null,\"tree\":null,\"actor\":\"cli\",$octx,\"ts\":\"2026-10-12T09:05:00Z\"}"
old "{\"type\":\"subagent_status\",\"agent\":\"reviewer\",\"agent_id\":null,\"status\":\"done_with_concerns\",\"violation\":null,\"missing\":[\"tests\"],\"duration_ms\":null,\"tokens\":null,\"actor\":\"hook\",$octx,\"ts\":\"2026-10-12T09:06:00Z\"}"
"$OLD" ledger --store "$OL" query > "$WORK/anneal-older.json"
cp "$OL" "$OUT/anneal-older.sqlite3"
cp "$WORK/prefix.sqlite3" "$OUT/prefix.sqlite3"
echo "$WORK"
```

`make-wal.py`:

```python
"""anneal-wal.sqlite3: anneal's own ledger up to an earlier point (prefix, by anneal 0.0.5),
switched to WAL with wal_autocheckpoint = 0, then the full ledger's later rows (byte for byte
as anneal wrote them) written through that connection, which is left without a checkpoint so
they live only in the -wal. Usage: make-wal.py <prefix> <full> <out>"""
import os, shutil, sqlite3, sys

prefix, full, out = sys.argv[1:4]
shutil.copyfile(prefix, out)
c = sqlite3.connect(out, isolation_level=None)
assert c.execute("PRAGMA journal_mode = wal").fetchone()[0] == "wal"
c.execute("PRAGMA wal_autocheckpoint = 0")
src = sqlite3.connect("file:" + full + "?mode=ro&immutable=1", uri=True)
last = c.execute("SELECT max(position) FROM event").fetchone()[0]
c.execute("BEGIN IMMEDIATE")
for row in src.execute("SELECT * FROM event WHERE position > ? ORDER BY position", (last,)):
    c.execute("INSERT INTO event VALUES (?,?,?,?,?,?,?,?)", row)
for row in src.execute("SELECT * FROM event_tag WHERE position > ?", (last,)):
    c.execute("INSERT INTO event_tag VALUES (?,?,?)", row)
for row in src.execute("SELECT * FROM tag_cardinality"):
    c.execute("INSERT OR REPLACE INTO tag_cardinality VALUES (?,?)", row)
for row in src.execute("SELECT * FROM sqlite_sequence"):
    c.execute("UPDATE sqlite_sequence SET seq = ? WHERE name = ?", (row[1], row[0]))
c.execute("COMMIT")
src.close()
sys.stdout.flush()
# Leave without closing, so SQLite neither checkpoints nor removes the -wal.
os._exit(0)
```

`gen-hs060.sh`:

```sh
#!/bin/sh
# bench-ledger-hs060.sqlite3: written by the workspace's `bench` binary (bench-ledger on
# happenstance 0.6.0) at the commit it was built from. Usage: gen-hs060.sh <bench-bin> <out-dir>
set -eu
# The script cd's into its scratch repository, so every argument is made absolute first.
B=$(printf '%s/%s\n' "$(cd "$(dirname "$1")" && pwd)" "$(basename "$1")")
mkdir -p "$2"
OUT=$(cd "$2" && pwd)
WORK=$(mktemp -d "${TMPDIR:-/tmp}/hs060-gen.XXXXXX")
unset ANNEAL_STORE REDKILN_STORE GITHUB_TOKEN GH_TOKEN || true
export HOME="$WORK/home"
mkdir -p "$HOME" "$OUT"
S=4c1d8f2e-6b3a-4f7c-9d2e-1a5b7c3d9e0f
WT=/home/user/anneal-fixture
R="$WORK/repo"
mkdir -p "$R/docs/specs" "$R/docs/plans"
cd "$R"
git init -q -b main
git -c user.name=fixture -c user.email=fixture@example.com commit -q --allow-empty -m init
printf '# Spec: export\n\n## Problem\n\nAll agreed.\n' > docs/specs/export.md
cat > docs/plans/export.md <<'EOF'
# Plan: export

- Spec: [export](../specs/export.md)

## Goal

Analysts export a report as CSV.

## Tasks

### T1: CSV writer

- Files: `src/csv.rs`
- Test: `cargo test -p report csv`
- Blocked by: none

## Review focus

- Quoting.
EOF
L=.git/anneal/ledger.sqlite3
"$B" plan approve docs/plans/export.md --actor human --via mod --session "$S" >/dev/null
"$B" verdict set --pr 7 --sha HEAD --level approved --source local --no-status >/dev/null
append() { "$B" ledger --store "$L" append "$1" >/dev/null; }
append "{\"type\":\"test_run\",\"task\":\"T1\",\"command\":\"cargo test\",\"outcome\":\"green\",\"exit\":0,\"duration_ms\":800,\"timeout_secs\":null,\"actor\":\"hook\",\"session_id\":\"$S\",\"worktree\":\"$WT\",\"branch\":\"main\",\"head\":null,\"ts\":\"2026-11-10T12:00:00.25Z\"}"
append "{\"type\":\"mode_set\",\"mode\":\"diagnose\",\"clean\":true,\"plan\":null,\"tree\":null,\"actor\":\"cli\",\"session_id\":\"$S\",\"worktree\":\"$WT\",\"branch\":null,\"head\":null,\"ts\":\"2026-11-10T12:01:00Z\"}"
append "{\"type\":\"subagent_status\",\"agent\":\"rust-implementer\",\"agent_id\":null,\"status\":\"wip\",\"violation\":null,\"missing\":[],\"duration_ms\":null,\"tokens\":null,\"actor\":\"hook\",\"session_id\":\"$S\",\"worktree\":null,\"branch\":null,\"head\":null,\"ts\":\"2026-11-10T12:02:00Z\"}"
"$B" ledger --store "$L" query > "$WORK/bench-ledger-hs060.json"
cp "$L" "$OUT/bench-ledger-hs060.sqlite3"
echo "$WORK"
```
