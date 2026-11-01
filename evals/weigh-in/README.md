# Eval seed for marker capture (#22)

The design's slice 1 exit criterion is marker capture precision of at least 80% on a labelled
seed ([v1 plan](../docs/v1-plan.md), Phase 3). `crates/weighin/tests/it/eval_seed.rs` scores a
seed by running each item through the real capture path in a scratch repository: a `Stop` hook for
`channel: marker`, a `PostToolUse` Write of a markdown file for `channel: scan`.

- **precision** = captured items labelled `capture` / captured items
- **recall** = captured items labelled `capture` / items labelled `capture`
- **status** = captured `capture` items whose first ledger item has `expect.status`

```sh
cargo test -p weighin --test it eval_seed -- --nocapture
```

## Format

One JSON object per line:

```json
{"id":"s2-01","session":"s2","source":"<session id>","repo":"Wet-Ink-Corporation/weigh-in",
 "origin":"real-message","channel":"marker","text":"<the assistant message, verbatim, trimmed>",
 "label":"capture","expect":{"status":"raised"},"why":"<one line>","by":"claude-draft","on":"2026-10-03"}
```

- `label`: `capture` (a decision the ledger should hold, open or as a logged default) or `skip`
  (noise, or no decision).
- `expect.status`, for `capture` items: `raised` (stays open) or `defaulted` (a judgment-bound
  two-way call the agent acted on).
- `origin`: `real-message` (an assistant message as written), `scripted-block` (a block written onto
  a real message), or `scripted-marker` (a test marker from an install check).
- Text is redacted: no client names, emails, tokens or personal data.

## Adding and reviewing items

