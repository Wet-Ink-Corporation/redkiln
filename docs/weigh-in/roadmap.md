# Roadmap and open items

State at 1.2.0 (2026-11-01). v1 is slice 1 made dependable, and every exit criterion of the
[v1 plan](v1-plan.md) is met, with one week of daily use instead of two
([D-0019](decisions/build-decisions.md#d-0019)). 1.0.0 shipped it on 2026-10-19
([B-27](decisions/build-decisions.md#b-27)). 1.1.0 (2026-10-29) shipped the first of slice 2: the
CI contract for assay ([#145](https://github.com/Wet-Ink-Corporation/weigh-in/issues/145), B-28)
and the Claude Code mod ([#112](https://github.com/Wet-Ink-Corporation/weigh-in/issues/112)): the
band and `/wi` (#114, B-29), the local board pane (#115, B-30, B-32), decisions shown on edits
(#116), and the owner from git config (B-31). The repository is public since 2026-10-30, and CI
runs on GitHub-hosted Linux, Windows and macOS runners ([B-33](decisions/build-decisions.md#b-33)).

Since 1.1.3 (2026-10-31) the mod ships as its own plugin, `weigh-mod`, because a desktop chat
turned off the whole `weigh` plugin for it ([B-35](decisions/build-decisions.md#b-35)).

1.2.0 (2026-11-01) ships the 2026-11-01 audit's fixes, #169 to #173 below, all closed; the
Cowork fetch script was confirmed in a real Cowork sitting before the release (D-0021).

**Where to pick up.** Status lives on the issues; this page only links them. The mod's Cowork
and Windows runs passed ([#113](https://github.com/Wet-Ink-Corporation/weigh-in/issues/113),
closed), and V2 from Cowork was closed as not planned
([#17](https://github.com/Wet-Ink-Corporation/weigh-in/issues/17), 2026-10-31). The 2026-11-01
audit before the redkiln move filed
[#169](https://github.com/Wet-Ink-Corporation/weigh-in/issues/169) (typed class and board),
[#170](https://github.com/Wet-Ink-Corporation/weigh-in/issues/170) (`scripts/seed.py`, wi-100d6d),
[#171](https://github.com/Wet-Ink-Corporation/weigh-in/issues/171) (surface routing out of the
skill), [#172](https://github.com/Wet-Ink-Corporation/weigh-in/issues/172) (split `main.rs` and
`ops.rs`) and [#173](https://github.com/Wet-Ink-Corporation/weigh-in/issues/173) (decisions
age). Integration with redkiln (the `decision` item type, kb-ingest, where the ledger's truth
lives) is settled on the redkiln side. The rest of slice 2 and later is the backlog on
[#27](https://github.com/Wet-Ink-Corporation/weigh-in/issues/27), starting with the research
clean-up, the rest of going public. The Cowork download is on from
1.1.0 (B-33) and was confirmed in a Cowork sitting before 1.2.0 (D-0021). Per-category suppression
([#25](https://github.com/Wet-Ink-Corporation/weigh-in/issues/25)) was not built: the pilot's
not-useful rate was 0%, under D-0013's 10% trigger. Left open on purpose: whether a cloud session
in this checkout captures twice when the organization-synced plugin also loads (noted on #143),
and the capture items held back for the pilot on #26.

v1 exit criteria ([v1 plan](v1-plan.md), Phase 3), at 1.0.0:

| Criterion | Status |
|---|---|
| Action rate at least 60%, not-useful at most 10% | **Met:** week 1 of [#23](https://github.com/Wet-Ink-Corporation/weigh-in/issues/23), six folders: 90 presented, 73% action rate, 0% not useful; [#25](https://github.com/Wet-Ink-Corporation/weigh-in/issues/25) stays out |
| Marker capture precision at least 80% | **Met:** 15 of 15 on the seed (110 items, 27 labelled `capture`), so the bar is in force (wi-a9df17); every label up to 83 items reviewed by Ryan on 2026-10-11 ([#22](https://github.com/Wet-Ink-Corporation/weigh-in/issues/22)) |
| No "pending" reported as "done"; one sitting returns "no decisions need you" | **Met:** sittings whose writes failed said nothing was recorded; sittings on 2026-10-18 returned "no decisions need you" |
| Every V-item closed or deferred by a decision | **Met:** V1, V6 and V7 passed their re-checks on 0.9.0 on 2026-10-18 ([#16](https://github.com/Wet-Ink-Corporation/weigh-in/issues/16), [#20](https://github.com/Wet-Ink-Corporation/weigh-in/issues/20), [#21](https://github.com/Wet-Ink-Corporation/weigh-in/issues/21)); V2 deferred (B-17), V3 moot (B-24), V4 and V5 closed |
| No ledger corruption, no unexplained `hook-errors.log` lines | **Met:** 0 hook errors across six ledgers in week 1 |
| Each machine installs without Rust; Cowork through the connected checkout | The `marketplace` branch: run on Windows and in Cowork ([#13](https://github.com/Wet-Ink-Corporation/weigh-in/issues/13)); macOS and Linux unverified. The organization-synced `weigh-in-plugin` copy works on Windows and in Cowork ([#76](https://github.com/Wet-Ink-Corporation/weigh-in/issues/76)). Projects tasks take the binary from a connected checkout kept at the current release (#143) |
| A 0.2.2 ledger opens under the candidate | **Met:** fixture tests pass at 1.0.0 (B-10) |

Open work is tracked as GitHub issues ([D-0011](decisions/build-decisions.md#d-0011)), in the order of
the [v1 plan](v1-plan.md). This page summarises them; the issue is where status is kept.

| Phase | Issues |
|---|---|
| 0: organise the repository | [#5](https://github.com/Wet-Ink-Corporation/weigh-in/issues/5) rulesets on `main` and `v*` tags · [#28](https://github.com/Wet-Ink-Corporation/weigh-in/issues/28) scaffolding, CI coverage, issue tracking · [#6](https://github.com/Wet-Ink-Corporation/weigh-in/issues/6) move the temper research |
| 1: use it here, upgrade safety (0.3.0) · **done** | [#7](https://github.com/Wet-Ink-Corporation/weigh-in/issues/7) use Weigh-In on this repository · [#8](https://github.com/Wet-Ink-Corporation/weigh-in/issues/8) ledger schema version · [#9](https://github.com/Wet-Ink-Corporation/weigh-in/issues/9) SessionStart says when capture is off · [#10](https://github.com/Wet-Ink-Corporation/weigh-in/issues/10) `weighin.cmd` picks the newer build · [#11](https://github.com/Wet-Ink-Corporation/weigh-in/issues/11) release |
| 2: distribution (0.4.x) · **done** | [#12](https://github.com/Wet-Ink-Corporation/weigh-in/issues/12) `marketplace` branch from `release.yml` · [#13](https://github.com/Wet-Ink-Corporation/weigh-in/issues/13) install from it; Cowork through a connected checkout · [#14](https://github.com/Wet-Ink-Corporation/weigh-in/issues/14) install guide · [#15](https://github.com/Wet-Ink-Corporation/weigh-in/issues/15) release · fixes from the installs: [#43](https://github.com/Wet-Ink-Corporation/weigh-in/issues/43) release rerun, [#46](https://github.com/Wet-Ink-Corporation/weigh-in/issues/46) auto-update, [#48](https://github.com/Wet-Ink-Corporation/weigh-in/issues/48) 0.4.1 |
| 3: verify and use (0.9.x) · **done** | [#16](https://github.com/Wet-Ink-Corporation/weigh-in/issues/16) V1 · [#17](https://github.com/Wet-Ink-Corporation/weigh-in/issues/17) V2 · [#18](https://github.com/Wet-Ink-Corporation/weigh-in/issues/18) V4 · [#19](https://github.com/Wet-Ink-Corporation/weigh-in/issues/19) V5 · [#20](https://github.com/Wet-Ink-Corporation/weigh-in/issues/20) V6 · [#21](https://github.com/Wet-Ink-Corporation/weigh-in/issues/21) V7 and hooks without Git Bash · [#22](https://github.com/Wet-Ink-Corporation/weigh-in/issues/22) eval seed · [#23](https://github.com/Wet-Ink-Corporation/weigh-in/issues/23) two weeks of daily use · [#24](https://github.com/Wet-Ink-Corporation/weigh-in/issues/24) sitting cost (**done**, D-0016) · [#25](https://github.com/Wet-Ink-Corporation/weigh-in/issues/25) suppression, only if not-useful exceeds 10% |
| 4: v1.0 · **done** (1.0.0, 2026-10-19) | [#26](https://github.com/Wet-Ink-Corporation/weigh-in/issues/26) release 1.0.0 and record v1 live |
| Slice 2, first release (1.1.0, 2026-10-29) | [#145](https://github.com/Wet-Ink-Corporation/weigh-in/issues/145) CI contract for assay · [#112](https://github.com/Wet-Ink-Corporation/weigh-in/issues/112) the Claude Code mod: [#114](https://github.com/Wet-Ink-Corporation/weigh-in/issues/114) band and `/wi`, [#115](https://github.com/Wet-Ink-Corporation/weigh-in/issues/115) board pane, [#116](https://github.com/Wet-Ink-Corporation/weigh-in/issues/116) decisions on edits, [#113](https://github.com/Wet-Ink-Corporation/weigh-in/issues/113) its Cowork and Windows runs |
| After 1.1.0 | [#27](https://github.com/Wet-Ink-Corporation/weigh-in/issues/27) slice 2 and later |

## Not yet verified

| Id | What | Issue |
|---|---|---|
| V1 | Session identity in Cowork: whether `${CLAUDE_SESSION_ID}` is substituted in a synced skill | [#16](https://github.com/Wet-Ink-Corporation/weigh-in/issues/16) |
| V2 | ArtifactData reads and writes from Cowork. **Deferred to slice 2** on 2026-10-05 ([B-17](decisions/build-decisions.md#b-17)): the board does not run from Cowork in v1 | [#17](https://github.com/Wet-Ink-Corporation/weigh-in/issues/17) |
| V4 | Editor links (`vscode://file/…`) opened from inside an Artifact. **Closed 2026-10-05: not supported.** On Windows, `vscode://` and `claude-cli://` links do nothing with `target=_top` and blank the frame without it; `https://` opens in the browser. The board keeps selectable `path:line` text | [#18](https://github.com/Wet-Ink-Corporation/weigh-in/issues/18) |
| V5 | Quotas of the board's `db` store. One document holds at most 256 KiB, which about 700 defaults overflowed; 0.5.1 caps the board at 200 ([#61](https://github.com/Wet-Ink-Corporation/weigh-in/pull/61)). **Run 2026-10-05:** the limit is exactly 262,144 bytes; a 1,000-default ledger exports 59,678 bytes (200 shown) and round-trips through `ArtifactData`; the store allows 25,000 documents per artifact. **Closed 2026-10-05**: Ryan saw the probe render the capped state | [#19](https://github.com/Wet-Ink-Corporation/weigh-in/issues/19) |
| V6 | Previews in the desktop Code tab and the Cowork question card. **Code tab, 2026-10-05:** a sitting's card worked, and the selected option's preview came back with the answer. The Cowork card is checked during V1's run | [#20](https://github.com/Wet-Ink-Corporation/weigh-in/issues/20) |
| V7 | The Windows binary from the installed plugin on Ryan's machine. **Verified 2026-10-03** ([#13](https://github.com/Wet-Ink-Corporation/weigh-in/issues/13)): installed from the `marketplace` branch, hooks fired under Git Bash, a sitting decided an item. Hooks without Git Bash: exec-form hooks from 0.5.0 ([B-16](decisions/build-decisions.md#b-16)), spiked on Linux, macOS and Windows; checked from the 0.5.0 release on 2026-10-03, including Cowork keeping the launcher's executable bit; still to confirm a marker captured on a Windows session without Git Bash. The "no binary" failure of [#76](https://github.com/Wet-Ink-Corporation/weigh-in/issues/76) came from a claude.ai-synced copy of `main`, which has no binaries; only `weigh-in-plugin` is synced, and the launcher now names that case ([B-18](decisions/build-decisions.md#b-18)) | [#21](https://github.com/Wet-Ink-Corporation/weigh-in/issues/21) |

## Known gaps

- **The board URL lands in the tracked config:** publishing the board writes its URL into
  `.kb/_ledger/config.json`. Keep it local (`git update-index --skip-worktree`), because a
  committed URL would point every clone's ledger, cloud containers included, at one board
  ([B-20](decisions/build-decisions.md#b-20)).
- **No binaries on `main`:** install from the `marketplace` branch instead ([B-14](decisions/build-decisions.md#b-14));
  verified on Windows and in Cowork ([#13](https://github.com/Wet-Ink-Corporation/weigh-in/issues/13)); macOS and Linux installs are unverified.
  A plugin without a binary says capture is off ([B-11](decisions/build-decisions.md#b-11)).
- **Cowork download confirmed:** on from 1.1.0 ([B-33](decisions/build-decisions.md#b-33)), and confirmed in a
  Cowork sitting before 1.2.0 ([D-0021](decisions/build-decisions.md#d-0021)). 1.0.0 and earlier cannot download ([D-0012](decisions/build-decisions.md#d-0012)), so Cowork uses a connected checkout of the `marketplace` branch ([#13](https://github.com/Wet-Ink-Corporation/weigh-in/issues/13)).
- **Two plugins in Claude Code:** the band and `/wi` need `weigh-mod@weigh-in` installed beside
  `weigh` ([B-35](decisions/build-decisions.md#b-35)).
- **Desktop chats run no hooks:** since Chat and Cowork merged, a chat connected to a folder
  ignores plugin hooks, so SessionStart's marker convention never reaches it and capture there
  rests on the sitting's sweep. Ask for a sitting in words or type `/weigh-in`
  ([B-34](decisions/build-decisions.md#b-34), [#113](https://github.com/Wet-Ink-Corporation/weigh-in/issues/113)).
- **This repository's ledger does not outlive a cloud session** ([D-0014](decisions/build-decisions.md#d-0014)):
  cloud sessions settle or defer everything before they end. Ryan's own clones, now installed from
  the `marketplace` branch, keep theirs. Since [B-21](decisions/build-decisions.md#b-21) the
  event history survives as a committed journal per cloud session, which `weighin stats` folds
  in, so cloud sittings count toward [#23](https://github.com/Wet-Ink-Corporation/weigh-in/issues/23).
  Cloud sittings before the change are lost, apart from their decisions.
- **No board from Cowork:** slice 2 ([#27](https://github.com/Wet-Ink-Corporation/weigh-in/issues/27)).
- **happenstance is held at 0.3.2**, now with a way past it: a schema change is one more ledger
  migration step ([B-10](decisions/build-decisions.md#b-10)).
- **The default owner comes from git** (`user.name`, else the email's local part, else `ryan`) unless `init --owner` sets it ([B-31](decisions/build-decisions.md#b-31), [#27](https://github.com/Wet-Ink-Corporation/weigh-in/issues/27)).
- **Sitting cost follows session length:** about 3 × the context at the cache-read price plus
  about 5,600 output tokens, recorded as the v1 figure ([D-0016](decisions/build-decisions.md#d-0016), [#24](https://github.com/Wet-Ink-Corporation/weigh-in/issues/24)).
- **Agents marked few of their own calls before 0.5.2.** On the eval seed at 72 items (20 labelled
  `capture`), recall was 1 of 7 before 0.5.2's one-block-per-call line and 12 of 13 after it.
  At 83 items (24 labelled `capture`, all reviewed by Ryan) precision is 14 of 14 and recall 14 of 24 ([#22](https://github.com/Wet-Ink-Corporation/weigh-in/issues/22)). The
  sessions after 0.5.2 are few and all on this repository, so the sitting's sweep still catches
  what markers miss.
- **kb-ingest does not read Weigh-In atoms yet.** The atoms pass Redkiln's `validate --kb`;
  the atoms become Knowledge's format (`knowledge-contract`) with the move into the `redkiln`
  monorepo ([B-36](decisions/build-decisions.md#b-36), superseding the redkiln-rs destination of
  [B-22](decisions/build-decisions.md#b-22), amending [wi-e15c77](decisions/wi-e15c77.md)).
- **Open from wi-4d8b6a:** which provider the separately held model key is for; slice 2 ([#27](https://github.com/Wet-Ink-Corporation/weigh-in/issues/27)).

## Slice 2

The next milestone after v1. Not in v1 ([D-0013](decisions/build-decisions.md#d-0013)), except per-category suppression if
the not-useful rate exceeds 10% ([#25](https://github.com/Wet-Ink-Corporation/weigh-in/issues/25)).
The items, with the design's estimates, are in the [v1 plan](v1-plan.md#decisions) and tracked in
[#27](https://github.com/Wet-Ink-Corporation/weigh-in/issues/27).
