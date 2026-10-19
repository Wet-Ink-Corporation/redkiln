# Audit: redkiln-rs (to become Redkiln Loop)

*Audited 2026-10-13 for the Redkiln Blueprint. Read-only. Evidence tags: [measured] = counted or run by me; [read] = stated in the repo; [inferred] = my judgement.*

> **Read this first: `main` is stale.** The local checkout at `/home/claude/redkiln-rs` and GitHub `main` stop at `353ea01` (2026-09-11, 63 commits, design-only, 112 ADRs, no `crates/`). The real work is on the **`build/phase-0-to-1`** branch at `e54ccac` (2026-10-09, 552 commits): 12 crates, ~238k lines of Rust, 1,453 `#[test]`s, **198 ADRs**, CI green on Linux and Windows [measured: scratch clone of `origin`]. The suite write-ups (`suite-structure.md:135` "no commits since 2026-09-11", "112 ADRs", "build stages all not started") describe `main` and are wrong about the project's state. Landing the branch on `main` is itself open work (#24). Everything below cites the build branch unless it says `main`.

---

## 1. Identity

**Job.** One Rust binary, `redkiln`, that owns the SDLC loop headlessly. A deterministic kernel handles stage selection, gates, budgets, retry, memo and replay. It dispatches one `claude -p` child per activity, gives each child its own card, schema, tool set and cost row, and records decisions as git trailers first and events second (`references/2026-09-08-architecture-design.md:9`) [read].

| Fact | Value |
|---|---|
| Version | No version or release yet. Every crate has `publish = false` (`Cargo.toml` `[workspace.package]`) [measured] |
| Commits | 552 on `build/phase-0-to-1`, 63 on `main`. First commit 2026-09-04, last 2026-10-09 (3 commits that day; 1 each on 10-07 and 10-08) [measured] |
| LOC | Rust: 584 files, **238,239 lines**, of which ~137k are under `*/tests/*`. Markdown: 516 files, 136,683 lines. TOML: 604 files, 29,201 lines. Python: 55 files, 16,109 lines (spike instruments and runbook validators). JSON: 59,917 lines (fixtures and recorded transcripts) [measured, `git ls-files`] |
| Tests | 1,453 `#[test]`s. By crate: rk-kernel 448, rk-cli 333, rk-git 159, rk-adapter-claude 158, rk-store 98, rk-types 83, rk-adapter-toolchain-rust 81, redkiln-migrate 26, rk-ports 26, node 19, sqlite 16, containment 6 [measured, grep] |
| Records | 198 ADRs (`references/adr/`, MADR-lite with a required "What this forbids" block); an AC register of 7,523 lines (`references/2026-09-08-acceptance-criteria.md`); an SM register of 1,438 lines; `spikes/DECISIONS-2026-09-{09..21}.md`; the runbook (`runbook/`, 24 stage files) [measured] |
| Status | Phase 1 is about 65% done and the whole vision about 30%. "The engine is built and well tested against a simulated model. No real Claude-driven fix or story lane has run through `redkiln run`." M5 forecast: P50 2026-10-23 (`references/audit/2026-10-06-status/README.md:7`) [read, judgement by that audit] |
| Distribution | **None.** There is no `plugin/` directory, no marketplace, no release workflow and no binary packaging. Design §8.8 (`:1607`) is marked PROVISIONAL: `cargo-dist` plus a JS shim plus a generated plugin (`rk-plugin-gen`, Phase 2) [read] |
| Toolchain | Rust 1.98.1 pin, edition 2024, resolver 3, `rust-version` equal to the pin (ADR-0102), the same pin as weigh-in and anneal [measured, `rust-toolchain.toml`] |
| Dev tooling | Adopts temper (rust-craft) in its own CI: `.claude/temper/` is vendored, `.claude/temper-baseline.json` holds the ratchet, and `rust.yml` runs it. There is a `backlog-ledger` skill in `.claude/skills/` [measured] |

## 2. Functional shape

### 2.1 The CLI (`crates/rk-cli/src/verbs/`) — built

