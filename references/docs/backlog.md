<!-- Exported from the Claude Doc https://claude.ai/code/artifact/6a5d3746-3d34-4c36-8776-2206b0aaf526 (revision 25) on 2026-10-19 as part of the project handover. The Doc is the original; this copy is frozen. Diagrams (2) could not be exported and show as [embedded content] placeholders; open the Doc to see them. -->

# Redkiln Backlog: what the plugin should entail

Oct 17, 2026 · @Ryan

## Summary

Redkiln Backlog should be a convention plus the tooling that enforces it: one meaning for each GitHub primitive, one label set, an issue body that is the implementation contract, and a board and roadmap the plugin builds and keeps current. Most of it already exists in redkiln-rs's `docs/BACKLOG.md` and anneal's `dag`, so the work is lifting and generalising, not inventing.

- **Issues.** Types carry the kind of work, sub-issues the hierarchy, native dependencies the order, issue fields (generally available since July 2026) priority, effort and dates, and labels only status, gates, area and source.
- **The body.** Each type has a form and a fixed set of headings with stable ids: requirements, acceptance criteria, test cases mapped to AC, a task checklist. Agents talk in the issue with a few typed comment kinds.
- **Board and roadmap.** Decided: we build them ourselves as projections over Issues, on `redkiln-view`, rather than use GitHub Projects, which needs a permission no automation removes. Generated roll-ups in Initiative and Epic bodies keep the issues list readable for people without the suite.
- **Grooming.** The first release is triage, refinement by example mapping, breakdown, a ready-lint, dependency analysis, hygiene sweeps and a closing summary; the traceability check itself is Review's. Prioritisation and forecasting wait, so Backlog does not become a product of its own.
- **Calls.** All five decided on 2026-10-19, as recommended. A and C are one-way: A (one suite-wide label set) is hard to undo, and C is answered jointly with the pressure test's call E.

## What the repos already do

Three of the repos already run the model Ryan describes, and redkiln-rs has written it down as a contract. Backlog should start by lifting that contract, not by designing a new one.

| Repo | What it does today | Worth lifting |
| --- | --- | --- |
| redkiln-rs (`build/phase-0-to-1`) | `docs/BACKLOG.md` is a full ledger contract. It sets issue types Initiative, Epic, Feature/Task, Bug; at most three levels; ordering only through native blocked-by; and label families `area/*`, `source/*`, `status/*`, `needs/*`, `prio/*`, `size/*`. It adds a five-part body contract, a definition of ready, an AFK pick-up query and a fingerprinted filing rule capped at 5 issues a session. It ships an idempotent `apply-manifest.mjs` for bulk changes, issue forms in `.github/ISSUE_TEMPLATE/` and a `backlog-ledger` skill. It also sketches a Projects board but never built it, because the `project` token scope was not granted. | Nearly all of it: the contract, the forms, the skill and the manifest runner |
| anneal | `anneal dag` reads a GitHub tracker (sub-issues of an issue, plus `Blocked by #n` in bodies) and computes the ready frontier; `dag check --plan` compares a plan's tasks with the tickets; `orchestrate` fans out ready tickets. The plan lives in issue #1 with Release Features as sub-issues. | The dependency graph, the frontier and the plan-to-tickets check. It still parses `Blocked by` prose; it should read native dependencies. |
| assay | The whole plan is GitHub Issues with issue types (Initiative, Epics as milestones M0 to M6, Stories), sub-issues, native dependencies, custom fields and an `owner-action` label for work only Ryan can do. Weigh-In atoms anchor to issue URLs. | The `owner-action` idea and the atom-to-issue anchor |
| temper | Issues, sub-issues, `P0`/`P1`/`P2` labels (wi-7c3212), Markdown issue templates for bug, false positive and rule proposal. | Its domain templates (false positive, rule proposal) as examples of per-plugin forms |
| weigh-in | Milestone `v1.0` plus labels `verify`, `slice-2`, `release`, `gap`, `decision`. Deferrals are the one Weigh-In object that is really an item. | Deferrals that become issues when Backlog is present |
| redkiln (TS prototype) | `.bklg/` markdown items with typed frontmatter (type, parent, status, stage, `blocked_by`, automation HITL/AFK, severity, owner, links to PR and commits), stage artifacts beside the card (`spec.md`, `plan.md`, `report.md`), and a terminal board, HTML board and Mermaid graph derived from an index. | The item schema as the field list to map onto GitHub, the HITL/AFK flag, and the board renderers' ideas. Not the store. |

