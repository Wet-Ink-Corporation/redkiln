# redkiln

redkiln is one monorepo for the Redkiln suite: separately installable Claude Code plugins (`rk-*`) over three shared layers, Core, kits and contract crates, in one Cargo workspace. The end state is `references/docs/blueprint.md`; the order things arrive in is `references/docs/migration-plan.md`. This file is the workspace charter temper reads, and the rules here are checked, not only written.

## Layout

| Path | Holds | Arrives |
|---|---|---|
| `Cargo.toml`, `rust-toolchain.toml`, `clippy.toml`, `rustfmt.toml`, `deny.toml`, `.cargo/` | One workspace, toolchain 1.98.1, temper 1.4.0's lint and supply-chain policy | phase 0 |
| `tools/xtask/` | `cargo xtask <task>`: repository tasks, starting with `layers` (the structural test below) | phase 0 |
| `tools/launcher/` | The shared plugin launcher (sh and `.cmd`) and its tests | phase 0 |
| `tools/labels/`, `.github/labels.yml` | The suite label manifest (Backlog call A, wi-cef669), its check and an idempotent apply script | phase 0 |
| `spec/` | Core SPEC, schemas, conformance fixtures | SPEC v1 draft, phase 0 |
| `crates/core/` | `redkiln-core`, `redkiln-calibrate`, `redkiln-capacity` | move 1 onward |
| `crates/kits/` | `redkiln-guard`, `-dispatch`, `-github`, `-craft`, `-catalog`, `-view` | moves 2 to 4, train 1 |
| `crates/<plugin>/` | A plugin's crates, its `<name>-contract` crate among them: `rust`, `bench`, `weigh-in`, `knowledge`, `review`, `loop`, `backlog` | with each move |
| `plugins/<plugin>/` | Manifest, skills, agents, hooks, `libexec/` launcher copies and builds; `plugins/redkiln/` is the bundle | with each move |
| `deploy/review/` | Review's workflow templates, App manifest and fetch-by-digest action | move 3 |
| `docs/<plugin>/`, `.kb/` | Imported records (series kept) and the one knowledge base | with each move |
| `references/` | The frozen planning handover | done |

## Crate map

| Crate | Layer | Purpose | Depends on |
|---|---|---|---|
| `tools/xtask` | tool | `cargo xtask layers` and later packaging, version and changelog tasks; never shipped | serde, serde_json, thiserror |

Each crate's `AGENTS.md` is its charter (ARC-08). The first crate of a layer also adds its `crates/<layer>/*` glob to the workspace members (cargo refuses a glob that matches nothing); tools are listed one by one, since `tools/` also holds scripts.

## Layer rules

`cargo xtask layers` (`tools/xtask/src/layers.rs`) reads `cargo metadata` and fails on any of these; CI runs it, and its tests pin each rule.

- **Placement.** Every member sits directly under `crates/core/`, `crates/kits/`, `crates/<plugin>/` for a listed plugin, or `tools/` (`crates/core/<crate>`, never deeper). A crate under `crates/<plugin>/` named `*-contract` is that plugin's contract crate. A new plugin folder is a crate-boundary change (H-16).
- **Core** depends on nothing outside Core.
- **Kits and contract crates** depend only on Core. A contract crate never reaches into its own plugin's internals.
- **A plugin's crates** depend on Core, kits, any plugin's contract crate and their own plugin's crates, never another plugin's internals. Another plugin's domain is reached through its contract crate, its events or its `--json` CLI (blueprint design rule 4).
- **Tools** depend on Core, kits and contract crates.
- Dev and build dependencies follow the same rules. Only crates with a binary target use `anyhow` outside their dev-dependencies (H-09), and no crate is publishable (`publish = false`; turning it on is H-16).

Beside the check, CI builds every crate alone (`cargo check -p`). Loop brings its stricter rules (no tokio in the kernel, git through one wrapper, no egress in kernel tests) when it moves, and they join the check then.

## Plugin rules

- **No `plugins/<plugin>/bin/`.** claude.ai rejects hosted plugins that ship one. Builds live in `libexec/<target>/`, behind copies of the shared launcher named after the binary (`tools/launcher/README.md`), and hooks call `${CLAUDE_PLUGIN_ROOT}/libexec/<binary>` by path in exec form. CI checks both.
- **Hooks fail open, guards fail closed, no hook waits on a sibling** (blueprint design rule 7). With no build at all, the launcher lets every hook through and says so at SessionStart.
- **Versions and tags are per package**: tags `rk-<plugin>--v<version>` (protected by the `release-tags` ruleset), each package with its own `CHANGELOG.md`.
- **Line endings are LF** everywhere except `*.cmd` (`.gitattributes`).

## Moves

Every import follows the migration plan's eight steps: one merge commit whose second parent is the old repository's unchanged tip (`git read-tree --prefix`, never squash, rebase or `filter-repo`), the rename at the door in a second commit, records keeping their series, old state imported once and never deleted, issues transferred, and the old repository archived with a redirect. `main` allows merge commits only, which is what keeps imported hashes resolvable.

## Commands