| Verb | What it does | State |
|---|---|---|
| `run <item> [--lane fix\|story]` | The only entry point. It is idempotent and has no `--resume` (ADR-0107, ADR-0128). It reconciles git into the store, selects, dispatches and commits | Built (`run.rs` is 6,481 lines, flagged as a god module in #70). **Never run against a real model**; #41, #62 |
| `answer <item> <option> [--attach k=path] --explain-sufficed --toml-opened` | Answers a human gate. Phase 1 has no other channel (ADR-0009, ADR-0129) | Built |
| `status`, `explain <item>`, `cost <item>` | Read-side roll-up; why this DAG ran, from the record; cost per activity (ADR-0163, ADR-0190) | Built. `status` has a per-item stage bug (#6) |
| `doctor` / `doctor --rebuild` | A configuration-time rule table at zero model cost. Rebuilds tier 1 from git | Built. Open: R-64 asks whether tags can be rebuilt (#76), and #22 |
| `init`, `lint`, `catalog search` | Idempotent init (writes `.redkiln/config.toml`); pack and catalog lint; keyword search | Built |
| `redkiln-migrate` (separate binary) | Converts the old TypeScript Redkiln's `.bklg`/`.kb` corpus into the typed layout. The only crate allowed a YAML dependency (ADR-0100) | v0 built (4,009 lines) |
| `doctor --probe`, `doctor --fix`, the tracker comment grammar, the web view | — | Planned (Phase 3) or CONDITIONAL |

**Plugin surface: none.** There are no skills, agents, hooks or commands for users. The only Claude Code assets are repository dev tooling (temper, the `backlog-ledger` skill). The tool is unattended by design: "every verb is a read of the world, a decision, and an exit; none of them waits" (design §8.1) [read].

### 2.2 Lanes and process
- **Built:** the `fix` lane (pack `bug-standard`) and the `story` lane (pack `story`). These are the two enabled packs (`.redkiln/config.toml`). Phase 1 is limited to these by ADR-0011 [measured].
- **Authored but not enabled:** 12 process packs in `spikes/p9/packs/` (bug-expedite, chore, docs, incident, initiative, project, security-vuln, spike, story-ue, tech-debt, plus the two above), and 102 catalog activities in `.redkiln/catalog/` with one row in `catalog-extended/` [measured].
- **Planned:** Phase 2 brings the remaining packs, parallel worktrees, a merge queue, .NET/Python/Unreal toolchains, `rk-plugin-gen` and corpus migration. Phase 3 brings GitHub and mdBook adapters, a web view and DORA metrics. Phase 4 brings the Codex adapter, only after S-CODEX passes (ADR-0167) (design §10.2 `:1839`) [read].
- **Two self-hosting items exist:** `.redkiln/items/rk-fix-doctor-test-count.toml` and `rk-story-codex-adapter.toml`. M5, "Redkiln-RS ran its own fix lane and authored a Phase-2 story", is not reached (PROGRESS §1, Step 8 "RE-OPENED 2026-09-24") [read].

### 2.3 Built vs stubbed vs planned
- **Built and tested against fakes:** selection, the predicate grammar, the lattice and rank-tie lints, memo, replay, the three gates (human, review, verify), reconcile, trailers, run worktrees, process containment, the Claude harness (argv, env denylist, throttle, TTL and headroom readers), the Node and Rust toolchains, the SQLite event store, layout-C rendering, and the migrator [read, PROGRESS §1 checkpoints].
- **Built but not wired into `run`:** budget exhaustion and degradation (#8, #43–#46), selection step 7 deviation (#14, #56), review quarantine and advisory retirement (#13, #52, #53), Ctrl-C cancel (#5), and the orphan/rewrite detectors (#15, #57–#60) [read, issues].
- **Stubbed:** the `Tracker` and `DocsSite` ports have fakes only (`crates/rk-ports/src/tracker.rs:37`, `docs_site.rs:19`). ADR-0134 records `tracker` in a warranted "expected-uncalled" set [measured].
- **In progress:** Step 8i, S-BIN (review without a text diff, the `ChangeSummary::Binary` arm), and 8j, the fixture proof of both lanes, which is "PLANNED, not started" [read].

## 3. Architectural shape

### 3.1 Components (the crate map, `AGENTS.md` and design §9.1 `:1692`)

| Crate | Layer | Src lines incl. tests [measured] | Owns |
|---|---|---|---|
| `rk-types` | domain | 7,888 | ids, typed artifacts, schema derivation (schemars), `StepError` (an 11-variant closed set, ADR-0095/0112), `EventKind` (**42 kinds**, `event_kind.rs:193`), `Drift`, `GateBasis`, the typed repro |
| `rk-ports` | ports | 4,181 | five **sync** traits held as `Arc<dyn Port + Send + Sync>`: `EventStore` (`append`, `append_tier1`, `query`, `snapshot`, `rebuild`), `Harness` (blocking `dispatch -> DispatchOutcome`), `Toolchain` (`detect/build/test/lint/classify/change_summary/run_verb`), `Tracker` (`project`, `inbox`), `DocsSite` (`build`, `check_links`, `drift`). Also one fake per port, the `CancelToken`, and a conformance suite |
| `rk-kernel` | application | **102,484** | modules `select/` (5.3k), `workflow/` (9.3k), `runtime/` (9.5k), `predicate/`, `review/`, `verify/`, plus lattice, rigor, the registry and facet lints, replay, memo and quality blocks. ADR-0076 bans tokio, futures and async-trait here |
| `rk-store` | infrastructure | 10,648 | the store at `<common-dir>/redkiln/store.sqlite3` (`conn.rs:84-89`), migrations, the canonical serialiser, the memo key, store identity, tier-1 tables, retention, layout-C render and locator, item write, the `toolchain:backlog` feed (`backlog.rs`) |
| `rk-git` | infrastructure | 15,717 | the one git exec wrapper, the trailer parse/write grammar, `reconcile()`, run worktrees (`worktree.rs` at 2,883 lines, #71), landing, release tags, review shallow clone, mutation. gix is restricted to reads (ADR-0077/0091/0098) |
| `rk-containment` | infrastructure | 2,708 | a Windows Job Object and a POSIX process group, bounded run. Holds the workspace's only `#![allow(unsafe_code)]` (ADR-0127) |
| `rk-adapter-claude` | adapter | 19,108 | the `claude -p` harness: argv per cache profile, card binder, env denylist, headroom/throttle, TTL verification, the stream reader, the dispatch record. tokio is confined here |
| `rk-adapter-sqlite` | adapter | 2,662 | `EventStore` over `rk-store` |
| `rk-adapter-toolchain-{node,rust}` | adapter | 3,626 / 10,248 | toolchain verbs. Rust: cargo/nextest output parsing and affected-test selection |
| `rk-cli` | binary | 54,544 | the composition root; the only place that reads env and owns the signal path |
| `redkiln-migrate` | binary | 4,009 | the legacy corpus converter |

**Layering is enforced mechanically**, not by convention. `ci.yml` has a `structural` job: anyhow is allowed only in binaries; the migrator is reachable only as its own binary; legacy formats are read only in the migrator; git is called through one wrapper; no worktree is resolved from cwd; there is a port-inventory check (AC-481/483/528). A `no-egress` job runs the kernel and pack suites with the network denied (AC-498). `tests/fixtures/*` holds must-fail fixture crates (AC-001..005) [measured, `.github/workflows/ci.yml:285-600`].

### 3.2 Runtime model
- **Headless CLI.** It runs from a shell, never inside a session. Each activity is a `claude -p` child with subscription auth only, never `--bare`, and it asserts that no API key is present (ADR-0010). A run exits at a gate and resumes on the next `run` (ADR-0017, ADR-0107) [read].
- **Activities run in a run-owned worktree** at `.redkiln/worktrees/<item>` on branch `rk-run/<item>`. The kernel makes content checkpoint commits there (ADR-0180, ADR-0188). It never writes to the user's checkout (AC-518) [read].
- **No in-session hooks or skills.** It is the opposite runtime from Bench, Weigh-In and Review, which are hook and skill plugins [inferred].

### 3.3 Data owned

| Store | Path / format | Authority |
|---|---|---|
| **Tier 1, git trailers** | `Story: <project>/<slug> <stage>` (ADR-0155, ADR-0183); `Slice-Verdict: <project>/<slice> <verdict>`; `Gate: <token> <tree_hash> <deadline>`, later extended with kind, reach and state (ADR-0184). Column-0 grammar, `-c trailer.ifexists=add`, hooks disabled on Redkiln commits (design §5.4 `:73-78`, ADR-0105) | **Authoritative.** The deleted-database test: "if the database were deleted, would `redkiln run` take a different next action?" (ADR-0109) |
| **Tier 1 mirror + tier 2, SQLite** | `<git-common-dir>/redkiln/store.sqlite3`, **rusqlite =0.40.2, `PRAGMA journal_mode = WAL`** asserted (`crates/rk-store/src/conn.rs:192-195`), `busy_timeout` 5000 ms, one writer per process behind a mutex. Tables: `rk_identity(root_commit, common_dir, schema_version)`, `t1_stage_transition`, `t1_gate`, `t1_gate_transition`, `t1_slice_seal`, `t2_event(kind, payload, anchor_sha)`, `t2_memo(key, result_json, live)`, `t2_usage` (`migrations/0001_initial.sql`, `0002_gate_transitions.sql`) | Tier 2 is disposable and rebuildable. Tier-1 rows are written only after their trailer commit (AC-162, ADR-0181) |
| **Event envelope** | `canonical_events = {kind, payload}`. A **tag set** (reserved keys `item`, `sha`, `run`) and a store-minted instant sit outside the canonical digest (ADR-0131, ADR-0182) | — |
| **Product config** | `.redkiln/` in the repository: `config.toml` (adapters, enabled packs, `[project] name`, which is mandatory for trailers per ADR-0159), `facets.toml`, `artifacts.toml`, `registry.toml` (verb registry), `roles.toml`, `declared.toml` (the `surfaces` map), `packs/*.toml`, `catalog/*.toml` (102), `cards/*.json` (8 roles), `checklists/`, `processes/questions.toml`, `kb/*.md` (4 notes), `items/*.toml` (2) [measured] | Hand-authored and reviewed |
| **Typed artifacts** | Layout C: typed frontmatter, read-only digest-stamped sections and opaque prose. Machine-only kinds are JSON sidecars rendered on demand. Nothing parses markdown (ADR-0005/0006/0075/0189) | Store-rendered |
| **Ids** | item `<project>/<slug>` (file `.redkiln/items/<id>.toml`); gate token; `tree_hash` (40-hex); `run_id`; memo key = SHA-256 over a length-prefixed ordered 8-tuple (ADR-0097); selection key (ADR-0119) | — |
| **Corpus** | `CORPUS_PIN`: `CORPUS_REPO=D:/repos/redkiln-corpus`, `CORPUS_SHA=61f590e…` — **local only, no remote** (#32) | Test oracle (ADR-0013) |

### 3.4 Dependencies [measured, `Cargo.toml [workspace.dependencies]`]
gix =0.87.1 (reads only), jsonschema =0.56.0 (no default features), **rusqlite =0.40.2 bundled**, yaml_serde =0.10.7 (migrator only), windows-sys =0.61.2, libc =0.2.189, chrono ^0.4, sha2 ^0.11, serde 1, serde_json 1 (preserve_order), schemars =1.2.2, insta, trybuild, proptest, thiserror 2, tokio 1 (adapter-claude only), clap 4, anyhow (binaries), toml 1, ctrlc 3. `cargo-deny` bans durable-execution frameworks (temporalio-sdk, dbos and others; ADR-0086). **happenstance is not a dependency.** It is the intended tier-2 adapter (ADR-0131, accepted 2026-09-21 by D21-6) but is gated on the unrun spike **S-HS** (`references/2026-09-20-operator-surface-proposal.md` §12.6) [read].

### 3.5 Platform constraints
- Built and measured on **Windows 11** (`D:/repos/...` paths appear 241 times in `runbook/phase-1`; Git Bash is the stated shell, `runbook/INDEX.md`). CI also runs on Ubuntu. The 8.3 temp path, CRLF and Job Object handling are all first-class (ADR-0164 makes everything LF; ADR-0151 pins one fixture `-text`) [measured].
- **Not Cowork-compatible as written.** The store asserts WAL and refuses anything else (`conn.rs:195`), but Cowork's connected folders need TRUNCATE (the suite brief; weigh-in B-01). A headless loop is unlikely to run in Cowork anyway [inferred].
- Claude Code version floor ≥ 2.1.251, measured instrument 2.1.268/2.1.269 (`spikes/FINDINGS.md` §Continued; PROGRESS Step 0) [read].

## 4. Contracts and integration surface

| Surface | Direction | Notes |
|---|---|---|
| `<git-common-dir>/redkiln/` | writes `store.sqlite3` | **Same directory as the suite's `events.sqlite3`.** It is a second, different SQLite file with a different journal mode [measured] |
| Git trailers `Story:`, `Slice-Verdict:`, `Gate:` | writes and reads | Unknown keys are ignored and counted. `Fix:`, `Slice-Repair:` and `Baseline-Repair:` are legacy keys the migrator reads (design §5.4 `:76`). No other repo writes these keys [inferred] |
| Branch `rk-run/<item>`, `.redkiln/worktrees/<item>` | writes | ADR-0180/0188 |
| `rk:*` label namespace | reserved | "Never create `rk:*` labels, because that namespace belongs to the product's tracker adapter (AC-507)" (`AGENTS.md:56`). Facets map to labels such as `rk:kind/*` and `rk:cos/*` (`spikes/p9/facets.toml:6,10`) [read] |
| `claude -p` stream-json | reads | `rate_limit_event`, `usage.cache_creation.ephemeral_{1h,5m}`, `total_cost_usd`. Headroom in integer ticks (ADR-0110), TTL verification (ADR-0113) |
| Exit codes | emits | `crates/rk-cli/src/exit.rs`, including `Exit::FindingsBlock` (ADR-0162) |
| Weigh-In | uses as a dev tool | The orchestrator used Weigh-In for operator approvals during the build (`runbook/phase-1/s8e-tier-one-and-the-lane.md:610`, `wi-ef7b00`). The operator-surface proposal calls its gate view "the weigh-in inbox" (`2026-09-20-operator-surface-proposal.md:320`). There is **no code coupling** [measured] |
| temper | dev tooling | vendored `.claude/temper/` with a baseline. `rust.yml` runs the ratchet |
| happenstance | planned | `rk-adapter-happenstance` behind `EventStore` at tier 2. DCB declined. Git stays tier 1 (ADR-0131 decisions 5–8) |
| Old Redkiln (TypeScript) | reads | `.bklg`/`.kb` corpus through `redkiln-migrate` only (AC-442) |
| anneal, assay | none | no references in code or docs [measured, grep] |

## 5. Backlog and plans

**Ledger.** GitHub Issues became the backlog for bugs, findings and post-M5 work on 2026-10-09 (PR #78, `docs/BACKLOG.md`). It uses native issue types (Initiative/Epic/Feature/Task/Bug), sub-issues, blocked-by links, and label families `area/* source/* status/* needs/* prio/* size/*`. "No bot syncs files and issues in either direction." The runbook still owns s8i, s8j and Step 8 until M5 [read]. **This is a working prototype of the suite's Backlog decision.**

**Open issues: 71, all open, none closed** [measured]. By theme:
- **Before M5 (`pre-m5`, mostly p1):** run-path defects before the first paid lane run (#41, #5, #6, #7, #11), containment zombie handling (#20), M5 definition and re-freeze (#25, #26, #30, #61, #62, #73, #74), CI and record hygiene (#23, #28, #33), and the corpus off-machine copy (#32).
- **Kernel wiring gaps:** budgets and degradation (#8, #43–#47), deviation (#14, #56), review gate (#13, #51–#53), catalog strictness (#10, #49, #50), capability routing (#55), and the lattice differential test (#54).
- **Store and git:** orphan/rewrite (#15, #57–#60), trailer paragraph (#18), identity bypass (#17), `GitError` Display (#16), and **R-64** "can `doctor --rebuild` re-derive every event-store tag from git alone?" (#76).
- **Records and traceability:** AC-to-test reconciliation (#27, #63–#66), stale-label sweeps (#29, #31, #67–#69), and god-module splits (#70, #71).
- **Phase placeholders:** #36 M5, #37 post-M5, #38 Phase 2, #39 Phase 3, #40 Phase 4.
- **Most recent:** #77 "Make the temper gate green on the merged Rust" (2026-10-08) and #75 R-37 (the spec supplier seam).

**In flight.** Step 8i (S-BIN) was IN PROGRESS at the last commit (PROGRESS §1). Nothing has been committed since 2026-10-09 [measured].

**Unrun spikes that gate design** (design §10.5 `:1908`; #42): S-HS (happenstance), S-CODEX, S-FACET, S-H1c (card route), S-SPF, S-TTL1, S-P7B, P2-B, P8 at long durations, R-SQLITE (two-process contention), and S-SUBTLE-SYM [read].

## 6. What the spikes measured (`spikes/FINDINGS.md`, `F29..F42-DRAFT.md`)

| F | Result | Suite relevance |
|---|---|---|
| F1/F14/F6 | Plan headroom is readable from an undocumented `rate_limit_event`. `utilization` is quantised to 0.01, so per-run cost is not measurable from it | **Capacity library in Core** |
| F2 | Settings hooks do fire in `-p` | Bench and Review hooks work headless |
| F7/F8 | `--agents` does nothing without `--agent`. A default `-p` inherits 12 MCP servers and about 280 tools (56% of the cache write) | **Every headless dispatcher in the suite** (assay, Loop) needs Loop's env denylist and isolation |
| F10/F13/F16/F18/F28 | Prompt cache TTL is about 1 h. Commits cost 31% of the prefix; `--system-prompt` costs 0. Half of human gates outlive the TTL. A gate costs a bounded 18.6k tokens | Harness knowledge for Core and Loop |
| F11 | The verification gate kills 9 of 10 test-reachable mutants | Review and verify |
| F12 | 20.3% of slices passed through a rejected state, so two trailer axes are needed | Loop's tier-1 design |
| F19/F22 | Suspension is lossless (174/174); drift 7%, 3.6× likelier on gates over 1 h (marginal); background jobs die | **Reconcile at every entry** is a suite-wide rule for anything that resumes |
| F21/F23/F24/F26/F27/F29/F31 | Adversarial review: 23/23 detection, MCC +0.917; headless matches in-session detection at **1.72× lower cost** ($56.47 vs $97.39); verdicts are a weighted coin at n=10; the typed findings contract costs no detection | **Redkiln Review (assay)**: the strongest external evidence for its design |
| F25 | Red→Green is evidenced per test, but an AC is one assertion, so per-AC mutation evidence is needed | Review and Bench verify |
| F32–F35 | The activity-graph TOML authors at a median 53–73 lines (kill >150). The lattice is 0.3% unsatisfiable after fixes, with 0 rank ties and 0 cycles | The process-catalog format is viable |
| F36/F39 | The **LLM-emitted context vector is unstable** (modal R identical on 5/20..2/20 items vs an 80% line). Derived facets fail the 0.95 bar (`auth` 0/12) | **Calibration in Core**: Loop's facets are now human-sourced (ADR-0090) |
| F37 | The adversary→policy block costs 2.64× and changes the root cause on 1 item in 10 | Review cost model |
| F38 | Resume across a real kill is lossless 3/3. Cost ordering is unmeasured | — |
| F40 | The corpus contains no staleness, so P2's recall has no denominator | — |
| F41/F42 | The port shape compiles on the pin; 285 of 287 predicates parse under ADR-0103 | Catalog grammar is settled |

## 7. The process catalog and its format

- **Two axes** (design §4.2, ADR-0014/0022/0029). A **process pack** owns stages, rigor policy per class, roles, slots and `require` rows. An **agent card** owns identity and return format and fulfils roles. A **technology pack** binds and contributes at slots and never restructures.
- **Pack format** (`.redkiln/packs/bug-standard.toml`; `spikes/p9/packs/*.toml`): `[pack] id, applies = "<predicate>", appetite`; `[policy] stages = {triage="mandatory", discovery="forbidden", …}` over **12 stages** (triage → closeout `finally`); `[stage.X] goal, baseline = [ArtifactKind…]`; `[[stage.X.require]] when = "<predicate>", artifacts, rigor`.
- **Activity format** (`.redkiln/catalog/bisect.toml`): `[activity] id, version, stage, provides, requires, derived_from, tags, applicable_if, method_rank, family, min_rigor/max_rigor, cost_class, evidence, budget, deviation_ok, keywords, rationale` plus `[[step]]` of **five closed kinds**, `agent | tool | check | gate | emit`, with `inputs` refs (`artifact:`, `${step.outputs}`, `toolchain:`), `retry`, `repeat`, `on_false`, `when`, `escalate`. Exits are `Materialized | Blocked | RequestDeviation`.
- **Rigor** is an ordinal deontic enum, `forbidden < discouraged < optional < recommended < mandatory` (ADR-0020/0023). The ladder must be monotone (ADR-0081).
- **Registries:** `facets.toml` (closed `source` set `human | llm | derived | kernel | declared`, ADR-0093), `artifacts.toml` (kinds, `keyed_by` toolchain), `questions.toml`, the verb `registry.toml`, and `surfaces.toml`/`declared.toml`.
- **Selection** is a pure function of four digests in seven steps. A rank tie is a load error and unsatisfiable means a gate (design §4.4). Grammar: ADR-0031–0073 and 0103, F42.
- **Size:** 102 activities over 12 packs (two shipped), 8 cards. The Python prototypes (`spikes/p9/lint.py`, `lattice.py`, about 2.2k lines) are superseded by `rk-kernel` [measured].

## 8. Shared-foundation candidates (for Redkiln Core)

| Candidate | Where | Size | Duplicated by a sibling? | Verdict |
|---|---|---|---|---|
| Store location and identity: `<common-dir>/redkiln/`, refuse a store from another repository (root commit + common dir), refuse an older schema with no implicit migration, refuse network filesystems | `rk-store/src/conn.rs`, `identity.rs`, `migrate.rs`; `rk-adapter-sqlite/src/path.rs` | ~1–2k | weigh-in (`<git-common-dir>/weighin/`), anneal ledger | **Core.** These rules belong in Core's opener. Loop's rules are the most rigorous version of them [inferred] |
| Connection discipline: one writer, bounded write transaction, busy_timeout | `rk-store/src/conn.rs` | ~250 | weigh-in, anneal (TRUNCATE open) | **Core**, but the journal mode **conflicts** (see §10) |
| Event envelope `{kind, payload}` + tags outside the digest + store-minted instant | ADR-0131, ADR-0182; `rk-types/src/event.rs`, `event_kind.rs` | ~1k | happenstance natively; weigh-in and anneal envelopes | **Core.** It already matches happenstance's tag model |
| Canonical serialiser and digests | `rk-store/src/canon.rs`, `rk-types/src/digest.rs` | <1k | weigh-in uses sha2 | Core |
| **Process containment** (Job Object / process group, bounded run) | `crates/rk-containment` | 2,708 | assay spawns headless `claude -p`; anneal shells out | **Core.** It is generic and already a separate crate with no Redkiln dependencies [measured: deps std, windows-sys, libc] |
| **Claude harness knowledge**: env denylist, `--agent` isolation assertion, cache profiles, TTL verification, stream parsing, cost rows | `rk-adapter-claude/src/{env_denylist,ttl,headroom,throttle,event,runner}.rs` | ~19k incl. tests | assay's headless review runner | **Split.** Move the stream parser, usage and cost types and the env denylist to Core as a "headless dispatch" library. The harness port stays in Loop |
| **Capacity**: headroom in integer ticks on visible windows, two-tick margin, throttle | `headroom.rs`, `throttle.rs`, ADR-0110 | <1k | assay A-03 semaphore | **Core capacity library** (already decided). ADR-0110 is its best-specified seed. #21 notes the throttle is process-local; a cross-plugin lease must fix that |
| **Calibration**: facet provenance, scored-stability gate (AC-071), abstention as `null`, human labels | `rk-kernel/src/facet_{provenance,stability,registry}.rs`, ADR-0079/0089/0090/0093, the P1 panels | ~2–3k | anneal Clef/Brier, weigh-in eval seed, assay QA-12 | **Core calibration library** (decided). Loop provides the pre-registration and bar discipline |
| Git exec wrapper and trailer grammar | `rk-git/src/git_exec.rs`, `trailer/` | ~3k | weigh-in "git through the command line" | **Core candidate** if a second plugin writes trailers. Today only Loop does [inferred] |
| Records conventions (MADR-lite with "What this forbids", ERRATA, precedence) | `references/adr/README.md`, ADR-0085/0092 | doc | Weigh-In atoms, anneal decision log | Knowledge's decision-atom spec |
| CI structural assertions (dependency confinement, must-fail fixtures, no-egress job) | `.github/workflows/ci.yml` (600+ lines), `tests/fixtures/` | — | temper gate | Pattern for Core's conformance kit |
| Launcher / libexec packaging | — | none | weigh-in, anneal | Loop has nothing to share. It should **adopt** the suite's packaging |

## 9. Granularity findings

1. **Loop contains three other domains, and the code now shows where.** `rk-store` owns item records (`backlog.rs`, `item_write.rs`, `.redkiln/items/*.toml`) and a `.kb` path (`.redkiln/kb/`, injected per activity into the memo key, design §8.6 `:1490`). `rk-ports` has `Tracker` and `DocsSite`, both fake-only. Under the suite decisions, items come from **Backlog (GitHub Issues)** with a snapshot event per issue started, atoms come from **Knowledge**, and docs are deferred. This **shrinks** Loop: delete `backlog.rs`/`item_write.rs` and point the `Tracker` port at the Backlog client. Practically, the change is small: items are 2 TOML files today, and both stub ports are uncalled [measured].
2. **Natural crate seams already exist.** `rk-containment` (standalone, Core), `rk-types`' event/digest half (Core), and `rk-store`'s connection and identity half (Core) separate from its artifact render path (Loop or Knowledge). `rk-adapter-claude` splits into a generic dispatch library (Core) and Loop's harness adapter. `rk-kernel` (102k lines) is one crate by decision (`select`, `catalog`, `packs`, `policy` and `verify` are modules, DECISIONS-2026-09-11 #15). It is the obvious internal split if the **catalog** is shared [inferred].
3. **The process catalog is a suite asset, not a Loop internal.** `suite-structure.md:173` already says the activity "is a catalog entry that Flow and Loop run". The TOML format, the predicate grammar (ADR-0103) and the rigor enum are a specification with a measured authoring cost (F32–F35). If Bench runs catalog activities in-session, the format goes in Core's written spec and `rk-kernel::{catalog,predicate,select,lattice}` becomes a library both depend on. If not, it stays in Loop. **This is the biggest granularity call this repository raises** [inferred].
4. **Review overlap with assay.** Loop's review gate ("reviewer proposes, machine confirms", ADR-0074; typed repro, ADR-0096; per-AC mutants, ADR-0104; advisory sink, ADR-0082; severity `blocking | advisory`, ADR-0168; quarantine) is a second review engine, and spikes E3, F21–F31 are its evidence. Under the suite, Loop's review stage should **call Redkiln Review's** finding contract rather than keep `rk-kernel/src/review/` (1.2k lines) as a fork [inferred].
5. **Human gates overlap Weigh-In.** A Loop gate (`(token, tree_hash, deadline)`, options from `questions.toml`, typed attachments) is a decision request with a deadline and a default terminal path. A Weigh-In card is the same shape. Phase 3's "answer from a phone" (design §10.2) is what Weigh-In's board already does. **Recommended:** Loop emits gates as Weigh-In questions through the shared store, and `redkiln answer` stays as a fallback. ADR-0009 (CLI-only) allows this, because it only restricts Phase 1 [inferred].
6. **The verify gate overlaps Bench and temper.** Affected-area selection plus per-AC mutation evidence (design §6.5) and the Rust toolchain adapter (cargo/nextest parsing, 10k lines) overlap temper's gate and anneal's verify checks. The toolchain adapters could become a Core "toolchain" library that Bench's checks also call [inferred].

## 10. Evidence that challenges the suite decisions

1. **The shared store's journal mode.** Loop asserts **WAL** and refuses anything else (`rk-store/src/conn.rs:192-195`; ADR-0105 clause 7; ADR-0084 "one SQLite writer"). That is a deliberate concurrency contract for two worktrees writing at once (AC-181). Core's decided opening is **TRUNCATE** for Cowork. Loop is the one plugin that never runs in Cowork and runs several processes at once (parallel worktrees in Phase 2). With one shared file, Loop must give up WAL, or Core must support per-host modes on one file, which is not safe to mix. **R-SQLITE**, the two-process contention test, is still unrun (design §3.3). This is the most concrete technical challenge to "one shared file" [read + inferred].
2. **Two stores in one directory.** Loop's store is `<common-dir>/redkiln/store.sqlite3` and has **tier-1 mirror tables** (`t1_*`) with a git-first write rule and a `doctor --rebuild` that drops and rebuilds them. A shared `events.sqlite3` with additive-only schema and append-only events does not fit "rebuild tier 1 from git" well. Either Loop keeps a private projection file beside the shared event file, or its tier-1 tables become happenstance projections. ADR-0131 already chose "happenstance at tier 2, git stays tier 1" and **declined DCB**. That fits the suite. But R-64 (#76), whether tags can be rebuilt from git, is open, and it decides whether the shared file can be a rebuildable cache for Loop [read].
3. **The single-writer assumption.** ADR-0105 and the design say "one writer connection per process behind a mutex". A shared file adds writers from Bench, Weigh-In and Review hooks during a Loop run. Ryan judged contention irrelevant at hundreds of events per session, but a Loop run's tier-2 volume (dispatch records with stdout heads and tails, `rate_limit` readings, `artifact.read` on every input) is larger. It is unmeasured [inferred].
4. **GitHub Issues as backlog supports the decision, with one caveat.** redkiln-rs already moved its own work tracking to GitHub Issues (PR #78) and reached the same conventions. The caveat is the design's reason for an in-repo item store: replay and memo keys need item content at a hash (`suite-structure.md:191`), and inbound tracker text is "a typed input, never a transition" (design §3.2). The suite's answer, a snapshot event per issue started, must also cover **mid-run edits**. The design measured 7% drift on gates (F19) and requires reconcile at every entry, so Loop must re-snapshot and record drift at each entry, not only at start [read + inferred].
5. **Knowledge as a separate plugin is supported.** `.redkiln/kb/` is 4 markdown notes and the store has no `.kb` code path beyond the migrator [measured]. The one hard requirement Loop puts on Knowledge is that **injected atoms are part of the activity memo key** (design §8.6). Knowledge must expose an atom-set digest per activity [read].
6. **`rk-plugin-gen` conflicts directly and is unbuilt.** DECISIONS-2026-09-11 #8 fixes an `rk-` prefix on every generated command, agent and skill, and AC-506 lints for it. The suite's IDs and the "Redkiln Loop" plugin make that obsolete. It is Phase 2, with no code [measured: no crate]. Supersede it with a hand-written Loop plugin (`/loop:run`, `/loop:answer`, `/loop:status`) that wraps the binary through the suite launcher [inferred].
7. **The `rk:*` label namespace.** `AGENTS.md:56` reserves `rk:*` labels for Loop's tracker adapter (AC-507: an orchestrator-only label edited by a human yields a `Drift`, not a transition). Under GitHub-as-backlog, the **Backlog plugin** owns the label taxonomy. Loop's orchestrator-only labels must be a namespace that Backlog's schema grants, or AC-507 moves to Backlog [read].
8. **Loop is not the least built.** `suite-structure.md:135` should be corrected: Loop is the largest codebase in the suite (about 101k non-test lines) and the most tested. That changes the cost of "Loop conforms to Core". Moving its store to Core touches `rk-store`, `rk-adapter-sqlite`, ADR-0105/0109/0131 and the AC-184 rebuild differential [measured].

## 11. ADR triage (198 records)

Grouped. "Core spec" means Core's written format spec or conformance kit. Statuses are from `references/adr/README.md` on the build branch. All are `accepted` except 0079 (superseded by 0090), 0041 and 0088 (superseded), 0004 (superseded by 0074) and 0197 (`proposed`). 0090 and 0091 are the runbook's gate records.

| ADRs | Subject | Disposition | Note |
|---|---|---|---|
| 0001, 0002, 0011, 0012, 0013, 0094, 0167 | The inversion bet, clean break, Phase-1 lanes, gating spikes, corpus repo, Rust adapter, Codex in Phase 4 | **Keep for Loop** | Programme scope |
| 0003 | Five ports with fakes; git is not a port; no Postgres | **Keep (Harness, Toolchain)** / **superseded in part** | The `Tracker` row becomes the Backlog client; `DocsSite` is deferred with Docs; `EventStore`'s real adapter becomes Core's store |
| 0005, 0006, 0075, 0189 | Layout C, prose-free human edits, render never parse | **Core spec** (artifact rendering) | Knowledge atoms and Review findings render the same way; one spec |
| 0007, 0010, 0016, 0078, 0113 | Codex validation-only; subscription-only auth; static system prompt; cache/TTL rule and verification | **Core spec** (headless dispatch) | Applies to assay's headless runner too |
| 0008, 0073, 0132, 0165 | Unreal S-BIN fixture; toolchain-keyed kinds; `ChangeSummary` enum | **Keep for Loop** | 0165's `ChangeSummary` could later feed Review |
| 0009, 0129 | CLI-only gate channel; answer grammar | **Keep, superseded in part** | Phase-3 channels become Weigh-In; the CLI stays as fallback |
| 0014, 0020, 0022–0030, 0047, 0056–0061, 0072, 0081, 0114, 0166 | Packs vs technology packs, rigor enum, encapsulated activities, facets, registries, catalog curation, pack tag | **Keep for Loop → Core spec if Bench runs catalog activities** | See §9.3 |
| 0031–0046, 0048–0055, 0062–0071, 0103, 0121, 0172 | Activity-graph schema and predicate grammar | **Same as above** | A format spec; F42 is its test |
| 0015, 0019, 0026, 0074, 0080, 0082, 0096, 0101, 0104, 0111, 0124, 0162, 0168, 0177, 0196 | Review tiers, per-AC discriminating tests, three-level review, reviewer proposes / machine confirms, advisory sink, typed repro, negative controls, per-AC mutants, severity, review verbs | **Move to Redkiln Review** (finding contract); Loop keeps the call site | Overlaps assay; E3 and F21–F31 are Review's evidence |
| 0017, 0062, 0063, 0087, 0106, 0123, 0145, 0146, 0184, 0198 | Gate record `(token, tree_hash, deadline)`, attachments, mid-graph gates, gate expiry | **Core spec** (decision-request record) **+ Weigh-In** | The record shape is the suite's "question with deadline and default" |
| 0018, 0105, 0109, 0155, 0156, 0158–0160, 0181, 0183, 0186, 0193 | Rewrite events, the git/store contract, tier test, trailer axes, `[project] name`, tier-1 writer, write set | **Keep for Loop**; the **store half of 0105** (clause 7: WAL, one writer, busy_timeout) is **superseded by Core** | Trailer grammar → Core spec only if another plugin writes trailers |
| 0021, 0043, 0097, 0119 | Memo keys | **Keep for Loop** | 0097's hashing scheme (length-prefixed SHA-256 tuple) is a good Core digest primitive |
| 0076, 0099, 0115, 0118, 0127 | Sync ports, cancel token, Arc dyn, interior mutability, containment crate | **Keep for Loop; 0127 → Core** | `rk-containment` is generic |
| 0077, 0091, 0098 | gix read-only, write path shells out | **Keep for Loop** (Core spec if the git wrapper moves) | — |
| 0079, 0089, 0090, 0093 | Context-vector source, facet provenance, human-sourced facets, source classes | **Core (calibration library) + Loop** | The pre-registration and bar discipline seeds calibration |
| 0083, 0084, 0100, 0102, 0164 | Toolchain and lint policy, dependency policy, 0.x pin list, LF | **Move to Redkiln Rust / monorepo policy** | Overlaps temper's rust-craft; reconcile in one workspace policy |
| 0085, 0092 | Documentation and traceability conventions; errata and precedence | **Knowledge** (decision-atom spec) | Weigh-In atoms carry the same immutability rule |
| 0086 | No durable-execution framework | **Keep for Loop** | Consistent with happenstance (an event store, not a workflow engine) |
| 0088 | (superseded by 0101) | — | — |
| 0095, 0108, 0112 | Closed output schema, structured output check, `StepError` set | **Core spec** (typed agent output) | Any plugin that dispatches with a schema needs it |
| 0107, 0128, 0130, 0137–0144, 0147–0154, 0157, 0161, 0163, 0169–0171, 0173–0180, 0185, 0187, 0188, 0190–0192, 0194, 0195 | CLI grammar, run record, step executor, lane FSM, toolchain verbs, resolver, step-grain commits, worktrees, verify, release | **Keep for Loop** | Engine internals |
| 0110, 0122, 0126, 0133 | Headroom ticks, retry charging, `dispatch_completed`, `rate_limit_reading` | **Core (capacity library)** + Loop | Seeds the cross-plugin lease |
| 0116, 0120, 0125, 0131, 0182, 0185 | Event-kind additions; tag selector; item key outside the digest | **Core spec** (envelope and tags) + Loop's kind table | 0131 already aligns with happenstance tags |
| 0117, 0025 | `stale_policy`, vouched roots | **Keep for Loop** | — |
| 0134 | Conformance: expected-uncalled ports | **Superseded by suite decisions** in part | `tracker` leaves the port set |
| 0197 (proposed) | Least-privilege cards, pinned model ids, run environment, launcher | **Keep for Loop; launcher → Core packaging** | Still proposed (#73) |
| DECISIONS-2026-09-11 #8 (not an ADR), design §8.8 | `rk-` prefix, `rk-plugin-gen` | **Superseded by suite decisions** | Replace with a hand-written Loop plugin under the suite ID |
| Design §5 `.bklg`/`.kb` in `rk-store`; 0186 (slice on the item record); `rk-store/src/backlog.rs` | In-repo items and atoms | **Superseded by suite decisions** (GitHub Issues; Knowledge) | Item record → a Core item schema mapped to issues; the slice field becomes a sub-issue or label |

**Load-bearing for the suite as a whole:** ADR-0109 (the deleted-database test: any plugin keeping state in the shared store should say which tier it is), ADR-0017 (gate binds to the tree hash; void, not reconcile), ADR-0131 (tags outside the canonical digest), ADR-0095 (no `oneOf`; closed output schemas are a vendor-contract rule for every structured dispatch), ADR-0010 (subscription auth, no `--bare`), ADR-0078/0113 (cache TTL verification), ADR-0110 (headroom), ADR-0079/0089 (no model-derived fact drives control flow until scored), ADR-0005/0189 (render, never parse), and ADR-0092 (record precedence) [inferred].

## 12. Migration notes for a monorepo move

- **Land the branch first.** `main` (63 commits) is a design snapshot. `build/phase-0-to-1` is about 490 commits ahead and carries all the code. #24 plans a `--no-ff` merge at M5. Moving `main` would lose the product. The other remote branches are `backlog/ledger-2026-10-08` (merged as #78) and `feature/bold-shannon-4no6ec` (#35, closed) [measured].
- **History size:** `.git` is 18 MB on a full clone (7.7 MB on the `main`-only checkout). It is small enough to import with history (`git subtree`/`filter-repo` into `loop/`) [measured].
- **Hard-coded paths will break.** `ci.yml` names `crates/` 40 times in its structural greps. Checkpoint scripts grep literal strings, for example the `Cargo.toml` comment kept "because Checkpoint 5 greps the literal string". `runbook/` has 241 `D:/repos/...` paths, and `CORPUS_PIN` points at `D:/repos/redkiln-corpus`. The workspace root `Cargo.toml` has `members = ["crates/*", "tests/fixtures/*"]` and excludes `spikes/*`. All of this assumes the repository root is the workspace root [measured].
- **The corpus has no remote** (#32). It holds the S-BIN evidence and AC-184's rebuild fixture. Back it up before any move.
- **`.redkiln/` is product config at the repository root** and the store is `<common-dir>/redkiln/store.sqlite3`. In a monorepo that self-hosts Loop, `.redkiln/` would sit beside the other plugins' config. The `redkiln/` directory name collides harmlessly with the shared `events.sqlite3` [inferred].
- **Line endings:** `.gitattributes` forces LF everywhere with `-text` exceptions for pinned fixture bytes (ADR-0151, ADR-0164). Merge these rules in, or golden digests move.
- **CI:** `ci.yml` (gates on Ubuntu and Windows, no-egress, structural) and `rust.yml` (temper ratchet). There are **no secrets, no deploy keys, no release workflow and no marketplace coupling** [measured: no `secrets.` in workflows]. `.githooks/pre-commit` and `.cargo/config.toml` need checking for path assumptions.
- **Vendored temper** in `.claude/temper/` with its own baseline (330 lines). In the monorepo it should come from Redkiln Rust directly.
- **Dependency unification:** rusqlite `=0.40.2` exact, gix `=0.87.1`, jsonschema `=0.56.0`. happenstance-sqlite asks for `"0.40"`, so it unifies (operator-surface §12.4). Merging with weigh-in/anneal's lockfiles needs one resolution of tokio (rt only vs rt-multi-thread) and rusqlite.
- **Record volume:** 198 ADRs, AC/SM registers and 24 runbook stage files with heavy cross-references (`§N.M`, `AC-NNN`, `RUNBOOK.md:NNNN` at pin `009a4ec`). Validators in `runbook/tools/*.py` and `references/audit/2026-09-10/scripts/` assume repository-root paths. Keep the records under `loop/references/` as they are; do not renumber (the ADR README forbids it) [read].
