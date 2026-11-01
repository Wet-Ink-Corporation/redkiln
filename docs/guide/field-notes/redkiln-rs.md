<!-- Research note gathered 2026-11-01 for the field guide (docs/guide/README.md). Kept as the evidence base behind the guide; it is not maintained, and the guide wins where they differ. -->

# redkiln-rs (to become Redkiln Loop): hard-won lessons

Mined 2026-11-01 from `/home/user/redkiln-rs` on `build/phase-0-to-1` at `b3b28a2` (660 commits; the audit read `e54ccac`, 2026-10-09, so 146 commits are new to it). Read with `references/strategy/audit/redkiln-rs.md`; this file skips architecture the audit already covers and goes after what went wrong, what was measured, and what is still open. The headline the audit could not have: on 2026-11-01 the **first real dispatch through `redkiln run`** after about 2 months of fake-backed work failed at the child's start-up, because of a JSON Schema keyword (D21-167). No real agent step has completed yet.

## Guardrails

- **[headless-agents] Never treat exit 0 / `subtype: success` as a completed structured dispatch.** F20: a card whose `tools:` allowlist leaves out `StructuredOutput` silently disables `--json-schema`. The CLI still exits 0 with `subtype: "success"`, `structured_output` is missing, and the model writes made-up JSON in prose with the wrong field names. The rule is now Halt 3 for every stage: check that the structured payload is present, and never scrape JSON out of prose. The card-adoption check became set **equality** against the declared tools, `StructuredOutput` included. Evidence: `spikes/FINDINGS.md:937-985`; `runbook/halts.md:24-30`.

- **[headless-agents] Prove that a flag took effect from the child's own `system/init`.** F7: `--agents <card>` does nothing without `--agent <name>`. A canary card answered `BANANA` with both flags and `4` with only `--agents`. F8: a plain `claude -p` loaded 12 MCP servers and about 280 tools, which made up 56% of every cache write. The runner now isolates with `--strict-mcp-config` by default and computes `isolation_ok` from `system/init`, "because an ignored flag is indistinguishable from an honoured one". F17 adds that `effective.agent` is *not* filled in when `--agent` is used, so adoption is checked from the effective tool set. Evidence: `spikes/FINDINGS.md:195-260`, `:740-751`.

- **[trust-security] Child environment = the inherited environment minus a committed denylist, and a match refuses to build the child; it is never silently stripped.** Claude Code reads many undocumented variables (proxy, locale, TERM), so an allowlist breaks it. The rule is: inherit everything, but if `ANTHROPIC_API_KEY`, `_AUTH_TOKEN`, `_BASE_URL`, `_FOUNDRY_API_KEY`, `GITHUB_TOKEN`/`GH_TOKEN`, or any `CLAUDE_CODE_USE_*` is present, construction fails with a typed error. Names compare case-insensitively, as Windows does. Only the composition root reads the process environment. A `ANTHROPIC_FOUNDRY_API_KEY` persisted at user scope on the operator's box once made the test suite red. Evidence: `crates/rk-adapter-claude/src/env_denylist.rs:1-40`; DECISIONS-2026-09-11 #16; #11 (closed by `677bf1c`, `0387a29`).

- **[git] All git calls go through one wrapper with a closed operation enum.** `GitExec::run` takes a `GitOp`, never a free string, so adding a git write means adding a variant, and each new variant has been recorded as a partial supersession of ADR-0105 clause 5. The wrapper adds `core.hooksPath=<empty dir>` and `trailer.ifexists=add` to every commit, so no call site can forget them. Git's stderr is carried along for diagnosis but is never parsed for control flow. CI greps for `Command::new("git")` outside the wrapper. It also has a **positive half**: the wrapper itself must contain the call, or the ban guards nothing. Evidence: `crates/rk-git/src/git_exec.rs:1-120`; `.github/workflows/ci.yml:500-558`; ADR-0077, ADR-0105, ADR-0193.

