# The ledger

The ledger records what happened in a repository's sessions: tests run, tasks done, plans
approved, gates and stops blocked. Hooks and commands append to it; gates query it. Since Move
2 step 3 (PR 3.4) it is Bench's share of the suite store, the one Core store every Redkiln
plugin writes (`spec/store.md`), reached through `redkiln-core` from
`crates/bench/bench/src/store.rs`, and driven by `bench ledger`. Bench's events are the 17
`bench.*` types (`bench-contract`). Since step 4 (PR 4.3) `bench ledger` reads and prints
them as Core stores them: the EV-01 envelope (`spec/events.md`), `type` `bench.<name>` and
the payload under `data`. The payload tables below give each type's short name, which is
the part after `bench.`; anneal's 0.x flat form survives only in old ledgers, which the
one-time import reads, and in what the recording commands (`grill confirm`, `plan approve`,
`mode`, ...) print.

## Where it lives

One file per repository, shared by every worktree and every plugin, never committed:
`<git-common-dir>/redkiln/events.sqlite3`. A command resolves it as Core does (ST-01), with
anneal's variable read below Core's:

1. `--store <file>` (relative paths are taken from the working directory);
2. `$REDKILN_STORE` (blank counts as unset);
3. `$ANNEAL_STORE`, which names an anneal ledger: the store goes beside it, as
   `<parent>/<stem>.events.sqlite3`, with a warning (D5 of the step 3 plan);
4. `store` under `[suite]` in `.redkiln/suite.toml`;
5. `<git-common-dir>/redkiln/events.sqlite3`, the same file from the main checkout and every
   linked worktree;
6. `<folder>/.redkiln/events.sqlite3` outside git: only when git says the directory is not in a
   repository. Any other git failure (for example "detected dubious ownership", or a `.git`
   file that points nowhere) stops the command with exit 70, so a repository git refuses never
   gets a second, per-folder store.

Query results report the file in `data.store` and the step that chose it in `data.source`
(`flag`, `env`, `config`, `git_common_dir` or `folder`).

## Old ledgers

anneal's ledger, `<git-common-dir>/anneal/ledger.sqlite3` (outside git
`<folder>/.anneal/ledger.sqlite3`), and the file `$ANNEAL_STORE` names are never opened in
place. `--store` or `$REDKILN_STORE` naming one is refused with exit 70 and the hint that bench
imports an anneal ledger rather than opening it; the file is left as it was. Under `--store`
or `$REDKILN_STORE` no old ledger is looked for: an explicit store is a test or a deliberate
setup, and importing the repository's ledger into it would copy a real ledger into a scratch
store.

### The one-time import (step 3.5, ST-15)

Each old ledger is imported once, read only from Core's copy (`redkiln_core::legacy`; both
happenstance schemas, WAL and rollback). Who imports it:

- SessionStart, within a hook's budget: sources up to 8 MiB together (each ledger with its
  `-wal`), no batch started 2 s after the hook began. A ledger over the budget waits for a
  command, and SessionStart says so.
- Every command that opens the store to record (`ledger append`, `mode`, `task done`,
  `verdict set`, `round start`, `grill confirm`, `repro record`, `migrate`, `plan approve`,
  `orchestrate park|rule`), with no deadline.
- Never `bench status`, which reports a due ledger as a `ledger/old-ledger` finding and
  imports nothing, nor any other hook or read. With nothing due no store is opened or created.
  Only SessionStart, recording commands and `bench status` mention a due ledger: a read
  (`ledger query|tail|journal`, `plan check`) beside one not yet imported shows the store as
  it is, without the old events and without a warning.

Each old event becomes `bench.<name>` (dropped and frozen types included) under its old
identity (`<old store id>:<old position>`), trust `asserted` with the writer's level in
`claimed`. One `core.import_completed` names the source (`{"legacy": {path, store_id}}`) and
its counts; a source so recorded is never read again, and a second run appends nothing. An
import that fails or stops records nothing and is retried; what it stored stays present.

Fields Core cannot take whole (decision D8): an actor Core refuses (anneal took any text,
such as `test`), a session it refuses or a payload no type reads skips the event, reported
by record number; a worktree, branch or head it refuses is dropped and counted, the event
kept; a `ts` it refuses (anneal dropped trailing zeros, `10:04:00.12Z`; a hand-written one may
carry an offset) is normalised through its Unix milliseconds (`10:04:00.120Z`). anneal's
worktree path becomes the ID-06 id this build computes for it, so an imported `mode_set` keys
on the worktree it was set in.

A `plan_approved` whose `by.actor` is `human` is also imported as `core.approval_recorded`
(AP-05; repository scope, no gates, D3), decided by the suite owner as `bench plan approve`
resolves it, under the identity `<first 16 bytes of sha256("bench-approval:" ‖ old store id)>:
<old position>` (D4). With no owner, a model's or an unknown approver, or a hash that is no
`sha256:` subject, only Bench's event is imported, and the report counts it.

