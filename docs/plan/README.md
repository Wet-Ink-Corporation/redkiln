# The live plan

`references/` is the planning handover, frozen on 2026-10-19. Its migration plan (`references/docs/migration-plan.md`) is still the plan: the move order, the gates, the eight steps of a move and the principles stand as signed off. This folder holds what has happened since: where each gate stands, which parts of the frozen text later decisions replaced, and the runbook for the next move. Where this folder and `references/` disagree, this folder is newer; where this folder and `AGENTS.md` or `spec/` disagree, those win, because they are the checked rules.

| File | What it is |
| --- | --- |
| [README.md](README.md) | Gate status, corrections to the frozen plan, open calls |
| [move-1-weigh-in.md](move-1-weigh-in.md) | The Move 1 runbook: Weigh-In 1.2.0 into `rk-weigh-in` 2.0.0, with `redkiln-core` extracted from its store |

Each later move gets its own runbook here when it is next, written from its plugin document in `references/docs/` and the same template.

## Where each gate stands (2026-11-01)

| Step | Gate | State | Evidence |
| --- | --- | --- | --- |
| Phase 0 | Skeleton, SPEC v1 draft, five spikes | Skeleton done (#2, #5). SPEC v1 draft merged with OW-1 to OW-15 confirmed (#6, #7). Org-sync spike done (wi-f041fb; packaging rows in `AGENTS.md`). Refs spike done, OW-13 settled (#9). Core edges hub-only (#8). CLI first, harness second (#10). Field guide (#11). **Open:** R-SQLITE, the cloud bootstrap at pinned versions, `libexec/` in Cowork | redkiln `main` at `a32e1d9` |
| Move 1, Weigh-In | 1.1 with `ci-check --json` released (WI call C) | **Clear.** 1.1.0 shipped `ci-check` (#145, tagged 2026-10-29); 1.2.0 is the current release (2026-11-01, `v1.2.0`, weigh-in `8587f57`); no open PR, one open issue (#27, the slice 2 backlog). `weigh-in-plugin` serves 1.2.0 (`7db2726`) | [move-1-weigh-in.md](move-1-weigh-in.md), Preconditions |
| Move 2, Bench | anneal's calibration report (#44) | Not clear: #44 open; Ryan's dogfood labels (#27) and the Tier 3 "seed a workspace" fix outstanding | anneal #44 |
| Move 3, Review | The in-place rename to `review` | Not started; owner setup already exists under assay names (four ordered steps in `references/README.md`) | assay `main` |
| Move 4, Loop | M5, `build/phase-0-to-1` on `main` (#24), #79 checklists | Not clear: s8j in progress, branch not landed | redkiln-rs #24, #79 |
| Rust 2.0 | temper 1.4.0 and Core 1.0 | 1.4.0 done; Core 1.0 waits for Move 2; port plan not amended for the monorepo | temper `docs/plans/temper-2-rust-port.md` |

Of the three open spikes, none gates Move 1 (the runbook says why, under Preconditions). R-SQLITE must run before Move 2, because Core 1.0 freezes ST-05's TRUNCATE rule there, and Loop's store (Move 4) depends on its result.

## What later decisions changed in the frozen plan

Each row names the frozen text, what replaced it and where that is recorded. The frozen files are not edited.

| Frozen text | Now | Recorded in |
| --- | --- | --- |
| Move 1 brings "happenstance 0.4.1" (migration plan, Work sequence; weigh-in.md, Moving the data) | Core pins every `happenstance-*` crate at `=0.4.5`, `happenstance-sqlite` with `features = ["sync"]`; ingest comes from that release, never a `[patch]` | SPEC ST-04; `AGENTS.md` decisions, 2026-10-31 and 2026-11-01 |
| Core carries "the 0.1.x and 0.2.2 fixtures" | Also a ledger written by weigh-in 1.2.0 on happenstance 0.3.2, which is what every live ledger is today | [move-1-weigh-in.md](move-1-weigh-in.md), Core extraction |
| Journals "where pressure-test call A says"; fallback "committed `.redkiln/journal/<session>.jsonl`" (migration plan, Risks) | OW-13 option D: one append-only ref per journal, `refs/redkiln/journal/<name>`; cloud sessions push a `redkiln/inbox/<name>` branch that an optional relay workflow merges; import reads every source, legacy committed journals included | SPEC JR-06 to JR-13; `AGENTS.md` decisions, 2026-11-01 |
| Core crates "depend on nothing internal" (blueprint, dependency rules) | Core edges go through `redkiln-core` only; `cargo xtask layers` checks it | `AGENTS.md`, Layer rules; decisions, 2026-10-31 |
| Plugins are "thin shells (hooks, skills, a launcher, one binary)" | Made a checked rule: CLI first, harness second. Hook protocol in one adapter module per binary; no scripts or policy data in `plugins/`; `cargo xtask plugins` enforces the layout. Bench must move anneal's `playbooks/` data into its CLI | `AGENTS.md`, CLI first, harness second; SPEC CLI-13 |
| Visualization A: "one suite mod … in its own optional plugin" | Holds as the end state; Move 1 keeps Weigh-In's mod as its own plugin, `rk-weigh-in-mod`, until the suite mod can reach several binaries (Ryan, 2026-11-01, runbook M1-C). The org-sync spike's "ship mods inside their plugin" was overtaken the same day: a desktop chat turns off a whole plugin that names a mod, skills included (weigh-in B-35, temper and anneal the same), so a mod is always its own plugin | `AGENTS.md`, CLI first (a mod lives in its own plugin); weigh-in B-35; anneal and assay decisions, 2026-10-31 |
| Org sync: "the synced repository must be private or internal" (org-sync spike, first pass) | Ryan's account is an individual plan: plugins arrive through Customize > Plugins (account marketplace), public or private both work. One release repository holds every `rk-*` plugin with Sync automatically on. Bundles do not work there and version ranges are not enforced | `AGENTS.md` decisions, 2026-10-31 (bundle); org-sync spike write-up, project files `redkiln-v1/org-sync-spike/README.md` |
| Step 1 of a move: "the required-check names for the new package are added" (owner) | No ruleset change: `main` requires only the aggregate `ok`, and each new job joins it in `ci.yml` | `AGENTS.md`, Commands (CI) |
| Step 4: "the four vendored `.claude/temper/` copies are deleted"; temper 1.4.0 vendored at the root | The root copy is already in place (phase 0). Each move deletes its own copy (weigh-in's is temper 1.3.0) and re-baselines at the new paths; weigh-in's baseline totals 0, so Move 1 adds no baseline entries | `AGENTS.md`, Commands; weigh-in `.claude/temper-baseline.json` |
| Weigh-In's own plan to move "into redkiln-rs" (weigh-in wi-b4c1d8, B-22; its README and charter still say so) | Superseded by the suite decision (2026-10-13): Weigh-In moves into this repository as `rk-weigh-in`; Loop's gates become Weigh-In requests (Blueprint call 7) | `references/decisions/README.md`; weigh-in #27 says it is settled on the redkiln side |
| `weigh-in-plugin` "nothing until the org-sync spike" | The spike passed (wi-f041fb). Ryan decided on 2026-11-01 to rename it to `redkiln-plugins`, the suite's release repository, at Move 1, ahead of the plan's Train 1 (runbook M1-G). Today it serves `weigh` and `weigh-mod` 1.2.0 | [move-1-weigh-in.md](move-1-weigh-in.md), Release and cutover |
| Field guide: D-0021 "is designed, not built (weigh-in #171)" (`docs/guide/plugins-and-surfaces.md`) | Built and released in 1.2.0: `plugin/libexec/cowork-fetch.sh`, tested, confirmed in a Cowork sitting | weigh-in `CHANGELOG.md`, 1.2.0 |

## Open calls for Ryan

Each call below has a recommendation; the work proceeds on the two-way ones and waits on the one-way ones. Move 1's own calls are listed in its runbook, under Decisions before the import.

| Call | Door | Recommendation | Source |
| --- | --- | --- | --- |
| Migration B, D, E | Two-way | Proceed on their recommendations, as the handover says | `references/decisions/README.md` |
| Field guide T1, T2, T4, T6, T7, T8, T10 | As listed | Each is handled at the move that needs it; T3 and T9 are Move 1's and sit in its runbook; T5 (agent identity) waits for Review | `docs/guide/README.md`, Tensions |
| Turn the plan into issues here (an Initiative per move, an Epic per gate, Backlog's body contract) | Two-way | Do it for Move 1 when its import PR opens, so the board has real data from the first move | `references/README.md`, Where to invest next |
