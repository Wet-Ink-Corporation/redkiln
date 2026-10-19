<!-- Exported from the Claude Doc https://claude.ai/code/artifact/19797633-7ab5-44b3-943e-89c25bfd5028 (revision 42) on 2026-10-19 as part of the project handover. The Doc is the original; this copy is frozen. Diagrams (1) could not be exported and show as [embedded content] placeholders; open the Doc to see them. -->

# Redkiln Loop: what the plugin should entail

Oct 18, 2026 · @Ryan

## Summary

Redkiln Loop should be the suite's unattended runner: the `redkiln` binary and its kernel, run from any host, working ready issues through the process catalog and stopping only at human gates. Most of it is already built on redkiln-rs's `build/phase-0-to-1` branch (12 crates, about 102k lines of non-test Rust across the workspace, 199 ADRs as of 2026-10-17). So the work is shrinking Loop onto the suite's shared parts and proving it against a real model, not designing it.

- **What it owns.** Unattended execution, the catalog's content, the trailer grammar and its own projection store. Items, atoms, findings, decisions and the generic dispatcher move to their owners.
- **Every host is ephemeral.** Each `redkiln run` rebuilds its state from git, GitHub and the ledger repository, so your laptop, an Actions job and a cloud routine are deployment choices, not designs.
- **Least privilege.** Loop writes only through validated outputs, accepts only attested answers at unattended gates (a witnessed one suffices only on the machine that holds the board), and never approves or merges its own pull request.
- **Questions travel.** A human gate becomes a Weigh-In decision request, published to a rendezvous every host can read, and Loop resumes from the answer wherever it next runs.
- **Status.** Phase 1 is about 65% done; Checkpoint 8i closed on 2026-10-15 and 8j (fixture proof of both lanes) is planned. No lane has yet run against a real model.
- **Calls.** Six, in the last section. Call A (the run's write identity: a GitHub App rather than your token) is the one hardest to undo once client repositories install it.

## What the repos already do

Loop is the largest and most tested codebase in the suite, and nearly all of it is on redkiln-rs's `build/phase-0-to-1` branch, not `main`. The other repositories each built one piece Loop needs, so the plugin is assembled by lifting, not invented.

| Repo | What it does today | Worth lifting |
| --- | --- | --- |
| redkiln-rs (`build/phase-0-to-1`, 3fa69db, 2026-10-17) | The `redkiln` binary: `run`, `answer`, `status`, `explain`, `cost`, `doctor`, `init`, `lint`, `catalog search`. A sync kernel (`rk-kernel`, about 103k lines of Rust including its tests, about 46k without) selects activities, runs gates, memo and replay, and dispatches one `claude -p` child per activity in a run-owned worktree. Git trailers (`Story:`, `Slice-Verdict:`, `Gate:`) are tier 1; a WAL SQLite store is the mirror. 102 catalog activities, 12 packs authored with two enabled (`bug-standard`, `story`), 8 agent cards. 1,453 tests, CI green on Linux and Windows, plus structural and no-egress jobs. Checkpoint 8i closed 2026-10-15; 8j (fixture proof of both lanes) is planned, not started. No release and no plugin surface | Nearly all of it: kernel, catalog, trailer grammar, worktrees, gates, containment, the harness, the structural CI. Not its store opener, `backlog.rs`/`item_write.rs`, the `Tracker` and `DocsSite` fakes, or the planned `rk-plugin-gen` |
| redkiln (Node prototype 0.21.0) | Backlog-as-code: `.bklg` items with typed frontmatter, 26 template files (23 artifact templates, two gate templates and a brief template: spec, plan, design, slices, story map, retrospective and more), `forge-*.js` workflow prompts (fix, implement, initiative, plan briefs, PRDs, design, closeout), HITL/AFK flag per item | The templates and prompts as sources for about half of the 28 unwritten checklists and conventions (redkiln-rs #79). Not `.bklg` as a store |
| anneal | `orchestrate fan-out\|park\|rule`, `brief build`, `round start\|status` (3 same-agent rounds, 2 escalated, then park), `dag frontier`, `watch pr\|ci` with one exit code per blocker, and `merge-gate`, which denies a merge without a fresh verdict | `watch` and `dag` via `redkiln-github`; the round and escalation ladder as Loop's escalation tiers; merge-gate's fresh-verdict rule |
| assay | A hardened headless `claude -p` runner, and a designed cross-repo semaphore: lease refs in a private ledger repository, created atomically and reaped only by compare-and-set (`git push --force-with-lease`), cost records `reserved` then `final` (ADR-19, ADR-23) | The runner into `redkiln-dispatch`; the lease design as `redkiln-capacity`'s git-refs backend |
| weigh-in | Decision items with door, authority, options, default and deadline; a sitting and an Artifact board; one-way items never defaulted; client repositories keep data local (wi-a93f02). Loop's build already used it for operator approvals (`wi-ef7b00`), with no code coupling | Loop's human gates as Weigh-In decision requests (Blueprint call 7) |

Two facts shape the design. First, Loop was built as the opposite runtime to the rest of the suite: a headless CLI that never waits, where the others are hooks and skills inside a session. Second, the code that would duplicate its siblings (items, `.kb`, review, the dispatcher, the store) is either small, fake-only or already a separate crate, so the shrink is cheap.

## What Loop is and owns

Loop owns one thing: running catalog activities against a work item with nobody watching, and recording what happened so any later run can pick up. Everything else it touches belongs to a sibling, reached through that sibling's contract crate.

### Is

- **The `redkiln` binary** in `plugins/loop/libexec/<target>/`, behind the suite's shared launcher, from the `rk-*` crates under `crates/loop/`.
- **The kernel:** selection as a pure function of four digests, the rigor lattice, memo and replay, budgets, retry, the three gates (human, review, verify) and reconcile. It stays sync, with no tokio (ADR-0076).
- **The catalog's content:** 12 process packs over 12 stages, 102 activities, 8 agent cards, the registries and checklists, shipped as a versioned data package that Bench also runs from (Blueprint call 8, amended to pressure-test rec 16).
- **Run worktrees and trailers:** `.redkiln/worktrees/<item>` on `rk-run/<item>`, content checkpoint commits, and the `Story:`, `Slice-Verdict:` and `Gate:` trailer grammar as tier-1 authority.
- **Toolchain adapters:** Rust and Node today, with affected-test selection and per-AC mutation evidence.
- **A small hand-written plugin** in place of the planned `rk-plugin-gen`.

### Commands

| Surface | Command | Does |
| --- | --- | --- |
| CLI | `redkiln run <item> \| --next [--lane fix\|story]` | The one entry point; --next (new) picks the next ready item marked mode/afk. Idempotent, no `--resume`: reconcile, snapshot, select, dispatch, commit, exit at a gate or at landing |
| CLI | `redkiln answer <item> <option>` | Fallback gate channel when Weigh-In is absent (ADR-0009 allows it) |
| CLI | `redkiln status`, `explain <item>`, `cost <item>` | Read side: where each item stands, why this DAG ran, cost per activity |
| CLI | `redkiln doctor [--rebuild]` | Zero-cost configuration checks; rebuild the projection from git |
| CLI | `redkiln init`, `lint`, `catalog search` | Set up `.redkiln/`, lint packs and catalog, search activities |
| Plugin | `/``rk-loop:run, /rk-loop:status, /rk-``loop:answer` | Start or inspect a run from chat; the run itself still happens out of session |
| Plugin | `/rk-loop:schedule` (new) | Writes the host config for a scheduled run (a cron, an Actions workflow, a cloud routine) |
| Hook | SessionStart, one line | "Loop: 2 items waiting on you, 1 running", read from `loop.*` events and Weigh-In |

### Hands off, keeps and drops

| Today in redkiln-rs | Goes to | Why |
| --- | --- | --- |
| `backlog.rs`, `item_write.rs`, `.redkiln/items/*.toml`, the `Tracker` port fake | Backlog, through `backlog-contract` over `redkiln-github` | Issues are the backlog (decided). Two TOML items exist, so the cut is small |
| `.redkiln/kb/` (4 notes) and the records conventions (ADR-0085, 0092) | Knowledge | The four notes' destination is pending Knowledge call F (the catalog package). Loop keeps the need: the atom-set digest is part of the memo key |
| `rk-kernel/src/review/` (1.2k lines), ADR-0074 re-execute-before-blocking | Review's finding contract and verifier | One finding shape for the suite; Loop keeps the call site |
| Gate records and `questions.toml` options | Weigh-In decision requests | Blueprint call 7 |
| `rk-containment`, the generic half of `rk-adapter-claude` | `redkiln-dispatch` | assay's runner needs the same env denylist and isolation (F7, F8) |
| Headroom and throttle (ADR-0110) | `redkiln-capacity` | Capacity crosses machines |
| Facet provenance and the stability gate | `redkiln-calibrate` | Shared with Bench's Clef and Review |
| Catalog, predicate, lattice and select modules | `redkiln-catalog` kit, now | Loop is the first user and Bench the second (Blueprint call 8, amended to pressure-test rec 16) |
| Store opener (WAL, ADR-0105 clause 7) | `redkiln-core` | One opener for the suite |
| Lint and dependency policy (ADR-0083, 0084, 0100) | Redkiln Rust | One workspace policy |
| `rk-plugin-gen`, the `rk-` command prefix (AC-506), `.bklg` | Dropped | Suite IDs and a hand-written plugin replace them |
| 199 ADRs, AC and SM registers, the runbook | Kept as they are under `docs/loop/` | No renumbering (ADR-0092) |

Loop is not a dashboard. Parallel-agent boards on worktrees are commoditised, so Loop shows its state through Backlog's and Weigh-In's views and the suite mod, and builds none of its own.

## The run lifecycle

Every `redkiln run` is the same short, idempotent pass: claim, rebuild, check the world, do one stretch of work, record it, and exit. Nothing depends on the previous process surviving, which is already Loop's rule (F22, AC-198) and is what lets any host resume any run.

1. **Claim capacity.** Take a lease from `redkiln-capacity` (holder, run, reserved cost, expiry). Whenever the repository has a ledger repository, the lease is a ref there, created atomically and reaped only by compare-and-set (assay ADR-19). The local-log backend is only for client repositories or repositories with no ledger. No lease, no dispatch.
2. **Rebuild.** Reconcile git into the projection: trailers on `main` and `rk-run/<item>` are tier 1, and the store is a cache that `doctor --rebuild` can drop (ADR-0109's deleted-database test).
3. **Snapshot the item, every entry.** Read the issue through `backlog-contract`: number, `updated_at`, a content hash of the body, labels, dependencies and the author association. Record it as a backlog.snapshot event, compare it with the last snapshot and record drift. Loop's own spikes measured 7% drift on gates (F19), so a snapshot only at start is not enough.
4. **Check open gates.** For each open gate, read its answer from Weigh-In (local log or the rendezvous). An answer counts only at the trust level the gate requires.
5. **Select and dispatch.** The kernel picks the next activity from four digests (item, catalog, facets, atom set). It dispatches one `claude -p` child per activity in the run worktree, with a least-privilege card, a pinned model and a closed output schema (ADR-0197).
6. **Validate the output.** The child's typed output is a request, not a write. The kernel validates it against the schema and the card's grants, then makes the commit, comment or label change itself. This is GitHub Agentic Workflows' safe-output pattern, and Loop already does it for commits.
7. **Gate or continue.** Verify and review gates run inline. A human gate raises a Weigh-In decision request, writes a `Gate:` trailer and exits.
8. **Land.** Push `rk-run/<item>` and open a pull request that closes the issue. The kernel writes consolidated `Story:`, `Slice-Verdict:`, `Gate:` and `Weigh-In:` trailers into the PR's final commit message so a squash merge keeps them (pressure-test call D). In Phase 1, a landed commit is the kernel's content checkpoint on rk-run/\<item> (ADR-0188), and a person merges the pull request directly once its gates pass. The merge queue (AC-196) arrives in Phase 2 and then decides what lands.
9. **Checkpoint and release.** Export durable facts at Core's checkpoint (journal, never at SessionEnd), release the lease with its final cost, and exit.

### Gates, what they bind to and who may answer

| Gate | Binds to | Answer must be | Where it is answered |
| --- | --- | --- | --- |
| Verify | Tree hash of the run worktree (ADR-0105), the verify gate's declared hash method | Computed by Loop | Nowhere; it is a check |
| Review | Patch-id plus base, from a fresh `review.verdict` | Attested when unattended (the Review App's check run) | Review's CI deployment, or `/rk-review:check` locally when attended |
| Human, two-way | Its subject's hash plus the decision request's id: the file's sha256 for a plan, design or artifact approval, and the tree hash only for a gate on this snapshot | Attested when unattended; witnessed (on the board or in a sitting) suffices only for a run on the machine that holds the board | Weigh-In sitting, board, decision issue; may take its default at the deadline (wi-4d8b6a) |
| Human, one-way | Same | Same as two-way; never defaulted | Same; the run blocks until answered |
| Plan approval from Bench | The plan file's sha256 | Witnessed | Answers Loop's planning gate when Loop recomputes the same hash |

An asserted event (anything written through a CLI or an imported journal) never satisfies a gate. That rule is what stops an unattended agent from approving its own question (pressure-test call C). A client repository has no attested channel. So an unattended client run stops at any human gate that needs an attested answer, the review gate included, until someone answers locally.

### What Loop may never do

- Approve, merge or enable auto-merge on its own pull request, or push to any branch other than `rk-run/*`.
- Write a human-actor event. `redkiln-guard`'s shared rule pack denies the Bash call.
- Act on an issue a maintainer has not marked ready and mode/afk, or treat issue text as an instruction. Loop reads mode/afk to know which ready items it may run unattended. Bodies and comments are untrusted input, and the snapshot records who wrote them.
- Set `status/ready`, `mode/afk` or any `needs/*` label other than `needs/human`. Its permitted label writes are the `rk:*` labels Backlog reserves for it (AC-507), `needs/human` when it parks an item, `status/in-progress` when it claims one and `status/in-review` when it lands one.
- Read secrets. The run environment strips them, as the harness already strips provider variables (#11).

## Hosts, data and events

Loop treats every host as ephemeral, so choosing a host is deployment, not architecture. What a run must keep lives in git, GitHub and the ledger repository; its local store is a cache it can lose.

### Where Loop runs

| Host | How it starts | What persists | Fits | Order |
| --- | --- | --- | --- | --- |
| Your own machine, scheduled | `/rk-loop:schedule` writes a cron or Task Scheduler entry that calls `redkiln run --next` | Everything | The only host for client repositories; the first host | 1 |
| GitHub-hosted Actions | A scheduled workflow, or the `mode/afk` plus `status/ready` labels a person sets; plugins arrive through claude-code-action's inputs | Nothing; a fresh VM per job, 6-hour cap | Bounded items in own repositories | 2 |
| Claude Code cloud routine | A routine in a Projects thread, after the suite bootstrap installs the pinned plugins | Only what is pushed; push rights may be limited to the session branch (unverified) | Natural for Projects users | 3, after the spike |
| Self-hosted runner | Same workflow, persistent runner | The runner's disk | Own repositories with long items | When needed |
| Managed Agents | Not yet | Persistent filesystem | Plugins and hooks are not documented there | Not a fit today |

The run never assumes the host has the previous run's log. A question raised in a cloud container and answered on your laptop must reach the next run wherever it starts, which is why the rendezvous below exists.

### Data

| Data | Where | Authority |
| --- | --- | --- |
| Stage transitions, gates, verdicts, slice seals | Trailers on checkpoint commits on `rk-run/<item>`, consolidated onto the landing commit | Tier 1, authoritative (ADR-0109) |
| Run events (dispatch records, rate-limit readings, artifact reads, memo) | `loop.*` events in the shared log | Tier 2, rebuildable |
| Tier-1 mirror, memo table | `<git-common-dir>/redkiln/loop.projections.sqlite3` | Disposable projection (Blueprint call 1) |
| Item content | Issue snapshots as `backlog.snapshot` events, one per entry | GitHub is the source |
| Spec, design and slice artifacts | Committed Layout C files linked from the issue at a SHA | Pressure-test call E, still open |
| Leases and cost | Refs in the ledger repository whenever there is one; the local log only for client repositories or repositories with no ledger | Whichever backend holds the lease |
| Questions and answers | Weigh-In items, plus the rendezvous | Weigh-In |
| Catalog, packs, cards | The catalog data package, overridable in `.redkiln/catalog/` | Reviewed config |

### WAL or TRUNCATE

Loop's store asserts WAL today (`rk-store/src/conn.rs`, ADR-0105 clause 7), while Core opens the shared log under TRUNCATE for Cowork. The Blueprint's answer stands: Loop gives up WAL for the shared log if R-SQLITE passes with eight concurrent writers, hooks included, on Windows and Linux. If it fails, Loop keeps tier-2 events in its own WAL file and exports a journal into the log. Its projection file can stay WAL either way, because only Loop opens it and Loop never runs in Cowork.

### The rendezvous for unattended questions

A decision raised with nobody present is published where every host can read it and every answer carries an identity the agent cannot forge (pressure-test call B, recommendation 6).

| Repo class | Rendezvous | Answer is attested by |
| --- | --- | --- |
| Own repository with Backlog | A GitHub issue of type Decision, linked to the item, carrying the Weigh-In id | A comment or label from a maintainer, read with its author association |
| Own repository without Backlog | A ref in the ledger repository, written by the board | The board's deploy key, compare-and-set |
| Client repository | None. Loop runs client work only on the machine that holds the board | Witnessed locally |

Weigh-In still owns the item; the rendezvous is only how the question and answer travel. When the answer arrives, Weigh-In emits `weighin.item_decided`, Knowledge writes the atom, and the next `redkiln run` reads it at step 4.

### Liveness and escalation

Two things Loop lacks next to Gas Town (pressure-test rec 19). A **liveness watcher**: a lease past its expiry with no checkpoint marks the run stalled, posts a handoff comment on the issue and frees the lease. **Escalation tiers**, taken from anneal's round ladder: three same-card attempts, two at a higher model tier, then park the item with `needs/human` and a Weigh-In question. The merge queue is already designed (AC-196 to AC-198) and is Phase 2.

## Prior art and patterns

The unattended agents that ship today agree on one rule Loop should adopt whole: the agent proposes, a deterministic layer with its own identity writes, and a human the agent cannot impersonate merges. None of them pairs that with a gated process catalog, which is where Loop is ahead.

| Prior art | What it is | Pattern to take | What to leave |
| --- | --- | --- | --- |
| [GitHub Agentic Workflows, safe outputs](https://github.github.com/gh-aw/reference/safe-outputs/) | Markdown workflows compiled to Actions | The agent job runs read-only and emits structured requests; a separate job with write permission performs each one, under a `max` per output type (one issue, one comment, three labels by default) | Its workflow language; Loop's catalog is richer |
| [Copilot cloud agent](https://docs.github.com/en/copilot/how-tos/use-copilot-agents/coding-agent/review-copilot-prs) | GitHub's hosted coding agent | The requester's approval does not count toward required reviews, and workflows do not run on its pushes until a person approves them. Loop's PRs need a reviewer who is not Loop's identity | Nothing; these are platform rules worth copying |
| [claude-code-action](https://code.claude.com/docs/en/github-actions) | Claude Code in Actions | `plugin_marketplaces` and `plugins` inputs install the suite on a runner; a `schedule` trigger runs with no user check. Scheduled workflows run only from the default branch and, in public repos, stop after 60 idle days | The shared Claude App's broad permission set; Loop wants its own narrow App (call A) |
| [Gas Town](https://rywalker.com/research/gastown) (third-party profile) | Multi-agent orchestrator on beads | A Refinery owning a bisecting merge queue, a Witness that unsticks workers, monitors that alert. Rec 19's three missing roles | "Chaotic by design" work, its own store (beads), tmux |
| beads (from the pressure test) | Git-backed agent tracker | Agent-written data off code branches, in its own refs | Per-clone SQLite plus JSONL on a branch, which beads abandoned |
| [Temporal human-in-the-loop](https://docs.temporal.io/ai-cookbook/human-in-the-loop-python.md) | Durable workflows | A wait on a decision with a timeout and a defined outcome when time runs out; Weigh-In's deadline and default, as the gate's contract | The engine itself; ADR-0086 bans durable-execution frameworks, and git trailers already give durability |
| anneal `round` and `orchestrate` | In-session fan-out | Three same-agent rounds, two escalated, then park: the escalation ladder | In-session subagents as the runtime |
| assay ADR-19, ADR-23 | Cross-repo semaphore | Lease refs created atomically, reaped only by compare-and-set; cost `reserved` then `final` | Nothing; it is the capacity backend |
| Spec Kit, OpenSpec (pressure test) | Spec-driven prompts | OpenSpec's living spec: deltas archived into a current spec, checked against Loop's docs-update and knowledge-harvest | Their gate-free flow; Loop's catalog already exceeds them on forms, checklists, adversaries and gates |
| Mergify, State of Merge Queues 2026 (cited in Loop's design §7.7) | Merge-queue vendor data | A queue serialises landing; AI-assisted PRs broke main 1.9% of the time with a queue | Nothing to lift; it is the evidence for AC-196 |

Two patterns are already Loop's own and need no import: reconcile on every entry (F19, F22) and "reviewer proposes, machine confirms" (ADR-0074), which Review inherits as a verifier mode.

## How it composes with the rest of the suite

Loop is the suite's heaviest consumer and its lightest owner: it reads five domains and owns only execution. Each sibling is reached through its contract crate, its events or its `--json` CLI, so a missing sibling turns a Loop feature off rather than failing a run.

[embedded content: Loop's place in the suite · 6 siblings, 1 shared base]

The two-way arrows on the left are the only places Loop writes into another domain, and each write is a typed, validated output.

- **Backlog** supplies the ready queue and an issue snapshot on every entry. Loop writes only its permitted labels (`rk:*`, `needs/human`, `status/in-progress`, `status/in-review`), claim and handoff comments, and the PR that closes the issue. In client repositories, items come from Backlog's local mode (a file store under `.redkiln/backlog/` with the issue body contract's fields), which Loop reads through `backlog-contract` as Bench's `dag` does.
- **Weigh-In** receives every human gate as a decision request with door, options, default and deadline, and returns answers through the rendezvous. Without Weigh-In, `redkiln answer` is the fallback channel.
- **Knowledge** supplies the atoms each activity injects and the atom-set digest that enters the memo key; Loop's closeout and knowledge-harvest activities propose new atoms back.
- **Review** owns the findings schema and verifier. Loop's review gate calls it and needs a fresh `review.verdict`; ADR-0074's re-execute rule becomes Review's verifier mode.
- **Redkiln Rust** runs its gate on Rust items, bound to that gate's declared hash method: the change fingerprint (Rust pathspecs, as in temper 1.x), not the tree hash that Loop's verify gate uses. It also owns the lint and dependency policy Loop's ADR-0083, 0084 and 0100 hold today.
- **Bench** runs the same catalog package in chat. An item specified and planned in Bench arrives with a plan whose file hash answers Loop's planning gate, so Loop does not re-plan it.
- **Core and kits** carry the shared log, approval records, trust levels and journals, and the kits Loop leans on: `redkiln-dispatch` (its own harness, generalised), `redkiln-capacity`, `redkiln-catalog`, `redkiln-github` and `redkiln-guard`.

## Calls for Ryan and next steps

Six calls shape the plugin; each has a recommendation. Only A is one-way, because colleagues' and client repositories will install the identity Loop writes with.

| # | Call | Recommendation | Door |
| --- | --- | --- | --- |
| A | Loop writes to GitHub as its own narrow GitHub App (Contents, Issues, Pull requests; never a bypass actor or an approver), not with your token or the shared Claude App | Yes. It makes never-approves-its-own-PR a platform fact, and Loop's own writes stay asserted for its own gates | One-way once others install it |
| B | Host order: your machine first, then Actions for own repositories, then cloud routines only after the bootstrap and push spikes | Yes | Two-way |
| C | Phase 2 scope adds a liveness watcher and anneal's escalation ladder beside the planned merge queue, and rules out a Loop dashboard (rec 19) | Yes | Two-way |
| D | Reach M5 on redkiln-rs first (s8j, then one paid fix lane), then land `build/phase-0-to-1` (#24) and import it into the monorepo with history | Yes. Moving first would reopen every checkpoint the move touches | Two-way |
| E | A change to an issue's requirements or AC after `ready` voids the item's open human gates; any other edit is recorded as drift | Yes | Two-way |
| F | A hand-written plugin (`/``rk-loop:run, /rk-loop:status, /rk-loop:answer, /rk-``loop:schedule`) replaces `rk-plugin-gen` and the `rk-` prefix lint (AC-506) | Yes | Two-way, nothing is built |

Calls already open elsewhere decide parts of Loop and are not asked again here:

- Blueprint call 1: the store shape, and Loop off WAL.
- Blueprint call 7: gates become Weigh-In requests.
- Blueprint call 8, amended to pressure-test rec 16: the redkiln-catalog kit now.
- Pressure-test call B: the rendezvous.
- Pressure-test call C: trust levels in Core from v1.
- Pressure-test call D: squash landing with consolidated trailers.
- Pressure-test call E: where spec artifacts live.
- Backlog call A: the label set that reserves `rk:*`.
- Weigh-In call A: the decision request record, the one gate record shape for every plugin's human gate.

### Suggested next steps

- Ryan decides A to F.
- Approve the s8j build plan (`spikes/plans/s8j-build-plan-2026-10-16.md`) and give the go-ahead for the first paid fix lane run, which is M5.
- Push the corpus bundle to the empty private `redkiln-corpus` remote, so #32 has an off-machine copy git can fetch, not only a OneDrive file.
- Run R-SQLITE with eight concurrent writers, hooks included, on Windows and Linux.
- Spike pushing an `rk-run/*` branch and a ledger ref from an Actions job and a cloud routine.
- Write the `loop.*` event types, the issue-snapshot event and the decision-request shape into Core's SPEC draft.
- Transplant the original Redkiln templates into the catalog's missing checklists (redkiln-rs #79), so Bench and Loop share them from day one.