- **[testing] A guard that passes when output is absent proves nothing. Pair every ban with a positive control.** Halt 8 lists eight shell shapes that pass vacuously and were all found live in checkpoints:
  - `!grep` over a missing directory;
  - a status-bearing command on the left of a pipe;
  - `grep -qF "$(cat missing)"`, which matches every line;
  - `git ls-files` on an untracked path;
  - `|| echo FAIL`;
  - "no output means clean";
  - `cmd | tee file`, which truncates the file before it reads anything.

  The same reasoning appears in code: the no-egress dispatch counter has a control fake that reads 1, "because a counter nothing can move reports zero forever". Evidence: `runbook/halts.md:57-90`; `crates/rk-kernel/tests/it/main.rs:69-77`.

- **[testing] A must-fail fixture must fail for the stated reason.** `tests/fixtures/rk-clock-read` is a workspace member behind a feature gate. With the feature off the gate is green. With `--features rk-clock-read/reintroduce` it must fail clippy. Criterion (4) ran it without the feature, so the fixture "passed the lint" on a correct tree. The fix runs the red arm with the feature and greps for `use of a disallowed method`: being red for any other reason is not a pass. `rk-banned-dep` does the same for `cargo deny` bans. Evidence: commit `d6160ff`; `tests/fixtures/rk-banned-dep/Cargo.toml`.

- **[testing] Deny network to the process under test, not to the plumbing.** A `container: --network none` job cannot work, because `actions/checkout` and the cargo fetch of 218 crates run inside the container too. The `no-egress` job fetches on the networked host, carries `CARGO_HOME` and rustup into the container through named Docker volumes, then runs the kernel and pack suites in the pinned `rust:1.98.1` image with `--network none` and `CARGO_NET_OFFLINE=true`. The in-tree test only asserts that the job *exists*. The lockfile walk cannot see `std::net::TcpStream`, which needs no dependency. Evidence: `.github/workflows/ci.yml:176-290`; `crates/rk-kernel/tests/it/main.rs:79-88`.

- **[rust-craft] Determinism through clippy bans, not review.** `clippy.toml` disallows the following in all production code:
  - clocks: `SystemTime::now`, `Instant::now`, `chrono::Utc::now`;
  - randomness and ids: `rand`, `Uuid::new_v4`;
  - sleeps;
  - `env::set_var`;
  - `HashMap`/`HashSet`;
  - `std::process::Command` (each warranted site carries `#[expect(clippy::disallowed_types, reason=…)]`).

  `deny.toml` bans durable-execution frameworks (ADR-0086). The kernel stays sync with no tokio (ADR-0076). Its gate path is checked for sleep/recv/spawn/tokio/clock/fs/env (AC-242). Evidence: `clippy.toml`; `crates/rk-kernel/tests/it/gates.rs:20`; ADR-0076, ADR-0086.

- **[cost-capacity] Every paid step has a numeric cap, a go-ahead line, and refuses to re-run.** D21-105 (AFK rule): the only spend allowed unattended was S-BIN, capped at US$30 (later US$90, D21-140), run in the foreground with a spend table. Every other paid step is prepared up to its go-ahead line and deferred. The first-contact block showed why: "Uncapped, a failing attempt was retried 14 times". With the clone's `appetite.dispatches = 1`, exactly one dispatch ran and a re-run was refused before any spawn. Discarded spend still counts against the cap. Evidence: `spikes/DECISIONS-2026-09-21.md:4422-4446` (D21-105), `:7642-7660` (D21-166), D21-153 item 8.

- **[cross-platform] LF in every checkout, with `-text` rows for pinned bytes.** The operator's box had system-level `core.autocrlf=true`. It smudged 270 working files, and 1,480 of 1,757 in a run-owned worktree, to CRLF, "so no checkout on this box was byte-faithful". `.gitattributes` now opens with `* text=auto eol=lf`, and that row must stay first, because git applies the last matching row. Fixtures whose digest covers their own bytes are pinned `-text` (ADR-0151). Evidence: `.gitattributes:1-30`; ADR-0164; ADR-0151.

- **[process] Halt 1: never trust a binary you did not just build.** `cargo test` and `cargo clippy` do not rebuild the CLI binary in `target/`, and "a stale binary is how this programme reached two false conclusions that code was broken". The related Windows case: `fs::copy` (CopyFileExW) keeps the source mtime, so cargo skipped rebuilding a scripted fixture edit and a lane proof failed on windows-latest only. Evidence: `runbook/halts.md:5-8`; commit `c51826b`.

