# The CLI envelope

Every Redkiln binary is called by people, by hooks, by sibling plugins and by CI. Its JSON output and its exit codes are therefore a contract, read by code that cannot see its source. This section takes anneal's envelope and exit codes, which already cover 24 commands, and weigh-in's versioned report schema.

## Result envelope

**CLI-01.** A command run with `--json` writes exactly one JSON object on one line to stdout, ended by `\n`, and nothing else on stdout.

**CLI-02.** The object has these keys:

| Key | Type | Meaning |
| --- | --- | --- |
| `schema` | string | `<namespace>.<command>/<n>`, the output's own versioned id (`bench.dag-check/1`, `weighin.open/1`) |
| `ok` | boolean | `true` exactly when the exit code is 0 |
| `command` | string | The command as invoked, without options (`dag check`) |
| `data` | object | The command's result, defined by its schema |
| `findings` | array | Problems the command found (CLI-03); `[]` when none |
| `proves` | string or absent | What a green result proves |
| `does_not_prove` | string or absent | What a green result does not prove |

A command whose result a gate or CI reads states `proves` and `does_not_prove`; others may leave them out. `ok` and the exit code come from one value, so they cannot disagree (anneal `envelope.rs`).

```json
{"schema":"bench.plan-check/1","ok":false,"command":"plan check","data":{"plan":"docs/plans/retention.md","approved_hash":"sha256:9f86…","current_hash":"sha256:2c26…"},"findings":[{"key":"plan/stale:docs/plans/retention.md","message":"the plan changed since its approval"}],"proves":"the plan is approved and unchanged since","does_not_prove":"that the plan is correct or complete"}
```

**CLI-03.** A finding is `{key, message}`, with optional `severity` (`error`, `warning`, `note`) and `at` (a worktree-relative path with an optional `:line` or `:line:col`). `key` is stable across runs, so a baseline can tell old findings from new; `message` is one line.

**CLI-04.** An output schema evolves like an event type (EV-06): additive within a version, a new version for anything else, and a golden fixture for each version. A report released before SPEC 1.0 keeps its shape under its existing id (`weighin.ci-check/1`, which Review's adapter reads) and adopts the envelope at its next version.

## Exit codes

**CLI-05.** Only success exits 0. Each command documents its own codes in a table generated from its source, with what a green result proves and does not prove (anneal `docs/exit-codes.md`). These codes mean the same for every command:

| Code | Meaning | stdout |
| --- | --- | --- |
| 0 | Success | The envelope |
| 2 | Usage error: bad arguments | Nothing |
| 64 | Not implemented yet | The envelope |
| 70 | Internal error: the command could not run (a refused store, a git failure, a bug) | The envelope when it can be written |
| 74 | Output error: stdout or stderr could not be written | Nothing usable |

Codes 1 and 3 to 63 are each command's own (1 is "cycle" for one command and "test red" for another). A new command never gives 2 a meaning of its own, because Claude Code reads a hook's exit 2 as "block". The one sanctioned exception is a guard that cannot write its deny decision (CLI-10): there exit 2 is the block it means. anneal gives 2 a command-specific meaning in a dozen of its exit-code tables (`ledger`, `task done`, `repro`, `dag`, `floor-guard`, `watch`, `rollout`, `brief`, `verify risk`, `eval`, `status`). Each of those writes an envelope on stdout, which tells it apart from a usage error, and Bench renumbers them before `rk-bench` 0.1.0, its first release.

**CLI-06.** Whether findings fail a command is the command's to state. A gate exits non-zero when its findings fail it; a report (`ci-check`, `status`) exits 0 whenever it wrote its report, and its findings are data for the caller to judge (weigh-in `ci.rs`).

**CLI-07.** When stdout is closed by its reader (a broken pipe, as in `weighin open | head`), the command exits 0 and writes nothing to stderr: the reader chose to stop. Any other output failure exits 74. This follows weigh-in; anneal exits 74 for both, and the SPEC prefers not to fail a pipeline its reader cut short.

**CLI-08.** Errors and warnings go to stderr, one line each: `<binary>: error: <message>` and `<binary>: warning: <message>`, the message holding the cause chain joined by `: `. A usage error names every missing argument at once.

## Hooks

**CLI-09.** A hook is `<binary> hook <event>`, called by path in exec form from `hooks.json` (`${CLAUDE_PLUGIN_ROOT}/libexec/<binary>`), never through a shell. Hooks fail open: a hook notes its failure in its output, or in one stderr line when stdout cannot be written, and exits 0, even on a bug. Its usage errors still exit 2, so a test runs every `hooks.json` entry's arguments through the binary.

**CLI-10.** A guard fails closed. It denies what it cannot judge with a deny decision on stdout and exit 0; when it cannot write that decision, it exits 2 with one stderr line, so Claude Code blocks the call.

**CLI-11.** With no build of a plugin's binary present at all, its launcher lets every hook through, guards included, and says once at SessionStart that the plugin is off (the shared launcher, `tools/launcher/README.md`).

## Status

**CLI-12.** Every plugin binary has `status --json`, whose schema is `<namespace>.status/1` and whose `data` holds at least:

| Key | Meaning |
| --- | --- |
| `plugin` | The install id (`rk-weigh-in`) |
| `version` | The binary's version |
| `spec` | The SPEC version it conforms to |
| `store`, `store_source` | The store and the step that chose it (ST-03) |
| `store_schema` | The store's schema as `<major>.<minor>`, or `null` when it was refused |
| `repo_class` | `own` or `client` as Core resolved it (ST-13) |
| `unreadable` | Events of this plugin's own types it skipped, by type (EV-10) |
| `dropped` | Telemetry events dropped on a busy lock since the count was last reported (ST-09) |
| `health` | `ok`, `degraded` or `off` |

The suite's status view reads every installed plugin's `status --json`, so a plugin adds its own keys beside these, never in place of them.
