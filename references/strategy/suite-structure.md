# The Redkiln suite, designed from first principles

*2026-10-12. A companion to [suite-vs-redkiln.md](suite-vs-redkiln.md) and [redkiln-vs-flow.md](redkiln-vs-flow.md). Grounded in temper, weigh-in, anneal, assay and redkiln-rs at their current heads, in the published `happenstance-core` crate, and in Claude Code's plugin docs (fetched today). It answers Ryan's five questions (Core, Loop, Knowledge, Docs, Backlog) and asks whether the suite should be broader.*

## The short answer

Your instinct is right about the **structure** and partly wrong about the **packaging**. The suite does have more domains than the four plugins we have been talking about, and the cleanest way to see them is by the durable things a software factory keeps, not by the tools that happen to exist. But a domain is not automatically a plugin, and a foundation should not be a plugin at all.

| Your question | Verdict | In one line |
|---|---|---|
| Redkiln Core | **Yes to the foundation, no to a plugin** | A small library plus a written format spec that every plugin embeds. What must be shared is the on-disk formats, not a running process. |
| Redkiln Loop | **Yes** | The headless conductor, renamed so that "Redkiln" means only the suite. Watch the Loop/Flow name pair. |
| Redkiln Knowledge | **Yes, and it is the strongest new idea** | `.kb` is already a hub with three producers and three consumers and no owner. |
| Redkiln Docs | **Not yet** | Docs is an activity, a check and a projection, with no code anywhere. Design for it; don't ship it. |
| Redkiln Backlog | **Yes, second wave, with one big call** | Items are the spine Loop runs on, but "the backlog lives in the repo, GitHub is a mirror" is the heaviest opinion in the suite. |

And two things the five questions miss:

1. **A bundle plugin called Redkiln.** Claude Code now supports plugin `dependencies`, and a plugin whose manifest is only a name and a dependency list installs a whole set in one command. That gives "install everything" without making anything required.
2. **Two shared foundations nobody has named:** *calibration* (earning trust in a model's judgement, which four repos each reinvent) and *capacity* (one subscription's budget, which the headless pieces will fight over). Both belong in Core as libraries, not as plugins.

The rule that ties it together: **design the domains now, ship a plugin only when a domain has its own user and its own code.**

## 1. Start from what a factory keeps, not from what exists

Strip away the repos and ask what a software factory run by agents, with one human in charge, has to remember. There are six kinds of durable thing:

| Noun | What it is | Who writes it | Who reads it |
|---|---|---|---|
| **Event** | What happened, when, by whom, at which commit | Everything | Everything (gates, replay, metrics) |
| **Item** | Work to do: an idea, a story, a ticket, a deferred question | Humans, Flow's plan, Weigh-In's deferrals, Review's follow-ups | Loop (its input), Flow, the human |
| **Atom** | What we know: a decision, a constraint, a finding, a learning, a term | Weigh-In, Flow (spec, repro), Review (dispositions), humans | Every agent, injected per activity |
| **Artifact** | The work product of one step: spec, plan, repro, findings, report | Flow and Loop | The next step, the reviewer, the human |
| **Approval** | Who approved what, at which content hash, by when | Humans (through Flow, Weigh-In, `redkiln answer`) | Gates in every runtime |
| **Standard** | What good looks like, in rules with IDs | The owner | Hooks, reviewers, gates |

And there are four ways work actually gets done, which is where the runtimes come from:

| Runtime | Human present? | Unit of work | Today |
|---|---|---|---|
| **In-session referee** | Yes, driving | One chat | anneal (Flow), temper (Rust) |
| **Human sitting** | Yes, deciding | A handful of open calls | weigh-in |
| **Headless conductor** | No | Many tickets, unattended | redkiln-rs (design only) |
| **Merge gate** | No (then a human approves) | One PR | assay (spec only) |

Everything else is a **projection**: a rendering of a noun for some audience. GitHub issues are a projection of items. A docs site is a projection of atoms, artifacts and code. Weigh-In's board is a projection of open calls. A dashboard is a projection of events. redkiln-rs already holds this line firmly: "a projection is never read back as authority, and an inbound tracker event is a typed input, never a transition" (`redkiln-rs/references/2026-09-08-architecture-design.md` §3.2).

Put those together and the suite falls into layers:

