# Move 3 runbook: Review (draft)

assay moves from `Wet-Ink-Corporation/assay` into this repository as Review: the `rk-review` plugin and the CI deployment under `deploy/review/`. assay finished its move preparation on 2026-11-07 and has been frozen by Ryan since 2026-11-08: no work happens there until this move starts.

This runbook is a **draft**, written early while assay's state is fresh. Move 2 (Bench) closed on 2026-11-20 and Rust 2.0's import comes next, before this move ([README.md](README.md), Rust 2.0 row), so the facts below are re-checked again when Move 3 is next, and the calls marked "at Move 3" are put to Ryan then. It does not re-plan: the end state is `references/docs/review.md`, the formats are `spec/`, the rules are `AGENTS.md`, and the template is [move-1-weigh-in.md](move-1-weigh-in.md).

Facts below were checked on 2026-11-08 against assay `main` at `eeb2a45`, assay-ledger `main` at `3ae05eb` and redkiln `main` at `d80e7fb`, and re-checked on 2026-11-25 against assay `main` at `eeb2a45` (identical: nothing merged since, per `compare/eeb2a45...main`), assay-ledger `main` at `3ae05eb` (still one commit, `README.md`) and redkiln `main` at `b5e4ec6`. Where a re-checked fact touches a call, a *Drift (2026-11-25)* note says so; the calls themselves are unchanged.

## Preconditions

