<!-- Exported from the Claude Doc https://claude.ai/code/artifact/af601899-044b-4779-bacb-f0ef57d4f7ae (revision 26) on 2026-10-19 as part of the project handover. The Doc is the original; this copy is frozen. Diagrams (1) could not be exported and show as [embedded content] placeholders; open the Doc to see them. -->

# Redkiln Review: what the plugin should entail

Oct 18, 2026 · @Ryan

## Summary

Redkiln Review decides whether a pull request may merge, as one required check that no agent and no PR author can forge. Most of it is already built in assay's `origin/main` (about 48k lines of Rust, six crates); what is missing is everything that touches GitHub, and that waits on five owner setup tasks (#39 to #43). So the work is finishing assay, renaming it while that is still cheap, and opening it to the rest of the suite.

- **Two deliverables.** A CI deployment (the `review` binary by digest, workflow templates, the verdict GitHub App, a private ledger repository) holds the authority. A thin plugin (`/rk-review:check`, `/rk-review:status`, and later a Review tab in the suite mod) is advisory and owns no state.
- **One finding shape.** Review owns the findings schema and the verifier, published as `review-contract`. Redkiln Rust's reviewer, Bench's doubt pass and Loop's review gate all emit it, so a finding raised in chat is recognised in CI.
- **Verdicts bound to content and identity.** A verdict names the patch-id and base it judged, so a clean rebase keeps it. Only the App's check run is attested, so only it satisfies a merge gate.
- **Shared with the suite, not owned alone.** Capacity leases over git refs (assay's ADR-19, ADR-23) become Core's cross-machine backend; calibration becomes `redkiln-calibrate`; the Rust checks assay re-implements move to Redkiln Rust subcommands, pinned by digest.
- **Interop outward.** SARIF output and a REVIEW.md generated from the rubric let GitHub code scanning and other reviewers apply the same standard. A traceability check ties each acceptance criterion in the closing issue to a test.
- **Calls.** Seven, in the last section. Call A (the rename and its new identifiers) is the one that is hard to undo, and only cheap until the App and ledger exist.

## What the repos already do

Four repositories already review code, and none of them shares a finding shape, a verifier or a verdict record with another. assay is the most complete and the only one built for CI; the others hold the pieces Review should absorb or serve.

| Repo | What it does today | Worth lifting |
| --- | --- | --- |
| assay (`origin/main`, d53ab77, 2026-10-14) | Six crates, about 48k lines of source and 41k of tests. Pure `decide` over rubric v1, tiering (`micro` to `full`), findings validation with evidence relocation, blind specialists, a blind verifier and a coordinator with sharding. It also has re-review with a carried finding set, a sanitiser and egress scan, a hardened `claude -p` runner, an eval harness (k = 3) with 30 mined cases, and `check --local` with an `assay.event/1` stream. Six versioned JSON schemas with 75 fixtures. Missing: the GitHub adapter (#53), workflows (#60, #61), the ledger and semaphore (#70 to #73), all blocked on owner setup #39 to #43 | Nearly all of it. It becomes the `review-*` crates, renamed |
| assay, decided | ADR-04: models never approve. ADR-11: one App publishes the only required check. ADR-13 and QA-05: LLM blockers stay advisory until the lower 90% Wilson bound of precision reaches 70% over 20 labelled blockers. ADR-19 and ADR-23: the ledger repository and a two-slot semaphore over git refs. ADR-25: the mod is a view with two owner actions, Waive and Override, never Dismiss (wi-f626a4). | The rules, unchanged, as Review's charter |
| temper | `temper:rust-reviewer` returns a validated verdict JSON bound to the reviewed tree, scored on the rust-craft rubric. Its checks for test inventory, manifest test switches and lint weakening run only in hooks, so assay re-implements them in `assay-gates` (9,466 lines of production code in its temper adapter) | The rubric's Rust dimensions stay in Redkiln Rust; the re-implemented checks move back there as subcommands Review calls by digest |
| anneal | `anneal verdict set` records `verdict_set` with `sha`, `patch_id` and `base`; a verdict is fresh when the head's patch-id matches. The `merge-gate` hook requires a review verdict once `review.require_plugin = true`, and waits for "the review plugin" to append `reviewed`, `approved`, `changes_requested` or `noise_only`. Its `verify` doubt pass shows reviewers the diff and the contract, never the claim | The patch-id freshness rule, and the merge-gate as Review's first in-session consumer through a `review.verdict` event |
| redkiln-rs (`build/phase-0-to-1`) | `rk-kernel/src/review/` (1.2k lines): "reviewer proposes, machine confirms" (ADR-0074, ADR-0096). A finding blocks only after a typed repro re-executes; a policy table is the only place a class becomes a block; quarantine and advisory rows | Re-execution as a verifier mode in Review: assay's deferred "executable witness" (spec `07 §5`) is the same idea |
| weigh-in | No `ci-check` yet. assay's CP2 needs a versioned `weighin ci-check --json --base <ref>` (D-WI). D-WI is not yet filed with Weigh-In; assay #44 tracks filing it | The request moves to Weigh-In's own blueprint as a contract, not a bilateral ask |

Two facts shape the design. First, the three review paths overlap in intent (blind reviewers, evidence by `file:line`, a confirm step) but share no code or schema. Second, assay's identifiers are now in crate names, GHCR packages, the check name, schema ids, labels, commands, env vars and a planned App, but nothing is released, so the rename is still cheap.

## Two deliverables: the deployment and the plugin

Review's authority comes from the CI deployment, because a marketplace install cannot deliver a trust anchor (Blueprint call 4). The plugin is a convenience over the same binary, and nothing it writes is ever read by CI. A client repository has no CI deployment: a local `/rk-review:check` works there, but there is no SARIF, no ledger and no App verdict.

|  | CI deployment | Plugin (`rk-review`) |
| --- | --- | --- |
| Installed by | A repository owner, once per consumer repository | Anyone, from the Redkiln marketplace |
| Ships as | The `review` binary on GHCR, fetched by digest; base-defined workflow templates; the `redkiln-review` App manifest; a fetch-by-digest action (`deploy/review/` in the monorepo) | Skills, a launcher and the same binary in `libexec/<target>/` |
| Runs | GitHub Actions in five zones, Z0 to Z4. No job holds PR code, the model secret and a write token together (ADR-03, ADR-12) | Your session, with the interactive `claude` |
| Holds state in | The private ledger repository (per-PR state, cost, leases), with compare-and-set on refs; a 90-day run bundle per run | Nothing of its own. Local runs write `review.*` events to the shared log |
| Its result | The `review / verdict` check, published only by the App, attested | `local · advisory`, witnessed at most |
| Commands | `/review recheck`, `waive`, `ack`, `override`, `retry`, `dismiss`, `weigh-in-ack` as PR comments from identified people | `/rk-review:check`, `/rk-review:status`, and later a Review tab in the one suite mod: read-only, plus Waive and Override (ADR-25) through the CLI, with no Fix |
| Platforms | Linux GitHub-hosted runners | Linux, Windows and macOS through libexec |

### What stays fixed from assay

- **Models never approve.** `decide` is a pure function of gate results, validated findings and the rubric; a model's output is an input to it, never a verdict (ADR-04).
- **Fail closed.** A missing input, a degraded role or an unknown reason refuses rather than guesses (wi-b5affa, #185).
- **Pinned tools.** The pipeline runs Redkiln Rust and every gate at the digest named in the consumer's base branch, never the PR's own copy (P2, T4). In the monorepo this means the CI never uses the in-tree build.
- **Promotion is a human edit.** LLM blockers and majors are promoted per repository, separately, when QA-05 is met, by a config change that is itself a trust-path change (#117).
- **Privileged identities are not agents.** The approver account and the App key are never reachable from a session (ADR-16, ADR-24).

### What changes in the move

- **Names.** `assay-*` crates become `review-domain`, `review-app`, `review-forge`, `review-gates` and `review`; `review-contract` is new. The check, App, labels, commands, `.review/` config, `REVIEW_*` env vars and schema ids follow (call A).
- **Shared code leaves.** The `claude -p` runner and process supervisor go to `redkiln-dispatch`; the ledger and semaphore design becomes `redkiln-capacity`'s git-refs backend; the Wilson-bound promotion rule goes to `redkiln-calibrate`; the PR and check-run client goes to `redkiln-github`.
- **The Rust checks go home.** Baseline raise, exceptions, gate config and test inventory (with manifest switches) become Redkiln Rust's `check` subcommands (Redkiln Rust call D; D-TEMPER-1, #45). `review-gates` keeps only a runner and a mapping into findings, so its temper adapter (9,466 lines of production code) shrinks to that.
- **Path rules narrow.** assay's self trust path is all of `crates/**`, and its architecture tests assert exactly six crates. Both must narrow to `crates/review/**` before the move, or every sibling PR becomes an owner-approved trust-path change.

## Findings, verdicts and trust

Review owns three things every sibling depends on: what a finding looks like, what a verdict is bound to, and which verdicts may be believed. Each is a type in `review-contract`, so siblings emit and read them through types, not text.

### One finding shape

`assay.findings/1` becomes `review.findings/1` with its fields unchanged: class (11 values, from `logic` to `deps`), kind (`defect` or `omission`), severity (`blocker`, `major`, `minor`, `nit`), `file` and `line`, evidence of at most 400 characters, title, body, and optional `rule`, `fix`, `trigger`, `caller`, `confidence`, `introduced_by` and `pre_existing`. Producers never supply the fingerprint; Review computes it, so the same defect matches across runs and surfaces.

| Producer | Today | Under the contract |
| --- | --- | --- |
| Review's specialists | `assay.findings/1` | Unchanged, renamed |
| Redkiln Rust's reviewer | Its own verdict JSON, scored on the rubric | Keeps its rubric scores; its findings array is `review.findings/1` with `rule` set to the rust-craft id |
| Bench's doubt pass | Prose, `file:line` or rejected | Emits `review.findings/1`; calls `review verify` in-session |
| Loop's review gate | `rk-kernel` review records | Emits `review.findings/1`; keeps its policy table, which reads Review's verifier result |
| Redkiln Rust's gate, other gates | temper's findings JSON | A gate source with a `conclusion`, as assay's adapters already map it |

The verifier is offered in three modes: **lite** and **full**, as assay built them (a blind verifier that never sees the rationale, evidence relocated against the tree), and **re-execute**, Loop's ADR-0074 rule that a finding blocks only after a typed repro fails in a sandbox. Re-execute is assay's deferred executable witness, so it lands once, here.

### What a verdict is bound to

A verdict is a `review.verdict` event: the PR, its head SHA, its base, the patch-id of `diff <base>...<head>`, the rubric version, the conclusion and its reason codes. This follows the pressure test's rule (recommendation 5) and anneal's freshness rule.

| Question | Answer |
| --- | --- |
| Is a verdict fresh after a clean rebase? | Yes, when the head's patch-id from the same base matches |
| After a squash merge? | Yes, for the same reason |
| After any new change? | No: the patch-id moves, and the verdict is void |
| What does a gate run bind to? | The gate's declared hash method: the change fingerprint for Redkiln Rust's gate, the tree hash for Loop's verify gate. Review records which gate runs it consumed |
| What do Waive and Override bind to? | The finding fingerprint or the reasons digest, plus the head they were given on, as assay specifies |

### Who can be believed

Core's three trust levels (pressure-test call C) decide what each surface may satisfy. Review is the suite's main producer of attested facts.

| Level | Review's examples | May satisfy |
| --- | --- | --- |
| Asserted | A local `/rk-review:check` result; any imported journal line; a finding from any producer | Context only: shown, never a gate |
| Witnessed | A local verdict after you press Run in the mod; a Waive you confirm in the session | Bench's in-session merge-gate, when the repo is configured to accept it |
| Attested | The App's `review / verdict` check run; a compare-and-set on the ledger repository with its deploy key; a `/review` command from a named maintainer | Merge rulesets in CI, Loop's unattended review stage |

Two rules follow. A PR author can never override their own verdict, which assay already enforces through P10 and the approver account. And `redkiln-guard` denies an agent's Bash call that would write a human-actor `review.*` event.

## Commands, data and hand-offs

One `review` binary serves CI, the plugin and siblings. Every command takes `--json` and returns the Core result envelope, so the mod, Bench and Loop read it without parsing text.

### The CLI

| Command | Where | What it does | State |
| --- | --- | --- | --- |
| `review plan`, `admit`, `run`, `decide`, `publish` | CI zones Z0 to Z4 | Tier the PR, take a capacity lease, run gates and reviewers, adjudicate, publish the check | Use cases built behind ports; wiring waits on #53 |
| `review check --local` | Plugin, any shell | Advisory review of the working tree; `--events` streams `review.event/1` | Built (#107, #215, #231) |
| `review status` | Plugin, mod, siblings | `review.status/1` for the current branch's PR: verdict, findings, degraded roles, Weigh-In state, cost | Schema and logic built; the GitHub read waits on #53 |
| `review verify` | Bench, Loop, Rust's reviewer | Validate a findings file and run the verifier in lite, full or re-execute mode | New entry point over built code |
| `review trace` | CI gate, Bench's verify | Check each AC in the closing issue maps to a test the PR adds or touches | New (below) |
| `review replay --verify` | Anyone | Re-run `decide` over a stored bundle and diff the result | Use case built (#74) |
| `review export sarif`, `review export review-md` | CI, release | Write SARIF from a run; write a `REVIEW.md` from the rubric | New (below) |
| `review eval`, `self-gate` | Review's own CI | Score reviewers on the corpus; replay `decide` over 26 golden cases | Built (#111, #76) |
| `review doctor` | Owner | Token expiry, canaries, later the App and ruleset | Partly built |

### Events and state

Review keeps its own state model, because CI jobs share no disk (ADR-07, 09, 15, 23). The shared log sees Review only in local mode and through imported CI journals.

| Data | Where | Authority |
| --- | --- | --- |
| Per-PR state, pushes, commands, cost, leases | Private ledger repository, compare-and-set on refs | Authoritative; the check run is a projection of it |
| Run bundle (`review.run-inputs/1`) | Actions artifact, 90 days | Replay source |
| `review.verdict`, `review.finding_disposed`, `review.run` | Shared log, written by local runs and imported from CI journals | Asserted when imported; Bench's merge-gate trusts an attested verdict it can confirm against the check run where the CI deployment exists, and a witnessed local verdict elsewhere (call F) |
| Repo config | `.review/` on the base branch: `plugins.toml`, owners, exceptions, suppress, globs | Committed and reviewed; changes are trust-path changes. `.review/` is the one exception to the suite's `.redkiln/<plugin>/` config home, because Review reads its trust-path config from the base branch. |

### Capacity

The measured collision is real: assay's unattended build paused at 90% of the weekly limit because builders and reviewers share one subscription. Review uses `redkiln-capacity` with its git-refs backend, which is assay's design (two slots, 45-minute TTL, release by `--force-with-lease`), generalised so a laptop session and Loop take leases from the same ledger (Blueprint call 2).

### New checks the suite asks of Review

- **Traceability.** Backlog's issue body carries R, AC and TC ids. `review trace` reads the closing issue through `redkiln-github`, then checks each AC has a test case naming a path or command the PR adds or touches, and that the test passed in the gate run. A gap is a `tests` finding with kind `omission`. It runs only in Backlog-enabled repositories (Blueprint call 10).
- **SARIF.** Each finding maps to a SARIF result with the fingerprint as `partialFingerprints`, so GitHub code scanning can track it across commits ([upload a SARIF file](https://docs.github.com/en/code-security/how-tos/find-and-fix-code-vulnerabilities/integrate-with-existing-tools/upload-sarif-file)). This is an extra output; the check run stays the authority. Code scanning on private repositories needs GitHub Code Security, so it is optional.
- **REVIEW.md.** A generated `REVIEW.md` states the rubric's severity meanings, skip rules and always-check rules, so Claude Code Review, Bugbot or a human applies the same standard (pressure-test recommendation 18).

### What Review consumes

| From | How | Note |
| --- | --- | --- |
| Redkiln Rust | `gate`, `scan`, `baseline --check --findings-out FILE`, and the new `check raised`, `check exceptions`, `check gate-config` and `check tests` subcommands (Redkiln Rust call D), run by digest | The digest is pinned separately from Review's own, so each releases alone. CI checks the Rust digest in `.review/plugins.toml` against `.redkiln/suite.toml`, the suite's one pin. |
| Weigh-In | `weighin ci-check --json --base <ref>`, versioned, with `blocks` as `merge`, `release`, run, `none` or `unknown` | Specified in Weigh-In's blueprint as a contract (replaces D-WI, which assay #44 tracks) |
| Backlog | Issue snapshots through `backlog-contract` | For `trace` only |
| Core | Envelope, ids, approvals, journals, calibration, capacity | Linked statically |
| Kits | `redkiln-dispatch`, `redkiln-github` | Replace `assay-runner`, `forge::process` and the unbuilt Forge adapter |

## Prior art and patterns

Hosted AI reviewers have converged on a verify step, severity tags, a guidance file and feedback learning, and every one of them stops short of blocking a merge. That is the gap Review fills: a gate whose verdict is computed, attested and replayable, with model findings promoted to blocking only once they are measured.

| Prior art | What it is | Pattern to take | What to leave |
| --- | --- | --- | --- |
| [Claude Code Review](https://code.claude.com/docs/en/code-review) | Anthropic's managed multi-agent PR review, research preview for Team and Enterprise | A verification step that filters candidates; three markers (Important, Nit, Pre-existing); `REVIEW.md` for severity, nit caps, skip rules and re-review convergence; a machine-readable severity line in the check output; annotations written independently of inline comments | Its check is always neutral and never blocks, by design; cost of $15 to $25 a review billed as credits. Review should read the same REVIEW.md, not compete |
| [Cursor Bugbot](https://cursor.com/changelog/04-08-26) | Hosted PR reviewer with autofix | Learned rules from reactions, replies and human comments, promoted on signal and disabled when they stop helping; a published resolution rate (78%) | Rules that change silently in a vendor store; Review promotes only by a human config edit (QA-05) |
| [GitHub SARIF upload](https://docs.github.com/en/code-security/how-tos/find-and-fix-code-vulnerabilities/integrate-with-existing-tools/upload-sarif-file) | Code scanning accepts any tool's SARIF | `partialFingerprints` for tracking a finding across commits; one upload per tool and category per commit | Code scanning as the gate: it needs Code Security on private repositories |
| CodeRabbit (from memory, not checked today) | Hosted reviewer configured by `.coderabbit.yaml`, with "learnings" from chat | Path-scoped instructions; a walkthrough summary first | Free-text learnings with no measurement |
| Gerrit Code-Review labels (from memory) | Votes from -2 to +2 with a submit rule | A verdict that is a typed vote by an identity, and a rule that computes "submittable" from votes | Per-repo Prolog rules; Review's `decide` is the same idea in Rust |
| GitHub Agentic Workflows (from the pressure test) | Markdown workflows compiled to Actions | Read-only agent tokens; every write a "safe output" a deterministic job validates | Nothing new: Review's zones already do this |
| Copilot cloud agent (from the pressure test) | GitHub's hosted coding agent | It cannot approve or merge its own PR | Nothing: Review's P10 already holds this |
| Reviewdog, Danger (from memory) | Linters to PR comments; rules as code in CI | One adapter per tool into one comment format | Comment spam: Review posts at most eight inline comments and one summary |
| redkiln-rs ADR-0074 | "Reviewer proposes, machine confirms" | A finding blocks only after a typed repro re-executes | Its own finding records, once the contract exists |

The common thread is that review findings are cheap and wrong often enough that nobody lets them block unmeasured. Review's answer, assay's QA-05 promotion by Wilson bound and the verifier's re-execute mode, is stricter than any of these, and it is the reason the suite needs its own reviewer rather than adopting a hosted one.

## How it composes with the rest of the suite

Review owns findings and verdicts and nothing else; siblings send it findings in one shape and read its verdict as an event, and only the App's check run is believed in CI.

[embedded content: Review in the suite · 3 producers, 3 consumers, 3 inputs]

Arrows run from producer to reader. Review links Core and the kits but calls no sibling's internals; Redkiln Rust and Weigh-In are reached through their CLIs, pinned by digest.

- **Redkiln Rust** runs inside Review's gate zone at a pinned digest, and gains the four check subcommands Review needs (check raised, exceptions, gate-config and tests). Its reviewer emits `review.findings/1`, so Review can validate an in-session Rust review the same way it validates its own specialists.
- **Bench** emits findings from its doubt pass and calls `review verify`. Its merge-gate reads `review.verdict` and checks freshness by patch-id, replacing `anneal ledger append` (call F).
- **Loop** takes capacity leases from the same git-refs ledger as Review, so an unattended build and the reviewers no longer collide on one subscription. Its review stage requires an attested verdict, and its re-execution rule becomes Review's third verifier mode (call C).
- **Weigh-In** answers `ci-check` so one-way decisions block a merge, and an owner ack (`/review weigh-in-ack`) is recorded on the PR. Review's own design questions keep going to Weigh-In, as assay's 16 atoms already do.
- **Backlog** supplies the closing issue's AC and test cases. Review owns the traceability check (`/rk-review:trace` in CI); Backlog only posts the closing summary, and Review never writes issues.
- **Knowledge** receives rulings: waived and false-positive finding classes become ruling atoms, and won't-fix findings and lessons become learning atoms. All are proposed until accepted, so the next review knows what was ruled before.
- **Core** holds the envelope, the approval record, trust levels, journals, `redkiln-capacity` (git-refs backend) and `redkiln-calibrate` (the QA-05 promotion rule). Review's CI journals import into the shared log as asserted events.

## Calls for Ryan and next steps

Seven calls are specific to Review; each has a recommendation. Three questions that shape Review are already asked elsewhere and are not repeated here: Blueprint call 2 (capacity over git refs), Blueprint call 4 (a deployment plus a thin plugin) and pressure-test call C (trust levels in Core from v1).

| # | Call | Recommendation | Door |
| --- | --- | --- | --- |
| A | Rename assay to Redkiln Review now, before the App, ledger repository and approver exist: check `review / verdict`, App `redkiln-review`, `.review/` config, `review.*/1` schema ids, `REVIEW_*` env vars, `review-*` crates | Yes, before #39 to #43 | One-way once the App exists and a consumer requires the check |
| B | Publish one finding shape, `review.findings/1` in `review-contract`, and have Redkiln Rust's reviewer, Bench's doubt pass and Loop's review gate emit it | Yes, while schemas still change in place (wi-93b515) | Two-way until v0.1.0 ships |
| C | Loop's re-execution rule (ADR-0074) becomes Review's third verifier mode, and Loop's gate calls it rather than keeping its own | Yes | Two-way |
| D | A verdict binds to the patch-id plus base, so clean rebases and squash merges keep it | Yes; this applies pressure-test recommendation 5 to Review | Two-way |
| E | See Redkiln Rust call D, which owns it: assay's temper checks move from `review-gates` into Redkiln Rust subcommands, called by digest | Answered in Redkiln Rust call D | Two-way |
| F | Bench's merge-gate accepts only an attested verdict where the CI deployment exists, and a witnessed local verdict elsewhere | Yes | Two-way |
| G | Wave 1 adds `review trace` and a generated REVIEW.md; SARIF export waits | Yes | Two-way |

### Suggested next steps

- Ryan decides call A first, because it gates the owner setup.
- Rename in assay's own repository, then do the owner setup (#39 to #43) under the new names.
- Narrow the self trust path and the six-crate architecture tests to Review's own crates, so the monorepo move does not trip them.
- File the two upstream contracts in their own repositories: Redkiln Rust's subcommands (D-TEMPER-1 and -2, #45) and Weigh-In's `ci-check` (D-WI, #44).
- Draft `review-contract` with `review.findings/1`, `review.verdict` and the verifier's three modes, with golden fixtures, then point anneal's merge-gate at `review.verdict` instead of `anneal ledger append`.
- Spike: one consumer repository (weigh-in, per #118) with the check required, to measure cost per PR and the lease contention with Loop.
