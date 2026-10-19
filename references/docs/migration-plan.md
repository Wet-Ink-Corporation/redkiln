<!-- Exported from the Claude Doc https://claude.ai/code/artifact/5b733df0-c15a-4699-8c6b-b2f026b9f0c6 (revision 16) on 2026-10-19 as part of the project handover. The Doc is the original; this copy is frozen. Diagrams (1) could not be exported and show as [embedded content] placeholders; open the Doc to see them. -->

# Redkiln roadmap and migration plan

Oct 18, 2026 · @Ryan

## Summary

Weigh-In moves first, then Bench, Review and Loop, one at a time, each only after a gate in its own repository clears; Redkiln Rust is rebuilt as 2.0 in the monorepo alongside them.

- **Now (phase 0):** lay the monorepo skeleton, run the five spikes (org sync first) and draft SPEC v1. The old repositories meanwhile finish only their gates: temper 1.4.0, Weigh-In 1.0.0 and 1.1, assay's rename, Loop's M5 and landing, and anneal's calibration run.
- **Move 1, Weigh-In,** after 1.1 ships. Core is extracted from its store, and it arrives as `rk-weigh-in` 2.0.0.
- **Move 2, Bench,** after anneal's calibration report. Core and SPEC freeze at 1.0 with two real users, and the guard, github and calibrate kits are cut from its code.
- **Move 3, Review,** after the in-place rename. Owner setup then happens once, against the monorepo.
- **Move 4, Loop,** after M5 lands on `main` (#24), the corpus is backed up (#32) and the #79 checklists are in. The catalog kit it brings lets Bench ship `rk-bench` 0.1.0.
- **Redkiln Rust 2.0** starts after temper 1.4.0 and Core 1.0, with 1.4.0 as its parity oracle. temper keeps shipping 1.4.x until 2.0 cuts over.
- **Every move** imports full history without rewriting hashes, renames at the door, imports old state once without deleting it, and archives the old repository with a redirect.

Calls A, C and F are settled: the original Node Redkiln is `redkiln-old`, `weigh-in-plugin` becomes the suite's release repository after the org-sync spike, and the paid runs have your go-ahead. Calls B, D and E proceed on their recommendations unless you say otherwise.

## Progress as of 2026-10-19

Three gates moved since the plan was written: temper 1.4.0 and Weigh-In 1.0.0 shipped, and the Loop corpus is backed up. One change in assay runs against the plan and needs your call (below).

| Repository | Done since 2026-10-17 | Next gate | Evidence |
| --- | --- | --- | --- |
| temper | 1.4.0 released (tag `v1.4.0`, PR #134) after the release-candidate smoke test and evals; v1 go-live (#20) and pilot close-out (#15, #16) closed; the seven pilot findings are open as #135 to #141; the lens PRs (#117 to #121, #123) merged | The 2.0 port plan still has no monorepo amendments (no mention of the monorepo in `docs/plans/temper-2-rust-port.md`) | PRs #133, #134, #142 |
| weigh-in | 1.0.0 released 2026-10-18 and recorded live (B-27, #146, #148); the pilot (#23) closed early; assay's D-WI filed as #145 | 1.1 with `ci-check --json` (#145, open), which gates move 1 | tag `v1.0.0` |
| weigh-in-plugin | Serves the Weigh-In 1.0.0 marketplace | None until the org-sync spike (call C) | commit `c86e5e7` |
| anneal | Many guard fixes (#121, #127, #128 closed; #144, #147, #150 open), a launcher that downloads the release build, spec-to-design-tree links (#77) and `dag` and git-timeout fixes | The calibration run and report (#44, open) | PRs #140 to #154 |
| assay | Owner setup done under the assay names: the `assay-verdict` App and rulesets requiring `assay / verdict` (#39), a second `assay-ledger-writer` App (ADR-26, #40), the agent identity (#42). The approver account is deferred for a hardware-key design (#41, #269). D-WI filed (#44) | The rename to `review` is not started, and owner setup no longer comes after it | ADRs 0004 and 26, PRs #266 to #268 |
| redkiln-rs | s8j build batch 2 merged (PRs #103 to #112: ADRs 0200 to 0203 accepted, rulings D21-161 to D21-163); the 20 AFK rulings ratified (D21-156, #28); the corpus has an off-machine copy (#32 closed) | Finish s8j, then the first paid fix lane for M5 (#36, #62); `build/phase-0-to-1` is 570 commits ahead of `main` (#24) | `build/phase-0-to-1` at `7cb39a0` |

What this changes in the plan:

- **Call F:** temper's paid runs already happened on 2026-10-18 (recorded in PR #133), so only Loop's first fix lane remains under that go-ahead.
- **Call B:** owner setup has largely happened before the move, against assay. Only the approver (#41) and environments (#43) remain.
- **Move 3:** Review blueprint call A said rename before owner setup, so that the App and check are created once under Redkiln names. They now exist as `assay-verdict`, `assay-ledger-writer` and `assay / verdict`. Renaming a GitHub App keeps its id, so the rulesets that pin integration `5371952` still hold. The required check's name would change, though, and it must be renamed before #116 makes it required on a consumer repository.

**Decided 2026-10-19: rename now.** The order matters, because `main`'s ruleset blocks merges until the check it requires is posted:

1. Rename the `assay-verdict` App to `redkiln-review` and `assay-ledger-writer` to `redkiln-review-ledger` (owner, App settings). Their ids, keys and installations stay.
2. Merge one assay PR that posts the check as `review / verdict` and renames the `ASSAY_VERDICT_*` secret references and the App names in the docs. It lands first in the rename to `review` (RV call A).
3. In the same sitting, change ruleset `25356300` to require `review / verdict` from integration `5371952` (owner).
4. Rename the secrets and variables in the `assay-publish` environment to match (owner).

**Visualization surfaces, all decided 2026-10-19.** One suite mod in its own optional plugin (wi-d5b04b) and one suite board grown from Weigh-In's board (wi-7de964), both reading the `redkiln-view` crate. A Ratatui TUI is in scope behind redkiln-rs's S-VIEW spike (wi-ab2df0), and there is no self-hosted web UI until Loop is hosted for several users (wi-ec4bc2). TypeScript and the `claude` CLI are approved in monorepo CI for the mod (wi-6e750e). For this plan, the skeleton's path-filtered CI makes room for a TypeScript package, and `redkiln-view` arrives with Backlog in train 1, since the board's first views are Weigh-In's and Backlog's.

**Handover 2026-10-19.** Planning continues in the `redkiln` repository. This plan, the blueprints, the audits and every decision record are copied to its `references/` folder.


## Starting point

Five of the six repositories are live products with frozen windows, and the target monorepo has no commits yet. Checked on 2026-10-18 from fresh clones and the GitHub issues cited.

| Repository | Becomes | State today | What holds it in place |
| --- | --- | --- | --- |
| temper | Redkiln Rust | 1.3.0 released; pilot fixes on `main`, unreleased; Node engine, no Rust yet | Pilot report due 2026-10-18, then 1.4.0 as v1 (wi-891617); hooks and gate frozen while the pilot runs (wi-b63bd6) |
| weigh-in + weigh-in-plugin | Weigh-In | 0.9.0 released as the v1.0 candidate; week 2 of the pilot (#23) opens on or after 2026-10-19 | Capture frozen until 1.0.0, due on or after 2026-10-20 (B-23, wi-7c3aa0) |
| anneal | Redkiln Bench | 0.0.5 on `main`, about 63k lines of non-test Rust in one CLI crate; v0.1.0 planned in #44 | Calibration needs n ≥ 50 labelled decisions per Clef point; #44 itself expects most points to ship in shadow |
| assay | Redkiln Review | Code on `origin/main` (default branch is still `spec/architecture`); 6 crates, about 48k lines; 0.1.0, no tags; about 140 open issues, mostly review follow-ups | Owner setup not done: the App, ledger repo, approver, agent identity and environments (#39 to #43) |
| redkiln-rs | Redkiln Loop | Real work on `build/phase-0-to-1` (12 crates, about 238k lines, 198 ADRs), last merge 2026-10-18; `main` is a September design snapshot | The branch has not landed on `main` (#24, a `--no-ff` merge at M5); the corpus has no remote (#32) |
| redkiln | The monorepo | Empty: no commits, no default branch | Nothing; it is ready for a skeleton |

One surprise: redkiln-rs #79 cites the original Node Redkiln at `Wet-Ink-Corporation/redkiln@e61b68f` as the source for about half of the catalog's 28 missing checklists. That history is no longer in this repository. If no other copy exists, those sources are gone (see the calls at the end).

## Principles

A product moves when its current promise is kept and before its next name is public. Six rules decide the timing.

1. **Never move a product while a pilot is measuring it.** Moving its store, hooks or install ID mid-pilot invalidates the measurement. temper waits for 1.4.0 and Weigh-In for 1.0.0.
2. **Released products finish their v1 in place, then arrive as their next major version.** The old repository ships the last 1.x; the monorepo ships Weigh-In 2.0 and Redkiln Rust 2.0. No user sees a half-moved product.
3. **Unreleased products move before their first release, under the new name.** assay, anneal and redkiln-rs have no public install IDs yet. A rename now costs a search and replace; after a release or the assay App exists, it costs a migration.
4. **Core is extracted, not invented.** Its first version is weigh-in's and anneal's ledgers made one (they are near-copies today). It ships only once two plugins use it, and Loop conforms to it last.
5. **Hashes survive.** Each repository comes in with its full history unrewritten, so commit hashes cited in ADRs, decision records and PROGRESS logs still resolve. That rules out squash, rebase and `filter-repo` (see Mechanics).
6. **One product in motion at a time.** Each move ends with the product green under the monorepo's gate, its old repository archived with a redirect, and its issues transferred, before the next move starts. Work on the other products continues in their own repositories meanwhile.

## When each product moves

Weigh-In moves first, then Bench, Review and Loop; Redkiln Rust is not moved but rebuilt as 2.0 alongside them. Each row's gate is a fact in its own repository, not a date. The dates shown are the earliest the cited sources allow.

| Order | Product | Moves when | Arrives as | Why here |
| --- | --- | --- | --- | --- |
| 1 | Weigh-In (weigh-in) | 1.0.0 is released (on or after 2026-10-20, B-23) and 1.1 with `weighin ci-check --json` is released from the old repository (WI call C) | `rk-weigh-in` 2.0.0 with a one-time ledger import (WI call E) | Smallest Rust codebase (17.6k lines, baseline at 0). Its store becomes `redkiln-core`, and every sibling consumes `weighin-contract`, so it has to exist first. Bench's hand-off already assumes Weigh-In is there. |
| 2 | Bench (anneal) | Its calibration window (2026-10-22 to 10-30 in #44) has run in place and the calibration report is cut (BE call E) | `rk-bench`, unreleased until it runs on the catalog | Its ledger is Core's second source, so Core 1.0 and SPEC 1.0 freeze here with two real users. Its code seeds three kits. |
| 3 | Review (assay) | The rename to `review` is done in assay's own repository and the trust path is narrowed to `crates/review/**` (RV call A, next steps) | `rk-review` plugin plus `deploy/review/` | The rename is free only while the App and ledger repository do not exist. Moving before owner setup means they are created once, against the monorepo. |
| 4 | Loop (redkiln-rs) | M5 is reached, `build/phase-0-to-1` lands on `main` with `--no-ff` (#24), the corpus is pushed (#32) and the #79 checklists are in (PT rec 16, LO call D). M5 P50 was 2026-10-23 in the 2026-10-06 audit. | `rk-loop` with its 12 crates under `crates/loop/` | Largest and least portable (40 `crates/` greps in CI, 241 `D:/repos` paths). It conforms to Core last, and its arrival unlocks the `redkiln-catalog` and `redkiln-dispatch` kits. |
| 5 | Redkiln Rust (temper) | temper 1.4.0 is released and Core 1.0 exists; the port runs alongside moves 3 and 4 | `rk-rust` 2.0.0, a Rust rewrite with 1.4.0 as its parity oracle (BP call 5) | The Node 1.x is not imported as code. temper's history comes in for the standard and the docs, and the temper repository keeps shipping 1.4.x fixes until 2.0 cuts over. |

Knowledge and Backlog are new plugins, so they have no move. `knowledge-contract` is born in move 1, because `atom.rs` goes into it byte for byte. The Knowledge plugin follows once Weigh-In 2.0 has shipped. Backlog comes after Loop, whose `docs/BACKLOG.md`, forms and `backlog-ledger` skill are its sources.

## Finish in place before moving

Each old repository finishes only what its gate needs, and no feature work that the monorepo would redo. Anything a blueprint assigns to a kit or to Core waits for the monorepo.

**temper (stays the 1.x line until Redkiln Rust 2.0 cuts over)**

- [x] Pilot report (due 2026-10-18, wi-292a7d) with one `pilot` issue per finding (#15)
- [x] Pilot fixes (#16), the `[hold]` lens PRs (#117 to #121, #123) and the staged MUST-rule checks (#58)
- [x] 1.4.0 release candidate: smoke test and evals, which are paid and need your go-ahead (#19, #101)
- [x] Release 1.4.0 through the release checklist (wi-891617)
- [ ] Amend `docs/plans/temper-2-rust-port.md` for the monorepo through Weigh-In: D1, D2, D3, D4, D10, D13, D14, D17 and invariant 3 (RU next steps)
- [ ] Write the `check raised|exceptions|gate-config|tests` JSON contracts against assay's fixtures (RU call D, assay #45)

* Not here: any Rust code for 2.0. The port starts in the monorepo.

**weigh-in**

- [x] Week 2 of the pilot (#23), the exit criteria, then 1.0.0 with no new code (B-23)
- [ ] 1.1: `weighin ci-check --json`, versioned, with `blocks` values and assay's reason codes (WI call C)
- [ ] Settle the ledger and commit `.kb/_ledger/journal/` in the last session before the move

* Not here: the atom path change to `.kb/decision/`. It lands with 2.0.0 (KN calls B and C).

**weigh-in-plugin**

- [ ] Nothing new. It keeps serving org sync for Weigh-In 1.x until the suite's release repository replaces it (see the calls).

**anneal**

- [ ] Keep #44's calibration run and cut the per-point verdict report, but do not tag or publish `v0.1.0` (BE call E)
- [ ] Land or close the open PRs (#113 and any others), so the import carries no half-finished branches
- [ ] Accept temper's `WIP` in the status hook, a small fix the pilots need now (BE pre-move list)

* Not here: lenient store reads, the skillcheck split and pointing merge-gate at `review.verdict`. Each is redone by Core, `tools/skillcheck` or Review's contract, so it is done once, in the monorepo.

**assay**

- [ ] Make `main` the default branch instead of `spec/architecture` (owner setting)
- [ ] Rename to `review`: crates, `REVIEW_*` env vars, schema ids `review.*/1`, the `review / verdict` check name and the App name in the docs (RV call A)
- [ ] Narrow the self trust path and the six-crate architecture tests to `crates/review/**`
- [ ] Fix the duplicate ADR-23 (spec ADR-23 and the mod's proposed ADR-23, #125)
- [ ] Triage the about 140 open issues into keep and close, so only live ones transfer

* Not here: owner setup #39 to #43. It happens once, after the move (see the calls).

**redkiln-rs**

- [x] Push the corpus to the private `redkiln-corpus` remote (#32). Do this first: today it exists only on your machine.
- [ ] Finish s8j and run one fix lane against a real model to reach M5. The lane is paid and needs your go-ahead.
- [ ] Land `build/phase-0-to-1` on `main` with `--no-ff`, after the checkpoints and CI pass (#24)
- [ ] Transplant the missing catalog checklists (#79), sourced from redkiln-old (call A)

* Not here: the `D:/repos` path fixes and the CI path greps. They change anyway on import, so they are fixed once, in the monorepo.

## Work sequence

The critical path runs through Weigh-In 1.1, then Core 1.0 at Bench's move, then Loop's M5; everything else runs beside it.

[embedded content: migration roadmap · 7 phases, each with its gate]

Read down the left column for the order: each phase starts when its gate clears, and the middle column is the work that clears it.

**Phase 0, starting now, in this order:**

1. Back up the Loop corpus (#32) and settle call A. These are the two losses that cannot be undone later.
2. Lay the monorepo skeleton: workspace `Cargo.toml`, toolchain 1.98.1, the `AGENTS.md` charter with crate map and layer rules, the structural layer test, path-filtered CI, the shared launcher and Backlog's label manifest. You set the ruleset, allowing merge commits.
3. Run the spikes, org sync first (BP call 11): org sync with several plugins, pushing `refs/redkiln/*` from cloud and Actions, a cloud bootstrap at pinned versions, R-SQLITE with 8 writers on Linux and Windows, and `libexec/` binaries in Cowork.
4. Draft SPEC v1 in `spec/`: store location, envelope, decision request record, trust levels, approval record, journal format and the CLI envelope. It stays text until move 1 gives it code.
5. In parallel, in the old repositories: temper 1.4.0, Weigh-In 1.0.0 and 1.1, the assay rename, Loop's M5 and landing, and anneal's calibration run.

**Then, in the monorepo, one move at a time:**

- **Move 1, Weigh-In:** `redkiln-core` from its store (ids, git, journal, happenstance 0.4.1 with the 0.1.x and 0.2.2 fixtures), `weighin-contract`, and `knowledge-contract` holding `atom.rs` byte for byte. Atoms move to `.kb/decision/` with `kb-` ids. Ship `rk-weigh-in` 2.0.0.
- **Move 2, Bench:** `anneal-ledger` conforms to Core, which freezes Core 1.0 and SPEC 1.0. Extract `redkiln-guard`, `redkiln-github` and `redkiln-calibrate`, move skill QA to `tools/skillcheck`, put the park bridge on `weighin-contract`, and freeze `rollout`, `perf` and `migrate`.
- **Move 3, Review:** `review-contract` with `review.findings/1`, then owner setup under Redkiln names and the required check on one consumer repository.
- **Move 4, Loop:** path fixes, the `redkiln-catalog` and `redkiln-dispatch` kits, and Loop's store on Core per the R-SQLITE result. Bench moves onto the catalog and ships `rk-bench` 0.1.0, and Backlog starts.
- **Rust 2.0:** starts after move 2 on Core 1.0, `redkiln-craft` and `redkiln-guard`. At parity it ships `rk-rust` 2.0.0, the root gate switches from vendored temper to it, and temper is archived.
- **Train 1:** the Knowledge and Backlog plugins, the `redkiln` bundle and the one org-sync release repository.

## How a move is done

Every move is the same eight steps, in one import PR plus a short tail. Only steps 1, 7 and 8 need you.

1. **Prepare the target (owner).** The monorepo's ruleset allows merge commits for import PRs, the old repository's labels exist in the monorepo through Backlog's label manifest (BL call A), and the required-check names for the new package are added.
2. **Import without rewriting.** One merge commit whose second parent is the old repository's tip, unchanged, with its files placed in the suite layout by `git read-tree --prefix` (crates to `crates/<pkg>/`, plugin to `plugins/<pkg>/`, docs to `docs/<pkg>/`). No squash, rebase or `filter-repo`, so every hash cited in ADRs, PROGRESS logs and decision records still resolves in the monorepo. Tags are not imported, because old `v1.0.0` tags would collide; they stay in the archived repository.
3. **Rename at the door.** Install ID `rk-<pkg>`, `.redkiln/<pkg>/` config, `<pkg>.*` event names and the `rk-<pkg>--v<version>` tag scheme land in a second commit, so the import commit stays a pure move.
4. **One gate, one baseline.** The four vendored `.claude/temper/` copies are deleted. Until Redkiln Rust 2.0 dogfoods, the monorepo runs temper 1.4.0 vendored once at the root, and each package's legacy findings are re-baselined at their new paths (Loop's 6,606 among them).
5. **Records keep their series.** ADRs, D-, B- and AC records move under `docs/<pkg>/` with their prefixes, never renumbered. The `.kb/_intake/decisions/` atoms move to `.kb/decision/` with their ids (KN call B).
6. **State is imported, never deleted.** The first run of the new binary imports the old ledger (`<git-common-dir>/weighin/`, `anneal/`, `.git/temper/`) and committed journals into `<git-common-dir>/redkiln/events.sqlite3` once. The old files stay, because Cowork refuses deletes.
7. **Issues transfer (owner or agent with admin).** Open issues move with GitHub's issue transfer after triage; closed ones stay where their PR links point.
8. **Archive with a redirect (owner).** The old repository gets a last commit whose README and marketplace entry point to `rk-<pkg>` in the `redkiln` marketplace, then it is archived. For Weigh-In and temper, which have users, the last 1.x release also prints a one-line SessionStart notice naming the new install.

## Risks and calls for Ryan

Calls A, C and F are settled; B, D and E proceed on their recommendations until you say otherwise.

| Call | Question | Recommendation | Flips if |
| --- | --- | --- | --- |
| A | Does another copy of the original Node Redkiln (`redkiln@e61b68f`) exist? | **Settled 2026-10-18:** it is `Wet-Ink-Corporation/redkiln-old`, which holds `e61b68f` (the 0.21.0 release merge, 2026-08-31). #79 sources its checklists from there, and redkiln-old stays read-only and is never deleted. | none |
| B | Does Review's owner setup (#39 to #43) happen before or after the move? | After. The App, `review-ledger` repository, approver, agent identity and environments are created once, with Redkiln names, against the monorepo | You need Review running on a consumer repository before Loop arrives |
| C | Which repository does claude.ai org sync read for the suite? | **Decided 2026-10-19 (wi-f041fb):** rename `weigh-in-plugin` into the suite's release repository once the org-sync spike passes; it serves Weigh-In 1.x until then | The spike shows org sync takes one plugin per repository: then one release repository per plugin |
| D | Does `rk-bench` 0.1.0 wait for the catalog? | Yes. Bench call A retires anneal's templates, and shipping on them first means a second migration for every user | Loop's M5 slips well past Bench's move: ship 0.1.0 on anneal's templates and switch in 0.2 |
| E | What gates Rust in the monorepo before Redkiln Rust 2.0 exists? | temper 1.4.0, vendored once at the root, with each package re-baselined at its new paths | none expected |
| F | Go-ahead for the paid runs on the critical path | **Decided 2026-10-19 (wi-b94a5e):** go ahead on both. Agents run temper 1.4.0's smoke test and evals, and Loop's first real fix lane for M5, once each one's prerequisites are green, and report the results | none |

| Risk | Effect | Fallback |
| --- | --- | --- |
| Loop's M5 slips | Move 4 and `rk-bench` 0.1.0 wait | Moves 1 to 3 and the Rust port go on; see call D |
| R-SQLITE fails under TRUNCATE with 8 writers | Loop cannot share the log directly | Loop keeps tier-2 events in its own WAL file and exports a journal (BP call 1) |
| Pushing `refs/redkiln/*` fails from cloud or Actions | Journals cannot live in refs | Committed `.redkiln/journal/<session>.jsonl` (PT call A) |
| happenstance 0.3.2 to 0.4.1 breaks an old ledger | A user loses decisions on upgrade | Core carries the 0.1.x and 0.2.2 fixtures before Weigh-In 2.0 ships, and the import never deletes the source |
| SPEC one-way items (decision request, trust levels) are wrong | Every plugin builds on them | SPEC 1.0 freezes only at move 2, with two real users |
| temper 1.4.0 slips | The Rust port starts later | No move depends on it |
