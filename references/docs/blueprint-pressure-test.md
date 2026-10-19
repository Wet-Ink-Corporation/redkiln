<!-- Exported from the Claude Doc https://claude.ai/code/artifact/9bc8a145-4af0-4f8e-b05c-b3abe60f70ad (revision 37) on 2026-10-19 as part of the project handover. The Doc is the original; this copy is frozen. Diagrams (3) could not be exported and show as [embedded content] placeholders; open the Doc to see them. -->

# Redkiln Blueprint: Pressure Test

Oct 13, 2026 · @Ryan

## Summary

The broad brushstrokes hold: no prior art or run mode argues against a suite of thin plugins over Core, kits and contract crates, or against one event format. What the Blueprint lacks is a model of the suite spread over many machines. It describes one shared log, but every laptop clone, cloud container, CI job and Loop host has its own log, and they meet only in git, GitHub and the ledger repository.

Five changes follow, all in Core, Data and state, and Loop:

1. **Say what is shared.** The log is per-clone working memory; git, GitHub and the ledger repository are the shared record. Events carry their origin and are exported at one defined checkpoint.
2. **Say who can be believed.** Approvals are bound to content but not to identity. Every event gets a trust level, and gates in CI or unattended runs accept only attested ones.
3. **Give unattended questions a rendezvous.** A question Loop raises in a container cannot be answered on your laptop's board today. It needs to travel through a GitHub issue or the ledger.
4. **Keep agent-written journals off code branches.** beads took the Blueprint's storage path and backed out of it for this reason.
5. **Pin the suite.** Separately installable plugins need a per-repository version pin, release trains, and a bootstrap for cloud sessions, which do not install plugins from repository settings.

On coverage, the suite leads on enforcement, decisions and a gated spec-and-plan process, where competitors are advisory or prompt-driven. The risk there is not missing rigor but stranded rigor: most checklists Loop's spec and plan steps call are not written yet, Bench runs separate templates, and dropping .bklg leaves spec artifacts without a named home. Recommendations lists 21 changes, five new calls (A to E) and five spikes.

## Prior art: what others already do

No framework found does what the suite does as a whole: refuse bad code before it is written, settle agents' questions on a ledger, and run unattended under the same rules. Every piece exists somewhere, though, and one finding changes broad strokes. beads walked the storage path the Blueprint is on and backed out of it. An earlier version of this section called spec and plan the suite's thinnest area. That was wrong: Loop's process catalog is more rigorous than Spec Kit or OpenSpec. The real risk is that the suite design strands that rigor inside Loop (see Spec and plan, corrected).

### The frameworks that matter most

