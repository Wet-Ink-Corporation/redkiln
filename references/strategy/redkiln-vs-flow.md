# Redkiln and Redkiln Flow: where they overlap, and how to compose them

*2026-10-12. Read from `redkiln-rs` at `353ea01` and `anneal` at `17584b8`. A companion to [suite-vs-redkiln.md](suite-vs-redkiln.md), which this does not change.*

## The short answer

They overlap almost completely on **what** a piece of work goes through (frame, specify, plan, build, reproduce, verify, review, ship) and hardly at all on **how** it is run. anneal (Redkiln Flow) is a referee inside a live Claude Code session with a human present. redkiln-rs (Redkiln) is a conductor outside any session, spawning one `claude -p` per step with nobody watching. Each one's design explicitly rejects the other's runtime shape, and both are right about their own case.

So do not make one run the other. Compose them at three seams:

1. **One contract for the artifacts and approvals** (spec, plan, repro, findings, decision, and "approved at this hash"), written by either runtime and readable by both.
2. **Flow's CLI becomes Redkiln's check library.** The deterministic checks anneal already has (EARS spec lint, plan lint, repro and leftovers, floor-guard, claim gate) are exactly the `kind = "tool"` and `kind = "check"` steps Redkiln's activities need. Redkiln calls them instead of rewriting them.
3. **A handoff, not a merge.** The human-heavy front of the funnel (grill, spec, plan approval) runs in Flow. The ticket-by-ticket middle (build, verify, review per ticket) runs in Redkiln, unattended. The result comes back to the human through a gate.

The earlier working idea, "anneal owns what the phases are and Redkiln runs them", is **half right**. Redkiln cannot run anneal's phases as they stand (they are prose skills driven by hooks and a human's verbatim answers), and Redkiln's process model (packs, rigor, typed activities) is the more formal of the two. The better split is: **Redkiln's catalog owns the process definition; Flow owns the in-session experience of it and the deterministic checks both use.**

## What each one actually is

