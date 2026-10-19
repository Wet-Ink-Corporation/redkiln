# Audit: weigh-in (+ weigh-in-plugin)

*2026-10-13. Audited at `main` = `Add the v1-checks session's last turns to the eval seed (#128)` (2026-10-12), version 0.6.1 plus `## Unreleased`. Local clone is shallow (50 commits); counts from the GitHub API are marked. Tags: [measured] = counted or run here, [read] = stated in a file or issue, [inferred] = my judgement.*

## 1. Identity

**Job.** A decision ledger for agent sessions: agents mark what they assumed, deferred or could not decide in an end-of-turn `weigh-in` block; command hooks capture those at zero model tokens; `/weigh-in` filters them hard and settles the ones that need the human in one sitting at the end of the chat; every decision lands as a `.kb` atom (`README.md:1-10`) [read].

| Fact | Value |
|---|---|
| Version | 0.6.1 (`Cargo.toml:6`, `plugin/.claude-plugin/plugin.json`) [read]; 10 GitHub releases, latest v0.6.1 2026-10-11 [measured, API] |
| Commits | 97 on `main` (GitHub API, `per_page=1` last page) [measured]; repo created as one seed commit on GitHub's initial commit 2026-10-01, pre-GitHub history kept only in local `archive/pre-github` (B-06) [read] |
| Age / activity | First design 2026-09-26; last commit 2026-10-12; issues updated 2026-10-13 [measured] |
| Rust LOC | 17,557 total [measured]: `weighin-core/src` 6,438, `weighin/src` 6,671, integration tests 4,468 (`crates/*/tests/it/`) |
| Other LOC | 9,578 md (docs + 41 atoms), 694 board.html, 414 sh, 561 yml; `.claude/temper/` vendored temper CLI ≈5,650 mjs [measured] |
| Tests | 274 `#[test]` attrs [measured]; README says 259 at 0.6.0: 105 core unit, 22 core property, 7 doctests, 73 CLI unit, 52 CLI integration (`README.md:158-160`) [read] |
| Lints | temper baseline at 0 findings after #82 remediated 721 (`AGENTS.md` decisions log) [read] |
| Visibility | `internal` repo, GitHub size 7,145 KB (binaries on the `marketplace` branch) [measured] |
| Distribution | (a) `marketplace` branch here, one force-pushed commit per release (D-0009, B-14); (b) `Wet-Ink-Corporation/weigh-in-plugin` default branch, read by claude.ai org sync (B-19); (c) GitHub release zip + raw binaries + SHA256SUMS; (d) cloud sessions on this repo wire it via `.claude/settings.json` (B-12) [read] |
| Maturity | Slice 1 built; v1 Phase 3 (verify and use) under way; 2-week pilot #23 opened 2026-10-12; status audit put v1 at ~63% (`docs/roadmap.md:13`) [read] |

## 2. Functional shape

### Plugin `weigh` (install id `weigh@weigh-in`, command `/weigh-in`, D-0004)

| Surface | What | Status |
|---|---|---|
| Skill `weigh-in` (`plugin/skills/weigh-in/SKILL.md`, 80 lines, `disable-model-invocation: true`) | The sitting: sweep (no tool call) → five filters (settled, recoverable, authority/door, decide-now, evasion) → `W batch` → one `AskUserQuestion` with ≤4 cards → `W batch` → close. Args: `init own|client`, `repo`, `digest`, `board`, `sweep <path>`, `stats`, `decide <id>: <opt>`, `defer <id> — …` (one skill with arguments, D-0005) | Built |
| Skill Cowork route (SKILL.md "Cowork") | Runs the binary through the device shell (`mcp__remote-devices__device_bash`) against `$HOME/mnt/<folder>`; download-from-release with pinned SHA-256 is dormant while internal (D-0003, D-0012) | Built; download dormant |
| Skill board route | Publishes `assets/board.html` (694 lines) as an Artifact with `db` capability; `board export`/`board apply` via ArtifactData | Built for Claude Code; not from Cowork (B-17) |
| `references/rubric.md` | Card/door rubric | Built |
| Hook `SessionStart` | Prints the marker convention + open counts (`hooks.rs:288-306`), or a `systemMessage` when capture is off / hook errors logged (B-11) | Built |
| Hook `Stop`, `SubagentStop` | Parse `weigh-in` blocks from `last_assistant_message`, raise them; in Cowork only nudge on one-way (`hooks.rs:310-326`) | Built |
| Hook `PostToolUse` (`Write|Edit|MultiEdit`) | Scans only the written text for "Open questions" bullets and new TODOs | Built |
| All hooks | Exec form: `${CLAUDE_PLUGIN_ROOT}/libexec/weighin` with `args` (`plugin/hooks/hooks.json`), 10 s timeout, never fail a session (B-16) | Built |

### CLI `weighin` subcommands (`crates/weighin/src/main.rs:57-109`)
`hook` · `prime` · `init [--class own|client] [--board artifact|local|off] [--board-url] [--owner] [--ledger]` · `open [--repo|--all] [--with-context] [--top] [--json]` · `show` · `context` (≤600-token evidence pack) · `raise --q … | --markers` · `batch` (ops: raise/triage/ground/present/decide/defer/default/dismiss/reopen/supersede/link/ack, with `@N` slots) · `decide` · `defer` · `dismiss` · `reopen` · `digest` · `stats` · `trailer` · `scan <path>…` · `board export|apply` · `journal`. Global `--store`, `--dir`, `--session` [read].