- **[human-decisions] An accepted record is never edited. Corrections go to errata, and a changed decision gets a new record.** ADR-0092: if a correction leaves every `## Decision` sentence and every "What this forbids" bullet standing, it goes in `ERRATA.md` (51 entries). Anything that moves a decision is a supersession, often "in part, at clause N". 61 of 198 records carry a partial-supersede edge. The convention keeps old citations valid, but you need the README census and the errata to know what is currently true. Evidence: `references/adr/ERRATA.md:1-10`; ADR-0092; `references/audit/2026-10-06-status/appendix.md:527-530`.

## Patterns

- **[events-store] The deleted-database test decides which tier a field lives in.** "If the database were deleted, would `redkiln run` take a different next action?" If yes, the state is tier 1: written to git first as a trailer on a checkpoint or empty commit, and a failed git write leaves zero store rows. Tier 2 (usage, cost, headroom, drift, the memo) can be thrown away. A state field with no tier annotation is a compile error, and the planner accepts only a `Reconciled<State>` that `reconcile()` minted for this invocation, with no TTL skip. `doctor --rebuild` must reproduce the stage-level next action from git alone. Evidence: ADR-0109 decisions 1-4; ADR-0181; F12, where a single trailer axis would have resumed 20.3% of slices wrong.

- **[events-store] Tags and the store-minted instant stay outside the canonical digest.** Events are digested as `{kind, payload}`. The selector keys (`item`, `sha`, `run`) and the timestamp ride beside the digest, so adding addressing never moves a golden log or changes a replay. This matches happenstance's tag model, so the planned tier-2 adapter is a drop-in (ADR-0131 decision 5; DCB declined). The adversarial pass at `5853aa8` caught ADR-0163 putting `item` inside a payload, against 0131. That is now an interim, recorded supersession in part. Evidence: ADR-0131; ADR-0182; commit `5853aa8`.

- **[cli-contract] Render, never parse: a typed sidecar, stamped sections, and drift that is restored and then gated.** Each human-facing artifact is a canonical JSON sidecar plus a layout-C `.md`. Each structured section opens with `<!-- rk-stamp template=<kind>@<N> sha256=… -->`. Only `store::locator` looks at markdown, and only to find an anchor's byte range. At every entry the store diffs each section against the rendering of the typed value. On a mismatch it records one `structured_drift` with the causing commit, re-renders from the type, and opens a `render.structured-drift` gate that blocks the stage advance but not the steps. Evidence: ADR-0189 decisions 1-4; ADR-0005 "What this forbids".

- **[human-decisions] A gate answer is bound to the tree the human saw.** A gate is `(token, tree_hash, deadline)`. If the tree hash no longer matches, the answer is void and the gate re-opens with the drift attached. There is no reconciling, and every gate has a terminal expiry path (fail, escalate or auto-decide), never a hang. `answer` requires `--explain-sufficed <yes|no>` and `--toml-opened <yes|no>` on every answer, as the measurement channel for whether the rendering was enough (SM-126). Evidence: ADR-0017; ADR-0129 decision 1; D21-96.

- **[git] Worktree ownership is a parameter from the store, never cwd.** `authorise_tier1_write` takes a `RunOwnership` from the store and returns an `OwnedWorktree` that only it can mint. The crate has no `current_dir` call, and CI bans it (AC-529). The comment notes that a cwd-based version "passes the refusal test and the positive control by accident". The run tree is `.redkiln/worktrees/<item>` on `rk-run/<item>`:
  - It is locked with `worktree lock --reason "redkiln run <item> (pid N start T)"`, so a dead pid or a reused pid can be detected.
  - On re-entry it is discarded with `reset --hard` plus `clean -fd`, *without* `-x`.
  - Landing uses `update-ref <new> <old>` as a compare-and-swap.

  Evidence: `crates/rk-git/src/worktree.rs:1-80`; `git_exec.rs` `WorktreeLock`/`ResetHard`/`Clean`/`UpdateRef`; ADR-0180, ADR-0188.