`scripts/seed.py` (wi-100d6d, #170) does both, with the standard library only (Python 3.9+):

```sh
python3 scripts/seed.py save ~/.claude/projects/<project>/<session>.jsonl --source session_<id>
python3 scripts/seed.py review     # label the `claude-draft` items; r/d/s/k/q per item
python3 scripts/seed.py selftest
```

`save` appends one `by: claude-draft` item per end-of-turn assistant message the seed does not
hold yet, as the next session (`s<n>`): `capture` when the message carries a `weigh-in` block
(`defaulted` for a two-way judgment block, else `raised`), `skip` otherwise. Read what it
appended: relabel prose calls the message made without a block, and redact. `review` records
each label you give as `by: ryan` and rewrites only the lines it changed.

## Files

- `seed-scripted.jsonl`: provisional. None of the five earlier sessions it draws on carried a real
  block (capture was off in one, #38), so its precision items are blocks written onto real
  messages, and its precision follows from how many noise blocks were written. The test pins its
  counts, so a change to what capture raises or defaults shows up.
- `seed.jsonl`: the real seed, which the 80% bar applies to
  (`real_seed_meets_the_precision_bar`). Each item is the last message of a turn, verbatim, from
  a Claude Code cloud session on this repository that ran with the marker convention (0.5.0 and
  later), read back from the session's transcript; with the ledger gone with the container
  (D-0014), the transcript is the only record. `by: claude-draft` until Ryan has reviewed the
  label, then `by: ryan`.

  From 2026-10-05 each session adds its own turns before it ends (wi-48657b, `CLAUDE.md`),
  parsed from its transcript under `~/.claude/projects/`, because a session can leave the
  sessions API before a later one reads it back (the 0.5.2 release session did).

  Drafted 2026-10-04 from three sessions (`s3`, `s5`, `s6`; 51 turns since 0.5.0): 25 items, 7
  labelled `capture`. Capture raises one of them, the only turn that carried a block, so
  precision is 100% on one captured item and recall is 14%. The test passes, but the exit
  criterion counts only once the seed holds at least 10 captured items (wi-a9df17): blocks from
  #23's weeks are added here as they come.

  Added 2026-10-05: three end-of-turn messages (`s7`) from the 0.5.2 release session, all true
  negatives. That session started before the 0.5.2 convention merged, so it counts as before 0.5.2.
  Only its last page of turns could be read: the session was gone from the API before the rest
  were fetched. 28 items, 7 labelled `capture`, 1 captured, so recall is still 14%. No
  session has yet run with the 0.5.2 convention line (one block per call), so recall after 0.5.2
  is not measured.

  Added 2026-10-05: four turns (`s8`) from the first session on the 0.5.2 convention, saved by
  the session itself. One carries a block and is captured; three are true negatives. The seed is now 32 items, 8
  labelled `capture`, 2 captured (precision 100%, recall 25%). After 0.5.2, recall is 1 of 1 on one
  session, which is too few to judge the new line.

  Added 2026-10-05: one turn (`s9`, `rs9-01`), the hand-off message of the session that followed
  s8. Before that turn the session had no finished turns, so it wrote the message into the seed
  before sending it, word for word, and checks it against its transcript after the sitting. It
  carries a block and is captured as a logged default. No other cloud session on this repository
  since 0.5.0 was missing from the seed, and s3, s5, s7 and s8 had left the sessions API, so none
  could be read again. The seed is now 33 items, 9 labelled `capture`, 3 captured (precision 100%,
  recall 33%). Before 0.5.2 recall is 1 of 7; after it, 2 of 2 (s8, s9). Both are
  messages that report on the seed itself, so they say little about the line on ordinary work.

  After the sitting, the session added its other five end-of-turn messages (`rs9-02` to
  `rs9-06`), parsed from its transcript, and checked `rs9-01` against it (word for word). One
  carries two blocks and is captured as a logged default; four are true negatives. Messages
  written mid-turn, before a tool call or a worker restart, are not end-of-turn messages and are
  left out. The seed is now 38 items, 10 labelled `capture`, 4 captured (precision 100%, recall
  40%). After 0.5.2, recall is 3 of 3 across s8 and s9.

  Later the same day: six more end-of-turn messages from s9 (`rs9-07` to `rs9-12`), all true
  negatives (the sitting's close, CI and merge notices, a check result). The seed is 44 items, 10
  labelled `capture`, 4 captured (precision 100%, recall 40%).

  Added 2026-10-06: nine more end-of-turn messages from s9 (`rs9-13` to `rs9-21`). One carries a
  block (dropping a planned fix after a trace) and is captured as a logged default; eight are true
  negatives. The seed is 53 items, 11 labelled `capture`, 5 captured (precision 100%, recall 45%).
  After 0.5.2, recall is 4 of 4 across s8 and s9.

  Added 2026-10-06: six end-of-turn messages from s10, the temper remediation session (PR #82),
  saved by the session itself (`rs10-01` to `rs10-06`). Three carry blocks: calls the agent acted
  on, plus one deferral, so they are captured as logged defaults. Three are true negatives. The
  seed is 50 items, 13 labelled `capture`, 7 captured (precision 100%, recall 54%). After 0.5.2,
  recall is 6 of 6 across s8, s9 and s10.

  Together, s9's and s10's items make the seed 59 items, 14 labelled `capture`, 8 captured
  (precision 100%, recall 57%). After 0.5.2, recall is 7 of 7 across s8, s9 and s10.

  Added 2026-10-06: one turn (`s11`, `rs11-01`), the hand-off message of the session that ran
  #26's code review and opened #84 to #87. The session had no finished turns before it, so, as
  s9 did, it wrote the message into the seed before sending it, word for word. The message
  carries four blocks: two authority questions, a call and a deferral. It is captured, and its
  first item stays raised. The seed is 60 items, 15 labelled `capture`, 9 captured (precision
  100%, recall 60%). After 0.5.2, recall is 8 of 8 across s8 to s11. One more captured item
  brings the 80% bar into force (wi-a9df17).

  Added 2026-10-07: ten end-of-turn messages from s12, the overnight review session (#90 to
  #96), saved by the session itself (`rs12-01` to `rs12-10`). Two carry one block each (calls on
  how a decision is written back into source files) and are captured as logged defaults. One
  (`rs12-03`) states two calls in prose with no block (two review findings kept as they were) and
  is a miss. Six are true negatives (status notes and CI acknowledgements; the session ended
  about 50 turns of that kind, and only a share is kept). The last, `rs12-10`, is the hand-off,
  written into the seed before it was sent, word for word; it carries 21 blocks and its first
  item stays raised. The seed is 70 items, 19 labelled `capture`, 12 captured (precision 100%,
  recall 63%). With 12 captured items the 80% bar is now in force (wi-a9df17), and it passes.
  After 0.5.2, recall is 11 of 12 across s8 to s12.

  Added 2026-10-08: two end-of-turn messages from s13, the status-audit session, saved by the
  session itself (`rs13-01` and `rs13-02`). The first is a status note while a workflow ran and is
  a true negative. The second is the audit hand-off, which carries one block (a two-way call on the
  headline progress figure) and is captured as a logged default. The session's other turns were a
  plan-mode question and close-off steps; the close-off message itself is not saved. The seed is
  72 items, 20 labelled `capture`, 13 captured (precision 100%, recall 65%).

  Added 2026-10-09: five end-of-turn messages from s14, the session that keeps cloud sessions'
  pilot evidence in committed journals (B-21), saved by the session itself (`rs14-01` to
  `rs14-05`). Three are status notes and true negatives. One is a status note that should have
  carried a block for a call it made (the weekly routine's schedule), and is a recall miss. The
  hand-off carries two blocks and is captured as a logged default. Its text says `s13`, because it
  was written before #102 took that number. The seed is 77 items, 22 labelled `capture`, 14
  captured (precision 100%, recall 64%).

  Reviewed 2026-10-11: Ryan checked every label in the seed, then 83 items (24 labelled
  `capture`, 59 `skip`), and agreed with all of them, `expect.status` included, so each is now
  `by: ryan` (#22). On the reviewed labels precision is 100% (14 of 14 captured) and recall 58%
  (14 of 24). Items added after this are `by: claude-draft` again until he reviews them.

  Added 2026-10-12: seven end-of-turn messages from s17, the session that walked Ryan through his
  v1 checks, saved by the session itself (`rs17-01` to `rs17-07`). One carries a block (a deferral
  of the Cowork check the agent acted on) and is captured as a logged default; six are true
  negatives (check instructions and results). The seed is 90 items, 25 labelled `capture`, 15
  captured (precision 100%, recall 60%).

  Added 2026-10-13: ten more end-of-turn messages from s17 (`rs17-08` to `rs17-17`): the 10-12
  wrap-up and the guided Cowork and Windows checks that found #126 and #127. All are true
  negatives. The seed is 100 items, 25 labelled `capture`, 15 captured (precision 100%, recall
  60%).

  Added 2026-10-19: five end-of-turn messages from s21, the AFK slice 2 session (`rs21-01` to
  `rs21-05`). Two carry blocks: one has three calls (a fresh ledger lists a logged default first),
  and one is the Actions-outage question. Three are true negatives. The seed is 124 items, 32
  labelled `capture`, 17 captured (precision 100%, recall 53%).

  Reviewed 2026-11-01: Ryan reviewed the 78 drafts added since 2026-10-11 (`rs17` to `rs23`),
  taking a suggested label for each, so every item is now `by: ryan`. Seventeen prose captures
  that had no `expect.status` got one, and three drafts labelled `skip` became `capture`
  (`rs17-17`, `rs23-03`, `rs23-08`): each made a call or a deferral in prose. The seed is 161
  items, 50 labelled `capture`, 17 captured (precision 100%, recall 34%).