Guards enforced by the CLI (`README.md:129-141`): 2–4 options; recommendation cannot fail a MUST or be evasive; one-way/unsure never defaulted; deferrals need flip/trigger/by/default; authority-word items dismissed as "already decided" only against a `kb-…` atom or the user's words; board export only in `artifact` mode (B-25) [read].

### Repo-only tooling
- `.claude/hooks/session-start.sh` (cloud only): builds `weighin`, copies the skill into `.claude/skills/weigh-in/` with `${CLAUDE_PLUGIN_ROOT}` rewritten, runs SessionStart (B-12).
- `.claude/hooks/weigh-in-journal.sh` (52 lines): after capture hooks and every Bash call in cloud sessions, appends `weighin journal` output to `.kb/_ledger/journal/<session>.jsonl` under `flock` (B-21).
- `scripts/weekly-stats.sh` (116 lines): `weighin stats` across folders as markdown for #23.
- `eval/` seed + `crates/weighin/tests/it/eval_seed.rs` (374 lines): capture precision/recall scorer.

### Planned only (slice 2 / 3, `#27`, `#112`–`#116`)
Sonnet grounder + 3-sample permuted stability check; `serve --wait` local page (likely replaced by mod pane, D-0017); Claude Code mod (ledger band, local board pane, `/wi` commands, just-in-time decision surfacing on edits); board from Cowork; reliance drills (wi-2b90f3); door classifier for unattended runs (wi-4d8b6a); owner default from git config; going public; Redkiln-native `decision` item type + kb-ingest of intake keys + AFK `blocked_by` gating + SDK pause loop (slice 3); happenstance past 0.3.2 [read]. Also requested by a sibling but unbuilt: `weighin ci-check --json` (assay `docs/architecture/03-contracts.md:88`, "D-WI") [read].

## 3. Architectural shape

### Crates and layering
| Crate | Layer | Size | Deps |
|---|---|---|---|
| `weighin-core` | domain, **no I/O, no clock, no async** (`crates/weighin-core/AGENTS.md`) | 6,438 LOC: `atom.rs` 1,107, `board.rs` 1,320, `projection.rs` 1,288, `marker.rs` 759, `event.rs` 729, `scan.rs` 621, `card.rs` 337, `id.rs` 228 | serde, serde_json, sha2 0.10, thiserror 2 |
| `weighin` | binary (composition root, single writer) | 6,671 LOC: `ops.rs` 1,879, `main.rs` 1,430, `store.rs` 872, `hooks.rs` 745, `env.rs` 700, `context.rs` 561, `git.rs` 157, `annotate.rs` 148, `journal.rs` 122 | weighin-core, happenstance-core 0.3.2 (`serde`), happenstance-sqlite 0.3.2 (→ rusqlite 0.40.2, libsqlite3-sys 0.38.2), tokio 1 (`rt` only), clap 4, anyhow, time 0.3, tempfile 3 |

Both `publish = false` (D-0007). Dependencies point inward (ARC-02). `weighin-core` was split explicitly so it "can be linked into redkiln-rs unchanged" (`crates/weighin-core/src/lib.rs:6`, wi-b4c1d8) [read]. Git via command line only, never a library (`crates/weighin/AGENTS.md`) [read].

**Seam quality [inferred]:** the core/CLI split is clean and real. But `weighin-core` is not one concern: it bundles (a) a generic-ish ledger event/projection model, (b) the `weigh-in` marker grammar + doc/TODO scanner (capture), (c) the `.kb` atom renderer (knowledge format), (d) the Artifact board exchange format (UI contract). Those four are exactly the suite's candidate seams (§7).

### Runtime model
- **In-session hooks** (Rust binary, 0 tokens) for capture; **in-session skill** for the sitting (3 requests budgeted); no headless mode except that the CLI is callable from anything (anneal shells to it). `AskUserQuestion` absent → print cards and stop (SKILL.md step 4) [read].
- **Cowork:** hooks write nothing (`CLAUDE_CODE_ENTRYPOINT=remote_cowork`, B-04); sitting runs binary via device shell (D-0002) [read].
- **CI:** `ci.yml` (fmt/tests/docs on Linux/Windows/macOS), `rust.yml` (vendored temper lint ratchet + gate + cargo deny, Linux), `release.yml` (tag-triggered) [read].

