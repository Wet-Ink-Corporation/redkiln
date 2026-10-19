# Audit: anneal (Redkiln Bench-to-be)

*2026-10-13. Read from `/home/claude/anneal` at `0d0a469` (main, 2026-10-12 20:32 -0700) and GitHub `Wet-Ink-Corporation/anneal` (68 issues, PRs #49 to #113). Evidence tags: [measured] = counted or run here, [read] = stated in code/docs/issues, [inferred] = my reading. Paths are relative to `anneal/` unless prefixed.*

## 0. Headline findings

1. **Nothing is a stub any more.** All 24 top-level commands dispatch to real code (`crates/anneal-cli/src/cli.rs:283-332`); `ship`, `watch`, `rollout`, `perf`, `migrate` were built in PRs #98 and #101 on 2026-10-12 [read]. Only a dead `CliError::NotImplemented` variant and the exit-64 row remain (`src/error.rs:46`, `docs/exit-codes.md:30`), slated for removal by wi-0c6a0b [read]. `suite-structure.md:207` ("anneal had stubs (`ship`, `watch`, `rollout`, `perf`)") is out of date.
2. **anneal is far bigger than "an in-session referee".** It holds 63k lines of non-test Rust [measured] in one 60k-line `anneal-cli` crate. Inside it are six or seven separable products: hooks/guards, the Clef judgement client plus calibration, a plugin-authoring kit (skill lint and three eval tiers), a GitHub PR/CI toolkit (verdict, watch, merge-gate, skip-rule), a ticket DAG and brief toolkit for fan-out, define-phase lints, and the ledger.
3. **The ledger is already 90% of Redkiln Core's event layer, and it is a near-copy of weigh-in's.** It uses the same envelope keys, the same TRUNCATE-via-template open, the same `user_version = 1` stamp and happenstance 0.3.2. The details differ in ways that matter for a single shared file (§6.1).
4. **A single shared `events.sqlite3` breaks anneal as written.** Unfiltered reads (`ledger query`/`tail`/`journal`, the bootstrap tail) decode *every* row strictly and fail on another plugin's event types (`crates/anneal-ledger/src/store.rs:474-479`) [read]. Event type names are unprefixed (`test_run`, `plan_approved`), and the worktree tag values disagree with weigh-in's (§8).

## 1. Identity

- **Job (one line):** a Claude Code plugin that turns prose SDLC phase gates (grill → spec → plan → build/diagnose → verify → ship) into enforced checks: model-invoked skills, a deterministic Rust CLI behind 14 hooks, and model "judgement calls" (Clef D1–D7) that log in shadow until calibration lets them enforce (`README.md:1-9`, issue #1) [read].
- **Maturity** [measured unless noted]:
  - Version `0.0.1` (`Cargo.toml:14`, `plugin/.claude-plugin/plugin.json:3`), released 2026-10-12 (CHANGELOG `## 0.0.1`, PR #93). No tags were fetched locally, but issue #1's comment says v0.0.1 is released [read].
  - A large `## Unreleased` section (CHANGELOG lines 5-35) covers Releases 2 to 6 of the plan.
  - 44 commits, all squash-merged PRs, built from 2026-10-10 21:34 to 2026-10-12 20:32 (about 47 hours, mostly an AFK agent run per issue #1's comment). Pack size 2.1 MiB.
  - LOC: Rust 102,887 (63,041 non-test, 39,943 test). Markdown 11,319 (`docs/` 4,163; skills and references 1,014). JSON 3,633, YAML 305, shell 135, TOML 292.
  - Tests: 1,877 `#[test]` attributes (57 in `anneal-ledger`, 402 in `crates/anneal-cli/tests/it/` across 49 files, the rest unit tests). There are also 27 Tier-3 eval cases (`evals/uplift/**.json`) and 7 routing fixtures.
  - Last activity: open PR #113 (guard redaction), updated 2026-10-13 04:38Z.
- **Distribution** [read]:
  - It is its own marketplace: `.claude-plugin/marketplace.json` lists one plugin at `./plugin`.
  - A tag `v*` runs `.github/workflows/release.yml`. That workflow checks versions and the changelog, reuses ci.yml, builds 4 targets, and publishes a GitHub release (zip, raw binaries, SHA256SUMS). It then **force-pushes an orphan `marketplace` branch** holding the tag's tree plus binaries (`release.yml:101-127`, PR #111).
  - Binaries live in `plugin/libexec/<target>/`. The launchers are `plugin/libexec/anneal` and `anneal.cmd`. There is no `plugin/bin/`, because claude.ai rejects it (CI asserts this, `ci.yml:69-70`).
  - Requires Claude Code ≥ 2.1.295 (`README.md:13`).

## 2. Functional shape

### 2.1 Skills (`plugin/skills/`, 12; each SKILL.md is 50 to 63 lines) [read]

| Skill | Invocation | Phase / job | CLI it drives |
|---|---|---|---|
| `grill` | model | define: question the human in rounds, design tree | `grill frontier/classify/confirm` |
| `spec` | model | EARS spec + Out of Scope + constitution check | `lint spec`, `lint terms` |
| `plan` | model | tracer-bullet tasks, approval by hash | `lint plan`, `plan approve/check`, `dag` |
| `build` | model | test-first, `temper:gate` in Rust repos | `mode build`, `task done`, `floor-guard` |
| `diagnose` | model | red repro first, `[DEBUG-id]` instrumentation | `mode diagnose`, `repro record/leftovers` |
| `verify` | model | fresh test run + doubt pass (stops at local) | `task done`, `verify risk` (D5) |
| `ship` | model | review depth, verdict, GO/NO-GO, migrate, rollout | `ship skip-rule`, `verdict`, `migrate`, `rollout`, `perf`, `watch` |
| `/orchestrate` | user | fan out approved plan across subagents | `orchestrate`, `brief`, `round`, `decide --point D2` |
| `/handoff` | user | continue/clear/compact at phase boundary | `.continue-here` |
| `/correct` | user | turn a mistake into a hook/lint/test | `lint rules` |
| `/write-skill` | user | write a skill test-first, Tiers 0-3 | `lint skills`, `eval routing/uplift` |
| `/calibrate` | user | score Clef points, propose thresholds | `calibrate [--apply]` |

`plugin/playbooks/` holds `route.json` (8 phases → entry skill, 4 sizes → depth), `clef-thresholds.json` (D1–D7 bands, models `@cf/wet-ink/clef-flash` / `@cf/wet-ink/clef`), `spec.template.md`, `plan.template.md`, and `schemas/{design-tree,plan}.schema.json`. `docs/bootstrap-index.md` is the 200-word SessionStart injection.

### 2.2 Hooks (`plugin/hooks/hooks.json`; 14 entries on 9 events; all exec form via `${CLAUDE_PLUGIN_ROOT}/libexec/anneal hook <id>`) [read]

| Hook | Event / matcher | Policy, timeout | What it does | src LOC [measured] |
|---|---|---|---|---|
| bootstrap | SessionStart `startup\|clear\|compact\|resume` | open, 15 s | inject index, then the ledger tail if it changed | 893 |
| route (D3) | UserPromptSubmit | open, 5 s | Clef phase+size hint (shadow) | 959 |
| ruled-before (D6) | UserPromptSubmit | open, 5 s | surface `.out-of-scope/` + weigh-in atoms (shadow) | 791 |
| guard (+D1) | PreToolUse Bash | **closed**, 5 s | shell-lexing denylist; D1 destructive (shadow) | 4,964 |
| merge-gate | PreToolUse Bash | **closed**, 5 s | deny `gh pr merge` without fresh verdicts | 693 |
| preconditions | PreToolUse `Edit\|Write` | **closed**, 5 s | build/diagnose mode preconditions | 685 |
| brief | PreToolUse `Agent` | **closed**, 5 s | no brief, no spawn; no nested spawns | 613 |
| fast-check | PostToolBatch | open, 15 s | floor-guard, `check_fast`, terms, spec/plan lint | 899 |
| injection-scan | PostToolBatch | open, 2 s | flag instruction-shaped text from Read/WebFetch | 1,293 |
| context-monitor | PostToolBatch | open, 5 s | warn 60%, block once 75% (needs statusline) | 175 |
| completeness (+D4) | Stop | open, 30 s | block unproven "done" claims and leftovers; cap 2 | 1,714 |
| status | SubagentStop | open, 15 s | record subagent status block, tokens, duration | 576 |
| handoff | PreCompact | open, 30 s | write `.continue-here` | 502 |
| standing-orders | SubagentStart | open, 5 s | inject `.anneal/standing-orders.md` | n/a |

Each hook has a metadata sidecar `plugin/hooks/hooks.metadata.json` (id, event, role guard/scaffolding, `model_generation: claude-5`, source story), checked against `hooks.metadata.schema.json` by `cargo test`. Hooks fail open and guards fail closed. A missing binary means the launcher exits 2 for `--fail-closed` (`plugin/libexec/anneal:41-58`), so **without a binary every Bash, Edit/Write and Agent call is blocked** (`README.md:24`).

### 2.3 CLI (`anneal`, 24 commands, closed enum `src/command.rs:7-56`; every command emits the JSON envelope `{ok, command, data, findings, proves, does_not_prove}`, `src/envelope.rs`) [read]

All 24 are **built** [read: `cli.rs:283-332`]. Grouped by capability, with non-test src LOC [measured]:

- **Ledger:** `ledger append|query|tail|journal` (304 + ledger crate 2,705).
- **Define/plan:** `grill frontier|classify|confirm` (1,295); `lint spec|plan|terms|brief|rules|skills` (6,940; skills alone 2,780); `plan approve|check` (507); `dag check|frontier` (1,809: markdown and GitHub trackers through `gh`, `Tracker` trait at `src/dag/tracker.rs:45`).
- **Build/diagnose:** `mode build|diagnose|off` (520); `task done` (389); `repro record|leftovers` (728); `floor-guard` (1,833, plus `.anneal/ratchets.json`).
- **Review/ship:**
  - `verdict set|valid` (745; patch-id freshness across rebases)
  - `ship skip-rule` (442)
  - `watch pr|ci|deploy` (1,782; long-poll through `gh`, one exit code per blocker 0/2/3/4/5/6/7)
  - `rollout decide` (450; pure banding of a metrics JSON into go/hold/rollback, with **no metrics source**)
  - `perf sample|noise` (749; Welch's interval)
  - `migrate start|done|contract` (502; expand/contract wave tracker in the ledger)
- **Orchestration:** `brief build` (1,045; 9-field brief + review package under `.anneal/briefs/`); `round start|status` (431; 3 same-agent rounds, 2 escalated, then park); `orchestrate fan-out|park|rule` (797 + weigh-in bridge 891).
- **Judgement:** `decide` (4,731: Clef client, thresholds, redaction 940, ureq transport 340, JSONL decision log); `decide label`; `verify risk` (517, D5); `calibrate [--apply]` (2,145).
- **Skill QA:** `eval routing` Tier 1 lexical / Tier 2 `--router claude` (5,321); `eval uplift` Tier 3 (3,184). Tiers 2 and 3 shell out to `claude plugin eval` and cost money (cap `--max-cost-usd 25`).
- **Misc:** `statusline` (171), `hook <id>` (16,190 across all hooks).

**Planned only** [read]:
- `anneal status --json` (#58)
- Cockpit mod: band, pane, one-key labels, approvals, dashboard (#56, #57 to #63; TypeScript; post-v0.1)
- ADO/Jira trackers (#45)
- per-harness hook generation for Codex/Cursor/Gemini (#46)
- Tier 4 trace review (#48)
- automatic outcome labels (D1 approve/deny, revert within 24 h; `docs/calibrate.md`)
- CI jobs for Tier 2/3 (`evals/README.md:136,270`, pending `ANTHROPIC_API_KEY`)

## 3. Architectural shape

### 3.1 Crates and layering [read]

| Crate | LOC (src+tests) | Role | Deps |
|---|---|---|---|
| `crates/anneal-ledger` | 3,295 | adapter + its event domain: `event` (registry v9, 1,149), `envelope` (385), `tag` (204), `store` (442) + `store/schema` (485) | happenstance-core/-sqlite 0.3.2, tokio `rt`, time, tempfile, serde(_json), thiserror |
| `crates/anneal-cli` | ~99k | binary + internal lib (`publish = false`, "not a stable API", `src/lib.rs:4-5`) | anneal-ledger, clap 4.6, serde, sha2 0.10, thiserror 2, ureq 3.4 (rustls, bundled roots) |

- Inside `anneal-cli`, modules are ports-and-adapters in places: `Transport`, `Clock` (`decide`), `Tracker`/`GhApi`/`GhGraphql` (`dag`), the weigh-in `Runner`, and the `perf` runner.
- Coupling hotspots [measured, `crate::` path counts]: `hook/guard`, `decide` and `calibrate` reference `crate::decide` 97 times. Tier 3 eval reaches into `eval::routing::real` (#108).
- There is no domain/application split. Hooks, commands and policies live in one crate.
- The charter (`AGENTS.md`) lists only two crates and is missing ureq and sha2 from the dependency column (issue #1 comment) [read].

### 3.2 Runtime model [read]

- **In-session only:**
  - Every hook is a fresh process per event, and the store is synchronous: it builds its own current-thread tokio runtime and refuses inside one (`store.rs:281-335`).
  - Clef calls are blocking HTTP with a 2 s timeout inside hook budgets.
- **Partly headless-usable:** `dag`, `brief`, `round`, `verdict`, `watch`, `ship skip-rule`, `floor-guard`, `task done`, `repro`, `lint *`, `plan check`, `migrate`, `perf`, `rollout` are plain argv-in, JSON-out commands. They have typed exit codes and bounded subprocesses (`src/process.rs`, 289), and they need no Claude Code session.
- **CI:**
  - `ci.yml` covers fmt, clippy, tests and lint skills on 3 OSes, plus the launcher smoke test, version check, no-`bin/` check and `claude plugin validate --strict`.
  - `rust.yml` (274 lines) runs temper's gate through the vendored `.claude/temper/temper.mjs`: baseline ratchet, per-crate builds, feature powerset, `cargo machete`, MSRV, and a compliance report commented on the PR.

### 3.3 Data owned [read]

| Data | Location | Format | Scope |
|---|---|---|---|
| Event ledger | `<git-common-dir>/anneal/ledger.sqlite3` (resolve order: `--store`, `$ANNEAL_STORE`, reserved config step, git common dir, `<folder>/.anneal/ledger.sqlite3` outside git; `docs/ledger.md:10-24`) | happenstance 0.3.2 schema, TRUNCATE, `user_version = 1`, registry v9 (18 types) | per repo, all worktrees |
| Clef decision log | `$ANNEAL_HOME` or `~/.anneal/decisions/decisions.jsonl` (+ `.1`–`.4` rotations, 16 MiB each, `counts.json`, `proposals.json`, `route-pending/<hash>.json`) | JSONL rows `kind: decision\|label` (`decide/log.rs:1-13`; fixture `tests/fixtures/calibrate/decisions/decisions.jsonl`) | **per user, cross-repo**, not in the ledger |
| Stop-block counter, context usage | `~/.anneal/stop-blocks.json`, `~/.anneal/context-usage.json` | JSON | per user |
| Repo config | `.anneal/config.json` (`check_fast`, `review.require_plugin`, `ship.sensitive_globs`, `orchestrate.leads`), `.anneal/ratchets.json` (versioned) | JSON, size-capped (`repo_config.rs`) | committed |
| Briefs | `.anneal/briefs/<id>.md`, `<id>.review.md` | markdown, 9 fields | working files |
| Design tree, spec, plan | anywhere; schemas in `plugin/playbooks/schemas/` | JSON / markdown | committed |
| Rulings | `.out-of-scope/<slug>.md` (`id, ruled_at, summary, weigh_in_ref?`); ADRs `docs/adr/NNNN-slug.md` | markdown + frontmatter (`docs/define.md`) | committed |
| Handoff | `.continue-here` at repo root (excluded via `.git/info/exclude`) | markdown | per worktree |

**Ids:**
- Ledger positions are u64.
- Decision ids look like `d<ms>-<pid?>-<n>` (`decide/id.rs`).
- Ticket names are lower-cased; Windows device names are refused.
- anneal mints no `wi-` ids; weigh-in mints them.

### 3.4 Toolchain and platform [read]

- Rust 1.98.1 pinned, edition 2024, `rust-version` = pin = weigh-in's. Lints come from temper's template, and the vendored temper is 1.3.0 (`.claude/temper/VERSION`).
- **Windows:** the `anneal.cmd` launcher; MSVC builds in release. Tier 2 eval needs `--claude <path to claude.cmd>`.
- **Cowork and claude.ai:** skills only. Hooks and Clef do not run there (`README.md:28-35`). TRUNCATE is kept for Cowork-connected folders anyway.
- **Cowork:** `docs/handoff.md` and the anneal home sit outside the plugin dir because `CLAUDE_PLUGIN_DATA` is deleted on uninstall (`decide/log.rs:4-5`).
- **Clef:** the external dependency is Cloudflare Workers AI custom models `@cf/wet-ink/clef{,-flash}`.
  - It is configured through plugin `userConfig` (`cf_account_id`, `cf_api_token`, `clef_mode`), which reaches hooks as `CLAUDE_PLUGIN_OPTION_*`.
  - Live Clef waits on Ryan's Cloudflare secrets (issue #1 comment), so every point today is unreachable or in shadow [read].

## 4. Contracts and integration surface

| Contract | Direction | Detail |
|---|---|---|
| `weighin batch` stdin JSON (`raise` + `defer` ops) | anneal → weigh-in | `docs/orchestrate.md:37-50`; binary found via `$WEIGHIN_BIN` then PATH (weigh-in's own libexec is invisible to anneal). Receipt is parsed **from human text lines** (`✓ raised wi-…`, `deferred wi-…`, `src/weighin/receipt.rs:7-11`). Needs weigh-in ≥ 0.6.0. 10 s timeout, 4 MiB output cap. |
| `weighin open --all --json --top 4096` | anneal ← weigh-in | D6 reads items decided but not yet written as atoms (`docs/orchestrate.md:96-104`) |
| `.kb/_intake/decisions/*.md` frontmatter | anneal ← weigh-in | D6 reads `weighin_item`, `status == accepted`, `title`/`question` only (`hook/ruled_before/precedents.rs:1-8,204-208`) |
| `weigh-in` marker blocks | must **not** emit | weigh-in's Stop hook would double-raise (`plugin/skills/orchestrate/SKILL.md:17`) |
| `temper:gate`, temper's escape-hatch scan | anneal → temper | `build`/`verify` call `temper:gate` in Rust repos; `floor-guard` skips `#[expect(` ("temper judges it", `docs/floor-guard.md`) |
| Status block grammar | anneal ← subagents | `Status: DONE\|DONE_WITH_CONCERNS\|NEEDS_CONTEXT\|BLOCKED` + `Gate/Review/Changed/Decisions needed/Not verified` (`hook/status/block.rs`; `docs/hooks.md:325-331`). **temper's `WIP` is recorded as a violation** (`docs/hooks.md:330`), and temper's grammar is `DONE\|WIP\|BLOCKED` (`temper/skills/rust-craft/SKILL.md:115`). |
| Verdict ledger `verdict_set{source: local\|review_plugin\|human}` | review plugin (assay) → anneal | `merge-gate` requires a review verdict; `review.require_plugin` makes only `review_plugin` count (`docs/verdict.md:111-160`). assay must append to **anneal's** ledger (via `anneal verdict set` or `ledger append`) [inferred]. |
| JSON envelope + typed exit codes | anneal → any caller | `docs/exit-codes.md` (generated from `src/exit_codes.rs`, 604 LOC): 2 = usage (block), 64 = not implemented (dead), 70 internal, 74 output; `ok ⇔ exit 0`; `proves` / `does_not_prove` per command |
| Hook output | anneal → Claude Code | one `ERROR <reason>` line; `permissionDecision` deny/ask; `additionalContext`; Stop `decision: block` |
| `ledger journal` | anneal → git | JSONL of all envelopes for cloud sessions to commit (mirrors weigh-in's `.kb/_ledger/journal/`), but anneal has no hook that does it [read `docs/ledger.md` Commands] |
| Env vars | in | `ANNEAL_STORE`, `ANNEAL_HOME`, `WEIGHIN_BIN`, `CLAUDE_PLUGIN_OPTION_{CF_ACCOUNT_ID,CF_API_TOKEN,CLEF_MODE}`, `CLAUDE_PROJECT_DIR`, `CLAUDE_PLUGIN_ROOT`, `CLAUDE_CODE_AUTO_COMPACT_WINDOW`, debug-only `ANNEAL_CLEF_URL` [measured grep] |
| Vendored copy | temper → anneal | `.claude/temper/` (temper 1.3.0) already differs from temper's `scripts/` in `hooks.mjs`, `shell.mjs` and `temper.mjs` [measured diff] |

**Reverse references:** none. `grep -ri anneal` finds nothing in weigh-in, temper, assay or redkiln-rs [measured]. The integration is one-way: anneal knows weigh-in and temper, and nothing knows anneal.

## 5. Backlog and plans

- **The plan is issue #1** (Initiative, target 2026-10-30). There are 8 Release Features, #2 to #8 and #56; only #2 (R1) is closed.
  - Status comment 2026-10-12: R1 to R5 merged, "paused at the usage limit".
  - Waiting on Ryan: Tier 2/3 CI with `ANTHROPIC_API_KEY`, the weigh-in binary CI step, the AGENTS.md crate map, temper repo access for #27, and Cloudflare secrets for live Clef.
  - Handoff notes live in `anneal-afk/` in project files, which were not present here [read].
- **In flight:**
  - #44, v0.1.0 release (blocked by #40 to #43, now all closed). The risk it states: "most points ship in shadow". It needs the release workflow and the marketplace branch (PR #111, merged).
  - #27, dogfood on temper (open).
  - PR #113, guard redaction (open).
- **Open follow-ups by theme** [read]:
  - Gates: #80 (MultiEdit/NotebookEdit unguarded), #84 (completeness claim vocabulary)
  - Plan/spec integrity: #75, #76, #77
  - Shared cycle detector between `dag` and `grill`: #71
  - Clef: #86, #89, #91
  - CLI UX: #67, #68
  - Evals refactor: #108
  - Post-v0.1: #45, #46, #48, and the Cockpit #56 to #63
- **Decision records:**
  - Twelve weigh-in atoms in `.kb/_intake/decisions/`, mostly test-expectation approvals under H-12.
  - wi-1154f8 (one-way): anneal **does** record the orchestrator's reversible rulings (`orchestrate.ruled`) despite "anneal never stores decisions". This blurs the weigh-in boundary.
  - wi-62022f: allow ureq/rustls in deny.toml.
  - wi-a53271: MIT OR Apache-2.0.
  - wi-92bc1b: Tier 2 cost cap and model.
- **Constraints worth carrying into the suite** [read]:
  - happenstance is pinned to 0.3.2 with weigh-in. Dependabot ignores it, and moving it is a ledger migration (H-16) (`Cargo.toml:28-31`).
  - The ledger refuses WAL and does not convert it (`crates/anneal-ledger/AGENTS.md` decisions, 2026-10-11, citing weigh-in wi-0940cf).
  - The registry is additive-only, with a version bump per change.
  - The boundaries in issue #1: weigh-in owns decisions, temper owns Rust standards, the review plugin owns PR review, and happenstance owns storage.
  - From the issue #56 design rule: guards never move into mods. "Hooks enforce, the mod shows and collects."

## 6. Shared-foundation candidates (→ Redkiln Core)

| # | Candidate | Where (LOC) | Sibling duplicate | Verdict |
|---|---|---|---|---|
| 6.1 | **Store opening under TRUNCATE** (header pre-read, refuse WAL/newer/foreign, template schema copy in `BEGIN IMMEDIATE`, `user_version`, busy timeout, sync façade over async happenstance, refuse-inside-runtime) | `crates/anneal-ledger/src/store.rs` + `store/schema.rs` (927 + 590 tests) | weigh-in `crates/weighin/src/store.rs` (872, anyhow-based, **converts** WAL and migrates 0.1.x/0.2.x ledgers, busy 5 s vs anneal 10 s, silently trims tags vs anneal's strict errors) | **Core, take anneal's typed version** plus weigh-in's migration steps. This is the single best extraction. |
| 6.2 | **Event envelope** `{type (flattened), actor, session_id, worktree, branch, head, ts}` | `anneal-ledger/src/envelope.rs` (385) | weigh-in `weighin-core/src/event.rs:634-656`: same keys exactly | **Core.** Unify the `worktree` value (anneal: full toplevel path, `/`, lower-cased on Win/mac, `docs/ledger.md:62`; weigh-in: "directory name", `event.rs:643`). Add a `plugin` / type namespace. |
| 6.3 | **Tag vocabulary + validation** (closed `TagKey`, ≤255 bytes, no control chars) | `anneal-ledger/src/tag.rs` (204) | weigh-in builds `item/session/worktree/channel/kind/door` tags ad hoc, truncating | Core owns the mechanism and shared keys (`session`, `worktree`); each plugin owns its own keys |
| 6.4 | **Store location resolver** (`--store` → `$X_STORE` → config → git common dir → folder; `data.store` + `data.source`; dubious-ownership git failure is fatal, not a fallback) | `anneal-cli/src/paths.rs` (289 + 273 tests) | weigh-in same order (`suite-vs-redkiln.md:13`) | **Core**, pointed at `<git-common-dir>/redkiln/events.sqlite3` |
| 6.5 | **Calibration library** (decision-row schema, Brier, 10 reliability bins, ask rate, cost-weighted threshold proposal 5:1, eligible-to-enforce verdict n≥50/Brier≤0.15/ask<20%, thresholds-file diff and pointer-checked apply) | `src/calibrate/` (2,145 + 1,591 tests), `src/decide/thresholds.rs` (403) | weigh-in eval seed, assay verifier calibration, redkiln-rs ADR-0079 (`suite-structure.md:205`) | **Core** for scoring and the row format. Keep Clef transport/points in Bench. Note: rows live in `~/.anneal/decisions/*.jsonl`, not the event store, so Core must choose. |
| 6.6 | **Redaction** of credentials before text leaves the machine (shell-aware, prose-aware, plus the secret options of about 20 named tools, per CHANGELOG line 71) | `src/decide/redact/` (940) + `hook/guard/redact/` (626) | none found | Core or a shared crate: any plugin sending text to a model (Review, Loop) needs it |
| 6.7 | **Shell lexer** (quotes, heredocs, `bash -c`, `eval`, wrappers, brace expansion, abbreviated long options) | `hook/guard/shell/` (938) + `denylist/` (2,847) | temper has its own Node `lib/shell.mjs` and `lexer.mjs` (vendored `.claude/temper/lib/`) | A shared crate (`redkiln-guard`). Loop's headless sandbox needs exactly this denylist. |
| 6.8 | **CLI envelope + typed exit codes + `proves`/`does_not_prove` + generated exit-code doc** | `src/envelope.rs` (151), `src/exit_codes.rs` (604), `src/command.rs` (273) | weigh-in prints its own text receipts (hence §4's fragile text parse) | **Core spec**: a suite-wide CLI result contract |
| 6.9 | **Launcher / libexec packaging** (`plugin/libexec/anneal` sh + `.cmd`, newest-build pick, PATH fallback, fail-closed when missing) + `scripts/build-plugin-bins.{sh,ps1}`, `test-launcher.sh`, `check-versions.sh`, `changelog-section.sh`, release workflow, marketplace branch job | ~400 lines of shell + 127-line workflow | weigh-in has the same set (`build-plugin-bins.sh`, `changelog-section.sh`, `marketplace-branch.sh`); #44 says "copied from weigh-in's release.yml" | Monorepo tooling (one parametrised copy), not a Core crate |
| 6.10 | **Bounded subprocess runner** (argv, timeout, output caps) and **bounded git/PR diff reads** (patch-id, numstat, ancestry) | `src/process.rs` (289), `src/pr_diff.rs` (482) | weigh-in `git.rs`; assay will need PR diffs | Shared crate candidate (rule of two holds with assay) |
| 6.11 | **hooks.metadata sidecar + schema** (role guard/scaffolding, model generation, failure policy, fingerprint) | `plugin/hooks/hooks.metadata*.json`, `tests/it/hooks_manifest.rs` | none | Suite convention for every plugin; supports the "living together in one session" problem |
| 6.12 | Capacity | — | none in anneal beyond eval `--max-cost-usd` caps and Clef's 2 s budget | nothing to extract |

## 7. Granularity findings

**Natural seams inside anneal** [inferred from module sizes and coupling]:

1. **Plugin-authoring kit** (about 15k LOC): `lint skills` (21 rules), `lint rules`, and `eval routing`/`uplift` (Tiers 1 to 3, Wilson and Newcombe intervals, the `claude plugin eval` driver).
   - It lints and evaluates *any* plugin's skills, not SDLC work.
   - Every suite plugin (temper, weigh-in, Review) should run it in CI.
   - It belongs in a dev-only crate/tool, perhaps `redkiln-skillcheck`, and not in Bench's runtime binary. Shipping it inside the hook binary bloats every hook process.
2. **Judgement layer** (`decide` + `calibrate` + `verify risk`, about 7.4k): the Clef client and its 7 points. The scoring half goes to Core (§6.5). The client stays with Bench unless Loop or Review adopt Clef.
3. **Guard** (about 5k): a deterministic Bash denylist with redaction. It is reusable as-is by Loop's sandboxed runs and by any plugin wanting a fail-closed Bash hook. It could be its own crate, and arguably the thing installed by everyone.
4. **PR/merge toolkit** (`verdict`, `ship skip-rule`, `watch`, `merge-gate`, `pr_diff`; about 4.1k): this is the seam with **Review (assay)**.
   - The verdict ledger is the hand-off point. assay produces verdicts and anneal gates on them.
   - `watch pr|ci` duplicates what a headless conductor needs to wait on CI.
   - Decide whether Review or a shared crate owns `verdict_set`. Today anneal owns the event type, the store, and the freshness rule (patch-id from the same base).
5. **Post-merge** (`rollout`, `perf`, `migrate`; about 1.7k): small, pure, and owned by no other plugin.
   - `rollout decide` has no metrics source.
   - `suite-structure.md:207` calls after-merge "an honest gap". It is now thin code with no user.
   - Candidate to move to Loop's release/operate stage, or to freeze.
6. **Orchestration** (`dag`, `brief`, `round`, `orchestrate`, the brief/standing-orders hooks; about 5k): this is **in-session fan-out**, which is exactly Loop's job done inside a chat.
   - `dag frontier` over GitHub sub-issues and `Blocked by #n` already implements the Backlog = GitHub Issues decision (§8).
7. **Define-phase lints** (`lint spec` EARS/constitution, `lint plan`, `lint terms`, `grill`, ADR numbering, `.out-of-scope` store; about 4.5k): Bench's core. The `.out-of-scope/` rulings and ADRs are knowledge atoms by another name, so there is overlap with **Knowledge**.
8. **Ledger crate**: already separate. It dissolves into Core (§6.1 to §6.4) plus an `anneal-events` registry module.

**Overlaps with siblings:**

| Overlap | anneal piece | Sibling piece | Recommendation |
|---|---|---|---|
| Store | `anneal-ledger` | weigh-in `store.rs` | merge into Core |
| Decisions | `orchestrate.ruled` (wi-1154f8), `.out-of-scope/`, ADR numbering | weigh-in owns decisions | route rulings to weigh-in/Knowledge |
| Gates | `build`/`verify` call `temper:gate`; `floor-guard` vs temper scan | temper gate | keep the split as documented |
| Status grammar | `DONE_WITH_CONCERNS`, `NEEDS_CONTEXT` | temper `WIP` | one Core grammar |
| Stop hooks | completeness (cap 2 blocks) | temper's gate/review Stop, weigh-in capture Stop | Core convention for Stop ordering and caps |
| Review | verdict ledger, merge-gate | assay | shared verdict event in Core |
| Fan-out/backlog | `dag`, `brief`, `round`, `orchestrate`, GitHub tracker | Loop (redkiln-rs), Backlog = GitHub Issues | lift `dag`/tracker into a shared crate Loop uses |

**What a headless conductor (Loop) can reuse today, unchanged as a subprocess** [read, inferred]:
- `dag check|frontier --tracker github` (ready set plus token estimate)
- `brief build` / `lint brief` (the worker brief contract)
- `round start|status` (fix-loop escalation, ledger-backed)
- `task done -- <cmd>` (test-run evidence)
- `repro record`
- `floor-guard` (refuses weakened tests or lints in a worker's diff; the strongest headless guard)
- `lint spec|plan`, `plan check` (approved-and-unchanged by hash)
- `verdict set|valid`, `ship skip-rule`, `watch pr|ci` (wait for CI with a typed exit)
- `orchestrate park` (parks a one-way call in weigh-in with a default; the "rulings, not stalls" protocol)
- the guard's denylist, as a library check of each worker's Bash command
- `perf noise`

**Not reusable headless:**
- the hooks bound to Claude Code events and transcripts: preconditions keyed to `mode_set`, completeness (claim regexes over the final message), status (SubagentStop transcript), context-monitor, handoff, route
- `/orchestrate` itself, which is a prose skill relying on in-session subagents and the `brief` hook

## 8. Evidence that challenges the decisions

1. **"One shared happenstance SQLite file … events tagged by plugin."** anneal's code assumes it owns the file:
   - (a) `Store::read` decodes every returned row strictly. An unfiltered query (`ledger query` with no `--type`, `tail`, `journal`, the bootstrap ledger tail) fails with `StoreError::Decode` on the first weigh-in event (`store.rs:438-445,474-479`; the error text says "not a registered ledger event").
   - (b) Event types are unprefixed (`test_run`, `task_done`, `plan_approved`, `mode_set`, `round_started`; dotted ones like `migrate.contract` are inconsistent). One registry needs namespacing, which means renaming persisted types, which is a migration under H-16.
   - (c) `PRAGMA user_version` is per file. Both plugins stamp it `1` and **refuse a newer stamp** (`OpenError::NewerSchema`), so one plugin's schema bump disables the other until it upgrades. This is the coupling `suite-structure.md:116` warned about.
   - (d) Shared tag keys disagree on values (`worktree`).
   - (e) anneal's busy timeout is 10 s and the guard's 500 ms per statement. Fourteen anneal hooks plus weigh-in's hooks, serialised on one write lock, make guard-recording failures likelier (guard denies anyway but drops the record, `docs/guard.md:31-37`).

   These are all fixable by Core: lenient unknown-type reads, `<plugin>.<type>` names, a Core-owned stamp. The decision can stand, but it is a breaking change for anneal (registry v10 plus a migration) and must be planned, not assumed.
2. **"Calibration library goes in Core."** It is supported, but anneal keeps calibration data **outside** the event store, per user and cross-repo (`~/.anneal/decisions/*.jsonl`, deliberately so it survives reinstall and spans repos). A Core calibration library over a per-repo event file would lose cross-repo pooling. Core needs a per-user store or must accept per-repo calibration (n ≥ 50 per point per repo is much slower).
3. **"Redkiln Bench (from anneal, in-session workflow)" as one plugin.**
   - anneal ships a 60k-line single binary in which roughly 15k lines (skill lint and evals) are authoring tools and roughly 9k (verdict/watch/merge-gate, orchestration) are what Review and Loop also need.
   - Packaging all of that as "Bench" makes Review and Loop depend on Bench's binary, or duplicate it.
   - This argues for **crates** (guard, pr-toolkit, dag/brief, skillcheck) shared across plugins inside the monorepo, with Bench as the hook-and-skill shell. It does not argue for more plugins.
4. **"Backlog = GitHub Issues, no in-repo .bklg."** Supported: `dag` already reads GitHub sub-issues and `Blocked by #n` through `gh`, and treats closed outside blockers as satisfied (CHANGELOG line 49). It still also supports local markdown tickets (`--dir tickets`), which the suite could keep as an offline fallback or drop.
5. **Weigh-In integration over a CLI.** The bridge parses weigh-in's human receipt text (`receipt.rs`). In a monorepo with a shared store, Bench could append weigh-in's `item_raised` event directly with a DCB condition. Equally, weigh-in's CLI should gain `--json` receipts. Either way the current text contract should not survive the move.
6. **"Docs deferred" and "suite not monolith".** Nothing in anneal contradicts these.
7. **Naming.** The internal docs still say "the review plugin (planned)" and `source: review_plugin` (`docs/verdict.md`), which is persisted in events. A rename to Redkiln Review changes no wire value, but `review_plugin` is now a permanent enum string.

## 9. Migration notes (monorepo move)

- **History:** 44 squash commits, 2.1 MiB pack. Trivial to import with `git subtree`/`filter-repo` into `plugins/bench/` or similar [measured].
- **CI/release:**
  - `ci.yml`, `rust.yml` and `release.yml` assume the repo root is the Cargo workspace root and the marketplace root.
  - The release tag pattern `v*` must become per-plugin (`bench-v*`).
  - The `versions` job greps `^version = "…"` in the root `Cargo.toml` and `plugin/.claude-plugin/plugin.json`. `scripts/check-versions.sh` resolves `$root/plugin/...`.
  - `rust.yml` runs temper's baseline with `--base` refs and per-crate `cargo check -p`.
  - All need path parametrisation.
- **Marketplace coupling:**
  - The `marketplace` job **force-pushes an orphan `marketplace` branch of the whole tree** (`release.yml:116-127`). In a monorepo, that would publish every plugin's tree on each plugin's release.
  - It needs a single suite marketplace branch assembled from per-plugin artifacts.
  - The marketplace name `anneal` and install id `anneal@anneal` change. Users are internal and pre-release.
- **Secrets:** no repository secrets are used today (only `github.token`) [measured grep]. Pending additions are `ANTHROPIC_API_KEY` for Tier 2/3 evals and the Cloudflare account and token as user config, not CI.
- **Vendored copy:** `.claude/temper/` (temper 1.3.0, already drifted). In the monorepo, point the hooks at the in-tree temper plugin and delete the vendored copy.
- **Path assumptions that break:**
  - `<git-common-dir>/anneal/ledger.sqlite3` becomes `…/redkiln/events.sqlite3`; this needs a migration or import of old ledgers. weigh-in already has a stepwise migrator; anneal refuses unknown files.
  - `$ANNEAL_STORE`, `$ANNEAL_HOME` and `~/.anneal/` become suite names?
  - `.anneal/` repo config, briefs and ratchets: the guard and preconditions hard-deny edits to `.anneal/config.json`, so a rename must update the denylist (CHANGELOG line 29).
  - `plugin/playbooks/clef-thresholds.json` is read from `CLAUDE_PLUGIN_ROOT` with a built-in fallback.
  - `.continue-here` sits at the repo root.
  - `cargo test` asserts that `plugin/` holds no `evals/` and runs `lint skills` over `plugin/skills`.
- **Workspace pins:** `[workspace.lints]` from temper's template, `deny.toml` (webpki-roots CDLA exception), `rust-toolchain.toml` 1.98.1, and `clippy.toml`. All are identical in kind to weigh-in's, so one root copy should serve both. Check for diffs before merging; the clippy.toml `tokio::sync::watch::Ref` entry differs: weigh-in has `allow-invalid = true` and anneal does not (`clippy.toml:36` in each) [measured].
- **Git hooks:** `.githooks/pre-commit` runs the vendored temper; weigh-in's `.githooks/pre-push` refuses `archive/*`. Merge them.
- **Line endings:** LF everywhere except `*.cmd` (`.gitattributes`, citing redkiln-rs ADR-0164).
- **Pending one-way work that interacts with the move:** the happenstance 0.3.2 → 0.4.1 bump, which is a ledger schema migration under H-16 and should be done once, in Core, for both plugins (`suite-structure.md:243`). Also the v0.1.0 release (#44), which would put a public `anneal` marketplace branch in place just before a rename. Consider holding #44's marketplace publication until the suite naming lands.