| | Redkiln Flow (anneal) | Redkiln (redkiln-rs) |
|---|---|---|
| Shape | Claude Code plugin: 6 skills, 8 hooks, a Rust CLI | One Rust binary that owns the loop; spawns `claude -p` per activity |
| Who drives | The model, with a human in the chat; hooks referee | The kernel, deterministically; the model only fills typed steps |
| Code today | ~37.5k LOC, 1,167 tests, v0.0.1 released today | **No product code.** Design, 112 ADRs, a runbook whose build stages are all "not started" (`runbook/PROGRESS.md`), and an ~11.8k-line spike runner that "is not a product" (`spikes/runner/src/main.rs:1-8`) |
| Process model | 7 phases as prose skills; `route.json` maps prompts to entry skills | 12 process packs × 12 stages × ~95 activities as TOML graphs, selected by a pure function of four digests (design §4.4) |
| Artifacts | Markdown from templates, validated by lint (`docs/spec.md`, `docs/plan.md`) | Typed values only; "markdown is a rendering and never a source" (ADR-0005/0006/0075) |
| Human gates | `grill confirm --quote`, `plan approve` by sha256 | `(token, tree_hash, deadline)` mark plus a `Gate:` trailer, answered by `redkiln answer` (ADR-0017, ADR-0009) |
| Store | happenstance 0.3.2 SQLite, TRUNCATE journal, `<git-common-dir>/anneal/` (`docs/ledger.md`) | Git trailers first; SQLite (WAL, one writer) second, rebuildable (design §3.2, ADR-0109) |
| Judgement calls | Clef D1-D7: an external model, shadow until calibrated (Brier ≤ 0.15 over ≥ 50 labels, `docs/calibrate.md`) | LLM-derived facets measured unreliable and barred from keying requirements (ADR-0079; P1's kill fired 4 of 4) |
| Knows about the other | Only via weigh-in, temper, "the review plugin" (issue #1 Boundaries) | Zero mentions of anneal, weigh-in, temper or assay anywhere in the repo |

## Where they overlap

### 1. The process itself (the real competition)

The same activities appear in both, under different names:

| Work | Flow | Redkiln catalog (`spikes/p9/catalog/`) |
|---|---|---|
| Question a vague request | `grill` (design tree, `is_decision`) | `problem-framing`, `assumption-mapping` |
| Requirements | `spec` (EARS, constitution check) | `structured-nl-requirements` (EARS checklist, adversary step) |
| Slice into tasks | `plan`, `dag frontier` | `vertical-slicing`, `walking-skeleton` |
| Build test-first | `build` | `tdd-red-green-refactor` |
| Reproduce, then fix | `diagnose` (`repro record`) | `minimal-repro`, `root-cause` |
| Prove done | `verify` (fresh run, doubt pass) | `confirmation-test`, `adversarial-review` |
| Ship and watch | `ship`, `watch`, `rollout`, `perf` (all stubs, `cli.rs:86-113`) | release and operate stages (`canary-release`, `rollback`, `smoke-test`, ...) |

If both keep their own definitions, the suite will have two answers to "what does a good spec look like" within a month. That is the drift to prevent.

### 2. Orchestration

Flow's unbuilt `/orchestrate` (#37) and `dag frontier` (tickets that fit one fresh context, token-estimated, fanned out) is a small conductor. Redkiln is the big one: lanes, budgets, retry classes, memo, resume, worktrees. Building both is the clearest waste in the portfolio.

### 3. Human approval records

Both bind an approval to content: Flow's `plan_approved` carries the plan's sha256 and `plan check` exits 4 once it changes; Redkiln's gate is void once `tree_hash` moves. Same idea, two encodings, and neither can read the other's.

### 4. Review

Three designs: Flow's `verify` doubt pass (reviewers see the diff and the contract, never the claim; file:line or rejected; three cycles), Redkiln's review gate ("reviewer proposes, machine confirms": a finding blocks only after a typed repro re-executes and flips, ADR-0074), and assay's PR verdict. Flow already defers PR review to "the review plugin". Redkiln does not.

### 5. Storage and events

Both keep an event log in SQLite under the git common dir, with incompatible choices: happenstance with TRUNCATE (required because Cowork folders refuse deletes, weigh-in B-01) against rusqlite with WAL. Redkiln's log is tier 2 by its own test ("if the database were deleted, would `redkiln run` take a different next action?"), so this conflict is cheap to resolve now and expensive later.

### 6. The plugin surface and the `rk-` prefix

Redkiln plans a generated plugin in Phase 2 (`rk-plugin-gen`, design §8.8, PROVISIONAL), with **an `rk-` prefix on every emitted command, agent and skill**. That collides head-on with the suite's chosen plugin IDs (`rk-rust`, `rk-flow`, ...) and would generate in-session phase skills that duplicate Flow's.

## Where they genuinely disagree

**Hooks enforcing order.** Redkiln's research took one lesson as foundational (from shipmates, which built a hook-enforced FSM and deleted 6,928 lines of it): "a hook can enforce a predicate on its own input, never a position in a sequence" (`references/2026-09-01-shipmates-lessons-for-redkiln-rs.md`, L1.1-L1.2). Flow's preconditions hook does enforce a sequence: no source edits until the spec lints, the tree is confirmed and the plan is approved.

Flow mostly sidesteps the failure shipmates hit. Shipmates tried to infer "which stage are we in" from slash-command scope across seven harnesses. Flow makes the stage an explicit event (`anneal mode build` writes `mode_set` keyed to the worktree, `docs/preconditions.md:17-34`), supports one harness, and checks a predicate over the tool input plus that event. So the critique lands only partly: Flow is still a referee, not a conductor, and the model still chooses what to do next. That is the correct design **when a human is driving**, and the wrong one for an unattended run. It is the clearest evidence that the two runtimes should stay separate.

**Cost.** Redkiln's E3 measured the same review at **1.72× the cost in-session** ($97.39 against $56.47, identical 23/23 detection; `spikes/FINDINGS.md:1262-1267`). Unattended, multi-ticket work belongs headless. Interactive work, where the human's attention is the scarce resource, does not.

**Prose versus typed values.** Flow lints markdown and extracts structure from it (`anneal lint plan` emits `data.plan`). Redkiln forbids parsing structure out of prose. They are closer than they look: Flow already has JSON schemas for the plan and the design tree (`plugin/playbooks/schemas/`). Make those schemas the source on both sides, and the markdown a rendering of them.

## Where each one is stronger

**Take from Flow:**
- Working, tested deterministic checks: EARS spec lint, constitution gates, plan lint and approval by hash, repro-red, `[DEBUG-` leftovers, floor-guard ratchets, the Stop-hook claim gate, and the shell-lexing guard denylist (which is exactly the "stateless predicate" hook Redkiln's own lesson allows).
- **Clef's calibration discipline.** Redkiln's hardest open problem is ADR-0079: no LLM-sourced facet (size, criticality, novelty) may key a requirement, because none cleared the bar. Flow already has the mechanism for earning that trust: shadow mode, outcome labels, Brier and reliability bins, and promotion only by a human edit. D3 already scores `size`. This is the most valuable thing Flow can give Redkiln.
- The grill. Redkiln expects its triage facets to become `source = "human"` at a triage gate (`runbook/phase-0/s4-blocking-gate.md`). Flow's grill is a working, interactive version of that gate.

**Take from Redkiln:**
- The process model: rigor as an ordinal enum, packs that select activities by predicate, the "technology packs may bind but never restructure" rule (ADR-0014, ADR-0029), and a curated catalog with lints already prototyped.
- Unattended execution: budgets per counter, retry per error class, `RateLimited` writes a mark and exits rather than sleeping, resume by re-running one idempotent `run`, worktree isolation, and "the orchestrator never writes the user's checkout".
- Review that re-executes findings before they block (ADR-0074). This belongs in Redkiln Review (assay) so that all three review paths share it.

## How to compose them

### Layering

```
            ┌──────────── process definition ────────────┐
            │ Redkiln catalog: packs, stages, activities │  one source of truth
            │ + shared artifact schemas (Spec, Plan, ...)│  (suite contract)
            └───────────────┬───────────────┬────────────┘
     in session, human      │               │      headless, unattended
   ┌────────────────────────▼──┐        ┌───▼───────────────────────────┐
   │ Redkiln Flow (plugin)     │handoff │ Redkiln (conductor)           │
   │ skills + hooks referee    ├───────►│ kernel runs activities per    │
   │ grill, spec, plan approve │◄───────┤ ticket; gates come back to    │
   │ ad-hoc build/diagnose     │ gate   │ the human                     │
   └──────────┬────────────────┘        └───┬───────────────────────────┘
              │       both call the same deterministic checks
              └──────────►  anneal CLI  ◄───┘   (lint spec/plan, repro, floor-guard, claim)
                     temper gate · Redkiln Review verdict · Weigh-In decisions
```

### The handoff in practice

1. In a Flow session, the human grills, specs and approves a plan. Flow writes `grill_confirmed` and `plan_approved` with hashes, and `dag` cuts the tickets.
2. `redkiln run --lane story --item <plan>` reads the approved plan as a typed `Plan` artifact (a Redkiln human gate that is already answered), and works the ticket frontier headless. Each activity's check steps call `anneal lint`, `anneal repro record`, `anneal floor-guard` and `temper gate`.
3. When an activity escalates, Redkiln writes a gate mark and exits. Flow's bootstrap hook shows open gates at the next session start, and the human answers there or with `redkiln answer`. Questions that are decisions go through Weigh-In; Clef's D6 already reads those atoms back.
4. Merge authority stays outside both: Redkiln Review and CI (Redkiln's own lesson L1.3).

Either half works alone. Flow without Redkiln is today's anneal. Redkiln without Flow takes its gates from `redkiln answer` and runs its checks through its toolchain adapter, which shells out to the anneal binary if present.

### Concrete changes

**In Flow (cheap, now):**
- Narrow #37 `/orchestrate` to an in-session fan-out over a few tickets, and state that unattended multi-ticket runs are Redkiln's.
- Drop the `ship`, `watch`, `rollout` and `perf` stubs from the v0.1 plan. Release and operate are Redkiln's stages, and merge is Redkiln Review's.
- Tag each skill with the catalog activity it implements (for example `grill → problem-framing`), so the two lists can be checked against each other in CI.
- Promote `plan.schema.json` and `design-tree.schema.json` into the suite contract, and give `lint spec` a typed `Spec` output beside it.

**In Redkiln (design changes, before Phase 1 code):**
- An ADR that the suite's plugins exist: the `Toolchain` and tool-step verbs call the anneal CLI and temper's gate rather than re-implementing spec lint, repro and ratchets.
- An ADR that a Flow `plan_approved` event (hash-bound) can answer a Redkiln planning gate, so a human approval in a session counts in the kernel.
- Adopt happenstance and the shared store-location rule for the tier-2 log (or record why not). TRUNCATE versus WAL is the one real technical choice here; Cowork support forces TRUNCATE for anything that may live in a connected folder.
- Rescope `rk-plugin-gen` (Phase 2) to the conductor's own commands under one `redkiln` plugin ID (`/redkiln:run`, `/redkiln:answer`), and no generated phase skills.
- Use Clef's calibration as the route by which an `llm` facet becomes admissible under ADR-0079 / ADR-0090.

**In the suite contract:** the artifact schemas, the hash-bound approval record, the gate-mark shape both sides read, the store-location rule, and an activity-id list shared by Flow's skills and Redkiln's catalog.

## Risks

- **Redkiln's design is very large and unbuilt.** 112 ADRs, no kernel, last commit 2026-09-11. Flow has merged 29 commits since 2026-10-11. If Flow waits on Redkiln for its process definition it stalls, so the contract must start from what Flow already has, and Redkiln conforms to it.
- **Redkiln's catalog is ~95 activities; Flow has 6 skills.** Do not try to give Flow all of them. The in-session surface should stay small and cover the activities a human does interactively.
- **Two runtimes can still disagree on a check.** Using the same binary for checks removes most of this. The rest is an "all installed" end-to-end test, already proposed in the suite write-up.

## Decisions for Ryan

1. **Who owns the process definition.** Recommended: Redkiln's catalog format, seeded from Flow's six skills so it starts from working code.
2. **Narrow `/orchestrate` and drop Flow's ship-side stubs.** Recommended: yes.
3. **Redkiln's tier-2 store on happenstance with TRUNCATE.** Recommended: yes, unless Redkiln's measurements need WAL.
4. **`rk-plugin-gen` limited to the conductor's commands, under the `redkiln` ID.** Recommended: yes.
