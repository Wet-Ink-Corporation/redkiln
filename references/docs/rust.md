<!-- Exported from the Claude Doc https://claude.ai/code/artifact/eb6f6d72-e605-40f9-b1c2-7126923e4353 (revision 32) on 2026-10-19 as part of the project handover. The Doc is the original; this copy is frozen. Diagrams (1) could not be exported and show as [embedded content] placeholders; open the Doc to see them. -->

# Redkiln Rust: what the plugin should entail

Oct 18, 2026 · @Ryan

## Summary

Redkiln Rust should be temper with its engine split in two: the rust-craft standard and Rust policy stay in the plugin, and the language-neutral half (about 3,500 of temper's 7,947 engine lines) moves to the redkiln-craft and redkiln-guard kits. That split is what lets Redkiln TypeScript and Python follow without a second rewrite, so it belongs in the 2.0 port, not after it.

- **The standard.** 182 rules with permanent IDs, 96 mechanically enforced, plus the review rubric. It is the product; everything else delivers it. The rule registry and rubric move unchanged.
- **The port.** temper's planned Node-to-Rust port happens in the monorepo as Redkiln Rust 2.0. It keeps the parity discipline (byte-identical fingerprint, the 365 Node tests as oracle) but reopens D1, D2, D3 and D10, as Blueprint call 5 proposes.
- **The engine split.** Completion gate, verdict validation, fingerprint, baseline ratchet and gate cache go to redkiln-craft; the 1,090-line shell reader and protected globs go to redkiln-guard. The scanner, lexer, manifest rules and cargo steps stay here.
- **One CI binary, no vendored copies.** The four vendored `.claude/temper/` copies are retired. CI runs a pinned binary, and Review calls its subcommands by digest instead of re-implementing them.
- **Facts as events.** Gate runs, verdicts, denials and telemetry become `rust.*` events in the shared log; the telemetry branch becomes a journal; the reviewer's findings adopt review-contract's schema.
- **Calls.** Six, in the last section. B (crate names and whether to publish them) is the one that is hard to undo.

## What the repos already do

temper is a working v1 candidate, and three siblings already carry a second copy of part of it. Redkiln Rust should start from temper as it stands and pull the duplicates in, not design afresh.

| Repo | What it does today | Worth lifting |
| --- | --- | --- |
| temper (main, 1.3.0) | The rust-craft standard: 182 rules in `skills/rust-craft/references/rules.md`, 96 with a mechanical enforcer. Five commands, three subagents, 9 hook handlers, a 7,947-line Node engine with no dependencies, 365 Node tests, a 2,757-citation registry check and 329 compiled doc examples. Baseline ratchet, report, `status --json` and `stats --json` (#116), a telemetry branch (wi-641f99). Ships as its own marketplace. 1.4.0 is v1, cut after the pilot report due 2026-10-18 (wi-891617, wi-292a7d). | All of it, as the plugin and the 2.0 port's oracle |
| temper port plan | `docs/plans/temper-2-rust-port.md`: 920 lines, D1 to D17 decided, about 14,500 lines of production Rust in four crates. No Phase 0a code yet. | The parity method, test list, invariants and divergence list (D16). Not the npm route, crate names or layering (see the port section) |
| anneal | A second guard engine: `guard/` (3.8k lines), `redact/` (1.5k), `glob.rs` (234, case-insensitive), `floor_guard.rs` (1,600: suppressions, deleted tests, loosened thresholds), `baseline.rs` (570), `verdict.rs` (745, bound to `git patch-id`). | The redaction and floor-guard rules into redkiln-guard; patch-id binding into Core's approval record |
| assay | `assay-gates/src/temper*` re-implements temper checks for PR review in 9,466 lines of production code (src/temper.rs and src/temper/, 29 files; its temper tests add 5,721 more): baseline raise, exception scan, gate-config diff, test inventory, manifest switches, vendored-copy drift. It runs the base's pinned `temper scan --all --json` and never links temper (P8). | The checks belong in Redkiln Rust as subcommands; Review keeps a thin adapter. The "blind spot" list (cfg\_attr, `#[path]`, `include!`) as scanner tests |
| redkiln-rs (build/phase-0-to-1) | A pilot adopter with a 6,606-finding baseline never tightened (#63). Its own lint and dependency policy: ADR-0083 (toolchain and lints), ADR-0084 and ADR-0100 (dependencies, cargo-deny), enforced by lint-control fixtures under `crates/*/tests/fixtures/lint-controls/`. | ADR-0083, 0084 and 0100 as rule proposals or a repository profile, so Loop stops owning Rust policy |
| weigh-in | Pilot adopter: baseline remediated from 721 findings to 0 with three approved `#[expect]` exceptions (AGENTS.md decisions log). Carries the vendored copy and charter text verbatim. | The pilot's evidence for 1.4.0, and the charter template as it reads in practice |

Two facts shape the design. The language-neutral half of temper already has a second implementation in anneal. And Review's largest temper dependency is a re-implementation, which a stable Rust CLI contract would remove.

## What it is and what it owns

Redkiln Rust owns one domain: what good Rust is, and the Rust-specific checks that prove it. It refuses bad code before it is written and blocks a turn until the gate is green and a fresh reviewer approves, exactly as temper does today.

### The three layers inside temper today

temper's engine mixes a generic enforcement engine with Rust policy. The split below follows that seam, so a later language plugin reuses the engine untouched.

| Layer | What it is (today's source, lines) | Lands in |
| --- | --- | --- |
| The standard | `skills/rust-craft/` (SKILL.md plus 15 references, about 14k lines of Markdown), the rule registry, `review-rubric.md`, the three agents, `templates/workspace/` and the example workspace | Redkiln Rust, unchanged |
| Rust policy | Scanner and lexer (`scan.mjs` 871, `lexer.mjs` 294, 56 rule keys), manifest policy (`manifest.mjs`, `toml.mjs`), cargo steps, cargo-message parser and rule map (`findings.mjs` 569), Rust pathspecs for the fingerprint, the 12 rubric dimensions | Redkiln Rust's own crates |
| Generic engine | Completion gate and Stop/SubagentStop choreography, verdict validation and storage (`verdict.mjs` 120), fingerprint, gate runner and per-fingerprint cache, baseline ratchet algorithm (`baseline.mjs` 504), report shell | redkiln-craft kit |
| Shell reading | `shell.mjs` (1,090): what a command writes, deletes or runs; protected-config globs | redkiln-guard kit, merged with anneal's `guard/`, `redact/` and `glob.rs` |
| Plumbing | Store location, session state files, telemetry, vendoring (`vendor.mjs`) | Core (store, journals); vendoring is deleted |

The port plan's D3 cuts horizontally (domain, app, host, cli), which puts the Rust scanner and the generic verdict in one `temper-domain` crate. The vertical cut above is the one that pays off when TypeScript arrives (audit section 7).

### Owns, and does not own

- **Owns:** the rule registry and its permanent IDs; the Rust rule packs that redkiln-guard runs; the Rust dimensions of the review rubric; the Rust checks Review needs (test inventory, manifest test switches, lint weakening, baseline raise); the workspace templates and `/temper:setup`'s successor; the `rust.*` event types.
- **Does not own:** the generic gate and verdict engine (redkiln-craft), the shell lexer (redkiln-guard), PR verdicts and the findings schema (Review, through review-contract), decisions about lint exceptions (Weigh-In), the stored atoms that record them (Knowledge).
- **Takes in:** redkiln-rs's ADR-0083, 0084 and 0100 lint and dependency policy, which the Blueprint hands to Rust, as rules or a repository profile.

## The surface: commands, agents, hooks and the CLI

The in-session surface carries over almost unchanged; what changes is that the CLI becomes a public, versioned contract that CI and Review call by digest. Command names follow Blueprint call 9 (install ID `rk-rust`); the names below assume it.

### In session

| Surface | Today (temper 1.x) | In Redkiln Rust |
| --- | --- | --- |
| Standard skill | `temper:rust-craft` | `rk-rust:rust-craft`. Keeping the bare name `rust-craft` matters, since adopters' charters cite it and Cowork lists skills by their own name |
| Commands | `/temper:setup`, `implement`, `review`, `gate`, `audit` | The same five, written `/rk-rust:setup`, `/rk-rust:implement`, `/rk-rust:review`, `/rk-rust:gate` and `/rk-rust:audit`. `setup` also migrates 1.x repositories (D15's hash-guarded rewrite) |
| Subagents | `rust-architect`, `rust-implementer`, `rust-reviewer` | Unchanged in role. The reviewer's verdict carries review-contract findings (next section) |
| Edit guard | PreToolUse on Write and Edit: skill-load gate, escape-hatch scan, manifest policy, protected config | Unchanged; the protected-glob matcher comes from redkiln-guard |
| Bash guard | PreToolUse on Bash over `shell.mjs` | redkiln-guard's engine with Rust's rule pack (cargo add, `--no-verify`, lockfile tricks). Bench runs the same engine with its own pack, so both agree on what a command does |
| Completion gate | Stop: gate green, approving verdict for the current fingerprint, `Status: WIP/BLOCKED` honoured, one-block loop breaker | Unchanged in behaviour, run by redkiln-craft. It caps its own blocks well under Claude Code's shared limit of eight and reads `stop_hook_active` (pressure test) |
| Status grammar | `Status:` plus `Gate:`, `Review:`, `Changed:`, `Decisions needed:`, `Declined:`, `Not verified:` (PROC-10) | Becomes Core's end-of-turn grammar, which every plugin accepts. The Stop gate releases on DONE (gate green and an approving verdict for the current fingerprint), WIP and BLOCKED. Core's DONE\_WITH\_CONCERNS counts as DONE, so it needs the same gate and verdict, with the concerns listed. NEEDS\_CONTEXT releases as BLOCKED does, naming what is missing |
| Lens | Read-only mod, PRs #117 to #121 and #123 on `[hold]` | Folds into the suite mod over every plugin's `status --json` (visualization study) |

### The CLI as a contract

Today CI runs `node .claude/temper/temper.mjs baseline --check`, and assay runs the base's pinned `scan --all --json`. In the suite the same binary is fetched by digest, and every command Review needs exists as a subcommand with a versioned JSON result in Core's CLI envelope.

| Subcommand | Used by | Status |
| --- | --- | --- |
| `gate [--check] [--json]` | Stop hook, Bench's build and verify, Loop's verify stage | Exists |
| `scan [files or --all] --json` | Edit guard; Review's exception check | Exists |
| `baseline --check --findings-out FILE` | Adopters' CI; Review's lint zone | Exists |
| `report --format json,md,html` | CI scorecard | Exists |
| `status --json`, `stats --json` | Suite mod, suite status, CI | Exists (#116) |
| `check raised --base REV` | Review: baseline counts rising, new entries | New, lifted from assay `temper/baseline.rs` and `ratchet.rs` |
| `check exceptions --base REV` | Review: ask-level hits not admitted, multi-lint `#[expect]`, blind spots | New, from assay `temper/exceptions.rs`, `statics.rs`, `reach.rs` |
| `check gate-config --base REV` | Review: steering keys changed in config | New, from assay `temper/gate_config.rs` |
| `check tests --base REV` | Review: test inventory, removed tests, manifest test switches (H-12) | New, from assay `temper/inventory.rs`, `switches.rs` |

With those four `check` subcommands, Review's 9,466-line temper adapter (production code only) shrinks to a runner and a mapping into its findings. The vendored-copy drift check (`vendored.rs`) goes away with the vendored copies.

## Data, events and the 2.0 port

temper is the one plugin with no event store: it keeps plain JSON files under `<git-common-dir>/temper/` and a JSONL telemetry stream. In the suite, its facts become `rust.*` events in Core's shared log, and its caches become a disposable projection.

### What it writes

| Today | Becomes | Kind |
| --- | --- | --- |
| `temper/gate/<fp>.json`, gate logs | `rust.gate_run`: fingerprint, steps, result, duration; the cache is a projection | Durable fact |
| `temper/verdicts/<fp>.json` (`temper_verdict: 1`) | `rust.verdict`: decider agent, subject hash, verdict, review-contract findings, 12 rubric scores | Durable fact, trust level asserted (it comes from a subagent) |
| Scan denials and asks in `telemetry.jsonl` | `rust.scan_denied`, `rust.scan_asked`: rule ID, file, line | Telemetry, with a retention window |
| The other 22 telemetry names (`session-start`, `stop-blocked`, `gate-start` and so on) | `rust.*` telemetry events | Telemetry; leaves the machine only as aggregates |
| `temper-telemetry` branch and `refs/temper/telemetry-tip` (wi-641f99) | Core's journal export at Stop and pre-push | Retired; where journals live is pressure-test call A |
| Session files, skill markers | Per-session projection keyed on Core's normalised worktree id | Disposable |
| `.claude/temper.json`, `.claude/temper-baseline.json` | Kept in the repository, moved to `.redkiln/rust/`, the suite's config home for this plugin | Committed config |

Approvals bind to content as the pressure test recommends. A gate result binds to the change fingerprint over Rust's pathspecs, which Core implements once as its own named hash method, distinct from the tree hash. The algorithm stays bit-identical to 1.x (port invariant). The in-session reviewer's rust.verdict binds to the same fingerprint (session scope); only Review's review.verdict on a pull request binds to the patch-id plus base. A lint exception (`#[expect]`) asks Weigh-In, and the decision lands as a Knowledge atom, as the 43 atoms in temper's own `.kb/_intake/decisions/` already do.

### The port, decision by decision

The port plan was decided for a standalone repository. Moving it into the monorepo reopens nine of its 17 decisions and one invariant; the other eight carry over. Blueprint call 5 already names D1, D2, D3 and D10.

| Port decision | Decided | In the monorepo |
| --- | --- | --- |
| D1, D6, D13: npm per-platform packages, Node launcher, cargo fallback, user CI by `taiki-e/install-action` | wi-836680, wi-4abeef, wi-37c1df | Prebuilt binaries in `libexec/<target>/` behind the suite launcher, which temper's own research says reaches Cowork and org-synced installs; CI fetches the release asset by digest from its tag, spelled \<install-id>--v\<version> (for example rk-rust--v2.0.0). No npm org |
| D2: publish `temper-cli`, `-domain`, `-app`, `-host` on crates.io | wi-222f58 | Do not publish; the crates are linked into one binary. The reserved 0.0.0 names stay idle (call B) |
| D3: four horizontal crates plus xtask | wi-0b4dc2 | `rust-craft-domain`, `rust-craft-app`, `rust-craft-cli` over redkiln-craft and redkiln-guard; the suite's xtask replaces the plugin's |
| D10: only regex, toml, sha2, wait-timeout; no clap, no async | wi-b03273 | Add Core's dependencies. Core exposes a synchronous API with tokio hidden inside, so ASY-01's intent holds (call C) |
| D4: MSRV 1.96 | wi-5fc813 | 1.98.1, the monorepo's single pin, as the Blueprint sets for every crate. Raising MSRV from 1.96 is an H-16 human decision, covered by Blueprint call 5 |
| D14, D17: `.claude/temper-version`, `TEMPER_BIN` on PATH | owner | The pin moves to `.redkiln/suite.toml`; the env names follow the rename |
| Invariant 3: 2.0 reads 1.x state files, no migration | port plan section 2.3 | 2.0 imports `.git/temper/` once into the log and never deletes it (Cowork refuses deletes) |
| D5, D7, D8, D9, D11, D12, D15, D16 | various | Carry over unchanged |

Timing is fixed by the pilot. No hook or gate behaviour changes while it runs (wi-b63bd6); the pilot report is due 2026-10-18 and 1.4.0 ships after it. The port then starts as Phase 0a inside the monorepo, with the 365 Node tests and the 1.4.0 engine as the parity oracle.

## More languages: TypeScript and Python on redkiln-craft

Redkiln Rust is the first of a family, so its port should define what any language plugin plugs into redkiln-craft. Redkiln TypeScript (ts-craft) and Redkiln Python (py-craft) are named in the Blueprint's "Later" list and are not built now; the cost of getting the seam right is paid once, in the Rust port.

A language plugin supplies a policy pack; redkiln-craft supplies everything else.

| A language pack supplies | Rust (exists) | TypeScript (later) | Python (later) |
| --- | --- | --- | --- |
| A standard skill with permanent rule IDs | rust-craft, 182 rules | ts-craft | py-craft |
| Escape hatches the edit guard refuses | `unwrap`, `unsafe`, `#[allow]`, panics, lossy `as` | `any`, `@ts-ignore`, `eslint-disable`, non-null `!` | `# type: ignore`, `# noqa`, bare `except` |
| Manifest policy (asks for new dependencies, lint tables) | `Cargo.toml` | `package.json`, `tsconfig.json`, ESLint config | `pyproject.toml` |
| Gate steps | fmt, clippy, rustdoc, test, deny | tsc, ESLint or Biome, the test runner | Ruff, a type checker, pytest |
| A findings parser and rule map | cargo JSON messages | ESLint and tsc output | Ruff and type-checker output |
| Fingerprint pathspecs | `*.rs`, `Cargo.*` | `*.ts`, `*.tsx`, `package.json` | `*.py`, `pyproject.toml` |
| Rubric dimensions and a reviewer agent | 12 dimensions, `rust-reviewer` | its own | its own |
| A Bash rule pack for redkiln-guard | cargo add, `--no-verify` | npm install, lockfile edits | pip install |

Each language plugin's gate covers only its own pathspecs, so a TypeScript-only change never runs the Rust gate, as temper already does for non-Rust changes. In a mixed repository two Stop gates run in parallel; each caps its own blocks, and the reviewer verdicts are separate events, each bound to its own plugin's change fingerprint. The tool lists for TypeScript and Python are from memory and need checking when those plugins are scoped.

## Prior art and patterns

Nothing found refuses bad code before it is written and then holds the turn until a gate and a reviewer agree; the closest tools advise and leave the agent to decide. The pieces exist separately, and three are worth taking: per-line ratchets, packaged command-guard rule packs, and agent-readable standards with stable IDs.

| Prior art | What it is | Pattern to take | What to leave |
| --- | --- | --- | --- |
| [Pragmatic Rust Guidelines](https://microsoft.github.io/rust-guidelines) (Microsoft) | A Rust guideline book (version 2026.6) with must and should rules, an "Agents & LLMs" section and an AI chapter | A source to cite in `sources.md` and check rules against; its must/should split matches rust-craft's MUST and review-only rules | Its loose "spirit, not letter" stance; rust-craft's rules are enforced |
| Rust API Guidelines and Clippy (from memory) | The community API checklist and the lint catalogue | Every rule that Clippy can check stays delegated to Clippy, as rust-craft already does | Re-implementing a lint the compiler has |
| [Cargo `[lints]` table, RFC 3389](https://rust-lang.github.io/rfcs/3389-manifest-lint.html) | Lint levels in `Cargo.toml`, inherited from the workspace | The template's `[workspace.lints]` stays the single home of lint policy; manifest policy asks on any weakening | Per-crate lint tables that drift |
| [Trunk Check, Hold the Line](https://docs.trunk.io/check/configuration/hold-the-line) | Lints the upstream and the worktree and reports only issues on new or changed lines, across config changes | Confirms temper's line-level ratchet; take its handling of config changes (a new lint does not flood old code) into redkiln-craft's ratchet | A hosted service |
| [Betterer](https://dev.to/phenomnominal/betterer-v1-0-0-301b) | Snapshot test of issue counts in `.betterer.results`; worse fails, better updates | Language-neutral ratchet results as a committed file, as `temper-baseline.json` does; the shape redkiln-craft generalises | Auto-updating on improvement without review; temper's tighten is explicit |
| [destructive\_command\_guard](https://cdn.jsdelivr.net/gh/dicklesworthstone/destructive_command_guard@main/SKILL.md) (dcg) | A Rust PreToolUse hook that blocks destructive shell commands in under a millisecond, with rules in modular packs | Rule packs per domain over one engine, which is redkiln-guard's design; a quick prefilter before regex work | Default allow on anything unmatched; temper reads the command's real write targets instead |
| [Semgrep Guardian](https://docs.semgrep.dev/guardian) | MCP server, hooks and skills that scan agent edits and return findings | Scanning at authoring time, before commit | Advice the agent may ignore; rust-craft denies, and CI is the authority |
| cargo-semver-checks (from memory) | Detects semver breaks in a crate's public API | A gate step for H-16's "API breaks are human decisions" on published crates | Nothing |

The pressure test adds one more from hosted reviewers: per-rule fired, overridden and accepted counts (its change 17). Redkiln Rust already has the telemetry to compute them per rule ID, and they are the evidence for promoting a review-only rule to a mechanical one, as wi-3fa7cf just did for five rules.

## How it composes with the rest of the suite

Redkiln Rust is a producer of facts and a provider of checks; no sibling depends on it to work, and it depends on no sibling. Siblings reach it through its CLI and its `rust.*` events, never its internals.

[embedded content: Redkiln Rust in the suite · 3 callers, 2 kits, 1 log]

Arrows point from a user to what it uses. Bench, Loop and Review call Rust; Rust calls the two kits and writes to Core's log.

- **Bench** runs `gate` in build and verify and reads `rust.gate_run`. Both guard Bash over redkiln-guard with their own packs, so with both installed a command is judged once by one engine. Bench stops recording Rust's `Status: WIP` as a violation once Core's end-of-turn grammar lands.
- **Loop** runs `gate --json` in its verify stage and takes over Rust policy it wrote for itself (call F). An unattended run's lint exception becomes a Weigh-In decision request that blocks, since H-16 calls are one-way. The scanner and the gate skip in-checkout worktree roots such as Loop's .redkiln/worktrees/, so a run's worktrees are never scanned or fingerprinted as part of the main checkout (pressure-test rec 10).
- **Review** calls `scan`, `baseline --check` and the four `check` subcommands by digest, at the version pinned in .redkiln/suite.toml, the suite's one pin; CI checks Review's .review/plugins.toml digest against it. The reviewer's findings use review-contract's schema, so Review's verifier can check an in-session verdict too.
- **Weigh-In** receives every H-16 ask (new dependency, lint exception, removed test, rust-version raise) as a typed item through weighin-contract, instead of a prose `Decisions needed:` line only.
- **Knowledge** stores the settled decisions and false-positive rulings as atoms, which the reviewer and SessionStart read back. The SessionStart hook keeps to the suite's context budget, and it exports no telemetry or journals from client repositories.
- **Backlog** carries rule proposals and false positives as issues, from temper's existing templates, as the Backlog study suggests for per-plugin forms.
- **Core and kits.** Core resolves the store from the hook's cwd, hashes approvals with the fingerprint method, and exports journals. redkiln-craft runs the gate and validates verdicts; redkiln-guard reads shell commands.

## Calls for Ryan and next steps

Six calls shape Redkiln Rust. A, B and C are details of Blueprint call 5 (port in the monorepo, libexec not npm), listed so each can be answered on its own. Call 5 also covers the MSRV raise from 1.96 to 1.98.1, which H-16 makes a human decision. Blueprint call 3 (the kits layer) is a prerequisite; the visualization study's calls cover the lens. Only B is hard to undo, because published crate names are permanent.

| # | Call | Recommendation | Door |
| --- | --- | --- | --- |
| A | Detail of Blueprint call 5: cut the port vertically: the generic engine into redkiln-craft and the shell reader into redkiln-guard from Phase 0a, replacing D3's four horizontal crates | Yes. Splitting after the port means re-splitting freshly parity-tested crates | Two-way until the crates exist |
| B | Detail of Blueprint call 5: publish Redkiln Rust's crates on crates.io (D2), or link them only into the plugin binary and leave the reserved `temper-*` names idle | Do not publish. CI fetches the binary by digest, which removes D2's only remaining reason | One-way once published |
| C | Detail of Blueprint call 5: amend D10 and the no-async constraint so the port links Core (happenstance, with tokio hidden behind a synchronous API) | Yes. Without it, Rust is the one plugin outside the shared log | Two-way |
| D | Move assay's temper checks into Redkiln Rust as four subcommands that Review calls by digest: `check raised`, `check exceptions`, `check gate-config` and `check tests`, each taking `--base REV` with a JSON result. This absorbs Review's former call E: its inventory and manifest-switch checks map to `check tests`, and its lint-diff to `check raised` and `check gate-config` | Yes. One owner for Rust rules; Review's adapter drops from 9,466 production lines to a runner | Two-way |
| E | Rename stored contracts (`temper_*` JSON keys, `.claude/temper*`, `TEMPER_*` env, `.git/temper/`) at the 2.0 cutover, importing 1.x state once | Yes. Renaming mid-port doubles the parity matrix, and 1.x users are all internal | Two-way |
| F | Take redkiln-rs's ADR-0083, 0084 and 0100 into Redkiln Rust as a repository profile, not as new rust-craft rules | Profile. They are Loop's house rules, and most of rust-craft already covers them | Two-way |

### Suggested next steps

- Let the pilot finish: the report on 2026-10-18, the fixes, then 1.4.0 (wi-891617). No hook or gate change before it (wi-b63bd6).
- Ryan decides calls A to F alongside Blueprint calls 3 and 5.
- Amend `docs/plans/temper-2-rust-port.md` for the monorepo: section 1.1 (workspace root), D1, D2, D3, D4, D10, D13, D14, D17 and invariant 3, recorded through Weigh-In.
- Specify redkiln-craft's language-pack interface from temper's \~3,500 agnostic lines, with the Rust pack as its first user.
- Write the four `check` subcommands' JSON contracts with assay's fixtures, so Review's adapter can switch before the port lands.
- Map the 22 telemetry names to `rust.*` event types, split into durable facts and telemetry, in Core's SPEC draft.