| Purpose | Command |
|---|---|
| Format | `cargo fmt --all` |
| Lint | `node .claude/temper/temper.mjs baseline --check`: clippy, rustdoc and temper's scan. With no baseline it is as strict as `-D warnings`; each move re-baselines its legacy findings at their new paths (migration call E) |
| Layer rules | `cargo xtask layers` |
| Test | `cargo nextest run --workspace` (or `cargo test --workspace`), plus `cargo test --doc --workspace` |
| Docs | `cargo doc --workspace --no-deps` (rustdoc lints are part of Lint) |
| Supply chain | `cargo deny check` |
| Launcher | `sh tools/launcher/test.sh`; on Windows `pwsh tools/launcher/test.ps1` |
| Labels | `python3 tools/labels/check.py` (needs PyYAML); `tools/labels/apply.sh <owner/repo>` applies them (`DRY_RUN=1` to preview) |
| CI-only checks | `cargo check -p <crate>` for each crate (ARC-16), `cargo hack check --workspace --feature-powerset` (API-16), `cargo machete` (CRF-05) |
| Full gate (Claude Code) | `/temper:gate`, or `node "$TEMPER_CLI" gate` |

CI is `.github/workflows/ci.yml` on GitHub-hosted runners: a `changes` job decides which jobs run from the paths a change touches (rust, Windows tests, launcher on three platforms, plugins, TypeScript, labels), and the `ok` job passes only when every job that ran passed. `ok` (source GitHub Actions; the PR list shows it as `ci / ok`) is the one check to require on `main`. The TypeScript job starts running once a `package.json` exists (the suite mod, wi-6e750e), and the plugins job once a plugin manifest does.

Toolchain 1.98.1 (`rust-toolchain.toml`). MSRV 1.98.1 (`rust-version`), equal to the pin as in weigh-in and anneal, raised only as an H-16 decision (MOD-01). Profiles are temper's template (PERF-09). Until Redkiln Rust 2.0 dogfoods, the gate is temper 1.4.0 vendored once at `.claude/temper/` (migration call E).

## Engineering standard (rust-craft)

All Rust here follows rust-craft. The full text, with rule IDs, examples, the review rubric and the process, is the `rust-craft` skill shipped in the temper plugin. The non-negotiables for production code (tests may use `unwrap`/`expect`/`assert!`):

- H-01 no `unsafe`; H-02 no `unwrap`/`expect`; H-03 no `panic!`/`todo!`/`unimplemented!`/`unreachable!`, and no std API that panics on input the code does not control; H-04 no panicking indexing or slicing, including `map[&k]` and `deque[i]`, which no lint catches.
- H-05 no `#[allow]`; `#[expect(lint, reason)]` needs human approval; never weaken lint, CI or hook config.
- H-06 no `dbg!`, no print macros (use `tracing` or `writeln!` on a locked handle), no commented-out code, no untracked TODOs.
- H-07 no `Box::leak`/`mem::forget`/`transmute`/`static mut`/`'static` to silence lifetimes; H-08 no clones to appease the borrow checker.
- H-09 typed errors in library APIs (`thiserror`), with `anyhow` only in binaries; H-10 never discard failures; H-11 no lossy `as` casts.
- H-12 never weaken tests or checks, including manifest test switches (`test`, `doctest`, `harness`, `autotests`) and `#[ignore]` or `cfg_attr(.., ignore)` without a reason; H-13 bounded and owned resources.
- H-14 no blocking in async code, and no lock guard, `RefCell` borrow, entered tracing span or `watch::Ref` held across `.await`; H-15 zero warnings, documented public API.
- H-16 new dependencies and major-version bumps, `[patch]` overrides, `rust-version` raises, API breaks, lint exceptions, crate-boundary changes, migrations and test rewrites are human decisions.

Process: explore, frame, plan (`.temper/plans/`), red, green, refactor, self-review, gate, independent review, then report with a Status block (`DONE`, `WIP` or `BLOCKED`, plus the gate result, the review verdict, what changed, decisions needed and what was not verified).


Pre-approved dependencies are listed in `.claude/temper.json`; any other is an H-16 decision.

## Decisions log

| Date | Decision | Rule | Decided by |
|---|---|---|---|
| 2026-10-19 | Agents may merge here under temper's wi-880b66 bar: every check green, no conflict, no open thread, a fresh-context approval; never release or `[hold]` PRs | process | Ryan (wi-9f5c91) |
| 2026-10-19 | `main` ruleset: pull request required, 0 approvals, threads resolved, merge commits only, Ryan bypasses for pull requests only | process | Ryan (wi-e3e5af) |
| 2026-10-19 | One suite-wide label set, shipped as `.github/labels.yml` until Backlog exists | Backlog A | Ryan (wi-cef669) |
| 2026-10-19 | TypeScript and the `claude` CLI are allowed in CI, for the suite mod | H-16 | Ryan (wi-6e750e) |
| 2026-10-30 | Phase 0 skeleton: temper 1.4.0's setup at the root (template lints, profiles, `deny.toml`, `.claude/`), MSRV 1.98.1, license `MIT OR Apache-2.0` as in weigh-in and anneal, one dev crate `tools/xtask` holding the layer check with `serde`, `serde_json` and `thiserror` (all pre-approved), and Core crates allowed to use other Core crates | ARC-02, H-16 | phase 0 PR, Ryan to confirm |