```
 ┌──────────────────────────────── experiences (plugins) ─────────────────────────────────┐
 │  Redkiln Flow        Redkiln Decide       Redkiln Loop          Redkiln Review         │
 │  in-session phases   (Weigh-In) sittings  headless conductor    PR verdict             │
 └──────────┬──────────────────┬───────────────────┬──────────────────────┬───────────────┘
            │      read and write the domains through their CLIs and formats            │
 ┌──────────▼──────────────────▼───────────────────▼──────────────────────▼───────────────┐
 │  domains (plugins when they have a user)          standards (one plugin per language)  │
 │  Redkiln Knowledge  .kb atoms                     Redkiln Rust (rust-craft)            │
 │  Redkiln Backlog    .bklg items  (wave 2)         Redkiln TypeScript, Python (later)   │
 │                                                                                        │
 │  projections, owned by the domain they render: GitHub issues (Backlog), docs site,     │
 │  Weigh-In's board, a suite timeline                                                    │
 └──────────────────────────────────────┬─────────────────────────────────────────────────┘
 ┌──────────────────────────────────────▼─────────────────────────────────────────────────┐
 │  Redkiln Core: a library and a format spec, embedded in every binary, never installed  │
 │  happenstance (events, DCB, projections) · envelope · ids · store location · approval  │
 │  record · artifact, item and atom schemas · activity ids · calibration · capacity      │
 └────────────────────────────────────────────────────────────────────────────────────────┘
          Redkiln (bundle plugin): name + dependencies only, "install the whole suite"
```

The rest of this note tests each of your five against that picture.

## 2. Redkiln Core

### The case for it is stronger than you may know

The suite already has **five event stores**, each built separately:

| Repo | Where events go | Format |
|---|---|---|
| temper | `.git/temper/telemetry.jsonl`, optionally pushed to a branch (`temper/README.md` §Local data and telemetry) | JSONL lines from Node |
| weigh-in | happenstance SQLite at `.git/weighin/`, plus `.kb/_ledger/journal/<session>.jsonl` committed | happenstance 0.3.2, TRUNCATE |
| anneal | happenstance SQLite at `.git/anneal/ledger.sqlite3` (`anneal/docs/ledger.md`) | happenstance 0.3.2, TRUNCATE, its own envelope |
| assay | per-job event stream and a budget ledger with a cross-repo semaphore (`assay/docs/architecture/01-context-goals.md` P7, A-03) | spec only |
| redkiln-rs | git trailers, then rusqlite under WAL (design §3.2, §3.3) | design only |

So the question "what happened across the suite in this session" has no answer today. Each store keys sessions differently, and two of them disagree on the journal mode.

The best part: **the foundation already exists, and it is yours.** `happenstance-core` is Wet Ink's own crate on crates.io: "DCB-compliant event sourcing contract: the EventStore and ProjectionStore ports, their value types, and in-memory reference stores." It is at 0.4.1 as of today, while weigh-in and anneal pin 0.3.2. Event store, projections and conditioned appends (DCB) are exactly the "event streams, observability, projections" in your question.

### Why Core should not be a plugin

A plugin is something a user installs to get a behaviour. Core has no behaviour of its own. If it is a plugin, it becomes a runtime every other plugin calls, which brings back the "take the whole system" problem one layer down:

- **Version intersection.** Claude Code resolves plugin dependencies by semver, and "constraints from several plugins intersect, and a conflict fails the install" (code.claude.com/docs/en/plugins/dependencies). If Flow needs Core `^0.5` and Rust still needs `^0.4`, the colleague who wanted both gets neither.
- **Disabled means dead.** "A dependency that is missing or disabled leaves the dependent plugin disabled." Every plugin would go down with Core.
- **No runtime discovery.** The docs name no way for one plugin to find another's binary or know it is enabled. `CLAUDE_PLUGIN_ROOT` is per plugin, and `bin/` cannot ship to claude.ai or Cowork anyway (the reason everything uses `plugin/libexec/`).

What actually has to be shared is the **formats on disk**: the event envelope, where stores live, the schemas of items, atoms, artifacts and approvals. Code that reads and writes those is a convenience. So: **share formats, embed code.**

### What Core is

