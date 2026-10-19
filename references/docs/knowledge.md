<!-- Exported from the Claude Doc https://claude.ai/code/artifact/9b950148-6984-42e9-87ec-484ce19b08b7 (revision 36) on 2026-10-19 as part of the project handover. The Doc is the original; this copy is frozen. Diagrams (1) could not be exported and show as [embedded content] placeholders; open the Doc to see them. -->

# Redkiln Knowledge: what the plugin should entail

Oct 18, 2026 · @Ryan

## Summary

Redkiln Knowledge should own the `.kb` format and everything between a producer writing an atom and a session reading it: lifecycle, validation, retrieval and injection under a budget. Three plugins already write atoms (100 across weigh-in, temper and assay) and two read them, but nobody promotes, indexes or retrieves them, so every atom sits in `_intake/` forever. The work is lifting the old TypeScript Redkiln's schema and rules into Rust, not inventing a store.

- **Atoms.** One markdown file per fact with typed frontmatter. Eight kinds: decision, constraint, ruling, finding, evidence, learning, term and open-question. Each carries a scope, anchors and, for standing decisions, the check that enforces it.
- **Lifecycle.** Producers write atoms straight to their final path with a status. Accepted bodies are immutable: change means supersede, and a wrong evidence line goes in an errata note. Only prose documents pass through `_intake/` for extraction.
- **Retrieval.** Lexical search over frontmatter and body plus explicit scopes, no embeddings in v1. Indexes are disposable projections, never committed.
- **Injection.** A small digest at SessionStart, more atoms just in time when a prompt or an edit touches their scope, and an explicit per-activity loader for Loop whose resolved atom-set digest enters Loop's memo key. Every injection is recorded so relevance can be measured.
- **Calls.** Seven, in the last section. Call C (the id format) and call B (where atoms live) are the hard ones to undo, because other repositories, trailers and issues will cite both.

## What the repos already do

The format exists and three repositories write it, but the only code that promotes, indexes or validates atoms is in the TypeScript Redkiln that redkiln-rs replaces. Knowledge starts from that schema and those rules.

| Repo | What it does today | Worth lifting |
| --- | --- | --- |
| redkiln (TypeScript prototype) | `KbFrontmatter` in `src/schema/kb.ts`: ten required keys, nine kinds, five statuses, decision-only fields, and `.passthrough()` so foreign keys survive. `validate --kb` (260 lines) enforces the schema, link resolution and accepted-decision immutability. `/redkiln:kb-ingest` runs a 1,366-line workflow in a worktree: extract claims, adjudicate against the corpus (merge-and-link over new atoms), place by a layer matrix, sync map atoms, validate, commit; a person merges the branch. Its own `.kb/` holds 61 atoms in kind folders | The schema and its passthrough rule, the immutability and no-dangling-link rules, merge-and-link, "a person merges the wave". Not the committed map atoms |
| weigh-in | `atom.rs` (1,107 lines) renders every settled item as a conforming atom: `.kb/_intake/decisions/<wi-id>-<slug>.md`, id `kb-decision-wi-<6 hex>`, a MADR 4 body plus "Why this might be wrong", and passthrough keys (`door`, `decider`, `provenance`, `tree_hash`, `flip_condition`). Open questions go to `_intake/open_questions/`. Its "already decided" guard accepts only a cited `kb-` atom. 41 atoms, none ever promoted | The writer, as the contract crate's renderer. The lineage keys. The rule that an authority-bound item is closed only by a cited atom |
| temper | 43 Weigh-In atoms in `.kb/_intake/decisions/`, indexed by hand in `docs/history/decisions.md`; temper reads none at runtime | Proof that a hand-kept index drifts; a generated one replaces it |
| assay | 16 Weigh-In atoms on main, some anchored to GitHub issue URLs (wi-fef408); a committed AFK journal with 88 defaults | Issue URLs as atom anchors |
| anneal | `.out-of-scope/<slug>.md` rulings (`id, ruled_at, summary, weigh_in_ref`). The `ruled-before` UserPromptSubmit hook offers rulings and accepted Weigh-In atoms as precedents, bounded by file, byte and count caps | Rulings as an atom kind; the hook's bounds as the first injection budget |
| redkiln-rs (build/phase-0-to-1) | Design §8.6 makes atoms typed inputs injected per activity by declared rules, and puts the resolved atom set's digests in the memo key (ADR-0097). It rejects Kiro-style file-match inclusion for cross-cutting atoms. ADR-0085 and ADR-0092 set the records conventions: MADR-lite with "What this forbids", supersede never edit, two-sided links, ERRATA for wrong evidence, a generated index. `.redkiln/kb/` holds four process notes (spec conventions, refactoring catalog, retro format, estimation history) | The memo-key contract, explicit loaders, the records conventions. The four notes belong to the process catalog, not here (call F) |

