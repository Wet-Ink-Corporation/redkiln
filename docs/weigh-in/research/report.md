# Settle agent questions while the context is warm

No team or tool keeps what Ryan is asking for: a ledger of the open questions and follow-up decisions that agents leave behind, where each item has a status, grounding, and a path into the repo's decision record. Today the field keeps per-thread "waiting on you" inboxes or writes prose handoffs. The nearest pieces each cover one slice: spec-kit caps and ranks clarifying questions, compound-engineering never re-asks a settled decision and escalates only authority-bound calls, Anthropic's project-artifact page has "Risks & open questions" and "Decisions" tabs, and Seeds requires a reason for every state change. Evidence from September 2026 points Weigh-In to a specific shape. **Capture** comes from agent-emitted markers plus command hooks that cost no model tokens. **Filter** hard before anything reaches Ryan: is it settled, recoverable, authority-bound, a one-way door, and does it pass a decide-now test and an evasion check? **Present** at most four cards in one native question call at the end of the chat that raised them. **Keep a per-repo board** as a published Artifact with a `db` store, which is the only surface that both Claude Code and Cowork can write to and read answers back from. The ledger is per-session, append-only JSONL under `.kb/_ledger/`, folded across worktrees by a script. Decided items are rendered as `decision` and `open_question` atoms in `.kb/_intake/` for kb-ingest to promote. Redkiln already uses this partitioned-JSONL pattern for telemetry, so the later native `decision` item type is a mapping, not a rewrite. Running the sitting at chat end is also the cheap option. On Opus 5.5, each extra request in a 150k-token chat costs about **$0.03 in cache reads**, and a cold cache re-processes the chat for about **$0.60**. The design therefore budgets **three requests and about 8k fresh main-thread tokens per sitting** (estimates), roughly 8% of the session's own spend. Slice 1 (capture, in-chat decisions, atom write-back) is about **16 hours** at the lower bound; the board and Anthropic-only grounding add about 12, and the Redkiln-native type plus the unattended pause loop about 15. Five calls remain for Ryan. The weightiest is whether IQBG client-repo items may be published to claude.ai at all.

## Amendment, 2026-09-27: seven decisions taken, slice 1 built

Ryan decided seven items on the Weigh-In board; each chose the recommended option. Two of them reopened defaults in the design below after reading redkiln-rs (`D:\repos\redkiln-rs`), so parts of that design are superseded rather than edited.

| Item | Decision | What it changes below |
|---|---|---|
| wi-7c21e4 | Per-repo board mode; client repos default to a local board | As proposed |
| wi-a93f02 | Client repos keep ledger data local | Its "commit event files in your repos" half is superseded by wi-6e2a47 |
| wi-4d8b6a | Unattended runs default judgment-bound two-way items; one-way items block | As proposed |
| wi-e15c77 | Reserve `.kb/_ledger/` (now config only) and teach kb-ingest the intake keys | As proposed |
| wi-2b90f3 | Reliance drills on two-way items only, 1 in 30 for 60 days, disclosed before write-back | As proposed (slice 2) |
| wi-b4c1d8 | Rust on redkiln-rs's toolchain (1.98.1, edition 2024), events through happenstance | **Supersedes** the Node CLI in "Components and fixed inputs". Its Cowork half ("board, no binary") is amended on 2026-09-28, below |
| wi-6e2a47 | Open items in a store under the git common dir; decisions reach git as atoms plus a commit trailer | **Supersedes** "Ledger: per-session event files, folded across worktrees" |

What redkiln-rs showed, and why the two defaults moved:

- redkiln-rs keeps one store per repository at `<git-common-dir>/redkiln/store.sqlite3`, shared by every worktree and never committed (`crates/rk-store/src/conn.rs:72`; ADR-0018).
- git stays the tier-1 authority; answers are trailer commits on the run's branch; the store is derived and rebuildable (ADR-0105 clauses 3–4; operator-surface proposal §12.3; F19 rebuilt 174 of 174 records from git).
- happenstance, Ryan's DCB event store, is proposed as redkiln-rs's tier-2 adapter (§12.1–12.3).
- In Phase 1 only a CLI answer reconciles (ADR-0129); Weigh-In's board answers are applied by the CLI, which fits.

**Slice 1 as built** (`weigh-in` repository):

- `weighin-core` (no I/O: events, ids, markers, scanning, cards, projection, atoms, board format) and the `weighin` CLI; 41 tests, clippy clean. An independent review found eleven defects; all are fixed with regression tests.
- The ledger is happenstance's SQLite event store at `<git-common-dir>/weighin/ledger.sqlite3`. Writes are conditioned: raising is idempotent, and two sessions cannot both decide one item. The per-session files, the worktree fan-out and the Lamport ordering in the superseded section are not needed.
- Hooks are command hooks (SessionStart, Stop, SubagentStop, PostToolUse) through one `weighin hook` entry point. The `/weigh-in` skill runs the sitting with one batch call, one question call and one batch call.
- Decision atoms pass Redkiln's `validate --kb`, with a malformed atom rejected as a negative control.
- Static binaries: Linux (musl) and Windows (mingw) cross-built; the launcher picks by platform.

**Measured in live Claude Code sessions** (Sonnet, plugin loaded with `--plugin-dir`):

- The agent emitted a `weigh-in` block unprompted from the SessionStart convention, and the hooks captured it and the plan's open questions.
- A sitting took 3–4 requests and cost $0.42–0.57 in short sessions, against the $0.28 estimate below. The difference is output: the batch JSON and the cards.
- Two defects surfaced and were fixed: the model changed into the plugin directory (the skill now pins `--dir "${CLAUDE_PROJECT_DIR}"`), and ripgrep searched stdin instead of the tree when called from a hook (now an explicit path and a null stdin).
- One behaviour needed a guard. The agent labelled a compliance question a two-way judgment call, and a sitting dismissed it as "already decided" because the task said "assume and keep going". Authority words now keep such markers open, and the CLI refuses that dismissal unless it cites an accepted atom or the user's own words. On the next run the question was presented.

## Amendment, 2026-09-28: Cowork runs the ledger through the computer's shell

A first Cowork session found four gaps. In Cowork, plugins come from the account, not a local marketplace. Skills and hooks run in a cloud workspace whose project folder is scratch. Edits to the user's folder go through the computer's shell, so `PostToolUse` never sees them. A ledger created in a connected folder failed with `SQLITE_IOERR_DELETE` (2570), because those folders refuse deletes and both WAL and SQLite's default journal delete files. What changed in 0.1.2:

- The ledger runs under a TRUNCATE journal and no longer goes through happenstance's WAL open. A shim that refuses deletes reproduces 2570 on 0.1.1. On 0.1.2 the same test creates, raises and decides without error and recovers 0.1.1's leftovers; strace, run in the container and on the computer's mount, showed no deletes or renames in the folder.
- The ledger location is settable (`--store`, `WEIGHIN_STORE`, `store` in the config). The default outside git stays `<folder>/.weighin/`. It is safe on one computer; for a synced folder used from several, point `store` outside the synced tree.
- The skill's Cowork route finds `weighin` in any connected folder and runs it through the computer's shell against the folder worked in. It scans the files the conversation changed and adds decisions to the folder's own ruling log when there is one. This amends wi-b4c1d8's Cowork half: the board path remains the fallback when no binary is reachable. Hooks write nothing in Cowork; they still prime the convention and nudge on one-way markers.
- `scripts/package-plugin.sh` builds the zip that Customize > Plugins takes.

## Later decisions

