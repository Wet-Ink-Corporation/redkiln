# Decisions taken during suite planning

The planning project (2026-10-12 to 2026-10-19) settled these calls. Most were taken on Weigh-In cards in cloud sessions, whose ledgers did not outlive their containers, so this file is the durable record. Only the five Backlog decisions survived as atom files; they are in [`backlog/`](backlog/). Decisions cited from the source repositories (temper, weigh-in, assay, anneal, redkiln-rs) live in those repositories and are not repeated here.

Newest first. "Doc" names the planning document in [`../docs/`](../docs/) that holds the reasoning.

| Date | Id | Decision | Doc |
| --- | --- | --- | --- |
| 2026-10-19 | wi-d5b04b | Visualization A: one suite mod, shipped in its own optional plugin, replaces the four per-repo mod plans (temper #107, anneal #56, assay #123 to #132, weigh-in #112) | visualization-surfaces |
| 2026-10-19 | wi-7de964 | Visualization B: one suite board Artifact, grown from Weigh-In's board | visualization-surfaces |
| 2026-10-19 | wi-ab2df0 | Visualization C: a Ratatui TUI is in scope, gated on redkiln-rs's S-VIEW spike | visualization-surfaces |
| 2026-10-19 | wi-ec4bc2 | Visualization D: no self-hosted web UI until Loop is hosted for several users | visualization-surfaces |
| 2026-10-19 | wi-6e750e | Visualization E (one-way, H-16): TypeScript and the `claude` CLI are approved in monorepo CI for the suite mod | visualization-surfaces |
| 2026-10-19 | wi-cef669 | Backlog A: one suite-wide label set and one label manifest | backlog |
| 2026-10-19 | wi-5edd9b | Backlog B: priority, effort and dates use GitHub issue fields, not `prio/*` and `size/*` labels | backlog |
| 2026-10-19 | wi-69aa12 | Backlog C: the issue body holds the implementation contract (R#, AC#, TC#, checklist); long specs are committed files it links | backlog |
| 2026-10-19 | wi-491ac0 | Backlog D: board and roadmap are our own projections over Issues (`redkiln-view`), not a GitHub Project (first decided in chat 2026-10-17) | backlog |
| 2026-10-19 | wi-824f37 | Backlog E: the first wave covers triage, refine, split, lint, next, sweep, import and board sync; prioritisation and forecasting come later | backlog |
| 2026-10-19 | wi-f041fb | Migration C: `weigh-in-plugin` is renamed into the suite's org-sync release repository once the org-sync spike passes | migration-plan |
| 2026-10-19 | wi-b94a5e | Migration F: go-ahead for both paid runs on the critical path (temper 1.4.0 smoke and evals, which ran 2026-10-18 in temper PR #133, and Loop's first real M5 fix lane) | migration-plan |
| 2026-10-19 | none (chat) | assay is renamed to Review now, although owner setup already happened under assay names; the four ordered steps are in the handover README | migration-plan |
| 2026-10-18 | none (chat) | Migration A: the original Node Redkiln is `Wet-Ink-Corporation/redkiln-old` (holds `e61b68f`); it stays read-only and is never deleted | migration-plan |
| 2026-10-18 | none (chat) | Plugin blueprint sign-off: every open call in the Blueprint, the pressure test and the seven plugin documents accepted as recommended, with Blueprint call 8 amended to pressure-test rec 16 (catalog engine becomes the `redkiln-catalog` kit, content a versioned data package) | all |
| 2026-10-13 | none (chat) | Suite direction: separately installable plugins under the Redkiln name in one `redkiln` monorepo; Core is a shared library and format spec, not a plugin; one shared happenstance SQLite event log tagged by plugin; the backlog is GitHub Issues | blueprint |

Migration calls B (Review owner setup after the move), D (`rk-bench` 0.1.0 waits for the catalog) and E (vendored temper 1.4.0 gates the monorepo until Redkiln Rust 2.0) were not decided; the plan proceeds on their recommendations unless they are reopened.