Two facts shape the design. First, 100 atoms (41 in weigh-in, 43 in temper, 16 in assay) sit in `_intake/` across three repositories because promotion never moved out of TypeScript. Second, every reader today parses a different subset of the frontmatter, so the schema must live in one contract crate.

## The atom model

An atom is one markdown file holding one fact, with typed frontmatter that Knowledge owns and a body that is a rendering. The schema lives in `knowledge-contract` and Core's SPEC, so every producer writes it through types and every reader decodes the same fields.

### Kinds

Eight kinds cover what the repos actually produce. The prototype's other kinds are still read, and map onto these on import.

| Kind | Holds | Typical producer | Body |
| --- | --- | --- | --- |
| decision | A choice between options, with who decided | Weigh-In, a person | MADR 4 plus "What this forbids" and "Why this might be wrong" |
| constraint | A must or must-not with no choice behind it (a licence, a platform limit) | A person, an audit | Statement, source, what it forbids |
| ruling | A scope ruling: this is out of scope, or in scope only as stated | Bench (from `.out-of-scope/`) | Summary, the request it answers |
| finding | A measured or documented fact about the system | Review, Loop, an audit | Claim, evidence tag (`measured`, `documented`, `inferred`), method |
| evidence | A source the findings cite: a spike result, a benchmark, a page | Loop spikes, research | What it is, where it lives at a SHA |
| learning | A lesson from an outcome: a finding disposition, a retrospective, a corrected mistake | Review, Bench, Loop | What happened, the rule it suggests |
| term | A word in the project's language and what it means | Bench (`lint terms`), a person | Definition, aliases, what it is not |
| open-question | A question raised and not yet settled | Weigh-In | The question, who it waits on |

The prototype's `concept`, `playbook`, `reference`, `narrative`, `map`, `roadmap` and `governance` stay readable as legacy kinds. Maps and roadmaps are generated views, so new ones are never written (see Lifecycle).

### Fields

The ten required keys stay as the prototype defined them: `id`, `title`, `kind`, `status`, `authority_tier`, `summary`, `depends_on`, `related`, `source_paths`, `last_reviewed`. Unknown keys still pass validation, because producers carry lineage (Weigh-In's `door`, `decider`, `tree_hash`). Knowledge adds six optional keys:

| Key | Meaning | Why |
| --- | --- | --- |
| `scope` | Where it applies: `always`, path globs, crates, catalog activity ids, or topics | Retrieval and injection key on it; explicit, never inferred from file location |
| `anchors` | Issue or PR URLs, `path:line@sha`, `wi-` ids | Ties a fact to its work item and code, as assay already does |
| `applies` | `once` or `always` | Pressure test rec 15: an `always` decision is standing policy |
| `confirmation` | The check that enforces it: a rule id, a test path, a lint, a command | MADR's Confirmation and ADR-0085 rule 4; `validate` checks it resolves |
| `trust` | `asserted`, `witnessed` or `attested`, copied from the event that produced it | Pressure test call C; only witnessed or attested atoms may be `accepted` |
| `producer` | The plugin and version that wrote it | Lets each producer upgrade its own body rendering |

### Ids and paths

An id is `kb-<kind>-<hash>`, where the hash is the first 10 hex digits of a SHA-256 over the producer's stable key (Weigh-In's item id, a ruling's slug, a finding's claim and source). Weigh-In's six-digit ids collide with about a 3% chance at 1,000 items across clones, which a monorepo with several clones will reach. So new wi- ids lengthen to 10 hex, old 6-hex ids are kept, and CI checks duplicates across the suite. Existing ids are kept as they are, because issues, trailers and other atoms cite them (call C).