The report: a command writes one `bench: warning:` line per source on stderr, then its
details (approvals recorded for bench only, dropped fields, the first 32 skipped records)
indented beneath; SessionStart writes the same lines in its context.

## Storage rules

Core's: the store's journal mode, stamp, refusals of a newer or foreign file, busy waits and
first-writer-wins appends are the SPEC's (ST-04 to ST-11) and Core's tests. What Bench adds:

- **Bench sees only Bench.** Every read and every append condition names Bench's 17 types
  (EV-08), so another plugin's events neither appear in `bench ledger query` nor make a Bench
  write conflict. Positions are the shared store's, so they skip what other plugins wrote.
- **Unusable events are counted, not errors** (EV-10): a `bench.*` event at a later version, or
  one whose data this build cannot read, is skipped; `query` and `tail` add a `ledger/unusable`
  finding with the count, and SessionStart's live state says how many it skipped.
- **Waits.** Commands wait up to 10 s for another writer's lock, hooks up to 2 s (Core's
  `Wait::Command` and `Wait::Hook`).
- **Conditioned appends (DCB).** `Store::append_if` writes only if no Bench event matching a
  condition exists (optionally: after a position the writer has read), checked inside the
  append's transaction; `bench plan approve` appends `plan_approved` only if no other approval
  of the plan landed after the one it read.
