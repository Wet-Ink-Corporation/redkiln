# `anneal status`: one read model for every UI

`anneal status` folds the ledger and the anneal home's state files into one document: the
state a person needs to see. A UI (the cockpit mod, a status pane, a script) renders it and
holds no logic of its own.

```text
anneal status [--json] [--fields <section>,...] [--tail <n>]
```

- Without `--json` it prints the text form: one line per section present, then one line for
  any problems, at most 12 lines of at most 200 characters. A control character (line breaks
  and escape sequences among them), U+2028, U+2029 and any other format or default-ignorable
  character is shown as a space, so recorded text cannot break a line or drive the terminal.
- With `--json` it prints the envelope `bench.status/1` (SPEC CLI-12, `docs/exit-codes.md`),
  whose `data` is the document below, read through `bench_contract::status`.
- `--fields phase,plan,gates` limits the work to the sections named (any of `phase`, `plan`,
  `tasks`, `gates`, `verdict`, `clef`, `context`, `ledger_tail`); an unnamed section reads
  nothing, so a frequent refresh can ask only for what it shows. An unknown name is a usage
  error.
- `--tail <n>` is how many events `ledger_tail` lists: 10 by default, 0 to 1000.

`status` is a report (SPEC CLI-06): it exits 0 whenever it wrote its document. When no ledger
exists the document is still written (only `clef` and `context` can have data then), with one
`status/no-store` finding of severity `note`. It never creates a ledger. The
ledger is the one `anneal ledger` resolves (`$ANNEAL_STORE`, else the git common dir, else
`.anneal/` outside git); the worktree is the one git finds from the working directory.

## The schema

The envelope's `schema` is `bench.status/1` (CLI-01); `data` no longer carries a `schema` of its
own (it held `"schema": 1` before step 4.4). **Fields are only ever added within version 1**
(CLI-04): a field, once shipped, keeps its name, type and meaning, and a reader must ignore
fields it does not know. Removing or changing a field is a new version, `bench.status/2`.
`spec/fixtures/status/valid/bench-ok.json` is its golden, and Bench's tests check every
`status --json` against `spec/schemas/status-1.schema.json`.

Every key is always present: a value Bench does not have is `null`, never left out.

### CLI-12's keys

The keys every plugin's status carries come first:

- `plugin`: `rk-bench`; `version`: the binary's; `spec`: the SPEC version it conforms to.
- `store` and `store_source`: the store's path (with `/` separators) and the step that chose it
  (`flag`, `env`, `config`, `git_common_dir`, `folder`). They are given whether or not a store
  exists yet; when git cannot say where it is, `status` fails (exit 70) rather than writing them
  as `null`.
- `store_schema`: the store's schema as `<major>.<minor>`, `null` when there is no store yet.
- `repo_class`: `own` or `client` (ST-13), from the suite config Core read while locating, or,
  when `--store` or a variable chose the store first, from the worktree's (outside git, the
  folder's) `.redkiln/suite.toml`; `client` when nothing sets it. git failing there is a
  `status/unreadable:repo_class` warning.
- `unreadable`: Bench events this build skipped, by stored type (EV-10), from one read of every
  Bench event; when any were skipped, one `ledger/unusable` warning says how many.
- `dropped`: always 0, since Bench records no telemetry (ST-09).
- `health`: from the findings alone: `off` with an error finding, `degraded` with a warning,
  else `ok`. `status/unreadable:*` and `ledger/unusable` are warnings; `status/no-store` and
  `ledger/old-ledger` are notes, so no ledger yet, or an anneal ledger waiting for its import,
  leaves `health` `ok`.

### Sections

A section is `null` when it was not asked for (`--fields`) or its data source does not exist
(no mode recorded, no verdict, no context reading). When its source exists but cannot be read
(a state file that does not parse, git failing), the section is `null` too and the envelope has
one warning `status/unreadable:<section>` saying why; the exit code does not change. A failure to
find the worktree is `status/unreadable:worktree`, and of the anneal home
`status/unreadable:home`. The mode is `phase`'s source, so a mode that cannot be read is
`status/unreadable:phase` even when only `plan` or `tasks` was asked for. `context` has two
sources: when one of them cannot be read, the other's reading is still shown, beside the
`status/unreadable:context` finding.