An atom lives at `.kb/<kind>/<id>-<slug>.md`, its path set by its kind and never moved after it is written. Status is a field, not a folder, so promotion changes no paths and breaks no links (call B).

## Lifecycle

Atoms in git at HEAD are the truth; everything else is derived. A producer writes an atom once, a person accepts it, and change happens only by supersession. This is the prototype's rule set plus redkiln-rs's records conventions, made mechanical.

### States

| Status | Meaning | Who sets it | Body editable? |
| --- | --- | --- | --- |
| proposed | Written by an agent or a tool, not yet accepted | Any producer | Yes |
| accepted | In force and injectable as binding | A witnessed or attested decider: a Weigh-In sitting, a PR merge by a person, a review approval | No. Only status, `superseded_by` and `last_reviewed` change |
| superseded | Replaced in full by the atom named in `superseded_by` | Knowledge, when the successor is accepted | No |
| withdrawn | Withdrawn with no successor | A person | No |

A Weigh-In decision from a sitting is born `accepted`, because the sitting is the human act; a finding from Loop is born `proposed` and is accepted when its PR merges. A two-way decision defaulted in an unattended session yields a proposed atom flagged for review, never an accepted one. Partial supersession leaves the predecessor accepted and says the scope in both bodies (ADR-0085 rule 3). A wrong evidence line on an accepted atom goes in a dated errata block appended under its body, which `validate` allows and nothing else may add (ADR-0092).

### Intake

`.kb/_intake/` is only for prose a person or an agent drops in to be distilled: a research report, a retrospective, meeting notes. `/rk-knowledge:ingest` keeps the prototype's shape (extract claims, adjudicate against the corpus with merge-and-link over new atoms, write proposed atoms, open a PR) and runs in a worktree. Typed producers skip intake and write their final path through the contract crate. The 100 existing atoms in `_intake/decisions/` move once to `.kb/decision/` in each repository's migration PR, keeping their ids.

### Indexes and maps

Indexes are projections, rebuilt from `.kb/` into Knowledge's projection file under `<git-common-dir>/redkiln/`. Nothing generated is committed: a committed index conflicts on every branch merge (pressure test), and temper's hand-kept `docs/history/decisions.md` already drifts. People get the views on demand (`knowledge map`, the suite mod pane, later Docs) (call D).

### Branches, merges and clones

- An atom settled on a branch is truth only once it reaches the default branch. A decision in the log but not in `.kb/` at HEAD shows as "settled, not yet on this branch" (pressure test, scenarios).
- One file per atom means two branches never conflict on content. `validate` runs on the merged tree in CI and fails on a duplicate id, a dangling link, a one-sided supersession or an edited accepted body (against the merge base).
- Fork PRs may propose atoms but never land one as accepted: CI treats a fork's atom as asserted whatever its frontmatter says.
- Client repositories keep `.kb/` in their own git as usual; Knowledge never copies a client atom into user scope or anywhere off the machine (Blueprint design rule 10).

## Retrieval and injection

Injection is where knowledge systems fail: too many atoms in context is worse than none (suite-structure §4). So Knowledge injects little by default, more only when a scope matches, and records every injection so relevance can be measured rather than assumed.

### Three injection points

| Point | Trigger | What it injects | Budget |
| --- | --- | --- | --- |
| Session digest | SessionStart | A count by kind, then accepted `always`-scoped decisions and constraints, newest first, one line each (title and id) | 1,500 characters, and less when sibling plugins are in the log, under Core's suite context budget |
| Just in time | UserPromptSubmit, and PreToolUse on Read, Edit and Write | Accepted atoms whose `scope` matches the paths touched or the prompt's terms, as id, title and summary | 2,400 characters and 5 atoms a turn (Weigh-In's context-pack size and anneal's precedent cap); an atom shown once per session is not shown again |
| Activity loader | Loop's step start, Bench's catalog skills | The atoms an activity's declared rule resolves to, in full | Set by the activity; the sorted digests of the resolved set enter Loop's memo key (ADR-0097 component 6) |