| # | Precondition | State | Evidence |
| --- | --- | --- | --- |
| P1 | The in-place rename to `review` (RV call A, the migration plan's gate) | **Met** | assay #399, merged by Ryan 2026-11-07: crates `review-<x>`, `REVIEW_*` env vars and secret references, schema ids `review.*/1`, checks `review / verdict` and `review / gate: <name>`, App names `redkiln-review` and `redkiln-review-ledger` in the docs. Golden `inputs_digest` values regenerated (owner-approved trust-path change) |
| P2 | Trust path and architecture test narrowed to Review's crates | **Met** | assay #399: trust globs `crates/review-*/**` and `crates/review/**`; `crates/review-cli/tests/it/architecture.rs` checks members under either root or named `review-*`, and finds the workspace root through cargo |
| P3 | The owner steps of the rename sitting | **Met** | Ruleset `25356300` requires `review / verdict` from integration `5371952` (read 2026-11-08 and re-read 2026-11-25, unchanged; enforcement `evaluate`; environments `assay-publish` and `assay-review` present). `main` is the default branch. Ryan confirmed on 2026-11-08 that both Apps were renamed (`redkiln-review`, `redkiln-review-ledger`) and that assay's environment secrets and variables were renamed |
| P4 | Weigh-In adapter on `weighin ci-check --json` | **Met** | assay #372 (`crates/review-gates/src/weigh_in.rs`): runs the pinned `weighin ci-check --json --base --dir --store` and reads `weighin.ci-check/1` strictly; the 1.x file reader is gone. Weigh-In writes `weighin.ci-check/2` since #48. Two gaps remain for Weigh-In 2.x, listed under Code work |
| P5 | No open PR, nothing half-finished | Met for human PRs | 2 open PRs, both Dependabot and opened after the freeze: #400 (`toml` 1.1.8 to 1.2.0, `Cargo.lock` only, 2026-11-08) and #402 (`oras-project/setup-oras` 2.0.3 to 2.0.4 and a `taiki-e/install-action` bump in `rust.yml`, `release.yml` and `warm-target.yml`, 2026-11-22). Five stale branches (`claude/assay-229-gates-test-followups`, `claude/project-thread-aezcw7`, `-jm46k7`, `-ncav8t`, `spec/architecture`) plus the two Dependabot branches, none imported (the import takes `main`'s tip only); Ryan deletes or keeps them |
| P6 | Open issues triaged into transfer or close | **Open** (proposal below) | 94 open issues (re-counted 2026-11-25, unchanged): the plan's milestones and epics (#1 to #34), v1 stories still open (#60, #67, #78, #85, #100, #101, #103, #110, #113, #114; parts of #60 and #67 are built per assay's decisions log), calibration and go-live items blocked on Ryan (#41, #79 to #81, #94, #104, #105, #115 to #117) and about 45 review follow-ups. Triage happens at the start of Move 3, since assay is frozen; a proposal for M3-D is under [P6 issue triage](#p6-issue-triage-proposal-for-call-m3-d-2026-11-25) |
| P7 | Core 1.0 and SPEC 1.0 | **Met** | Cut 2026-11-20 at Move 2's close (M2-G, #134). Review builds on Core's envelope, trust levels and approvals (`references/docs/review.md`, What Review consumes) |
| P8 | The kits Review is meant to use | **Met** for the two cut at Move 2 | `redkiln-github` (`crates/kits/redkiln-github`) and `redkiln-calibrate` (`crates/core/redkiln-calibrate`) were cut from anneal at Move 2. `redkiln-capacity`, named in code work step 5, does not exist yet. Review imports on its own code and converges onto the kits afterwards, as Weigh-In converged onto Core |

*Drift (2026-11-25):* P5's two Dependabot PRs did not exist on 2026-11-08. Neither is merged (assay is frozen) and the import takes `main`'s tip, so both close unmerged with the archive unless Ryan merges them first. #400's `toml` 1.2.0 is a lock-only bump inside the 1.1 floor, so it does not change the root's `toml` row below; #402 touches only workflows that are not placed (rewritten into `ci.yml`). Neither touches M3-A to M3-F.

## What is different about this move

- **Review is not live anywhere yet.** No repository runs the pipeline: the workflow templates (`workflows/assay.yml.tmpl`, `assay-decide.yml.tmpl`) are pinned by tests but not installed, the M1 canary suite (#78) has not run, and `review / verdict` is required on no consumer (#81, #116). So nothing breaks for a user during the move, and the door renames below are cheap now and costly once a consumer installs the workflow.
- **It is two deliverables.** The CI deployment (binary by digest, workflow templates, the Apps, the ledger repository) holds the authority. The plugin (`/assay:check`, `/assay:replay`, `/assay:report`) is advisory and owns no state (Blueprint call 4).
- **assay-ledger is not imported.** It stays Review's private ledger repository (ADR-19), written only by the ledger App (ADR-26). Today it holds one commit (`README.md`). Whether to rename it (to `review-ledger`) is Ryan's call at Move 3.

## Where the files land (defaults, settled in the import PR's plan)

| From (assay) | To (redkiln) | Notes |
| --- | --- | --- |
| `crates/review-<x>/` (six crates) | `crates/review/review-<x>/` | Add the `crates/review/*` glob to the members. `review-contract` joins later (code work, step 3) |
| `plugin/.claude-plugin/`, `plugin/commands/` | `plugins/review/` | Install id `rk-review`. Under CLI first the commands run the binary through a `libexec/` launcher copy, not from `PATH`; `cargo xtask plugins` then checks that every `libexec/<binary>` they name ships |
| `plugin/hooks/pre-push` | `tools/review/pre-push` | A git hook script, not a Claude Code hook; `plugins/` holds no scripts (charter, CLI first). It only runs `assay pre-push`, so it can later become a `review` subcommand that installs it |
| `plugin/prompts/<version>/` | `deploy/review/prompts/<version>/` | Prompt bundles are compiled into the binary (`include_str!` in `crates/review-cli/src/bundle.rs`), not plugin content, and `cargo xtask plugins` has no place for them. Moving them means updating the include paths with the bytes unchanged, since their hash is part of the evidence key (`PromptBundle`) |
| `workflows/*.yml.tmpl` | `deploy/review/workflows/` | Consumer templates. Their argv tests (`crates/review-cli/tests/it/pipeline_argv.rs`) stay with the crate and are retargeted at the new path |
| `.github/actions/fetch-by-digest/`, `warm-target/` | `deploy/review/actions/` | Consumers reference them as `Wet-Ink-Corporation/redkiln/deploy/review/actions/<name>@<sha>` |
| `rubric/`, `schemas/` | `deploy/review/rubric/`, `deploy/review/schemas/` | What the deployment reads. `schemas/findings-1.schema.json` becomes `review-contract`'s `review.findings/1` (step 3) |
| `corpus/` (1,512 files, about 12 MiB in git) | `evals/review/corpus/` | Eval cases and goldens, never shipped |
| `docs/` | `docs/review/` | Architecture 01 to 07 (the ADRs are in `07-decisions-assumptions.md`), decision records 0002 to 0007 (`docs/decisions/`) and the reviews keep their paths below it |
| Root `AGENTS.md`, `CLAUDE.md` | `docs/review/charter.md`, `docs/review/working-rules.md` | The decisions log (76 rows) moves with the charter |
| `.kb/_intake/decisions/` (28 atoms), `.kb/_ledger/journal/` (3 journals), `.kb/_ledger/defaults-digest-2026-10-12.md` | The same paths | Atoms keep their ids and move to `.kb/decision/` with the rest (Knowledge call B); journals are imported once by Weigh-In's legacy reader (Move 1 code work, step 8) |
| `.assay/` (`globs.toml`, `owners`, `agents`) | `.review/` at the door | Review's own config for gating this repository. `.review/` is the suite's one exception to `.redkiln/<plugin>/`, because Review reads it from the base branch (`references/docs/review.md`, Events and state; Review call A). Its trust globs name assay's paths and must be rewritten for this layout, a trust-path change for Ryan (05 T16) |
| `.github/workflows/` (`rust.yml`, `release.yml`, `warm-target.yml`), `.github/dependabot.yml` | Not placed | Jobs rewritten into `ci.yml` under `ok`; release and warm target into this repository's release flow |
| `.claude/` (temper 1.3.0 copy, `temper.json`, rules), `Cargo.*`, toolchain, `clippy.toml`, `rustfmt.toml`, `deny.toml`, `.cargo/`, `.githooks/` | Not placed | The root's copies govern; diff each in the import PR (below) |

## What the root must provide

Diff in the import PR; anything missing is an H-16 or H-05 call, not a silent merge.

| assay has | redkiln at `b5e4ec6` (2026-11-25) | Call |
| --- | --- | --- |
| `toml` 1.1 (`std`, `parse`, `serde`) | `toml` 1.1 (`std`, `parse`, `serde`); was 0.9.12 at `d80e7fb` | Settled and done: the root stepped up to 1.1 before Move 2 (Ryan, 2026-11-08) |
| `sha2` 0.11 (no default features) | `sha2` 0.11 (default features); was 0.10 at `d80e7fb` | Settled and done: the root stepped up to 0.11 before Move 2 (default features stay the root's) |
| `jsonschema` 0.58 as a **normal** dependency of `review-app`, with `deny.toml`'s `MIT-0` (borrow-or-share) and `ahash`/`wasm-bindgen` wrapper exceptions | `jsonschema` 0.58.6 (default features off) is a dev-dependency of `redkiln-core`, `weighin`, `bench` and `skillcheck`, never a normal one (`docs/decisions.md`, 2026-11-08 D1 and 2026-11-11 C3). Of assay's `deny.toml` exceptions only the `ahash` ban wrapper was applied; the `MIT-0` exception (`borrow-or-share`) and the `wasm-bindgen` wrapper were not, because cargo-deny 0.20.2 raised neither for dev-only use | Normal-dependency use is a new H-16 row |
| `proptest` 1.12, `syn` 2 (`parsing`, dev) | `proptest` 1.12 (dev, `std`) is a root row since R2-E (`docs/decisions.md`, 2026-11-15), used by `redkiln-calibrate`; `syn` absent | H-16 rows (both are assay pre-approvals, not redkiln's) |
| `rust-version` 1.96 | 1.98.1 | Already decided: products take the suite's one pin (Blueprint call 5, signed off 2026-10-18). Clippy may raise new MSRV-gated lints, fixed in the import PR |
| No licence field: private code (assay decisions log, 2026-10-11) | Every member inherits `license = "MIT OR Apache-2.0"` | Importing relicenses the crates and docs: Ryan's call at Move 3 (M3-F) |
| No baseline (as strict as `-D warnings`) under temper 1.3.0 | temper 1.6.0 at the root (#52, done 2026-11-10; R-4, the latest 1.x), until Rust 2.0's import replaces the vendored copy | Expect no re-baseline; any finding the root's temper adds is fixed or baselined at the new paths |

*Drift (2026-11-25):* two rows moved since `d80e7fb`. `jsonschema`: the 2026-11-08 decision row adds the `MIT-0` and `wasm-bindgen` exceptions only if a finding appears, and none did for dev-only use; `review-app`'s **normal** dependency makes cargo-deny check `borrow-or-share`'s licence, so the import PR should expect to add the `MIT-0` exception under that new H-16 row. `proptest`: already a root row, so only `syn` still needs a new H-16 row. Neither touches M3-A to M3-F.

## Code work, in order (sketch)

1. **Import** (placement merge, then the door rename). The crates move one level deeper and the data moves, so every path to repository data is retargeted. In production code these are `include_*!` paths: schemas (`review-app/src/contract.rs`), prompts (`review-cli/src/bundle.rs`) and the rubric (`review-cli/src/check.rs`), each embedded byte for byte. In tests about 34 files reach data through `../..` and `CARGO_MANIFEST_DIR` joins (schemas, corpus, rubric, prompts, `.assay/`, fixtures in other crates); retargeting them is a test change to list for Ryan (H-12), though no assertion changes. The architecture test gains the seventh crate when `review-contract` lands (H-16 test change).
2. **Weigh-In 2.x gaps** (with redkiln #48, wi-e41ab9). `gates-static` fetches `refs/redkiln/journal/*` beside the pinned base and head, and the findings stop anchoring journal violations at `.kb/_ledger/journal` (`crates/review-gates/src/weigh_in/findings.rs`). #48 ships `weighin.ci-check/2`: read it with `weighin_contract::ci_check::v2::Data::from_envelope`; anchor `kind: file` violations at their path and `kind: ref` violations at the ref and commit; treat an empty `data.read.refs` (or the `ci-check/no-journal-refs` finding) as nothing fetched; map the new reasons (`merge`, `tree`, `unended`, `too-large`, `rewritten`, `unreadable`); and decide what a non-zero `data.unreadable` does (counted, not a violation, EV-10).
3. **`review-contract`** at `crates/review/review-contract/`: `review.findings/1` with its fields unchanged (Review call B), depending only on Core. Rust, Bench and Loop emit it.
4. **The pinned temper.** `gates-static` and `gates-lint` take `--temper` only from an `oci:sha256:` pin (D8 A), and temper's GHCR artifact (temper #193) was never built; temper's own triage says Review pins temper inside the monorepo instead (wi-39f19a, decided by Ryan on 2026-11-08 in temper #215: closed as superseded; it read `open` here on 2026-11-08). Since temper is imported before this move (Ryan, 2026-11-08), Review can pin the engine already in this repository; settled with wi-39f19a at that import ([rust-2.md](rust-2.md)). On 2026-11-25 that import has not happened yet (no `crates/rust/`); it is the current milestone.
5. **Converge onto Core and the kits**: events on Core's envelope, capacity leases on `redkiln-capacity` (not yet cut on 2026-11-25), GitHub through `redkiln-github`, calibration through `redkiln-calibrate`. Each is its own PR after the import is green.
6. **Owner setup against this repository** (call B was overtaken: setup happened against assay first): install `redkiln-review` and `redkiln-review-ledger` on `redkiln`, create the environments, and require `review / verdict` here only once Review runs on this repository (assay #118, retargeted at redkiln).

## Calls at Move 3

Put to Ryan when Move 3 is next, each with a recommendation then. Listed now so nothing is lost.

| # | Call | Door |
| --- | --- | --- |
| M3-A | Door renames #399 left: the binary `assay` to `review`, `/assay` commands, `assay:*` labels, `.assay/` to `.review/`, `refs/assay/*`, check output titles `assay: failure (<code>)`. Cheap only while no consumer runs the workflow | One-way once a consumer installs it |
| M3-B | The environments `assay-publish` and `assay-review`, the GHCR packages (`assay`, `assay-warm-target`) and the `assay-ledger` repository: keep or rename | Owner |
| M3-C | Placement of prompts, rubric, schemas and corpus (defaults above) | Two-way |
| M3-D | Which of the 94 open issues transfer | Two-way |
| M3-E | When Review starts gating this repository, and with which trust globs | Owner (trust paths) |
| M3-F | Licence: assay's crates and docs carry none (private code); this workspace is `MIT OR Apache-2.0` | Owner |

## P6 issue triage (proposal for call M3-D, 2026-11-25)

**A proposal for Ryan's call M3-D, not acted on.** Nothing in assay has been transferred, closed, labelled or commented on. Read from `gh issue list -R Wet-Ink-Corporation/assay --state open` on 2026-11-25: **94 open issues**, the same count as on 2026-11-08, with follow-ups spot-checked against assay `main` at `eeb2a45`.

The rules it applies: open issues that are still live transfer, closed ones stay where their PR links point (migration plan, Work sequence step 7 and the triage item "so only live ones transfer"); and, as at Move 2 ([move-2-bench.md](move-2-bench.md), P4), each transferred issue is relabelled from the suite manifest (`area/review` and `status/triage`, plus `source/review` for review follow-ups and `needs/human` for `owner-action`), and assay's own labels (`blocked`, `security-control`, `calibration`, `owner-action`, `canary`, `area:*` and the rest) are dropped rather than added. Paths in the bodies use the pre-#399 names (`crates/assay-<x>`), which become `crates/review/review-<x>` at the import; `/assay` commands and `assay:*` names follow M3-A.

- **Trackers.** assay's plan hierarchy is the v1 umbrella (#1), seven milestones (#2 to #8) and fourteen Features (#9 to #34), linked as sub-issues. The proposal keeps the six milestones whose exit criteria are unmet and closes the umbrella, M0 and the Features: a Feature holds no content of its own ("done when every sub-issue is closed"), and Review's Initiative here replaces the umbrella when the import PR opens ([README.md](README.md), open calls). Each open leaf is re-parented under its milestone after transfer, if the sub-issue link does not survive the transfer (not verified).
- **Done in place: none.** Spot-checks found several follow-ups partly done on `main` (#149, #152, #219, #240, #260, #339, and #118's trust-glob step), but none fully, so each transfers with its done items noted. Not every item of every follow-up was checked against the code.

| # | Title (short) | Proposal | Reason |
| --- | --- | --- | --- |
| 1 | assay v1: PR review orchestrator | close | Umbrella over M0 to M6; Review's Initiative replaces it, `references/docs/review.md` is the end state, and its live milestones transfer on their own |
| 2 | M0: spec sign-off and owner prerequisites | close | Spec signed off and H-decisions recorded (decision 0002; P1 to P3); its two open leaves, #38 and #41, transfer and re-parent under M1 |
| 3 | M1: walking skeleton, no LLM | transfer | Exit criteria unmet (required check, canaries #78, H-08 check #79); its "required on assay" criterion is retargeted at redkiln (#118, M3-E) |
| 4 | M2: headless runner, micro and lite tiers | transfer | A-10, A-12, A-15 and A-21 not measured (#89, #94) |
| 5 | M3: full fan-out, re-review, quota | transfer | QA-01 to QA-04, role ablation and the A-18 kill check not done (#100 to #105) |
| 6 | M4: Weigh-In integration | transfer | Required CP2 (#110) not built; now against Weigh-In 2.x here (code work step 2) |
| 7 | M5: evaluation and shadow mode | transfer | Corpus metrics and the 4-week shadow not run (#113 to #115) |
| 8 | M6: cutover | transfer | No repository requires the check yet (#116 to #118) |
| 9 | Spec sign-off (Feature) | close | Container; its last open leaf, #38, transfers |
| 10 | Platform and identity setup (Feature) | close | Container; Apps and environments set up against assay (P3); its last open leaf, #41, transfers |
| 16 | Workflows (Feature) | close | Container; its leaf #60 transfers |
| 17 | Gates (Feature) | close | Container; its leaf #67 transfers |
| 21 | Canaries and go-live on assay (Feature) | close | Container for a go-live on assay, which is archived; #78 to #80 transfer, #81 closes |
| 23 | Review job and output safety (Feature) | close | Container; #85 and #89 transfer |
| 24 | micro and lite tiers (Feature) | close | Container; #94 transfers |
| 26 | Re-review and feedback (Feature) | close | Container; #100 and #101 transfer |
| 27 | Quota deferral and requeue (Feature) | close | Container; #103 transfers |
| 28 | Calibration (Feature) | close | Container; #104 and #105 transfer |
| 30 | CP2 CI gate (Feature) | close | Container; #110 transfers |
| 32 | Reporting (Feature) | close | Container; #113 transfers |
| 33 | Shadow mode (Feature) | close | Container; #114 and #115 transfer |
| 34 | Cutover (Feature) | close | Container; #116 to #118 transfer |
| 38 | Genesis ADR: record the trusted first tree | transfer | Not written (no `docs/decisions/0001-*`); retarget at the redkiln commit where Review starts gating this repository (T16, M3-E) |
| 41 | Dedicated approver account (H-13) | transfer | Owner step deferred to go-live (decision 0005); needed before any repository requires the check |
| 60 | Pipeline workflow template | transfer | v1 story, partly built (ST-1 to ST-10, see #322 to #339), acceptance unticked; the template moves to `deploy/review/workflows/` |
| 67 | gates-test on a separate VM | transfer | v1 story, partly built per assay's decisions log; acceptance unticked |
| 78 | Run the M1 canary PR suite | transfer | Never run; retarget at redkiln, where Review first runs |
| 79 | H-08 behavioural check | transfer | Owner calibration, blocked; independent of the repository |
| 80 | Measure PR history and warm-target reuse | transfer | Calibration, blocked; measured over redkiln's history now |
| 81 | Make the verdict required on assay | close | assay is archived at the move, so the check is never enforced there; superseded by #118 and M3-E |
| 85 | Review job: environment, egress, context pack | transfer | v1 story, open; the wiring many follow-ups wait on |
| 89 | Egress-block canary (A-12) | transfer | Not run; it decides H-07's runner choice |
| 94 | Calibrate lite/micro cost and limits | transfer | Calibration, blocked on a live pipeline; later through `redkiln-calibrate` (code work step 5) |
| 100 | `/assay dismiss` and feedback events | transfer | v1 story, not built; the command name follows M3-A |
| 101 | `/assay retry` with cap ×1.5 | transfer | v1 story, not built; #287 lands first |
| 103 | Deferral state machine and requeue | transfer | v1 story, not built |
| 104 | Role ablation and model-tier split | transfer | Calibration, blocked |
| 105 | Full-tier calibration, 7 days | transfer | Calibration, blocked |
| 110 | Required CP2: blocking set, owner ack | transfer | Partly built (`weigh-in-ack` parsed in `review-app/src/command.rs`), the required mode is not; now against `weighin.ci-check/2` (code work step 2) |
| 113 | `assay report` and weekly workflow | transfer | v1 story, not built |
| 114 | Shadow mode configuration | transfer | v1 story, not built |
| 115 | Shadow 4 weeks, blind labels | transfer | Calibration, blocked on #114 |
| 116 | Require the verdict on the first consumer; retire the SaaS | transfer | Owner cutover, blocked; if redkiln is the first consumer it overlaps #118, and the two can merge at transfer |
| 117 | Promote `llm_blockers` when QA-05 is met | transfer | Owner, blocked on labelled data |
| 118 | Roll out to redkiln (H-03) | transfer | About this repository; its trust-glob step is done in place by #399 (`crates/review/**`), the cutover is M3-E |
| 149 | #148 follow-ups: large prompt, leading dash, `NODE_` | transfer | Large-prompt test and `NODE_` reservation done; the `LeadingDash` call is open |
| 152 | #62 follow-ups: conclusion types, exec paths, env names | transfer | F2, F3, F5, F6 done (#243) and F7 done (allowlist by name, `review-gates/src/environment.rs`); F1, F4, F8, F10 open |
| 160 | Prompt bundle follow-ups (#90) | transfer | Length-prefix helper and test nits open |
| 163 | Tiering follow-ups (#50) | transfer | Symlink-as-code fix open, and the owner's `.gitmodules` trust-path question (relevant to M3-E's globs) |
| 178 | exec-surface round-5 follow-ups | transfer | Stale docs and a CLI conversion, open |
| 183 | #55 plan follow-ups | transfer | Tests and the GitShort known limit, open |
| 185 | #92 degradation follow-ups | transfer | Empty-reason schema break open; Z4 and publish items wait on #85 |
| 195 | #121 clean-init follow-ups | transfer | Owner's monthly canary workflow and the mod-canary scope, open |
| 196 | #69 warm-target follow-ups | transfer | First item done; digest pin and reuse proof open; `warm-target.yml` is rewritten here |
| 202 | #83 preflight follow-ups | transfer | Error-chain and test items open |
| 203 | #91 blind verifier follow-ups | transfer | Two owner calls (LEFT-side findings, retry spend) and minors, open |
| 204 | #88 sanitiser follow-ups | transfer | Open; F5 lands before the comments adapter |
| 206 | #87 egress scan follow-ups | transfer | Generator fix and a circular property test, open |
| 209 | #74 decision replay follow-ups | transfer | Memory peak, replay wiring and provenance, open |
| 212 | #97 full-tier verifier follow-ups | transfer | Charter wording, an untested budget path, an owner confirmation |
| 217 | #95 fan-out follow-ups | transfer | Retry scheduling (H-13) and an owner default, open |
| 219 | #107 `check --local` follow-ups | transfer | R1 done (`is_file` check in `review-forge`); R2, F7, F11, F12 and the owner defaults open |
| 223 | #96 coordinator follow-ups | transfer | Tie-break ranking (OUT-05) and owner defaults, open |
| 224 | #99 re-review follow-ups | transfer | Owner bundle call decided 2026-10-14; R2-1 to R2-3 and the other defaults open |
| 227 | #98 sharding follow-ups | transfer | Wording, property and wiring items and owner defaults, open |
| 240 | #229 follow-ups: `--no-tests=fail` | transfer | The `--` case done (`own_args`, with a test); the option-value case and the exec-gate observation remain |
| 248 | #152 cause-chain follow-ups | transfer | Doc contract, clipping jsonschema text, nits, open |
| 250 | #184 follow-ups: test names, mod lookups | transfer | The "quickly" test names remain; the cost note and an H-16 option open |
| 252 | #153/#156 reserved names follow-ups | transfer | Test, API and charter items, open |
| 253 | #173 caller-role follow-ups | transfer | Test and charter items, open |
| 254 | #219 hardening follow-ups | transfer | FIFO open mode and untested arms, open |
| 257 | #189 shared sanitisers follow-ups | transfer | Stale [`plain`] links still in `review-gates`, open |
| 260 | #186 linear tiering follow-ups | transfer | F1 done (`index.everything = true`); F2, F3 and two cost notes open |
| 269 | Design: hardware-key-signed approvals | transfer | Owner design open; decides the approver (with #41) before any repository requires the check |
| 279 | #108 follow-ups: pushed refs, weighin stderr | transfer | Pre-push items; the hook moves to `tools/review/pre-push` |
| 282 | #58 follow-ups: ledger read side, comment adapter | transfer | Not built |
| 283 | #70 follow-ups: ledger goldens, CLI wiring | transfer | Open; its live canary names assay-ledger (touches M3-B) |
| 285 | #75 follow-ups: composite pins, naming | transfer | Owner call on composite actions (they move to `deploy/review/actions/`), open |
| 287 | Enforce one retry per head and role | transfer | Not built; must land before `/assay retry` (#101) |
| 292 | Waiver carry: narrow the binding? | transfer | Owner question unanswered |
| 294 | #273 follow-ups: `GhEnvironment` redaction tests | transfer | Tests not added |
| 298 | Unify `SnapshotCommand` with `ledger-command/1` | transfer | Two models still exist (`review-app/src/eligibility/commands.rs`); a contract call (H-16) |
| 322 | #60 ST-1 follow-ups: one number parser | transfer | `canonical_positive` still in both `review-forge` and `review-cli` |
| 326 | #60 ST-3a follow-ups | transfer | Tests and a neutral base64 module, open |
| 328 | #60 ST-6 follow-ups | transfer | `FIELDLESS` still hand-kept; the owner's Z2b residual risk is decided before the template lands |
| 339 | #60 ST-5 and ST-10 minors | transfer | ST-5 F2 (`plan_job.rs` split) and ST-10 F1 (`ledger_job` module) done; the `out_dir` test, charter items and an owner wording call open |
| 342 | Closed type for `ReviewedFinding.role` | transfer | Still a `String` (`review-domain/src/decide/inputs.rs`); a run-inputs contract call |
| 373 | A draft run's success stays on the verdict | transfer | Owner call not recorded; a fail-open gap |
| 374 | fetch-by-digest tar-layer mode for temper | close | Superseded: temper's GHCR artifact (temper #193) was never built and wi-39f19a closed it; Review pins the temper engine in this repository (code work step 4) |
| 380 | gates: crate roots from explicit target paths | transfer | Known gap in `review-gates`' layout, open |
| 381 | runner: probe setpriv's `--pdeathsig` | transfer | The candidate is still never run (`parent_death.rs`) |
| 382 | runner tests: ETXTBSY race | transfer | Both stand-ins still use `fs::copy`; the fix is known |
| 386 | Template tests: pin every step key | transfer | TST-05 gap open; the tests retarget with the templates |
| 389 | forge: make the optional DTO keys required | transfer | `repo: Option<..>` keys still read a missing key as `None` |

**Proposed: 76 transfer, 18 close, 0 done in place (94 in all).** The transfers are 6 milestones, 22 v1 stories, calibration and owner items, and 48 review follow-ups and design questions; the closes are the umbrella, M0, the 14 Features, #81 and #374.

## Done when

- The six crates and the plugin pass this repository's gate, `cargo xtask layers` and `cargo xtask plugins`, with the import's history intact.
- The prompt bundle hashes and the golden decisions are byte-identical after the move (`self-gate`, run as `assay` or `review` depending on M3-A).
- assay is archived with its redirect, its issues transferred. assay-ledger stays.