Two facts shape the design. First, every repo invented its own label set, and they overlap but do not match. Second, nothing anywhere automates GitHub Projects yet.

## The issue model

The plugin's core is one convention, written once in Core's SPEC and enforced by the plugin: what each GitHub primitive means. GitHub now has a native home for almost every piece, so labels can shrink to what nothing else carries.

### Which primitive carries what

| Concept | GitHub primitive | Rule |
| --- | --- | --- |
| Kind of work | Issue type: Initiative, Epic, Story, Task, Bug, Spike, Decision | Never a label. Org-level, so one set for every repo. |
| Hierarchy | Sub-issues | Initiative, then Epic, then Story/Task/Bug. At most three levels. A Story or Task is exactly one PR. |
| Order | Native dependencies (blocked by / blocking) | Never in prose. Cross-repo links are allowed, which the monorepo will need less but client repos will. |
| Priority, effort, dates | Issue fields: Priority, Effort, Start date, Target date | Generally available since 2026-07-02, with these four as defaults. They replace the `prio/*` and `size/*` labels redkiln-rs called interim. |
| State | Open/closed plus `status/*` labels, which set the Board's columns | Closed as completed or not planned is the end. Blocked is a dependency, never a label. |
| Gates and routing | `needs/*` labels | An agent never picks up an issue carrying one. |
| Ownership and origin | `area/*`, `source/*` labels; assignee | `source/agent` marks what an agent filed. |
| Release grouping | Milestones, optional | Only for a dated release. Initiatives carry outcomes. |
| Discussion | Comments | All talk about the work happens on the issue, as Ryan wants. |
| Closing link | `Closes #N` in the PR | The PR is the evidence the item is done. |

### The label set

One suite-wide set, shipped as a `labels.yml` the plugin applies idempotently. Each family has one rule, so an agent can check it.

| Family | Values | Rule |
| --- | --- | --- |
| `status/` | `triage`, `ready`, `in-progress`, `in-review` | Exactly one while open |
| `needs/` | `decision`, `repro`, `human`, `breakdown`, `spec` | A gate; zero or more |
| `area/` | one per crate or plugin | At least one |
| `source/` | `human`, `agent`, `audit`, `ci`, `review` | Exactly one |
| `mode/` | `afk`, `hitl` | Whether Loop may run it unattended, from the prototype's automation flag |
| `rk:` | reserved | Written only by Loop's tracker adapter (AC-507); a human edit is drift. Loop's permitted writes are `rk:*` plus `needs/human` (on park), `status/in-progress` (on claim) and `status/in-review` (on landing). Loop reads `mode/afk`. |

### The issue body is the contract

Each issue type gets an issue form and a matching body schema. The headings are fixed and the items carry stable ids, so a plugin can parse them, a test can cite them and a review can check them. A Story's body:

1. **Context**: the problem and why it matters, with evidence as `path:line` at a SHA.
2. **Requirements**: numbered `R1`, `R2`, each one testable sentence.
3. **Acceptance criteria**: `AC1`, `AC2` as checkboxes in Given/When/Then, each naming the requirements it covers.
4. **Test cases**: `TC1`, `TC2`, each naming its AC and the exact command or test path that proves it.
5. **Tasks**: a checklist of steps, ticked as the work lands. Anything bigger than a step is a sub-issue instead.
6. **Out of scope.**
7. **Links**: parent, spec file, decisions (`wi-` ids), related PRs.
8. **Provenance**: the last line, `<!-- redkiln: src=… id=… -->`, so a refiling finds its duplicate.

A Bug swaps Requirements for Steps to reproduce, Expected and Actual. An Epic carries Outcome, Scope, and the slice plan. An Initiative carries Outcome, Success measure and Target date. A Decision's form carries the question, the options, a recommendation, the door (one-way or two-way), the `wi-` id and how to answer. The item it blocks carries `needs/decision` and is natively blocked by the Decision issue, which is the rendezvous for the pressure test's call B.

The AC-to-test mapping is what makes the body useful during implementation: Bench's `build` reads it as the test list, Review checks that every AC has a passing test, and the issue's checkboxes tick as the PR lands.

### Where long specs live

The pressure test's call E (spec artifacts as committed files linked from the issue) fits under this, rather than against it. The body holds the contract: requirements, AC, test cases and the checklist, which everyone needs to see on the issue. A long design or spec goes in a committed file the body links, because it is reviewed in a PR and versioned with the code. GitHub caps a body at 65,536 characters, which a contract never reaches and a design sometimes does.