Scope matching is explicit. A path glob in `scope` matches files touched, the way Claude Code's path-scoped `.claude/rules/` do; a cross-cutting atom says `always` or names topics and activities. Folder location never implies scope, which is redkiln-rs's finding against Kiro-style file-match inclusion. Superseded, withdrawn and proposed atoms are never injected as binding; a proposed atom may be shown once, labelled proposed.

### Query

`knowledge query <terms>` answers "what did we decide about the store?" for a person, a skill or another plugin, with `--json`. v1 is lexical: BM25 over title, summary, body and terms' aliases, boosted by scope match, status and recency, built in Knowledge's projection. It needs no model call, no embedding service and no new dependency beyond what Core already links (call E). Weigh-In's "already decided" lookup and Bench's `ruled-before` hook call this query when Knowledge is present and fall back to their own scans otherwise.

### Measuring relevance

Every injection writes a `knowledge.atom_injected` telemetry event (atom id, point, session). An atom counts as used when the turn cites its id, or when a later Weigh-In sitting dismisses an item as already decided by it. Knowledge ships a relevance report and an eval seed in the shape Weigh-In already uses: real prompts labelled with the atoms that should and should not appear, scored for precision before any budget is raised.

### Staleness

An accepted atom with `last_reviewed` older than 180 days, or whose `source_paths` or `anchors` changed since that date, is listed by `knowledge stale`. A review only bumps the date or opens a supersession; nothing goes stale silently, and nothing is withdrawn automatically.

## Commands, hooks and events

The plugin is a thin shell over one `knowledge` binary, as the Blueprint lays out: skills for people, hooks for injection, a CLI with `--json` for everything else.

### Skills

| Skill | Does | Writes |
| --- | --- | --- |
| `/rk-knowledge:knowledge` | Answers "what do we know about X" from `query`, citing atom ids | Nothing |
| `/rk-knowledge:capture` | Turns something said in chat into a proposed atom (a learning, a term, a constraint), previewed first | One proposed atom |
| `/rk-knowledge:ingest` | Distils a prose document in `_intake/` into proposed atoms on a branch, merge-and-link first | Atoms on a branch, then a PR |
| `/rk-knowledge:review` | Walks `stale` and open questions; each one is kept, re-dated or superseded | Status and date fields, successor atoms |
| `/rk-knowledge:setup` | Creates `.kb/`, migrates `_intake/decisions/`, adds the CI check | Files in one PR |

In Cowork the skill names stand alone (D-0004), so the router skill is `knowledge`, not a prefixed name.

### CLI

