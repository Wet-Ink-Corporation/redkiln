# The Redkiln Blueprint

*2026-10-13. Draft for Ryan's review. It describes the end state: what the suite's packages are, what each one owns, and how they fit together. The migration plan from today's six repositories comes after this is agreed.*

*Grounded in five audits written today, one per repository, under [audit/](audit/): [temper](audit/temper.md), [weigh-in](audit/weigh-in.md) (with weigh-in-plugin), [anneal](audit/anneal.md), [assay](audit/assay.md) and [redkiln-rs](audit/redkiln-rs.md). It builds on [suite-vs-redkiln.md](suite-vs-redkiln.md), [redkiln-vs-flow.md](redkiln-vs-flow.md) and [suite-structure.md](suite-structure.md), and keeps every decision recorded there unless section 2 says otherwise. Citations of the form `audit/x.md §n` point at the evidence.*

## 1. In one page

The decisions hold up. A suite of separately installable plugins, one shared event log, a Core that is a library and a spec rather than a plugin, Backlog on GitHub Issues, Knowledge in the first wave, Docs deferred: the audits found nothing that overturns any of them. Three of the six repositories had already reached the same conclusions on their own (GitHub Issues as the backlog in assay, redkiln-rs and temper; happenstance for events in weigh-in, anneal and Loop's ADR-0131; `<git-common-dir>/<name>/` for stores everywhere).

What the audits change is the **granularity**. The suite is much bigger than the write-ups assumed, and the shared code is not one Core library but three kinds of thing:

1. **Core**: the formats, plus the few crates every binary embeds (event log, envelope, ids, store location, journals, calibration, capacity).
2. **Kits**: shared libraries that two or more plugins already implement separately today. Four exist in duplicate: a shell guard, a headless `claude -p` dispatcher, a GitHub client and a generic enforcement engine.
3. **Contract crates**: small crates in which a domain plugin publishes its own event types and schemas, so siblings write to it through types instead of parsing its text output.

Plugins then become thin shells (hooks, skills, a launcher, one binary) over their own domain crates, Core and kits.

```
 ┌──────────────────────────────── products ───────────────────────────────────────────────┐
 │ Redkiln Rust   Redkiln Bench   Weigh-In   Redkiln Knowledge   Redkiln Review   Redkiln Loop   Redkiln Backlog │
 │ standard+gate  in-session      decisions  atoms, recall       PR verdict (CI)  headless       GitHub Issues    │
 │ (plugin)       (plugin)        (plugin)   (plugin)            (CI + plugin)    (binary+plugin) (plugin, wave 2)│
 └──────┬──────────────┬─────────────┬────────────┬──────────────────┬──────────────┬─────────────┬──────────────┘
        │   each owns its domain crates; siblings reach it only through its contract crate,  │
        │   its events in the shared log, or its versioned `--json` CLI                       │
 ┌──────▼─────────────────────────────────────── kits ────────────────────────────────────────▼──────┐
 │ redkiln-guard (shell reader, denylist, redaction)   redkiln-dispatch (headless claude -p, containment) │
 │ redkiln-github (issues, sub-issues, deps, PR diffs, checks)   redkiln-craft (fingerprint, verdict,     │
 │ completion gate, ratchet)   [later: redkiln-catalog]                                                   │
 └──────────────────────────────────────────────┬─────────────────────────────────────────────────────┘
 ┌──────────────────────────────────────────────▼─────────────────────────────────────────────────────┐
 │ Redkiln Core: SPEC + conformance kit + redkiln-core (event log, envelope, ids, tags, store location,  │
 │ journals, approval record, git tree hash, surface and repo-class detection) + redkiln-calibrate +     │
 │ redkiln-capacity                                                                                      │
 └───────────────────────────────────────────────────────────────────────────────────────────────────────┘
   Redkiln (bundle plugin): name + dependencies only        Dev tools: skillcheck, xtask (not shipped)
```

Everything lives in one `redkiln` monorepo with one Cargo workspace. Each plugin still builds, versions, tags and installs on its own.

## 2. What the audit changed

### 2.1 Three facts the earlier write-ups had wrong

The local checkouts of two repositories were stale, so the write-ups described them as less built than they are.

| Write-up said | Actually, on 2026-10-13 | Evidence |
|---|---|---|
| redkiln-rs is "the least built and the most designed", no code, 112 ADRs, no commits since 2026-09-11 | `main` is a design snapshot. The work is on `build/phase-0-to-1`: 552 commits, 12 crates, about 101k lines of non-test Rust plus 137k of tests, 1,453 tests, 198 ADRs, CI green on Linux and Windows. Phase 1 is about 65% done; it has never run a lane against a real model. **Loop is the largest codebase in the suite.** | audit/redkiln-rs.md §1, §2 |
| assay is "spec v1.1 only, not implemented" | `origin/main` has 46 commits, six crates, 47.6k lines of source and 40.7k of tests, about 1,969 tests. Built: pure `decide`, rubric, tiering, the temper adapter, findings validation, the hardened `claude -p` runner, verifier and specialists. Not built: everything that touches GitHub, which waits on four owner setup tasks (#39 to #42) | audit/assay.md §1, §2.2 |
| anneal has stubs for `ship`, `watch`, `rollout`, `perf` | All 24 commands are built (PRs #98, #101). anneal is 63k lines of non-test Rust in one crate, with 12 skills and 14 hooks | audit/anneal.md §0, §2 |

The practical consequence: "Loop conforms to Core, not the other way round" is still right, but it is now expensive, because Loop's store is 10k lines with its own guarantees. And renaming assay is no longer free, only cheap while it is unreleased.

### 2.2 Where the evidence pushes back on a decision, and what this Blueprint does about it

| Decision | What the audits found | Blueprint's position |
|---|---|---|
| **One shared event file, `<git-common-dir>/redkiln/events.sqlite3`** | Four frictions. Loop asserts WAL and runs several processes, and keeps tier-1 mirror tables it drops and rebuilds from git (audit/redkiln-rs.md §10.1-3). Review's main runtime is CI jobs on separate machines with no shared disk (audit/assay.md §8.1). anneal's unfiltered reads fail on the first foreign event, its types are unprefixed, and both plugins refuse a newer `user_version`, so one plugin's upgrade disables the other (audit/anneal.md §8.1). temper writes from every Bash and Edit hook (audit/temper.md §8.2) | **Keep it, and say what it is.** The shared file is the append-only **event log**. Projections are disposable and may live in per-plugin files. CI and cloud sessions write **journals** that import into the log. Core owns the one schema version, lenient reads and namespaced types. Loop gives up WAL for the log, on condition that Loop's two-process contention test (R-SQLITE) passes against TRUNCATE. Details in §6 |
| **Capacity is a Core library** | The real need crosses machines: GitHub runners, the owner's laptop and an unattended build share one subscription. The assay AFK build paused at 90% of the weekly limit. assay already designed a lease protocol over git refs (ADR-19, ADR-23) (audit/assay.md §8.2) | **Core owns a protocol with two backends**: the local event log, and a git-refs ledger for anything that crosses machines |
| **Calibration is a Core library** | Supported, with a twist: anneal's Clef log is per user and pooled across repositories (`~/.anneal/decisions/`), and assay's promotion is per consumer repository (audit/anneal.md §8.2, audit/assay.md §8.3) | **Core's calibration library has two scopes**, user and repository, and a promotion is always a human edit to a config file |
| **Core is the only shared layer** | Four pieces of code exist twice already: the shell guard (temper `shell.mjs` and anneal `guard/`), the `claude -p` dispatcher (assay-runner and Loop's `rk-adapter-claude` plus `rk-containment`), the GitHub client (anneal `dag`/`pr_diff`/`watch`, Loop's tracker port, assay's planned forge) and the enforcement engine (temper's verdict, fingerprint and ratchet against anneal's `verdict.rs` and `baseline.rs`) | **Add a kits layer**, kept out of Core because it is behaviour, not format. Core stays small (§3) |
| **Redkiln Review is a plugin** | Most of assay is a CI deployment: workflow templates, a GitHub App, a ledger repo, and a binary pinned by digest in each consumer repo. A marketplace install cannot deliver that trust anchor (audit/assay.md §8.4) | **Review is a product with two deliverables**: the CI deployment (OCI binary by digest plus templates) and a thin plugin for in-session checks and status |
| **Redkiln Bench is one plugin from anneal** | anneal's single binary carries about 15k lines of skill lint and evals that apply to any plugin, and about 9k lines of PR, verdict and fan-out tooling that Review and Loop also need (audit/anneal.md §7, §8.3) | **Bench stays one plugin, but its binary slims**: the guard and GitHub code move to kits, skill QA becomes a dev tool, the verdict becomes Review's event, `rollout`/`perf`/`migrate` freeze until a Ship product exists |
| **Backlog is GitHub Issues** | Supported by every repo. Two caveats: client repositories and Cowork folders that are not GitHub repos cannot depend on Issues, and client items must not leave the machine (wi-7c21e4); Loop needs item content fixed at a hash on **every** entry, not only at start (F19: 7% drift) (audit/weigh-in.md §8, audit/redkiln-rs.md §10.4) | **Backlog is optional and gated by repo class.** Without it, Weigh-In keeps deferrals itself. Loop re-snapshots the issue each time it enters a run |
| **Weigh-In stays its own plugin** | weigh-in's own slice 3 planned to dissolve into Redkiln as a `decision` item type, and nothing records that this is dropped (audit/weigh-in.md §8) | **Record the supersession.** Weigh-In stays the single owner of decisions; Loop's human gates become Weigh-In decision requests (§5.3) |
| **temper's 2.0 port as planned** | The approved port plan ships through npm, which its own research found does not reach Cowork or org-synced installs; forbids an async runtime, which happenstance needs; allows only four crates; keeps 1.x state formats with no migration; and reserves `temper-*` crate names (audit/temper.md §8) | **Do the port inside the monorepo as Redkiln Rust 2.0**, with libexec packaging, Core's store, and a generic/Rust split from the first commit. That reopens port decisions D1, D2, D3 and D10 (§11) |

Nothing found contradicts: suite rather than monolith, Knowledge in the first wave, Docs deferred, or "craft" as the standards family.

## 3. Design rules

These are the rules the package boundaries follow. Most are already decided somewhere in the six repos; the source is in brackets.

1. **Share formats, embed code.** Every binary links Core statically. Nothing at runtime depends on another plugin being installed. A missing sibling turns a feature off; it never fails (suite-vs-redkiln §3).
2. **Rule of two.** Code enters Core or a kit only when two packages need it now and at least one already has it. Every kit in §4.2 meets this today.
3. **Core holds formats and the code that reads and writes them, never policy.** Gates, guards, prompts and UI live in plugins or kits.
4. **A domain has one owner.** Decisions belong to Weigh-In, atoms to Knowledge, work items to Backlog, findings and verdicts to Review, the Rust standard to Redkiln Rust, the process catalog to Loop (its format to Core). Others reach a domain through its contract crate, its events or its `--json` CLI, never its internals.
5. **Events are facts; projections are disposable.** The event log is append-only and shared. Anything derived from it can be deleted and rebuilt. Loop's deleted-database test (ADR-0109) applies to every plugin: each says which of its state is authoritative.
6. **Render, never parse.** Typed values are the source; markdown is a rendering (Loop ADR-0005, ADR-0189). Text receipts between plugins, like anneal parsing `✓ raised wi-…`, are replaced by typed contracts.
7. **Hooks fail open, guards fail closed, no hook waits on a sibling.** All matching hooks run in parallel and in no fixed order, so each hook is independent and idempotent (suite-structure §8; anneal, weigh-in B-16).
8. **A model's judgement does not drive control flow until it has been scored.** Every judgement call is recorded, labelled and calibrated; promotion is a human edit (Loop ADR-0079, assay QA-05, anneal Clef).
9. **Content-bound approvals.** An approval names the hash of what was approved and is void when the hash moves (temper fingerprint, anneal patch-id, Loop tree hash).
10. **Client data stays local.** A repository marked `client` never has its events, journals or items published, synced or projected off the machine (weigh-in wi-a93f02, wi-7c21e4). Core enforces this, not each plugin.

## 4. The packages

Five kinds of package, so that "package" does not have to mean "plugin":

| Kind | Installed by users? | Examples |
|---|---|---|
| **Spec** | No | Core SPEC, schemas and fixtures |
| **Library crate** (Core, kit, contract, domain) | No, linked into binaries | `redkiln-core`, `redkiln-guard`, `weighin-contract` |
| **Binary** | Inside a plugin's `libexec/`, or by digest in CI | `rk-rust`, `weighin`, `bench`, `redkiln`, `review` |
| **Plugin** | Yes, from the Redkiln marketplace | Redkiln Rust, Bench, Weigh-In, Knowledge, Review, Loop, Backlog, the bundle |
| **Deployment** | Yes, by a repository owner | Review's workflow templates, GitHub App and ledger repository |

### 4.1 Redkiln Core

**Job.** Make the suite's shared data mean the same thing in every binary.

| Part | Scope | Built from |
|---|---|---|
| **SPEC** (`spec/SPEC.md`, versioned on its own) | Store location rule; event envelope, namespacing and tags; schema-version rule; journal format; approval record; end-of-turn grammar (`Status:` line and fenced blocks); CLI result envelope and exit codes; `status --json` convention; hook conventions (metadata sidecar, budgets, fail policy); repo class; artifact rendering (Layout C); the process-catalog format (§5.6) | anneal's envelope, exit codes and hook metadata; weigh-in's marker grammar; temper's Status grammar; Loop's ADR-0005/0095/0131/0189 |
| **Conformance kit** (`spec/fixtures/`, a test runner) | Golden valid and invalid fixtures per schema; "foreign events are skipped" test; contention test (R-SQLITE); the all-plugins-installed e2e | assay's contract and fixture discipline (75 fixtures), happenstance-testkit |
| **`redkiln-core`** | Store location resolver; TRUNCATE opening with WAL conversion and stepwise migration; schema version; envelope, `<plugin>.<type>` names and tag validation; lenient typed reads; DCB guard helpers; content-hash ids; canonical digests; git helpers (common dir, tree hash per Loop ADR-0105 clause 4); journal export and idempotent import; surface detection (Claude Code, cloud, Cowork, Projects thread, Windows desktop); repo-class gate. A synchronous API over happenstance, with tokio's current-thread runtime hidden inside | anneal-ledger's typed opening; weigh-in's WAL conversion and 0.1.x/0.2.x fixtures; anneal `paths.rs`; weigh-in `id.rs`, `git.rs`; Loop `identity.rs`, `canon.rs`; weigh-in journal; weigh-in #126/#127 |
| **`redkiln-calibrate`** | Judgement record, outcome label, scores (Brier, reliability bins, Wilson and Newcombe intervals, MCC), promotion verdicts; user and repository scope | anneal `calibrate/` (2,145 lines) and thresholds; assay QA-05/QA-12 rules; Loop's facet provenance and stability gate; weigh-in's eval seed schema |
| **`redkiln-capacity`** | Lease protocol `{holder, run, reserved, expires_at}`, reserve and final cost records, rate-limit readings in integer ticks; local backend (event log) and git-refs backend (atomic refs, compare-and-set, leases with TTL) | Loop ADR-0110 headroom and throttle; assay ADR-19/23 ledger and semaphore design; `aeu.rs` |

**Out of scope.** Gates, guards, prompts, dispatch, GitHub, anything a user sees. Core has no binary.

### 4.2 Kits

Each kit is a library with its own tests, owned by the suite (not by one plugin), depending only on Core.

| Kit | Job | Consumers | Built from (duplicates today) |
|---|---|---|---|
| **`redkiln-guard`** | What does this shell command read, write, delete or run? Shell lexer (quotes, heredocs, `bash -c`, wrappers), write/delete target resolution, a denylist engine with per-plugin rule packs, protected-path globs, credential redaction | Redkiln Rust (pre-bash), Bench (guard), Loop (worker command check), Review and Loop (redact before text leaves the machine) | temper `shell.mjs` (1,090) and glob; anneal `guard/shell` and `denylist` (3.8k), `redact/` (1.5k), `glob.rs` |
| **`redkiln-dispatch`** | Run `claude -p` safely and account for it. Argv profiles, env denylist and `--agent` isolation, auth preflight, stream-json parser (init, `rate_limit_event`, usage, cost), TTL checks, process containment (Job Object, process group) | Loop (harness adapter), Review (reviewers), Bench (evals, through skillcheck) | assay-runner and `assay-forge/src/process.rs`; Loop `rk-adapter-claude` generic half and `rk-containment` (2.7k, already standalone); Loop F7/F8 findings |
| **`redkiln-github`** | GitHub as data: issues with types, sub-issues, dependencies and labels; issue snapshots with content hash; PR diffs, patch-id and ancestry; check runs; wait-for-CI with typed exits | Backlog, Bench (`dag`, merge-gate), Loop (tracker port, CI waits), Review (forge) | anneal `dag/tracker.rs`, `pr_diff.rs`, `watch`; Loop `Tracker` port (fake only); assay #53 (unbuilt) |
| **`redkiln-craft`** | The language-neutral enforcement engine: change fingerprint over per-language pathspecs, reviewer-verdict validation and storage, the completion-gate choreography (Stop, SubagentStop, WIP/BLOCKED honesty), baseline ratchet over changed lines, gate step runner with per-fingerprint cache | Redkiln Rust now; Redkiln TypeScript and Python later; Bench's `floor-guard` and verdict freshness | temper's agnostic ~3,500 lines (`verdict.mjs`, `gate.mjs` runner, `baseline.mjs`, fingerprint, completion gate); anneal `verdict.rs`, `baseline.rs` |
| `redkiln-catalog` (**later**) | Load, lint and select from the process catalog | Loop now; Bench if it ever runs catalog activities | `rk-kernel::{catalog, predicate, lattice, select}`. Extract only when a second consumer exists (§5.6) |

**Dev tools, never shipped:** `skillcheck` (skill lint and Tiers 1 to 3 evals for every plugin's CI, from anneal's ~15k lines) and `xtask` (libexec packaging, version agreement, changelog section, marketplace assembly, from the three copies in weigh-in, anneal and temper).

### 4.3 The products

Each entry gives the one-liner, scope in and out, what it owns, and where it comes from.

#### Redkiln Rust (from temper)

*"Rust that meets the standard, enforced as you write it."*

- **Is:** the rust-craft standard (182 rules, permanent IDs), the three subagents, `/setup`, `/implement`, `/review`, `/gate`, `/audit`, the hooks (skill-load gate, escape-hatch scan, manifest policy, completion gate), the Rust workspace templates, and a CLI for CI (`gate`, `scan`, `baseline --check`, `report`, `status --json`).
- **Owns:** the standard and its rule registry; Rust policy (scanner, lexer, manifest rules, cargo steps, cargo-message parsing); the Rust dimension set of the reviewer rubric; the checks assay currently re-implements (test inventory, manifest test switches, lints-table weakening, inheritance), offered as CLI subcommands so Review keeps only an adapter (assay's D-TEMPER-1).
- **Not:** the generic engine (in `redkiln-craft`), the shell reader (in `redkiln-guard`), PR verdicts (Review).
- **Crates:** `rust-craft-*` domain, app and CLI crates on top of `redkiln-craft`. Binary in `libexec/`.
- **From temper:** the 2.0 port happens here. Its parity discipline (byte-identical fingerprint, D16's divergence list, the 365 Node tests as the oracle) carries over. Vendoring (`.claude/temper/`) is retired: monorepo siblings use the workspace binary and outside repos pin a release.

#### Redkiln Bench (from anneal)

*"Work through a change with Claude: grill, spec, plan, build, verify."*

- **Is:** the in-session phases as skills (`grill`, `spec`, `plan`, `build`, `diagnose`, `verify`, `ship` trimmed to review depth and GO/NO-GO), the in-session fan-out (`/orchestrate` over a few tickets, `brief`, `round`), `/handoff`, `/correct`, the hooks (bootstrap, route, ruled-before, guard, merge-gate, preconditions, brief, fast-check, injection-scan, context-monitor, completeness, status, handoff, standing-orders), Clef and its seven judgement points, and the deterministic define-phase lints (`lint spec|plan|terms|brief|rules`, `plan approve|check`, `repro`, `floor-guard`, `task done`).
- **Owns:** phase state (`mode_set`), plan and grill approvals, briefs, repros, Clef's points and thresholds, the in-session claim gate.
- **Hands off:** the guard engine to `redkiln-guard`; skill lint and evals to `skillcheck`; `dag`, `pr_diff` and `watch` to `redkiln-github`; verdicts to Review (Bench's merge-gate reads `review.verdict` events instead of owning `verdict_set`); calibration scoring to Core; `.out-of-scope/` rulings to Knowledge as atoms; `orchestrate.ruled` (wi-1154f8) to Weigh-In.
- **Freezes:** `rollout`, `perf` and `migrate` (about 1.7k lines, no metrics source, no user). They wait for a Redkiln Ship product.
- **Tags:** each skill names the catalog activity it implements (`grill → problem-framing`), checked in CI against Loop's catalog.

#### Weigh-In (from weigh-in)

*"Agents' assumptions settled by you, at the end of the chat."*

- **Is:** the `/weigh-in` sitting, capture hooks (Stop, SubagentStop, PostToolUse, SessionStart), the board (Artifact), and the `weighin` CLI.
- **Owns:** the **decision domain**: every question, assumption, deferral and call, from any plugin, with door, authority, options, default and deadline; their lifecycle; and the sittings that settle them. One read API (`open --json`, `ci-check --json`) serves Bench, Loop and Review.
- **Contract crate `weighin-contract`:** the item and decision event types, so Bench's `orchestrate park`, Loop's human gates and Review's owner acks raise items by appending typed events, not by piping text to `weighin batch`.
- **Hands off:** the `.kb` atom renderer (`atom.rs`, 1,107 lines) to Knowledge, which Weigh-In then calls; the marker grammar shell to Core's end-of-turn spec (Weigh-In keeps its key set); `store.rs`, `env.rs` and `git.rs` (about 1,700 lines) to Core.
- **Supersedes:** slice 3 (moving into Redkiln as a `decision` item type). Record this as a decision.

#### Redkiln Knowledge (new)

*"Your repo remembers what was decided and why, and every session starts knowing it."*

- **Is:** the `.kb` format and its lifecycle (intake, promotion, supersession, index), retrieval (`/knowledge` query), injection into sessions under a context budget, and `validate`.
- **Owns:** atoms of kinds decision, constraint, finding, evidence, learning, term and ruling; the records conventions (Loop ADR-0085/0092: "What this forbids", errata, precedence); the **atom-set digest** per activity, which Loop needs in its memo key.
- **Contract crate `knowledge-contract`:** atom schema, writer, digest.
- **Built from:** weigh-in `atom.rs`; the `.kb` frontmatter of the old TypeScript Redkiln; anneal's `.out-of-scope/` rulings and ADR numbering; Loop's `.redkiln/kb/` notes and records conventions; kb-ingest's behaviour (to be re-implemented, not ported).
- **Producers:** Weigh-In (decisions), Bench (rulings, repro learnings, constitution), Review (finding dispositions), Redkiln Rust (approved lint exceptions). **Consumers:** every plugin's SessionStart; Bench's ruled-before hook; Loop's activity cards; Review's reviewer context.

#### Redkiln Review (from assay)

*"Whether a PR may merge, as one required check."*

- **Is, as a deployment:** the `review` binary published by digest to GHCR, the base-defined workflow templates, the `verdict` GitHub App, and the ledger repository. Pure `decide`, rubric, tiering, findings validation, blind specialists and verifier, sanitiser and egress scan, replay bundles.
- **Is, as a plugin:** `/review:check` (local, advisory), `/review:status`, and later a read-only view.
- **Owns:** the **finding and verdict domain**: the findings schema (`assay.findings/1`, renamed), the verifier, the rubric and `decide`, and the `review.verdict` event that Bench's merge-gate and Loop's review stage read.
- **Contract crate `review-contract`:** findings and verdict schemas, so temper's reviewer verdict, Bench's doubt pass and Loop's review gate produce one finding shape. Loop's "reviewer proposes, machine confirms" re-execution (ADR-0074) becomes a verifier mode here; it is exactly assay's deferred "executable witness".
- **Uses:** `redkiln-dispatch` for reviewers (replacing `assay-runner`), `redkiln-github` for the forge, `redkiln-capacity` with the git-refs backend for the cross-repo semaphore, `redkiln-calibrate` for blocker promotion, Redkiln Rust's CLI (pinned by digest, never in-tree, per assay T4) and Weigh-In's `ci-check`.
- **Keeps its own state model:** per-job JSONL and the ledger repo, not the local event log (ADR-07/09/15). In local mode it writes `review.*` events to the log; CI runs export a journal that a local import can fold in.

#### Redkiln Loop (from redkiln-rs)

*"Claude works the backlog while you're away."*

- **Is:** the `redkiln` binary (`run`, `answer`, `status`, `explain`, `cost`, `doctor`, `init`, `lint`, `catalog search`), the kernel (selection, lanes, memo, replay, three gates, reconcile, budgets), the process catalog (12 packs, 12 stages, 102 activities, 8 cards), run worktrees, git trailers as tier-1 authority, and the toolchain adapters. Plus a small hand-written plugin (`/loop:run`, `/loop:answer`, `/loop:status`) that wraps the binary through the suite launcher.
- **Owns:** unattended execution; the process catalog's content; the trailer grammar (`Story:`, `Slice-Verdict:`, `Gate:`); its projection store (the tier-1 mirror and memo tables).
- **Hands off:** item records (`backlog.rs`, `item_write.rs`) to Backlog, whose GitHub client replaces the `Tracker` port; `.redkiln/kb/` to Knowledge; `DocsSite` to the deferred Docs; its tier-2 events to the Core log; `rk-containment` and the generic half of `rk-adapter-claude` to `redkiln-dispatch`; headroom and throttle to `redkiln-capacity`; facet provenance and the stability gate to `redkiln-calibrate`; `review/` (1.2k lines) to Review's verifier; ADR-0083/0084/0100 lint and dependency policy to Redkiln Rust.
- **Human gates become Weigh-In decision requests** (a gate is a question with options, a deadline and a terminal default; ADR-0017's `(token, tree_hash, deadline)` maps onto Core's approval record). `redkiln answer` stays as the fallback channel, which ADR-0009 allows.
- **Drops:** `rk-plugin-gen` and the `rk-` prefix on generated commands (DECISIONS-2026-09-11 #8); `.bklg` and in-repo items.
- **Keeps:** its records (198 ADRs, the AC and SM registers, the runbook) as they are, under `docs/loop/`. The ADR README forbids renumbering.

#### Redkiln Backlog (new, second wave)

*"Your agents read and update your issues properly."*

- **Is:** a skill that teaches the suite's issue conventions, commands to cut, link and snapshot issues, and the label taxonomy.
- **Owns:** the **work-item domain** mapped onto GitHub: item types to issue types, hierarchy to sub-issues, blocking to issue dependencies, state and facets to labels, including the orchestrator-only namespace Loop reserves (AC-507). Issue snapshots (number, `updated_at`, content hash) as events, re-taken on every Loop entry.
- **Contract crate `backlog-contract`:** the item schema and its mapping, over `redkiln-github`.
- **Built from:** anneal's `dag` GitHub tracker (sub-issues, `Blocked by #n`); redkiln-rs `docs/BACKLOG.md` (PR #78); the conventions assay, temper and redkiln-rs already run on.
- **Opt-in by repo class.** Client repositories and non-GitHub folders run without it; Weigh-In keeps deferrals itself and Bench's `dag` falls back to local tickets.

#### Redkiln (bundle plugin)

A manifest with a name and a dependency list. `install redkiln@redkiln` installs the set. It carries no code. If a suite-wide status view is built later (temper's lens #107 generalised over every plugin's `status --json`), it ships here as a mod.

#### Later, named so the gaps don't fill piecemeal

- **Redkiln TypeScript, Python:** a new policy layer on `redkiln-craft` per language.
- **Redkiln Ship:** release, rollout, watch, incidents. Starts from Bench's frozen `rollout`, `perf`, `migrate` and Loop's release and operate stages, when there is a user.
- **Redkiln Docs:** deferred as decided. The activity is a catalog entry, the drift check a Review gate, the site a projection.

## 5. How they compose

### 5.1 Who writes and reads what

Event types are namespaced `<plugin>.<type>`. Readers skip types they do not know.

| Data | Owner (writes) | Readers |
|---|---|---|
| `rust.*`: gate runs, verdicts, scan denials, telemetry | Redkiln Rust | Rust (status), Bench (build/verify), Loop (verify stage), suite status |
| `bench.*`: mode, grill and plan approvals, task done, repro, briefs, rounds, Clef decisions | Bench | Loop (approved plan is an answered gate), Weigh-In (context packs) |
| `weighin.*`: items and their lifecycle | Weigh-In, plus any plugin through `weighin-contract` | Bench (ruled-before), Loop (blocked gates), Review (`ci-check`) |
| Atoms in `.kb/` | Knowledge, called by Weigh-In, Bench, Review, Rust | Everyone's SessionStart; Loop (with digest) |
| `review.*`: verdicts, findings dispositions | Review (local mode); CI journals | Bench (merge-gate), Loop (review stage), Knowledge |
| `loop.*`: tier-2 run events | Loop | Loop, suite status |
| `backlog.*`: issue snapshots | Backlog, or Loop through `backlog-contract` | Loop, Bench (`dag`) |
| `core.*`: capacity leases and cost, calibration records, journal imports | Core libraries, on behalf of any plugin | Loop and Review (capacity), every judgement point (calibration) |

### 5.2 Approvals

One record in Core: `{subject_kind, subject_hash, hash_method, scope, decider, verdict, at, expires_at?}`. `hash_method` names how the hash was taken: temper's 16-hex change fingerprint, anneal's patch-id, Loop's tree hash, a file's sha256. Core implements each method once. A Bench plan approval answers Loop's planning gate if Loop recomputes the same hash before acting (redkiln-vs-flow §3; no cross-plugin DCB guard is needed).

### 5.3 Decision requests

Any plugin that needs the human raises a Weigh-In item: Bench parking a one-way call, Loop hitting a human gate, Review needing an owner ack, Rust asking to approve a lint exception. Weigh-In settles them in a sitting or on the board; the answer is a `weighin.item_decided` event the raiser reads; the record becomes an atom through Knowledge. A plugin running with no human (Loop, Review) follows wi-4d8b6a: two-way judgement items may take their default, one-way items block.

### 5.4 Findings and verdicts

One findings schema, owned by Review. temper's reviewer `findings[]`, Bench's doubt pass and Loop's review gate all emit it; Review's verifier checks it, optionally by re-executing a typed repro. A verdict is a `review.verdict` event bound to a subject hash. Bench's merge-gate and Loop's review stage require a fresh one.

### 5.5 One session, several plugins

- **End of turn:** one grammar in Core. A `Status:` line from one closed set (`DONE`, `DONE_WITH_CONCERNS`, `WIP`, `NEEDS_CONTEXT`, `BLOCKED`), which every plugin accepts, so Bench stops recording Rust's `WIP` as a violation; then optional `Gate:`, `Review:`, `Changed:`, `Decisions needed:`, `Declined:`, `Not verified:` lines; then fenced blocks per plugin (`weigh-in` today).
- **Stop hooks:** Rust's gate, Bench's completeness check and Weigh-In's capture all run in parallel. Each caps its own blocks, and none blocks for something another plugin owns.
- **SessionStart:** a suite context budget split by convention; each plugin injects less when it sees siblings in the log.
- **Guards:** Rust and Bench both guard Bash, with different rule packs over the same `redkiln-guard` engine. They check different predicates, so running twice is correct, and they agree on what a command does.
- **Conformance:** the all-plugins-installed e2e is the top test in the conformance kit.

### 5.6 The process catalog

The catalog (packs, stages, activities, rigor, the predicate grammar of ADR-0103) is Loop's content and a suite format. Its **format** goes into Core's SPEC now, because Bench's skills already name catalog activities and the two lists must be checked against each other. Its **engine** stays in Loop's kernel. It moves to a `redkiln-catalog` kit only if Bench starts running catalog activities in-session.

## 6. Data and state

| Store | Where | Mode | Authority |
|---|---|---|---|
| **Event log** | `<git-common-dir>/redkiln/events.sqlite3`; overrides `--store`, `$REDKILN_STORE`, `.redkiln/suite.toml`; `<folder>/.redkiln/events.sqlite3` outside git | happenstance, TRUNCATE, opened only by `redkiln-core`; one schema version owned by Core, additive only | Authoritative for in-session plugins (Weigh-In, Bench, Rust); tier 2 for Loop |
| **Projections** | In the log, or `<git-common-dir>/redkiln/<plugin>.projections.sqlite3` | Plugin's choice; disposable; rebuilt from the log (and from git for Loop) | Never authoritative. Loop's tier-1 mirror and memo live here, so `doctor --rebuild` keeps working |
| **Git** | Trailers (`Story:`, `Gate:`, `Weigh-In:`), committed atoms, committed config | — | Tier 1 for Loop; the durable record for decisions |
| **Journals** | `.redkiln/journal/<session>.jsonl` (committed, own repos) or a telemetry branch (where commits are unwelcome); CI artifacts for Review | JSONL of envelopes, one format | Export and idempotent import. Answers "cloud containers lose `.git/`" once, replacing weigh-in's committed journal and temper's telemetry branch as two answers |
| **User scope** | `~/.redkiln/` | Core-managed files | Calibration logs pooled across repositories (Clef); per-user counters |
| **Remote** | A private ledger repository, git refs with compare-and-set | `redkiln-capacity`'s git backend; Review's per-PR state | Cross-machine leases and Review's authoritative state (ADR-19, ADR-23) |
| **Repo config** | `.redkiln/suite.toml` (repo class, store override, surfaces) and `.redkiln/<plugin>.*` | TOML | Committed and reviewed |

Rules that make the shared log safe:

- **Lenient reads.** A reader decodes only its own types and skips the rest. This is a conformance test, and it is the change anneal most needs (`store.rs:474-479`).
- **One version stamp.** Only Core writes `user_version`. A plugin built on an older Core refuses only a newer **major** schema; additive changes never lock a sibling out.
- **Short writes, never waiting on the hot path.** Hook writes take a bounded busy timeout; best-effort events (telemetry) are dropped and counted when the lock is busy; guards decide first and record second.
- **Migration imports, never deletes** (Cowork refuses deletes). Old ledgers at `<common>/weighin/` and `<common>/anneal/` and committed journals are imported once.
- **Repo class.** A `client` repository never exports journals, pushes a telemetry branch, publishes a board or projects items to GitHub.
- **WAL.** Loop gives up WAL for the shared log, conditional on R-SQLITE (two processes, parallel worktrees) passing under TRUNCATE in the conformance kit. If it fails, Loop keeps its tier-2 events in its own WAL file and exports a journal into the log. Either way the suite has one event format.

## 7. Distribution

- **Plugins** ship as `plugins/<name>/` with prebuilt binaries in `libexec/<target>/` behind one shared launcher (sh and `.cmd`, newest build wins, no `bin/`). This is what weigh-in and anneal do today and what temper's own research says reaches Cowork and org-synced installs.
- **One marketplace, `redkiln`**, assembled at release from per-plugin artifacts. Claude Code users add the monorepo's marketplace branch; claude.ai org sync reads a release-only repository whose default branch is that assembled marketplace (weigh-in B-19's pattern, generalised to all plugins, one deploy key). **Unverified:** whether org sync accepts several plugins in one repository and honours plugin `dependencies`. This needs a spike before the migration commits to it.
- **Review's deployment** ships outside the marketplace: the binary by digest on GHCR, workflow templates and the App. A consumer repo pins Review and Redkiln Rust by separate digests, so the monorepo must keep them independently releasable.
- **Versions and tags are per package**: `rust-v2.0.0`, `bench-v0.1.0`, `weigh-in-v1.0.0`, and so on, each with its own CHANGELOG. The Core SPEC has its own version; each plugin declares the SPEC version it conforms to. The bundle bumps when a member does.
- **Plugin IDs.** Display names as decided. Recommended install IDs: `rk-rust`, `rk-bench`, `rk-knowledge`, `rk-review`, `rk-loop`, `rk-backlog`, with Weigh-In keeping its bare skill name `/weigh-in` (Cowork lists skills by their own name, D-0004).

## 8. The monorepo

```
redkiln/
├─ AGENTS.md  .claude/CLAUDE.md              suite charter: crate map, layer rules, decisions log
├─ Cargo.toml  rust-toolchain.toml (1.98.1)  one workspace; MSRV equal to the pin everywhere
├─ deny.toml  clippy.toml  rustfmt.toml  .gitattributes (LF, Loop's -text fixtures)
├─ spec/                                     Core SPEC, schemas, conformance fixtures
├─ crates/
│  ├─ core/       redkiln-core  redkiln-calibrate  redkiln-capacity
│  ├─ kits/       redkiln-guard  redkiln-dispatch  redkiln-github  redkiln-craft
│  ├─ rust/       rust-craft-domain  rust-craft-app  rust-craft-cli
│  ├─ bench/      bench-core  bench (binary)
│  ├─ weigh-in/   weighin-contract  weighin-core  weighin (binary)
│  ├─ knowledge/  knowledge-contract  knowledge (binary)
│  ├─ review/     review-contract  review-domain  review-app  review-forge  review-gates  review (binary)
│  ├─ loop/       rk-types  rk-ports  rk-kernel  rk-store  rk-git  rk-adapter-*  rk-cli  redkiln-migrate
│  └─ backlog/    backlog-contract  backlog (binary)
├─ plugins/       rust/ bench/ weigh-in/ knowledge/ review/ loop/ backlog/ redkiln/ (bundle)
│                 each: .claude-plugin/plugin.json, skills/, agents/, hooks/, libexec/ launcher
├─ deploy/review/ workflow templates, App manifest, fetch-by-digest action
├─ tools/         xtask, skillcheck
├─ templates/     Rust workspace templates and the example workspace (excluded from the workspace)
├─ evals/<plugin>/
├─ docs/          blueprint, suite decisions; docs/<plugin>/ (Loop's references/ and runbook/ under docs/loop/)
└─ .kb/           one knowledge base; wi- ids are already unique across repos
```

**Dependency rules, checked by a structural test** (the pattern Loop and assay already use): Core crates depend on nothing internal. Kits depend on Core. Contract crates depend on Core. A plugin's crates depend on Core, kits and other plugins' contract crates, never on another plugin's internals. `anyhow` only in binaries. Every binary builds alone (`cargo check -p`). Loop keeps its stricter rules (kernel without tokio, git through one wrapper, no egress in kernel tests).

**CI** is path-filtered per package, with one shared job set: fmt, the Redkiln Rust gate on the whole workspace (dogfood, replacing four vendored copies), tests on Linux and Windows (macOS on release), `cargo deny`, the conformance kit, `skillcheck` over every plugin, `claude plugin validate --strict` per plugin, and the version and changelog checks per package.

## 9. Where today's code lands

| Today | Lands in |
|---|---|
| temper `skills/`, `agents/`, `templates/`, rule registry and Python checks | Redkiln Rust plugin and `rust-craft-*` crates (as the 2.0 port) |
| temper `shell.mjs`, glob; agnostic verdict, gate, baseline, fingerprint | `redkiln-guard`; `redkiln-craft` |
| temper telemetry | `rust.*` events; journals |
| weigh-in `weighin-core` (event, projection, card, id, marker, scan, board) | `weighin-core`, `weighin-contract`; marker grammar shell to Core SPEC |
| weigh-in `atom.rs` | `knowledge-contract` |
| weigh-in `store.rs`, `env.rs`, `git.rs`, journal | `redkiln-core` |
| weigh-in-plugin | Replaced by the suite's release-only marketplace repository |
| anneal `anneal-ledger` | `redkiln-core` (typed opening, envelope, tags) plus `bench.*` event types |
| anneal hooks, skills, define lints, Clef, orchestrate | Bench |
| anneal `guard/`, `redact/`, `glob.rs` | `redkiln-guard` |
| anneal `dag`, `pr_diff`, `watch`, `process.rs` | `redkiln-github` (process runner into `redkiln-dispatch`) |
| anneal `verdict`, merge-gate | `review.verdict` event; merge-gate stays a Bench hook |
| anneal `calibrate/`, thresholds | `redkiln-calibrate` (Clef client stays in Bench) |
| anneal `lint skills`, `eval` | `tools/skillcheck` |
| anneal `rollout`, `perf`, `migrate` | Frozen in Bench, earmarked for Ship |
| assay `assay-domain`, `assay-app`, `assay-forge`, `assay-gates`, `assay-cli` | `review-*` crates |
| assay `assay-runner`, `forge::process` | `redkiln-dispatch` |
| assay `assay-gates/src/temper` re-implementations | Redkiln Rust subcommands; Review keeps the adapter |
| assay ledger and semaphore design | `redkiln-capacity` git backend |
| assay schemas and fixtures discipline | Core SPEC practice and conformance kit |
| redkiln-rs `rk-*` crates | Loop |
| redkiln-rs `rk-containment`, generic half of `rk-adapter-claude` | `redkiln-dispatch` |
| redkiln-rs headroom/throttle; facet provenance | `redkiln-capacity`; `redkiln-calibrate` |
| redkiln-rs store connection and identity rules | `redkiln-core` (with TRUNCATE) |
| redkiln-rs `backlog.rs`, `item_write.rs`, `Tracker` | Backlog |
| redkiln-rs `.redkiln/kb/`, records conventions | Knowledge |
| redkiln-rs `review/` | Review's verifier |
| redkiln-rs catalog format and predicate grammar | Core SPEC (format); engine stays in Loop |
| Four vendored `.claude/temper/` copies | Deleted; the workspace runs Redkiln Rust directly |

## 10. Constraints the migration plan has to respect

These come straight out of the audits. They shape the order of work, not the end state.

- **Two pilots are frozen.** temper's hooks and gate until the pilot report around 2026-10-18 and v1 (1.4.0) after it (wi-b63bd6); Weigh-In's capture until 1.0.0, on or after 2026-10-20 (wi-7c3aa0, B-23). Moving their stores or hooks earlier invalidates the measurements.
- **redkiln-rs's work is not on `main`.** `build/phase-0-to-1` (about 490 commits ahead) must land first (#24), and its corpus has no off-machine copy (`CORPUS_PIN` at `D:/repos/redkiln-corpus`, #32). Back it up before anything moves.
- **assay's owner setup has not happened yet** (App, ledger repo, approver, agent identity, #39 to #42). Renaming before those exist is free; after, it is not.
- **anneal's v0.1.0 marketplace publication (#44)** would put a public `anneal` install ID in place just before the rename. Hold it, or publish only into the Redkiln marketplace.
- **happenstance 0.3.2 to 0.4.1** is a migration with fixtures from 0.1.x and 0.2.2 ledgers. Do it once, in Core.
- **Path assumptions** break on the move: Loop's CI greps `crates/` 40 times and its runbook holds 241 `D:/repos` paths; assay's tests assert exactly six crates and treat all of `crates/**` as its trust path; temper, weigh-in and anneal assume the repository root is the plugin and marketplace root.
- **Owner-only steps:** rulesets and required-check names, the `release` environment, crates.io trusted publishing (bound to temper's repository), deploy keys, org-sync settings, GHCR. The migration plan lists them for Ryan rather than assuming them.
- **Install IDs in the wild:** `temper@temper`, `weigh@weigh-in`, `anneal@anneal`, plus adopters' `extraKnownMarketplaces` and cloud setup scripts. Old repositories keep a redirecting marketplace for a transition period.
- **Decision-log merges:** assay's spec ADR-23 and the ADR-23 proposed for its mod (#125) already clash, and each repository numbers its own records (Loop's ADR-0001 to 0198, anneal's `docs/adr/`, weigh-in's D- and B- series). Keep each series under its package's `docs/` with its prefix rather than renumbering. Weigh-In `wi-` ids are content-hashed and do not collide.

## 11. Calls for Ryan

Each has a recommendation. The ones marked one-way are hard to undo once built on.

1. **Store shape** (§6): one shared event log as decided, plus disposable per-plugin projection files, journals for CI and cloud, and Loop off WAL if R-SQLITE passes. *Recommended: yes.* Flip: R-SQLITE fails, or Loop's tier-2 volume proves large.
2. **Capacity as a protocol with a git-refs backend** (assay's design) rather than a library over the local file. *Recommended: yes.* One-way once Review's ledger repository exists.
3. **A kits layer** (`guard`, `dispatch`, `github`, `craft`) and **contract crates**, beside a Core that stays formats-only. *Recommended: yes.*
4. **Review is a CI deployment plus a thin plugin**, delivered by digest outside the marketplace. *Recommended: yes.*
5. **temper's 2.0 port becomes Redkiln Rust 2.0, built in the monorepo**: libexec instead of npm (reopens D1, D6, D13), a generic/Rust crate split on `redkiln-craft` (reopens D3), Core's store and a hidden tokio runtime (reopens D10 and the state-compatibility invariant), and new crate names instead of the reserved `temper-*` (reopens D2). *Recommended: yes.* One-way for the published crate names.
6. **Bench slims**: guard and GitHub code to kits, skill QA to a dev tool, verdicts to Review, `rollout`/`perf`/`migrate` frozen. *Recommended: yes.*
7. **Loop's human gates become Weigh-In decision requests, and Weigh-In's slice 3 is formally superseded.** *Recommended: yes.*
8. **Process catalog format into Core's SPEC now, engine stays in Loop.** *Recommended: yes.*
9. **Plugin install IDs `rk-*`**, with `/weigh-in` kept as the bare skill name. *Recommended: yes.* One-way once colleagues install.
10. **Backlog is opt-in by repo class**, so client repositories and non-GitHub folders never depend on it. *Recommended: yes.*
11. **A spike on claude.ai org sync** with several plugins and plugin dependencies in one repository, before the migration plan commits to one release repository. *Recommended: yes, first.*