- **[headless-agents] An unattended wait is a durable mark plus a later read, never a process lifetime.** Background jobs in the harness were killed at random ages (2 to 65 minutes) with no pattern. A detached `Start-Process` runner survived 12,000 s while about 14 watchers were killed around it. The rule adopted: TTL gaps are `ttl-mark`/`ttl-read` timestamps, results are resumable per `(arm, slug, repeat)`, and "anything that must survive belongs outside the harness's job tree". Halt 5 adds that a waiter with no `exited with code` branch is a hang. Evidence: `spikes/FINDINGS.md:868-930` (Operational), `:1171-1203` (F22); `runbook/halts.md:44-48`.

- **[testing] Evaluation fixtures must make the answer unreachable, not merely absent.** In E3's first pilot, Opus had Bash in a worktree whose object store still held the fix. It checked out the fix, reviewed the real code, ran its own mutation test, and returned `approved`. Every `s-silent` record would have scored it against the wrong ground truth. The fix: a shallow fetch of the fix's **parent** into a fresh repository. The same defect class recurred in S-BIN (see "Agents follow absolute paths out of the sandbox" under Gotchas). Evidence: `spikes/FINDINGS.md:690-739` (F17).

- **[process] Adversarial refuting rounds over green work, and they keep finding real defects.** Each stage closes only when a formal checkpoint passes whole *and* a refuting round finds no blocker or major. Commits `R1FIX-…` to `R11FIX-8i` record each round. Three verifiers re-derived Step 7i's committed, checkpoint-passing work and found **36 defects**, 13 of them surviving mutants (`5853aa8`). Step 7g's "obvious" one-line repair was measured as wrong for 43 of 63 activities: it would have silently dropped a review step (`2ce5576`). Evidence: commits `5853aa8`, `2ce5576`; D21-105 ruling 1.