`knowledge query | show | write | supersede | validate [--base <ref>] | index | digest <activity> | stale | map | inject <point> | status`. `write` takes typed JSON from the contract crate and is the only way a sibling creates an atom; `digest` prints the resolved atom set and its hash for Loop; `inject` is what the hooks call. `knowledge status --json` feeds the suite views (the board's Knowledge tab and the suite mod pane): atom counts by kind and status, the stale list, and the latest `knowledge.*` events (`atom_written`, `atom_status_changed`, `atom_injected`, `validate_run`). Exit codes and the result envelope follow Core's SPEC.

### Hooks

SessionStart (digest), UserPromptSubmit and PreToolUse on Read, Edit and Write (just in time), and PostToolUse on Write and Edit under `.kb/`, which runs `validate` on that one file and reports in context. All fail open. A PreToolUse guard refuses an agent's Edit that changes an accepted atom's body; that guard fails closed, and redkiln-guard's shared pack denies the same through Bash.

### Events and state

The `.kb/` files are authoritative; the log holds only what happened to them. Knowledge passes Loop's deleted-database test: delete its projection and `index` rebuilds it from `.kb/`.

| Event | Kind | Payload |
| --- | --- | --- |
| `knowledge.atom_written` | durable fact | id, kind, status, producer, trust, path, content hash |
| `knowledge.atom_status_changed` | durable fact | id, from, to, by, successor |
| `knowledge.atom_injected` | telemetry | id, point, session, budget used |
| `knowledge.validate_run` | telemetry | tree hash, result, error counts |

### Dev and CI

CI runs `knowledge validate --base origin/main` as a required check in repositories that adopt Knowledge, so the rules hold for people, Copilot and Codex too, not only for Claude Code sessions (pressure test rec 14).

## Prior art and patterns

Every coding agent now has a memory or rules file, and they converge on three inclusion modes: always, by file match, and by description. None of them separates proposed from accepted knowledge, binds a decision to the check that enforces it, or measures whether an injected note helped. Those three are where Knowledge can be better rather than another copy.

| Prior art | What it is | Pattern to take | What to leave |
| --- | --- | --- | --- |
| [Claude Code memory](https://code.claude.com/docs/en/memory) | CLAUDE.md and AGENTS.md written by people, `@` imports, `.claude/rules/` with `paths` frontmatter, and auto memory Claude writes itself (first 200 lines or 25 KB loaded) | Path-scoped loading triggered on Read, Write and Edit, which Knowledge's just-in-time point mirrors; a hard load cap | Free-form notes with no status or supersession; auto memory is per user, not reviewed in a PR |
| [Cursor rules](https://cursor.com/docs/context/rules) | `.cursor/rules` files applied always, by glob, by the agent reading a description, or by @-mention | The four modes as values of `scope` (always, paths, topics, manual) | Agent-requested inclusion as the default: it spends context on a guess and leaves no record |
| [Kiro steering](https://kiro.dev/docs/steering/index) | `.kiro/steering` files with `inclusion: always, fileMatch, manual, auto`, workspace and global scope | Workspace beats global on conflict | `fileMatch` for cross-cutting facts, which redkiln-rs found fails for globally scoped files (design §8.6) |
| [Bugbot learned rules](https://cursor.com/blog/bugbot-learning) | Review rules learned from reactions, replies and missed-issue comments; candidates become active as evidence builds and are disabled on negative signal. 44,000 rules across 110,000 repos | Learnings as candidates scored by outcomes before they bind; Review's dispositions feeding `learning` atoms | Rules held in a hosted dashboard, outside the repository and its review (a [forum request](https://forum.cursor.com/t/api-or-git-based-workflow-to-promote-bugbot-learned-rules-into-repository-rules/171018) asks for exactly that git workflow) |
| [MADR](https://ozimmer.ch/practices/2022/11/22/MADRTemplatePrimer.html) | The Markdown ADR template Weigh-In already renders | The optional Confirmation section, made a typed `confirmation` field `validate` resolves | MADR's permission to edit accepted records in place; the repos chose immutability (ADR-0085) |
| [Log4brains](https://github.com/thomvaill/log4brains/) | ADRs as markdown in git, built into a searchable static site with a timeline | A generated, browsable view over committed records, which Docs can do later | A second store or build step people must run; its install instructions still point at a beta channel |
| [Tessl](https://tessl.io/blog/fixing-api-misuse-how-tessl-improves-agent-accuracy-by-up-to-33x) | Versioned "tiles" of library usage guidance, each with evals; vendor-reported gains of about 1.2x on average | Every piece of injected context earns its place with an eval, as Weigh-In's seed already does | A registry of third-party library knowledge; Knowledge is about this repository |
| Zettelkasten (from memory) | One idea per note, dense links, notes never rewritten but added to | Atomicity and merge-and-link, already the prototype's rule | Unbounded linking with no kinds or status |
| redkiln-rs records conventions | ADR-0085 and ADR-0092 | "What this forbids", two-sided supersession, errata blocks, a generated index | Its flat ADR numbering for atoms, which needs a central counter across clones |

The lesson common to all of them: the inclusion rule is the product. Knowledge's `scope` field, its budgets and its injection log are the design's center, not the file format.

## How it composes with the rest of the suite

Knowledge owns atoms and nothing else: siblings write through `knowledge-contract` and read through `query`, `digest` and the hooks, never by parsing `.kb/` themselves.

[embedded content: Knowledge's place in the suite · 5 producers, 1 contract, 5 consumers]

Every arrow is optional: with Knowledge absent, producers still render atoms through the contract crate's writer and consumers fall back to their own scans.

- **Weigh-In** decides; Knowledge keeps. A settled item becomes an accepted decision atom, written by the contract's renderer (today's `atom.rs` moves there). Weigh-In's "already decided" guard calls `query`. A `wi-` id stays the atom's anchor, and the `Weigh-In:` trailer still links commits to it.
- **Bench** writes rulings from `.out-of-scope/` and terms from `lint terms` as atoms, and its `ruled-before` hook reads precedents through `query` instead of reading Weigh-In's files.
- **Review** turns finding dispositions into atoms: waived and false-positive classes become ruling atoms, and won't-fix dispositions and lessons become learning atoms. Both stay proposed until accepted, scored by outcomes before they bind, the way Bugbot promotes rules. Its reviewers get the accepted atoms scoped to the changed paths.
- **Loop** declares an atom rule per activity, calls `digest`, and puts the resolved set's hashes in its memo key, so superseding an atom re-runs the work it fed. Its spikes and audits write finding and evidence atoms.
- **Redkiln Rust** reads constraints and decisions scoped to crates; an approved lint exception is a Weigh-In decision, so it arrives as an atom with a `confirmation` naming the `#[expect]` site.
- **Backlog** issues are anchors: an atom cites the issue it came from, and a closed Initiative's retrospective becomes learning atoms (Backlog's "Later" wave).
- **Core** holds the atom schema in its SPEC, the `knowledge.*` event types and the store location; Knowledge's projection is disposable.
- **Docs**, when it comes, renders maps and a browsable decision log from the same atoms; until then `knowledge map` and the suite mod pane show them.

## Calls for Ryan and next steps

Seven calls shape the plugin, each with a recommendation. B and C are hard to undo once colleagues' repositories, issues and trailers cite atom paths and ids; the rest are two-way. Several neighbouring questions are open elsewhere and are not re-asked here. Knowledge in the first wave is decided. Journals in refs are pressure test call A, and trust levels in Core's format are pressure test call C, which trust assumes. Where spec artifacts live is pressure test call E, and Weigh-In's slice 3 supersession is Blueprint call 7.

| # | Call | Recommendation | Door |
| --- | --- | --- | --- |
| A | Eight atom kinds (decision, constraint, ruling, finding, evidence, learning, term, open-question), with the prototype's other kinds read as legacy; the open question kind is spelled open-question in frontmatter and folder names | Yes | Two-way: kinds can be added, and legacy ones keep reading |
| B | Atoms are written once to `.kb/<kind>/<id>-<slug>.md` with status as a field; `_intake/` holds only prose awaiting extraction, and today's 100 intake atoms move once | Yes | One-way once others link to the paths |
| C | New ids are `kb-<kind>-<10 hex>` content hashes, existing ids are kept, and CI fails on a duplicate over the merged tree. Weigh-In's new wi- ids also lengthen to 10 hex (old 6-hex ids are kept), and CI checks wi- duplicates across the suite, which closes the collision risk cited under Ids and paths | Yes | One-way once cited |
| D | Indexes and maps are never committed; they are rebuilt projections shown on demand | Yes | Two-way |
| E | Retrieval v1 is lexical with explicit scopes; no embeddings or vector store until the relevance eval shows lexical misses | Yes | Two-way |
| F | `.redkiln/kb/`'s four process notes move to the catalog content package, not Knowledge, each beside the activity that reads it: spec-conventions.md with fix-spec and bdd-scenarios, refactoring-catalog.md with tdd-red-green-refactor, retro-format.md with lightweight-retro, and estimation-history.md with t-shirt-sizing | Yes | Two-way |
| G | `applies` (once or always) and `confirmation` are in the v1 schema, so standing decisions name the check that enforces them (pressure test rec 15, brought forward from Later). Knowledge owns this question; Weigh-In's call D now says "See Knowledge call G" | Yes, fields only; enforcement stays with the named check | Two-way while optional |

### Suggested next steps

- Ryan decides calls A to G.
- Write the atom schema, kinds and id rule into Core's SPEC draft, with golden fixtures from the 100 existing atoms (41 weigh-in, 43 temper, 16 assay) and the prototype's 61.
- Spike: `knowledge validate` and `query` in Rust over those fixtures, then a relevance seed of 50 real prompts from this project's threads, scored before any budget is set.
- Move `atom.rs` into `knowledge-contract` behind Weigh-In's existing output, byte for byte, so Weigh-In's 1.0 pilot sees no change (its capture freeze runs to 1.0.0, on or after 2026-10-20).
- Point anneal's `ruled-before` precedents and Weigh-In's already-decided guard at `query`, each with its current scan as the fallback.