Positions (`event`) are ledger positions, as `anneal ledger query` prints them; times (`ts`)
are the events' RFC 3339 timestamps.

```json
{
  "plugin": "rk-bench", "version": "0.0.5", "spec": "1.0.0",
  "store": "/work/repo/.git/redkiln/events.sqlite3", "store_source": "git_common_dir",
  "store_schema": "1.0", "repo_class": "own", "unreadable": {}, "dropped": 0, "health": "ok",
  "phase": {
    "mode": "build",
    "playbook": {"name": "build", "requires": ["bench.test_run"]},
    "event": 1,
    "plan": "docs/plan.md",
    "tree": "docs/tree.json",
    "clean": true
  },
  "plan": {
    "id": "docs/plan.md",
    "status": "approved",
    "hash": "sha256:…",
    "approved": {"hash": "sha256:…", "event": 2, "ts": "2026-10-19T10:00:02Z"}
  },
  "tasks": {"done": 1, "total": 2, "next": {"id": "T2", "title": "CLI flag"}},
  "gates": [
    {"id": "preconditions.plan", "kind": "bench.gate_blocked", "reason": "…", "event": 3,
     "ts": "2026-10-19T10:00:03Z"}
  ],
  "verdict": {
    "pr": 7, "head": "<commit>", "base": "main", "outcome": "valid", "valid": true,
    "verdict": {"event": 4, "verdict": "tests_verified", "source": "local", "sha": "<commit>",
                "fresh": "by_sha"},
    "stale": 0
  },
  "clef": {
    "configured": true,
    "points": [{"point": "D1", "answered": 3, "configured_mode": "enforce", "mode": "shadow"}]
  },
  "context": {"used_tokens": 120000.0, "window": 200000, "window_source": "model",
              "percent": 60.0, "level": "warn"},
  "ledger_tail": [
    {"event": 1, "type": "bench.mode_set", "summary": "mode_set build", "actor": "cli",
     "ts": "2026-10-19T10:00:01Z"}
  ]
}
```

(`clef.points` always lists D1 to D7; one is shown.)

### `phase`

The worktree's anneal mode, from its latest `mode_set` (`mode::active`, as the preconditions
hook reads it); `null` with no mode, `off`, or outside a worktree. No ledger event records a
"phase" or "playbook" of its own, so this is the most grounded reading: the mode is the phase,
and its playbook is the Stop hook's rule table for that mode
(`hook::completeness::playbook::RULES`), `requires` naming the events its rules need since the
mode was entered, by stored name (`bench.test_run` for build, `bench.repro_red` for diagnose).
`plan`, `tree` and `clean` are build mode's; they are `null` for diagnose mode and for a build mode recorded without a
plan or tree. Clef D3's routed phase is not shown: it is a per-prompt guess, not state.

### `plan`

Build mode's plan, judged as `anneal plan check` judges it (`plan::approval_record`):
`status` is `approved` (the latest approval has the hash of the plan's current bytes),
`stale` (it has another hash: the plan changed since) or `none` (never approved); `hash` is
the current bytes' `sha256:` hash and `approved` the latest approval, when there is one.
`null` outside build mode; a plan file that cannot be read is a finding.

### `tasks`

Build mode's plan's tasks (read as `lint plan` reads them) done and in all: a task is done
when the worktree has a `task_done` for it since build mode was entered, as the handoff note
counts them (`hook::handoff::facts`). `next` is the first task not done, its title cut to 200
characters; `null` when all are done.

### `gates`

The newest 10 `bench.gate_blocked` (a guard, the preconditions, brief or merge-gate hook denied
a call) and `bench.stop_blocked` (the Stop hook refused a stop) events of the ledger, oldest
first: `id` is the gate as the event names it, `kind` its stored type, with its `reason`,
`event` and `ts`. These are the
gates that recently blocked. The Stop hook's *open* gates (a success claim without a fresh
green run, a `[DEBUG-<id>]` leftover) are not run here: judging them needs the session's
transcript and a scan of the working tree, too slow for a frequent refresh; the handoff note
lists them. The events carry no worktree, so the list is the ledger's, not one worktree's.
`[]` when nothing ever blocked.

