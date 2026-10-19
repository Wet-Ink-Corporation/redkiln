<!-- Exported from the Claude Doc https://claude.ai/code/artifact/742c955b-eb50-4a3a-9cd2-8da82651aa39 (revision 41) on 2026-10-19 as part of the project handover. The Doc is the original; this copy is frozen. Diagrams (1) could not be exported and show as [embedded content] placeholders; open the Doc to see them. -->

# Redkiln Bench: what the plugin should entail

Oct 18, 2026 · @Ryan

## Summary

Redkiln Bench should be the in-chat front end to the suite's process: the same activities, checklists and templates Loop runs unattended, driven live with you, refereed by hooks. anneal already is most of it, so the work is slimming a 63k-line binary and pointing its skills at the shared catalog, not building new phases.

- **What it is.** The phase skills (grill, spec, plan, build, diagnose, verify), in-session fan-out, handoff and correct, the 14 hooks, Clef's seven judgement points and the define-phase checks. It owns phase state, approvals of grills and plans, briefs, repros and the claim gate.
- **One catalog.** Bench and Loop run from one versioned catalog package through the redkiln-catalog kit crate (Blueprint call 8, amended to pressure-test rec 16). Each Bench skill renders a catalog activity and its checklist in chat; `bench activity show` and `bench activity check` are the bridge.
- **The issue is the plan.** A Backlog Story's requirements, acceptance criteria and test cases (R#, AC#, TC#) become Bench's plan and test list. `task done` ticks the checklist and proves each TC with a recorded test run.
- **Slimmer binary.** About 13k lines of skill QA move to the skillcheck dev tool, about 12.5k of guard, GitHub and verdict code to kits and Review, and rollout, perf and migrate freeze until Ship exists.
- **Judgement stays in shadow until scored.** Clef keeps its seven points; scoring moves to redkiln-calibrate, and a point enforces only after n of at least 50, Brier of 0.15 or less and an ask rate under 20%.
- **Calls.** Six, in the last section. Call A (retire anneal's templates for the catalog's) is the hardest to undo, because every spec written in chat accumulates in whichever format wins.

## What the repos already do

anneal is Bench almost whole, and Loop's catalog is the process it should render. The rest of the suite contributes conventions Bench must read rather than code it must absorb.

