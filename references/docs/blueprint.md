<!-- Exported from the Claude Doc https://claude.ai/code/artifact/cf13f63c-14d9-4ff9-8082-c476876edef2 (revision 21) on 2026-10-19 as part of the project handover. The Doc is the original; this copy is frozen. Diagrams (1) could not be exported and show as [embedded content] placeholders; open the Doc to see them. -->

# The Redkiln Blueprint

Oct 13, 2026 · @Ryan

## In one page

The suite decisions hold up; what the audits change is the granularity. The suite is a set of separately installable plugins over three kinds of shared code, built in one `redkiln` monorepo with one Cargo workspace. Each plugin still builds, versions, tags and installs on its own.

None of these decisions is overturned: a suite of separate plugins, one shared event log, a Core that is a library and a spec rather than a plugin, Backlog on GitHub Issues, Knowledge in the first wave, Docs deferred. Three repositories had already reached the same conclusions independently: GitHub Issues as the backlog (assay, redkiln-rs, temper), happenstance for events (weigh-in, anneal, Loop's ADR-0131), and `<git-common-dir>/<name>/` for stores (all of them).

The shared code splits three ways:

1. **Core**: the formats, plus the few crates every binary embeds (event log, envelope, ids, store location, journals, calibration, capacity).
2. **Kits**: shared libraries for things two or more plugins already implement separately. Four exist in duplicate today: a shell guard, a headless `claude -p` dispatcher, a GitHub client, and a generic enforcement engine.
3. **Contract crates**: small crates in which a domain plugin publishes its own event types and schemas, so siblings write to it through types instead of parsing its text output.

Plugins become thin shells (hooks, skills, a launcher, one binary) over their own domain crates, Core and the kits. The evidence is in five audits written today, one per repository, in the project files under `strategy/audit/` (temper, weigh-in, anneal, assay, redkiln-rs).

[embedded content: suite layers · products, kits, Core]

Read it top down: each layer depends only on the layers below it, and products never reach into each other's code.

## What the audit changed

The local checkouts of two repositories were stale, so the earlier write-ups understated them, and in nine places the evidence pushes back on a decision.

### Three facts the earlier write-ups had wrong

| Write-up said | Actually, on 2026-10-13 |
| --- | --- |
| redkiln-rs is the least built: no code, 112 ADRs, no commits since 2026-09-11 | `main` is a design snapshot. The work is on `build/phase-0-to-1`: 552 commits, 12 crates, about 101k lines of non-test Rust plus 137k of tests, 1,453 tests, 198 ADRs (199 as of 2026-10-17), CI green on Linux and Windows. Phase 1 is about 65% done and no lane has run against a real model. Loop is the largest codebase in the suite |
| assay is spec only | `origin/main` has 46 commits, six crates, 47.6k lines of source and 40.7k of tests. Pure `decide`, rubric, tiering, the temper adapter, findings validation, the hardened `claude -p` runner, verifier and specialists are built. Everything that touches GitHub waits on four owner setup tasks (#39 to #42) |
| anneal has stubs for `ship`, `watch`, `rollout`, `perf` | All 24 commands are built (PRs #98, #101). anneal is 63k lines of non-test Rust in one crate, with 12 skills and 14 hooks |

So "Loop conforms to Core" is still right but now costs real work, because Loop's store is 10k lines with its own guarantees. Renaming assay is no longer free, only cheap while it is unreleased.

### Where the evidence pushes back

| Decision | What the audits found | Blueprint's position |
| --- | --- | --- |
| One shared event file, `<git-common-dir>/redkiln/events.sqlite3` | Loop asserts WAL, runs several processes and rebuilds its tier-1 tables from git. Review's CI jobs run on separate machines with no shared disk. anneal fails on the first foreign event, uses unprefixed type names, and refuses a newer `user_version`, so one plugin's upgrade would lock out another. temper writes from every Bash and Edit hook | Keep it as the append-only event log. Projections are disposable and may live in per-plugin files. CI and cloud sessions write journals that import into the log. Core owns the schema version, lenient reads and namespaced types. Loop gives up WAL if its contention test (R-SQLITE) passes |
| Capacity is a Core library | The need crosses machines: GitHub runners, a laptop and unattended builds share one subscription. assay's unattended build paused at 90% of the weekly limit. assay already designed leases over git refs (ADR-19, ADR-23) | Core owns a protocol with two backends: the local log, and a git-refs ledger for anything cross-machine |
| Calibration is a Core library | anneal's Clef log is per user across repositories; assay promotes per consumer repository | Two scopes, user and repository; promotion is always a human config edit |
| Core is the only shared layer | Four pieces exist twice: shell guard, `claude -p` dispatcher, GitHub client, enforcement engine | Add a kits layer, outside Core because it is behaviour |
| Redkiln Review is a plugin | Most of assay is a CI deployment: workflow templates, a GitHub App, a ledger repository, a binary pinned by digest. A marketplace install cannot deliver that trust anchor | Review has two deliverables: the CI deployment and a thin plugin |
| Redkiln Bench is anneal as one plugin | anneal's binary carries about 15k lines of skill QA for any plugin and about 9k of PR and fan-out tooling that Review and Loop need | Bench stays one plugin with a slimmer binary (section 4) |
| Backlog is GitHub Issues | Client repositories and non-GitHub Cowork folders cannot depend on Issues, and client items must stay local. Loop needs item content at a hash on every entry, not only at start | Backlog is opt-in by repo class; Loop re-snapshots on every entry |
| Weigh-In stays its own plugin | weigh-in's slice 3 planned to dissolve into Redkiln, and nothing records that this is dropped | Record the supersession; Loop's human gates become Weigh-In decision requests |
| temper's 2.0 port as planned | The plan ships through npm (its own research says that misses Cowork and org-synced installs), forbids async (happenstance needs it), allows four crates, keeps 1.x formats with no migration, and reserves `temper-*` crate names | Port inside the monorepo as Redkiln Rust 2.0. Reopens port decisions D1, D2, D3 and D10 |

Nothing contradicts: suite rather than monolith, Knowledge in the first wave, Docs deferred, or "craft" as the standards family.

## Design rules

Ten rules set the package boundaries. Most are already decided in one of the six repositories.

1. **Share formats, embed code.** Every binary links Core statically. A missing sibling turns a feature off; it never fails.
2. **Rule of two.** Code enters Core or a kit only when two packages need it now and at least one already has it. Every kit below meets this today.
3. **Core holds formats and the code that reads and writes them, never policy.** Gates, guards, prompts and UI live in plugins or kits.
4. **A domain has one owner.** Decisions belong to Weigh-In, atoms to Knowledge, work items to Backlog, findings and verdicts to Review, the Rust standard to Redkiln Rust, the process catalog to Loop (its format to Core). Others reach a domain through its contract crate, its events or its `--json` CLI, never its internals.
5. **Events are facts; projections are disposable.** The event log is append-only and shared; anything derived from it can be rebuilt. Each plugin says which of its state is authoritative (Loop ADR-0109, the deleted-database test).
6. **Render, never parse.** Typed values are the source and markdown is a rendering (Loop ADR-0005, ADR-0189). Text receipts between plugins are replaced by typed contracts.
7. **Hooks fail open, guards fail closed, no hook waits on a sibling.** Matching hooks run in parallel in no fixed order, so each is independent and idempotent.
8. **A model's judgement does not drive control flow until it is scored.** Record, label, calibrate; promotion is a human edit (Loop ADR-0079, assay QA-05, anneal Clef).
9. **Approvals are bound to content.** An approval names the hash of what was approved and is void when the hash moves.
10. **Client data stays local.** A repository marked `client` never has its events, journals or items published, synced or projected off the machine (weigh-in wi-a93f02, wi-7c21e4). Core enforces this.

## The packages

The suite has five kinds of package, so "package" does not have to mean "plugin".

| Kind | Installed by users? | Examples |
| --- | --- | --- |
| Spec | No | Core SPEC, schemas, conformance fixtures |
| Library crate (Core, kit, contract, domain) | No, linked into binaries | `redkiln-core`, `redkiln-guard`, `weighin-contract` |
| Binary | Inside a plugin's `libexec/`, or by digest in CI | `weighin`, `bench`, `review`, `redkiln`, `knowledge`, `backlog` |
| Plugin | Yes, from the Redkiln marketplace | Redkiln Rust, Bench, Weigh-In, Knowledge, Review, Loop, Backlog, the bundle |
| Deployment | Yes, by a repository owner | Review's workflow templates, GitHub App and ledger repository |

### Redkiln Core

Core makes the suite's shared data mean the same thing in every binary. It has no binary and no behaviour.

| Part | Scope | Built from |
| --- | --- | --- |
| SPEC (versioned on its own) | Store location; event envelope, namespacing and tags; schema-version rule; journal format; approval record; end-of-turn grammar; CLI result envelope and exit codes; `status --json` convention; hook conventions; repo class; artifact rendering (Layout C); process-catalog format | anneal's envelope, exit codes and hook metadata; weigh-in's marker grammar; temper's Status grammar; Loop ADR-0005, 0095, 0131, 0189 |
| Conformance kit | Golden fixtures per schema; a foreign-events-are-skipped test; the two-process contention test (R-SQLITE); the all-plugins-installed end-to-end test | assay's 75-fixture contract discipline; happenstance-testkit |
| `redkiln-core` | Store location resolver; TRUNCATE opening with WAL conversion and stepwise migration; one schema version; envelope, `<plugin>.<type>` names, tag validation; lenient typed reads; DCB guard helpers; content-hash ids; canonical digests; git helpers including the tree hash (Loop ADR-0105 clause 4); journal export and idempotent import; surface detection (Claude Code, cloud, Cowork, Projects thread, Windows desktop); repo-class gate. A synchronous API with tokio hidden inside | anneal-ledger's typed opening; weigh-in's WAL conversion and 0.1.x/0.2.x fixtures; anneal `paths.rs`; weigh-in `id.rs`, `git.rs`, journal; Loop `identity.rs`, `canon.rs`; weigh-in #126, #127 |
| `redkiln-calibrate` | Judgement records, outcome labels, scores (Brier, reliability bins, Wilson and Newcombe intervals, MCC), promotion verdicts; user and repository scope | anneal `calibrate/` (2,145 lines); assay QA-05, QA-12; Loop's facet provenance and stability gate; weigh-in's eval seed |
| `redkiln-capacity` | Lease protocol (holder, run, reserved, expiry), reserve and final cost records, rate-limit readings in integer ticks; a local backend on the log and a git-refs backend | Loop ADR-0110; assay ADR-19, ADR-23 and `aeu.rs` |

### Kits

Each kit is a library owned by the suite, depending only on Core.

| Kit | Job | Consumers | Built from (today's duplicates) |
| --- | --- | --- | --- |
| `redkiln-guard` | What a shell command reads, writes, deletes or runs: lexer, target resolution, denylist with per-plugin rule packs, protected globs, credential redaction | Rust, Bench, Loop, Review | temper `shell.mjs` (1,090) and glob; anneal `guard/` (3.8k) and `redact/` (1.5k) |
| `redkiln-dispatch` | Run `claude -p` safely and account for it: argv profiles, env denylist, `--agent` isolation, auth preflight, stream-json parser, TTL checks, process containment | Loop, Review, skillcheck | assay-runner and `forge::process`; Loop's `rk-adapter-claude` generic half and `rk-containment` (2.7k) |
| `redkiln-github` | Issues with types, sub-issues, dependencies and labels; issue snapshots with a content hash; PR diffs and patch-id; check runs; wait-for-CI | Backlog, Bench, Loop, Review | anneal `dag`, `pr_diff.rs`, `watch`; Loop's `Tracker` port (fake only); assay #53 (unbuilt) |
| `redkiln-craft` | The language-neutral enforcement engine: change fingerprint, reviewer-verdict validation, completion-gate choreography, baseline ratchet, gate runner with cache | Rust now, TypeScript and Python later, Bench | temper's \~3,500 agnostic lines; anneal `verdict.rs`, `baseline.rs` |
| `redkiln-catalog` | Load, lint and select from the process catalog | Loop first, then Bench | Loop's `rk-kernel` catalog, predicate, lattice and select modules |

Two dev tools are never shipped: `skillcheck` (skill lint and eval Tiers 1 to 3 for every plugin's CI, from anneal's \~15k lines) and `xtask` (libexec packaging, version agreement, changelog, marketplace assembly, from three copies today).

### Redkiln Rust (from temper)

*Rust that meets the standard, enforced as you write it.*

- **Is:** the rust-craft standard (182 rules), the three subagents, `/setup`, `/implement`, `/review`, `/gate`, `/audit`, the hooks, the workspace templates, and a CI CLI (`gate`, `scan`, `baseline --check`, `report`, `status --json`).
- **Owns:** the standard and rule registry; Rust policy (scanner, lexer, manifest rules, cargo steps); the Rust dimensions of the reviewer rubric; the checks assay re-implements today (test inventory, manifest switches, lints weakening), offered as subcommands so Review keeps only an adapter.
- **Not:** the generic engine (`redkiln-craft`), the shell reader (`redkiln-guard`), PR verdicts (Review).
- **From temper:** the 2.0 port happens here, keeping its parity discipline (byte-identical fingerprint, the 365 Node tests as oracle). Vendored `.claude/temper/` copies are retired.

### Redkiln Bench (from anneal)

*Work through a change with Claude: grill, spec, plan, build, verify.*

- **Is:** the phase skills (grill, spec, plan, build, diagnose, verify, a trimmed ship), in-session fan-out (`/orchestrate`, `brief`, `round`), `/handoff`, `/correct`, the 14 hooks, Clef's seven judgement points, and the define-phase checks (`lint spec|plan|terms|brief|rules`, `plan approve|check`, `repro`, `floor-guard`, `task done`).
- **Owns:** phase state, grill and plan approvals, briefs, repros, Clef points and thresholds, the claim gate.
- **Hands off:** the guard engine to `redkiln-guard`; skill QA to `skillcheck`; `dag`, `pr_diff`, `watch` to `redkiln-github`; verdicts to Review (merge-gate reads `review.verdict`); calibration scoring to Core; `.out-of-scope/` rulings to Knowledge; `orchestrate.ruled` to Weigh-In.
- **Freezes:** `rollout`, `perf`, `migrate` (about 1.7k lines, no metrics source, no user) until a Ship product exists.
- **Tags:** each skill names the catalog activity it implements, checked against Loop's catalog in CI.

### Weigh-In (from weigh-in)

*Agents' assumptions settled by you, at the end of the chat.*

- **Is:** the `/weigh-in` sitting, the capture hooks, the board, the `weighin` CLI.
- **Owns:** the decision domain: every question, assumption, deferral and call from any plugin, with door, authority, options, default and deadline, and the sittings that settle them. One read API (`open --json`, `ci-check --json`) serves Bench, Loop and Review.
- **Contract crate:** `weighin-contract` lets Bench, Loop and Review raise items by appending typed events, instead of piping text to `weighin batch`.
- **Hands off:** the atom renderer (`atom.rs`, 1,107 lines) to Knowledge; the marker grammar's shell to Core; `store.rs`, `env.rs`, `git.rs` (about 1,700 lines) to Core.
- **Supersedes:** slice 3 (dissolving into Redkiln as a `decision` item type). Record this as a decision.

### Redkiln Knowledge (new)

*Your repo remembers what was decided and why, and every session starts knowing it.*

- **Is:** the `.kb` format and lifecycle (intake, promotion, supersession, index), retrieval, injection into sessions under a context budget, and `validate`.
- **Owns:** atoms of kinds decision, constraint, finding, evidence, learning, term and ruling; the records conventions (Loop ADR-0085, 0092); the atom-set digest per activity that Loop's memo key needs.
- **Contract crate:** `knowledge-contract` (atom schema, writer, digest).
- **Built from:** weigh-in `atom.rs`; the old TypeScript Redkiln's `.kb` frontmatter; anneal's `.out-of-scope/` rulings; Loop's `.redkiln/kb/` and records conventions.
- **Producers:** Weigh-In, Bench, Review, Rust. **Consumers:** every SessionStart, Bench's ruled-before hook, Loop's activity cards, Review's reviewers.

### Redkiln Review (from assay)

*Whether a PR may merge, as one required check.*

- **Is, as a deployment:** the `review` binary by digest on GHCR, base-defined workflow templates, the verdict GitHub App, the ledger repository; pure `decide`, rubric, tiering, findings validation, blind specialists and verifier, sanitiser, replay bundles.
- **Is, as a plugin:** `/review:check` (local, advisory), `/review:status`, later a read-only view.
- **Owns:** findings and verdicts: the findings schema, the verifier, the rubric, `decide`, and the `review.verdict` event Bench and Loop read. Loop's re-execute-before-blocking rule (ADR-0074) becomes a verifier mode here; it is assay's deferred executable witness.
- **Contract crate:** `review-contract`, so temper's reviewer, Bench's doubt pass and Loop's review gate emit one finding shape.
- **Uses:** `redkiln-dispatch`, `redkiln-github`, `redkiln-capacity` (git-refs backend), `redkiln-calibrate`, Redkiln Rust's CLI pinned by digest, Weigh-In's `ci-check`.
- **Keeps its own state model** (per-job JSONL plus the ledger repository, ADR-07, 09, 15). Local mode writes `review.*` events; CI runs export a journal.

### Redkiln Loop (from redkiln-rs)

*Claude works the backlog while you're away.*

- **Is:** the `redkiln` binary (`run`, `answer`, `status`, `explain`, `cost`, `doctor`, `init`, `lint`, `catalog search`), the kernel, the process catalog (12 packs, 12 stages, 102 activities, 8 cards), run worktrees, git trailers as tier-1 authority, toolchain adapters, and a small hand-written plugin (`/loop:run`, `/loop:answer`, `/loop:status`).
- **Owns:** unattended execution, the catalog's content, the trailer grammar, its own projection store.
- **Hands off:** item records to Backlog (whose client replaces the `Tracker` port); `.redkiln/kb/` to Knowledge; `DocsSite` to the deferred Docs; tier-2 events to the Core log; containment and the generic dispatcher to `redkiln-dispatch`; headroom to `redkiln-capacity`; facet provenance to `redkiln-calibrate`; `review/` to Review; lint and dependency policy (ADR-0083, 0084, 0100) to Redkiln Rust.
- **Human gates become Weigh-In decision requests;** `redkiln answer` stays as a fallback, which ADR-0009 allows.
- **Drops:** `rk-plugin-gen`, the generated `rk-` command prefix, `.bklg` and in-repo items.
- **Keeps:** its 198 ADRs, AC and SM registers and runbook as they are, under `docs/loop/`.

### Redkiln Backlog (new, second wave)

*Your agents read and update your issues properly.*

- **Is:** a skill teaching the suite's issue conventions, commands to cut, link and snapshot issues, and the label taxonomy.
- **Owns:** work items on GitHub: item types as issue types, hierarchy as sub-issues, blocking as dependencies, state and facets as labels (including Loop's orchestrator-only namespace, AC-507); issue snapshots as events, re-taken on every Loop entry.
- **Contract crate:** `backlog-contract`, over `redkiln-github`.
- **Built from:** anneal's GitHub tracker; redkiln-rs `docs/BACKLOG.md`; the conventions assay, temper and redkiln-rs already use.
- **Opt-in by repo class.** Client repositories and non-GitHub folders run without it; Weigh-In keeps deferrals and Bench's `dag` uses local tickets.

### Redkiln, the bundle

A manifest with a name and a dependency list, so `install redkiln@redkiln` installs the set. It carries no code. A suite-wide status view (temper's lens, #107, generalised over every plugin's `status --json`) would ship here later.

### Later, named so the gaps don't fill piecemeal

- **Redkiln TypeScript, Python:** a policy layer per language on `redkiln-craft`.
- **Redkiln Ship:** release, rollout, watch, incidents, starting from Bench's frozen commands and Loop's release and operate stages, when there is a user.
- **Redkiln Docs:** deferred as decided.

## How they compose

Plugins meet in four places: namespaced events in the shared log, one approval record, Weigh-In decision requests, and Review's findings. They never call each other's internals.

### Who writes and reads what

Event types are named `<plugin>.<type>`, and readers skip types they do not know.

| Data | Written by | Read by |
| --- | --- | --- |
| `rust.*`: gate runs, verdicts, scan denials, telemetry | Redkiln Rust | Rust, Bench (build, verify), Loop (verify stage), suite status |
| `bench.*`: mode, grill and plan approvals, task done, repro, briefs, rounds, Clef decisions | Bench | Loop (an approved plan answers its planning gate), Weigh-In |
| `weighin.*`: items and their lifecycle | Weigh-In, and any plugin through `weighin-contract` | Bench, Loop, Review (`ci-check`) |
| Atoms in `.kb/` | Knowledge, called by Weigh-In, Bench, Review, Rust | Every SessionStart; Loop, with the atom-set digest |
| knowledge.\*: atoms written, status changes, injections, validate runs | Knowledge | Knowledge (its disposable projection, usage measures), Weigh-In, suite status |
| `review.*`: verdicts, finding dispositions | Review in local mode; CI journals | Bench (merge-gate), Loop (review stage), Knowledge |
| `loop.*`: tier-2 run events | Loop | Loop, suite status |
| `backlog.*`: issue snapshots | Backlog, or Loop through `backlog-contract` | Loop, Bench (`dag`) |
| `core.*`: capacity leases and cost, calibration records, journal imports | Core libraries, for any plugin | Loop and Review (capacity), every judgement point |

### Approvals

Core holds one approval record: subject kind, subject hash, hash method, scope, decider, verdict, time and optional expiry. The hash method names how the hash was taken (temper's change fingerprint, anneal's patch-id, Loop's tree hash, a file's sha256), and Core implements each method once. A Bench plan approval answers Loop's planning gate when Loop recomputes the same hash before acting, so no cross-plugin guard is needed.

### Decision requests

Any plugin that needs the human raises a Weigh-In item: Bench parking a one-way call, Loop at a human gate, Review needing an owner ack, Rust asking to approve a lint exception. Weigh-In settles it in a sitting or on the board, the raiser reads the `weighin.item_decided` event, and Knowledge records the atom. With no human present (Loop, Review), two-way judgement items may take their default and one-way items block (wi-4d8b6a).

### Findings and verdicts

Review owns the one findings schema. temper's reviewer, Bench's doubt pass and Loop's review gate all emit it, and Review's verifier checks it, optionally by re-running a typed repro. A verdict is a `review.verdict` event bound to a subject hash; Bench's merge-gate and Loop's review stage require a fresh one.

### One session, several plugins

- **End of turn:** one grammar in Core. A `Status:` line from one closed set (DONE, DONE\_WITH\_CONCERNS, WIP, NEEDS\_CONTEXT, BLOCKED) that every plugin accepts, so Bench stops recording Rust's WIP as a violation. Then optional `Gate:`, `Review:`, `Changed:`, `Decisions needed:`, `Declined:`, `Not verified:` lines, then each plugin's fenced block (`weigh-in` today).
- **Stop hooks:** Rust's gate, Bench's completeness check and Weigh-In's capture run in parallel; each caps its own blocks and none blocks for another plugin's concern.
- **SessionStart:** a suite context budget split by convention; each plugin injects less when it sees siblings in the log.
- **Guards:** Rust and Bench both guard Bash with different rule packs over the same `redkiln-guard` engine, so they agree on what a command does.
- **Conformance:** the all-plugins-installed end-to-end test is the kit's top test.

### The process catalog

The catalog is Loop's content and a suite format. Its format goes into Core's SPEC now, because Bench's skills already name catalog activities and the two lists must be checked against each other. Its engine moves into the `redkiln-catalog` kit now; Loop is its first user and Bench its second (call 8, pressure-test rec 16).

## Data and state

One shared event log holds the facts; everything else is either disposable, an export of the log, or deliberately remote.

| Store | Where | Mode | Authority |
| --- | --- | --- | --- |
| Event log | `<git-common-dir>/redkiln/events.sqlite3`, overridable by flag, `$REDKILN_STORE` or `.redkiln/suite.toml`; `<folder>/.redkiln/events.sqlite3` outside git | happenstance, TRUNCATE, opened only by `redkiln-core`; one schema version, additive changes only | Authoritative for Weigh-In, Bench and Rust; tier 2 for Loop |
| Projections | In the log, or `<git-common-dir>/redkiln/<plugin>.projections.sqlite3` | Disposable, rebuilt from the log (and from git, for Loop) | Never authoritative. Loop's tier-1 mirror and memo live here |
| Git | Trailers (`Story:`, `Gate:`, `Weigh-In:`), committed atoms and config | — | Tier 1 for Loop; the durable record of decisions |
| Journals | `.redkiln/journal/<session>.jsonl` (committed, own repositories), a telemetry branch where commits are unwelcome, CI artifacts for Review | JSONL of envelopes | Export and idempotent import. One answer to cloud containers losing `.git/`, replacing weigh-in's committed journal and temper's telemetry branch |
| User scope | `~/.redkiln/` | Core-managed files | Calibration pooled across repositories; per-user counters |
| Remote | A private ledger repository, git refs with compare-and-set | `redkiln-capacity`'s git backend; Review's per-PR state | Cross-machine leases (assay ADR-19, ADR-23) |
| Repo config | `.redkiln/suite.toml` (repo class, store override, surfaces, and one `[<plugin>]` table per plugin) plus `.redkiln/<plugin>/` for each plugin's own files. One exception: Review reads its trust-path config from the base branch | TOML | Committed and reviewed |

Rules that make the shared log safe:

- **Lenient reads.** A reader decodes only its own types and skips the rest. This is a conformance test, and the change anneal most needs.
- **One version stamp.** Only Core writes `user_version`. A plugin built on an older Core refuses only a newer major schema; additive changes never lock a sibling out.
- **Nothing on the hot path waits.** Hook writes use a bounded busy timeout; best-effort events such as telemetry are dropped and counted when the lock is busy; guards decide first and record second.
- **Migration imports, never deletes** (Cowork refuses deletes). Old ledgers under `weighin/` and `anneal/`, and committed journals, are imported once.
- **Repo class.** A `client` repository never exports journals, pushes a telemetry branch, publishes a board or projects items to GitHub.
- **WAL.** Loop gives up WAL for the log if R-SQLITE (two processes, parallel worktrees) passes under TRUNCATE. If it fails, Loop keeps tier-2 events in its own WAL file and exports a journal into the log. Either way the suite has one event format.

## Distribution

Plugins ship with prebuilt binaries through one Redkiln marketplace; Review's deployment ships separately by digest.

- **Plugins** ship as `plugins/<name>/` with binaries in `libexec/<target>/` behind one shared launcher (sh and `.cmd`, newest build wins, no `bin/`). This is what weigh-in and anneal do today, and what temper's own research says reaches Cowork and org-synced installs.
- **One marketplace, `redkiln`,** is assembled at release from per-plugin artifacts. Claude Code users add the monorepo's marketplace branch. claude.ai org sync reads a release-only repository whose default branch is that marketplace (weigh-in's B-19 pattern, generalised, with one deploy key). **Unverified:** whether org sync accepts several plugins in one repository and honours plugin dependencies. This needs a spike before the migration plan relies on it.
- **Review's deployment** ships outside the marketplace: the binary by digest on GHCR, the workflow templates and the App. A consumer repository pins Review and Redkiln Rust by separate digests, so both stay independently releasable.
- **Versions and tags are per package.** Release tags are `<install-id>--v<version>` (`rk-rust--v2.0.0`, `rk-bench--v0.1.0`, `rk-weigh-in--v1.0.0`), each package with its own CHANGELOG. The SPEC has its own version and each plugin declares the version it conforms to. The bundle bumps when a member does.
- **Plugin install IDs:** display names as decided; recommended IDs `rk-rust`, `rk-bench`, `rk-knowledge`, `rk-review`, `rk-loop`, `rk-backlog`, `rk-weigh-in`. Weigh-In keeps its bare skill name `/weigh-in`, since Cowork lists skills by their own name (D-0004).

## The monorepo

One repository, one Cargo workspace on the 1.98.1 toolchain with MSRV equal to the pin, and a structural test that enforces the layers.

| Path | Holds |
| --- | --- |
| `AGENTS.md`, `.claude/CLAUDE.md` | Suite charter: crate map, layer rules, decisions log |
| `Cargo.toml`, `rust-toolchain.toml`, `deny.toml`, `clippy.toml`, `rustfmt.toml`, `.gitattributes` | One workspace; LF everywhere, keeping Loop's `-text` fixture rules |
| `spec/` | Core SPEC, schemas, conformance fixtures |
| `crates/core/` | `redkiln-core`, `redkiln-calibrate`, `redkiln-capacity` |
| `crates/kits/` | `redkiln-guard`, `redkiln-dispatch`, `redkiln-github`, `redkiln-craft`, `redkiln-catalog`, `redkiln-view` |
| `crates/rust/` | `rust-craft-domain`, `rust-craft-app`, `rust-craft-cli` |
| `crates/bench/` | `bench-core`, `bench` (binary) |
| `crates/weigh-in/` | `weighin-contract`, `weighin-core`, `weighin` (binary) |
| `crates/knowledge/` | `knowledge-contract`, `knowledge` (binary) |
| `crates/review/` | `review-contract`, `review-domain`, `review-app`, `review-forge`, `review-gates`, `review` (binary) |
| `crates/loop/` | `rk-types`, `rk-ports`, `rk-kernel`, `rk-store`, `rk-git`, `rk-adapter-*`, `rk-cli`, `redkiln-migrate` |
| `crates/backlog/` | `backlog-contract`, `backlog` (binary) |
| `plugins/` | One folder per plugin plus `redkiln/` (the bundle): manifest, skills, agents, hooks, `libexec/` launcher |
| `deploy/review/` | Workflow templates, App manifest, fetch-by-digest action |
| `tools/` | `xtask`, `skillcheck` |
| `templates/` | Rust workspace templates and the example workspace, excluded from the workspace |
| `evals/<plugin>/` | Plugin evals |
| `docs/` | Suite decisions; `docs/<plugin>/` per plugin, with Loop's references and runbook under `docs/loop/` |
| `.kb/` | One knowledge base. Today's 6-hex `wi-` ids can collide across clones, so new ids move to 10 hex (old ids are kept) and CI checks for duplicates across the suite |

**Dependency rules, checked by a structural test** (Loop and assay already use this pattern):

- Core crates depend on nothing internal; kits and contract crates depend only on Core.
- A plugin's crates depend on Core, kits and other plugins' contract crates, never another plugin's internals.
- `anyhow` only in binaries, and every binary builds alone (`cargo check -p`).
- Loop keeps its stricter rules: no tokio in the kernel, git through one wrapper, no egress in kernel tests.

**CI** is path-filtered per package over one shared job set: fmt; the Redkiln Rust gate on the whole workspace, replacing four vendored copies; tests on Linux and Windows (macOS on release); `cargo deny`; the conformance kit; `skillcheck` over every plugin; `claude plugin validate --strict` per plugin; version and changelog checks per package.

## Where today's code lands

Every significant piece of today's code has a home; nothing is rewritten from scratch except kb-ingest's behaviour.

| From | Today | Lands in |
| --- | --- | --- |
| temper | `skills/`, `agents/`, `templates/`, rule registry, Python checks | Redkiln Rust plugin and `rust-craft-*` crates, as the 2.0 port |
| temper | `shell.mjs`, glob; agnostic verdict, gate, baseline, fingerprint | `redkiln-guard`; `redkiln-craft` |
| temper | Telemetry | `rust.*` events and journals |
| temper | Four vendored `.claude/temper/` copies | Deleted; the workspace runs Redkiln Rust directly |
| weigh-in | `weighin-core` (event, projection, card, id, marker, scan, board) | `weighin-core`, `weighin-contract`; marker grammar's shell to the SPEC |
| weigh-in | `atom.rs` | `knowledge-contract` |
| weigh-in | `store.rs`, `env.rs`, `git.rs`, journal | `redkiln-core` |
| weigh-in-plugin | Release-only repository | Replaced by the suite's release-only marketplace repository |
| anneal | `anneal-ledger` | `redkiln-core` plus `bench.*` event types |
| anneal | Hooks, skills, define lints, Clef, orchestrate | Bench |
| anneal | `guard/`, `redact/`, `glob.rs` | `redkiln-guard` |
| anneal | `dag`, `pr_diff`, `watch`; `process.rs` | `redkiln-github`; `redkiln-dispatch` |
| anneal | `verdict`, merge-gate | `review.verdict` event; merge-gate stays a Bench hook |
| anneal | `calibrate/`, thresholds | `redkiln-calibrate`; the Clef client stays in Bench |
| anneal | `lint skills`, `eval` | `tools/skillcheck` |
| anneal | `rollout`, `perf`, `migrate` | Frozen in Bench, earmarked for Ship |
| assay | `assay-domain`, `-app`, `-forge`, `-gates`, `-cli` | `review-*` crates |
| assay | `assay-runner`, `forge::process` | `redkiln-dispatch` |
| assay | temper re-implementations in `assay-gates` | Redkiln Rust subcommands; Review keeps the adapter |
| assay | Ledger and semaphore design | `redkiln-capacity` git backend |
| assay | Schema and fixture discipline | SPEC practice and the conformance kit |
| redkiln-rs | `rk-*` crates | Loop |
| redkiln-rs | `rk-containment`, generic half of `rk-adapter-claude` | `redkiln-dispatch` |
| redkiln-rs | Headroom and throttle; facet provenance | `redkiln-capacity`; `redkiln-calibrate` |
| redkiln-rs | Store connection and identity rules | `redkiln-core`, with TRUNCATE |
| redkiln-rs | `backlog.rs`, `item_write.rs`, `Tracker` port | Backlog |
| redkiln-rs | `.redkiln/kb/`, records conventions | Knowledge |
| redkiln-rs | `review/` | Review's verifier |
| redkiln-rs | Catalog format and predicate grammar | SPEC (format); the engine to the redkiln-catalog kit |

## Constraints on the migration

These shape the order of work, not the end state.

- **Two pilots are frozen.** temper's hooks and gate until the pilot report around 2026-10-18 and v1 (1.4.0) after it (wi-b63bd6). Weigh-In's capture until 1.0.0, on or after 2026-10-20 (wi-7c3aa0). Moving their stores or hooks earlier invalidates the measurements.
- **redkiln-rs's work is not on `main`.** `build/phase-0-to-1`, about 490 commits ahead, must land first (#24). Its test corpus has no off-machine copy (`D:/repos/redkiln-corpus`, #32); back it up before anything moves.
- **assay's owner setup hasn't happened** (App, ledger repository, approver, agent identity, #39 to #42). Renaming before those exist is free.
- **anneal's v0.1.0 marketplace release (#44)** would create a public `anneal` install ID just before the rename. Hold it, or publish only into the Redkiln marketplace.
- **happenstance 0.3.2 to 0.4.1** is a migration needing fixtures from 0.1.x and 0.2.2 ledgers. Do it once, in Core.
- **Path assumptions break on the move.** Loop's CI greps `crates/` 40 times and its runbook holds 241 `D:/repos` paths. assay's tests assert exactly six crates and treat all of `crates/**` as its trust path. temper, weigh-in and anneal assume the repository root is the plugin and marketplace root.
- **Owner-only steps:** rulesets and required-check names, the `release` environment, crates.io trusted publishing (bound to temper's repository), deploy keys, org-sync settings, GHCR. The migration plan lists these for Ryan.
- **Install IDs in use:** `temper@temper`, `weigh@weigh-in`, `anneal@anneal`, plus adopters' `extraKnownMarketplaces` and cloud setup scripts. Old repositories keep a redirecting marketplace for a while.
- **Decision logs merge without renumbering.** Each series (Loop's ADR-0001 to 0198, anneal's `docs/adr/`, weigh-in's D- and B- records, assay's ADRs) stays under its package's `docs/` with its prefix. assay already has two ADR-23s (the spec's and #125's), which needs fixing either way.

## Calls for Ryan

Eleven calls, each with a recommendation; pick a value in the last column or comment on the row. One-way calls are hard to undo once something is built on them.

| # | Call | Recommendation | Door | Ryan's call |
| --- | --- | --- | --- | --- |
| 1 | Store shape: the shared event log as decided, plus disposable per-plugin projection files, journals for CI and cloud, and Loop off WAL if R-SQLITE passes | Yes. Flip if R-SQLITE fails or Loop's tier-2 volume proves large | Two-way |  |
| 2 | Capacity as a protocol with a git-refs backend (assay's design), not a library over the local file | Yes | One-way once Review's ledger repository exists |  |
| 3 | A kits layer (guard, dispatch, github, craft) and contract crates, with Core kept formats-only | Yes | Two-way |  |
| 4 | Review is a CI deployment plus a thin plugin, delivered by digest outside the marketplace | Yes | Two-way |  |
| 5 | temper's 2.0 port becomes Redkiln Rust 2.0 in the monorepo: libexec not npm (D1, D6, D13), a generic/Rust split (D3), Core's store with hidden tokio (D10, state compatibility), new crate names (D2), and raising assay's MSRV from 1.96 to 1.98.1 (an H-16 decision) | Yes | One-way for published crate names |  |
| 6 | Bench slims: guard and GitHub code to kits, skill QA to a dev tool, verdicts to Review, `rollout`/`perf`/`migrate` frozen | Yes | Two-way |  |
| 7 | Loop's human gates become Weigh-In decision requests, and Weigh-In's slice 3 is formally superseded | Yes | Two-way |  |
| 8 | Process-catalog format into the SPEC now, and where the catalog engine lives | Move the engine into the redkiln-catalog kit now; Loop is the first user and Bench the second (pressure-test rec 16) | Two-way |  |
| 9 | Plugin install IDs `rk-*`, with `/weigh-in` kept as the bare skill name; Weigh-In's install ID is rk-weigh-in | Yes | One-way once colleagues install |  |
| 10 | Backlog is opt-in by repo class, so client repositories and non-GitHub folders never depend on it | Yes | Two-way |  |
| 11 | Spike claude.ai org sync with several plugins and dependencies in one repository, before committing to one release repository | Yes, first | Two-way |  |