### `verdict`

The review verdict for `HEAD`, judged as `anneal verdict valid` judges it
(`verdict::pull_request::read` and `verdict::assess`). Verdicts are recorded per pull request
and nothing maps `HEAD` to one, so the pull request is the one of the newest `verdict_set`
whose commit is `HEAD`, else of the newest `verdict_set` in the ledger (of its latest 64).
`outcome` is `valid`, `noise_only`, `no_verdict`, `stale` or `changes_requested`; `valid` is
true exactly when it is `valid`. `verdict` is the latest verdict still fresh for `HEAD`, and
its `fresh` says why: `by_sha` (set for this commit) or `by_patch_id` (set for an earlier
commit whose `git patch-id --verbatim` from the same base is `HEAD`'s, so a rebase kept it);
`null` when none is fresh, as `base` is when the latest verdict names none. `null` with no verdict in the ledger or an unborn `HEAD`. Each git
call has 2 s.

### `clef`

For each decision point D1 to D7: `answered`, the answered decisions logged for it (the
`decisions/counts.json` the shadow period counts, so rotation of the decision log does not
reset it); `configured_mode` from `clef_mode`; and `mode`, the one in effect: `off` without an
account, `shadow` while a configured `enforce` has fewer than 50 answered decisions, else the
configured one. `configured` is whether an account is set. `null` when no account is set and
no decision was ever answered. Unanswered decisions are not counted: counting them would mean
reading up to 80 MiB of decision log on every refresh (`anneal calibrate` reads it).

### `context`

The newest session's context usage, as `anneal statusline` (`context-usage.json` in the anneal
home) or the mod, through `anneal snapshot write` (`context-usage-mod.json`), last saved it:
`used_tokens` (null before Claude Code reports usage), the auto-compact `window` and where it
came from, `percent` to one decimal, and `level`, the context monitor's threshold reached
(`warn` at 60%, `stop` at 75%, null below). Of the two files' newest readings the fresher is
shown, by the time it was saved, the mod's on a tie (a reading saved before readings carried a
time is older than any that does); the context monitor judges the same way. The files key
sessions by a hash of their id, so the reading is the newest of any session, not necessarily
this one's. `null` until the status line or the mod saves a reading. When one file cannot be
read, the other's reading is shown and the `status/unreadable:context` finding names the one
that could not be.

### `ledger_tail`

The newest `--tail` events of the ledger, oldest first: `event`, `type` (its stored name,
`bench.task_done`, as `bench ledger tail` prints it), `summary` (its short type name and what
it is about, as the handoff note writes it: `task_done task:T1`), `actor` and `ts`.
`[]` for an empty ledger.

## Cost

`--fields phase,plan,gates` reads the latest `mode_set` of the worktree, the latest
`plan_approved` of the plan, the plan file and the newest 10 blocking events: each read asks
the store for at most a few events, newest first. A measurement, reported here and not gated
(release build, Linux container, 2026-10-19; median of 20 runs, wall time of the whole process):

| Ledger | `--fields phase,plan,gates` | all sections |
|---|---|---|
| 12 events | 24 ms | 25 ms |
| 10,002 events | 21 ms | 24 ms |

The time does not grow with the ledger: it is starting the process, opening the store and the
git calls that find the ledger, the worktree and `HEAD`.

Since step 4.4 that no longer holds: CLI-12's `unreadable` reads every Bench event once to
count the ones this build cannot use, whatever `--fields` names, so the time grows with the
ledger. Measured on a debug build (Linux container, 2026-11-11; mean of 5 runs, whole process),
not yet on a release build: `--fields gates` took 57 ms over 2,000 events and 272 ms over
10,000, against 8 ms for `bench ledger tail -n 1` over the same 10,000. Method: a scratch repository in build
mode with an approved plan (`anneal plan approve`, `anneal mode build`), then N events appended
with `anneal ledger append` (one `gate_blocked` in ten, the rest `task_done`), then
`anneal status --json --fields phase,plan,gates` and `anneal status --json` each run 20 times.
