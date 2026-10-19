# references: the suite planning handover

Planning for the Redkiln suite ran from 2026-10-12 to 2026-10-19 in a Claude project outside this repository. That project is closed, and planning continues here. This folder is its frozen output: the Blueprint, the per-plugin documents, the roadmap and migration plan, the repository audits behind them, and every decision taken along the way.

Start with [the migration plan](docs/migration-plan.md). It says what moves into this repository, in what order and behind which gate, and it lists what is already done.

## What is here

| Path | What it is | As of |
| --- | --- | --- |
| [docs/blueprint.md](docs/blueprint.md) | The Redkiln Blueprint: the end state, with every package, what it owns and how they fit | 2026-10-18, signed off |
| [docs/blueprint-pressure-test.md](docs/blueprint-pressure-test.md) | Prior art, run modes and git scenarios tested against the Blueprint, with the changes it led to | 2026-10-18, signed off |
| [docs/migration-plan.md](docs/migration-plan.md) | Roadmap and migration plan: move order, gates, how a move is done, progress, risks | 2026-10-19 |
| [docs/weigh-in.md](docs/weigh-in.md), [rust.md](docs/rust.md), [bench.md](docs/bench.md), [review.md](docs/review.md), [loop.md](docs/loop.md), [knowledge.md](docs/knowledge.md), [backlog.md](docs/backlog.md) | One document per plugin: what it should entail, its crates, contracts and calls | 2026-10-18, signed off |
| [docs/visualization-surfaces.md](docs/visualization-surfaces.md) | Suite mod, board Artifact, TUI and web options, scored, with the `redkiln-view` crate shape | 2026-10-19, decided |
| [decisions/](decisions/README.md) | Every decision the planning took, with its Weigh-In id; the five Backlog atoms as files | 2026-10-19 |
| [strategy/audit/](strategy/audit/) | One audit per source repository: temper, weigh-in (with weigh-in-plugin), anneal, assay, redkiln-rs | 2026-10-13 |
| [strategy/suite-vs-redkiln.md](strategy/suite-vs-redkiln.md), [redkiln-vs-flow.md](strategy/redkiln-vs-flow.md), [suite-structure.md](strategy/suite-structure.md) | The first write-ups that chose a suite of plugins over one product | 2026-10-12 |
| [strategy/blueprint.md](strategy/blueprint.md) | The Blueprint's first draft, kept for its history; `docs/blueprint.md` supersedes it | 2026-10-13 |
| [images/](images/) | A screenshot from the project of the Blueprint's layer diagram as plain text | 2026-10 |

Each file in `docs/` was exported from a Claude Doc, which stays the original; the first line of each file links it. Diagrams did not survive the export and show as `[embedded content: …]` lines that name what they drew; open the Doc to see them. The early write-ups predate two corrections: redkiln-rs's real work is on `build/phase-0-to-1`, not `main`, and assay's code is on `main`, not its default `spec/architecture` branch. They understate both.

## Waiting on Ryan

These steps need the owner's account. Nothing in this folder does them.

**Rename assay to Review.** Decided 2026-10-19. The order matters, because `main`'s ruleset blocks merges until its required check is posted:

1. Rename the `assay-verdict` App (id 5371952) to `redkiln-review` and `assay-ledger-writer` to `redkiln-review-ledger`. Their ids, keys and installations stay.
2. Merge the assay PR that posts the check as `review / verdict` and renames the `ASSAY_VERDICT_*` secret references.
3. In the same sitting, change ruleset `25356300` to require `review / verdict` from integration 5371952.
4. Rename the secrets and variables in the `assay-publish` environment to match.

**Set up this repository** (phase 0, step 2 of the plan): a ruleset on `main` that allows merge commits, because every import is a merge commit that keeps the old history's hashes.

**Settle what assay still owes**: make `main` its default branch, design the approver account (#41, deferred for a hardware key) and create the environments (#43).

## Open work in the source repositories

Each item is a gate in the migration plan or an open loop the planning found. They are tracked in their own repositories.

| Repository | Open | Why it matters here |
| --- | --- | --- |
| weigh-in | 1.1 with `ci-check --json` (#145) | Gates move 1, the first move into this repository |
| anneal | Calibration run and report (#44); guard PRs #144, #147 and #150 | The report gates move 2 and the Core 1.0 freeze |
| assay | The rename above; approver (#41) and environments (#43) | Gates move 3 |
| redkiln-rs | Finish s8j; Loop's first paid M5 fix lane (#36, #62, already approved under wi-b94a5e); land `build/phase-0-to-1` on `main` (#24, 570 commits ahead); write the catalog checklists (#79, sources in the read-only `redkiln-old`) | Gate move 4 and `rk-bench` 0.1.0 |
| temper | Pilot findings #135 to #141; amend `docs/plans/temper-2-rust-port.md` for the monorepo (D1, D2, D3, D4, D10, D13, D14, D17 and invariant 3) | The Rust 2.0 port starts here after Core 1.0 |
| weigh-in-plugin | Nothing until the org-sync spike (wi-f041fb) | Becomes the suite's release repository |

Migration calls B, D and E were not decided and proceed on their recommendations unless reopened (see [decisions/](decisions/README.md)).

## Where to invest next

Suggestions only; none of them is started.

1. **Lay the phase 0 skeleton here**: workspace `Cargo.toml`, toolchain 1.98.1, the `AGENTS.md` charter with crate map and layer rules, the structural layer test, path-filtered CI with room for the TypeScript mod (wi-6e750e), and Backlog's label manifest (wi-cef669). Everything else lands on it.
2. **Run the org-sync spike first.** It decides whether one release repository serves the whole suite (wi-f041fb) and shapes every plugin's packaging.
3. **Draft SPEC v1 in `spec/`**: store location, envelope, decision request record, trust levels, approval record, journal format and CLI envelope. Its one-way items freeze only at move 2.
4. **Turn the plan into issues here**: an Initiative per move with Epics for each gate, in Backlog's body contract (wi-69aa12), so the board views have real data from day one.
5. **Start the #79 catalog checklists early.** About half of the 28 unwritten checklists need new writing (C4, security, use cases, UX, release, incident), which is the longest lead time on move 4.
6. **Redraw the 14 diagrams the export dropped** as Mermaid in these files, so the repository copy stands alone without the Docs.
