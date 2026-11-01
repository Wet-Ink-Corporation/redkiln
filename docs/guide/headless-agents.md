# Headless agents

Review runs reviewers with `claude -p`, Loop runs every stage that way, and the blueprint's `redkiln-dispatch` kit will own the supervisor. assay and redkiln-rs each built a hardened runner independently; they agree on almost everything, and between them they met every trap below by observation. The harness-agnostic proposal suggested a model runner behind a port (not yet adopted; the merged rule in `AGENTS.md`, CLI first, harness second, covers the plugin edge only); these guardrails are what the Claude adapter behind that port has to do.

## Guardrails

### HA-1 Build the child's environment on purpose, and refuse conflicting credentials

The child environment is a deliberate construction, and only the composition root reads the process environment. If a variable that would change billing or identity is present (`ANTHROPIC_API_KEY`, `_AUTH_TOKEN`, `_BASE_URL`, provider switches `CLAUDE_CODE_USE_*`, `GITHUB_TOKEN`/`GH_TOKEN`), construction fails with a typed error, even when the value is empty; it is never silently stripped. Names compare case-insensitively, as Windows does. `Debug` prints names, never values.

- **Why.** In `-p` mode `ANTHROPIC_API_KEY` beats the OAuth token, so runs would bill at API rates unnoticed. `NODE_OPTIONS` loads code, `NODE_EXTRA_CA_CERTS` behind a proxy can intercept the token, and `CLAUDE_CODE_PLUGIN_DIRS` loads mods. A user-scope `ANTHROPIC_FOUNDRY_API_KEY` once turned redkiln-rs's suite red.
- **The open choice.** assay builds from nothing (an allowlist against a pinned CLI); redkiln-rs inherits everything minus the denylist, because Claude Code reads undocumented variables (proxy, locale, TERM) and an allowlist broke it. See [Tension T1](README.md#tensions-with-the-plan).
- **Evidence.** assay `crates/assay-runner/src/headless/env.rs`, decision 0007, #149; redkiln-rs `crates/rk-adapter-claude/src/env_denylist.rs`, #11.

### HA-2 Prove every flag from the child's own `system/init`

Read the `system/init` event and check that what you asked for is what the child got: the tool set (by set equality), MCP servers, plugins, skills, agents, `plugin_errors`. "An ignored flag is indistinguishable from an honoured one."

- **Why.** `--agents <card>` does nothing without `--agent <name>` (a canary card answered `BANANA` with both and `4` with one). A plain `claude -p` loaded 12 MCP servers and about 280 tools from the operator's environment, 56% of every cache write. `blockReadsOutsideWorkingDirectories` at the wrong nesting level is silently ignored. Tool allowlists do not remove Bash: `Bash(rg:*)` allows `rg --pre <cmd>`, and `git show --output=<path>` writes files; only `--tools Read,Grep,Glob` removes the tool.
- **Enforce.** Canaries per behaviour you rely on: a hook canary at user, project and in-tree scope; a fence canary (`Read //proc/self/environ`); mod and `AGENTS.md` canaries that write a split canary word. "Clean" is an allowlist pinned per CLI version, because `--safe-mode` still loads built-in mods, skills and agents.
- **Evidence.** redkiln-rs F7, F8, F17; assay `docs/architecture/03-contracts.md` §8, `crates/assay-runner/src/headless/canary.rs`, `tests/fixtures/plugins-2.1.296.toml`, `05-security.md` T2.

### HA-3 Exit 0 is not a completed structured dispatch

A structured stage is complete only when the structured payload is present and validates. Never scrape JSON out of prose, and never treat `subtype: "success"` as success on its own.

- **Why.** A card whose tool allowlist omits `StructuredOutput` silently disables `--json-schema`; the CLI still exits 0 with `success`, and the model writes made-up JSON in prose with the wrong field names.
- **Evidence.** redkiln-rs F20, `runbook/halts.md:24-30` (Halt 3).

### HA-4 The stream's `result` decides, and stray lines cannot launder an ending

Parse stream-json tolerantly: skip kinds you do not know, keep only what decides (`result`, rate-limit events, assistant errors). A budget, turn or quota ending is final whatever else the transcript holds; an unreadable line only stops a success from counting; the exit status is not trusted. Keep a fixture per outcome per CLI version, and prefer captured fixtures to ones written from the schema.

- **Evidence.** assay `crates/assay-runner/src/stream.rs`, `tests/fixtures/stream/README.md` (only `init_2_1_296.jsonl` is captured; the rest are synthetic, an open gap).

### HA-5 Cost is charged at the cap when it is unknown

`total_cost_usd` exists only in the final `result`, is a client-side list-price estimate, and is lost on SIGTERM (exit 143). `--max-budget-usd` can overshoot by one response. So the supervisor never kills on its own accounting: the hard stops are the CLI's caps and a wall clock, and an unknown cost (timeout, kill, capped output) is charged at the reservation. With `--resume`, the reported total includes earlier runs; store the delta.

- **Evidence.** assay `docs/architecture/06-operations.md:8-16`, fixture `budget_exhausted.jsonl` (0.7512 against 0.74); Agent SDK cost tracking (prior-art/agents.md §3).

### HA-6 Headless isolation is chosen per auth mode and checked by canary

Isolate the child from the operator's environment: no inherited hooks, MCP servers or plugins unless the run asks for them (`--strict-mcp-config`, explicit `--settings`, `--agents`, `--plugin-dir`). Anthropic recommends `--bare` for scripted runs and says it will become the default for `-p`; `--bare` also ignores the subscription OAuth token. So the isolation flags depend on how the run authenticates, and a canary checks both isolation and billing on every CLI pin move.

- **Why.** Without isolation, `-p` runs a project's hooks and `.mcp.json` servers even in an untrusted folder, which matters when the folder is a hostile PR. Settings hooks fire in headless children (redkiln-rs F2).
- **Evidence.** assay decision 0007, `03-contracts.md` §8; redkiln-rs F2, F8; [Run Claude Code programmatically](https://code.claude.com/docs/en/headless). See [Tension T2](README.md#tensions-with-the-plan).

### HA-7 Pin the CLI version, key version-sensitive argv by it, and refuse an unknown pin

Version-sensitive arguments live in a table keyed by the pinned CLI version (`PERMISSION_MODE_ARGV = [("2.1.303", "manual")]`); a pin with no row is refused, with no fallback. A help-capture test pins each row to a recorded `--help`. A floor (`>= 2.1.251`) is not a match. A launcher records the binary's hash before and after a run to detect a move. Data sets that span a version boundary are reported per version.

- **Why.** `--permission-mode default` became `manual` by 2.1.303. `DISABLE_AUTOUPDATER=1` stops only automatic updates: a manual update moved the pin from 2.1.289 to 2.1.303 mid-stage.
- **Evidence.** redkiln-rs D21-164, `@ea33bcf`, `crates/rk-adapter-claude/src/command.rs:134-177`, `runbook/halts.md:10-22`.

### HA-8 Schemas you send are checked against what the CLI accepts

`--json-schema` rejects draft 2020-12's `$schema` keyword, which every `schemars` export carries, and `oneOf` is outside the structured-output contract (schemars emits it for internally tagged enums; `#[serde(untagged)]` emits `anyOf`, which a grep for `oneOf` misses). Strip what the CLI refuses from the argv copy only, validate the return against the full export, and run every exported schema through the transform in a test.

- **Why.** The first real Loop dispatch, after two months of fake-backed tests, failed at start-up on `$schema`. The live probes had used hand-written schemas without it.
- **Evidence.** redkiln-rs D21-167, `@34c36fe`, ADR-0095.

### HA-9 The prompt goes on stdin from a file, and the process group dies on every path

Linux caps one argv string at 128 KiB, which a context pack can exceed; stdin from an anonymous file cannot be mistaken for an option and no writer can block. The child leads its own process group (a Job Object on Windows), the whole group is killed on every exit path, and the call returns only once the group is shown gone. On POSIX, `kill(-pgid, 0)` succeeds while any member is an unreaped zombie, so emptiness is read from a zombie-filtered process list. Send SIGINT before SIGTERM so the CLI can record its result. Report `may_have_run()` so the caller knows whether to charge.

- **Evidence.** assay wi-9fe37e, `crates/assay-runner/src/headless.rs:40-115,197-223`; redkiln-rs `@d2b6a86`, #20; anneal `crates/anneal-cli/src/process.rs` (a grandchild holding a pipe is a timeout); prior-art/platform.md §6 (tokio `kill_on_drop` kills only the direct child).

### HA-10 Agents leave the sandbox when the task gives them a motive

Assume an agent with tools will follow any absolute path, object store or credential it can reach if the task rewards it. Fixtures make the answer unreachable, not merely absent; absolute spellings of the operator's paths are stripped and refused before spawn; the prompt says plainly what the agent cannot do and that an outside read voids the record.

- **Why.** In E3 a reviewer with Bash checked out the hidden fix from the worktree's object store, reviewed the real code and approved. In S-BIN a reviewer followed an absolute path in `CLAUDE.md` into the operator's real repository; the pass ($16.07) was discarded.
- **Evidence.** redkiln-rs F17, D21-153, D21-143.

### HA-11 Every paid step has a numeric cap, a go-ahead, and refuses to re-run

Unattended spend is capped in currency per step, run in the foreground with a spend table, and refuses a re-run before any spawn. Discarded spend counts against the cap. Validate one item before paying for the set.

- **Why.** "Uncapped, a failing attempt was retried 14 times." Validating one item first "paid for itself twice in one sitting".
- **Evidence.** redkiln-rs D21-105, D21-166, F20; this project's rule that paid runs need Ryan's go-ahead.

### HA-12 An unattended wait is a durable mark and a later read

Never hold a process open to wait. Write a timestamped mark, end, and read it later; results are resumable per unit of work. Anything that must survive lives outside the harness's job tree.

- **Why.** Background jobs in the harness were killed at random ages (2 to 65 minutes); a detached runner survived 12,000 s while about 14 watchers were killed around it.
- **Evidence.** redkiln-rs `spikes/FINDINGS.md:868-930`, F22, Halt 5.

## Patterns worth copying

- **Fan-out reports "N of M produced; K degraded".** The Node Redkiln's `reconcile` replaced `.filter(Boolean)` so a null agent result is named, not silently dropped; the inversion RFC called it "the single most reusable thing in the repo" (redkiln-old `workflows/_lib/forge-lib.js:52,86-90`).
- **Validate the seed before any fan-out.** Workflow arguments arrived as an object or a JSON string depending on the launch path; unparsed, `slug` became `undefined` and nine subagents spent 361k tokens writing to `.bklg/undefined/` (redkiln-old `references/2026-07-20-initiative-workflow-fail-fast.md`).
- **Pin model tier and reasoning effort at every dispatch layer; an omitted effort is not a default.** Otherwise "the cost and quality profile of a run was a property of the caller" (redkiln-old `AGENTS.md:90-170`; redkiln-rs F15: effort applies per dispatch).
- **Headroom in integer ticks from `rate_limit_event`.** Utilisation is quantised to 0.01, so throttle on visible windows with a two-tick margin and never credit headroom from a cache metric (redkiln-rs ADR-0110). A `setup-token` token cannot read plan utilisation, so "pause at 70%" is not implementable (assay A-13).
- **Spend ranks volume, then cache, then tier.** Output-token volume varied 180x across runs; cache state swings about 12.5x; tier is pinned because it is free to check, not because it is where the money is (redkiln-old `AGENTS.md:73-88`).
- **The prompt cache lives about an hour** and half of human gates outlive it; a resumed gate costs a bounded 18.6k tokens. Worktrees fragment the cache (redkiln-rs F10, F13, F16, F18).

## Gotchas

- **Review and authoring share one subscription window.** assay's first unattended run paused at 90% of the weekly limit; a daily dollar cap cannot protect the window, because a burst of agent PRs is a burst of reviews (assay `docs/handoff-2026-10-12.md:3`).
- **The Workflow runtime's resume journal keyed on prompt and options**, so re-dispatching review after a fix replayed the pre-fix verdict (redkiln-old `workflows/forge-implement.js:2068-2071`). Any memo of a model call must key on everything it reads.
- **Agent sessions have limited GitHub access**: GraphQL and user endpoints blocked, the environments API unreachable, merges sometimes refused by the classifier (assay `docs/handoff-2026-10-12.md`).
- **Background agents stall silently on protected-file prompts** (`.github/`, settings, `AGENTS.md`, `Cargo.toml`); batch those edits or hand them over as diffs (assay `docs/handoff-2026-10-12.md`, "Protected files").
- **Halt, do not repair, shared git state.** A subagent tried a guessed raw rewrite of a NUL-filled `.git/config` shared by ten worktrees (redkiln-old `@78c29dba`).

## Open problems

- **No real agent step has completed through `redkiln run`** as of `b3b28a2` (redkiln-rs D21-168).
- **Budget admission is not atomic in Loop**, and its throttle is per process, so a resumed run's first dispatch skips the check (redkiln-rs #9, #21, #47). assay's admit, reserve and release jobs are the model to copy ([State and storage](state-and-storage.md)).
- **Orphaned `claude` processes** after a SIGKILLed supervisor: `setpriv --pdeathsig` or accept until caps stop it (assay #154).

## Prior art

- The stream-json contract: the last line is `result`; `system/init` lists `plugins`, `plugin_errors` and MCP servers; `system/api_retry` carries a closed error category. **Adopt**: fail on non-empty `plugin_errors`, and map `api_retry` into the infrastructure-failure class. [Run Claude Code programmatically](https://code.claude.com/docs/en/headless)
- `--permission-prompts none` turns would-be prompts into `permission_denials`. **Adopt** for unattended lanes so they cannot hang. [Headless docs](https://code.claude.com/docs/en/headless)
- Anthropic's research system uses resumable agents and "rainbow deployments" so a running agent keeps its version. **Adopt**: pin a lane to the plugin and binary version it started with. [Multi-agent research system](https://www.anthropic.com/engineering/built-multi-agent-research-system)
- Frameworks "can add abstraction layers that hide prompts and responses". **Agree**: supervise `claude -p` directly with recorded prompt bundles. [Building effective agents](https://www.anthropic.com/engineering/building-effective-agents)
