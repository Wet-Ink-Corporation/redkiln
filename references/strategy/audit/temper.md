# Audit: temper (becomes Redkiln Rust)

*2026-10-13. Repo `Wet-Ink-Corporation/temper` at `main` 2b4bc04 (merge of #126, 2026-10-12 21:31 -0700), checked out at `/home/claude/temper`. Evidence tags: [measured] = counted or run here today; [read] = stated in a file or issue; [inferred] = my judgement from those. Paths are repo-relative to `temper/` unless prefixed with another repo name.*

---

## 1. Identity

**One-line job.** A Claude Code plugin that ships the rust-craft Rust standard (182 rules with permanent IDs) and enforces it in session through hooks, subagents and a completion gate, and in CI through a copy of its CLI vendored into each adopting repo. [read] `.claude-plugin/plugin.json:5`, `AGENTS.md:3`

| Fact | Value |
|---|---|
| Version | 1.3.0 (`.claude-plugin/plugin.json:4`). Large `## Unreleased` section (`CHANGELOG.md:3-85`). v1 is to be **1.4.0**, cut after the pilot fixes (wi-891617). [read] |
| Releases | v1.0.0 (2026-09-27) to v1.3.0 (2026-10-05); 1.0 to 1.2.3 were published retroactively; 1.3.0 is the first through `release.yml` (`docs/plans/v1-readiness.md`, G5, 3.1). [read] |
| Commits | 233 on `main`. First commit in this history 2026-10-02 (earlier history merged in). Last activity 2026-10-13 04:31Z (#126). [measured] |
| PRs | 126 numbered issues and PRs. All work goes through PRs since 2026-10-02 under a ruleset that requires `Checks`, `Unit tests (Windows)` and `CHANGELOG entry` (#62, wi-2d6d35). [read] |
| LOC by language | Node engine: **7,947** lines of `.mjs` (`scripts/temper.mjs` 691 plus 12 `scripts/lib/*.mjs` modules), plus the generated `scripts/lib/rulemap.json` (1,675). Node tests 7,279. Python checks 1,546 (5 files). Shell 1,779. Markdown 26,340 (the standard is about 13,958 of that: `skills/rust-craft/references/*` plus the skills and agents). Rust 3,140, all in `templates/example-workspace` (no Rust engine yet). [measured] |
| Tests | `node --test tests/*.test.mjs`: **365 tests, 364 pass, 1 skip** (27.6 s) [measured]. Plus `tests/registry.py` (2,757 rule-ID citations), `lint-claims.py`, `gen_rulemap.py --check`, `doc_examples.py` (329 code blocks compiled or linted), the brownfield e2e and the example-workspace gate, all orchestrated by `tests/ci.sh` (9 steps). [read] `tests/ci.sh:21-55`, #58 |
| Evals | 12 `claude plugin eval` cases in `evals/`, 4 of them authored but not yet run (#126, wi-20594c). Last confirming run: discriminating mean Δ +0.42 vs a +0.4 target (#62). [read] |
| Distribution | The repo **is** the marketplace (`.claude-plugin/marketplace.json`, `"source": "./"`, marketplace name `temper`, wi-d75e63). Marketplace users install whatever `main` holds (`AGENTS.md:7`). claude.ai organization sync reads the same repo (`docs/troubleshooting.md:44`). Internal visibility (wi-fd3ac9, 0.7). Release = `release/X.Y.Z` branch, `release.yml` check job, owner-approved `publish` job in the `release` environment that tags and creates the GitHub release, then the owner merges with a merge commit (`AGENTS.md:79-87`). [read] |
| Adopters | weigh-in (`.claude/temper/VERSION` 1.3.0, baseline remediated to 0), anneal (1.3.0, no baseline), redkiln-rs on `build/phase-0-to-1` only (baseline 6,606 findings, #63). assay has none. [measured, read] |

## 2. Functional shape

### Commands (skills), agents, hooks

| Surface | Items | State |
|---|---|---|
| Standard skill | `temper:rust-craft` (`skills/rust-craft/SKILL.md` 147 lines; 15 reference files; `references/rules.md` registry of 182 rules, 96 with a mechanical enforcer, 86 review-only, 31 of them MUST, #58) | Built, mature [read] |
| Slash commands | `/temper:setup`, `/temper:implement`, `/temper:review`, `/temper:gate`, `/temper:audit` (`skills/*/SKILL.md`, 15-49 lines each) | Built |
| Subagents | `rust-architect` (plans), `rust-implementer` (test-first), `rust-reviewer` (fresh-context adversarial review that must emit a `temper_verdict` JSON block) (`agents/*.md`) | Built |
| Hooks (`hooks/hooks.json`) | 9 handlers, all exec-form `node ${CLAUDE_PLUGIN_ROOT}/scripts/temper.mjs hook <event>` (dispatch table `scripts/temper.mjs:49-59`) | Built |

Hook events and what each does [read] `scripts/lib/hooks.mjs`:

| Event (matcher) | Handler | Behaviour |
|---|---|---|
| SessionStart (`startup\|resume\|clear\|compact\|fork`, 30 s) | `session-start` | Records the session's base rev, prunes state, appends `TEMPER_CLI`/`TEMPER_BASE` to `CLAUDE_ENV_FILE`, injects the iron rules and current gate/review state as context (`hooks.mjs:88-121`) |
| UserPromptSubmit (15 s) | `user-prompt` | Reminds that the skill is not loaded (`:125-130`) |
| PreToolUse `Write\|Edit\|MultiEdit` (30 s) | `pre-edit` | Skill-load gate (deny), escape-hatch scanner on the post-edit text (deny/ask on new hits), manifest policy (new deps, lint tables, profiles, `[patch]`, rust-version: ask), protected-config globs (ask), state-dir/baseline writes (deny) (`:247-695`) |
| PreToolUse `Bash` (15 s) | `pre-bash` | Shell-write and shell-delete guards over a real shell reader (`scripts/lib/shell.mjs`, 1,090 lines): state-dir writes and baseline raises denied, protected-config writes/deletes ask, `--no-verify`/`--ignore-rust-version`/lockfile tricks denied, `cargo add` asks, copies/moves/patches of `.rs` scanned (`:697-1104`) |
| PreToolUse `SubagentHandback` (60 s) | `pre-handback` | Validates a reviewer's verdict before delivery in cloud sessions; denies an invalid one (`:1106-1121`; restored by #98 after wi-103aa0 removed it) |
| PostToolUse `Skill\|Read` (15 s) | `post-tool` | Writes the skill-loaded marker (`:1159-1170`) |
| SubagentStart (all, 120 s) | `subagent-start` | For temper agents: injects root, base, fingerprint, changed files, cached gate, plans and (reviewer) the fingerprint the verdict must carry; runs `cargo fmt` before a review (`:1295-1320`) |
| SubagentStop (all, 1200 s) | `subagent-stop` | Reviewer: extract and validate the verdict (from `last_assistant_message` or the transcript), record it. Implementer: run the completion gate without review (`:1264-1292`) |
| Stop (1200 s) | `stop` | **Completion gate**: if Rust changed in this session, run the gate; block on red; block without an approving verdict for the current fingerprint; honour `Status: WIP/BLOCKED`; one-block loop breaker; then spawn a detached telemetry push (`:1187-1247`) |

### CLI subcommands (`scripts/temper.mjs:1-21`)

`hook <event>`, `gate [--check] [--no-cache] [--json]`, `scan [files|--all] [--json]`, `status [--json] [--session]`, `setup [--dry-run] [--force] [--vendor-only] [--keep-going]`, `stats [--json] [--session] [--branch]`, `telemetry push --session`, `baseline [--init|--update|--tighten|--check [--base] [--findings-out] [--json] [--locked]]`, `report [--findings] [--base] [--out] [--format json,md,html]`. All built. [read]

### Built vs planned

| Item | State |
|---|---|
| 1.x Node engine, standard, templates, baseline ratchet, report, telemetry branch, `status/stats --json` (#116) | Built |
| Behaviour PRs held for the pilot window (`Declined:` line #100, `rm` ask #104) | Merged 2026-10-11 into Unreleased [read] |
| **temper lens** (#107, #109-#113): a read-only Claude Code *mod* (band above prompt, `/temper-lens` pane, live gate progress) | Designed; PRs #117-#121, #123 open with `[hold]` [read] |
| **temper 2.0 Rust port** (`docs/plans/temper-2-rust-port.md`, 920 lines, D1-D17 all decided) | Planned only. No Phase 0a artifact exists; gated behind v1 live (#64 `port-2.0-1-2`) [read, measured: no `Cargo.toml` at root] |
| Pre-release model-check dispatch job (#19), tiered CI for the port (#24) | Planned |

## 3. Architectural shape

### Modules today (Node, zero dependencies, Node 18+)

| Module | Lines | Concern | Language-specific? [inferred] |
|---|---|---|---|
| `scripts/temper.mjs` | 691 | CLI dispatch, `status`, `stats`, `setup`, `baseline` commands | Mixed |
| `lib/hooks.mjs` | 1,331 | 9 handlers, deny/ask/block helpers, iron rules text, completion gate | Mixed: the session/Stop/verdict choreography is generic; pre-edit policy is Rust |
| `lib/shell.mjs` | 1,090 | Shell reader: command walk with `cd`, variables, `git -C`, write/delete targets, patch files | **Agnostic** |
| `lib/scan.mjs` + `lib/lexer.mjs` | 871 + 294 | 56 regex rule keys over masked Rust source; test/const/trait-impl regions | Rust |
| `lib/project.mjs` | 622 | Discovery, config, glob, sessions, markers, retention, telemetry, fingerprint | Mostly agnostic; discovery requires `Cargo.toml` and the fingerprint's pathspecs are Rust (`:520-540`) |
| `lib/gate.mjs` | 366 | Step runner with timeouts, per-fingerprint cache, manifest checks, render | Runner/cache agnostic; steps are cargo (`:20-41`) |
| `lib/findings.mjs` | 569 | cargo-message parser, rule map, crate attribution, findings cache | Rust |
| `lib/baseline.mjs` | 504 | Committed lint baseline, line-level ratchet over `git diff`, rename detection, tighten | Ratchet algorithm agnostic; keys are (crate, lint) |
| `lib/report.mjs` | 783 | Scorecard JSON/MD/HTML | Shape agnostic; vocabulary Rust |
| `lib/verdict.mjs` | 120 | Verdict schema, validation, storage by fingerprint | **Agnostic** except the 12 dimension names and the `.rs` coverage filter |
| `lib/manifest.mjs`, `lib/toml.mjs` | 244 + 266 | Cargo manifest policy, a TOML reader | Rust |
| `lib/vendor.mjs`, `lib/requirements.mjs` | 113 + 83 | Vendoring and drift detection; Node/Claude Code minimums | Agnostic |

Roughly 3,500 of 7,947 engine lines are language-agnostic, about 3,100 are Rust-specific, and the rest is mixed. [inferred, from the table]

### Runtime model

- **In session:** hooks plus skills plus three subagents. Every hook is a fresh `node` process; nothing persistent. Hooks fail open (a thrown handler becomes a `systemMessage`, exit 0: `scripts/temper.mjs:84-89`); `TEMPER_DISABLE=1` disables everything. [read]
- **CI and pre-commit in adopting repos:** the vendored copy `node .claude/temper/temper.mjs baseline --check` plus `report` (`templates/workspace/.github/workflows/rust.yml:92-138`, `templates/workspace/.githooks/pre-commit`). CI fails closed. [read]
- **No headless mode** of its own. assay plans to call it headless through an adapter (assay `docs/architecture/03-contracts.md`). [read]

### Data owned

| Store | Path | Format |
|---|---|---|
| State dir | `<git-common-dir>/temper/`, or `target/temper/` without git (`project.mjs:67-75`) | Plain files, atomic writes (`writeAtomic`) |
| Sessions | `temper/sessions/<sid>/`: base rev, start fingerprint, `stop-<agent>.json`, `verdict-<agent>`, `telemetry-pushed` | Text and JSON |
| Gate cache, logs | `temper/gate/<fp>.json`, `temper/logs/` (`gate.mjs:199,228`) | JSON |
| Verdicts | `temper/verdicts/<fp>.json` (`verdict.mjs:103-107`) | `temper_verdict: 1` JSON plus `recorded_at`, `agent_id`, `stale` |
| Findings cache | `temper/findings-<fp>.json` (`findings.mjs:535`) | `temper_findings: 1` |
| Telemetry | `temper/telemetry.jsonl`, rotated to `telemetry.1.jsonl` (`project.mjs:383-404`) | JSONL rows `{t, sid, ccr?, agent, event, ...detail}` with 22 event names (`session-start`, `deny-scan`, `ask`, `stop-blocked`, `verdict`, `gate-start`, ...) |
| Telemetry branch (opt-in) | `origin` branch from `telemetry.branch` (pilots use `temper-telemetry`), file `sessions/<date>-<ccr-or-sid>.jsonl`, private ref `refs/temper/telemetry-tip`, commits authored `temper`, `[skip ci]` (`project.mjs:423-465`, wi-641f99) | JSONL |
| Skill markers | `${CLAUDE_PLUGIN_DATA}/sessions/<sid>/skill-<agent>` (falls back to `tmpdir/temper-plugin-data`) (`project.mjs:192-204`) | Timestamp files |
| In the repo | `.claude/temper.json` (config, `project.mjs:79-104`), `.claude/temper-baseline.json` (`temper_baseline: 1`), `.claude/temper/` (vendored CLI plus `VERSION`), `.temper/plans/`, `target/temper-report/` | JSON, Markdown |

**Ids.** The **tree fingerprint** is the key of everything: `sha256("head:<sha>\nrev:<sha>\n" + git diff --binary <base> -- <Rust pathspecs> + untracked Rust files + changed gate config)[..16 hex]` (`project.mjs:576-605`; the port must keep it bit-identical, port plan §2.3). Sessions key on Claude Code's `session_id`; agents on `agent_id` (`agentKey`). No happenstance, no envelope, no event ids. [read]

**External dependencies.** None at runtime (Node built-ins only). Tooling: Node 22 in CI (`ci.yml`), Python 3.11+, toolchain **1.98.1** (`templates/workspace/rust-toolchain.toml`), CI's Claude Code pin `@anthropic-ai/claude-code@2.1.286` (`ci.yml:41-42`; minimum supported 2.1.286, `requirements.mjs:13`). Planned for 2.0: `regex` 1.13, `toml` 1.1 (`preserve_order`), `sha2` 0.11, `wait-timeout` 0.2 only; explicitly **no clap, no async runtime** (D10 wi-b03273; port plan "Constraints": "No async runtime (ASY-01)"); MSRV 1.96 (D4). [read]

**Platform constraints.** Linux and Windows unit-tested in CI; macOS only via the on-demand `runner-probe.yml` (wi-5fbb56). Hooks run in Cowork (exec-form `node` works there) but the plugin is missing from Claude Code cloud sessions unless installed from the environment's setup script (`docs/troubleshooting.md:41-52`). Cloud containers lose `.git/temper/`, hence the telemetry branch. Root `CLAUDE.md` is forbidden because the repo root is the plugin root (`AGENTS.md:58`). [read]

## 4. Contracts and integration surface

| Contract | Direction | Where |
|---|---|---|
| **Vendored CLI** `.claude/temper/{temper.mjs, lib/*, VERSION}` | temper writes into weigh-in, anneal, redkiln-rs (build branch), example workspace | `scripts/lib/vendor.mjs`; byte-identity enforced for the example (`tests/ci.sh:33`); adopter CI flags (not fails) a changed copy (wi-bd1cde, #106) |
| `node .claude/temper/temper.mjs baseline --check` | Every adopting repo's charter and CI names this command | weigh-in `AGENTS.md` Commands table; anneal `AGENTS.md:19`, `README.md:84` |
| Charter boilerplate (iron rules H-01..H-16, Process line, "temper (Claude Code specifics)") | Copied into adopters' `AGENTS.md`/`CLAUDE.md` from `templates/workspace/AGENTS.md`, `CLAUDE.md` | weigh-in and anneal `AGENTS.md` carry it verbatim |
| `/temper:*` commands, `TEMPER_CLI`, `TEMPER_BASE`, `TEMPER_DISABLE` env | Adopters' docs and sessions | `hooks.mjs:95-98`, weigh-in `CLAUDE.md` |
| `.claude/temper.json`, `.claude/temper-baseline.json` | Committed in adopters | weigh-in has both |
| **Status block** (end-of-turn grammar): `Status: DONE\|WIP\|BLOCKED`, `Gate:`, `Review:`, `Changed:`, `Decisions needed:`, `Declined:`, `Not verified:` | Parsed by the Stop hook (last `Status:` line wins, markdown decorations tolerated: `hooks.mjs:1174-1177`) | `skills/rust-craft/references/process.md:160-180` (PROC-10) |
| **Reviewer verdict** fenced ```json with `temper_verdict: 1`, `verdict`, 16-hex `fingerprint`, `findings[{rule,severity,file,line?,evidence,fix}]`, `scores{12 dims: 0-4 or n/a}`, `gate{}`, `evidence{files_read,commands_run}` | Produced by `rust-reviewer`, validated by hooks | `verdict.mjs:38-101`. assay deliberately does **not** consume it (local-only state), only the findings JSON (assay `docs/architecture/07-decisions-assumptions.md` ADR-06) |
| `baseline --check --findings-out FILE` findings JSON (`temper_findings: 1`), `report.json` | Read by assay's planned temper adapter | assay `docs/review/01-adversarial-review-round-1.md:28-35` checked exit-code semantics against `baseline.mjs:445` |
| `status --json` (`temper_status: 1`) and `stats --json` (`temper_stats: 1`) | New public, versioned JSON for the lens, CI and scripts; the state layout stays private | `scripts/temper.mjs:212-268, 558-580`, #108 |
| `.kb/_intake/decisions/*.md` (41 atoms) | **Written by Weigh-In** into temper; temper reads none of them at runtime | `AGENTS.md:57`, `docs/history/decisions.md` (index) |
| Telemetry branch | Pushed from adopters' sessions to their own remote | wi-641f99 |
| Research on native binaries | Moved here from weigh-in; compares weigh-in's libexec bundling with temper's npm plan | `docs/research/shipping-native-binaries/README.md:7-9` |

Cross-repo references: temper's code references no sibling. Its docs cite weigh-in (research, pilots) and redkiln-rs (pilot). Siblings depend on temper only through the vendored CLI, the charter text, the `.claude/` files and the skill. redkiln-rs's design never mentions temper (`redkiln-vs-flow.md:29`). [read]

## 5. Backlog and plans

**Open issues (24), by theme** [measured via GitHub, 2026-10-13]:

| Theme | Issues |
|---|---|
| v1 go-live (target 1.4.0) | #6 tracking; #15 collect pilot findings (pilot report scheduled 2026-10-18, wi-292a7d); #16 fix pilot findings and cut 1.4.0; #19 pre-release model checks; #20 go-live checklist |
| 2026-10 audit leftovers (P1/P2) | #58 standard (31 review-only MUST rules, 17 scan-advisory-only rules); #61 release/CI (ruleset not verifiable, no macOS/arm64 CI); #62 v1 plan (P1: critical path, owner budget alerts); #63 pilots (required-check verification, redkiln-rs's 6,606-finding baseline never tightened); #64 port readiness (stale test list, crates.io single-owner bus factor); #66 closure of earlier review findings; #70 backlog hygiene (two-way defaults lack review trail). #57, #59, #60, #65, #67, #68, #69 closed |
| 2.0 port | #21 tracking; #22 fold pilot findings; #23 release environment and npm trusted publishing (crates.io done 2026-10-11); #24 tiered CI (waits for Phase 0a, wi-932e17); #103 setup recognises older pristine templates |
| temper lens (P2, after v1, wi-2dd2cf; strictly read-only, wi-4aa7eb) | #107 tracking; #109-#113; held PRs #117-#121, #123 |
| Evals | #101 orchestration-loop and more rule families (4 cases authored, #126) |

**Live plans.** `docs/plans/v1-readiness.md` (Phase 0 done; Phase 1 done; Phase 2 pilot running on weigh-in and redkiln-rs since 2026-10-04; Phase 3 mostly done; Phase 4 go-live unticked; Phase 5 = the port). `docs/plans/temper-2-rust-port.md` (9 phases 0a..8, ~26,000 lines added and ~14,000 deleted at cutover, "Next: Phase 0a, after v1 goes live", `:920`). [read]

**Decisions that constrain the suite** (`docs/history/decisions.md`, `.kb/_intake/decisions/`):

- **wi-30ed25**: port to Rust, shipped as per-platform npm packages `@wet-ink/temper-<platform>-<arch>` (D1 wi-836680) with a cargo fallback into `${CLAUDE_PLUGIN_DATA}`, a Node launcher, exec-form hooks. **D2 wi-222f58**: publish four crates `temper-cli`, `temper-domain`, `temper-app`, `temper-host` (names reserved as 0.0.0 on crates.io, trusted publishing bound to `release.yml`). **D3 wi-0b4dc2**: four crates plus xtask, layered domain/app/host/cli. **D10**: only regex, toml, sha2, wait-timeout. **D11**: 2.0 is Rust-only. **D13/D14/D15**: user CI installs via `taiki-e/install-action`, pin in `.claude/temper-version`, hash-guarded `setup --migrate`. **D16**: behaviour byte-identical with 1.x except ten listed divergences. Licence `MIT OR Apache-2.0` for crates (wi-4679b3).
- **wi-48fb44**: verify-install on the claude.ai org-sync and cloud setup-script paths gates Phase 0b.
- **wi-b63bd6**: no behaviour change to hooks or gate while a pilot runs on `main` (held until the 2026-10-18 report). **wi-880b66**: agents may merge green PRs with a fresh-context approval. **wi-d75e63**: the repo is the `temper` marketplace (the name `wet-ink` was kept for a possible all-plugins marketplace). **wi-fd3ac9**: stay internal.
- **wi-d23383, wi-bd1cde**: CI flags, never fails, on a raised baseline or a changed vendored copy. **wi-46d668**: Stop passes with a notice (UNAVAILABLE) when cargo is unusable.

**In flight now:** the pilot window (ends ~2026-10-18), lens PRs on hold, eval cases for #101, stats fix #125. No port code. [read]

## 6. Shared-foundation candidates (for Redkiln Core or a shared craft engine)

| Candidate | Where today (size) | Sibling duplicate | Verdict [inferred] |
|---|---|---|---|
| **Event store opening / envelope / ids** | temper has none: plain JSON files and a JSONL telemetry stream under `.git/temper/` | weigh-in, anneal: happenstance 0.3.2 SQLite, TRUNCATE (weigh-in `Cargo.toml:16-17`, anneal `Cargo.toml:27-30`) | temper is the **odd one out**. Core's envelope should be designed so that temper's `telemetry.jsonl` events (22 names) map onto it, and so that a verdict and a gate result can be events. See §8 for the conflict with the port plan. |
| **Store location** | `<git-common-dir>/temper/` resolved with `git rev-parse --git-common-dir` (`project.mjs:67-72`) | Same rule in weigh-in and anneal (`<git-common-dir>/<name>/`) | Same rule, three implementations. Core owns the resolver; temper's state moves to `<git-common-dir>/redkiln/` or keeps a `rust/` subdir. |
| **Tree fingerprint and content-bound approval** | `fingerprint()` 30 lines plus `RUST_RELEVANT` pathspecs (`project.mjs:484-605`); verdicts keyed by it | anneal: `verdict set --sha` with `git patch-id --verbatim` (anneal `crates/anneal-cli/src/verdict.rs`, 745 lines); anneal `content_hash.rs`; redkiln-rs `(token, tree_hash, deadline)` (`redkiln-vs-flow.md:26,55`) | Strong Core candidate: one **approval record** schema `{kind, subject_hash, scope_pathspecs, decider, verdict, at}` with the pathspec set as a per-plugin parameter. |
| **Reviewer-verdict schema and validator** | `verdict.mjs` 120 lines; rubric of 12 dimensions | anneal `verdict.rs` (per PR); assay `assay.findings/1` (spec) | Shape is generic (rule ID, severity, file, evidence, fix, scores). The `findings[]` item belongs in Core next to assay's schema; the 12 Rust dimensions stay in Redkiln Rust. |
| **Status block / end-of-turn grammar** | `parseStatus` 4 lines (`hooks.mjs:1174`); grammar in PROC-10 | weigh-in's `weigh-in` block, anneal's claim gate (`suite-structure.md` §7) | Core spec item (grammar only). temper's `Status:` and `Declined:` lines are the most mature example. |
| **Shell write/delete reader** | `shell.mjs` 1,090 lines, ~22 test cases in `shell.test.mjs`, decisions wi-6e0cc2, wi-8b8a16, wi-2000a3 | anneal's guard denylist and `floor_guard.rs` (1,600 lines: suppressions, deleted tests, loosened thresholds) and a 5,758-line `hook.rs` | High-value shared library (a "guard" crate), language-agnostic. Two independent implementations of "what does this Bash command write" already exist. |
| **Protected-config globs** | `globMatch` (`project.mjs:125-154`), `protected` defaults (`:90-94`) | anneal `glob.rs` (234 lines, case-insensitive) | Merge into the guard crate. Note the semantics differ (anneal ignores ASCII case). |
| **Baseline ratchet** | `baseline.mjs` 504 lines: committed baseline, line-level new-code detection via `git diff -U0`, rename detection, tighten-only | anneal `baseline.rs` (570, session-start snapshot) and `floor_guard` ratchet thresholds | The diff-and-ratchet core is generic ("findings on changed lines or counts above baseline fail"); keys are per-language. Candidate for the shared craft engine, not Core. |
| **Telemetry and stats** | `telemetry()`, rotation, `pushTelemetry` branch push, `statsOf` (`project.mjs:383-495`, `temper.mjs:558-660`) | anneal ledger, weigh-in journal `.kb/_ledger/journal/<session>.jsonl` committed | Fold into Core events. The **telemetry-branch push** is the only working answer here to "cloud containers lose `.git/`"; weigh-in solves the same problem by committing its journal. Core needs one answer. |
| **Hook I/O helpers and fail-open policy** | `ctx/deny/ask/block/notice` (`hooks.mjs:37-43`), never-wedge wrapper (`temper.mjs:69-92`) | anneal `hook.rs` output helpers ("hooks fail open, guards fail closed") | Small; a shared Rust `hook-io` module once temper is Rust. |
| **Launcher / binary packaging** | Planned: npm optionalDependencies plus Node launcher plus cargo fallback (port §5) | weigh-in and anneal: prebuilt binaries in `plugin/libexec/<target>/` behind a launcher | **Divergent plans**; see §8. Core should own one packaging scheme and one launcher. |
| **Release and CI scripts** | `tests/release-check.sh`, `release.yml` (107 lines, owner-approved environment), `changelog.yml` (41), SHA-pinned actions | weigh-in `scripts/changelog-section.sh`, anneal `scripts/check-versions.sh`, `release_check.rs` | Three version-agreement checks. A monorepo wants one, parameterised per plugin. |
| **The gate (step runner, per-fingerprint cache, Stop/SubagentStop choreography, review requirement)** | `gate.mjs` runner and cache (~150 of 366), `completionGate` (~60, `hooks.mjs:1179-1240`), `subagentStart/Stop` | anneal's gates, assay's verdict | The language-agnostic engine named in `suite-vs-redkiln.md:94`. Belongs in a **shared craft engine** library, not Core (it is behaviour). |
| **Vendoring mechanism** | `vendor.mjs` 113 lines | none | Retired by the port (D11, D14). Do not generalise it. |
| **Calibration / capacity** | none in temper (eval Δ tracking is manual, `docs/history/evals.md`) | anneal `calibrate.rs` (2,592 lines) | temper is a consumer at most (reviewer-verdict accuracy would be a calibration target). |

## 7. Granularity findings

1. **Three natural layers inside temper** [inferred]:
   - **The standard** (`skills/rust-craft/` plus `agents/` plus `templates/`, ~14k lines of Markdown, 182 rules, Python checkers): pure content, versioned by rule IDs. It is consumed by four sibling repos' charters, by assay's reviewers and could be consumed by Loop's activities. It could ship as the skill inside Redkiln Rust and also be referenced by Review and Loop.
   - **A generic enforcement engine**: skill-load gate, session base, fingerprint, completion gate, verdict validation and storage, shell guards, telemetry, baseline ratchet, report shell. About 3,500 lines today.
   - **Rust policy**: scanner (56 rules), lexer, manifest policy, cargo steps, cargo-message parser, rule map.
   The port plan's crate split (domain/app/host/cli) cuts **horizontally by layer**, not along this language seam: `temper-domain` mixes the Rust scanner with the generic verdict, glob, fingerprint and shell reader (port plan §1.3). For a suite with ts-craft and py-craft coming, the vertical seam (generic engine vs Rust policy) matters more.
2. **Merge candidates with anneal (Redkiln Bench).** Shell guards, protected globs, floor-guard vs H-12 asks, baseline snapshots vs ratchet, content-bound verdicts, Stop-hook claim gates, statusline vs lens. Each pair is a second implementation of the same idea. With both plugins installed, a turn is refereed twice (two `Stop` gates, two Bash guards). `suite-vs-redkiln.md:51` already names this as the most likely clash.
3. **Overlap with Redkiln Review (assay).** temper's `rust-reviewer` plus verdict is an in-session reviewer; assay is the PR-time reviewer that consumes temper's findings JSON but not its verdict (assay ADR-06). Both need one findings schema. The verdict's `findings[]` and assay's `assay.findings/1` should be one Core schema.
4. **temper lens is a separable package.** It is a mod (`hooks.json` `"modules"`), read-only, reading only `status --json`. A suite-wide "Redkiln lens" over every plugin's `status --json` is the natural generalisation; shipping it as temper-only now creates a third status UI next to anneal's statusline. [inferred]
5. **Templates and example workspace.** `templates/example-workspace` (3,140 lines of Rust, its own `Cargo.lock`) is the standard's reference implementation and a CI fixture. In a monorepo it must be excluded from the root cargo workspace or it becomes a member. [inferred]

## 8. Evidence that challenges the suite decisions

1. **The port plan rules out the Core store as written.** Core's decision is one happenstance SQLite file with a shared envelope. The approved port plan says: on-disk state keeps its 1.x formats with "no schema migration" (`docs/plans/temper-2-rust-port.md`, "Constraints", and §2.3 "State compatibility"); D10 approves only four new crates and rejects others; "No async runtime (ASY-01)". happenstance's API is async (anneal drives it with a current-thread tokio runtime, anneal `Cargo.toml:37`). Linking Core into Redkiln Rust therefore needs **new owner decisions** that amend D10, the no-async constraint and the state-compatibility invariant (#3 of the port's invariants). Do it before Phase 0a, not after.
2. **A shared event file is risky on temper's hot path.** temper writes telemetry from every `PreToolUse` on Bash and Edit (15-30 s budgets) and runs a gate for up to 1200 s at Stop. The suite note's own table warns that "hooks from four plugins serialise on one lock" and that a plugin refusing a newer schema "can disable siblings" (`suite-structure.md` §2, "One store or many"). The decision taken (one shared file) contradicts that note's recommendation (one file per plugin). temper's telemetry is best-effort today (`project.mjs:386-402`); under a shared file a contended write must stay non-blocking. [inferred]
3. **The npm distribution plan does not reach Cowork or synced installs.** The port ships the binary through npm optionalDependencies (D1, wi-30ed25). temper's own research found that Claude Code's npm auto-install **did not run** for a claude.ai-synced plugin in Cowork (`docs/research/shipping-native-binaries/report.md:126-146`: "Per-platform npm packages do not reach Cowork"), and that cargo is not in either Cowork image. wi-48fb44 makes verify-install on those paths a Phase 0b exit criterion, so the risk is known, but the suite's choice of `plugin/libexec/<target>/` packaging (weigh-in, anneal) is the route the research says works. A shared Core launcher argues for **reopening D1/D6/D13 in favour of libexec bundling**, which would also remove the npm org, six npm packages and the lockfile guard.
4. **Four published crates under the `temper-*` names conflict with the rename.** D2 reserved `temper-cli/-domain/-app/-host` on crates.io (single owner, #64 `port-2.0-1-19`) and the npm scope packages are named `temper-*`. Becoming Redkiln Rust (`rk-rust`, `suite-vs-redkiln.md:87`) means new names, or publishing crates whose name contradicts the plugin. If the crates move into a monorepo and are only linked into the plugin binary, D2's reason for publishing (cargo-binstall for user CI, D13) is the only remaining one. Worth re-asking whether user CI should install a `redkiln` binary instead of per-plugin binaries. [inferred]
5. **"No craft umbrella plugin yet" is supported; "shared engine later" is cheaper now than after the port.** The port plan sizes ~14,500 lines of production Rust laid out along a Rust-only crate split. Extracting the generic engine after the port means re-splitting freshly reviewed, parity-tested crates. If ts-craft is plausible within a year, the port should cut a `craft-engine` crate (verdict, fingerprint, completion gate, shell guards, ratchet) from day one. This does not challenge the "one plugin per language" decision; it challenges the port's crate boundary (D3). [inferred]
6. **The rename touches stored contracts, not only names.** `temper_verdict`, `temper_baseline`, `temper_findings`, `temper_status`, `temper_stats` JSON keys; `.claude/temper/`, `.claude/temper.json`, `.claude/temper-baseline.json`; `.git/temper/`; `TEMPER_*` env; `refs/temper/telemetry-tip`; the `temper-telemetry` branch. The port invariant #3 requires both engines to read each other's files. A rename during the port doubles the compatibility matrix; do the rename either before Phase 0a or as a 2.x step after cutover. [inferred]
7. **No challenge found** to: Backlog = GitHub Issues (temper already runs entirely on issues, labels, sub-issues and priority labels, wi-7c3212); Weigh-In as the decision channel (temper consumes it as designed); Docs deferred.

## 9. Migration notes for a monorepo move

- **History size.** 233 commits, 1.73 MiB pack, 1,983 objects [measured]. Trivial to import with `git filter-repo --to-subdirectory-filter plugins/rust` or a subtree merge.
- **Root = plugin root = marketplace root.** `marketplace.json` uses `"source": "./"`; `tests/ci.sh`, `tests/release-check.sh`, `ci.yml` (`claude plugin validate --strict .claude-plugin/plugin.json` and `.`), `AGENTS.md` paths and the vendoring recipe all assume it. The port plan puts the **Cargo workspace at the plugin root** ("plugin root = git root = Cargo workspace root", §1.1), which collides with a monorepo root workspace. Rewrite §1.1 before Phase 0a. The no-root-`CLAUDE.md` rule moves to the plugin subdirectory. [read]
- **Marketplace and sync coupling.** Users run `claude plugin marketplace add Wet-Ink-Corporation/temper` then `install temper@temper` (`CHANGELOG.md:135`); adopters' `.claude/settings.json` (`extraKnownMarketplaces`) and cloud setup scripts name that repo (`docs/troubleshooting.md:43-52`); claude.ai organization sync is configured on `Wet-Ink-Corporation/temper` with "Sync automatically" (`CHANGELOG.md` Unreleased, Documentation). Moving the plugin breaks all three until each adopter and the org-sync setting are updated. Keep the old repo as a redirecting marketplace (a `marketplace.json` whose source points at the monorepo path) for a transition period. [read, inferred]
- **"main is what users get".** Any merge to the monorepo's `main` would ship every plugin. temper's release model (release branch, owner-approved environment, merge-commit tag on `main`) needs per-plugin tags (`rust-vX.Y.Z`) and a per-plugin `release-check` (it currently checks `plugin.json`, the CHANGELOG heading, the vendored `VERSION` and a `vX.Y.Z` tag). [read]
- **Secrets and trusted publishing.** `release.yml` runs in the `release` environment with the owner as required reviewer. crates.io trusted publishing for the four crates is configured for this repo and `release.yml` (v1-readiness 0.8, #23); it is bound to repository and workflow, so it must be reconfigured for the monorepo. npm trusted publishing is not yet set up (#23). No long-lived secrets in CI. [read]
- **Required checks.** The `Protect main` ruleset requires `Checks`, `Unit tests (Windows)` and `CHANGELOG entry` by job name; weigh-in requires `gate`, redkiln-rs's build branch requires `temper` (wi-1f933a). Renaming jobs in a monorepo needs matching ruleset edits (owner-only). [read]
- **Vendored copies.** `.claude/temper/` exists in weigh-in, anneal and redkiln-rs (build branch) and in `templates/example-workspace`. Their CI and pre-commit hooks run it with `node`; their charters name it. The port retires vendoring for a pinned binary (D14 `.claude/temper-version`, D15 `--migrate`), and both pilots' `rust.yml` are customised, so migration prints manual steps (#64 `port-2.0-1-13`). A monorepo also opens a third option: siblings in the same monorepo use the workspace binary directly. [read]
- **Pinned versions.** Toolchain 1.98.1 in `ci.yml` and `templates/*/rust-toolchain.toml` (shared with weigh-in and anneal); Claude Code CLI 2.1.286 in `ci.yml`; SHA-pinned actions via Dependabot. Monorepo CI must keep the Windows job (Windows path handling is a tested contract). [read]
- **Paths that break on move.** `hooks.mjs:33-35` resolves the CLI from `CLAUDE_PLUGIN_ROOT` or relative to its own file (safe). `vendor.mjs:22-28` reads `../.claude-plugin/plugin.json` relative to `scripts/` (safe inside the plugin dir). `gen_rulemap.py` and `doc_examples.py` resolve `skills/rust-craft/` from the repo root (needs a root parameter). [read, inferred]
- **Process load.** The pilot freeze (wi-b63bd6) and six `[hold]` PRs mean temper's `main` is deliberately quiet until about 2026-10-18; a move before 1.4.0 ships would collide with the v1 release and the pilot report. [read]