Decisions taken after 2026-09-28 (the command name, how binaries reach Cowork, the repository) are recorded in [`docs/decisions/`](../decisions/README.md). Where they differ from the design below, the decision records win. A draft amendment at the end, [what v1 shipped](#amendment-draft-2026-10-14-what-v1-shipped), sums them up against this design.

## Every tool keeps a thread inbox; none keeps a question ledger

Teams handle the questions agents leave behind in three ways, and none of them is a ledger.

**The end-of-session sweep** comes in four families:

- **Handoff documents** record decisions as prose for the next agent. HumanLayer's `/create_handoff` has fixed sections for tasks, changes, learnings and next steps, but **no open-questions or decisions section** ([HumanLayer create_handoff](https://github.com/humanlayer/humanlayer/blob/main/.claude/commands/create_handoff.md)). compound-engineering's `ce-handoff` writes an immutable snapshot that lists "Decisions, constraints, and rejected alternatives". For each decision the writer must say "whether it was the user's, your inference, or your own call" ([ce-handoff create.md](https://github.com/EveryInc/compound-engineering-plugin/blob/main/skills/ce-handoff/references/create.md)).
- **Learnings sweeps** route rules into CLAUDE.md ([revise-claude-md](https://github.com/anthropics/claude-plugins-official/blob/main/plugins/claude-md-management/commands/revise-claude-md.md)) or into a solutions folder. On 2026-09-24 the solutions-folder entries gained a `retire_when` condition ([compound-engineering PR #1772](https://github.com/EveryInc/compound-engineering-plugin/pull/1772)).
- **Recaps** compress the session. Claude Code's away summary is "Under 40 words" and was capped at 400 characters after "runaway session recaps" ([Claude Code changelog](https://code.claude.com/docs/en/changelog)).
- **Anthropic's `project-artifact` plugin** is the nearest official thing to a visual ledger. It builds a tabbed status page with **"Risks & open questions", "Decisions / FAQ" and an "Attention" tab**. Each Attention item is "the exact action (a paste-ready message or a one-word decision)", and the page refreshes in place, telling the user "only what changed" ([project-artifact SKILL.md](https://github.com/anthropics/claude-plugins-official/blob/main/plugins/project-artifact/skills/project-artifact/SKILL.md)).

All four keep text. None keeps an item with a status that survives the session and waits for a human decision.

**The orchestrator's "waiting on you" lane** is the second approach. Claude Projects puts threads into groups, including "**Waiting on you**". It resolves idle threads "automatically after a week with no activity". Approvals must be answered inside the thread, because "Telling Claude in the project conversation to go ahead doesn't reach it" ([Claude Code docs: Projects](https://code.claude.com/docs/en/claude-projects)). Devin's command-center columns, Linear's `awaitingInput` and `stale` session states, and Vibe Kanban's In Review column all work the same way at session or task level ([Devin](https://docs.devin.ai/desktop/agent-command-center); [Linear](https://linear.app/developers/agent-interaction); [Vibe Kanban](https://vibekanban.com/docs/core-features/reviewing-code-changes)). Everywhere, the unit of attention is the thread, and ranking is by state, not by how much a question matters. Users are asking for the layer below. One Claude Code request wants a persistent sidebar of open questions, because they "get buried in the scrollback and missed" and cause "wasted work (Claude proceeding on assumptions) or stalled work" ([anthropics/claude-code #79078](https://github.com/anthropics/claude-code/issues/79078)). Another wants the plan-start options held back while unresolved open questions sit "at the bottom of the plan" ([#64178](https://github.com/anthropics/claude-code/issues/64178)).

**Resolving questions inside a planning artifact** is the third approach, and it holds the reusable rules.

spec-kit's `/speckit.clarify`:

- allows a "Maximum of 5 total questions", ranked by "(Impact * Uncertainty)";
- leads with the recommended option and accepts "yes" as an answer;
- writes each answer back as "Q: → A:" under a dated session heading;
- reports unasked questions as Deferred or Outstanding instead of dropping them ([spec-kit clarify.md](https://raw.githubusercontent.com/github/spec-kit/main/templates/commands/clarify.md)).

compound-engineering has added rules that map onto Ryan's ban on papering over and kicking the can:

- **Settled-decisions protocol.** Conversation decisions are classified as Settled, Directive or Unlabeled. Agents may not settle their own proposals, and the protocol says "Never re-ask a settled decision" ([settled-decisions.md](https://github.com/EveryInc/compound-engineering-plugin/blob/main/skills/ce-plan/references/settled-decisions.md)).
- **Adjudicate before escalating (2026-09-22).** Only **authority-bound** items go to the human: security, billing, data retention, migrations, product calls, or intent only the user knows. Everything else follows the rule "uncertainty resolves to fixing … not escalating" ([PR #1765](https://github.com/EveryInc/compound-engineering-plugin/pull/1765); [evaluation-rubric.md](https://github.com/EveryInc/compound-engineering-plugin/blob/main/skills/ce-resolve-pr-feedback/references/evaluation-rubric.md)).
- **Fail safe when no human is reachable.** The sweep skill says: "With no usable blocking-question tool, behave as non-interactive" ([ce-sweep SKILL.md](https://github.com/EveryInc/compound-engineering-plugin/blob/main/skills/ce-sweep/SKILL.md)).

Seeds, a June 2026 relative of beads built for deliberations, adds a lifecycle rule: "every state transition — resolving, deferring, abandoning — takes a reason" ([Introducing seeds](https://jigsaw.io/tools/2026/06/10/introducing-seeds.html)).

Visual review tools that last share two traits: **they attach to a stop point the user already has, and they return one batch**:

- **Plannotator** has 8.9k stars and shipped weekly releases through September. It hooks `ExitPlanMode` with a `PermissionRequest` command hook whose timeout is 345,600 seconds, and returns approval or structured feedback on stdout ([Plannotator](https://github.com/backnotprop/plannotator)).
- **compound-engineering's prototype loop** runs a blocking `wait` that "exits 0 with a JSON array of annotation records" when the human presses Send. The agent spends nothing while the human reviews ([annotation-loop.md](https://github.com/EveryInc/compound-engineering-plugin/blob/main/skills/ce-prototype/references/annotation-loop.md)).
- **Vibe Kanban**, Claude Desktop's diff review and Antigravity all collect comments and submit them together, "as a complete review" ([Vibe Kanban](https://vibekanban.com/docs/core-features/reviewing-code-changes)).
- **The superpowers visual companion** adds a caution Weigh-In should respect. It keeps "Tradeoff lists" and "Technical decisions" in the terminal and opens a browser only when "the content itself is visual" ([visual-companion.md](https://github.com/obra/superpowers/blob/main/skills/brainstorming/visual-companion.md)).

Recent failures are specific enough to set guardrails:

- **Automatic recaps backfired.** OpenAI Codex's automatic recaps drew a September "un-request": "A `/recap` command does not bother me … Automated recap does bother me." Users also reported hidden model calls. In one bug, a recap turned an approval-gated test into "verified and complete" because the recap prompt saw only a truncated 900-byte window ([openai/codex #47306](https://github.com/openai/codex/issues/47306); [#41896](https://github.com/openai/codex/issues/41896); [#41626](https://github.com/openai/codex/issues/41626)). Weigh-In therefore runs only when invoked, extracts from explicit markers or the full context rather than a truncated tail, and carries "pending" and "assumed" through word for word.
- **Review bots show what gets acted on.** Hunk-level comments were addressed 6.5–19.2% of the time, against 0.9–4.2% for file-level ones. Concise, code-heavy comments did better. For one tool, manually triggered comments were addressed 12.8% of the time against 6.8% for automatic ones; across all tools the gap (14.4% vs 9.0%) was not significant ([Sun … Treude 2026](https://ctreude.ca/pdf/does-ai-code-review-lead-2026.pdf)). PostHog switched off nine CodeRabbit walkthrough sections because only one comment "holds findings" ([PostHog #100461](https://github.com/PostHog/posthog/pull/100461)).
- **In-repo trackers failed on storage.** beads replaced JSONL with Dolt as its database, which broke community tools ([beads Discussion #1836](https://github.com/steveyegge/beads/discussions/1836)). beads_rust silently dropped a child item, and the losing side's comments, on merge while reporting "Merge complete" ([beads_rust #512](https://github.com/Dicklesworthstone/beads_rust/issues/512)). One team found that `merge=union` on an append-only ledger merged conflicting `accepted` and `rejected` verdicts "cleanly" into results that depended on line order ([arhugula-harness #1563](https://github.com/thestoryportal/arhugula-harness/pull/1563)). Status-by-folder and committed generated indexes produced duplicate IDs and constant conflicts ([pyrite #209](https://github.com/pyrite-wiki/pyrite/issues/209); [cachy-app #2864](https://github.com/mydcc/cachy-app/issues/2864)).
- **A reference implementation went away.** The 12 September brief modelled its question object on HumanLayer, whose open-source code is now "pretty much all deprecated" ([humanlayer/humanlayer](https://github.com/humanlayer/humanlayer)).
- **Decision records die without a ritual.** About half of GitHub repositories with ADRs hold only one to five of them ([Buchgeher et al.](https://se.jku.at/using-architecture-decision-records-in-open-source-projects-an-msr-study-on-github/)).

The verdict splits into what to adopt and what to leave.

Adopt:

- spec-kit's cap, ranking and Q→A write-back;
- compound-engineering's settled-decision taxonomy, no self-settling, adjudicate-before-escalate, attribution and fail-safe rules;
- Seeds' reason on every transition;
- beads' hash IDs and ready-queue semantics;
- project-artifact's tab structure and delta refresh;
- the blocking-wait return loop;
- MADR 4 as the record body.

Leave:

- transcript mining;
- a registry MCP server;
- `merge=union`;
- status-by-folder;
- committed generated indexes;
- anything that fires automatically.

## Five filters decide what deserves Ryan's attention

The volume problem is solved upstream of any UI. A meta-analysis of 106 studies found that human–AI teams did **worse than the better of the two alone (Hedges' g = −0.23)**. Losses were larger on decision tasks, and separately, larger where the AI outperformed the humans ([Vaccaro et al., Nature Human Behaviour 2024](https://www.nature.com/articles/s41562-024-02024-1)). Agents told to ask questions over-ask. On Ambig-SWE, Claude models reached the same information gain with about half as many questions by exploring first ([Vijayvargiya et al., ICLR 2026](https://arxiv.org/abs/2502.13069)). Google's static-analysis experience set a 10% effective-false-positive bar before users stop trusting findings ([Sadowski et al., CACM 2018](https://cacm.acm.org/research/lessons-from-building-static-analysis-tools-at-google/)). Facts the repo can answer should be settled by the agent. Ryan's attention belongs on value trade-offs and one-way doors, where his context is the stronger input. Five filters, applied in order from cheapest to dearest, enforce that.

**Filter 1: is it already settled?**

- **Settled in the chat.** A choice Ryan made with the trade-off in view is recorded as decided, with provenance `user-directed` or `user-approved`, and never re-asked.
- **Directive.** A choice he asserted without examining it gets at most one challenge card, and only when grounding finds contrary evidence. This follows compound-engineering's rule that research may contradict a settled decision "only on evidence" ([settled-decisions.md](https://github.com/EveryInc/compound-engineering-plugin/blob/main/skills/ce-plan/references/settled-decisions.md)).
- **Settled in `.kb`.** A candidate that matches an accepted `decision` atom is dismissed as `already-decided:<atom id>`. The only exception is when new evidence contradicts the atom. Then it becomes a supersede proposal that must display that evidence, because accepted decisions are immutable in Redkiln (`validate --kb` flags in-place edits, per Redkiln's `src/store/validate-kb.ts`).

**Filter 2: can the repo answer it?** If the answer can be read from code, tests, docs or git history, the agent answers it, logs the evidence, and dismisses the item as `recoverable`.

**Filter 3: does it need Ryan's authority, and is it a one-way door?** The authority-bound versus judgment-bound split comes from compound-engineering's rubric. Door type comes from Bezos's distinction: some decisions are "consequential and irreversible or nearly irreversible", while most "are changeable, reversible – they're two-way doors" ([Amazon 2015 letter](https://www.sec.gov/Archives/edgar/data/1018724/000119312516530910/d168744dex991.htm)). A judgment-bound, two-way item with a stable recommendation is decided by the agent, logged as a default, and listed in a "decided for you" digest that Ryan can reverse in one click. It never becomes a question.

**Filter 4 is the decide-now test**, which is what stops can-kicking. Information-value theory says waiting is worth something only if some identifiable information would change the choice. Annie Duke's stopping rule asks "Is there some information that I could find out that would change my mind?" and says to get it if it is cheap ([Duke, How to Decide excerpt](https://www.annieduke.com/site/uploads/howToDecide_excerpt.pdf); [Value of information](https://en.wikipedia.org/wiki/Value_of_information)). Real options expire, and they should not be committed to early "unless you know why" ([Matts & Maassen, InfoQ](https://www.infoq.com/articles/real-options-enhance-agility/)). "Last responsible moment" is widely criticised as licensed procrastination. Zimmermann's "most responsible moment" comes **early** for decisions that set conceptual integrity or spawn many follow-on decisions ([Wirfs-Brock](https://wirfs-brock.com/rebecca/blog/2011/01/18/agile-architecture-myths-2-architecture-decisions-should-be-made-at-the-last-responsible-moment/); [Zimmermann, ITARC 2024](https://ozimmer.ch/assets/presos/ZIO-ITARCKeynoteTADv101p.pdf)). Event model, aggregate boundaries and tenancy all fall in that category.

Each item gets five checks:

1. **Flip condition.** Name the specific fact that would flip the recommendation. If none can be named, decide now.
2. **Obtainable now?** If the fact can be got now or cheaply, get it and decide.
3. **Arrival versus expiry.** If the fact arrives after the option expires, decide now.
4. **Cost of waiting, in natural units.** If blocked work or rework is visible, decide now.
5. **Closeness.** If the top options would each be acceptable as the only option, decide now and mark the choice cheap to revisit. This is Duke's only-option test.

A deferral is legitimate only when every check points to waiting. It must also carry an owner, a trigger event, a resolve-by date, and a default that applies automatically if nobody acts.

**Filter 5 is an evasion check** that the recommendation must pass before it is shown. No published checklist of evasive recommendations exists; the table below combines single-pattern sources.

| Evasion pattern in a recommendation | Clears only if the card states | Evidence |
|---|---|---|
| "Make it configurable" | the second concrete value and who needs it now | 6.1–16.7% of parameters set by most users; up to 53.3% of config errors come from wrongly kept defaults ([Xu et al. FSE 2015](https://neverworkintheory.org/2016/06/09/too-many-knobs.html)) |
| "Put it behind a feature flag" | flag kind, removal owner, removal trigger; a flag choosing between designs is an undecided decision | flags are "inventory which comes with a carrying cost" ([Hodgson](https://martinfowler.com/articles/feature-toggles.html)); Uber's Piranha found 17% of flags stale ([Piranha](https://manu.sridharan.net/files/ICSE20-SEIP-Piranha.pdf)) |
| "Add an interface so we can swap later" | a second implementation needed within the planning horizon | "Duplication is far cheaper than the wrong abstraction" ([Metz](https://sandimetz.com/blog/2016/1/20/the-wrong-abstraction)); refactoring that eases change is not YAGNI ([Fowler](https://martinfowler.com/bliki/Yagni.html)) |
| "Revisit later / phase 2 / follow-up ticket" | a passing decide-now test with trigger, default and owner | deliberate-prudent debt names its payoff ([Fowler quadrant](https://martinfowler.com/bliki/TechnicalDebtQuadrant.html)) |
| Riskiest part phased last | the weakest "what would have to be true" condition tested in phase 1 | "Worst first" ([Zimmermann](https://ozimmer.ch/assets/presos/ZIO-ITARCKeynoteTADv101p.pdf)) |
| Agent adds TODO/FIXME/"temporary" | a ledger item with default and trigger, or removal before hand-off | 22.7% of AI-introduced issues survive to the latest version ([Liu et al. 2026](https://arxiv.org/html/2603.28592v2)); agent SATD names problems "without clear guidance on resolution" ([Brahmi et al. MSR 2026](https://2026.msrconf.org/details/msr-2026-mining-challenge/28/Characterizing-Self-Admitted-Technical-Debt-Generated-by-AI-Coding-Agents)) |
| "Both / hybrid", "it depends", "needs a spike", "test later" | why supporting both is cheapest; a conditional pick ("A unless X"); the result that flips it and a time-box as the next action; the test and why it cannot run now | synthesis of the rows above |
| Recommendation relocates the question | the original question answered, not moved into config, a flag, an interface, a ticket or a later phase | general test |

Risk is expressed without false precision:

- **No risk matrices or heat colours.** Typical risk matrices "can correctly and unambiguously compare only a small fraction (e.g., less than 10%) of randomly selected pairs of hazards" and can rank risks backwards ([Cox 2008](https://pubmed.ncbi.nlm.nih.gov/18419665/)). Experts' 90% intervals contained the truth only 55–78% of the time ([Hubbard & Evans 2010](http://paci.com.au/downloads_public/risk/Douglas-Hubbard_May2010.pdf)).
- **MUSTs as pass/fail screens.** Constraints come from decisions already in force. An option that fails one is struck through, not scored down ([Kepner-Tregoe](https://www.mindtools.com/atznth6/the-kepner-tregoe-matrix/)).
- **Consequence-if-wrong in natural units**, such as "migrates every event stream, 2–4 days".
- **Reversibility × blast radius**, with concrete anchors.
- **Two to four "what would have to be true" conditions per option**, with the weakest one flagged ([Roger Martin](https://rogermartin.medium.com/what-would-have-to-be-true-83dac5bd2189)).
- **A swing statement instead of weighted totals**: "A wins unless you weight operability above time-to-market."

Confidence comes from agreement, not self-report:

- LLM verbalised confidence is inflated. Reasoning models report more than 85% even when wrong, and "Claude 3.7 Sonnet becomes more poorly calibrated" with introspection ([Mei et al. 2025](https://arxiv.org/html/2506.18183)). Consistency across samples improves calibration ([Xiong et al. ICLR 2024](https://arxiv.org/abs/2306.13063)).
- Longer explanations raise trust without raising accuracy ([Steyvers et al. 2025](https://arxiv.org/abs/2401.13835)).
- First-person uncertainty ("I'm not sure, but…") cut agreement with the AI from 80.9% to 74.8% and raised accuracy from 63.9% to 72.8% ([Kim et al. FAccT 2024](https://ar5iv.labs.arxiv.org/html/2405.00623)).

**Pre-selecting the recommendation is a default.** Defaults have an overall effect of d = 0.68 across 58 studies, moderated by perceived endorsement and by endowment ([Jachimowicz et al. 2019](https://www.cambridge.org/core/journals/behavioural-public-policy/article/when-and-why-defaults-influence-decisions-a-metaanalysis-of-default-effects/67AF6972CFB52698A60B6BD94B70C2C0)), and disclosing a default does not weaken it ([Bruns et al. 2018](https://www.sciencedirect.com/science/article/abs/pii/S0167487017307845)). A bare verdict produced the lowest agreement of any presentation (66.7%) ([Gao et al. 2026](https://pith.science/paper/2607.24601)). Ryan usually accepts the recommendation, so Weigh-In earns the pre-selection rather than hiding it:

- Mark it "(Recommended)" only when it passed the evasion check and held across permuted samples.
- Always show the flip condition beside it, so accepting means accepting "A unless X".
- Present options first, with first-person uncertainty, only for one-way doors or unstable picks.

Deliberative designs improved accuracy from 52.4% (a standard explainable-AI display) to 59.8%, but raised median time per decision from 33 to 236 seconds ([Ma et al. CHI 2025](https://arxiv.org/html/2403.16812v2)). That cost is justified only at one-way doors.

Sequencing follows dependency:

- Upstream and one-way decisions come first. Downstream cards whose options depend on them are regenerated after the upstream decision, not shown in parallel.
- Within a tier, "worst first".
- The rest are grouped by shared context.

The reason to batch is context-switch cost, not "decision fatigue". A 36-lab preregistered ego-depletion test found d = 0.06 ([Vohs et al. 2021](https://journals.sagepub.com/doi/abs/10.1177/0956797621989733)).

## Scripts supply the context; one board spans every client

Deciders need rationale, alternatives and code context, on one screen:

- Reviewers' biggest information need is "what instigated the change" ([Bacchelli & Bird 2013](https://www.microsoft.com/en-us/research/wp-content/uploads/2016/02/ICSE202013-codereview.pdf)).
- Across 900 review threads, "suitability of an alternative solution" was the most frequent need, with code context and invocation paths fourth ([Pascarella et al. CSCW 2018](https://fpalomba.github.io/pdf/Conferencs/C36.pdf)).
- Related items work best "visible on the screen at the same time" ([Baum et al. 2017](https://sback.it/publications/icsme2017.pdf)).
- Reviewers of LLM multi-file changes cannot scale their effort to risk when the AI shows uniform confidence ([Heander et al. 2026](https://arxiv.org/html/2606.01969v1)).

So each card carries:

- the trigger;
- two to four options;
- two or three `path:line` anchors;
- one excerpt of 3–15 lines;
- a blast-radius list;
- a door badge.

The excerpt length follows git's three-line context default and the one-screen rule. SWE-agent's 100-line window is an optimum for models reading code, not for human cards ([SWE-agent ACI](https://swe-agent.com/0.7/background/aci/)).

A script computes the context and the model reads only its JSON. That is the token-cheap path:

- `git diff --numstat`;
- ripgrep reference counts and top call sites (the LSP tool is inactive in cloud sessions) ([tools reference](https://code.claude.com/docs/en/tools-reference));
- `git log -L` reduced to one line ("N commits; last sha, date, subject"), because raw output on an active function runs to thousands of tokens;
- tests that reference the symbol;
- matching `.kb` atoms;
- SHA-pinned permalinks.

Links take three forms:

- plain `path:line`, which survives every renderer. Claude Code emits clickable OSC 8 links only in tool-call headers, not in response text ([#69008](https://github.com/anthropics/claude-code/issues/69008));
- a SHA-pinned GitHub or Azure DevOps permalink;
- an editor URI (`vscode://file/<abs>:line:col`) on HTML surfaces ([VS Code CLI docs](https://code.visualstudio.com/docs/configure/command-line)).

A `claude-cli://open?q=…` deep link can pre-fill a `/weigh-in decide` prompt without sending it, within a 5,000-character limit ([deep links](https://code.claude.com/docs/en/deep-links)).

Non-code decisions in Cowork need verbatim quotes checked by a script. In one study only **74.5% of generative-search citations supported their sentence** ([Liu, Zhang & Liang 2023](https://aclanthology.org/2023.findings-emnlp.467/)). Fine-grained attribution "significantly reduces the time required for fact verification" ([Slobodkin et al. ACL 2024](https://aclanthology.org/2024.acl-long.182/)).

Each rendering surface covers only part of Ryan's clients:

- **AskUserQuestion** exists in every Claude Code client, including Cowork's question card. It allows 1–4 questions of 2–4 options, a header of at most 12 characters, and is unavailable to subagents ([SDK user input](https://code.claude.com/docs/en/agent-sdk/user-input)). Its output now includes per-question `annotations.notes`, a free-form `response`, and `afkTimeoutMs` ([SDK TypeScript reference](https://code.claude.com/docs/en/agent-sdk/typescript)).
- **Option previews** render in the terminal's preview dialog, with a notes field (layout observed, not documented). The VS Code panel never showed them, and since v2.1.281 Claude stops attaching them there ([changelog](https://code.claude.com/docs/en/changelog)).
- **MCP Apps** render in claude.ai, Desktop, Cowork and the Desktop Code tab, but the terminal hides `ui://` resources ([Claude Code MCP docs](https://code.claude.com/docs/en/mcp)).
- **Elicitation** is documented only for Claude Code ([Claude Code MCP docs](https://code.claude.com/docs/en/mcp)).
- **Local pages** cannot reach Cowork or cloud sessions.
- **Channels** (`--channels`, a research preview) disable AskUserQuestion ([changelog](https://code.claude.com/docs/en/changelog); [channels](https://code.claude.com/docs/en/channels)).

The one surface that spans the Claude Code CLI, Desktop, cloud and Cowork is a **published Artifact**. Its runtime `db` store is written by the page and read back by Claude through `ArtifactData`, and during September it gained updates that remove a single field (v2.1.273) and ordered queries that say when a result is one page and how to read the rest (v2.1.283) ([changelog](https://code.claude.com/docs/en/changelog)). The public docs do not document the `db` store or name the `ArtifactData` tool; both come from the tool definition and runtime contract served to this session. A 2026-09-15 Desktop fix let Cowork tasks read and update existing artifacts ([Desktop/Cowork changelog](https://claude.com/docs/cowork/changelog)). Artifacts have limits:

- They need a claude.ai login and the Anthropic API, not Bedrock, Vertex or Foundry.
- They are unavailable in CMEK, HIPAA or ZDR organisations ([Artifacts](https://code.claude.com/docs/en/artifacts)).
- Viewers can write `db` data, so answers read back are untrusted input.

The repo therefore stays the system of record, and the board is a view and edit surface over it.

Token mechanics set the rest of the shape:

- **Skills are cache-safe.** They append their body as a user message. A plugin's skills, commands, agents, hooks, monitors and themes never invalidate the cache; a plugin that adds MCP servers whose tools load into the prefix does force a full re-read ([prompt caching](https://code.claude.com/docs/en/prompt-caching)).
- **`model:` on a non-fork skill costs a full cache miss** when it names a model other than the session's current one. It forces the next request to re-read the whole conversation. On a `context: fork` skill or subagent it is cheap, but the subagent starts a fresh, uncached prefix with a five-minute TTL. Explore and Plan agents skip CLAUDE.md and git status ([sub-agents](https://code.claude.com/docs/en/sub-agents)).
- **`!` injection costs zero model turns**, but Cowork replaces every `!` line with a placeholder. Skills synced from claude.ai run `!` in cloud sessions, but in local terminal sessions the `!` line reaches Claude as literal text. Weigh-In therefore installs as a local plugin from the marketplace and keeps a Bash fallback ([skills](https://code.claude.com/docs/en/skills)).
- **Hooks differ in cost.** Command hooks cost no model tokens. Prompt hooks call Haiku on every firing. Hook injections over 10,000 characters spill to a file ([hooks](https://code.claude.com/docs/en/hooks)).
- **Cache lifetime depends on billing.** The main-conversation cache lives one hour on a subscription and five minutes on API or usage credits ([prompt caching](https://code.claude.com/docs/en/prompt-caching)).
- **Opus 5.5 prices** are $4 input, $20 output and $0.20 cache reads per million tokens ([changelog](https://code.claude.com/docs/en/changelog)).
- **Context files cost more than they return.** They raised inference cost by more than 20% without improving task success ([Gloaguen et al. 2026](https://arxiv.org/abs/2602.11988)). Decisions should therefore reach future agents through just-in-time hooks, not a growing CLAUDE.md.

Several positions in the 12 September brief change as a result:

| 12 September brief | Evidence as of 26 September | Effect on Weigh-In |
|---|---|---|
| URL-mode elicitation documented only for auth; unproven as a review-page launcher | v2.1.281 (Sep 23) added URL-mode elicitation on 2026-07-28 protocol connections for browser flows | Works, but Claude Code-only; not needed once the Artifact board exists |
| AskUserQuestion previews described as an SDK feature | Terminal renders markdown previews plus a notes field; VS Code never shows them | Previews carry the evidence box in the terminal; option descriptions must stand alone |
| Rich review via a local page or a static Cowork artifact | Artifacts carry a `db` that Claude reads back from Claude Code and Cowork | One persistent board per repo for every client |
| Registry MCP server (SQLite) in Phase 2 | CLI output adds no per-tool listing ([costs](https://code.claude.com/docs/en/costs)); a JSONL fold suffices below a few hundred items ([Yegge](https://steve-yegge.medium.com/beads-best-practices-2db636b9760c)); storage churn broke beads tooling | Node CLI plus JSONL; no MCP server unless MCP Apps are later needed |
| Prompt-hook classifier on every Stop in Phase 1 | Prompt hooks call a model each firing; command hooks cost nothing | Deterministic marker extraction; model judgment only inside the invoked sitting |
| Recommendation hidden behind a reveal for every item | Defaults are robust (d = 0.68) and disclosure does not weaken them; bare verdicts get the lowest agreement | Earned pre-selection; options-first only for one-way or unstable items |
| Judge from a second model family, or Codex | Ryan: Anthropic models only | Cross-tier permuted sampling on Sonnet against the Opus pick |
| Defer loop can wait if unattended work is under a quarter | Ryan: work is about half unattended | Unattended capture and auto-default in slice 1; pause loop in slice 3 |
| HumanLayer as the wire-shape model | HumanLayer OSS deprecated | Keep the fields, drop the reference |
| Stat that "63% of ADRs are opened already accepted" (ICSA 2026) | This research could not locate the study | Not relied on |

## The Weigh-In design, with every number filled in

### Components and fixed inputs (runtime superseded by wi-b4c1d8: Rust, not Node)

Weigh-In ships as a Claude Code plugin, `weigh-in`, in Ryan's Wet-Ink marketplace. It is skills-first, with one dependency-free Node CLI, and it is designed so that every event and state maps onto a future Redkiln `decision` item type. Decided records land as `.kb` atoms through `.kb/_intake/` and kb-ingest in every repo, judges are Anthropic models only, about half the traffic comes from unattended runs, and the codename stays.

Node is chosen as an assumption, for two reasons. Redkiln's hooks already run `node "${CLAUDE_PLUGIN_ROOT}/dist/…"`, so the later port reuses code. Cowork cannot install `bin/` executables ([plugin platform support](https://claude.com/docs/plugins/platform-support)), so the CLI lives under the skill directory and is run through Bash there.

| Component | Path in plugin | Kind | Model-token cost |
|---|---|---|---|
| `/weigh-in [session\|repo\|sweep <path>\|board\|digest\|stats]` | `skills/weigh-in/SKILL.md` | skill, `disable-model-invocation: true`, no `model:` field, body ≤150 lines | 0 always-on; ~1,800 on invoke |
| `weigh-in-grounder` | `agents/weigh-in-grounder.md` | subagent, `model: sonnet`, tools Read/Grep/Glob plus read-only Bash (`git`, `rg`) | ~60 always-on roster line; ≤30k per one-way item |
| `weighin` CLI | `skills/weigh-in/scripts/weighin.mjs` | Node ≥20, single file, no dependencies, built from TypeScript | 0; only its printed output enters context |
| Prime hook | `hooks.json` SessionStart → `weighin prime` | command | ~150 tokens per session |
| Capture hooks | Stop and SubagentStop → `weighin capture`; PostToolUse `Write\|Edit` → `weighin scan` | command | 0 |
| Board page | `skills/weigh-in/assets/board.html` | static Artifact page with `db`, published by file path | 0 generation |
| Atom templates | `skills/weigh-in/templates/{decision,open_question}.md` | rendered by the CLI | 0 |

The CLI is the only writer of ledger state, following the ce-sweep precedent of one writer. Every command prints compact text, because its output is what the model pays for.

| Command | Purpose | Printed output (est. tokens) |
|---|---|---|
| `prime` | SessionStart: marker convention, session key, open counts | ≤150 |
| `capture` | Parse `weigh-in` blocks from the hook's `last_assistant_message` and append `item.raised` | none |
| `scan <path>` | Headings ("Open questions", "Assumptions", "Decisions needed") and newly added TODO/FIXME/HACK lines in the written file | none |
| `open [--session k \| --repo] [--top 4] [--with-context]` | Projection view for a sitting | ~60 per item, ≤600 per context pack |
| `decide <id> <opt> [--note] [--provenance]` | Event, atom render, source annotation | ~40 per item |
| `defer <id> --flip --trigger --by --default` | Legitimate deferral; open_question atom if it outlives the session | ~40 |
| `dismiss <id> --reason` / `default <id> --reason` | Terminal events | ~20 |
| `digest` | Defaults taken by unattended runs since the last sitting | ~40 per item |
| `board export` / `board apply <file>` | Write the sync file for ArtifactData; apply answers read back | ~60 |
| `serve --wait` | Local review page for tenants without artifacts; exits with a JSON answer batch | ~60 per answer |
| `stats`, `compact` | Health metrics; prune terminal history | ≤200 |

### Capture: markers first, hooks at zero model tokens

There are three capture channels, in falling order of precision. None of them reads transcripts, whose format "is internal to Claude Code and changes between versions" and which are deleted after 30 days by default ([sessions](https://code.claude.com/docs/en/sessions)).

**The primary channel is the agent's own marker.** `weighin prime` injects a convention of at most 120 tokens at SessionStart, together with the session key and open counts. The convention tells the agent: whenever it assumes, defers, makes a call the user did not examine, or cannot decide, it emits a fenced block like this one (roughly 55–70 output tokens):

````
```weigh-in
kind: assumption          # question | assumption | deferral | call
door: one-way             # one-way | two-way | unsure
authority: authority      # authority | judgment
q: Should retention overrides attach per record or per series?
did: per-series, matching RetentionPolicy; per-record not built
flip: a tenant needs per-record legal hold
at: src/Retention/RetentionPolicy.cs:88
blocks: none
```
````

The convention also contains two further rules:

- **No unmarked TODOs.** The agent may not leave a TODO, FIXME or "revisit later" without a block.
- **Fail safe when no one can be asked.** When AskUserQuestion is unavailable, the agent takes no one-way-door path. It emits the block with `did: blocked` and continues with independent work. This is ce-sweep's fail-safe, and it covers the half of Ryan's work that runs unattended.

The Stop and SubagentStop command hooks parse these blocks from `last_assistant_message`. Subagents cannot ask the user, so their reports are captured at SubagentStop. Each block is appended as an `item.raised` event carrying the verbatim quote, `session_id`, `agent_id` and `channel: marker`. The event ID is derived from the session and a hash of the block, so a hook that fires twice appends nothing new.

**The second channel is the artifact scan.** `weighin scan` runs on every Write or Edit of files matching configurable globs: markdown under `docs/`, `.bklg/`, the plans directory and run-notes folders by default, plus code files for newly added TODO lines only. It raises lower-precision `channel: scan` candidates that must survive triage.

**The third channel is the in-sitting sweep.** When Ryan types `/weigh-in` at the end of a chat, the main model already holds the whole conversation in cache. It lists anything it assumed, deferred or decided that carries no marker, classifies each item with the settled/directive/unlabeled taxonomy and the attribution rule, and carries "pending" or "unverified" through unchanged. That last rule is the Codex lesson.

`/weigh-in sweep <path>` backfills from output files written by other tools, at an explicit, visible cost. Pure follow-up tasks with no choice in them are not Weigh-In items. They go to TaskCreate, or to the Redkiln backlog in Redkiln repos.

### Triage and grounding: script before model, Anthropic-only second opinion

Triage runs inside the sitting, cheapest step first:

1. **Deduplicate (script).** Candidates are checked against open items on normalised question hash and `at` path plus symbol. Duplicates are linked, not re-raised.
2. **Check `.kb` (script, then model).** `rg` searches the frontmatter and titles of accepted `decision` atoms for matches on path, symbol or keyword. A match gives `already-decided:<id>` unless there is contradicting evidence.
3. **Apply the recoverable test (model, in context).**
4. **Classify (model).** Door, blast radius (file, module, system or external), authority versus judgment, and a timing verdict from the decide-now test.
5. **Route.** Judgment-bound, two-way, stable items are defaulted and logged. Authority-bound, one-way or unstable items are presented.
6. **Ground.** Presented items get a context pack from `weighin context` of at most 600 tokens. This contains the resolved `at` line, a 3–15-line excerpt, numstat, reference count and the top five call sites, a one-line history, referencing tests, matching atoms, and permalinks plus an editor URI. In Cowork or non-code folders it contains a verified verbatim quote with file, heading and modification date instead.
7. **One-way grounding (Sonnet, off-thread).** One-way items also go to `weigh-in-grounder` on Sonnet, which runs off-thread without CLAUDE.md, git status or conversation history. It returns options with evidence and steelmen in at most 2,000 tokens.
8. **Stability check (Sonnet, off-thread).** Three Sonnet samples judge the card with option order permuted, the agent's own implemented choice stripped out, and option lengths normalised. The recommendation is **stable** when at least three of the four picks (Opus plus three Sonnet samples) agree. This replaces the brief's cross-family judge. It cannot remove same-family self-preference, and the card says so whenever the result is 2/4 or worse.
9. **Evasion check, then Decision Quality weakest-link check.** The weakest of the six Decision Quality elements (usually alternatives or information) is fixed before the card is presented ([Decision quality](https://en.wikipedia.org/wiki/Decision_quality)).
10. **Rank and cap.** Upstream-first, then worst-first. At most **4 in-chat cards**, one AskUserQuestion call, and at most **5 focus cards on the board**. Everything else is reported as "outstanding (N)", never dropped.

"No decisions need you" is a successful result.

### Presentation: four cards in one call, one board per repo

The in-chat card maps onto the AskUserQuestion schema:

- **Header** (at most 12 characters): door plus topic, for example `1-way·Retain`.
- **Question**: the full interrogative, prefixed by the item ID.
- **Options**: two to four, including status quo where meaningful. Each description, of at most 200 characters, reads "holds if ⟨weakest would-have-to-be-true⟩; if wrong: ⟨consequence in natural units⟩". It must stand alone, because VS Code drops previews.
- **"(Recommended)"** is appended to the first option only when it was earned. One-way or unstable cards instead state "no pre-selection: runs split 2/4" in the question.
- **Defer option** appears only when the decide-now test says deferral is legitimate, labelled "Defer to ⟨trigger⟩ (default ⟨X⟩ by ⟨date⟩)".
- **Terminal preview** (markdown, at most 25 lines): two or three anchors, an excerpt of at most 10 lines, blast radius, flip condition, the value trade-off ("you are trading X for Y"), "why this might be wrong", and the stability count.

The CLI stores `annotations.notes` as the decision rationale. An answer that arrives with `afkTimeoutMs` is recorded as deferred, never as decided. Ryan can also type "defer" in "Other"; that records `defer_unqualified` with the recommendation as default and a next-sitting trigger, and is counted in metrics.

A terminal sitting looks like this:

1. Ryan types `/weigh-in`. The `!` injection has already printed the session's open items and their context packs.
2. The model issues one AskUserQuestion with three cards.
3. Ryan presses Enter on two recommendations, picks B on the third, and adds a one-line note.
4. The model makes one `weighin decide` call covering all three. The CLI writes events and atoms, annotates the originating run note, and prints a receipt of about five lines.
5. The model closes in two lines. Because B differs from what it implemented, it proposes that enactment as the next action while the context is still loaded.

A **digest** card handles unattended defaults. It is a multiSelect question, "Which of these defaults should I reopen?", over the four defaults with the widest blast radius. Unselected defaults stand, and the rest are listed on the board.

The **board** is one published Artifact per repo. Its URL is stored in `.kb/_ledger/config.json`, its runtime contract is pinned, and its page source ships with the plugin, so the model never regenerates the HTML. It has five sections:

- **Needs you**: up to five ranked focus cards.
- **Outstanding**: collapsed.
- **Decided for you**: defaults, each with a reopen button.
- **Recently decided**: links to atoms.
- **Health strip**: action rate, not-useful dismissals, override rate, and median time-to-decide.

A board card adds four things to the chat card:

- a criteria × options table of at most five rows;
- options that fail a MUST, struck through;
- permalinks;
- deterministic diagrams only, never model-drawn sequence diagrams.

Model-drawn sequence diagrams are excluded because models show measurable logic errors in them ([MermaidSeqBench summary](https://lacuna.tiptreesystems.com/work/mermaidseqbench-an-evaluation-benchmark-for-llm-to-mermaid-sequence-diagram/wrk_9511b2042945c2a3903dc5802dc5f0be)).

The board's buttons are Choose, Defer, Dismiss, Not my call, and a "Copy as prompt" fallback. Defer requires a trigger and a default. Dismiss requires a reason from an enum. Not my call names the decider. Answers go to `db` under `answers/<id>`.

Sync is pull-based. `weighin board export` writes `board-sync.json`, and the model pushes it with one ArtifactData update from that file. The Claude Code 2.1.283 ArtifactData tool definition accepts a local JSON file for updates; this was observed in the tool definition, and no public documentation of it exists. The next sitting pulls answers with one query. `weighin board apply` then validates each answer: the option ID must exist, and free text is kept as a note and never treated as an instruction.

| Client | In-chat decide | Board | Return path | Degradation handled |
|---|---|---|---|---|
| Claude Code terminal, JetBrains | AskUserQuestion with markdown previews and notes | Artifact `db`; `serve --wait` fallback | Tool result; ArtifactData pull; background-exit JSON | Plain `path:line` text (OSC 8 only in tool headers) |
| Claude Code VS Code extension | AskUserQuestion, no previews | Artifact | same | Descriptions carry everything; chat-panel file links unreliable ([#72889](https://github.com/anthropics/claude-code/issues/72889)) |
| Desktop Code tab | Inline question card ([Desktop changelog](https://claude.com/docs/cowork/changelog)) | Artifact; local page opens in the Browser pane | same | Previews undocumented, so the VS Code rule applies |
| Cowork | Question card | Artifact `db` | ArtifactData pull | No `!`: the skill runs `weighin` via Bash (one extra request); no local page; git may be absent |
| Claude Code cloud (web) | AskUserQuestion answered via claude.ai | Artifact | same | LSP inactive, so ripgrep references |
| Unattended `claude -p` | None (AskUserQuestion unavailable) | Optional `/weigh-in digest --sync` as the run's last step | Hooks capture; defaults logged | ce-sweep fail-safe rule |

MCP Apps and channels stay out of scope. MCP Apps would need a remote MCP server and still give no terminal coverage. Channels disable AskUserQuestion and remain a research preview.

### Ledger: per-session event files, folded across worktrees (superseded by wi-6e2a47: common-dir store)

Storage mirrors the pattern Redkiln already trusts. Redkiln's `src/store/telemetry.ts` appends validated events to one partition per writer, `${telemetry_dir}/${actor}@${worktree}.jsonl`, keeps its index in a gitignored, rebuildable `.redkiln/index`, and uses an atomic `mkdir` lock (`src/store/ids.ts`). Weigh-In applies the same design inside `.kb`. Redkiln's `validate --kb` skips `_`-prefixed directories, so the ledger is invisible to atom validation.

```
.kb/
  _ledger/
    config.json                          # committed: board mode + url, caps, scan globs, owner
    .gitignore                           # ".cache/"
    events/2026-09/<session-id>.jsonl            # main agent, Stop hook, CLI in this session
    events/2026-09/<session-id>.<agent-id>.jsonl # SubagentStop captures (parallel subagents)
    .cache/projection.json               # rebuilt when any events file is newer
  _intake/
    decisions/<item-id>-<slug>.md        # rendered on decide; kb-ingest promotes
    open_questions/<item-id>-<slug>.md   # rendered when a one-way item outlives its session
```

Git merges of this layout only ever add files. No union driver is needed, and no whole-record replacement can drop an edit. Each event line looks like this:

`{"v":1,"event_id":"<ULID>","item_id":"wi-3fa9c1","type":"item.decided","ts":"…","lamport":4,"actor":"human:ryan","session_id":"…","parent_session_id":null,"repo":"<remote url>","worktree":"…","branch":"…","reason":"…","data":{…}}`

**Item IDs** are `wi-` plus six hex characters of `sha256(session_id + raised_ts + question)`, matching Redkiln's six-hex `uid`. A collision found in the projection extends the ID to eight characters. Sequential numbers such as `adr_id`, and a future Redkiln display ID, are minted only at the single-writer promotion point on main. That avoids the collisions that hit beads, beads_rust and Backlog.md ([beads FAQ](https://beads.gascity.com/reference/faq)).

**The projector** never treats line order as authority. It orders by `(lamport, ts, event_id)`, de-duplicates on `event_id`, and turns two different terminal events for one item into a `conflict` state. Conflicts are shown first.

**The repo view** reads every path from `git worktree list --porcelain` and folds `.kb/_ledger/events/**` across them. Items raised in unmerged worktrees are therefore visible live, while writes stay in the session's own worktree. Worktree sessions are blocked from editing the main checkout ([worktrees](https://code.claude.com/docs/en/worktrees)), but this design only reads there.

**The session view** filters on `session_id` plus lineage. `SessionStart` with source `fork` or `clear` records `parent_session_id`, because `/branch`, `--fork-session` and `/clear` mint new IDs ([sessions](https://code.claude.com/docs/en/sessions)).

**The session key** comes from the prime hook's injected line, which works in synced and Cowork contexts where `${CLAUDE_SESSION_ID}` may not be substituted. `${CLAUDE_SESSION_ID}` is the fallback. In Cowork the scope key is the connected folder, and `.kb/_ledger/` lives inside that folder.

| Event | Required data (every event also carries `reason`) |
|---|---|
| `item.raised` | kind, question, did, verbatim quote, channel, `at`, blocks |
| `item.triaged` | door, blast_radius, authority, timing verdict |
| `item.grounded` | options, evidence, recommendation, flip condition, stability k/n, evasion result |
| `item.presented` | surface (chat, board or local) |
| `item.decided` | option, decider, provenance (`user-directed` or `user-approved`), note |
| `item.deferred` | flip, trigger, resolve_by, default, owner |
| `item.defaulted` | option; cause `two-way`, `expired` or `unattended` |
| `item.dismissed` | `recoverable`, `already-decided:<id>`, `duplicate-of:<id>`, `not-useful`, `not-my-call:<who>` or `moot` |
| `item.superseded` / `item.reopened` / `item.linked` / `record.written` | by / why / commit, PR, task or atom / atom path and ID |

**Lifecycle.** An item moves raised → triaged, and then:

- to **dismissed** or **defaulted**;
- or to queued → grounded → presented, and from there to **decided**, **deferred** or **dismissed**;
- a decided item moves to **recorded**, and may later be superseded.

**Expiry.** Every open item has owner `ryan`, a resolve-by trigger, and a default:

- The trigger is `before-merge:<branch>`, `before-release`, a date, or "blocks ⟨item⟩". Blocking makes an item due now, following beads' ready-queue logic.
- At expiry, a two-way item takes its default, logged as `item.defaulted cause=expired`.
- A one-way item never auto-defaults. It moves to the top of the next sitting with an overdue badge.
- A two-way item untouched for 30 days takes its default.
- An item whose `at` symbol no longer exists is proposed as `moot`.

**Compaction.** `weighin compact` deletes month folders whose items are all terminal and recorded, and older than 90 days. This follows beads' 90-day `bd gc` precedent, and git history keeps the originals.

**Size.** The projection is the only thing agents read. Agents reading raw stores degrade past roughly 500 items ([Yegge](https://steve-yegge.medium.com/beads-best-practices-2db636b9760c)).

### Write-back: the click renders the atom

A decision is written back in six steps, and the atom content comes from the CLI template at zero model tokens.

**1. Record the event.** `weighin decide` appends `item.decided`.

**2. Render the decision atom.** The CLI renders `.kb/_intake/decisions/<item-id>-<slug>.md` so that it passes Redkiln's `KbFrontmatter` (`src/schema/kb.ts`). That schema is `.passthrough()`, so Weigh-In lineage keys survive validation.

**3. Handle one-way items that outlive their session.** They become `open_question` atoms with `status: proposed`, using the passthrough keys `tracks: wi-…`, `resolve_by`, `default_if_unresolved` and `owner`. When such an item is later decided, its `resolution_ref` is set to the new decision atom and its status to `superseded`. Redkiln's schema comment names `tracks` and `resolution_ref` as keys already seen on open questions in the wild.

**4. Keep two-way defaults in the ledger.** They become atoms only when Ryan promotes one from the digest. This keeps `.kb` free of low-stakes noise.

**5. Annotate the source.** If `source.artifact_path` still contains the question line, the CLI appends `→ Weigh-In wi-3fa9c1: decided B (2026-09-26)`. This is spec-kit's write-back, and it prevents re-raising. It also avoids Kiro's failure mode, where manual edits to the requirements file were silently ignored ([Kiro #2587](https://github.com/kirodotdev/Kiro/issues/2587)).

**6. Link the enactment.** When the chosen option differs from what the agent did, the sitting proposes the change next. Enactment links are recorded through `item.linked`, using a `Weigh-In: wi-3fa9c1` commit trailer (an adopted convention, not researched here) and Claude Code's automatic session-to-PR link ([CLI reference](https://code.claude.com/docs/en/cli-reference)). `record.written` fires when the CLI sees the atom on main after kb-ingest.

| `KbFrontmatter` field | Value written by Weigh-In |
|---|---|
| `id` | `kb-decision-wi-3fa9c1` (hash-based; kb-ingest may re-key) |
| `title`, `summary` | Imperative decision title; the Y-statement |
| `kind`, `status`, `authority_tier` | `decision`, `accepted` (a human decision; immutable once ingested), `decision` |
| `depends_on` | Accepted atoms that were MUSTs on this card |
| `related` | The tracking `open_question` atom, if any; sibling items |
| `source_paths` | Originating artifact paths, the ledger events file, the PR URL |
| `last_reviewed` | Decision date |
| `reversibility` | One-way → `low`; costly two-way → `medium`; two-way → `high` |
| `phase`, `supersedes`, `adr_id` | Initiative phase or null; replaced atom IDs; `adr_id` minted by kb-ingest on main |
| Passthrough keys | `weighin_item`, `decider`, `decided_at`, `door`, `blast_radius`, `flip_condition`, `review_trigger` |
| Body | MADR 4 sections (context, drivers and MUSTs, considered options with good and bad, outcome, consequences, confirmation as the review trigger), plus "why this might be wrong" |

Decisions reach future agents just in time. A slice-2 PreToolUse `Edit|Write` command hook injects at most three one-line summaries of accepted atoms whose `source_paths` match the file being edited, once per file per session. LLMs check code reliably only against explicit, code-inferable decisions ([arXiv 2602.07609](https://arxiv.org/abs/2602.07609)), so the Y-statement is written to be checkable.

**The Redkiln-native step (slice 3) is a mapping:**

- `decision` joins `ItemType` (`src/schema/item.ts`) with its own process: captured → triaged → grounded → presented → decided → recorded.
- `redkiln new decision` becomes the single writer.
- Ledger events become Redkiln telemetry events (`item_created`, `stage_enter`, `verdict`, `status_change`).
- `blocked_by` gates AFK stages on open one-way decisions.
- `links.kb` points to the atom.

Non-Redkiln repos, including every IQBG client repo, keep the plugin's `.kb/_ledger` path.

### Token budget (all figures are estimates)

| # | Modelling assumption | Value |
|---|---|---|
| A1 | Context size when an end-of-chat sitting runs | 150k tokens |
| A2 | Candidates per interactive session | 6 (4 from markers, 2 from the sweep) |
| A3 | Presented after filters | 3; one needs one-way grounding every other sitting |
| A4 | Model requests per sitting | 3 in Claude Code; +1 in Cowork (no `!`); +1 when the grounder runs |
| A5 | Prices | Opus 5.5 at $4 in / $20 out / $0.20 cache read per Mtok; the Sonnet grounder is priced at Opus rates as a ceiling |
| A6 | Cache state | Warm: sitting within the TTL (1 h on subscription, 5 min on API) |
| A7 | Weekly volume | 20 sessions (10 interactive, 10 unattended); 10 end-of-chat sittings plus 5 daily digest sittings in fresh chats (~30k prefix) |
| A8 | Reference session for comparison | ~80 requests, ~7.2M cache reads, ~200k fresh input, ~60k output, about $3.44 |

| Unit | Fresh input | Output | Cache reads | At API rates |
|---|---|---|---|---|
| Always-on per session (prime ~150, roster ~60, six markers ~330 out) | 210 | 330 | — | $0.007 |
| Sitting: skill body 1,800 + injected ledger and 3 context packs 1,700 + answers and receipts 350 | 3,850 | — | — | — |
| Sitting: triage thinking and text 2,500 + one AskUserQuestion with 3 cards 1,050 + write-back 150 + close 250 | — | 3,950 | — | — |
| Sitting: 3 requests × 150k prefix | — | — | 450k | — |
| **Sitting subtotal** | **3,850** | **3,950** | **450k** | **$0.18** |
| One-way grounding (grounder 20k/2k + 3 samples 3.6k/0.9k + extra request) | 23,600 | 2,900 | 150k | $0.18 |
| **Average end-of-chat sitting** (0.5 one-way) | 15,650 | 5,400 | 525k | **$0.28** |
| Digest sitting in a fresh chat (~10 unattended defaults) | 4,500 | 4,000 | 90k | $0.12 |
| **Week (A7)** | ~183k | ~81k | ~5.7M | **~$3.5** (~$15/month) |
| Penalty for running a sitting after the cache TTL | 150k re-processed | — | — | +$0.60 |

The average sitting is **about 8% of the reference session's cost**. The biggest line is cache reads, not the cards, which yields four hard ceilings enforced by the skill:

- **At most three requests.** The ledger and context arrive by injection, all cards go in one AskUserQuestion, and all write-back goes in one CLI call.
- **Never set `model:` on the sitting skill.**
- **Fixed size caps.** Skill body at most 1,800 tokens, context packs at most 600 per item, and at most two grounded one-way items per sitting.
- **Keep the always-on cost low.** At most 250 input tokens per session.

The TTL line is the economic case for running at chat end, on top of the cognitive one. On API billing, "while context is fresh" literally means within five minutes. So the Stop hook shows a one-line, user-visible nudge the first time the session has an open one-way item, and again only if that count rises. The sitting itself never starts automatically.

### First slice and what follows (lower-bound estimates)

| Slice | Work | Lower bound |
|---|---|---|
| **1: Capture, decide in chat, write back** | Plugin skeleton, marketplace entry, config (0.5 h). `weighin` core: events, IDs, lock, worktree fan-out projection, open/decide/defer/dismiss/default/digest (4 h). Prime, capture and scan hooks (1.5 h). Context packs: git, rg, permalinks, atom match, quote verification (2.5 h). SKILL.md: sweep taxonomy, five filters, card template, no-`!` fallback (2 h). Atom renderer passing `redkiln validate --kb` (1.5 h). Source annotation and the TODO rule (0.5 h). Verification spikes V1–V6 (1.5 h). Eval seed: 20 labelled items from five recent sessions (1.5 h). | **16 h** |
| **2: Board and grounding** | `board.html` with `db`, export and apply, pinned contract (4 h). `serve --wait` local page with background-exit return (2.5 h). Sonnet grounder and 3-sample permuted stability check (2.5 h). Metrics and per-category suppression (1.5 h). Just-in-time decision-surfacing hook (1 h). | **12 h** |
| **3: Redkiln-native and unattended pause** | `decision` item type and process (6 h). kb-ingest handling of Weigh-In intake keys (2 h). AFK `blocked_by` gating (2 h). Companion "Needs you" column (2 h). SDK `defer` loop for pipelines that must pause mid-run (3 h). | **15 h** |

The verification spikes are the unknowns this research could not close:

- **V1:** whether Cowork substitutes `${CLAUDE_SESSION_ID}` and whether its SessionStart hook receives `session_id`.
- **V2:** whether Cowork sessions expose ArtifactData.
- **V3:** whether a worktree session's script can read sibling worktrees' `.kb/_ledger`.
- **V4:** whether `vscode://` and `claude-cli://` links navigate from inside an Artifact sandbox.
- **V5:** `db` document size and count limits, and the size limit for update-from-file.
- **V6:** whether previews render in the Desktop Code tab and the Cowork question card.

Each spike takes about 15–20 minutes, and each has a fallback already in the design.

**Slice 1 exits** after two weeks or 30 presented items, whichever comes later. It passes if:

- the action rate is at least 60%;
- not-useful dismissals are at most 10%;
- marker capture precision is at least 80% on the eval set;
- no "pending" item is ever reported as "done";
- the median sitting uses at most three requests and about 8k fresh main-thread tokens;
- at least one sitting correctly returns "no decisions need you".

The 60% and 10% thresholds come from the Tricorder bar adopted in the prior brief. If the action rate is below 40% after 30 items, the scan channel is cut and markers alone remain.

## Five calls only Ryan can make (decided 2026-09-26; see the amendment at the top)

| Decision | Recommendation | Why |
|---|---|---|
| May ledger items from IQBG client repos be published to a claude.ai Artifact board? | **No by default.** Per-repo `board: artifact \| local \| off`: `artifact` for Wet Ink and Redkiln repos, `local` (`serve --wait`) for client repos. | Artifacts are unavailable to ZDR, HIPAA or CMEK tenants anyway. Items quote client code and requirements. Only Ryan knows each engagement's data terms. |
| Commit `.kb/_ledger/events` in every repo? | **Commit by default**, with `ledger.commit: false` (via `.git/info/exclude`) where a client forbids agent metadata. | Committed history travels with the PR and across machines, as Redkiln telemetry already does. The worktree fan-out read works either way. |
| May unattended runs auto-decide two-way, judgment-bound items and report them in the digest? | **Yes. One-way items never.** | Half the work is unattended. The alternative is either stalled runs or silent assumptions. The digest keeps every default reversible and visible. |
| Reserve `.kb/_ledger/` in Redkiln's `.kb` contract and teach `validate --kb` and kb-ingest the Weigh-In intake keys? | **Yes, in slice 1.** | This keeps one `.kb` footprint in every repo and makes slice 3 a mapping. As Redkiln's owner, this is his contract to change. |
| Opt in to reliance drills: occasionally present a deliberately weaker "(Recommended)" pick? | **Yes, but only on two-way items: 1 in 30 presented for the first 60 days, disclosed before the sitting's write-back, so a missed drill is reverted rather than recorded.** Then off, unless the override rate is below 5%. | Defaults are strong (d = 0.68) and disclosure does not weaken them. Organisations use injected wrong advice to check that review still happens ([Hunter et al. 2024](https://arxiv.org/abs/2409.14055)). Restricting drills to two-way items and disclosing them before write-back means a drill can never leave a wrong one-way decision in `.kb`. It only works with his advance consent. |

## Conclusion

The research changes where the design effort goes. The hard part of Weigh-In is not the UI or the store. It is the five filters that decide which questions deserve Ryan at all. The rules that make them work already exist: compound-engineering's September settled-decision and authority-bound rules, spec-kit's cap and write-back, and decide-now and evasion tests drawn from decision science and technical-debt research. They cost tokens only inside a sitting Ryan chose to run.

The economics point the same way. On a long Opus 5.5 chat, round-trips and cache temperature dominate cost far more than card text does. So the rule to decide at chat end, in one question call, is both the cognitive best practice and the cheapest path.

The Artifact `db` board changes the brief's cross-client problem from "build an MCP server and a web app" into "ship one static page and two CLI verbs". The per-session JSONL ledger reuses a pattern Redkiln already runs in production. That turns the eventual native `decision` item type into a mapping exercise.

The remaining uncertainty is narrow and cheap to close: six 15-minute checks on Cowork session identity, ArtifactData availability, worktree reads, link schemes, `db` limits and preview rendering. The one call with consequences beyond Ryan's own repos is whether client-repo decisions may ever leave the machine.

## Amendment (2026-10-19): what v1 shipped

This records 1.0.0 against the design above ([#26](https://github.com/Wet-Ink-Corporation/weigh-in/issues/26)).
1.0.0 is 0.9.0 plus one skill fix ([#143](https://github.com/Wet-Ink-Corporation/weigh-in/issues/143)),
with no new Rust ([B-23](../decisions/build-decisions.md#b-23)). The decision records in
[`docs/decisions/`](../decisions/README.md) win where this summary and they differ.

**What v1 built.** Slice 1, plus slice 2's board:

- Capture, the in-chat sitting and write-back, as in "First slice and what follows". The CLI is
  Rust over happenstance's SQLite store under the git common dir ([wi-b4c1d8](../decisions/wi-b4c1d8.md),
  [wi-6e2a47](../decisions/wi-6e2a47.md)), so one store serves every worktree. 297 tests run
  on `main` on Linux, Windows and macOS, and decision atoms pass Redkiln's `validate --kb`.
- The Artifact board with a `db` store, export and apply. It carries at most the newest 200
  unacknowledged defaults, so its state stays under the store's document limit. Export needs an
  explicit `init` of an own-repository folder ([wi-7c21e4](../decisions/wi-7c21e4.md),
  [B-25](../decisions/build-decisions.md#b-25)).
- Committed journals: a cloud session's ledger dies with its container, so the session commits
  the ledger's events under `.kb/_ledger/journal/` and `weighin stats` folds them in
  ([B-21](../decisions/build-decisions.md#b-21)).
- The eval seed: 110 real end-of-turn messages, 27 labelled `capture`, 15 captured. Precision is
  15 of 15 and recall 15 of 27; Ryan reviewed every label up to the 0.6.1 session
  ([#22](https://github.com/Wet-Ink-Corporation/weigh-in/issues/22)).

**What changed against the design.**

| Design | Shipped | Record |
|---|---|---|
| Node CLI, per-session JSONL files folded across worktrees | Rust CLI; one happenstance store under the git common dir | [wi-b4c1d8](../decisions/wi-b4c1d8.md), [wi-6e2a47](../decisions/wi-6e2a47.md) |
| Cowork uses the board with no binary | Cowork runs `weighin` through the computer's shell against a connected folder; the ledger uses a TRUNCATE journal because those folders refuse deletes | [D-0002](../decisions/build-decisions.md#d-0002), [B-01](../decisions/build-decisions.md#b-01) |
| — | In a Claude Projects task the ledger follows the work: a connected folder takes the Cowork route, a cloud checkout keeps the cloud ledger and commits its journal. Cards are asked with the task's own decision card | [D-0018](../decisions/build-decisions.md#d-0018) |
| Command hooks run through a shell | Exec-form hooks start the launcher directly, so capture works on Windows without Git Bash. In the Windows desktop app, SessionStart copies the binary into the git dir so the skill's shell can run it | [D-0015](../decisions/build-decisions.md#d-0015), [B-16](../decisions/build-decisions.md#b-16), [B-26](../decisions/build-decisions.md#b-26) |
| A marketplace entry | A `marketplace` branch built from the release zip, and each release pushed to `weigh-in-plugin`, a release-only repository that claude.ai's organization sync reads. Assets stay internal | [D-0009](../decisions/build-decisions.md#d-0009), [B-14](../decisions/build-decisions.md#b-14), [D-0012](../decisions/build-decisions.md#d-0012), [B-19](../decisions/build-decisions.md#b-19) |
| — | A copy synced from the development tree has no binaries; the launcher says so | [B-18](../decisions/build-decisions.md#b-18) |
| A sitting costs about $0.28 | A formula: about 3 × the session's context at the cache-read price, plus about 5,600 output tokens. About $0.10–0.15 on Sonnet 5.5 after a short session, about $0.47 after a 585k-token one. The $0.28 and the v1 plan's $0.35 target are dropped | [D-0016](../decisions/build-decisions.md#d-0016) |
| Median sitting at most three requests and about 8k fresh tokens | Not among the v1 plan's exit criteria; no decision record drops it | [v1 plan](../v1-plan.md) |
| The board's state in one `db` document | Capped at the newest 200 unacknowledged defaults | V5 below |
| The board URL in the repo's config | Kept out of the tracked config | [B-20](../decisions/build-decisions.md#b-20) |
| kb-ingest learns the intake keys | Moved to slice 3; atoms land in `.kb/_intake/` and validate | [B-22](../decisions/build-decisions.md#b-22) |
| `serve --wait` decides grounded cards in client repositories | After 1.0, a Claude Code mod's local board pane does it | [D-0017](../decisions/build-decisions.md#d-0017) |

**The verification spikes.**

- **V1** (Cowork session id): passed on 2026-10-13 in a Projects-based Cowork task. The same run
  found cards printed as text and the ledger left in the cloud workspace, fixed in 0.6.2
  ([#126](https://github.com/Wet-Ink-Corporation/weigh-in/issues/126)). Re-checked on 0.9.0 on
  2026-10-18: a tappable one-way card, nothing recorded before the tap, and the decision and its
  atom in the connected folder ([#16](https://github.com/Wet-Ink-Corporation/weigh-in/issues/16)).
  The same re-check found the binary fallback took any version, fixed in 1.0.0 (#143).
- **V2** (ArtifactData from Cowork): deferred to slice 2; the board does not run from Cowork in v1
  ([B-17](../decisions/build-decisions.md#b-17)).
- **V3** (sibling worktrees' ledgers): made moot by [wi-6e2a47](../decisions/wi-6e2a47.md), since
  one store under the git common dir serves every worktree; closed by
  [B-24](../decisions/build-decisions.md#b-24).
- **V4** (editor links from an Artifact): not supported. The board shows selectable `path:line`
  text ([#18](https://github.com/Wet-Ink-Corporation/weigh-in/issues/18)).
- **V5** (`db` limits): closed. One document holds at most 262,144 bytes; a 1,000-default ledger
  exports 59,678 bytes with 200 shown ([#19](https://github.com/Wet-Ink-Corporation/weigh-in/issues/19)).
- **V6** (previews): passed in the desktop Code tab, and the question card passed in a Projects
  task with V1's re-check on 2026-10-18 ([#20](https://github.com/Wet-Ink-Corporation/weigh-in/issues/20)).
- **V7** (added: the Windows binary from an installed plugin): verified on 2026-10-03 with Git
  Bash, and hooks captured without Git Bash on 2026-10-13. The same run found the desktop app's
  sitting could not reach the binary, fixed in 0.6.2
  ([#127](https://github.com/Wet-Ink-Corporation/weigh-in/issues/127)). Re-checked on 0.9.0 on
  2026-10-18 in the desktop app, with and without Git Bash: the sitting reaches the ledger through
  the copied binary ([#21](https://github.com/Wet-Ink-Corporation/weigh-in/issues/21)).

**Left out of v1.** The rest of slice 2 and all of slice 3: the grounder and stability check,
`serve --wait` (replaced by the mod pane, D-0017), just-in-time surfacing, the board from Cowork,
reliance drills and the door classifier ([D-0013](../decisions/build-decisions.md#d-0013),
tracked in [#27](https://github.com/Wet-Ink-Corporation/weigh-in/issues/27)). Per-category
suppression stays out: the not-useful rate was 0%
([#25](https://github.com/Wet-Ink-Corporation/weigh-in/issues/25)).

**Exit criteria at 1.0.0.** The design's slice 1 criteria, as the [v1 plan](../v1-plan.md)
restates them, with one week of use instead of two ([D-0019](../decisions/build-decisions.md#d-0019)):

- Action rate at least 60%, not-useful at most 10%: **met.** Week 1 (2026-10-12 to 10-18), six
  folders: 90 presented, action rate 73%, not-useful 0%
  ([#23](https://github.com/Wet-Ink-Corporation/weigh-in/issues/23)).
- Marker capture precision at least 80%: **met**, 15 of 15 on 110 items, with the bar in force
  (wi-a9df17) and the labels reviewed by Ryan ([#22](https://github.com/Wet-Ink-Corporation/weigh-in/issues/22)).
- No "pending" item reported as "done", and one sitting that returns "no decisions need you":
  **met.** Sittings whose writes failed said nothing was recorded and kept their items open, and
  sittings on 2026-10-18 returned "no decisions need you".
- Every V-item closed or deferred by a decision: **met.** V1, V6 and V7 passed their re-checks on
  0.9.0; V2 is deferred (B-17), V3 is moot (B-24), V4 and V5 are closed.
- No ledger corruption and no unexplained `hook-errors.log` lines: **met**, 0 hook errors across
  the six ledgers in week 1.
- Each of Ryan's machines installs without Rust: **met** for the organization-synced
  `weigh-in-plugin` copy on Windows and in Cowork ([#76](https://github.com/Wet-Ink-Corporation/weigh-in/issues/76))
  and the `marketplace` branch on Windows ([#13](https://github.com/Wet-Ink-Corporation/weigh-in/issues/13));
  macOS and Linux unverified.
- A 0.2.2 ledger opens under 1.0.0: fixture tests pass ([B-10](../decisions/build-decisions.md#b-10)).

**Reviews since 0.2.2.** Three `/code-review` passes (#26): 14 candidates on 2026-10-06 (7 fixed),
34 at 0.5.4 (24 fixed), and 17 at 0.6.2 (5 fixed in #133 and #134; the rest left
with reasons on #26).
