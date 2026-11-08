# Move 3 runbook: Review (draft)

assay moves from `Wet-Ink-Corporation/assay` into this repository as Review: the `rk-review` plugin and the CI deployment under `deploy/review/`. assay finished its move preparation on 2026-11-07 and has been frozen by Ryan since 2026-11-08: no work happens there until this move starts.

This runbook is a **draft**, written early while assay's state is fresh. Move 2 (Bench) comes first, so the facts below are re-checked when Move 3 is next, and the calls marked "at Move 3" are put to Ryan then. It does not re-plan: the end state is `references/docs/review.md`, the formats are `spec/`, the rules are `AGENTS.md`, and the template is [move-1-weigh-in.md](move-1-weigh-in.md).

Facts below were checked on 2026-11-08 against assay `main` at `eeb2a45`, assay-ledger `main` at `3ae05eb` and redkiln `main` at `d80e7fb`.

## Preconditions

| # | Precondition | State | Evidence |
| --- | --- | --- | --- |
| P1 | The in-place rename to `review` (RV call A, the migration plan's gate) | **Met** | assay #399, merged by Ryan 2026-11-07: crates `review-<x>`, `REVIEW_*` env vars and secret references, schema ids `review.*/1`, checks `review / verdict` and `review / gate: <name>`, App names `redkiln-review` and `redkiln-review-ledger` in the docs. Golden `inputs_digest` values regenerated (owner-approved trust-path change) |
| P2 | Trust path and architecture test narrowed to Review's crates | **Met** | assay #399: trust globs `crates/review-*/**` and `crates/review/**`; `crates/review-cli/tests/it/architecture.rs` checks members under either root or named `review-*`, and finds the workspace root through cargo |
| P3 | The owner steps of the rename sitting | **Met** | Ruleset `25356300` requires `review / verdict` from integration `5371952` (read 2026-11-08; enforcement `evaluate`). `main` is the default branch. Ryan confirmed on 2026-11-08 that both Apps were renamed (`redkiln-review`, `redkiln-review-ledger`) and that assay's environment secrets and variables were renamed |
| P4 | Weigh-In adapter on `weighin ci-check --json` | **Met** | assay #372 (`crates/review-gates/src/weigh_in.rs`): runs the pinned `weighin ci-check --json --base --dir --store` and reads `weighin.ci-check/1` strictly; the 1.x file reader is gone. Two gaps remain for Weigh-In 2.x, listed under Code work |
| P5 | No open PR, nothing half-finished | Met for PRs | 0 open PRs. Five stale branches (`claude/assay-229-gates-test-followups`, `claude/project-thread-aezcw7`, `-jm46k7`, `-ncav8t`, `spec/architecture`), none imported (the import takes `main`'s tip only); Ryan deletes or keeps them |
| P6 | Open issues triaged into transfer or close | **Open** | 94 open issues: the plan's milestones and epics (#1 to #34), v1 stories still open (#60, #67, #78, #85, #100, #101, #103, #110, #113, #114; parts of #60 and #67 are built per assay's decisions log), calibration and go-live items blocked on Ryan (#41, #79 to #81, #94, #104, #105, #115 to #117) and about 45 review follow-ups. Triage happens at the start of Move 3, since assay is frozen |
| P7 | Core 1.0 and SPEC 1.0 | Waits for Move 2 | Review builds on Core's envelope, trust levels and approvals (`references/docs/review.md`, What Review consumes) |
| P8 | The kits Review is meant to use | Waits for Move 2 | `redkiln-github` and `redkiln-calibrate` are cut from anneal at Move 2. Review imports on its own code and converges onto the kits afterwards, as Weigh-In converged onto Core |

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
| `corpus/` (17 MB) | `evals/review/corpus/` | Eval cases and goldens, never shipped |
| `docs/` | `docs/review/` | Architecture 01 to 07 (the ADRs are in `07-decisions-assumptions.md`), decision records 0002 to 0007 (`docs/decisions/`) and the reviews keep their paths below it |
| Root `AGENTS.md`, `CLAUDE.md` | `docs/review/charter.md`, `docs/review/working-rules.md` | The decisions log (about 70 rows) moves with the charter |
| `.kb/_intake/decisions/` (28 atoms), `.kb/_ledger/journal/` (3 journals), `.kb/_ledger/defaults-digest-2026-10-12.md` | The same paths | Atoms keep their ids and move to `.kb/decision/` with the rest (Knowledge call B); journals are imported once by Weigh-In's legacy reader (Move 1 code work, step 8) |
| `.assay/` (`globs.toml`, `owners`, `agents`) | `.review/` at the door | Review's own config for gating this repository. `.review/` is the suite's one exception to `.redkiln/<plugin>/`, because Review reads it from the base branch (`references/docs/review.md`, Events and state; Review call A). Its trust globs name assay's paths and must be rewritten for this layout, a trust-path change for Ryan (05 T16) |
| `.github/workflows/` (`rust.yml`, `release.yml`, `warm-target.yml`), `.github/dependabot.yml` | Not placed | Jobs rewritten into `ci.yml` under `ok`; release and warm target into this repository's release flow |
| `.claude/` (temper 1.3.0 copy, `temper.json`, rules), `Cargo.*`, toolchain, `clippy.toml`, `rustfmt.toml`, `deny.toml`, `.cargo/`, `.githooks/` | Not placed | The root's copies govern; diff each in the import PR (below) |

## What the root must provide

Diff in the import PR; anything missing is an H-16 or H-05 call, not a silent merge.

| assay has | redkiln at `d80e7fb` | Call |
| --- | --- | --- |
| `toml` 1.1 (`std`, `parse`, `serde`) | `toml` 0.9.12 | Settled: the root steps up to 1.1 before Move 2 (Ryan, 2026-11-08) |
| `sha2` 0.11 (no default features) | `sha2` 0.10 | Settled: the root steps up to 0.11 before Move 2 (default features stay the root's) |
| `jsonschema` 0.58 as a **normal** dependency of `review-app`, with `deny.toml`'s `MIT-0` (borrow-or-share) and `ahash`/`wasm-bindgen` wrapper exceptions | Move 1's code work step 3 (conformance fixtures) is to bring `jsonschema` 0.58 as a dev-dependency with the same `deny.toml` exceptions; Ryan approved it in that step's thread on 2026-11-08, and its `docs/decisions.md` row lands with the step | Normal-dependency use is a new H-16 row |
| `proptest` 1.12, `syn` 2 (`parsing`, dev) | Absent | H-16 rows (both are assay pre-approvals, not redkiln's) |
| `rust-version` 1.96 | 1.98.1 | Already decided: products take the suite's one pin (Blueprint call 5, signed off 2026-10-18). Clippy may raise new MSRV-gated lints, fixed in the import PR |
| No licence field: private code (assay decisions log, 2026-10-11) | Every member inherits `license = "MIT OR Apache-2.0"` | Importing relicenses the crates and docs: Ryan's call at Move 3 (M3-F) |
| No baseline (as strict as `-D warnings`) under temper 1.3.0 | temper 1.4.0 at the root (or later, if [rust-2.md](rust-2.md) R-4 lands first) | Expect no re-baseline; any finding the root's temper adds is fixed or baselined at the new paths |

## Code work, in order (sketch)

1. **Import** (placement merge, then the door rename). The crates move one level deeper and the data moves, so every path to repository data is retargeted. In production code these are `include_*!` paths: schemas (`review-app/src/contract.rs`), prompts (`review-cli/src/bundle.rs`) and the rubric (`review-cli/src/check.rs`), each embedded byte for byte. In tests about 34 files reach data through `../..` and `CARGO_MANIFEST_DIR` joins (schemas, corpus, rubric, prompts, `.assay/`, fixtures in other crates); retargeting them is a test change to list for Ryan (H-12), though no assertion changes. The architecture test gains the seventh crate when `review-contract` lands (H-16 test change).
2. **Weigh-In 2.x gaps** (with redkiln #48, wi-e41ab9). `gates-static` fetches `refs/redkiln/journal/*` beside the pinned base and head, and the findings stop anchoring journal violations at `.kb/_ledger/journal` (`crates/review-gates/src/weigh_in/findings.rs`). Built against whatever `ci-check` version #48 ships; a `weighin.ci-check/2` is read here first.
3. **`review-contract`** at `crates/review/review-contract/`: `review.findings/1` with its fields unchanged (Review call B), depending only on Core. Rust, Bench and Loop emit it.
4. **The pinned temper.** `gates-static` and `gates-lint` take `--temper` only from an `oci:sha256:` pin (D8 A), and temper's GHCR artifact (temper #193) was never built; temper's own triage says Review pins temper inside the monorepo instead (wi-39f19a, open). Since temper is imported before this move (Ryan, 2026-11-08), Review can pin the engine already in this repository; settled with wi-39f19a at that import ([rust-2.md](rust-2.md)).
5. **Converge onto Core and the kits**: events on Core's envelope, capacity leases on `redkiln-capacity`, GitHub through `redkiln-github`, calibration through `redkiln-calibrate`. Each is its own PR after the import is green.
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

## Done when

- The six crates and the plugin pass this repository's gate, `cargo xtask layers` and `cargo xtask plugins`, with the import's history intact.
- The prompt bundle hashes and the golden decisions are byte-identical after the move (`self-gate`, run as `assay` or `review` depending on M3-A).
- assay is archived with its redirect, its issues transferred. assay-ledger stays.
