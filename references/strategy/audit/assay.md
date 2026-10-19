# Audit: assay (to become Redkiln Review)

*2026-10-13. Repo `Wet-Ink-Corporation/assay`. I read `origin/main` at `3c7e9da` (extracted with `git archive`), all of `docs/architecture/` (spec v1.2) and `docs/review/` (three rounds), the handoff note, the decision atoms, the 174 issues and PRs #102 to #216.*

> **Correct the brief first.** The brief, `suite-structure.md:44,85` and `suite-vs-redkiln.md:24` all call assay "spec only". That was true of the stale local checkout. That checkout is on branch `spec/architecture`, five commits deep, at `3008628`. **`origin/main` is not spec-only.** From 2026-10-11 to 2026-10-13 an unattended run merged about 33 Story PRs. They add roughly 47.6k lines of Rust source and 40.7k lines of Rust tests across six crates, with about 1,969 test functions [measured]. The only cash-free things still missing are the GitHub adapter and everything behind it. To see main, I had to run `git fetch origin` in `/home/claude/assay`. That updated the remote-tracking refs only. The worktree and branch are untouched.

---

## 1. Identity

- **Job.** assay decides whether a PR may merge. It runs deterministic gates (temper, Weigh-In, `exec`) and a tiered fan-out of read-only `claude -p` reviewers. A pure `decide` function adjudicates all of it into **one required check, `assay / verdict`**, which a dedicated GitHub App publishes. Every run persists a bundle so the decision can be replayed, and an eval harness measures the reviewers. Sources: `docs/architecture/README.md`, `AGENTS.md:1-3`. [read]
- **Maturity** [measured on `origin/main`]:
  - Version `0.1.0` (`Cargo.toml [workspace.package]`). No tags and no releases (`git ls-remote --tags` is empty; the handoff says "M1 v0.1.0 waits on Ryan's M1 tree review and genesis (#38)").
  - 46 commits on main, dated 2026-10-10 to 2026-10-12 UTC. Last merge `3c7e9da` (#216), 2026-10-13.
  - One open PR, #215 (`assay check --local`, #107).
- **Lines of code** [measured, `wc -l`]:

  | Area | Lines |
  |---|---|
  | Rust source | 47,630 |
  | Rust tests | 40,741 |
  | Markdown, excluding corpus | 3,959 |
  | JSON and TOML (schemas, fixtures, config) | 6,847 |
  | `.github` YAML and shell | 1,307 |
  | Vendored temper `.mjs` (`.claude/temper/`, VERSION 1.3.0, the same as temper's current `plugin.json`) | 7,755 |

  Per crate, source and then tests:

  | Crate | Source | Tests |
  |---|---|---|
  | `assay-app` | 24,941 | 20,227 |
  | `assay-gates` | 9,946 | 6,397 |
  | `assay-domain` | 5,193 | 4,786 |
  | `assay-forge` | 4,288 | 1,571 |
  | `assay-runner` | 1,680 | 2,478 |
  | `assay-cli` | 1,582 | 5,282 |

- **Tests.** 1,969 occurrences of `#[test]`, `#[tokio::test]` and `proptest!` [measured, grep]. Proptest regression files are committed. `corpus/golden/` holds 26 rubric cases. `corpus/dev`, `corpus/test` and `corpus/inject` hold only README files [measured].
- **Process.** The plan lives entirely in GitHub Issues, using issue types:
  - #1 is the Initiative.
  - #2 to #8 are the Epics, milestones M0 to M6.
  - #9 to #34 are Features.
  - #35 onwards are Stories and Tasks, linked by native "blocked by" dependencies.
  - Custom fields Priority and Effort are in use.

  This is already the suite's chosen backlog model [read] (issue #1, `docs/handoff-2026-10-12.md:9-11`).
- **Distribution.**
  - **Binary.** A `v*` tag publishes the `assay` binary to GHCR as an OCI artifact with oras 1.3.4. Consumers fetch it by digest with `.github/actions/fetch-by-digest` (`.github/workflows/release.yml:15-17,104-120`, AGENTS decisions log 2026-10-11, ADR-14).
  - **Plugin.** Not packaged. There is no `.claude-plugin/` anywhere. `plugin/` holds only prompts and README stubs. A marketplace entry is planned in #127.
  - **Consumer workflows.** `workflows/*.tmpl` is still a README (#60, #61 open). [measured]

## 2. Functional shape

### 2.1 What a user can invoke today [measured: `crates/assay-cli/src/main.rs:23-50`]

| Surface | State |
|---|---|
| `assay self-gate` | **Built.** The `assay-self` golden gate replays `decide` over `corpus/golden/**` and exits 0 or 1 (T16, #76) |
| `assay doctor` | **Partly built.** Covers token-expiry warning and canary probes (#93, #121). Ruleset and App checks wait on #53 |
| `assay status` | **Parses arguments only.** It returns "the GitHub adapter it reads through is not built yet (#53)". The `assay.status/1` schema and the app logic are built (#126) |
| `assay plan`, `admit`, `open`, `decide`, `replay`, `review`, `report`, `check --local` | **Not wired in the binary.** The application use cases exist for `plan` (#55), `decide` (#56, #57), `replay --verify` (#74) and `publish` (#88). The verifier exists for lite (#91) and full (#97), as do the specialists and the bounded fan-out (#95), preflight (#83) and the egress scan (#87). Each is tested against fakes behind ports. `check --local` is PR #215 (open) |
| Slash commands `/assay:check`, `/assay:replay`, `/assay:report` | Planned (`plugin/commands/README.md`, #108) |
| Hooks | None. A pre-push helper is planned (#108, #132). It **never denies** (wi-f626a4) |
| Agents and skills | None |
| Claude Code "mod" (in-session hooks module: verdict pane, finding actions, live local run) | Planned as optional M7 (#123, #124, #127 to #132, #137). ADR-23 is proposed in #125. Owner actions (Waive and Override post the `/assay` comment as the human) were decided in `wi-f626a4` |

### 2.2 The capability map from the spec, with build state

| Capability | Spec | Code state [measured, issue state] |
|---|---|---|
| Plan and tiering (`needs-split`, `trivial`, `full`, `lite`, `micro`), trust paths, exec surface | `04 §2` | Built: `assay-app/src/tiering/**`, `assay-domain/src/tiering/**` (#50, #65) |
| temper adapter, Z2s: `raised`, `exception`, gate-config diff, test inventory, manifest switches, lints, inheritance | `03 §2.1` | Built: `assay-gates/src/temper/**`, 26 files (#63, #64) |
| temper adapter, Z2b `baseline --check` with ratchet assertions | `03 §2.1` step 5 | Built (#66) |
| `exec` adapter and `plugins.toml` | `03 §1` | Built (#62) |
| Weigh-In adapter (CP1 and CP2) | `04 §9` | **Not started.** It does not appear in `assay-gates/src/` (#29, #30, #106, #109, #110). Upstream contract D-WI is not filed (#44) |
| Findings validation (schema, evidence relocation, placement, trigger, injection screen, suppression) | `04 §4` | Built: `assay-app/src/validation/**` (#86) |
| Fingerprint and normaliser | `03 §3` | Built: `assay-domain/src/{fingerprint,normalise}.rs` (#49) |
| Rubric v1 and pure `decide` | `04 §5` | Built: `assay-domain/src/decide/**`, `rubric/v1.toml` (#51, #92) |
| Authenticating inputs, recomputing the plan, publishing, persisting the bundle and its pointer | `02 §4-5` | Built behind ports (#56, #57) |
| `ModelRunner` with `ClaudeHeadless` and `Replay`; preflight; clean init with built-in-mod allowlist | `03 §7-8` | Built: `assay-runner/**` (#82, #83, #121) |
| Specialists, coordinator, verifier | `04 §3` | Specialists and verifier built (#95, #91, #97). Coordinator open (#96). Sharding open (#98) |
| Sanitiser and egress scan | `03 §3`, `04 §6` | Built (#87, #88) |
| Ledger, semaphore, `admit`, `release`, `reconcile` | `06 §1` | **Not built.** The port `PublishLedger` exists (`assay-app/src/publish/ports.rs:120`). The adapter is #70, slots #71, admit #72 and reconcile #73. All are blocked on the owner creating the `assay-ledger` repo (#40) |
| Forge (`gh`) adapter | `02 §2` | **Not built** (#53, blocked on the App #39) |
| Workflows `assay.yml` and `assay-decide.yml` | `02 §1`, `05` T19-T22 | Not built (#60, #61) |
| Re-review, dismiss, retry, requeue | `04 §6-7`, `06 §1` | Not built (#99, #100, #101, #103) |
| Eval harness, corpus, shadow mode, report | `04 §8`, `06 §7` | Not built (#111 to #115) |

**Summary.** The pure core and the adapters that do not touch GitHub are done. Everything stateful and networked waits on four owner actions: the App (#39), the ledger repo (#40), the approver account (#41) and the agent identity (#42). M0 and M1 cannot close until those exist (`docs/decisions/0002-owner-answers-h01-h14.md`). [read]

## 3. Architectural shape

### 3.1 Crates and layering [measured]

Six crates. Dependencies run `domain ← app ← {forge, runner, gates} ← cli`. A test enforces this: `crates/assay-cli/tests/it/architecture.rs` reads `cargo metadata` and fails on any other edge, on a seventh crate, or on a publishable crate (`AGENTS.md` crate map).

- **`assay-domain`** depends only on `serde`, `sha2` and `thiserror`. It holds findings, closed vocabularies, the `Event` envelope, `RunUid`, `Aeu`, fingerprints, rubric, tiers and `decide`.
- **`assay-app`** holds use cases plus about 15 small port traits: `ModelRunner`, `GateRunner`, `VerdictForge`, `CommentForge`, `PublishLedger`, `BundleStore`, `PullRequestState`, `RunArtifacts`, `StatusForge`, `CliProbe`, `CanaryProbe`, `BundleSource`, `SuppressRules`, `NonceSource`, `PullRequestObjects` (grep of `crates/assay-app/src`). The spec's six named ports were split per use case.
- **`assay-forge`** holds the hardened git layer, process-group supervision and `/proc` scanning (`src/process.rs`, 1,175 lines). It has no GitHub code yet. Note: `AGENTS.md` says forge depends on app and domain, but its `Cargo.toml` has only `tempfile` and `thiserror`.
- **`assay-runner`**: supervises `claude -p`, parses stream-json, accounts for cost.
- **`assay-gates`**: the temper and exec adapters.
- **`assay-cli`**: the composition root.

### 3.2 Runtime model: all three, in strict zones [read]

The question was whether assay is a GitHub App, an Action, or headless `claude -p`. **It is all three, separated by trust zone.**

- **GitHub Actions** is the compute. Two base-defined workflows, `assay.yml` (`pull_request_target`, with no `branches:` filter) and `assay-decide.yml` (`issue_comment`, `labeled`, `schedule`), split work into jobs Z0 to Z4. No job combines PR-code execution, the model secret and a write token (`02 §1,3`, ADR-03, ADR-12).
- **A dedicated GitHub App, `assay-verdict`,** is the only publisher of the required check. The ruleset pins its `integration_id`, because any PR-added workflow posts as "GitHub Actions" (ADR-11, T7). The same App holds a second installation on the ledger repo (H-14).
- **Headless `claude -p`** runs only in the Z3 `review` job, with `CLAUDE_CODE_OAUTH_TOKEN` from `claude setup-token` on the Max subscription (ADR-02).
  - Hardened profile: `--restricted --safe-mode`, `--tools Read,Grep,Glob` (no Bash), an empty cwd with the PR tree passed through `--add-dir`, `--json-schema`, `--max-budget-usd = cap`, and `--output-format stream-json --verbose` (`03 §8`).
  - Built and asserted in `crates/assay-runner/src/headless.rs:171-185`.
  - The `ModelRunner` port has `ClaudeHeadless`, `ClaudeApi` (the same CLI with an API key) and `Replay` adapters (`03 §7`).
- **Local mode.** `assay check --local` uses the interactive `claude` and is advisory only (P9, ADR-10).
- **Not in-session.** assay is an orchestrator outside the chat, like redkiln-rs Loop, and unlike temper, Weigh-In and anneal. The planned mod is the only in-session piece, and it is a read-only view plus human-pressed commands (#125).

### 3.3 Data owned [read, measured]

| Store | Where | Format | Keyed by |
|---|---|---|---|
| Versioned contracts | `schemas/*.schema.json` | JSON Schema; unknown fields and versions rejected; 8 MiB cap; duplicate keys refused | `assay.findings/1`, `assay.run/1`, `assay.decision/1`, `assay.event/1`, `assay.run-inputs/1`, `assay.status/1` |
| Per-job event stream | One JSONL per job inside the run; merged by Z4 into the bundle | Envelope `{schema, job, idx, ts, run_key, run_uid, stage, kind, data}`. **No global seq**; an "audit record, not an event-sourced aggregate" (`03 §6`) | `run_uid = {repo_id, run_id, run_attempt}` (`assay-domain/src/run_uid.rs`) |
| Run-input bundle | Actions artifact, 90-day retention | `assay.run-inputs/1`: `{schema, run_uid, evidence_key, documents[], inputs, rubric_version, decision, events[]}` (`03 §6a`) | `evidence_key` and `decision_key` (sha256 compositions, `02 §5`) |
| **Central ledger** | Private repo `Wet-Ink-Corporation/assay-ledger` | Cost records `ledger/YYYY-MM.jsonl` (`reserved` or `final`, deduplicated by `run_uid`); `pushes/<repo_id>/<pr>.jsonl`; `commands/<repo_id>/<pr>.jsonl`; `state/<repo_id>/<pr>.json`, written with compare-and-set on the file sha. **Semaphore** refs `refs/heads/slot-0` and `slot-1` carry lease commits (TTL 45 min). Retry-grant refs. Per-head publish lock `refs/heads/lock-<repo_id>-<head_sha>` (TTL 2 min). Release is `git push --force-with-lease` (`06-operations.md:36-43` on main; ADR-19, ADR-23) | `run_uid`, repo_id, PR |
| Local mode output | `.git/assay/<tree-fp>.json` | advisory | tree fingerprint |
| Decisions | `.kb/_intake/decisions/wi-*.md` (14 atoms on main) and `.kb/_ledger/journal/assay-afk-2026-10-12.jsonl` (331 lines, 88 defaults) | Weigh-In formats | `wi-` ids |

- **No SQLite and no happenstance.** None of the six crates depends on happenstance, tokio or rusqlite (`Cargo.toml [workspace.dependencies]`). The workspace dependencies are thiserror, tempfile, jsonschema 0.58.6, serde_json, serde, toml 1.1, sha2 0.11, proptest and syn [measured].
- **The authoritative state store is a git repository** used through refs and compare-and-set, deliberately, because GitHub Actions jobs have no shared disk (ADR-19, ADR-23).
- **Toolchain.** Pinned at 1.98.1. MSRV is **1.96**, not equal to the pin. weigh-in and anneal set MSRV equal to the pin. Edition 2024, `publish = false`, licence omitted.
- **Platform constraints.**
  - Linux GitHub-hosted runners only. Egress is blocked in block mode (A-12, unverified, #89). `setpriv --pdeathsig` is under discussion (#154). `/proc` is scanned in forge. There is no Windows or macOS target, and no Cowork story.
  - assay depends on Claude Code CLI behaviour pinned at `claude_cli = "2.1.296"`, version floor 2.1.257 (A-10).
  - The built-in mods allowlist (#121, wi-1fc020) and an accepted Anthropic-bound telemetry mod inside Z3 (wi-8c8bdd) tie it to the CLI's internal plugin roster.

## 4. Contracts and integration surface

| Direction | Contract | Evidence |
|---|---|---|
| assay → **temper** | It runs a **pinned** temper (git SHA or OCI digest, never the PR's `.claude/temper/`): `temper scan --all --json` in Z2s, and `baseline --check --locked --base <40-hex> --findings-out target/temper/findings.json` in Z2b. It parses `{temper_findings:1, findings[{lint,rules[],severity,file,line,message,exception?}], ratchet{mode,base,raised,…}}` and exit codes 0, 1 and 2. It **reads `.claude/temper.json` and `.claude/temper-baseline.json` directly** (base against head, key by key) | `03 §2.1`. A-09 and A-25 assume the contract survives temper 2.0. D-TEMPER-1 and D-TEMPER-2 (a CLI for config diff, inventory, `--fail-on-raise`, blob comparison) are not filed (#45) |
| assay ↔ temper (code) | `assay-gates` **re-implements temper's hook-only checks**: test inventory with a Rust lexer cross-checked against syn, manifest test switches, lints-table weakening and lint inheritance. temper enforces these only in `hooks.mjs`, not in `gate` or `baseline` | `03 §2.1` step 4, `crates/assay-gates/src/temper/{inventory,switches,manifests,lint}.rs` |
| assay → **Weigh-In** | Planned: `weighin open --json` (unversioned; experimental CP1) and a requested **`weighin ci-check --json --base <ref>`**, versioned, with `blocks` as an enum `merge|release|none|unknown`. It would join journal `item_id`, atom `weighin_item` and the `Weigh-In: <id>` trailer, verify journals append-only at event level, and use an owner `/assay weigh-in-ack <set_digest>` | `04 §9`, ADR-17, D-WI (#44 not filed). Facts were verified against weigh-in v0.6.0. Weigh-In is now 0.6.1 |
| Weigh-In → assay | assay's own decisions are Weigh-In atoms (G6 dogfood). The AFK run exported its ledger journal into `.kb/_ledger/journal/` | `.kb/**` on main |
| **anneal → "the review plugin"** | anneal **expects** a review plugin to append `reviewed`, `approved`, `changes_requested` or `noise_only` verdicts into anneal's ledger (`anneal ledger append`). Its `merge-gate` hook requires one once `review.require_plugin = true` | `anneal/crates/anneal-cli/AGENTS.md:324,328`, `anneal/docs/verdict.md:32,59`, `anneal/CHANGELOG.md:15`. **assay has no such output, and its spec never mentions anneal** [measured: grep] |
| assay → GitHub | Check `assay / verdict` with conclusions `success`, `failure`, `neutral` and `in_progress`-as-pending, a closed enum of 20+ reason codes, and `external_id` `pending:<run_id>:<seq>` then `decision_key`. Summary comment marker `<!-- assay-summary -->`. Labels `assay:needs-owner`, `assay:llm-pending`, `assay:requeue`. Comment commands `/assay recheck|waive|ack|override|retry|dismiss|weigh-in-ack` | `03 §5`, `04 §5-6` |
| Consumer repo config | `.assay/{plugins.toml, owners, agents, exceptions.toml, suppress.toml, globs.toml}`, all read from `base_sha` | `03 §1` |
| Env vars set for gates | `ASSAY_BASE_SHA`, `ASSAY_HEAD_SHA`, `ASSAY_RUN_KEY`; the environment is scrubbed to an allowlist | `03 §2` |
| Vendored copy | `.claude/temper/` (temper 1.3.0) for assay's **own** dogfood gate. It is not used by the pipeline | `.claude/temper/VERSION` |
| Machine-readable status | `assay status --json` returns `assay.status/1` for in-session views (the mod and other plugins) | `03 §6b`, `schemas/status-1.schema.json` |

**redkiln-rs and anneal (verify doubt pass) never mention assay as a contract.** anneal's `verify` skill stops at "the PR and its review are … the review plugin's" (`anneal/plugin/skills/verify/SKILL.md:19`) [read].

## 5. Backlog and plans

**Issue state** [measured]: 174 issues and PRs in total, about 30 Story issues closed. Open issues, by theme:

- **Owner setup (`owner-action`): 13 issues.** #36 (record the H answers, now done in `docs/decisions/0002`), #37 (A-01 terms), #39 (App and ruleset), #40 (ledger repo), #41 (approver), #42 (agent identity), #43 (environments), #81, #116, #117, #118, plus #195 and #196 (canary secret, GHCR privacy). **This is the critical path.**
- **Upstream:** #44 (D-WI) and #45 (D-TEMPER-1 and -2). Neither is filed with the sibling.
- **GitHub-bound M1 work:** #53 Forge, #54 open, #58 P10 snapshot, #59 waive and override, #60 and #61 workflows, #67 gates-test, #70 to #73 ledger, slots, admit, reconcile, #75 doctor, #77 break-glass, #78 canary suite.
- **M2 and M3:** #84, #85, #89, #94, #96, #98 to #101, #103 to #105.
- **M4 Weigh-In:** #106, #108 to #110.
- **M5 eval:** #111 to #115.
- **M7 mod (optional):** #123 to #132, #137.
- **Review follow-ups:** about 35 issues, #143 to #218. These are minors and nits folded into later PRs. The one notable item is #184, quadratic code on main.

**In flight.** PR #215 is open. The AFK heartbeat trigger is disabled, and the run was paused at 90% weekly usage (`docs/handoff-2026-10-12.md:5,15`). This is itself a **measured capacity collision**: an unattended build agent and the planned assay reviewers share one subscription window.

**Decisions that constrain the suite (IDs):**

- **ADR-01.** assay is its own repo and plugin, and consumes temper and Weigh-In only through JSON adapters ("no `weighin-core` link"). **P8** says no sibling crate is ever linked.
- **ADR-02.** Max OAuth in headless `claude -p` behind `ModelRunner`.
- **ADR-03, ADR-11, ADR-20.** `pull_request_target`, the dedicated App, and decide-only on default-branch triggers.
- **ADR-04.** `decide` is pure, and models cannot approve (P1).
- **ADR-07.** Rust, six crates. "`assay-events` folded into domain/app (the event store is an audit stream, not an aggregate)".
- **ADR-09, ADR-15.** Decision replay over a 90-day bundle, not event-sourced replay.
- **ADR-13, QA-05.** LLM blockers stay advisory until the lower 90% Wilson bound of blocker precision is at least 70% over at least 20 labelled blockers. Promotion is per repo, and blockers and majors are promoted separately.
- **ADR-16, ADR-24 (R3-01).** Privileged identities are accounts never reachable by an agent. The owner's daily account is only a `maintainer`, and a dedicated approver account is mandatory.
- **ADR-18.** Path B is the agreed fallback: `claude-code-action` plus a pinned temper check, with kill criteria in `06 §8`.
- **ADR-19.** A central ledger and a 2-slot cross-repo semaphore in a git repo.
- **ADR-23** (in the spec at `07:31`; note #125 proposes *another* "ADR-23" for the mod, an ID collision). The ledger repo holds the authoritative per-PR state, and the check run is only a projection.
- **wi-b5affa.** Fail closed: "refuse rather than model".
- **wi-73d2d6.** A review whose only objection is code already on main counts as approval.
- **Open question `07 §5`.** An "executable witness for blockers (a failing test or repro run in `gates-test`)" is **deferred**.

## 6. Shared-foundation candidates (for Redkiln Core)

| Candidate | Where in assay (size) | Sibling duplication | Recommendation |
|---|---|---|---|
| **Calibration**: Wilson intervals, promotion and demotion rule, confusion matrix, live reject-rate band | `QA-05`, `QA-06`, `QA-12` (`01 §7`); `assay-app/src/verify/tally.rs` (reject-rate band 10–60% over at least 20 runs); `04 §8` scorer, k = 3, flip-based acceptance | anneal's Clef Brier and shadow mode, weigh-in's eval seed, redkiln-rs ADR-0079 and its F-findings, which recompute Wilson bounds by hand (`suite-structure.md:205`) | **Strong Core candidate.** assay has the most precise *promotion* rule (lower Wilson bound and minimum n, separately per severity, demotion by upper bound). Extract `record judgement → label → score → promote-by-human-edit` with Wilson and MCC. assay's tally code is small (about 100 lines); the value is in the rule |
| **Capacity**: budget ledger, AEU, a cross-repo semaphore with leases | `06 §1` (ADR-19, ADR-23); `assay-domain/src/aeu.rs` (101 lines); the cost parse in `assay-runner/src/cost.rs`; ledger adapter unbuilt (#70, #71) | redkiln-rs Loop's per-run budgets; the AFK pause at 90% weekly use | **Core should own the *protocol*.** That means the lease record `{run_uid, holder, reserved_aeu, expires_at}`, reserve and final cost records, and the rate-limit signal. The *store* cannot be the local SQLite file: assay's jobs run on ephemeral runners with no shared disk. Core needs two backends, local happenstance and git refs, or capacity must be a remote service. See §8 |
| **`claude -p` runner**: hardened profile, stream-json parser (init, `rate_limit_event`, `result.total_cost_usd`, `error_max_*`), auth preflight, process-group supervision, clean-init allowlist, hook and fence canaries | `assay-runner` (1,680 source + 2,478 tests) plus `assay-forge/src/process.rs` (1,175); fixtures for CLI 2.1.296 in `assay-runner/tests/fixtures/stream/*.jsonl` | redkiln-rs `spikes/runner` (Harness port, throttle.rs, event.rs, the headroom parse) | **High-value Core crate (`redkiln-runner` or similar).** Both orchestrators parse the same CLI stream and fight the same SIGTERM-loses-cost problem. Fixtures pinned per CLI version make a natural conformance kit |
| **Event envelope** | `assay-domain/src/event.rs` and `payload.rs` (486 lines); `schemas/event-1.schema.json` with fixtures | weigh-in and anneal use happenstance events in SQLite | assay's envelope is per-job JSONL with no global seq, and is never in SQLite. Core's envelope should be able to carry assay's `run_uid`, `job` and `idx` as fields, so a decide step *can* project into the shared file when one exists (local mode). Do not force CI onto SQLite |
| **Contract and schema discipline**: versioned ids, validate-on-read, unknown fields rejected, size cap, duplicate-key refusal, golden valid and invalid fixtures | `assay-app/src/contract/**`; `schemas/` with 75 fixtures | anneal and weigh-in have ad-hoc JSON contracts | **Copy into the Core format-spec and conformance-kit practice.** This is the best example in the suite of "contracts tested by fixtures" |
| **Hardened git for untrusted objects** | `assay-forge/src/git/**` (about 2,975 lines with the stream code): `GIT_CONFIG_NOSYSTEM`, `check-attr` policing, a manifest-only sanitised copy | redkiln-rs uses gix; temper's `.gitattributes` blind spot | Keep in Review unless Loop also handles hostile PRs. Possibly a Core "git hygiene" module later |
| **temper re-implementations** (test inventory, manifest switches, lints diff, inheritance) | `assay-gates/src/temper/**` (about 9.9k source, the largest adapter) | temper `hooks.mjs`, Node, now being ported to Rust (temper 2.0) | **Merge candidate.** Once temper 2.0 is Rust in the same monorepo, these belong in Redkiln Rust as CLI subcommands (D-TEMPER-1). assay keeps only the adapter. This removes a large duplicate that drifts |
| Fingerprint and normaliser, `content_hash` | `assay-domain/src/{fingerprint,normalise,content_hash}.rs` (705 lines) | anneal and redkiln fingerprint findings too | Medium. A shared "finding" vocabulary across Review, Bench's doubt pass and Loop's gate would let a finding raised in-session be recognised in CI |
| Process supervisor | `assay-forge/src/process.rs` (wi-603c63 put it in forge so gates and runner share it, #150) | redkiln-rs runner kill and job-object work; anneal hooks | Core candidate together with the runner |
| Delivery: OCI-by-digest release, fetch-by-digest action, warm target | `.github/workflows/release.yml`, `.github/actions/fetch-by-digest`, `warm-target` (about 1.3k YAML and shell) | weigh-in and anneal ship `libexec/<target>/` binaries inside the plugin zip | **Different model.** CI consumers need digest pinning; plugin installs need libexec. Core should define both, not one |

## 7. Granularity findings

**Natural seams inside assay** [inferred from the crate graph and the zones]:

1. **The pure verdict core** (`assay-domain`: rubric, `decide`, tiers, fingerprints), **separate from the CI orchestrator** (`app`, `forge`, `cli` workflows). The domain needs no GitHub. A shared verdict function could serve anneal's in-session merge-gate and Loop's gate as well as CI.
2. **The model runner** (`assay-runner` plus `forge::process`) is generic over the reviewer role. It is the most reusable unit and belongs below both Review and Loop.
3. **The temper adapter** (`assay-gates/src/temper`) is 21% of the source and mostly re-implements temper. It is really Redkiln Rust code living in Review.
4. **The CI deployment** (workflow templates, the App, ledger repo, GHCR) **versus the plugin** (prompts, slash commands, mod, pre-push band). Today they share a repo, but they are different products. The plugin is a thin client over `assay status --json` and `check --local`.

**Overlaps with siblings:**

- **Three review paths:**
  - redkiln-rs ADR-0074: "reviewer proposes, machine confirms". A finding blocks only after a typed repro re-executes.
  - anneal's `verify` doubt pass: reviewers see the diff and the contract, never the claim; file:line or rejected.
  - assay: blind specialists, a blind verifier that never sees the rationale, evidence relocation, and a trigger required from `major` up.

  assay's *blindness and evidence rules* are the same idea as anneal's doubt pass. assay's deferred "executable witness" (`07 §5`) **is** ADR-0074. Today the three paths share nothing: no finding schema, no verifier, no labels.
  - Recommendation: Redkiln Review owns `assay.findings/1` and the verifier, and offers them as a library or CLI (`review --local` already exists as #107 and PR #215).
  - anneal's doubt pass then calls it in-session.
  - Loop's gate calls it headless, with ADR-0074's re-execution added as a verifier mode that runs in a Z2b-class sandbox.
- **Who records the review verdict.** anneal waits for "the review plugin" to append `reviewed` or `approved` into anneal's ledger. assay's spec only writes the GitHub check. With a shared event file, Review should emit a `review.verdict` event (tagged plugin=review) that Bench reads. That replaces anneal's `ledger append` coupling. Nobody has specified this yet.
- **Gate naming.** "Gate" means three things: temper's gate, anneal's gates, assay's verdict. assay already says `assay / gate: <name>` for per-plugin checks.
- **Weigh-In.** assay's CP2 needs a versioned `ci-check`. That is exactly the kind of contract Core's format spec should own, not an assay-to-Weigh-In bilateral request.

## 8. Evidence that challenges the suite decisions

1. **"One shared happenstance SQLite file at `<git-common-dir>/redkiln/events.sqlite3`".** This does not fit Redkiln Review's main runtime [read, strong]:
   - assay's CI jobs run on separate ephemeral VMs (`gates-test` runs on "a different VM" by design, ADR-12) with no shared disk.
   - Its authoritative state deliberately lives in a remote git repo using compare-and-set refs (ADR-19, ADR-23).
   - The spec explicitly rejects an event-sourced store (ADR-07, ADR-09, ADR-15).

   The shared file can serve only local mode and the mod. Core's "one file" decision should say plainly that the CI plugin is a *producer of exported events*, not a writer of the file, or Core needs a second, remote backend.
2. **"Capacity library in Core".** This is right in intent, but the measured need is *cross-machine*: two slots across every repo on one subscription, plus the owner's own sessions (A-03), as the AFK pause at 90% weekly usage showed. A library over a local SQLite file cannot arbitrate between a GitHub runner and a laptop. Capacity is a protocol plus a shared remote store, and the store choice (git refs in `assay-ledger`, or something else) is a Core decision, not a library detail.
3. **"Calibration in Core".** Supported. One nuance: assay's promotion is *per consumer repo* and is changed by a base-config edit that is itself a trust-path change. So Core's "promote by human edit" must allow the edit to live in the consumer repo's config, not only in the suite's own state.
4. **"Separately installable plugins" for Review.** Partly challenged. Most of assay is not a Claude Code plugin at all. It is a CI deployment: workflow templates, a GitHub App, a ledger repo, and a GHCR-delivered binary pinned by digest in the consumer's base branch. A marketplace install cannot deliver the trust anchor (P2: the binary is pinned by digest in `.assay/plugins.toml`). "Redkiln Review" as a plugin will be the thin in-session client. The real product ships as an OCI artifact plus templates. The bundle plugin's dependency mechanism does not cover that.
5. **ADR-01 and P8** (consume siblings only through JSON, never link). In a monorepo, linking Core is fine. But assay's threat model wants the gate tooling pinned by digest *independently of assay's own version* (03 §1 `pin = "git:<sha>"`). A monorepo with lockstep versions must still let a consumer pin temper and assay separately, or the trust model weakens.
6. **Toolchain.** assay's MSRV of 1.96 differs from the pin; weigh-in and anneal use MSRV equal to the pin. A minor divergence for a single workspace.
7. **"Backlog = GitHub Issues".** Strongly supported. assay is the working example: issue types, sub-issues, native dependencies, custom fields, and Weigh-In atoms anchored to issue URLs (`wi-fef408`).
8. **The background write-ups' "spec only".** Factually wrong as of 2026-10-13 (see the top note). Re-weigh the "cheaper to rename now" argument (`suite-vs-redkiln.md:105`). The name `assay` is now in six crate names, the GHCR packages `assay` and `assay-warm-target`, the check name `assay / verdict`, the App `assay-verdict`, labels, `/assay` commands, schema ids `assay.*/1`, `.assay/`, the env vars `ASSAY_*`, and the ledger repo. It is still unreleased, so renaming remains cheap, but it is no longer free.

## 9. Migration notes for a monorepo move

- **History.** Small: a 2.2 MiB pack, 2,073 objects, 46 commits on main, squash-merged with `(#PR)` suffixes [measured]. It can move with `git subtree` or a filter-repo path prefix without pain. A stale `spec/architecture` branch and the `claude/*` branches also exist.
- **Path assumptions that break under a `review/` or `plugins/review/` prefix:**
  - Self trust paths in `.assay/globs.toml` (`crates/**`, `rubric/**`, `plugin/**`, `schemas/**`, `workflows/**`, `corpus/golden/**`).
  - The T16 bootstrap rule: **all of `crates/**` is a trust path for assay's own repo**. In a monorepo that would make every sibling crate's PR an owner-approved trust-path change. The rule must narrow to Review's own crates.
  - `crates/assay-cli/tests/it/{architecture,layout,layering}.rs`, which assert exactly six crates, layout READMEs and no publishable crate. They fail as soon as other workspace members exist.
  - The `plugin/prompts/v1/` hash, which is part of `evidence_key`.
  - `rubric/v1.toml`, read by tests through relative paths.
- **CI and release:**
  - `rust.yml` (gate, msrv, and a weekly cron for latest-deps, aarch64 and miri), using the vendored `.claude/temper` lint.
  - `release.yml` (protected `v*` tags, oras to `ghcr.io/<owner>/assay`). In a monorepo, tags need a prefix (`review-v*`), and the tag-protection ruleset changes with them.
  - `warm-target.yml` builds at `/work/ws` on pushes to main and publishes `assay-warm-target`. In a monorepo it would build every crate. Scope it to Review's crates or a shared warm target.
  - The workflow tests (`workflow.rs`, `workflow_cache.rs`, `release_workflow.rs`, `warm_target_workflow.rs`) parse these YAML files by path.
- **Secrets and owner infrastructure:**
  - Planned and not yet created: environments `assay-review` (OAuth token) and `assay-publish` (App key), the `assay-verdict` App with a second installation on `assay-ledger`, the approver account, the agent identity (#39 to #43), and the canary workflow secret `CLAUDE_CODE_OAUTH_TOKEN` (#195).
  - These are owner actions, so **renaming before creating them is free**. Do the rename first.
- **Marketplace coupling.** None today (no `.claude-plugin/`). #127 plans a repo-root `marketplace.json` with `source: "./plugin"`. In the suite it should go into the shared marketplace instead.
- **Vendored copies.** `.claude/temper/` (temper 1.3.0, 7.8k `.mjs`) exists only for dogfooding. In a monorepo it disappears in favour of the in-tree Redkiln Rust. The pipeline itself must still run a **digest-pinned** temper, never the in-tree one (T4).
- **Unattended-session tooling.** `.claude/settings.json` holds a large permission allowlist. `.githooks/pre-commit` exists. The handoff's rules include "squash-merge with head SHA pinned", "`gh api` REST only; GraphQL is blocked", and a trigger id for the heartbeat routine. These are repo-specific and must be merged with the monorepo's `AGENTS.md`.
- **ID collision.** Spec ADR-23 (ledger state) and the proposed #125 "ADR-23" (mod) clash. Renumber when the decision logs merge.