| Repo | What it does today | Worth lifting |
| --- | --- | --- |
| anneal (main, 47e8959) | 12 skills (`plugin/skills/`), 14 hooks on 9 events (`plugin/hooks/hooks.json`), 24 built CLI commands with a JSON envelope and typed exit codes (`src/command.rs`, `docs/exit-codes.md`). Define checks: `lint spec` (EARS, constitution), `lint plan`, `lint terms`, `grill frontier/classify/confirm`, `plan approve/check` by sha256. Build checks: `mode`, `task done -- <cmd>`, `repro record/leftovers`, `floor-guard`. Fan-out: `dag`, `brief`, `round`, `orchestrate`. Clef D1 to D7 in shadow, `calibrate`. Since the audit: `dag check --plan` (#75, #76), MultiEdit gated (#80), the completeness matcher (#84). Release 0.0.5; v0.1.0 (#44) open. | Nearly all of the skills, hooks and define and build checks. The ledger crate dissolves into Core. |
| redkiln-rs (build/phase-0-to-1) | `.redkiln/catalog/` holds 102 activities over 12 stages, each a TOML graph of draft, self-check, adversary, policy and gate steps; `structured-nl-requirements` v3 is one example. `.redkiln/packs/story.toml` and `bug-standard.toml` select them by facets. 11 of 39 referenced checklists exist (registry in `spikes/p9/registry-v2.toml`); #79 transplants the rest. | The activity, checklist and template content Bench should render in chat, and the hash-bound gate an approved plan can answer. |
| redkiln (TS prototype) | 26 template files (23 artifact templates, two gate templates and a brief template) and four agent prompts, with EARS and design rules inside its workflow prompts. | Source for about half of the missing checklists, through #79, not directly into Bench. |
| temper | `temper:gate`, called by Bench's build and verify in Rust repos; the Status grammar `DONE`, `WIP`, `BLOCKED`. | The gate as the build check. Its `WIP` must stop counting as a violation in Bench's status hook (`docs/hooks.md:330`). |
| weigh-in | `weighin batch` (raise, defer) and `open --json`, which `/orchestrate` and the D6 hook call; `.kb/_intake/decisions/` atoms D6 reads. | The decision route for one-way calls. The text receipt parse (`src/weighin/receipt.rs`) is replaced by weighin-contract. |
| assay | Produces PR verdicts; anneal's `merge-gate` denies `gh pr merge` without a fresh one (`docs/verdict.md`). | The verdict hand-off: Review owns `review.verdict`, Bench only reads it. |

Two facts shape the design. Nothing outside anneal references anneal today, so every integration is one-way and textual. And anneal keeps its own `spec.template.md` and `plan.template.md` (`plugin/playbooks/`), so an item specified in chat skips the catalog's checklists and adversary.

## What Bench is and owns

Bench keeps everything that needs a person in the chat and a hook on the session, and gives away everything another plugin also needs. That cuts the shipped binary to roughly the define, build and judgement layers.

### Keep, move or freeze

| anneal piece | Size (non-test lines) | Goes to | Why |
| --- | --- | --- | --- |
| Phase skills: grill, spec, plan, build, diagnose, verify | skills plus about 4.5k of define checks | Bench | The in-chat experience; rendered from catalog activities (next section) |
| `/handoff`, `/correct`, `.continue-here` | about 0.5k | Bench | Session-bound |
| `/orchestrate`, `brief`, `round`, brief and standing-orders hooks | about 5k | Bench, narrowed | In-session fan-out over a few tickets; unattended runs are Loop's (call C) |
| Hooks: route, ruled-before, preconditions, fast-check, injection-scan, context-monitor, completeness, status, handoff, bootstrap | about 9.7k | Bench | Bound to Claude Code events |
| Hook: guard and redaction | about 7.6k | redkiln-guard kit; Bench keeps its rule pack | Rust, Loop and Review need the same lexer (Blueprint, Kits) |
| `dag`, `pr_diff`, `watch` | about 4k | redkiln-github kit | Backlog, Loop and Review need them |
| `verdict set/valid`, `ship skip-rule` | about 1k | Review owns `review.verdict`; Bench keeps the `merge-gate` hook as a reader | One owner per domain |
| `lint skills`, `eval routing/uplift` | about 13k | tools/skillcheck, never shipped | Lints every plugin's skills; bloats every hook process today |
| `decide` (Clef client), `verify risk` | about 5.2k | Bench | The seven points are Bench's |
| `calibrate` scoring | about 2.1k | redkiln-calibrate (Core) | Review and Loop score judgements too |
| `rollout`, `perf`, `migrate` | about 1.7k | Frozen in Bench | No metrics source, no user; earmarked for Ship |
| `anneal-ledger` | about 2.7k | redkiln-core plus `bench.*` types | One store (Blueprint, Data and state) |

### The surface after slimming

- **Skills.** grill, spec, plan, build, diagnose, verify, and a trimmed ship that only asks Review for a verdict and stops. User-invoked: `/rk-bench:orchestrate`, `/rk-bench:handoff`, `/rk-bench:correct`, `/rk-bench:calibrate`. `/write-skill` moves to skillcheck's own docs, since it authors plugins, not product code.
- **Hooks.** The same 14 entries, each a thin call into the binary, each capped well under Claude Code's shared limit of 8 Stop blocks (pressure test, hooks from several plugins). Guards fail closed; everything else fails open. The bootstrap SessionStart hook stays within the suite's SessionStart context budget.
- **CLI.** `bench` with the existing envelope `{ok, command, data, findings, proves, does_not_prove}`, which becomes Core's CLI result contract. New: `bench activity show|check` and `bench status --json` (anneal #58), which the suite mod and the board read.
- **Config.** `.anneal/` becomes `.redkiln/bench/`, plus a \[bench\] table in .redkiln/suite.toml, the one suite pin that fixes Bench's and the catalog package's versions. The guard's hard-deny on edits to its own config moves with it.

## One catalog, two front ends

Bench and Loop should run the same process, and only the driver differs: you in the chat for Bench, the kernel for Loop. Loop's catalog is the more rigorous definition, with ranked spec forms, facet-driven design artifacts, checked slicing and a per-step adversary, so Bench renders it instead of keeping its own.

### How a Bench skill runs a catalog activity

1. The skill asks `bench activity show <id>` for the activity's steps, inputs, checklist and template, resolved from the catalog package plus any `.redkiln/catalog` overrides.
2. The draft step is the conversation: grill's rounds, spec's questions, plan's slicing. The model writes the artifact from the activity's template.
3. `bench activity check <id> <artifact>` runs the deterministic part: the checklist self-check, the policy expression and Bench's existing lints (`lint spec`, `lint plan`, `lint terms`). An adversary step runs as a fresh subagent when the item's rigor is at least recommended.
4. A gate step becomes a witnessed approval in chat (`grill confirm --quote`, `plan approve`), bound to the artifact's file hash, which Loop's planning gate accepts when it recomputes the same hash (Blueprint, Approvals).

The engine that loads, lints, selects and renders activities becomes the redkiln-catalog kit crate now, synchronous for Loop's kernel, with Bench as its second user (Blueprint call 8, amended to pressure-test rec 16). The content ships as one versioned data package with the suite train.

### Skill to activity map

| Bench skill | Catalog activities it renders | Artifact | What Bench adds |
| --- | --- | --- | --- |
| grill | problem-framing, assumption-mapping | ProblemStatement, AssumptionList | Design tree, `grill frontier`, verbatim confirmation |
| spec | user-story-ac, structured-nl-requirements, bdd-scenarios (ranked by the spec-form family) | Spec | `lint spec` (EARS, constitution), `lint terms` |
| plan | vertical-slicing | SliceSet, DependencyDAG | `lint plan`, `plan approve` by hash, `dag check --plan` |
| build | tdd-red-green-refactor, walking-skeleton | Changeset | `mode build`, preconditions, `floor-guard`, `task done` |
| diagnose | minimal-repro, root-cause | ReproCase, RootCause | `repro record`, `[DEBUG-id]` leftovers |
| verify | confirmation-test | ConfirmationResult | Fresh test run, doubt pass, Clef D5 |
| ship (trimmed) | none; asks Review | none | `merge-gate` reads `review.verdict` |

The other 90 or so activities stay reachable through `bench activity show`, but get no skill of their own: the in-chat surface stays small, as redkiln-vs-flow.md argued. CI checks that every skill's activity tags exist in the catalog (Blueprint, Bench: Tags).

### What this retires

- anneal's `spec.template.md` and `plan.template.md` (`plugin/playbooks/`), replaced by the catalog's templates (call A).
- `route.json`'s eight phase entries, which become a lookup of the catalog's stage order (`.redkiln/packs/story.toml`, `stage_order`).
- The risk that work specified in chat skips checklists Loop would apply. Most of those checklists are still unwritten (11 of 39 exist), so Bench's checks go no further than #79 has reached. That is porting work, not a design gap.

## From Story to plan: phase state and events

When a Story exists, Bench starts from its body instead of a blank grill. The Backlog contract already carries what Bench's plan needs: requirements, acceptance criteria in Given/When/Then, test cases mapped to AC, and a task checklist (Backlog study, The issue body is the contract).

### What Bench reads from the issue

| Issue body item | Becomes in Bench | Checked by |
| --- | --- | --- |
| R1, R2 (requirements) | The Spec's requirement list; `lint spec` checks each is one testable sentence | `lint spec`, the activity checklist |
| AC1, AC2 (Given/When/Then) | Acceptance criteria the plan's tasks must cover | `lint plan`: every AC covered by a task |
| TC1, TC2 (each naming its AC and a command or test path) | The build's test list, in order: red first, then green | `task done -- <cmd>` records a `bench.test_run` per TC |
| Tasks checklist | The plan's tasks; `dag check --plan` already refuses tickets that disagree with the plan (#75) | `plan check` by hash |
| Links: spec file, decisions | Inputs to grill and spec; open `wi-` ids block the plan | Weigh-In's `open --json` |

With no Story (a client repo, a Cowork folder, a quick fix), grill and spec produce the same artifacts locally, and Backlog stays optional (Blueprint call 10). If Backlog is present, `bench plan approve` offers to write the approved R, AC and TC back into the issue through backlog-contract, never by editing the body directly.

### Phase state

- **Per worktree.** Mode (build, diagnose, off), the active plan hash, open repros and round counts key on Core's normalised worktree id. Today anneal lower-cases the full toplevel path and weigh-in uses the directory name (audit, section 6.2).
- **Derived, not stored.** Phase is a projection over `bench.*` events, so a deleted projection rebuilds. `.continue-here` stays a working file, excluded from git.
- **Shown, not hidden.** `bench status --json` reports phase, plan hash, open TCs and open rounds for the suite mod, the board and Loop.

### Events

anneal's 18 types become `bench.*` names in the shared log, with lenient reads so a foreign type never fails a query (today `store.rs:474-479` fails on the first weigh-in event). The rename is a one-time import of old ledgers, done once in Core with the happenstance 0.4.1 move. anneal ledger journal maps to Core's export checkpoint, which Bench runs at Stop and before a push.

| Today | In the suite | Read by |
| --- | --- | --- |
| `grill_confirmed`, `plan_approved` | `bench.grill_confirmed`, `bench.plan_approved` (file hash, witnessed) | Loop's planning gate |
| `test_run`, `task_done`, `repro_red` | `bench.test_run`, `bench.task_done`, `bench.repro_red` | Bench's completeness hook, Loop's verify stage (in-session evidence only; CI does not read it) |
| `mode_set`, `round_started`, `gate_blocked`, `stop_blocked`, `subagent_status` | same names under `bench.` | Bench, suite status |
| `verdict_set` | `review.verdict`, owned by Review | Bench's merge-gate |
| `orchestrate.ruled` (wi-1154f8) | a two-way Weigh-In item raised through weighin-contract (call F) | Weigh-In, Knowledge |
| `weighin_parked`, `bridge.unavailable` | dropped; weighin-contract replaces the text bridge | none |
| `migrate.*` | frozen with migrate | none |

## Clef, calibration and the merge gate

Clef's seven points stay Bench's, and none of them drives control flow until its record earns it. That is already anneal's rule and the suite's (Blueprint design rule: a model's judgement does not drive control flow until it is scored).

### The seven points

| Point | Hook or skill | Question | Action when sure | Fallback |
| --- | --- | --- | --- | --- |
| D1 guard | PreToolUse Bash | Is a command the denylist did not decide destructive? | deny at 0.90, ask at 0.40 | ask |
| D2 orchestrate | `/rk-bench:orchestrate` | Is this call a one-way door to park in Weigh-In? | park at 0.50 | fail open |
| D3 route | UserPromptSubmit | Which phase does this prompt belong to? | hint at 0.60 | fail open |
| D4 completeness | Stop | Is each required report section present and grounded? | pass at 0.50, else block | fail open |
| D5 verify | verify | How risky is the change; may the doubt pass be skipped? | doubt review at 0.67 | fail open |
| D6 ruled-before | UserPromptSubmit | Was this question ruled on already? | surface at 0.70 | fail open |
| D7 grill | grill | Is this a decision for you or a fact to look up? | ask you at 0.50 | fail open |

Source: `docs/decide.md:111-117`, `plugin/playbooks/clef-thresholds.json`. Every point is in shadow today, because live Clef waits on the Cloudflare secrets (anneal #1). #44 expects most points to ship in shadow at v0.1.0.

### Calibration

- **Scoring moves to Core.** redkiln-calibrate takes anneal's row format, Brier, reliability bins and the promotion verdict (n of at least 50, Brier of 0.15 or less, ask rate under 20%). Review and Loop score their own judgements with it.
- **Two scopes.** Rows pool per user across repositories in `~/.redkiln/`, as anneal's `~/.anneal/decisions/` does today, with an optional per-repository scope (Blueprint, Calibration). Promotion stays a human edit of the thresholds file.
- **Labels come from the suite.** A Weigh-In decision labels D2 and D7; a Review verdict labels D5; a reverted command within 24 hours labels D1. These are the automatic labels anneal's `docs/calibrate.md` plans and the cockpit's one-key labels (#61) supplement.
- **The model behind Clef is a transport.** Today it is Cloudflare Workers AI custom models (`@cf/wet-ink/clef`, `clef-flash`). Bench keeps the `Transport` port so a point can later run on a Claude call through redkiln-dispatch without changing its record (call D).

### The merge gate

`merge-gate` denies `gh pr merge` in a session unless a fresh verdict exists, fresh meaning the same patch-id against the same base (`docs/verdict.md`). In the suite it reads `review.verdict` events, which Review writes in local mode and imports from CI journals. The authority moves to GitHub: a required check from Review (anneal #116), so a merge from the web UI or another agent is gated too. Which verdict the hook trusts follows Review call F: attested where CI exists, witnessed local elsewhere. The hook stays as fast feedback inside the chat, which is the three-layer enforcement the pressure test asks for (rec 14).

## Prior art and patterns

The in-chat workflow tools have converged on one funnel: question the idea, write a spec, cut tracer-bullet tasks, build test-first, verify before claiming done. Bench already has that funnel; what it adds is hooks that enforce each step and a catalog that does not change with the driver. None of the tools below enforces with hooks, and none shares its process with an unattended runner.

| Prior art | What it is | Pattern to take | What to leave |
| --- | --- | --- | --- |
| [superpowers](https://github.com/obra/superpowers) | A skill library: brainstorming, writing-plans, executing-plans, test-driven-development, systematic-debugging, verification-before-completion, subagent-driven-development | Skills that trigger themselves; a bootstrap re-injected after compaction; review checkpoints between plan batches | Enforcement by "mandatory" wording, which decays after compaction (pressure test, prior art) |
| [Matt Pocock's skills](https://skillselion.com/guide/matt-pocock-skills-map) | grill-with-docs, to-spec, to-tickets, implement, code-review; wayfinder above them | The same lineage as anneal's grill and tracer bullets; terms and decisions written down while grilling; the spec published straight to the tracker | A PRD as free text; tickets that list blockers in prose rather than native dependencies |
| [Spec Kit](https://code.morphllm.com/github/spec-kit/raw/tag/v0.3.1/docs/quickstart.md) | constitution, specify, clarify, checklist, plan, tasks, analyze, implement | clarify as targeted questions before planning; analyze for cross-artifact consistency; a constitution check, which anneal's `lint spec` already runs | One template per artifact; optional gates (the catalog ranks forms and gates each step) |
| [Kiro](https://kiro.dev/blog/introducing-kiro/) | An IDE whose specs are requirements in EARS, a design and dependency-ordered tasks, plus event-driven agent hooks | Each task links to the requirements it implements, so coverage is checkable: Bench's AC-to-task lint | Hooks that run an agent in the background on save; Bench's hooks are deterministic checks |
| OpenSpec (from the pressure test) | A change folder of spec deltas, archived into a living spec | Check against the catalog's docs-update and knowledge-harvest activities | No phase gates by design |
| Claude Code plan mode (from memory, not checked today) | A read-only planning mode before edits | Bench's build preconditions are the enforced version: no edit without an approved plan hash | Approval by a keypress with no record or hash |
| Example mapping (Backlog study) | Rules, examples and questions on cards | grill's output shape: rules become AC, examples become TC, questions become Weigh-In items | Nothing |

The pattern worth naming is the one Bench is ahead on: approval bound to a hash, and evidence recorded as events. superpowers asks for verification output before "done"; Bench's completeness hook blocks a claim that has no `bench.test_run` behind it.

## How it composes with the rest of the suite

Bench owns the in-chat experience of the process and nothing else: it reads the catalog and the issue, and writes approvals and evidence that other plugins gate on.

[embedded content: Bench in the suite · 2 inputs, 5 siblings, 1 Core]

Every line below goes through a typed event or a contract crate, never another plugin's internals or text output.

- **Loop** runs the same catalog unattended. A `bench.plan_approved` bound to a file hash answers Loop's planning gate when Loop recomputes the hash, so a plan approved in chat need not be approved twice. Work handed to Loop leaves Bench through the ready queue, not through `/rk-bench:orchestrate`.
- **Backlog** supplies the Story body as Bench's plan input, and takes back approved R, AC and TC through backlog-contract. Bench's `dag` code moves into redkiln-github, which Backlog also uses. In client repos, Bench's dag reads local tickets from Backlog's local mode (a file store under .redkiln/backlog/) through backlog-contract.
- **Weigh-In** takes every one-way call Bench parks (D2, `/rk-bench:orchestrate`, plan questions) through weighin-contract, replacing the text receipt parse. Its decisions label D2 and D7 for calibration.
- **Redkiln Rust** stays the build and verify check in Rust repositories. Bench's floor-guard skips `#[expect(` because Rust judges it. Both guard Bash with their own rule packs over redkiln-guard.
- **Review** owns `review.verdict`; Bench's merge-gate only reads it, and a required check on GitHub carries the authority (anneal #116). Review owns the traceability check (`/rk-review:trace` in CI); Bench's `bench.test_run` per TC is in-session evidence only, and CI does not read it.
- **Knowledge** takes `.out-of-scope/` rulings and ADRs as atoms; Bench's ruled-before hook (D6) reads them back at prompt time, and defers to Knowledge's just-in-time injection when Knowledge is installed.
- **Core** holds the log, the approval record, the Status grammar (so temper's `WIP` stops counting as a violation) and redkiln-calibrate.

## Calls for Ryan and next steps

Six calls shape Bench. Several points are already open elsewhere and are not re-asked here: the catalog package and kit crate (Blueprint call 8, amended to pressure-test rec 16), where spec artifacts live (pressure-test call E), Bench's slimming (Blueprint call 6) and Backlog being optional (Blueprint call 10).

| # | Call | Recommendation | Door |
| --- | --- | --- | --- |
| A | Retire anneal's own spec and plan templates; Bench's skills render the catalog's activities, checklists and templates, writing to pressure-test call E's home | Yes | One-way once in-chat specs accumulate in either format |
| B | Give skills only to the seven activity groups in the map above; every other activity is reachable through `bench activity show` | Yes | Two-way |
| C | Narrow `/rk-bench:orchestrate` to in-session fan-out over a handful of tickets; unattended multi-ticket runs are Loop's | Yes | Two-way |
| D | Keep Clef's Cloudflare transport behind a port, so a point can move to a Claude call through redkiln-dispatch later | Yes, defer the move | Two-way |
| E | Hold anneal's v0.1.0 marketplace release (#44) and publish Bench only into the Redkiln marketplace, after the rename | Yes; still cut the calibration report | One-way once an `anneal` install ID is public |
| F | Route `/rk-bench:orchestrate` rulings (`orchestrate.ruled`, wi-1154f8) to Weigh-In as two-way items, reversing anneal's own record of them | Yes | Two-way |

### Suggested next steps

- Ryan decides calls A to F, and Blueprint call 8 (amended to pressure-test rec 16) with pressure-test call E, both of which A depends on.
- Spike: render `user-story-ac` and `vertical-slicing` in chat from the catalog, with `bench activity show|check` over the existing rk-kernel catalog modules, before they move into redkiln-catalog.
- Write Bench's skill-to-activity tags and a CI check against the catalog, the cheap half of the alignment.
- Make Bench's status hook accept temper's `WIP` now, ahead of Core's one Status grammar.
- Split skill QA out of the binary into tools/skillcheck, the largest single cut (about 13k lines).
- Keep running Clef in shadow and collecting labels; v0.1.0's calibration report is useful whether or not the release ships.