- **Plan approvals for the suite.** `bench plan approve --actor human` also writes
  `core.approval_recorded` (AP-01: subject the plan's `sha256:`, scope `repository`, decider
  `human:<owner>`, the owner from `[suite] owner` or git's `user.name`), in the same
  transaction, so other plugins' gates can see it. With no owner only `plan_approved` is
  written, and a `plan/approve/no-owner` finding says so.
- **Reads create nothing.** `query`, `tail`, `journal`, `bench status` and every hook that only
  reads open the store only when it exists.

## The envelope

Every event is one EV-01 envelope (`spec/events.md`): a JSON object with the payload nested
under `data`, as Core stores it. `bench ledger query` and `tail` print it as stored, keys in
EV-01's order and optional ones left out when absent.

| Key | Type | Meaning | `append` |
|---|---|---|---|
| `type` | string | `bench.` and the type's short name, from the registry below | required |
| `version` | integer | The payload version: 1 for every Bench type (EV-06) | optional, must be 1 |
| `ts` | string | When it was written: RFC 3339 UTC with a `Z`, to the second or the millisecond (EV-01); Bench writes seconds, except `bench task done`, which stamps milliseconds as its command ends | filled with now when absent or `null` |
| `actor` | string | Who acted: `cli`, `hook`, `agent`, `ci`, `human:<name>` or `app:<name>` | filled from `--actor` (default `cli`) when absent or `null` |
| `trust` | object | `{"level": "asserted"}` for every event Bench writes (`spec/trust.md`) | refused: Bench writes it |
| `writer` | object | `{"binary": "bench", "version": <the build's>, "spec": "1.0.0"}` | refused: Bench writes it |
| `session` | string, absent | The writing agent session (ID-05) | optional, filled from `--session` |
| `worktree` | string, absent | The writer's worktree id, `wt-` and 12 hex (ID-06); `bench mode`, `task done` and `repro record` set it | optional |
| `branch` | string, absent | The checked-out branch | optional |
| `head` | string, absent | The commit at `HEAD` | optional |
| `origin` | string, absent | The event's identity once it left its first store (ID-02): imported events carry it | refused |
| `cites` | array, absent | Events this one answers (ID-03); Bench writes none | refused |
| `data` | object | The type's payload (below) | required |

```json
{"type":"bench.task_done","version":1,"ts":"2026-10-11T09:30:00Z","actor":"cli","trust":{"level":"asserted"},"writer":{"binary":"bench","version":"0.0.5","spec":"1.0.0"},"session":"s1","data":{"task":"T1"}}
```

Every payload field name is a persisted JSON key: renaming one is a migration. Optional fields
read back from either `null` or a missing key, so adding one is additive. Writing is strict: a
`type` that is not one of the 17 `bench.*` types (a 0.x short name such as `task_done` is
refused with the stored name), a `version` other than 1, a key EV-01 does not define (the 0.x
`session_id` included), a key Bench writes itself (`trust`, `writer`, `origin`, `cites`; `null`
reads as absent), a missing or mistyped payload field, a payload key the type does not define
(named from `data.`), a `ts` outside EV-01's forms or a tag value that is not valid is a schema
error (exit 1) and nothing is written. Reading ignores keys a newer Bench or SPEC added.

## Tags

The store indexes events by `key:value` tags. The keys are a closed vocabulary:

| Tag | Carried by |
|---|---|
| `session:<session>` | every event with a `session` |
| `worktree:<worktree>` | every event with a `worktree` (`mode_set`, `repro_red` and `test_run` are selected by it) |
| `task:<task>` | `test_run` (when it names a task), `task_done`, `repro_red`; `weighin_parked`, `bridge.unavailable` and `orchestrate.ruled` (their `ticket`) |
| `plan:<plan>` | `plan_approved`, `grill_confirmed` |
| `pr:<number>` | `verdict_set`, `round_started` (with a `pr`) |
| `point:<point>` | `gate_blocked`, `stop_blocked` |
| `agent:<agent type>` | `subagent_status` |
| `migration:<id>` | `migrate.wave.started`, `migrate.wave.done`, `migrate.contract` |

A tag value must be non-empty, free of control characters and at most 255 bytes with its key.

## Event registry (version 10)

Each type's minimal payload. Later stories extend payloads with optional fields (bumping the
registry version) or add types. Version 2 added `hash` and `quote` to `grill_confirmed` (#16);
version 3 added `exit`, `duration_ms` and `timeout_secs` to `test_run` (#22, `docs/task.md`);
version 4 added `mode_set` (#23, `docs/preconditions.md`); version 5 added `subagent_status` and
the `agent:` tag (#26, `docs/hooks.md`); version 6 added the levels `tests_verified` and
`reviewed` and the fields `source`, `sha`, `patch_id` and `base` to `verdict_set` (#33,
`docs/verdict.md`); version 7 added `migrate.wave.started`, `migrate.wave.done` and
`migrate.contract` and the `migration:` tag (#35, `docs/migrate.md`), whose dotted names are
the story's; version 8 added `ticket` to `round_started` and made its `pr` optional (#36,
`docs/brief.md`); version 9 added `weighin_parked`, `bridge.unavailable` and
`orchestrate.ruled` (#37, `docs/orchestrate.md`), the last two under dotted names as #35's;
version 10 added the status `wip` to `subagent_status` (temper's `Status: WIP`, Ryan 2026-11-08).

| Type | Payload | Subject tag |
|---|---|---|
| `test_run` | `task?` string, `command` string, `outcome` `green`/`red`, `exit?` integer (`null` when killed), `duration_ms?` integer, `timeout_secs?` integer (set only when a timeout killed the command); `anneal task done` sets the envelope's `worktree` in a git worktree, which the Stop hook selects runs by | `task:` |
| `task_done` | `task` string | `task:` |
| `plan_approved` | `plan` string (the plan's path as given to `anneal plan approve`, `.` components dropped; normally from the repository root), `hash` string (`sha256:<64 hex digits>` of the approved bytes; `docs/plan.md`), `by?` object (#62: who gave it, `actor` `human`/`model`, and through what, `via` `mod`/`bash`/`terminal`, each written only when given; `by` itself only when one is, so an event without them keeps its earlier shape; the key is not `actor`, which is the envelope's writer; never empty, so `"by": {}` does not read; `ledger append` refuses it, see Commands; added without a registry bump, see the decisions log) | `plan:` |
| `grill_confirmed` | `plan` string (for `anneal grill confirm`, the design tree's path as given), `hash?` string (`sha256:` and 64 hex digits of the confirmed tree), `quote?` string (the human's confirmation, verbatim); see [grill.md](grill.md) | `plan:` |
| `repro_red` | `task` string, `test` string (the failing reproduction); `anneal repro record` sets the envelope's `worktree` ([preconditions.md](preconditions.md)) | `task:` |
| `verdict_set` | `pr` number, `verdict` `tests_verified`/`reviewed`/`approved` (levels, lowest first) or `changes_requested`/`noise_only` (review outcomes), `source?` `local`/`review_plugin`/`human`, `sha?` string (the full commit the verdict is for), `patch_id?` string (`git patch-id --verbatim` of `git diff <base>...<sha>`; null for an empty diff), `base?` string (the base ref as given), `by?` object (#62: who gave it, `actor` `human`/`model`, and through what, `via` `mod`/`bash`/`terminal`, each written only when given; `by` itself only when one is, so an event without them keeps its earlier shape; the key is not `actor`, which is the envelope's writer; never empty, so `"by": {}` does not read; `ledger append` refuses it, see Commands; added without a registry bump, see the decisions log); written by `anneal verdict set`, see [verdict.md](verdict.md) | `pr:` |
| `round_started` | `pr?` number (required before version 8), `ticket?` string (registry v8: the ticket of a fix-loop round, written by `anneal round start`), `round` number (from 1); at least one of `pr` and a non-empty `ticket`, else the event does not read; each is written only when set, so a pull request round keeps the version 6 shape; `ledger append` refuses a field the event would not write back, so an explicit `"pr": null` or `"ticket": null` is refused as an unknown field: leave the key out; see [brief.md](brief.md) | `pr:` when `pr` is set; a ticket round has none and is read by type |
| `gate_blocked` | `point` string (the gate), `reason` string | `point:` |
| `stop_blocked` | `point` string (the check that blocked the stop), `reason` string | `point:` |
| `subagent_status` | `agent` string (the subagent's type, `unknown` when the ledger cannot tag it), `agent_id?` string, `status?` `done`/`done_with_concerns`/`needs_context`/`blocked`/`wip`, `violation?` string (what is wrong with its status block, one line), `missing` string array (required sections absent; `[]` when none), `duration_ms?` integer and `tokens?` object (`input`, `output`, `cache_read`, `cache_creation`) from its transcript; written by `anneal hook status` | `agent:` |
| `mode_set` | `mode` `build`/`diagnose`/`off`, `clean` boolean (no change, untracked files included, at entry), `plan?` string and `tree?` string (build: paths relative to the worktree root, `/`-separated); the latest one for a worktree is its mode, see [preconditions.md](preconditions.md) | `worktree:` (from the envelope) |
| `migrate.wave.started` | `migration` string (the migration's id), `wave` integer (from 1); see [migrate.md](migrate.md) | `migration:` |
| `migrate.wave.done` | `migration` string, `wave` integer | `migration:` |
| `migrate.contract` | `migration` string | `migration:` |
| `weighin_parked` | `item_id` string (the weigh-in item, `wi-` and its id), `ticket` string (the ticket the call blocks), `decision?` string (the Clef D2 decision id, written only when set); written by `anneal orchestrate park` once per item and ticket. The decision is weigh-in's; see [orchestrate.md](orchestrate.md) | `task:` (the ticket) |
| `bridge.unavailable` | `ticket` string, `reason` string (why weigh-in could not park the call, one line); written by `anneal orchestrate park`, which then exits 4 | `task:` (the ticket) |
| `orchestrate.ruled` | `ticket` string, `what` string, `why` string, `cost_if_wrong` string, `decision?` string (as `weighin_parked`); written by `anneal orchestrate rule` | `task:` (the ticket) |

## Commands

```text
bench ledger [--store <file>] append '<EV-01 envelope json>' [--actor <who>] [--session <id>]
bench ledger [--store <file>] query [--type <type>]... [--tag <key:value>]... [--session <id>]
bench ledger [--store <file>] tail [-n <N>] [same filters as query]
bench ledger [--store <file>] journal
```

- `append` takes an EV-01 envelope's writable keys (the table above), for example
  `'{"type":"bench.task_done","data":{"task":"T1"}}'`, and fills `actor` (from `--actor`,
  default `cli`), `session` (from `--session`) and `ts` (now) when it leaves them out or sets
  them to `null`; Bench writes `version`, `trust` and `writer`. Its envelope's `data` holds
  `store`, `source`, `position` and `event`, the envelope as stored (read back from its
  position). It refuses a `by` on `bench.plan_approved` or `bench.verdict_set` as a usage error
  (exit 2, one `bench: error:` line on stderr, nothing on stdout, nothing appended and no ledger
  opened): who gave an approval or a verdict is recorded only by their writers, `bench plan
  approve` and `bench verdict set` (`--actor`, `--via`). For `verdict_set` that is where the
  guard's `human-only` rule sees the actor (`docs/guard.md`), so a raw append cannot claim a
  human verdict; `plan approve` is allowed with any actor (wi-720468), so for `plan_approved`
  the refusal only keeps one writer of `by`.
- `query` and `tail` filter by any of the `--type`s and all of the `--tag`s (`--session <id>` is
  `--tag session:<id>`). `--type` takes the stored name (`bench.task_done`); a 0.x short name
  (`task_done`) still filters, with one `bench: warning:` line naming the stored name. Their
  `data` holds `store`, `source` and `events`, each `{"position": n, "event": {...}}` in
  position order, `event` the envelope as stored and `position` the store's (EV-02). `tail`
  keeps the last `N` (default 10).
- `journal` prints every Bench event as its journal line (JR-02: the Core envelope with
  `origin`, `bench.*` names), in position order, with no result envelope: the form Core's
  journal import reads. The export checkpoint at Stop and before a push waits for
  step 6.

Exit codes: 0 ok, 1 schema error (an envelope with one `ledger/schema` finding on stdout and
the 17 types `append` takes in `data.types`: an event the parse or the store refuses, such as
an actor outside EV-01's, a worktree that is not a `wt-` id or a `ts` in an offset), 2 usage
error (`append` with a `by`, above, or an unknown `--type`), and
the global codes in [exit-codes.md](exit-codes.md) (70 when the store cannot be opened or
written).