### Data owned
| Data | Path | Format |
|---|---|---|
| Live ledger | `<git-common-dir>/weighin/ledger.sqlite3`; resolution: `--store` → `$WEIGHIN_STORE` → `store` in `.kb/_ledger/config.json` → git common dir → `<cwd>/.weighin/ledger.sqlite3` outside git (`crates/weighin/src/env.rs:331-356`, B-02, wi-6e2a47) | happenstance 0.3.2 SQLite schema, TRUNCATE journal, `PRAGMA user_version = 1` (`store.rs:45,51`) |
| State dir | same dir: `hook-errors.log`, `hook-errors.seen` offset file (overwritten, never deleted) (B-11) | text |
| Config | `.kb/_ledger/config.json` (committed): `repo_class`, `owner`, `board{mode,url}`, `caps{chat_cards,board_focus,context_chars}`, `scan{markdown,todos,exclude}` [read] | JSON |
| Decision atoms | `.kb/_intake/decisions/<wi-id>-<slug48>.md`, id `kb-decision-<wi-id>` (`atom.rs:19-47`) | YAML frontmatter (Redkiln `KbFrontmatter` + Weigh-In passthrough keys) + MADR 4 body |
| Open-question atoms | `.kb/_intake/open_questions/<wi-id>-<slug>.md`, id `kb-open-question-<wi-id>` | same |
| Journals | `.kb/_ledger/journal/<session>.jsonl` + `<session>.hook-errors.log` (committed; 5 files, ~90 KB here) [measured] | one `EventEnvelope` JSON per line, append-only |
| Source annotations | the anchored file/line (markdown only since #95) gets a decided note | text |
| Commit trailer | `Weigh-In: <id> decided <X>` (`main.rs:911`) | git trailer |
| Board state | ArtifactData collection `board`, doc `state`; answers in collection `answers` (SKILL.md "Board") | `weighin-core::board` JSON, capped at 200 defaults (V5: 262,144-byte doc limit) |

**Ids:** `wi-` + 6 lowercase hex of `sha256(session \n normalised question)`, extended to 8 on collision (`crates/weighin-core/src/id.rs:1-15`) [read]. Deterministic, coordinator-free.

**Envelope** (`event.rs:634-654`): flattened `LedgerEvent` + `actor` (`agent|hook|human:<name>|cli`), `session_id`, `worktree`, `branch`, `head`, `ts` (RFC 3339). 13 event types, snake_case (`event.rs:532-546`): `item_raised, item_triaged, item_grounded, item_presented, item_decided, item_deferred, item_defaulted, item_dismissed, item_reopened, item_superseded, item_linked, record_written, item_acknowledged`. Store tags `item:`, `session:`, `worktree:`, plus `channel:`/`kind:`/`door:` on raise — "named to match redkiln-rs's event-kind table … a later import into redkiln-rs's store is a tag remap" (`event.rs:4-6`) [read]. Every write is DCB-conditioned: `Guard::NotYetRaised` / `ItemUnchangedSince(pos)` (`store.rs:63-75,154-186`) [read]. Projection is a fold in store-position order, never timestamps.

**Tree hash** follows "redkiln-rs ADR-0105 clause 4" (temporary index, untracked excluded) so hashes compare across tools (`crates/weighin/src/git.rs:43`, `event.rs:346`) [read].

### The TRUNCATE store opening (B-01, B-10) — `crates/weighin/src/store.rs:99-390`
1. `create_dir_all(parent)`; rename (never delete) a stale `-wal` aside as `-wal.orphaned-<ns>` (`set_aside_wal`, :200) — renames are allowed where deletes are not.
2. Own `rusqlite::Connection` (`connect`, :215): `busy_timeout 5s`, `PRAGMA journal_mode=truncate` retried until busy deadline (converts 0.1.x WAL ledgers), `synchronous=FULL`, `foreign_keys=ON`.
3. `ensure_schema` (:264): if no `store_meta`, build a template with `SqliteEventStore::open` **on the local temp disk**, `ATTACH` it, and inside one `BEGIN IMMEDIATE` copy every table/index DDL and rows (store identity) into `main`, stamping `user_version`.
4. `migrate` (:360): 0 → 1 step-wise; newer `user_version` refused untouched ("upgrade weighin").
5. Hand the connection to `SqliteEventStore::new(conn)`; a current-thread tokio runtime wraps the async API with `block_on`.
Tests cover WAL→TRUNCATE conversion, leftovers from a failed Cowork creation, concurrent creators sharing one identity, failed commits rolled back, 0.1.x and 0.2.2 fixture ledgers (`store.rs:435-870`) [read]. Consequence: **happenstance pinned at 0.3.2**; a schema change is a new migration step (B-10). Dependabot ignores happenstance (B-07) [read].

### Toolchain and platform constraints
- Rust 1.98.1, edition 2024, `rust-version = 1.98.1`, pinned "to redkiln-rs's channel so weighin-core stays adoptable there" (`rust-toolchain.toml`, wi-b4c1d8) [read]. Release profile: thin LTO, 1 CGU, strip.
- Lints: temper's `[workspace.lints]` (deny unwrap/expect/panic/indexing/as_conversions/print/exit…); three approved `#[expect]` exceptions (`AGENTS.md` decisions log 2026-10-05) [read].
- **Cowork:** connected folders refuse deletes (→ TRUNCATE) and WAL shm; hooks run in scratch workspace; `!` injection doesn't run; `PostToolUse` never sees device-shell edits (D-0002, B-01, B-04) [read].
- **claude.ai hosted plugins:** no top-level `plugin/bin/` (B-03) → `plugin/libexec/` + launcher by path.
- **Windows:** exec-form hooks resolve extensionless `libexec/weighin` to `libexec/weighin.exe`; `.cmd` cannot be spawned (EINVAL) (B-16). Skill still needs Git Bash `sh` (D-0015). **New, #127:** desktop app's sandboxed shell cannot see the plugin install folder, so every `W` call fails while hooks work [read].
- **Claude Projects (beta) threads, #126:** not detected as Cowork; no `AskUserQuestion` (has `mcp__hearthbot__ask_decision`); ledger lands in the cloud workspace even with a connected folder; org-synced plugins also load in cloud sessions, possibly double-capturing here (contradicts B-12) [read].
- Launcher (`plugin/libexec/weighin`, 74 lines sh; `weighin.cmd` 28) picks `libexec/<target>/weighin[.exe]`, newest wins among listed targets (B-05), falls back to `PATH`, names a binary-less dev copy (B-18) [read].

## 4. Contracts and integration surface

| Contract | Producer → consumer | Notes |
|---|---|---|
| **`weigh-in` fenced block** (`marker.rs:1-18`; prime text `hooks.rs:328`) | any agent → Weigh-In Stop hook | Keys: `kind: question|assumption|deferral|call`, `door: one-way|two-way|unsure`, `authority: authority|judgment`, `q:` (required), `did:`, `flip:`, `at: path:line`, `blocks:`. Forgiving parse (case, spacing, comments, aliases e.g. `todo`→deferral); no `q:` = not an item. Convention: one block per call (0.5.2, #68); "If you cannot ask the user, take no one-way path: `did: blocked`" |
| **`.kb` atom** (`atom.rs`) | Weigh-In → Redkiln `validate --kb` / kb-ingest (TS Redkiln `src/schema/kb.ts`) | Frontmatter: `id, title, kind, status, authority_tier, summary (Y-statement), depends_on, related, source_paths, last_reviewed, reversibility, phase, supersedes, superseded_by` + passthrough `weighin_item, question, door, blast_radius, decider, provenance, decided_at, tree_hash, recommended, flip_condition`. Values written as JSON strings. kb-ingest does **not** read them yet (B-22, slice 3) |
| **Ledger events / journal JSONL** | Weigh-In → `weighin stats`, any reader | `weighin journal` emits envelopes; journals committed; stats dedupe events held twice (B-21) |
| **CLI JSON + batch ops** | anneal → Weigh-In | anneal's `crates/anneal-cli/src/weighin.rs` locates `$WEIGHIN_BIN` or `weighin` on PATH, pipes `weighin batch` (raise + defer) for `anneal orchestrate park`, reads `weighin open --all --json --top 4096`; anneal records `weighin_parked` events (`anneal-ledger/src/event.rs:596`) [read] |
| **`weighin ci-check --json`** | requested by assay (`assay/docs/architecture/03-contracts.md:86-135`, reason codes `weigh-in-open-blocking`, `weigh-in-no-evidence`, `weigh-in-pending-triage`) | not built; assay adapter runs "experimental" on journals and atoms |
| **Board exchange** (`weighin-core::board`: `BoardState`, `Answer`, `card_rev`, `validate_answer`) | CLI ↔ Artifact `board.html`; future mod pane (#112) | answers treated as untrusted |
| **Commit trailer** `Weigh-In: <id> decided <X>` | agent commits → git readers | mirrors redkiln-rs's trailer-as-authority (ADR-0105) |
| Env vars read | `WEIGHIN_STORE`, `WEIGHIN_SESSION`, `CLAUDE_SESSION_ID`, `CLAUDE_CODE_SESSION_ID`, `CLAUDE_CODE_ENTRYPOINT`, `CLAUDE_CODE_REMOTE` (journal hook), `WEIGHIN_LAUNCHER` (launcher) [measured] | |
| Exit codes | `hook` always 0; other commands exit 1 on error (closed stdout → 0) (`main.rs:68-69`); launcher 127 with no binary | |
| SessionStart output | plain stdout, or `hookSpecificOutput.additionalContext` + `systemMessage` when something to say (B-11) | |
| Vendored copy | `.claude/temper/` = temper CLI 1.3.0 (matches temper `plugin.json` 1.3.0) + `.claude/temper.json`, `.claude/temper-baseline.json`, `.claude/rules/*.md` [measured] | rust.yml depends on it |

Cross-references: weigh-in → redkiln-rs (toolchain pin, ADR-0105 tree hash, event-kind naming, `<git-common-dir>/redkiln/store.sqlite3` precedent in `docs/research/report.md:21`), → TS Redkiln (`KbFrontmatter`, `src/schema/item.ts` for slice 3), → temper (lint ratchet, issue wi-680975 filed about temper's Subagent hooks in cloud). Inbound: anneal (bridge + `weigh_in_ref` in scope ledger, `anneal/plugin/skills/grill/references/scope-ledger.md:26-34`), assay (adapter spec), redkiln-rs `references/2026-09-09-change-classification.md` [measured]. Ryan's `D:\repos\redkiln-rs` itself has a Weigh-In config and ledger in use (#126 comment) [read].

## 5. Backlog and plans

**Milestone `v1.0`; 15 open issues, 23 closed** [measured]. Labels: `verify`, `slice-2`, `release`, `gap`, `decision`.

| Theme | Open issues |
|---|---|
| Verification on real surfaces (Ryan) | #16 V1 Cowork session id (Projects-Cowork partial pass 2026-10-13), #20 V6 Cowork card, #21 V7 Windows w/o Git Bash, #126 Projects threads (text cards, ledger in workspace, surface detection), #127 Windows desktop sandbox can't reach binary |
| Pilot | #23 two weeks of daily use (week 1 opened 2026-10-12, week 2 ≥ 2026-10-19); #25 suppression only if not-useful > 10% |
| Release | #26 1.0.0 (B-23: 0.6.x patches → 0.9.0 RC after Ryan's checks → 1.0.0 with no new code, due ≥ 2026-10-20) |
| After v1 | #27 slice 2+, #112 mod (sub-issues #113 spike, #114 band + `/wi`, #115 local board pane, #116 just-in-time surfacing), #17 V2 deferred |

**In flight now [read]:** nothing agent-side; `main` holds unreleased fixes (wi-56bd73, wi-a01b14, wi-727fbf, wi-32f76d); #126/#127 are fresh (2026-10-13) and are real v1 blockers for the Projects and Windows-desktop surfaces. Recent PRs are journal/eval-seed/session close-outs (#122–#128).

**Plans:** `docs/v1-plan.md` (352 lines; phases 0–4), `docs/roadmap.md`, `docs/research/report.md` (671 lines; the design plus four amendments), `docs/audits/2026-10-06-status-audit.md`.

**Slice 3 — "the move into redkiln-rs" (`docs/research/report.md:504-512`, `docs/v1-plan.md:126`, #27):** `decision` joins Redkiln's `ItemType` with process captured→triaged→grounded→presented→decided→recorded; `redkiln new decision` becomes the single writer; ledger events become Redkiln telemetry events (`item_created`, `stage_enter`, `verdict`, `status_change`); `blocked_by` gates AFK stages on open one-way decisions; `links.kb` points to the atom; kb-ingest learns intake keys (2 h); "Needs you" column; SDK `defer` pause loop. 15 h lower bound. Non-Redkiln repos (incl. IQBG client repos) keep the plugin's `.kb/_ledger` path. Note it references TS Redkiln paths (`src/schema/item.ts`), i.e. predates redkiln-rs as target in places [read/inferred].

**Decision highlights that constrain the suite:**
- **wi-b4c1d8** Rust on redkiln-rs's toolchain, events through happenstance (supersedes Node CLI).
- **wi-6e2a47** one store per repo under git common dir; decisions reach git as atoms + trailer (supersedes per-session JSONL).
- **wi-a93f02** client repos keep ledger data local. **wi-7c21e4** per-repo board mode; client repos default `local` (one-way, authority) — IQBG client items must never reach claude.ai. Enforced in CLI (B-25).
- **wi-e15c77** reserve `.kb/_ledger/` in Redkiln's `.kb` contract; kb-ingest half moved to slice 3 (B-22).
- **wi-4d8b6a** unattended runs may default two-way judgment items; one-way items block.
- **wi-2b90f3** reliance drills (slice 2). **Q3** only Anthropic models judge one-way recommendations.
- **D-0002 / D-0003** Cowork runs the ledger via the device shell; release download pinned by SHA-256 (dormant while internal).
- **D-0004** skill names must stand alone (Cowork lists bare skill names). **D-0005** one skill with arguments, no split.
- **D-0007 / D-0012** internal repo, assets internal for v1; **B-19** amends with a release-only repo.
- **D-0008** v1 = daily use by Ryan only; other users, public plugin, non-`ryan` owner default are not v1.
- **D-0010** dogfooding; **D-0011** open work in GitHub Issues under milestone `v1.0`.
- **D-0013** no slice 2 in v1 (except #25). **D-0015** hooks work on Windows without Git Bash; the sitting still needs it.
- **D-0014 / B-21** cloud ledger dies with the container; event history survives as committed journals.
- **D-0016** sitting cost = ~3 × context at cache-read price + ~5.6k output tokens.
- **D-0017** post-1.0 mod pane decides grounded cards and is the local board for client repos.
- **B-01** TRUNCATE; **B-03** no `plugin/bin`; **B-05** one binary version at a time; **B-10** `user_version` schema stamp.
- **B-12** cloud sessions wired through repo settings (now questioned by #126); **B-14** marketplace branch = one commit, no ruleset.
- **B-16** exec-form hooks; **B-18** only the release repo is org-synced; **B-20** board URL kept out of tracked config.
- **B-22** kb-ingest to slice 3; **B-23** release path; **B-24** V3 moot (one store per repo); **B-25** board export needs explicit `init`.
- **wi-7c3aa0** do not change capture behaviour during the pilot.
- 41 Weigh-In atoms in `.kb/_intake/decisions/` record the post-D-0010 calls (e.g. wi-218cc3 CI split, wi-a9df17 precision bar in force at ≥10 captured, wi-48657b eval seed upkeep) [measured].

**Eval seed / calibration (#22, closed):** `eval/seed.jsonl` 100 lines (real end-of-turn messages, labelled `capture|skip`, `expect.status raised|defaulted`, `origin`, `by: claude-draft|ryan`), `eval/seed-scripted.jsonl` 47 lines (provisional, pinned counts) (`eval/README.md`). Scorer runs each item through the real Stop/PostToolUse path in a scratch repo. Result: precision 14/14 (bar 80%, in force), recall 14/24; recall 1/7 before the 0.5.2 one-block-per-call line, 12/13 after; all labels reviewed by Ryan 2026-10-11 (`docs/roadmap.md` exit table) [read]. Upkeep is a per-session obligation (`CLAUDE.md`, wi-48657b). Pilot metrics come from `Health` (`projection.rs:587-605`: presented, acted, not_useful, chosen_against_recommendation, median_seconds_to_decide, deferred_unqualified, defaults_unacknowledged) [read].

## 6. Shared-foundation candidates (→ Redkiln Core)

| Candidate | Where, size | Duplicated in sibling? | Recommendation [inferred] |
|---|---|---|---|
| **TRUNCATE happenstance opening** (template-on-temp-disk schema copy, `BEGIN IMMEDIATE`, WAL set-aside, journal-mode retry, `user_version` stamp/migrate/refuse) | `crates/weighin/src/store.rs:1-390` (~390 non-test LOC + ~480 test LOC) | **Yes:** `anneal/crates/anneal-ledger/src/store.rs` (442) + `store/schema.rs` — same doc text nearly verbatim, but anneal *refuses* WAL rather than converting, and has typed `OpenError` vs weigh-in's `anyhow` | Core's first module. Take anneal's typed errors + weigh-in's WAL conversion and 0.1/0.2 fixtures. One `user_version` namespace for the shared file. |
| **Store location resolution** | `crates/weighin/src/env.rs:331-366` (flag → env → `.kb/_ledger/config.json` `store` → `<common>/weighin/ledger.sqlite3` → `<cwd>/.weighin/`) | anneal: `<git-common-dir>/anneal/ledger.sqlite3` | Core owns `<common>/redkiln/events.sqlite3`; keep the override chain (Cowork synced-folder case, B-02) and the non-git fallback (Cowork folders). |
| **Event envelope** (`actor`, `session_id`, `worktree`, `branch`, `head`, `ts`; `key:value` tags) | `crates/weighin-core/src/event.rs:634-674` | anneal `anneal-ledger/src/envelope.rs` (385) + `tag.rs` (204) | Unify in Core with a `plugin:` tag; weigh-in adds `item:` tags and DCB guards on them. |
| **Ids** `wi-` sha256 prefix with collision extension | `id.rs` 228 | different schemes elsewhere [not compared in depth] | Generic "content-hash id with prefix" helper in Core; prefix stays per plugin. |
| **DCB guard helpers** (`NotYetRaised`, `ItemUnchangedSince`) | `store.rs:63-186` | anneal `Store::append_if` | Core. |
| **Git helpers** (common dir, worktree, tree hash per ADR-0105 cl. 4, permalinks, path-in-repo B-08) | `git.rs` 157, `env.rs` ~700 | redkiln-rs has ADR-0105 implementation; anneal resolves common dir | Core (tree hash must match across tools). |
| **Hook-safety envelope** (never fail a session, `hook-errors.log` + `.seen` offset, SessionStart reports) | `hooks.rs:1-60,200-280` | anneal "hooks fail open, guards fail closed" (`anneal/AGENTS.md`) | Core convention + small helper. |
| **Launcher/libexec packaging** (`libexec/weighin` sh + `.cmd`, exec-form, `weighin.exe` copy, newest-build pick, no-binary message, `package-plugin.sh` stray check, SKILL `V=;S=` stamp) | `plugin/libexec/*` 102 lines, `scripts/package-plugin.sh` 74, `build-plugin-bins.*` 41 | anneal ships `plugin/libexec/anneal` + `anneal.cmd` (same decision, `anneal/AGENTS.md` "Plugin rules") | Core packaging kit (templated by binary name) + one CI job. |
| **Release → marketplace → org-sync pipeline** | `release.yml` 151, `scripts/marketplace-branch.sh` 54, `changelog-section.sh` 13 | anneal `scripts/check-versions.sh`; temper's own release flow | Suite-level; one release-only repo for the whole suite (see §9). |
| **Calibration** (labelled seed JSONL, scorer through real capture path, precision bar "in force at N", `Health` metrics, weekly stats) | `eval/` (README 151), `tests/it/eval_seed.rs` 374, `projection.rs` `Health`, `scripts/weekly-stats.sh` 116 | anneal Clef Brier scores, assay verifier confusion matrix (per `suite-structure.md:205`) | Core gets the seed schema + metric types; scorer harness stays per plugin since it drives plugin-specific capture. |
| **`.kb` atom renderer + slug/YAML-safe writer** | `atom.rs` 1,107 | Knowledge is new; anneal's scope ledger cites weigh-in decisions | Belongs to Redkiln Knowledge (format) with Weigh-In as producer; the MADR/Y-statement body is Weigh-In-specific. |
| **Session journal to committed JSONL** | `weigh-in-journal.sh` 52, `journal.rs` 122 | none seen | Core (any plugin's events need to survive cloud containers). |
| **End-of-turn block grammar** | `marker.rs` 759 | temper Status block, anneal gates | Core spec for the common shell (fenced block, `key: value`), per-plugin key sets. |

## 7. Granularity findings

1. **weighin-core is four packages in one crate [inferred]:** (a) ledger domain (event, projection, card, id) — Weigh-In proper; (b) capture (`marker`, `scan`) — the grammar should become a Core spec; (c) `atom` — Knowledge's format; (d) `board` — a UI exchange contract also wanted by the mod pane (#112). In a monorepo, (c) moves to a `redkiln-knowledge` crate and Weigh-In depends on it; (a)+(b)+(d) can stay one `weigh-in-core` crate. No need to split further: everything is ~6k LOC and changes together during the pilot.
2. **The CLI crate is one binary with a natural store/env/git layer** (`store.rs`, `env.rs`, `git.rs`, ≈1,700 LOC) that is pure infrastructure and should be replaced by Core, leaving `ops.rs`/`hooks.rs`/`context.rs`/`main.rs` as the plugin's binary.
3. **Plugin boundary is right as one plugin [inferred].** Skill + 4 hooks + one binary; D-0005 rejected splitting the skill. The mod (#112) may need a second plugin `weigh-mod` if hosted sync rejects `modules` — a packaging, not a product, seam.
4. **Overlap with anneal is large and already coupled:** same store design, same libexec packaging, same toolchain, anneal shells to `weighin batch` and records `weighin_parked`. In a shared event file, anneal's bridge should become a direct append of Weigh-In events (or a Core API), not a subprocess + duplicate event.
5. **Overlap with Knowledge:** Weigh-In's "already decided" filter (step 2.1) searches `.kb` atoms via context packs; that lookup is Knowledge's query when present (`suite-structure.md:159`).
6. **Overlap with Backlog = GitHub Issues:** deferrals (flip, trigger, resolve-by, default) are items; slice 3 wanted them as Redkiln items with `blocked_by`. Under the Issues decision they would project to issues (see §8).
7. **Overlap with Loop (redkiln-rs):** slice 3's AFK `blocked_by` gating and SDK pause loop, plus wi-4d8b6a's unattended defaults, are Loop concerns consuming Weigh-In state. assay wants `weighin ci-check --json` for the PR gate. One read API (`open --json` / ci-check) serves Loop, Review and Bench.

## 8. Evidence that challenges the listed decisions

- **"Weigh-In" as a standalone suite plugin vs slice 3 "Redkiln-native `decision` item type, `redkiln new decision` the single writer" (`report.md:504-510`).** The repo's own long-term plan was to dissolve into redkiln-rs's item model, not to stay a separate plugin. The suite decision supersedes that; slice 3's "single writer" and telemetry-event mapping need an explicit record that they're dropped or re-homed (Weigh-In remains writer; Loop reads). Not a contradiction, but an unrecorded supersession [inferred].
- **One shared events file at `<common>/redkiln/events.sqlite3`.** Supports it: wi-6e2a47 and B-24 already chose one per-repo store under the common dir, mirroring redkiln-rs's `<common>/redkiln/store.sqlite3`. Challenges: (1) the store override chain (B-02) exists because Cowork folders synced by OneDrive/Dropbox must point the ledger *outside* the folder — per-plugin overrides become a single suite override; (2) **client-repo data isolation (wi-a93f02, wi-7c21e4, one-way, authority)**: a shared file makes every plugin's events co-resident; fine locally, but any suite feature that publishes/syncs the shared file (board, dashboards, journals committed by B-21) must respect `repo_class: client` per plugin. Core needs a repo-class gate, not just Weigh-In; (3) migration: existing ledgers at `<common>/weighin/ledger.sqlite3` and committed journals must be imported; `user_version` is per file, so Core must own one version namespace [inferred].
- **Core owns TRUNCATE opening.** Strongly supported; two repos already duplicate it (§6). Note weigh-in converts WAL ledgers while anneal refuses them — Core must pick (weigh-in has real 0.1.x WAL ledgers in the wild) [read].
- **"Bump both to happenstance 0.4.1" (suite-structure.md:243).** B-01/B-10 pin 0.3.2 precisely because the schema is copied from happenstance's template; any bump is a migration step and must be fixture-tested against 0.1.x/0.2.2/0.6.x ledgers. Feasible, but not free [read].
- **Backlog = GitHub Issues, no in-repo backlog.** Challenge: Weigh-In's use cases include **IQBG client repos and Cowork folders that are not GitHub repos at all** (README "Cowork"; ledger falls back to `<folder>/.weighin/`), and client items must not leave the machine (wi-7c21e4). Deferrals therefore cannot *depend* on GitHub Issues; projecting them to Issues must be opt-in per repo class [inferred].
- **Separately installable plugins via org sync.** Evidence of fragility: B-18/B-19 needed a dedicated release-only repo because org sync reads only a default branch; #126 notes org-synced plugins also load in cloud sessions (contrary to B-12). Six plugins from one monorepo will need either one release-only repo holding all plugins as a marketplace (does org sync accept multi-plugin repos? unverified) or six release repos with six deploy keys. Plugin dependencies through org sync are unverified anywhere in this repo [inferred].
- **Projects/hearthbot surface.** #126 shows a new surface (Claude Projects threads) without `AskUserQuestion`, not flagged as Cowork, ledger in an ephemeral workspace. Any suite plugin with human interaction faces the same; surface detection belongs in Core, not per plugin [read].
- **Calibration in Core.** Supported but thin here: Weigh-In's "calibration" is a labelled capture seed plus action-rate metrics, not model-confidence calibration; the grounder stability check (the real calibration piece) is unbuilt [read].
- None found against "suite not monolith", "Redkiln Rust from temper", or "Docs deferred".

## 9. Migration notes for a monorepo move

- **History:** 97 commits on `main`, ~1 MB pack locally (shallow); GitHub size 7.1 MB due to `marketplace` branch binaries [measured]. Pre-GitHub history is local-only on `archive/pre-github` and must stay local (client org name in a fixture, B-06; `.githooks/pre-push` refuses `archive/*`). Import with `git subtree`/filter-repo of `main` only; never import `marketplace` (force-pushed binaries) or `temper-telemetry` [inferred].
- **Other branches:** `feature/dreamy-davinci-h400y3-spike21`, `fix/cowork-unreleased-copy-diagnosis`, `proposal/org-sync-from-marketplace-branch` (the rejected #79), `temper-telemetry` (cloud temper telemetry, #103) [measured].
- **Workflows:** `ci.yml` (114; 3-OS matrix; plugin package job runs `claude plugin validate`), `rust.yml` (276; vendored temper ratchet, PR comment by marker `<!-- temper-report -->`, cargo deny), `release.yml` (151; tag `v*` → versions check (tag on `main`, `Cargo.toml` and `plugin.json` agree, changelog section) → reuse ci.yml → 3-target build (musl, msvc, darwin arm64) → package → release → force-push `marketplace` → SSH push to `weigh-in-plugin`). In a monorepo, tags must become per-plugin (`weigh-in-v0.9.0`) and every path (`plugin/`, `Cargo.toml` grep `^version =`, `scripts/*`) is repo-root-relative [read].
- **Secrets / keys:** `WEIGH_IN_PLUGIN_DEPLOY_KEY` (write deploy key of `weigh-in-plugin`; workflow refuses to publish without it); GitHub's ed25519 host key hard-coded (`release.yml:142`). `GITHUB_TOKEN` force-pushes `marketplace` (no ruleset by Ryan's choice, B-14). Rulesets on `main` and `v*` tags (#5) [read].
- **Marketplace coupling:** users installed via `Wet-Ink-Corporation/weigh-in#marketplace` (Claude Code), a connected checkout of `marketplace` (Cowork binary source), and org sync of `weigh-in-plugin`. The install id `weigh@weigh-in` (marketplace name `weigh-in`, plugin `weigh`) is in users' settings; renaming the marketplace breaks updates. Keep publishing those two targets during transition, or announce a re-install [inferred].
- **Hard-coded paths:** skill's Cowork download URL `github.com/Wet-Ink-Corporation/weigh-in/releases/download/v$V/weighin-x86_64-unknown-linux-musl` (dormant, but stamped into SKILL.md); Cowork fallback globs `$HOME/mnt/*/plugin/libexec/...` and `$HOME/mnt/*/libexec/...` (assumes plugin at repo root `plugin/` in a checkout); `.claude/settings.json` allowlists `/home/user/weigh-in/plugin/libexec/weighin`; `session-start.sh` builds into `plugin/libexec/<arch>-unknown-linux-gnu/`; `weekly-stats.sh` looks under `~/.claude/plugins/cache/weigh-in/weigh/*/libexec/weighin`; anneal locates `weighin` via `$WEIGHIN_BIN` or PATH [read].
- **Data paths that change if Core moves the store:** `<common>/weighin/ledger.sqlite3`, `hook-errors.log`, committed `.kb/_ledger/journal/*.jsonl` (stats folds them), `.kb/_ledger/config.json` (`store` key). A Core migration must import, not delete (Cowork: no deletes) [inferred].
- **Vendored copy:** `.claude/temper/` (temper 1.3.0) + baseline + `.claude/rules/`; in a monorepo, temper is a sibling plugin, so the vendor step becomes a workspace path [inferred].
- **Repo-level working rules** that travel with it: `/weigh-in` at end of each session (D-0010), commit journals (B-21), eval-seed upkeep per cloud session (wi-48657b), `git config core.hooksPath .githooks`, versions move together (`Cargo.toml` = `plugin.json`), SKILL.md stamp stays `V=dev; S=none;` [read].
- **Pilot timing:** wi-7c3aa0/B-23 freeze capture behaviour until 1.0.0 (≥ 2026-10-20). A monorepo move that rebuilds hooks or the store before then would invalidate #23's measurement [read].

### weigh-in-plugin (generated distribution repo)

- **What it is:** private/internal repo, default branch `main` holding exactly **one commit** (`cf38c37 Weigh-In 0.6.1 marketplace`, github-actions, 2026-10-11) [measured]. Force-pushed each release by `release.yml` over SSH with the deploy key (B-19); identical tree to the `marketplace` branch built by `scripts/marketplace-branch.sh`.
- **Contents [measured]:** `.claude-plugin/marketplace.json` (marketplace `weigh-in`, plugin `weigh`, `source: ./plugin`); `plugin/.claude-plugin/plugin.json` (0.6.1); `plugin/hooks/hooks.json`; `plugin/skills/weigh-in/{SKILL.md (stamped V=0.6.1; S=none), assets/board.html, references/rubric.md}`; `plugin/libexec/{weighin (100755), weighin.cmd (100644), weighin.exe 4.6 MB, x86_64-pc-windows-msvc/weighin.exe 4.6 MB (same blob), x86_64-unknown-linux-musl/weighin 4.65 MB, aarch64-apple-darwin/weighin 3.9 MB}` — ≈17.8 MB of binaries; GitHub size 6,045 KB.
- **Why it exists:** claude.ai org sync reads only a default branch; `main` of weigh-in is the dev tree with no binaries (B-18), so a separate repo whose default branch *is* the release (B-19, amending D-0012's "no second repository").
- **Migration implications [inferred]:** it is fully regenerable from a release zip, so nothing in it needs preserving. For a suite, either (a) one `redkiln-plugins` release repo whose single commit holds a multi-plugin `marketplace.json` (needs a check that org sync handles multiple plugins and dependencies), or (b) one release repo per plugin (N deploy keys). Unverified: whether a force-pushed default branch re-syncs cleanly (B-19 lists it as unverified; 0.5.4→0.6.1 releases suggest it works).