1. **A written spec, versioned on its own** (`redkiln-core/SPEC.md`): store location rule, event envelope, id scheme, approval record, the three schemas, the activity-id list, the end-of-turn block grammar. This is the "suite contract" from the earlier write-up, given a home and a name.
2. **Rust crates that implement it**, linked statically into each plugin's binary: `happenstance-*` underneath, then a thin `redkiln-core` (envelope, ids, location resolver, schemas, readers for every other plugin's stream). temper's Node CLI reads and writes the formats directly until it is ported (its 2.0 plan already moves it to Rust).
3. **A conformance kit**: fixtures and a test runner each plugin's CI runs, so "conforms to Core 1.2" is a checked claim, not a hope.

### One store or many

Both work on happenstance. I recommend **one file per plugin, same schema, same directory**: `<git-common-dir>/redkiln/<plugin>.sqlite3`.

| | One shared file | One file per plugin (recommended) |
|---|---|---|
| Cross-plugin query | One SQL query | A reader in Core opens several files |
| Atomic cross-plugin conditions (DCB) | Yes | No. The one case that needs it (Loop accepting Flow's plan approval) only *reads* Flow's events, and Loop's own append is the guarded one |
| Schema migration | Every plugin moves together, and a plugin refuses a newer file (anneal does this today), so one upgrade can disable siblings | Each plugin migrates its own file |
| Write contention | Hooks from four plugins serialise on one lock | Independent |

The `redkiln/` directory and the shared envelope give observability nearly all of what one file would, without coupling releases.

### Keep Core small

Core is where suites go to die: everything "foundational" drifts in, and every change ripples through every release. Two rules keep it honest:

- **Rule of two.** Something enters Core only when two plugins need it now, with code. (The same rule the earlier write-up used for a shared craft engine.)
- **No behaviour.** Core holds formats and the code that reads and writes them. Gates, policy and UI stay in plugins. A dashboard is not Core; Core is what makes a dashboard cheap.

## 3. Redkiln Loop

Yes. Calling the headless conductor **Redkiln Loop** fixes a real problem in the current plan, where "Redkiln" meant both the suite and one plugin in it. With Loop, Redkiln is only the brand and the bundle.

Three cautions.

- **Loop and Flow sound like synonyms.** A colleague will not guess which is the in-session one. The distinction that matters is attended versus unattended, so either lean into it in the one-liners ("Flow: work through a change with Claude. Loop: let Claude work the backlog while you're away.") or pick a name with that sense (Redkiln Run, Redkiln Shift, Redkiln Night).
- **Loop is the least built and the most designed.** redkiln-rs has 112 ADRs, a runbook whose build stages are all not started, and no commits since 2026-09-11. The suite's shape must not wait for it. Loop conforms to Core and consumes Flow's checks, not the other way round (as [redkiln-vs-flow.md](redkiln-vs-flow.md) recommended).
- **Loop's design currently contains three other plugins.** Its store owns `.bklg` and `.kb`, and its ports include `Tracker` and `DocsSite` (design §3.1, §3.3). In this structure those move out: Loop *uses* Backlog and Knowledge through their formats, and its tracker and docs ports become those domains' projections. This is a real re-scope of redkiln-rs, and it shrinks it.

## 4. Redkiln Knowledge

This is the best idea of the five, because the evidence shows a hub with no owner.

**What exists.** The `.kb` format comes from the original Redkiln (the TypeScript one redkiln-rs replaces): typed frontmatter atoms, an `_intake/` area, `kb-ingest` to promote and index them, `validate --kb` to refuse in-place edits of accepted decisions (weigh-in `docs/research/report.md:130`, `:413`, `:478`). weigh-in already writes conforming atoms (see `weigh-in/.kb/_intake/decisions/`). redkiln-rs plans the lifecycle "intake, adjudicate, integrate" and injects atoms into each activity by declared rules, as part of the memo key (design §8.6).

**Why it wants to be its own plugin:**

| Producers today or planned | Consumers today or planned |
|---|---|
| Weigh-In: decisions | Weigh-In: "already decided" filter, before it asks you anything |
| Flow: specs, repro learnings, constitution | Flow: Clef's D6 reads decision atoms back |
| Review: finding dispositions | Loop: atoms injected per activity |
| Rust: approved lint exceptions | Review: context for reviewers |

Nobody owns the middle: promotion out of intake, supersession, indexing, retrieval ("what did we decide about the store?"), and injection into a session. Today that lives in a TypeScript tool slated for replacement and in a section of an unbuilt design.

**A standalone pitch that works:** "Your repo remembers what was decided and why, and every Claude session starts knowing it." That is useful with no other Redkiln plugin installed.

**Make it broader than decisions.** The repos are full of a second kind of knowledge: evidence. redkiln-rs tags every claim `[measured]`, `[documented]` or `[inferred]` and keeps F-numbered findings; weigh-in and assay have research reports and eval seeds. Atom kinds should cover decision, constraint, finding, evidence, learning and term from the start.

**Where it meets Weigh-In.** Weigh-In is how a human *makes* a decision; Knowledge is where decisions *live*. Weigh-In keeps its own pitch and writes into Knowledge's intake. Its "already decided" lookup becomes a Knowledge query when Knowledge is present, and its own scan otherwise. This also settles the naming question from the earlier write-up: next to Knowledge, **Redkiln Decide** reads naturally.

**Caution: injection is where knowledge systems fail.** Too many atoms in context is worse than none. redkiln-rs already found that Kiro-style file-match inclusion does not work for cross-cutting atoms and requires an explicit loader (design §8.6). Knowledge's hard problem is relevance, and it should ship with a budget and a measurement for it, not only storage.

## 5. Redkiln Docs

Not yet. Here is the challenge in full, because the idea is attractive.

**What "docs" would actually be.** In redkiln-rs, docs is three different things: an **activity** (update the docs as part of done), a **check** (docs drift when their sources change; build and links pass), and a **projection** (a site built by mdBook, Starlight or rustdoc from a typed `DocSet`, with "agents propose, CI and CODEOWNERS gate", design §8.6). None of the three is a domain with its own durable noun: the source is the `docs/` folder, the code, and Knowledge's atoms.

**What exists.** Zero code anywhere. The `DocsSite` port has no adapter until redkiln-rs's Phase 3, and none of the other repos mention a docs site.

**Where each part belongs instead:**

- The activity is a catalog entry that Flow and Loop run.
- The drift and link check is a Review gate (and a Flow check in session).
- The site is a projection adapter, owned by whichever domain it renders.

**What would flip this.** If "keep my docs in sync with my code" turns out to be the thing colleagues ask for first, or the suite starts producing end-user documentation for client products (IQBG), Docs earns a plugin with that pitch. Also worth settling what you meant: documentation *for the software being built* (this note assumes that) or documentation *of the suite itself* (that is just each repo's README and a suite site, not a plugin).

## 6. Redkiln Backlog

Yes, in the second wave, and only after one call that is yours.

**Why items deserve a domain.** Loop's whole input is an item (`redkiln run --lane story --item …`). Flow's `plan` and `dag` already cut tickets and send outside blockers "to the tracker" (`anneal/docs/plan.md:71`). Weigh-In's deferrals name a trigger, a resolve-by date and a default (`weigh-in/README.md:145`), which is to say they are items. Review's follow-ups are items. Four plugins produce work items and none owns them.

**What exists.** The original Redkiln keeps `.bklg/` in the repo; redkiln-rs migrates a corpus of 1,312 files and 6,877 records from it (design §5.2) and plans GitHub issues as a projection with drift records (§8.6). The brainstorm states the rule plainly: "`.bklg/` owns content and the FSM. Issues render status; comments are inputs" (`references/2026-09-03-reimagining-brainstorm.md`).

**The call.** Is the backlog's source of truth in the repo, with GitHub as a mirror, or is it GitHub?

| | In-repo `.bklg`, GitHub mirrors (Redkiln's design) | GitHub (or Linear) is the backlog |
|---|---|---|
| Loop | Items are readable at a commit, which replay and memo keys need | Remote, mutable, rate-limited input; Loop has to snapshot it |
| Colleagues | A second backlog to learn, beside the one their team uses | Works with what they already have |
| Sync cost | Drift is real: redkiln-rs measured 12 of 174 = 7%, 3.6 times likelier on gates open over an hour (design §3.2) | None |
| Standalone pitch | "An agent-native backlog in your repo" | "Your agents read and update your issues properly" |

My recommendation is a hybrid: the item *schema* lives in Core so every plugin speaks it, and Backlog ships with GitHub as the default store and `.bklg` as the store Loop uses when it runs. That keeps adoption cheap and keeps the factory's need for content at a hash. It is a two-store design, so it is more work than either pure option. Pick a pure option if you'd rather pay less and accept its downside.

**Caution.** Backlog is the domain most likely to swallow the suite. Prioritisation, estimation, roadmaps and sprints are each a product. Keep it to items, their states, their links to artifacts and approvals, and the projection.

## 7. Is the suite broader than we think?

Yes, in four directions. Only one of them is a new plugin now.

1. **Craft standards per language** (already planned): Redkiln Rust now, TypeScript and Python when someone needs them, with a shared enforcement engine extracted once a second language exists.
2. **Calibration, a missing foundation.** Four repos reinvent "how do we know a model's judgement is good enough to act on": anneal's Clef shadow mode with Brier scores (`anneal/docs/calibrate.md`), weigh-in's labelled eval seed, assay's verifier confusion matrix (QA-12), and redkiln-rs's ADR-0079, which bars any model-derived facet until it clears a bar. One library in Core (record a judgement, label an outcome, score, promote only by a human edit) serves all four. This is the most valuable thing the suite does not yet share.
3. **Capacity, a missing foundation.** assay plans a semaphore across every repo on the subscription (A-03), and Loop plans per-run budgets. Once both run headless on one subscription they compete, and nothing arbitrates. A lease protocol in Core (who may spend, how much, until when) is cheap to define now and painful to retrofit.
4. **After merge, an honest gap.** Nothing in the suite owns release, rollout, watching and incidents. anneal had stubs (`ship`, `watch`, `rollout`, `perf`) and the earlier write-up recommended dropping them; redkiln-rs has release and operate stages in its catalog. A future **Redkiln Ship** is a real domain. It is not a plugin until there is code and a user, but name the gap so it doesn't get filled piecemeal inside Flow or Loop.

**And one that is not:** observability is not a plugin. With Core's envelope and the shared `redkiln/` directory, a suite timeline (`what did every plugin do in this session?`) is a small reader. Put it in the Redkiln bundle's CLI, or in Loop's `status` and `explain`.

## 8. Living together in one session

The plugin docs change one recommendation in the earlier write-up. It proposed "a fixed hook order"; that is not possible. **All matching hooks run in parallel**, and no order is documented across plugins (code.claude.com/docs/en/hooks-guide). How several `Stop` blocks combine was not in the docs I could read.

So the rules become part of Core's spec instead:

- Every hook is independent of every other plugin's hook, and idempotent.
- `SessionStart` context has a suite-wide budget, split by convention, and each plugin's injection is short when siblings are present (each can see the others' stores in `redkiln/`).
- End-of-turn blocks (temper's status, Weigh-In's `weigh-in` block, Flow's gates) share one grammar, so a turn that ends under three plugins ends with one coherent message, not three demands.
- The "all installed" end-to-end test from the earlier write-up becomes the conformance kit's top test.

## 9. The structure, distilled

**Plugins, in order:**

| Wave | Plugin | From | One-liner |
|---|---|---|---|
| Now | Redkiln Rust | temper | Rust that meets the standard, enforced as you write it |
| Now | Redkiln Decide | weigh-in | Agents' assumptions settled by you, at the end of the chat |
| Now | Redkiln Bench (was Flow) | anneal | Work through a change with Claude: grill, spec, plan, build, verify |
| Now | Redkiln Knowledge | new, from `.kb` and kb-ingest | Your repo remembers what was decided and why |
| Next | Redkiln Review | assay | Whether a PR may merge, as one required check |
| Next | Redkiln Backlog | new, from `.bklg` | Work items every agent reads and updates |
| Next | Redkiln Loop | redkiln-rs | Claude works the backlog while you're away |
| Later | Redkiln TypeScript, Python | new | Same as Rust |
| Later | Redkiln Ship, Redkiln Docs | new | When there is code and a user |
| Always | **Redkiln** | new | Bundle: name plus dependencies, installs the suite |

**Not plugins:** Redkiln Core (library, spec, conformance kit), calibration and capacity (in Core), observability (a reader), projections (owned by their domain).

**Order of work:**

1. Write Core's spec from what weigh-in and anneal already share (store location, envelope, TRUNCATE, the `.kb` atom shape), and bump both to happenstance 0.4.1 together.
2. Stand up the Redkiln marketplace and the bundle plugin pointing at today's repos.
3. Carve Knowledge out: the `.kb` format, intake promotion and retrieval, with Weigh-In as its first producer.
4. Re-scope redkiln-rs into Loop: `.bklg`, `.kb`, tracker and docs move out to their domains, and the store moves to Core's.
5. Decide the backlog's source of truth, then build Backlog.

## Calls for Ryan

1. **Core as a library and spec, not a plugin.** Recommended: yes. Flip: if you want one `redkiln` binary that every plugin shells out to, a required Core plugin is the way, and you accept install failures on version conflicts.
2. **One store file per plugin under `<git-common-dir>/redkiln/`.** Recommended: per plugin. Flip: if cross-plugin atomic conditions turn out to matter beyond Flow-to-Loop handoff.
3. **Rename the conductor to Redkiln Loop, and decide whether Loop and Flow are distinct enough as names.** Recommended: Loop, with one-liners that carry attended versus unattended.
4. **Knowledge as a first-wave plugin, with atom kinds beyond decisions.** Recommended: yes.
5. **Docs deferred.** Recommended: yes, until there is a user asking for it.
6. **Backlog's source of truth.** Recommended: GitHub by default, `.bklg` for Loop, item schema in Core. This one changes redkiln-rs's design most and is the hardest to reverse once colleagues have items in it.
7. **Calibration and capacity into Core.** Recommended: yes, calibration first (four consumers already).

## Ryan's answers (2026-10-12)

| # | Call | Answer |
|---|---|---|
| 1 | Core | A library and spec, not a plugin |
| 2 | Store per plugin or shared | **One shared file**, `<git-common-dir>/redkiln/events.sqlite3`, with every event tagged by plugin. Contention is not a concern at our volume (hundreds of events per session). Core keeps write transactions short and catches projections up in short batches. Decided 2026-10-13 |
| 3 | Loop and Flow names | **Redkiln Bench** for the in-session plugin (anneal, was Flow), beside Redkiln Loop for the headless conductor. "Craft" stays the standards family (`rust-craft`). Decided 2026-10-13 |
| 4 | Knowledge in the first wave | Yes |
| 5 | Docs deferred | Yes |
| 6 | Backlog's source of truth | **GitHub Issues**, using labels, sub-issues, issue types and dependencies. No in-repo `.bklg` |
| 7 | Calibration and capacity in Core | Yes |

### What happenstance gives under one file per plugin

Read from `happenstance-core` 0.4.1 and the crates.io listing:

- **Projections: yes, per plugin.** `ProjectionStore` is a frozen port (ADR-0063), with a SQLite adapter. A read model and its checkpoint commit in one transaction. The typed projection runner in the `happenstance` crate is still behind `unstable-projection`.
- **DCB: yes, but only within one store.** An `AppendCondition` guards against events in the same store. With one file per plugin, a guard cannot cover a sibling's events. The one place this matters is Loop acting on Flow's plan approval. That approval is already bound to the plan's hash, so Loop re-checks the hash before it acts rather than relying on a guard.
- **Cross-plugin views: not built in (corrected 2026-10-13).** happenstance's projection runner reads one event store. It stamps each store with a `StoreId` and has a per-peer sync watermark (`ProjectionId::sync_watermark`), but the replication crate that would use them is not published. With one file per plugin, a suite view is our own multi-store reader. With one shared file, it is an ordinary happenstance projection. This flips the recommendation to **one shared file**, `<git-common-dir>/redkiln/events.sqlite3`, with every event tagged by plugin. DCB guards then work across plugins, and Core owns the table layout with additive-only changes. Confirmed by Ryan, 2026-10-13.
- **Testkit.** `happenstance-testkit` 0.4.1 holds conformance suites for adapters, which Core's conformance kit can build on.
- **Caveat.** weigh-in and anneal avoid happenstance's own SQLite open, because it switches to WAL and Cowork needs TRUNCATE. Core has to own that opening path once, for the event store and the projection store alike.

### Consequences of GitHub Issues as the backlog

- Redkiln Backlog becomes a GitHub-native plugin. Item types map to issue types, hierarchy to sub-issues, blocking to issue dependencies, and state to labels. The item schema in Core describes that mapping, not a file format.
- Loop records a snapshot event of each issue it starts (number, `updated_at`, content hash), so a run can be replayed and its memo keys stay stable when someone edits the issue later.
- redkiln-rs drops `.bklg` from its store and its `Tracker` port becomes the Backlog plugin's client. Its migration tool converts the old `.bklg` corpus into issues.
- Weigh-In deferrals and Flow's outside blockers become issues when Backlog is present.