### Talking in the issue

Comments stay human-readable. Agent comments use a small set of kinds so they can be found and folded. The kinds are **claim** (I am starting this, on this branch), **progress** (what landed, what is next), **question** (with a Weigh-In id if it waits on Ryan), **finding** (evidence for a filed or duplicate issue) and **handoff** (where it stopped and why). Each ends with a hidden `<!-- redkiln: kind=… -->` marker. An edit to the body's contract after the item is ready also leaves an **amendment** comment saying what changed, because GitHub's edit history keeps only 100 edits and does not say why.

Issue and comment text is untrusted input to every agent that reads it, as redkiln-rs already rules.

## Board and roadmap: our own projections

Issues are the source of truth, and the board and roadmap are projections Backlog builds itself (Ryan, 2026-10-17). GitHub Projects is not used: it can be created through the API, but it needs the `project` scope for people and a stored app token or PAT for Actions, which no amount of automation removes ([views API](https://docs.github.com/en/rest/projects/views), [automating projects](https://docs.github.com/en/enterprise-server@3.22/issues/planning-and-tracking-with-projects/automating-your-project)).

### Why our own

- **No extra permission.** The views read only what every plugin can already read: issues, types, sub-issues, dependencies, issue fields, labels and closing PRs.
- **Works where the suite works.** Cowork, cloud sessions and offline (from the last snapshot), not only github.com in a browser.
- **Shows the suite's own state.** Open Weigh-In calls, Review verdicts, gates and Loop runs sit beside the items they belong to, which a GitHub Project cannot show.
- **One read model.** It is the `redkiln-view` kit crate (`crates/kits/redkiln-view`) the visualization study already proposed, so Backlog's views are two more view documents, not a new surface.

### The views

| View | Shows | Built from |
| --- | --- | --- |
| Board | Stories, Tasks and Bugs in columns Triage, Ready, In progress, In review, Done, grouped by Initiative; blocked cards marked with their blocker | `status/*` labels, open/closed, dependencies |
| Roadmap | Initiatives and Epics as bars from Start date to Target date, each with percent done; at-risk bars where open work outruns the date | Issue fields, sub-issue progress |
| Ready queue | What an agent or Loop picks next, by Priority | The definition-of-ready query |
| Needs Ryan | Everything waiting on a person: `needs/decision`, `needs/human`, open Weigh-In calls. It is Weigh-In's tab on the suite board, fed by these labels. | Labels, Weigh-In events |
| Hierarchy | The Initiative, Epic, Story tree with roll-up counts, for grooming | Sub-issues |
| Dependencies | The graph of blocked-by links, with the ready frontier and the critical path | Native dependencies (anneal's `dag`) |

### How it is built

[embedded content: Backlog's views · one read path, three surfaces]

Issue data and the suite's events meet in `redkiln-view`; the dashed line is the only write back, the generated roll-ups in issue bodies.

- **Surfaces.** Backlog's views are tabs on the one suite board Artifact, for when you are away or on your phone; client repos get no Artifact. They also show in the suite mod's pane in Claude Code, and as `backlog board` in the terminal with `--json` for scripts.
- **Freshness.** Each view is a snapshot and says so: it prints its as-of time and the repos it covers. A refresh is one GraphQL query per repo, with conditional requests so it stays inside rate limits.
- **Writes.** The views are read-only in the first cut. A change goes through the issue or a plugin command, and each card links to its issue.
- **For people without the suite.** `backlog sync` writes a generated roll-up into each Initiative and Epic body, between markers: a progress line and a Mermaid graph of its children, which GitHub renders natively. So the issues list alone still tells the story.

### What we give up

Drag-and-drop editing, Projects' iteration field and its insights charts, and a board colleagues can open on github.com. Iterations, if we need them, become an org issue field; charts belong in the views.

## Planning and grooming the plugin could support

The valuable activities are the ones where an agent does the tedious part and a person makes the call: triage, refinement, breakdown and hygiene. Prioritisation and forecasting are where a backlog tool turns into a product of its own, so they wait. suite-structure.md already warned that Backlog is the domain most likely to swallow the suite.

| Activity | What the plugin does | Who decides | Backlog release |
| --- | --- | --- | --- |
| Intake and triage | Proposes type, area and parent for a new issue; finds duplicates by provenance fingerprint; applies `status/triage` | A person sets `status/ready` | 1 |
| Definition of ready | `backlog lint #N` checks the body against its type's schema: every requirement has an AC, every AC a test case, no open blocker, no `needs/*` | The lint | 1 |
| Refinement by example mapping | Turns a story into rules, examples and questions; examples become AC and test cases; questions go to Weigh-In | Ryan answers the questions | 1 |
| Breakdown | Slices an Epic into one-PR Stories with dependencies, previewed as a manifest before anything is written | A person approves the manifest | 1 |
| Dependency analysis | Ready frontier, critical path, cycles, cross-repo blockers (anneal's `dag`, on native links) | Nobody: it is computed | 1 |
| Hygiene sweep | Stale `in-progress`, orphans with no parent, open children under a closed parent, label drift, missing fields | Agent fixes its own; flags the rest | 1 |
| Closing summary | Posts a closing summary on the issue. The traceability check (every AC ticked, a PR closed it, its tests exist) is Review's `/rk-review:trace` in CI. | Review, as a gate | 1 |
| Plan import | Turns an anneal plan, a spec-kit task list or the `.bklg` corpus into issues | A person approves the manifest | 1 |
| Status reporting | Weekly roll-up per Initiative from sub-issue progress, shown in the Roadmap view and posted as a comment on the Initiative | Ryan sets on track or at risk | 2 |
| Estimation | Proposes Effort with a reason; flags anything over one PR for splitting | A person accepts | 2 |
| Iteration planning | Fills an iteration (an org issue field) from the ready queue within capacity (Loop's lease) | A person commits | 2 |
| Prioritisation | Scores with WSJF or RICE as a suggestion beside the Priority field | Ryan | Later |
| Forecasting | Monte Carlo dates from throughput, as redkiln-rs did for M5 | Nobody, but it is a model | Later |
| Retrospective | Lessons from closed Initiatives become Knowledge atoms | Ryan | Later |

Backlog ships in the suite's second wave. As commands, its first release is roughly `/rk-backlog:setup`, `new`, `triage`, `refine`, `split`, `lint`, `next`, `sweep`, `import` and `board`/`sync`, each a skill over the one `backlog` CLI. Every bulk write goes through a manifest with a dry run first, which is the pattern redkiln-rs proved.

## Prior art and patterns

The agent-backlog tools all converge on the same four ideas: a spec before tasks, a computed ready queue, atomic claiming, and "discovered" work filed as it is found. None of them uses GitHub's newer native primitives (types, dependencies, issue fields, API-created views) as fully as Backlog could, which is where it can be better rather than another copy.

| Prior art | What it is | Pattern to take | What to leave |
| --- | --- | --- | --- |
| [CCPM](https://github.com/Ninegd/ccpm) (Claude Code PM) | Claude Code commands: PRD, then epic, then tasks, synced to GitHub issues; progress as issue comments | Issue comments as the audit trail; `/pm:next`; parallel streams per issue in worktrees | Local task files as the source with a sync step; labels for type; the `gh-sub-issue` extension, now native |
| [Spec Kit](https://docs.runmaestro.ai/speckit-commands) (GitHub) | specify, clarify, plan, tasks, analyze, checklist, then `taskstoissues` | `clarify` (targeted questions before planning), `analyze` (cross-artifact consistency), checklists that test the requirements themselves | One-way export: issues go stale against the task file |
| [Beads](https://mintlify.wiki/steveyegge/beads/introduction) | Git-backed tracker for agents | `bd ready`, atomic `--claim`, a `discovered-from` link for work found mid-task, JSON on every command | Its own store, which the suite decided against |
| Backlog.md, OpenSpec, BMAD | One markdown file per item in the repo (the prototype's own family, per its `backlog-as-code` page) | Stable ids, typed frontmatter, stage artifacts beside the item | The in-repo store |
| [Example mapping](https://www.cucumber.io/docs/bdd/example-mapping) (Cucumber) | Story, rules, examples, questions on cards | The `refine` skill's shape: rules become AC, examples become test cases, questions become Weigh-In calls, a pile of rules means split the story | Nothing; it is a technique |
| Linear | Hosted tracker (from memory, not checked today) | A triage inbox, cycles, project updates with on-track/at-risk health, initiatives over projects | Its own store |
| Shape Up (Basecamp) | Appetite, shaping, betting table, hill charts (from memory) | Appetite as a fixed budget on an Epic; hill-chart "figuring it out vs. making it happen" as a status for spikes | Six-week cycles as a rule |
| IssueOps and GitHub Actions | Slash commands in comments trigger workflows | Commands like `/ready` or `/split` in a comment, so Ryan can groom from his phone in GitHub itself | Heavy bots that rewrite issues silently |
| redkiln-rs `BACKLOG.md` | This suite's own ledger contract | Fingerprinted filing, the 5-issue cap, manifests with dry runs, "only a human sets ready" | Interim `prio/*` and `size/*` labels, now issue fields |

The classic grooming vocabulary still applies and is cheap to adopt as lint rules: INVEST for stories, a Definition of Ready and a Definition of Done per type, SPIDR for splitting (spike, path, interface, data, rules), and Given/When/Then for AC.

## How it composes with the rest of the suite

Backlog owns the work-item domain and nothing else; every other plugin reads and writes issues through its contract crate, so the conventions are enforced in one place.

[embedded content: Backlog's place in the suite · 5 consumers, 1 contract]

- **Bench** reads a Story's requirements, AC and test cases as its plan and test list, posts claim and progress comments, and ticks the checklist as it lands. Its skills already name catalog activities, so each Story can name the activity it runs.
- **Loop** picks from the ready queue and, on every Loop entry, records an issue snapshot as a `backlog.snapshot` event: number, `updated_at`, content hash and author association. It alone writes `rk:*` labels, and may also set `needs/human`, `status/in-progress` and `status/in-review`. The snapshot answers the Blueprint's caveat that Loop needs item content fixed at a hash.
- **Review** owns the traceability check (`/rk-review:trace` in CI): each AC in the closing issue maps to a test the PR adds or touches. Backlog only posts the closing summary.
- **Weigh-In** keeps decisions. A blocked item carries `needs/decision` and links to its Decision issue, which carries the `wi-` id. A deferral with a trigger becomes an issue when Backlog is present.
- **Knowledge** anchors atoms to issue URLs, as assay already does, and takes lessons from closed Initiatives.
- **Core** holds the item schema (the mapping above, not a file format) and the `backlog.*` events in the shared log; `redkiln-github` is the client.

Backlog's crates are `backlog-contract`, the library every plugin reads and writes items through, and the `backlog` binary. Both depend on the `redkiln-view` kit (`crates/kits/redkiln-view`), and Backlog's events are `backlog.*`.

The Blueprint's call 10 still holds: Backlog's GitHub side is opt-in by repo class, so client repos and Cowork folders that are not on GitHub never depend on GitHub Issues. Their local tickets are owned by Backlog's local mode: a file store under `.redkiln/backlog/` with the same fields as the issue body contract. Bench's `dag` and Loop read it through `backlog-contract`.

## Calls for Ryan and next steps

All five calls are decided: on 2026-10-19 Ryan chose the recommendation on each (Weigh-In wi-cef669, wi-5edd9b, wi-69aa12, wi-491ac0, wi-824f37). Each has a recommendation. A is one-way, because colleagues' repos will carry the labels, and C is one-way and is answered jointly with the pressure test's call E. B and E are two-way.

| # | Call | Door | Recommendation |
| --- | --- | --- | --- |
| A | Adopt one suite-wide label set and issue types (above, including Decision), replacing each repo's own | One-way | Yes. Migrate temper, weigh-in, anneal, assay and redkiln-rs with one manifest each. |
| B | Use issue fields for Priority, Effort and dates instead of `prio/*` and `size/*` labels | Two-way | Yes, now that they are generally available |
| C | The issue body holds the contract (requirements, AC, test cases, checklist); long specs are committed files it links | One-way, answered jointly with the pressure test's call E | Yes. This refines the pressure test's call E rather than replacing it. |
| E | First release scope: triage, refine, split, lint, next, sweep, import and the board; prioritisation and forecasting wait | Two-way | Yes |

**Decided.** Call D (primary board and roadmap) was decided by Ryan on 2026-10-17: our own views on `redkiln-view`, projected over Issues, and no GitHub Project.

### Suggested next steps

- [ ] Ryan decides calls A, B, C and E.
- [ ] Spike: one GraphQL read of a repo's issues with types, sub-issues, dependencies and issue fields, rendered as the Board and Roadmap view documents in `redkiln-view`.
- [ ] Write the item schema into Core's SPEC draft: types, label families, body headings per type, comment kinds, the roll-up markers.
- [ ] Lift redkiln-rs's `BACKLOG.md`, issue forms, `backlog-ledger` skill and `apply-manifest.mjs` into the plugin's first cut, generalised beyond one repo.
- [ ] Point anneal's `dag` at native dependencies instead of `Blocked by` prose.