| Framework | What it is | What it means for Redkiln |
| --- | --- | --- |
| [beads](https://github.com/steveyegge/beads) and [Gas Town](https://github.com/steveyegge/gastown) | Agent issue tracker in git, and an orchestrator on top of it | beads began as SQLite per clone plus JSONL in git, the Blueprint's shape. It moved to Dolt with data under `refs/dolt/data`, off code branches, because SQLite merges badly and JSONL queries slowly. Along the way it dropped hidden worktrees that blocked branch switches, and found that clones migrating separately forked the database ([architecture](https://beads.gascity.com/architecture.md), [migrations across clones](https://blog.gascity.com/posts/beads-1-1-dolt-migrations-with-clones/)). Gas Town adds what Loop lacks: a bisecting batch merge queue, a liveness watcher, severity-tiered escalation |
| [git-bug](https://github.com/git-bug/git-bug) and [git-appraise](https://github.com/google/git-appraise) | Bugs and reviews stored in git refs | The textbook model for agent-written data: append-only operations under their own refs, Lamport clocks for order, ids from content, deterministic merge across clones ([data model](https://raw.githubusercontent.com/git-bug/git-bug/master/doc/design/data-model.md)). git notes, the obvious alternative, are lost on squash and rebase |
| [GitHub Agentic Workflows](https://github.github.com/gh-aw/) | Markdown workflows compiled to Actions | The closest thing to Loop. The agent gets read-only tokens and no secrets; every write is a "safe output" a deterministic job validates. Cost caps and OpenTelemetry traces are built in |
| [Copilot cloud agent](https://docs.github.com/en/copilot/concepts/agents/coding-agent/risks-and-mitigations) | GitHub's hosted coding agent | Pushes only to its own branches, cannot approve or merge its own PR, and workflows wait for a human on its pushes. These are the guardrails a hosted Loop needs |
| [Spec Kit](https://github.com/github/spec-kit), [OpenSpec](https://github.com/Fission-AI/OpenSpec), [Kiro](https://kiro.dev/docs/specs/) | Spec-driven development | Prompt-driven spec workflows with few enforced gates; Loop's catalog already exceeds them on structure and gates (below). OpenSpec's change folder of spec deltas, archived into a living spec, is the one idea worth checking against Loop's docs-update and knowledge-harvest activities |
| [superpowers](https://github.com/obra/superpowers), [wshobson/agents](https://github.com/wshobson/agents) | Large Claude Code skill and plugin collections | Enforcement there is persuasion, which decays after compaction; hooks are the fix, so the suite's mechanical gates are its differentiator. wshobson ships granular plugins from one source tree to seven harnesses, a working reference for the monorepo shape |
| [Claude Code Review](https://code.claude.com/docs/en/code-review), [Bugbot](https://cursor.com/docs/bugbot), [CodeRabbit](https://docs.coderabbit.ai/pr-reviews/pre-merge-checks) | Hosted AI review | Converging on a neutral check plus machine-readable severity, warning before error, logged overrides, an author who cannot override their own check, and per-rule acceptance analytics. `REVIEW.md` is becoming a shared convention |
| [Renovate's dashboard](https://docs.renovatebot.com/key-concepts/dashboard/) | A GitHub issue as a control panel | Proof that an issue with checkboxes works as an approval board, on mobile, with nothing to host. A precedent for Weigh-In's rendezvous |
| [OpenHands events](https://docs.openhands.dev/sdk/arch/events), [CloudEvents](https://github.com/cloudevents/spec/blob/main/cloudevents/spec.md), [OTel GenAI](https://opentelemetry.io/docs/specs/semconv/gen-ai/) | Event logs and telemetry standards | OpenHands uses an append-only log as memory and integration point, as Core does, and types each event's source. CloudEvents makes `source` plus `id` unique for deduplication across producers. OTel's agent spans are still in development |
| [MADR](https://adr.github.io/madr/), [Temporal](https://docs.temporal.io/develop/python/message-passing), [ACP](https://agentclientprotocol.com/protocol/tool-calls) | Decision records, durable waits, agent permissions | Three ideas for Weigh-In: MADR's Confirmation field (which check enforces this decision); Temporal's validated update with a timeout and default; ACP's once-or-always outcomes, so a decision can become standing policy |
| [HumanLayer](https://www.humanlayer.dev/) | Human approval for agents | Its per-tool-call approval SDK is deprecated; plan- and artifact-level checkpoints survived. That supports Weigh-In's batched sitting over interruptions |

Parallel-agent dashboards on worktrees are commoditised (Claude agent teams, GitHub's Agent HQ, Conductor), and vibe-kanban shut down for lack of a business model ([notice](https://www.vibekanban.com/blog/shutdown)). Loop should not compete on that surface.

### Does the suite cover everything?

The suite is strong where others are weak (enforcement, decisions, a gated spec-and-plan process) and thin where others are strong (triage, learning from feedback).

| SDLC stage | Covered by | Gap | Who does it well |
| --- | --- | --- | --- |
| Intake and triage | Backlog (second wave), Weigh-In deferrals | Nothing grooms or triages the backlog | Spec Kit's assess gate, Renovate-style dashboards |
| Spec and plan | Loop's catalog (ranked spec forms, design, slicing to a dependency DAG); Bench (grill, EARS spec, plan approved by hash) | Most checklists the catalog's steps call are not written yet; Bench and Loop keep separate templates; no home is named for spec artifacts once .bklg goes | Redkiln's own catalog, once ported; Spec Kit and OpenSpec are mostly prompts |
| Implement | Rust's implementer, Loop | None at the architecture level |  |
| Standards enforcement | Rust; redkiln-craft for later languages | Rust only for now | Nobody else refuses before write; Semgrep Guardian is advisory |
| Verify | Rust's gate, Bench's evidence | Nothing verifies merged stacks | Gas Town's bisecting merge queue |
| Review | Rust's reviewer, Review | No feedback loop, no `REVIEW.md` interop | Bugbot, CodeRabbit, Claude Code Review |
| Decisions | Weigh-In, Loop's gates | Decisions do not become enforced policy | Nobody closes this loop yet |
| Orchestration | Loop, capacity leases | Merge queue, liveness watcher, escalation tiers | Gas Town, gh-aw |
| Knowledge | Knowledge | No learned rules, no per-rule outcome data | Bugbot learned rules, Tessl's skill evals |
| Observability and cost | Core events, capacity | No standard export, no budgets per item | gh-aw cost caps with OTel |
| Security | Rust's dependency rules, Review's sanitiser | No prompt-injection or secret-scanning layer for Loop | Copilot cloud agent defaults, gh-aw threat detection |
| Release | Bench's frozen commands (Ship later) | Deferred, as decided |  |
| Onboarding | Rust's setup and baseline | Standards are not discovered from existing code | Agent OS v3 |

The gaps that touch architecture now are keeping the catalog's rigor available outside Loop, decisions becoming policy, and Loop's least-privilege writes. The others can wait for Ship, Backlog or a later wave, but the Blueprint's "Later" list should name triage so it does not fill piecemeal.

### Spec and plan, corrected

The suite's spec-and-plan process is its most rigorous part, not its weakest, and the earlier rows missed it because they counted Bench and Redkiln Rust but left out Loop's catalog. What the suite design does lose is reach. As drawn, that rigor runs only when Loop runs, and part of it has not been ported yet.

On `build/phase-0-to-1`, the catalog has 102 activities over 12 stages. Specification picks one form from a ranked family: user story with acceptance criteria, then EARS structured requirements, then BDD scenarios, then use case, then model-based (`.redkiln/kb/spec-conventions.md`). Design adds a design doc, ADR, C4 views or a threat model when the item's facets call for them (`.redkiln/packs/story.toml`). Planning slices the spec into vertical slices with a checked-acyclic dependency DAG that parallel worktrees consume (`.redkiln/catalog/vertical-slicing.toml`). Each activity drafts, self-checks against a checklist, runs an adversarial reviewer when rigor is at least recommended, loops on a policy check, and stops at a human gate.

|  | Loop's catalog | Spec Kit | OpenSpec |
| --- | --- | --- | --- |
| Spec form | Chosen per item from a ranked family | One template | Proposal plus delta specs |
| Design artifacts | Required by facets (size, novelty, external dependency) | `plan.md` | `design.md` |
| Decomposition | Vertical slices plus an acyclic dependency DAG, checked | `tasks.md` | `tasks.md` |
| Quality checks | Checklist self-check, adversarial review, policy loop | Optional clarify, checklist and analyze steps | None by design ("no phase gates") |
| Human gates | Per step, bound to a tree hash | A `gate` step in its workflow engine | None |
| Living spec | Not modelled; docs-update and knowledge-harvest come closest | No | Yes, deltas archived into `openspec/specs/` |

Loop's own research records the lineage: it measured Spec Kit and OpenSpec as prompt-only and kept their structure while adding weighted gates (`references/2026-08-31-rust-inversion-RFC.md`, `references/2026-09-03-reimagining-brainstorm.md`).

Three places where rigor is at risk in the suite design:

1. **The checklists and conventions are mostly unwritten.** The catalog's steps name 39 checklists and convention files; 11 exist. Missing are the ones spec and plan lean on most: `ears-requirements`, `ac-quality`, `ac-coverage`, `design-doc`, `adr`, `c4`, `slicing-heuristics` and `architecture-conventions`, among 28. The registry declares them (`spikes/p9/registry-v2.toml`), so this is porting work, not lost design. The original Redkiln repository holds sources for about half of them: 26 artifact templates, four agent prompts, and the EARS and design rules inside its workflow prompts. The other half (C4, security, use cases, UX flow, release and incident work) have no earlier source and need new writing. The transplant is filed as redkiln-rs issue #79.
2. **Interactive work runs different templates.** Bench brings anneal's grill, EARS spec and plan skills with their own `spec.template.md` and `plan.template.md`. The Blueprint's call 8 only checks that Bench's skills name catalog activities. An item specified in chat therefore gets neither the ranked forms, the checklists nor the adversary that the same item would get under Loop.
3. **Spec artifacts lose their home.** Loop's design kept spec and plan bodies as files in the repository, with the issue carrying only a link (`references/2026-09-03-reimagining-brainstorm.md`). The Blueprint drops `.bklg` and in-repo items and makes GitHub Issues the backlog, without saying where a Spec, DesignDoc or SliceSet now lives.

Recommendation: make the catalog's content (activities, checklists, conventions, templates, artifact kinds) one suite package that both Bench and Loop run from. Bench's skills render the same activity and checklist in chat; Loop runs it unattended. The engine that loads, lints, selects and renders activities (today Loop's rk-kernel catalog, predicate, lattice and select modules) moves into the redkiln-catalog kit crate now, because Bench becomes its second user. The crate stays synchronous for Loop's kernel, and Bench's skills call it through a CLI such as bench activity show and bench activity check. The content ships as a versioned data package with the suite train, and a repository can override parts of it in .redkiln/catalog. Name a committed home for spec artifacts in Layout C, linked from the issue (call E).

## Run modes: where the suite runs and how facts move

The suite runs in five places, and they share nothing but git, GitHub and the ledger repository. Three things the Blueprint leaves open follow from that: where an unattended run's question is answered, when local facts are exported, and how plugins reach a cloud session.

| Mode | Who is there | How plugins arrive | Event log | How facts leave | Where you answer |
| --- | --- | --- | --- | --- | --- |
| Local chat (CLI, desktop app, Remote Control) | You, live | Marketplace install, user or project scope | Lasts as long as the clone | Committed journal in own repositories; nothing for client ones | In chat, or on the board |
| Cowork | You, live | claude.ai org sync | `<folder>/.redkiln/`, TRUNCATE | Nothing leaves | In chat. Hooks run in a scratch workspace and edits go through the device shell (weigh-in D-0002, B-04). Whether `libexec/` binaries run there is undocumented |
| Cloud session (web, mobile, Projects threads) | You, now and then | Not from repository settings: a repository's `enabledPlugins` are not installed in cloud sessions ([cloud environments](https://code.claude.com/docs/en/cloud-environments.md)). Today each plugin wires itself in through its own SessionStart script (weigh-in B-12) | Dies when the VM is reclaimed | Journal committed and pushed to the session's branch | In chat, or on a Projects decision card; Projects threads have no `AskUserQuestion` (weigh-in #126) |
| CI (GitHub Actions) | Nobody | Review and the Rust CLI by digest; `claude-code-action` can install plugins through its inputs ([GitHub Actions](https://code.claude.com/docs/en/github-actions.md)) | One per job, gone afterwards | Check runs, PR comments, artifacts, the ledger repository | A GitHub review or comment |
| Hosted Loop | Nobody, for hours | Depends on the host (table below) | Depends on the host | Trailers, pull requests, the ledger repository | Undefined today. This is the main gap |

[embedded content: data topology · where the suite runs (Cowork sits on your laptop) and the one shared record]

Within a session, plugins meet through the log, hook context and the end-of-turn grammar; between worktrees of one clone, through the log; between machines, only through the shared record.

### Gap 1: an unattended run has nowhere to receive your answer

Weigh-In's log and board are per clone. If Loop runs in a cloud container and you settle its question on your laptop's board, the answer lands in your laptop's log, which Loop never reads. The Blueprint's "Loop's human gates become Weigh-In decision requests" (call 7) is right, but it needs a rendezvous.

Recommendation: a decision raised where no human is present is published to the shared record and answered there. In Backlog-enabled repositories that is a GitHub issue of a "decision" type, so the answer is a comment or a label from an identified person (attested, see Trust). In other own repositories it is a ref in the ledger repository written by the board. Client repositories have no shared record, so Loop runs on client work only on the machine that holds the board. Weigh-In stays the owner of the item; the rendezvous is only where the question and answer travel.

[embedded content: a human gate in an unattended run · five steps across three lanes]

The answer is stored where every host can read it and with an identity the agent cannot forge; the run that asked may not be the run that resumes.

### Gap 2: export has no defined moment

Today weigh-in exports from a cloud-only hook after every Bash call, temper pushes a telemetry branch, and anneal has a journal command no hook calls. SessionEnd cannot carry it: SessionEnd hooks share a 1.5-second budget.

Recommendation: Core defines one checkpoint. It runs at Stop and from a pre-push git hook, is idempotent, and writes only durable facts to `.redkiln/journal/<session>.jsonl`. In client repositories it writes nothing. Whether a cloud session may push anything other than its own branch (a telemetry branch, a ledger ref) is unverified; spike it before relying on it.

### Gap 3: plugins do not reach cloud sessions by themselves

The Blueprint's distribution covers marketplace installs and org sync. Cloud sessions get neither from repository settings. Recommendation: release one suite bootstrap with each train. A repository's SessionStart hook or the cloud environment's setup script runs it, and it installs the versions pinned in `.redkiln/suite.toml`, through `claude plugin install` or a plugin directory named in `CLAUDE_CODE_PLUGIN_DIRS` (untested). It replaces each plugin's own session-start script.

### Where Loop can be hosted

| Host | Plugins and hooks | State between runs | Limits | Fit |
| --- | --- | --- | --- | --- |
| Your own machine, scheduled | All | Persists | Machine must be on | The only host for client work; the first host |
| Self-hosted GitHub runner | All, through the `claude` CLI | Persists on a persistent runner | You secure the runner | Own repositories |
| GitHub-hosted Actions, scheduled or on a label | Through `claude-code-action` plugin inputs | None; a fresh VM each run | 6-hour job limit; Actions minutes | Bounded items |
| Claude Code cloud routine | Only after the bootstrap runs | Only what is pushed | Push rights may be limited to the session branch (verify) | Natural for Projects users |
| Managed Agents | Skills and MCP; Claude Code plugins and hooks are not documented ([overview](https://platform.claude.com/docs/en/managed-agents/overview)) | Persistent filesystem | Beta | Not a fit until plugins run there |

Recommendation: Loop treats every host as ephemeral. Each entry rebuilds its state from git, GitHub and the ledger repository, which Loop's tier-1 rule already intends, and its local store is only a cache. Host choice then becomes deployment, not architecture. Start on your own machine, then add Actions or cloud routines for own repositories.

## Source control: branches, worktrees, clones

The shared event log is per clone, not per repository, while everything git carries is per branch. The Blueprint reads as if the log were the repository's memory. In practice every laptop checkout, CI runner and cloud container has its own log, and only what is committed or pushed crosses between them.

Proposed rule for the Blueprint's Data and state section: **the local log is working memory; git, GitHub and the ledger repository are the shared record. Any fact that matters beyond this clone is exported before the clone can disappear, and a reader never assumes another clone's log.**

### Scenarios

| Scenario | What happens under the Blueprint | Recommendation |
| --- | --- | --- |
| Linked worktrees (`claude --worktree`, Loop's run worktrees, parallel sessions) | All share `--git-common-dir`, so all share one log. This is the case the design handles best. But per-worktree state (Bench phase, handoff, session gates) shares the file too, and the `worktree` tag has two spellings today (anneal's lower-cased full path versus weigh-in's) | Core defines one normalised worktree id. Every per-worktree projection keys on it. Run R-SQLITE with at least eight concurrent writers, hooks included, not two |
| Worktree folders inside the checkout (`.redkiln/worktrees/`, `.claude/worktrees/`) | Cargo, ripgrep, Redkiln Rust's scanner and Review's `crates/**` trust path can walk into another worktree's files | Core names the worktree roots; the scanner, the gate and Review exclude them; a conformance test proves it |
| A second clone (another laptop, a colleague, a CI runner, a cloud container) | Separate log. A decision settled in one clone is invisible in the other until its atom or journal is pushed | The replication model under Run modes |
| Branch switch or abandoned branch | Log facts are branch-agnostic; atoms and journals are branch-scoped. A decision settled on an abandoned branch is decided in the log but its atom never reaches `main` | Knowledge treats committed atoms at `HEAD` as truth. Log-only decisions show as "settled, not yet on this branch" |
| Squash merge (anneal and assay squash today) | Per-commit trailers (`Story:`, `Gate:`, `Weigh-In:`) are folded into one message. GitHub's default puts them mid-body, where git's trailer parser does not find them. Loop treats trailers as tier-1 authority | Authority on `main` must not depend on branch commits surviving. Loop's landing step writes consolidated trailers into the final commit, or the merge policy per repo class is merge-commit only. Add a squash-merge test to the conformance kit |
| Rebase or force-push | Commit SHAs change, so events that cite them dangle. A tree hash changes whenever the base moved. A patch-id survives a clean rebase | Pick a hash method by what the approval should survive (table below). Projections tolerate SHAs that no longer exist |
| Cherry-pick or backport | Trailers and committed journals are duplicated | Journal import is idempotent by event id (already planned). Loop's reconcile tolerates duplicate `Story:` trailers |
| Shallow clone (`actions/checkout` defaults to depth 1; weigh-in's own checkout here is 50 deep) | Trailer reconcile, merge-base, patch-id against base and "what changed since" all need history | Core's git helpers detect a shallow clone and deepen, or report degraded. They never compute silently against a truncated history |
| CI on a pull request (detached `HEAD` at `refs/pull/N/merge`) | No branch name, and `HEAD` is a synthetic merge commit nobody else will ever see | The envelope records ref and PR number separately. Verdicts bind to the PR head and base, not to the merge commit |
| Fork pull request | An outside author can commit a journal or a `.kb` atom that says a decision was taken or a plan approved | Imported events never satisfy a gate. See Trust under Other stress points |
| Submodules and multi-repo work | Each submodule has its own common dir, so its own log. Work across repositories (this project spans six) has no shared log | A user-scope registry of known stores in `~/.redkiln/` serves "open across repos" views. Cross-repo work items live in Backlog |
| Re-clone, `rm -rf`, container reclaimed | The log is gone, and with it anything not yet exported | An export checkpoint at end of turn and before push (under Run modes) |
| Folder without git (Cowork, client folders) | `<folder>/.redkiln/` under TRUNCATE, nothing to commit | As designed. Repo class defaults to client when no config says otherwise |
| Merging two branches' `.kb` | Atoms and journals are one file each, so they do not conflict. A committed generated index would. `wi-` ids are six hex digits, and the collision check runs only inside one ledger: two clones can mint the same id, with roughly a 3% chance by 1,000 items | Never commit generated indexes. Knowledge's `validate` checks id uniqueness over the merged `.kb` in CI |

### Where committed suite data should live

Two kinds of suite data go to git, and they want different homes. Decisions, atoms and config are reviewed with the change, so they belong on the code branch. Journals are not reviewed: on a code branch they ride abandoned branches, swell pull requests, and arrive from forks unchecked. git-bug and beads both moved such data into refs of its own, off code branches.

Recommendation: journals go to append-only refs, one per store (`refs/redkiln/journal/<store-id>`, one file per session), fetched with an explicit refspec; the code branch carries only atoms and config. This is call A under Recommendations. It needs a spike first, because cloud sessions may be limited to pushing their own branch. The fallback is today's committed journal files.

### Which hash an approval binds to

The Blueprint's approval record names a hash method, which is right. It should also say which method each kind of approval uses, because the methods survive different git operations.

| Hash method | Survives a rebase onto a moved base | Survives a squash merge | Fits |
| --- | --- | --- | --- |
| Commit SHA | No | No | Pinning exactly what CI ran |
| Tree hash (Loop ADR-0105) | No | No | "This exact snapshot passed the gate" |
| Change fingerprint over Rust pathspecs (temper 1.x, bit-identical) | Only when the base changed no Rust file | Only when the base changed no Rust file | Redkiln Rust's gate: "this Rust change passed" |
| Patch-id of the diff against the merge base (anneal) | Yes, when the rebase is clean | Yes | "This change was reviewed and approved" |
| File sha256 | Yes | Yes | A plan, spec or brief that was approved |

Recommendation: a gate result binds to its gate's declared hash method (the change fingerprint for Redkiln Rust's gate, the tree hash for Loop's verify gate), a review verdict to the patch-id plus its base, and a plan or design approval to the file's hash. Merge queues and auto-rebase then re-run gates without voiding human approvals.

## Other stress points

The largest gap is trust: approvals are bound to content, but nothing yet says who can be believed to have given them. The rest are smaller and mostly need a rule written down before code depends on them.

### Trust: who said so

An event's `actor` (`human:ryan`, `agent`, `hook`) is text the writer asserts. Anything that can run the binary can write it: the agent through Bash, a hook, or a contributor who commits a journal or a `.kb` atom. Loop's unattended agent runs with the same rights as the hooks, so nothing stops it recording its own approval. Fork pull requests can carry forged journals into CI.

Recommendation: Core defines three trust levels on every event, and each gate states the level it needs.

| Level | How it is produced | May satisfy |
| --- | --- | --- |
| Asserted | Any write through the CLI or a hook, and every imported journal line | Nothing that gates; context and telemetry only |
| Witnessed | Taken through a surface the agent cannot drive: an `AskUserQuestion` answer, a board submission, a Projects decision card, a GitHub review by a named user | Local gates: Bench's plan approval, Weigh-In one-way items in a chat |
| Attested | Signed or written by an identity the PR author does not hold: the Review GitHub App's check run, a compare-and-set on the ledger repository with its deploy key, a signed commit | Merge gates in CI, Loop's human gates when it runs unattended. Exception: on the machine that holds the board, a witnessed answer given on the board or in a sitting suffices |

Two smaller rules follow. redkiln-guard's shared rule pack denies an agent's Bash call that writes a human-actor event (for example a `decide` subcommand). And Backlog snapshots record each issue's author association, so Loop acts only on items a maintainer marked ready, extending Loop's orchestrator-only labels (AC-507). Issue bodies are untrusted text that becomes a prompt.

### Version skew

"Separately installable" means any mix of versions meets in one session, and the CI digests can differ from both. Claude Code also updates plugins in the background mid-session, while running hooks keep the old plugin root until `/reload-plugins` ([plugins: loading](https://code.claude.com/docs/en/plugins/loading.md)).

Recommendation: a repository pins its suite in `.redkiln/suite.toml`, the way `rust-toolchain.toml` pins a toolchain. SessionStart compares installed plugins with the pins and warns once; CI fetches the pinned digests. Release the bundle as a tested set of versions, a release train such as `redkiln 2026.11`. Claude Code already supports this: `plugin.json` takes a `dependencies` array with version ranges, which resolve against git tags named `<plugin>--v<version>` ([plugin dependencies](https://code.claude.com/docs/en/plugins/dependencies.md)). The Blueprint's tag scheme (`rust-v2.0.0`) should follow that spelling instead.

### Hooks from several plugins at once

The Blueprint's "no hook waits on a sibling" rule matches how Claude Code runs hooks: all matching hooks run in parallel, a PreToolUse deny beats ask beats allow, and every hook's added context is kept ([hooks guide](https://code.claude.com/docs/en/hooks-guide.md)). Three consequences are not yet in the design:

- **One shared Stop budget.** Claude Code overrides Stop hooks after eight blocks in a row with no tool call between. Rust's gate, Bench's completeness check and Weigh-In's capture draw on that one allowance, so a noisy plugin can spend the budget another needs. Each plugin caps its own blocks well under eight and reads `stop_hook_active`.
- **SessionEnd is not a checkpoint.** SessionEnd hooks share a 1.5-second budget by default ([hooks reference](https://code.claude.com/docs/en/hooks.md)). Journal export belongs at Stop and before push, not at session end.
- **Latency adds up.** With five plugins, every Bash call starts several launchers and binaries in parallel, and every write contends for one SQLite file. Add a per-hook latency budget (a p95 target measured on Windows, the slowest platform for process start) to the conformance kit.

One more fact matters for store resolution: `CLAUDE_PROJECT_DIR` stays at the original root when a session moves into a worktree, while the hook input's `cwd` follows it ([worktrees](https://code.claude.com/docs/en/worktrees.md)). Core's resolver must start from `cwd`.

### Event schemas over time

Core owns the table layout and one `user_version`, which covers the file. It does not yet cover payloads: `bench.plan_approved` will change shape, and a reader built on an older contract crate will meet the new shape. Add a version to each event type, allow only additive fields within a version, put upcasters in the contract crates, and keep a golden fixture for every version ever written.

### Order and time

Once journals from several clones import into one log, wall-clock timestamps from different machines no longer order events. Order by origin store and sequence, and let an event cite the event it answers (a decision cites the item, a verdict cites the gate run).

### Volume, retention and privacy

Telemetry from every tool call will dwarf the facts. Split event kinds into durable facts and telemetry. Telemetry gets a retention window, is never committed, and leaves the machine only as aggregates. Journals carry durable facts only. Repo class defaults to client when `.redkiln/suite.toml` is missing, so a forgotten config never publishes.

### Enforcement outside Claude Code

Hooks bind only Claude Code. People, Copilot and Codex commit to the same repositories. Make the layers explicit: hooks give fast feedback as code is written, an optional pre-commit hook covers people, and the CI required check is the authority. The Blueprint implies this through Review; it should say it.

### Several people

Today there is one owner. Once colleagues join, Weigh-In needs to know who may decide what. Map decision authority in `.redkiln/suite.toml`, CODEOWNERS-style, keyed to GitHub identities.

### Testing the combinations

Seven plugins make 127 non-empty install sets, on five surfaces. Test each plugin alone, all of them together (already planned), and the pairs that share a domain (Bench with Rust, Bench with Weigh-In, Loop with Review). The property to prove is that adding an unrelated sibling never changes a plugin's behaviour beyond what its SPEC documents.

## Recommendations

The package boundaries stand. The changes land in Core, Data and state, and Loop, and five of them are new calls for you.

[embedded content: the suite with this review's changes · five layers]

The new parts sit in Core and the shared record; no plugin gains a dependency on another.

### Changes to fold into the Blueprint

"Before the plan" means the migration plan depends on it; "later" can follow the first wave.

| # | Change | Blueprint section | When |
| --- | --- | --- | --- |
| 1 | State the replication model: the log is per-clone working memory; git, GitHub and the ledger repository are the shared record. Every event carries its origin store and a per-store sequence, unique together as in CloudEvents, and cites the event it answers | Design rules; Data and state | Before the plan |
| 2 | Journals move off code branches to append-only refs, one per store (call A) | Data and state | Before the plan, after a spike |
| 3 | One export checkpoint at Stop and pre-push, idempotent, durable facts only; never at SessionEnd | Data and state | Before the plan |
| 4 | Every event carries a trust level (asserted, witnessed, attested); every gate states the level it needs; redkiln-guard denies agents writing human-actor events (call C) | Design rules; Approvals | Before the plan |
| 5 | Each approval kind names its hash method: gate to its declared method (fingerprint or tree hash), verdict to patch-id plus base, plan to file hash | Approvals | Before the plan |
| 6 | Unattended decisions travel through a rendezvous in the shared record: a decision issue in Backlog repositories, a ledger ref otherwise, the same machine for client work (call B) | Decision requests; Weigh-In; call 7 | Before the plan |
| 7 | Loop treats every host as ephemeral, writes only through validated outputs, never approves or merges its own PR, and its landing writes consolidated trailers so squash merges keep them (call D) | Loop | Before the plan |
| 8 | `.redkiln/suite.toml` pins the suite; the bundle ships as a tested release train; plugins declare `dependencies` with ranges; tags follow Claude Code's `<plugin>--v<version>` spelling | Distribution | Before the plan |
| 9 | One cloud bootstrap per train replaces each plugin's session-start script | Distribution | Before the plan |
| 10 | Core's resolver starts from the hook's `cwd`, normalises the worktree id, names worktree roots for the scanners to skip, detects shallow clones, and records PR ref and number separately | Redkiln Core | Before the plan |
| 11 | Repo class defaults to client when config is missing | Design rules | Before the plan |
| 12 | The conformance kit adds R-SQLITE with eight writers, a squash-merge test, a per-hook latency budget, each plugin's Stop blocks capped under the shared limit, and pairwise install sets | Redkiln Core | Before the plan |
| 13 | Event types carry their own version, with upcasters in contract crates; durable facts and telemetry are split, and telemetry has a retention window | Data and state | Before the plan |
| 14 | Enforcement has three layers, named: hooks for feedback, an optional pre-commit hook for people, the CI required check as the authority | Design rules | Before the plan |
| 15 | Decisions can become policy: Weigh-In outcomes are once or always, and atoms get a Confirmation field naming the check that enforces them | Weigh-In; Knowledge | Later |
| 16 | The catalog's content (activities, checklists, conventions, templates) becomes one versioned data package, in a format set by Core's SPEC, that Bench and Loop both run from. The engine that loads, lints, selects and renders it becomes the redkiln-catalog kit crate now, not later, and Bench reaches it through a CLI its skills call. This revises Blueprint call 8, which kept the engine in Loop; the 28 missing checklists and conventions are transplanted from the original Redkiln templates or written new before the migration (redkiln-rs #79); "Later" names triage | Bench; Loop; The process catalog; Later | Before the plan |
| 17 | Rule ids record fired, overridden and accepted counts; plugin evals become a release gate | Redkiln Rust | Later |
| 18 | Review emits SARIF and generates a `REVIEW.md` from the rubric, so other reviewers apply the same standard | Review | Later |
| 19 | Loop's scope names a merge queue, a liveness watcher and escalation tiers, and rules out a dashboard | Loop | Later |
| 20 | Decision authority per person, mapped in `.redkiln/suite.toml` | Weigh-In | When colleagues join |
| 21 | Spec, DesignDoc and SliceSet artifacts get a named committed home in Layout C, linked from their issue (call E) | Redkiln Loop; Redkiln Backlog; Data and state | Before the plan |

### Calls this review adds

These continue the Blueprint's numbered calls. Pick a value in the last column or comment on the row.

| Call | Question | Recommendation | Door | Your call |
| --- | --- | --- | --- | --- |
| A | Where do journals live in own repositories: committed files on the code branch, or append-only refs (`refs/redkiln/journal/<store>`) outside it? | Refs, if a spike shows cloud sessions and Actions can push them; otherwise committed files as today | One-way once adopters' history holds either |  |
| B | Where does an unattended run's decision request go: a GitHub issue of a decision type, or the board's own store? | A Decision issue (Backlog's Decision issue type, which the blocked item links through needs/decision) in Backlog repositories, a ledger ref elsewhere, local only for client work | Two-way |  |
| C | Should trust levels be part of Core's event format from the first release? | Yes; adding them later means every existing event is "asserted" forever | One-way once events are written |  |
| D | How do Loop-landed changes reach `main`: squash with consolidated trailers, or merge commits only? | Squash with Loop writing the trailers into the final message, so repositories keep their current merge settings | Two-way |  |
| E | Where do spec and plan artifacts live once .bklg is dropped and issues are the backlog: committed files linked from the issue, or the issue body? | Committed Layout C files per item (for example .redkiln/work/\<item>/spec.md), owned by the catalog package; the issue links to them at a SHA, as Loop's design intended | One-way once artifacts accumulate in either place |  |

### Spikes before the migration plan

- [ ] Push `refs/redkiln/*` from a cloud session and from a GitHub-hosted Actions job (decides call A)
- [ ] Install the suite in a cloud session through a SessionStart bootstrap, at pinned versions
- [ ] Run R-SQLITE with eight concurrent writers, hooks included, under TRUNCATE on Windows and Linux
- [ ] Org sync with several plugins and dependencies in one repository (the Blueprint's call 11)
- [ ] Check whether `libexec/` binaries run in Cowork, which the platform docs do not cover