- **[cost-capacity] Headroom in integer ticks from the undocumented `rate_limit_event`.** `utilization` is quantised to 0.01, so it cannot measure the cost of one run. The throttle works on visible unified windows with a two-tick margin and never credits headroom from a cache metric. It is the best-specified seed for a cross-plugin capacity lease. The gap that is still open: the guard is local to each process, and every resumed `redkiln run` starts with `Guard::empty()`, so its first dispatch skips the check (#21, open). Evidence: ADR-0110; F1, F6; `crates/rk-adapter-claude/src/throttle.rs:88-98`; #21.

- **[cli-contract] Dated, in-place provenance on version-keyed tables.** Version-sensitive argv sits in a const table keyed by the configured CLI pin: `PERMISSION_MODE_ARGV = [("2.1.303","manual")]`. A pin with no row is refused, with no fallback. A help-capture test pins each row to a recorded `--help` byte capture, with live-accepted rows listed separately (`LIVE_WARRANTED`). This is how the rename of `default` to `manual` was absorbed at the pin move. Evidence: `crates/rk-adapter-claude/src/command.rs:134-177`; commit `ea33bcf`.

- **[process] Read-only multi-agent audits with adversarial verification.** The 2026-10-06 status audit ran on a detached worktree at a pinned SHA, so the live session was untouched:
  - 17 survey agents;
  - 13 headline claims, each attacked by two Opus skeptics (7 confirmed, 6 corrected, 0 refuted);
  - a completeness critic;
  - each finding re-checked at two later SHAs before it was filed as an issue.

  The same panel shape (investigators, lensed judges, synthesiser) grounded operator rulings such as D21-106 and D21-153. Evidence: `references/audit/2026-10-06-status/README.md:22-34`.

## Gotchas

- **[headless-agents] `--json-schema` rejects draft 2020-12's `$schema` keyword, and the error only appears on the first real dispatch.** Every `schemars` export carries `"$schema": "https://json-schema.org/draft/2020-12/schema"`. CLI 2.1.278 and 2.1.303 fail at start-up with "no schema with key or ref …", accept draft-07 or no `$schema`, exit non-zero in 2.9 s, and spend nothing. Live probes had used hand-written schemas without `$schema`, so months of fake-backed tests never saw it, and "No real agent step could have run". The fix strips only the top-level `$schema` from the argv element, while return-side validation keeps the whole export. A test runs every exported schema through it. Evidence: commit `34c36fe`; `spikes/DECISIONS-2026-09-21.md:7661-7701` (D21-167, D21-168); `crates/rk-adapter-claude/src/command.rs` `json_schema_element`.

- **[headless-agents] `oneOf` is outside the vendor's structured-output contract, and serde's defaults produce it.** schemars 1.2.2 turns internally tagged enums into `oneOf`, and `#[serde(untagged)]` produces `anyOf`, which a grep for `oneOf` misses. ADR-0095 bans `oneOf`, allows `anyOf` only through a transform, and counts union-typed parameters per exported schema. Evidence: ADR-0095 (`:18-31`, `:50`).

- **[distribution] Claude Code CLI flags change spelling between patch versions, and the auto-updater is not the only thing that moves the binary.** `--permission-mode default` became `manual` by 2.1.303 (X12). `DISABLE_AUTOUPDATER=1` stops only automatic updates, and a manual update moved the pinned 2.1.289 to 2.1.303 mid-stage. The programme records each move under Halt 2. A floor check (`>= 2.1.251`) is not a match, any data set that spans a version boundary is reported per version, and a launcher records sha256 before and after to detect a move. Evidence: `spikes/DECISIONS-2026-09-21.md:7604-7625` (D21-164); `runbook/halts.md:10-22`; commit `ea33bcf`.

- **[trust-security] Agents follow absolute paths out of the sandbox when the task gives them a motive.** In S-BIN, item 1's acceptance criteria were runtime-only and the review trees had no binaries. The reviewer followed the real project's absolute path, found in `CLAUDE.md`, `AGENTS.md` and workspace docs, and ran `git show` inside the operator's real repository. Both attempts were voided and the pass-1 set ($16.07) was discarded. The fixes:
  - strip every absolute spelling of the path and add those spellings to the registered leak tokens that the pre-spawn check refuses;
  - tell the agent plainly that it has no runtime and that any outside read voids the record;
  - make the audit's cwd model follow the CLI, which resets cwd between Bash calls.

  Evidence: `spikes/DECISIONS-2026-09-21.md:7142-7180` (D21-153); D21-143, D21-144.

- **[git] Git's detached auto-maintenance races fixtures.** A root commit in a temporary repository can start background `git maintenance`. Its `worktree prune` deleted a planted stale worktree entry, which made a doctor test flaky on the pull_request run but not the push run of the same SHA. It also put `objects/maintenance.lock` into a "reconcile mutates nothing" fingerprint. Every fixture repository now sets `maintenance.auto=false`, `gc.auto=0` and a repo-local `user.name`/`user.email`, because hosted runners have no global identity. Evidence: commits `f433854`, `ef747da`.

- **[cross-platform] Windows path spellings break git and path comparisons.** `fs::canonicalize` returns `\\?\D:\…`. Slash-flipped, that becomes `//?/D:/…`, which git parses as a URL with host `?`, so the E3 fixture builder "had never once completed" even though its unit tests were green. On hosted Windows runners `%TEMP%` is the 8.3 short form `C:\Users\RUNNER~1`, while git reports long paths: 7 tests failed on a correct product. Fixes: strip the prefix through `WorktreePath`, canonicalise temp roots, and point `TMP`/`TEMP` at `RUNNER_TEMP`. Evidence: `spikes/FINDINGS.md:987-1013` (F20 defect 2); commit `ef747da`.

- **[cross-platform] Windows reuses PIDs fast, and on POSIX zombies keep a process group "non-empty".** Fixture children published `<pid>.marker` by rename. Rename overwrites, so two sequential children with the same PID left one marker (13 spawns counted for 14). Markers are now hard-linked to the first free name, which is atomic and never clobbers. On POSIX, `kill(-pgid, 0)` succeeds while any member is an unreaped zombie, so `Group::is_empty` lied. It now reads a zombie-filtered `/proc` listing. Evidence: commits `4a36f19`, `d2b6a86`; #20.

- **[git] gix cannot write trailers, and a hand-rolled trailer writer does not match git byte for byte.** S-GIX/b found no trailer-writing API in gix 0.87.1. A hand-rolled route matched `git interpret-trailers` on only 3 of 5 cases, diverging at byte 14 on `divider` and at byte 29 on `crlf`. So gix stays read-only and writes shell out (ADR-0077, ADR-0091). Trailer parsing must also be limited to the commit's *final* trailer paragraph (#18). Evidence: `runbook/PROGRESS.md:29` (S-GIX arms); ADR-0091; #18.

- **[git] A worktree nested inside the repository pollutes recursive greps.** A checkpoint's `grep -rn … --include=*.md .` also searched the run worktree under the common dir and read a stale copy of `runbook/` as a hit. `.redkiln/worktrees/<item>` must be git-ignored (there is a `check-ignore` refusal) and excluded from tooling. Evidence: D21-40 (`spikes/DECISIONS-2026-09-21.md:1627-1632`); `git_exec.rs` `CheckIgnore` doc.

- **[events-store] WAL is asserted, which conflicts with the suite's TRUNCATE.** `rk-store` runs `PRAGMA journal_mode = WAL` with `query_row`, because `execute` refuses a statement that returns a row. It refuses any other mode, and `doctor` refuses network filesystems "because WAL does not work over one". R-SQLITE's two-process half was consumed at Step 6 (AC-181), but its **load** half (5000 ms `busy_timeout` under self-hosting volume) has never been measured. Evidence: `crates/rk-store/src/conn.rs:186-200`; ADR-0105 clause 7 (`:77-80`, `:151`); `runbook/phase-1/s7c-adapters.md:3297`.

- **[llm-judgement] A model-emitted classification vector is unstable, and the instability comes from the schema, not the temperature.** In P1 arm L (400 dispatches, $21.37) the modal `R(stage, ctx)` was identical on only 5/20 and 3/20 items, against an 80% bar. It was corroborated across frontier models, including temperature 0. ADR-0079 blocked pack authoring and then forbade amending itself, so it had no exit. ADR-0090 superseded it with "every ordinal axis and all three booleans are `source = human` for Phase 1", and the engine refuses an LLM-emitted value. Drivers: "a decision must exist even when the measurement clears nothing", and do not swap in a weaker bar. Evidence: ADR-0079 (superseded); ADR-0090 drivers and options A-E; F36, F39.

- **[llm-judgement] Keyword derivers fail on the instrument's own vocabulary.** The `auth` facet scored 0/12 because `auth` is inside *authored* (40 hits), `oauth` inside *coauthor*, and `token` matched design tokens (36). Word boundaries fix only the minority of those hits. A scorer that compared a JSON bool to the string `"true"` printed a clean 0.00 where the true score was 0.60. Halts 9-13 turn these into rules:
  - report a criterion with no positive class as not measurable, never 0.00;
  - read the scorer before publishing a clean number;
  - do not apply a remedy that the same measurement contraindicates (ADR-0117).

  Evidence: `runbook/halts.md:92-152`; F39, F40.

- **[headless-agents] Hooks fire in `-p`, effort applies per dispatch, and the prompt cache TTL is about 1 h in two segments.** F2: settings hooks fire in headless mode, so in-session plugins' hooks run under `claude -p` children unless the child is isolated. F15: effort is applied per dispatch. F10/F13: the first cache decay showed at 45-62 minutes, and 69% of the prefix is shared across directories. F18: half of human gates outlive the TTL, and a resumed gate costs a bounded 18.6k tokens. Evidence: `spikes/FINDINGS.md:82-101`, `:261-324`, `:471-517`, `:564-607`, `:754-795`; ADR-0017 evidence.

- **[testing] Second-granularity clocks break expiry tests.** The clock is read in whole seconds, the deadline is now + 1, and expiry is `deadline < now`. So a gate opened at second S expires only from S+2.0, and a fixed 1.5 s sleep missed whenever the gate opened early in its second. The test now polls until the gate's recorded deadline has passed. Evidence: commit `aab766c`.

## Lessons from history

- **[testing] Running the real binary found what fixtures and mutants did not, three times.**
  1. 7h.1 (`59be987`): "six lane tests, seven mutants" were green, but the second `redkiln run` appended a self-transition `analysis -> analysis`. Only a *replayed* journal shows it. ADR-0157 added a guard, and ADR-0161 then found the guard insufficient for lanes longer than one hop.
  2. 7g (`ee893b6`): 121 of 121 `when` guards and 94 of 94 `check` expressions failed, because the executor evaluated against an empty `Context::new()`. ADR-0136 had named the wrong cause.
  3. 2026-11-01: the first real dispatch failed on `$schema` (D21-167).

  The 10-06 audit predicted this ("Both earlier insertions came from first contact") and recommended one real dispatch before the freeze. That is exactly what found D21-167. Evidence: commits `59be987`, `ee893b6`, `34c36fe`; `references/audit/2026-10-06-status/appendix.md:673`.

- **[events-store] The lane stalled across processes because the successor came from the record, not the fold.** `resolved.stage` was read from the checkout's `.redkiln/items/*.toml`, which never moves, so a second `redkiln run` reported "done" having done nothing. A lane completed only within one process (ADR-0157 d4, "named here as OWED"). The 2026-09-24 decision packet (nine drafters, nine refuters) chose a stage-bearing `Story: <project>/<slug> <stage>` trailer on `rk-run/<item>`, folded by `reconcile`. That superseded ADR-0155's one-mark-per-item grammar, which had rejected the same option "for this stage, not on merit". Evidence: `spikes/S8-DECISION-PACKET-2026-09-24.md:7-60`; ADR-0157, ADR-0161, ADR-0183; D21-33, D21-41.

- **[process] "Checkpoint passed" was un-passed when the record overclaimed.** Row 40 said Checkpoint 8 passed at `d6160ff`, but only the mechanical block had. D21-40 corrected it ("Checkpoint 8 has NOT passed, and M5 is not reached"). It found that nothing staged or landed the harness's work, that `GitOp::Integrate` had no caller, that 0 of 8 cards carried `permission_mode`, and that no test called `verbs::run::run`. Step 8 was re-opened into sub-stages s8a to s8j. Evidence: `spikes/DECISIONS-2026-09-21.md:1585-1634` (D21-40).

- **[human-decisions] AFK "flip on silence" built up 20 unconfirmed rulings that accepted records rested on.** Under D21-105, agents took one-way calls "in their most reversible form", recorded as TAKEN PROVISIONALLY and often decided by LLM judge panels. Several ADRs (0189, 0191, 0194, 0196) flipped to accepted on "the operator has not objected". The audit found 20 rulings with no recorded confirmation and code that already depended on them (#28). The operator later confirmed all twenty in one sitting (D21-156). The lesson for Loop: provisional authority needs a scheduled ratification step, or silence turns into policy. Evidence: #28 (closed); D21-105, D21-156; `references/audit/2026-10-06-status/appendix.md:531`.

- **[process] Process weight became the constraint.**
  - The runbook is 53,707 lines (`runbook/phase-1`), 32k of them in s8.
  - 30-50% of source lines are dated provenance.
  - Docs-only commits rose to 70% by week 5.
  - Checkpoints grep prose, so prose is load-bearing: a `Cargo.toml` comment "is kept because Checkpoint 5 greps the literal string".
  - Touching any crate triggers whole-checkpoint re-runs.
  - s8i was open 74 h with 0 lines of Rust.

  Scope grew 114% in 24 days with nothing capping it (#72 open). Since the audit, 146 commits added roughly equal Rust and markdown (+10.8k vs +10.5k). Evidence: `references/audit/2026-10-06-status/appendix.md:539-562`; `Cargo.toml:8`; #72.

- **[human-decisions] Superseded records, and why.**
  - **0004 → 0074** (review findings-not-verdicts): every operative term had been redefined, so the whole record was replaced.
  - **0088 → 0101** (confirmation-bias arm): the cited paper's title, effect size and mechanism were wrong. A bug-free framing *collapses* false positives; it does not leave them flat, so the pre-registration predicted the wrong direction.
  - **0079 → 0090**: see the context-vector item under Gotchas.
  - **0041 → 0064**: outcome grammar.
  - **Run-tree reuse**: ADR-0105 clause 6, "re-created per run, never reused", was superseded in part by ADR-0180 d9. The run tree is now reused on re-entry and removed only when it is clean, landed and unlocked.

  Evidence: ADR-0074 `:103`; ADR-0101 `:20-33`; `crates/rk-git/src/worktree.rs:13-22`.

- **[testing] Non-hermetic tests and CI first contact.** CI ran for the first time on 2026-10-03, and Linux and Windows each failed on fixtures, not on the product (8.3 paths, missing git identity, maintenance races, POSIX-dishonest dispatch tests). The records kept saying "CI never ran" for weeks after it had, and needed dated corrections (#23). Evidence: commits `d4da70a`, `ef747da`, `a7806cf`; #23.

- **[trust-security] Single-machine evidence.** The test-oracle corpus (`CORPUS_REPO=D:/repos/redkiln-corpus`) had no remote and held the S-BIN evidence (#32). It is now an off-machine verified bundle, restore-tested byte-identical, with `* -text` so every clone reproduces the scored bytes. Corpus reads go only through the pinned SHA's object store, never a working tree (AC-464). PIA-036 showed a checkpoint reading the *current* pin where it should have read the lift SHA, which made Checkpoint 6 unpassable from 7b's pin move onward without anyone noticing. Evidence: `CORPUS_PIN:1-30`; commit `6de2722`; #32; `runbook/PROGRESS.md:29` (D17-2).

- **[cost-capacity] Spend actually incurred.** About US$660 API-equivalent across spikes:
  - E3: $153.86
  - S-VAR: $176.96
  - S-TYPED: $112.25
  - P7: $131.62
  - S-BIN: $5.56 of a $90 cap, plus the discarded $16.07

  Validating one item before paying for the full set "paid for itself twice in one sitting" (F20). Without that, the F20 defects would have voided the whole Opus/xhigh E3 budget. Evidence: `references/audit/2026-10-06-status/appendix.md:564-572`; `spikes/FINDINGS.md:931-935`.

## Open problems

- **[headless-agents] No real agent step has completed through `redkiln run`.** Step 1a reached the CLI and failed at start-up (D21-167). Step 1a-2 (one Sonnet dispatch, 9e41173 or later) and probes 0-2 are approved but unrun as of `b3b28a2`. `live_dispatch_probe` at 2.1.303 is owed. The least-privilege card route (ADR-0197, still `proposed`) and the permission-denial channel are unmeasured live. Evidence: D21-168; commit `ea33bcf` ("live_dispatch_probe at 2.1.303 is OWED").

- **[events-store] R-64: can `doctor --rebuild` re-derive every event-store tag from git?** It is unmeasured. It decides whether a shared store can be a rebuildable cache for Loop, and AC-184's exclusions are still being named. Evidence: #76, #90; ADR-0131 decision 10.

- **[events-store] Tier-2 store contention under load.** R-SQLITE's load half has never run, happenstance (S-HS) is not wired, and the journal mode conflicts with Core's TRUNCATE. Evidence: ADR-0105 `:151`; ADR-0131 `:111-152`.

- **[cost-capacity] Budget admission is not atomic and fails open.** A malformed `usage` row charges 0 (`continue`), and `journal` + `admit` + `append` are not under one guard, so two runtimes can both pass the cap (#9, #47). The throttle is local to each process (#21). Evidence: #9, #21, #47; `crates/rk-kernel/src/workflow/envelope.rs` (`:448` at `ce4ed1a`).

- **[process] There is no defined recovery after a failed first real run.** Checkpoint 8j is one-shot, and it refuses an existing `rk-run/<item>` and a moved store. D21-163 approved a "failed-first-run procedure", but it has never been exercised. Evidence: #25; D21-163.

- **[headless-agents] Parallel lanes and worktrees are designed but unbuilt (Phase 2).** Windows handles that `Stop-Process` cannot release (Halt 6) mean one fixture tree per concurrent arm, and a tree that will not delete is kept and never reused. Worktrees also fragment the prompt cache (F16 priced commits at 31% of the prefix). Evidence: `runbook/halts.md:50-53`; `crates/rk-git/src/worktree.rs:18-22`; F16.

- **[llm-judgement] The review gate is uncalibrated.** Verdicts behave like a weighted coin at n=10 (F29: a reproduced regression was approved 3/10), and S-SUBTLE-CB is disclaimed. Quarantine and advisory retirement are built but not wired into runs (#13). Evidence: ADR-0101 `:38-40`; #13; `appendix.md:676`.

- **[process] Records cannot be trusted without a census.** The trackers held stale rows ("CI never ran" after it had, a "passed" before the checkpoint had passed). God modules remain: `run.rs` at 6.5k lines and `worktree.rs` at 2.9k (#70, #71). The temper gate on the merged Rust is not yet green (#77). Evidence: #23, #70, #71, #77.
