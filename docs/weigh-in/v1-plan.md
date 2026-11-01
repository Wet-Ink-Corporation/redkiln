# Plan: from 0.2.2 to v1 in active use

Evaluation of the repository and its source control on 2026-10-02, and the plan that follows from
it. It starts with getting the repository organised for ongoing development and ends with v1
live. The calls it needs are listed under [Decisions](#decisions); those taken are recorded in
`docs/decisions/build-decisions.md`.

## Where Weigh-In stands

**Update, 2026-10-19 (1.0.0).** Phases 0 to 3 are done and every exit criterion is met, with one
week of use instead of two ([D-0019](decisions/build-decisions.md#d-0019)); 1.0.0 is 0.9.0 plus
the #143 skill fix. **Update, 2026-10-11 (0.6.0).** Phases 0 to 2 are done and Phase 3 is under way; the current
state and the exit criteria are kept in [`roadmap.md`](roadmap.md). The release path is set by
[B-23](decisions/build-decisions.md#b-23): 0.6.x patches during the two weeks of use, 0.9.0 as
the release candidate, and 1.0.0 with no new code. The rest of this section is the evaluation
of 2026-10-02.

**Built and working (slice 1).** Capture by hooks at zero model tokens, the in-chat sitting
(`/weigh-in`), the five filters and card guards in the CLI, `.kb` atoms with source annotation and
commit trailers, the Artifact board for own repos, the Cowork route through the computer's shell,
and the TRUNCATE-journal ledger that survives Cowork's refused deletes. About 7,600 lines of Rust, tests included,
over two crates (`weighin-core` with no I/O, `weighin` the CLI), an 80-line skill and a 41-line
launcher.

**Gate is green.** Checked on this branch at `c54139a`: `cargo test --workspace` passes 57 tests
(31 core, 18 store/git/paths, 8 end to end), and `cargo fmt --check`, clippy with `-D warnings`,
rustdoc with `-D warnings` and `claude plugin validate .` are all clean. CI (`ci.yml`) has passed
on every run on `main` since it was added, on Linux and Windows.

**Released once.** `v0.2.2` is tagged and published (Linux musl, Windows msvc, macOS arm64,
the zip and `SHA256SUMS`). The first release run failed on a slow Windows runner and passed on
re-run; `ae3ccb0` widened the launcher test's deadline to stop it recurring.

**Decisions are well kept.** Seven board decisions (MADR, one file per `wi-` id), D-0001 to
D-0007 and B-01 to B-08 in `build-decisions.md`, each with provenance and what it supersedes.
`docs/roadmap.md` lists the open verifications (V1, V2, V4–V7), known gaps and slice 2.

**Measured in use, but only in trials.** Live Claude Code sittings on Sonnet cost $0.42–0.57
against a $0.28 design estimate. No repository records a sustained run of real sittings, and
this repository does not use Weigh-In on itself: it has no `.kb/`, and its decisions are kept
by hand.

### Source control and repository setup

| Area | Now | Gap |
|---|---|---|
| History | 13 commits on `main`, linear, clear messages; pre-GitHub history kept only in a local `archive/pre-github` branch (B-06) | That branch names a client and must never be pushed; nothing enforces that beyond memory |
| Branching | Ryan's commits go straight to `main` (`ae3ccb0`, `60c5620`, `c54139a`); only Dependabot opens PRs | No ruleset requiring a PR and green CI on `main`; CI catches a break only after it has landed |
| Tags and releases | `v*` tag triggers `release.yml`, which checks the versions agree with the tag | Any `v*` tag publishes, from any commit; nothing checks the tagged commit is on `main`; no tag protection |
| CI coverage | fmt, clippy, tests, rustdoc on Linux and Windows | macOS is built for release but never tested, though B-08's symlinked-temp-dir case is macOS's; `claude plugin validate` and `package-plugin.sh` (stamp check, no `plugin/bin`) run only by hand |
| Dependencies | Dependabot weekly for actions and Cargo; majors and happenstance held back | No `cargo audit` / `cargo deny`; happenstance pin has no exit path yet (B-01) |
| Work tracking | `docs/roadmap.md` only; no issues, labels or milestones on GitHub | Open items cannot be assigned, linked from PRs or closed by a commit |
| Contributor scaffolding | `CLAUDE.md` holds the gate and the hard-won rules | No PR template, `CHANGELOG.md` or CODEOWNERS; release notes are a fixed sentence |
| Repository scope | `docs/research/reports/Shipping native binaries in plugins.md` and its notes (`60c5620`, `c54139a`) are mostly about temper | Research for another project lives here; temper's own repository is its natural home, with a link back |
| Cloud sessions | Toolchain is pinned (`rust-toolchain.toml`, 1.98.1) | No SessionStart hook for Claude Code cloud sessions, so a fresh session depends on the image having the toolchain |

### What stands between 0.2.2 and active use

1. **A git or marketplace install has no binaries.** `plugin/libexec/*/` is ignored, so
   `/plugin install weigh@weigh-in` from the repository gets launchers and no `weighin`. The
   launcher then exits 0 for every hook (`plugin/libexec/weighin`, last lines), so capture is
   silently off. Today it works only after `scripts/build-plugin-bins.*` on each machine, or from
   the uploaded zip.
2. **Failures are silent.** Hook errors go to `<state dir>/hook-errors.log` and the hook exits 0
   (`crates/weighin/src/hooks.rs`). That is right for a fail-soft ledger, but nothing ever tells
   the user that capture has been off. The native-binaries research names this as the one thing
   temper must not copy.
3. **Cowork download is dormant** while releases are internal (D-0003, D-0007): Cowork works only
   with a connected folder holding this repository.
4. **Six surfaces are unverified** (V1, V2, V4–V7), including the Windows binary from an installed
   plugin (V7) and the Cowork session id (V1).
5. **No upgrade guarantee.** The ledger copies happenstance 0.3.2's schema (B-01) with no schema
   version or migration step; a v1 promise to keep a user's ledger needs one, and a test that opens
   a 0.2.x ledger.
6. **Single-user defaults.** The owner is `ryan` unless `init --owner` sets it
   (`crates/weighin/src/env.rs:93`), so records say `human:ryan` for anyone else.
7. **Small launcher drift.** `weighin.cmd` prefers the msvc build whatever its age, while the `sh`
   launcher takes the newer build (B-05); the release ships only msvc, so this bites only local builds.
8. **Sitting cost** is roughly double the estimate. Not a blocker, but v1 should either set the
   measured figure as the target or trim the batch and card output.

## Decisions

Ryan answered all six on 2026-10-02; they are recorded as D-0008 to D-0013 in
[`build-decisions.md`](decisions/build-decisions.md).

| # | Question | Answer | Effect on the plan |
|---|---|---|---|
| P-1 | What does "v1 live" mean? | **Daily use by Ryan** (D-0008) | Exit criteria are Ryan's own machines and Cowork; no install path for other people, and the `ryan` owner default stops blocking |
| P-2 | Where do installable binaries come from outside the zip? | **A built marketplace in this internal repository** (D-0009) | The release workflow publishes the built plugin to a `marketplace` branch here |
| P-3 | Should release assets be public? | **Stay internal for v1** (D-0012) | One repository; Cowork through a connected checkout |
| P-4 | Does this repository use Weigh-In on itself? | **Yes** (D-0010) | Phase 1, step 1 |
| P-5 | Where does open work live? | **GitHub Issues** (D-0011) | Phase 0, step 3 |
| P-6 | Is any slice 2 work in v1? | **None, unless not-useful exceeds 10%** (D-0013) | Per-category suppression is conditional in Phase 3 |

**P-3: public assets, and whether cleaning up the research avoids a second repository.** The
second repository was only ever a way to make release assets public. Nothing else needed it. So
cleaning up the research doesn't change how many repositories there are: it is what would let
*this* one go public, and one repository is the end state either way. With v1 for Ryan alone,
nothing needs to be public:

- Claude Code installs from a private GitHub marketplace with the git credentials already on
  the machine (`gh auth setup-git` or SSH). Background auto-update uses the same credential
  helper and fails quietly without one; a bare `GITHUB_TOKEN` is not enough on its own
  ([docs](https://code.claude.com/docs/en/plugins/install.md#add-a-private-marketplace)).
- Cowork takes the plugin as an uploaded zip, which Ryan downloads from the internal release.
  On the computer's shell, the skill finds the Linux binary in a connected folder
  (`$HOME/mnt/*/libexec/x86_64-unknown-linux-musl/weighin`). A local checkout of the
  `marketplace` branch, connected once, provides that binary with no build and no download. Keep that checkout at the current release: the skill skips a binary of any other version (#143).

Going public later means cleaning up the research (it names IQBG and client-repo policy, in
`docs/research/` and `wi-7c21e4`), then making this repository public and packaging without
`--no-download`. The Cowork download then works with no more changes. That is a post-v1 step.

**P-6: what slice 2 is.** The design ([report](research/report.md#first-slice-and-what-follows-lower-bound-estimates))
had three slices. Slice 1 has been built, plus slice 2's board. What remains, with the design's
lower-bound estimates:

| Item | What it does | Estimate |
|---|---|---|
| Sonnet grounder and stability check | A second model writes or checks each card's grounding, and the recommendation is re-judged three times with the options shuffled; an unstable one is shown options-first | 2.5 h |
| `weighin serve --wait` | A local review page for client repos, where the Artifact board is off ([wi-7c21e4](decisions/wi-7c21e4.md)) | 2.5 h |
| Metrics and per-category suppression | Stop raising item kinds that keep being dismissed `not-useful` (`weighin stats` already measures this) | 1.5 h |
| Just-in-time surfacing | A hook shows a past decision when an edit touches the code it is anchored to | 1 h |
| Board from Cowork | The Artifact board in Cowork, staging state through the device shell | not estimated |
| Reliance drills | 1 in 30 two-way cards recommends a weaker option, to check you are not rubber-stamping ([wi-2b90f3](decisions/wi-2b90f3.md)) | not estimated |
| Door classifier for unattended runs | An independent model judges one-way or two-way before an unattended run defaults anything ([wi-4d8b6a](decisions/wi-4d8b6a.md)) | not estimated |
| Slice 3: Redkiln-native | `decision` item type in redkiln-rs, kb-ingest of intake keys, AFK gating, a "Needs you" column, an SDK pause loop ([wi-e15c77](decisions/wi-e15c77.md)) | 15 h |

Decided (D-0013): none of it is in v1, with one conditional exception. If Phase 3's not-useful rate
misses its 10% bar, per-category suppression comes into v1, because it is the design's fix for
exactly that miss. Everything else serves client repos, unattended runs or Redkiln, and v1 needs
none of those for Ryan's daily use.

## The plan

Five phases. Sizes are rough working days for one person with an agent; phases 0 and 1 can
overlap. Each phase ends at a version so progress is visible in releases.

### Phase 0 — Organise the repository (about 1 day) → no release

The aim is that every later change arrives by PR, is gated, and is tracked.

1. **Ruleset on `main`** (GitHub settings): require a pull request, require the `ci` checks
   (`check (ubuntu-latest)`, `check (windows-latest)`) to pass, block force pushes and deletion,
   squash or rebase merges only so history stays linear. Ryan may bypass, for emergencies only.
   (Superseded by D-0020: PRs now land as merge commits, and linear history is no longer required.)
2. **Tag ruleset for `v*`**: only Ryan creates them. Add a step to `release.yml` that fails unless
   the tagged commit is an ancestor of `origin/main`.
3. **Work tracking (P-5)**: labels `verify`, `gap`, `release`, `decision`, `slice-2`, `deps`;
   a `v1.0` milestone; one issue per V-item, known gap and phase task below. `roadmap.md`
   becomes a short summary that links them.
4. **Contributor scaffolding**: `.github/pull_request_template.md` (what changed, the gate run,
   decisions touched, version bump yes/no); `CHANGELOG.md` starting at 0.2.2, written from the
   commit messages; `.github/CODEOWNERS` with Ryan as owner. Keep `CLAUDE.md` as the one
   contributor guide rather than adding `CONTRIBUTING.md`.
5. **CI additions**: a macOS leg in `ci.yml`; a `plugin` job that runs `claude plugin validate .`
   and `scripts/package-plugin.sh --no-download` against a Linux build (stamp count, no
   `plugin/bin`, zip produced); `cargo deny check` (advisories, licences) as its own job.
6. **Release notes from the changelog**: `release.yml` takes the `## <version>` section of
   `CHANGELOG.md` as the release body and fails when it is missing.
7. **Housekeeping** (done 2026-10-02: the research is in temper, with
   [a summary here](research/shipping-native-binaries.md)): move the temper research (`docs/research/reports/Shipping native binaries in
   plugins.md` and its notes) to temper's repository, keeping a one-paragraph summary of the
   Weigh-In findings here and linking it from `docs/README.md`. Write a pre-push hook, or a note
   in `CLAUDE.md`, that stops `archive/pre-github` from ever being pushed.
8. **Cloud sessions**: a SessionStart hook in `.claude/settings.json` that installs the pinned
   toolchain and warms the cargo cache, so any Claude Code cloud session can run the gate.

Done when: a test PR cannot merge red, an unprotected tag cannot release, every roadmap item is an
issue in the `v1.0` milestone, and CI covers macOS and the plugin package.

### Phase 1 — Dogfood and upgrade safety (about 2–3 days) → 0.3.0

1. **Use Weigh-In here (P-4)**: `weighin init --class own` in this repository, commit `.kb/`, and
   run `/weigh-in` at the end of every working session on it from now on. New decisions get atoms;
   `docs/decisions/` stays as the record of everything before.
2. **Ledger schema version**: stamp `PRAGMA user_version` in `Store::open`, add a migration step
   (a no-op for the first version), and add fixture tests that open 0.1.x WAL and 0.2.2 TRUNCATE
   ledgers and read them. This gives happenstance a way past 0.3.2 (B-01).
3. **Make silent failure visible**: when the launcher finds no binary, or `hook-errors.log` has new
   lines since the last session, SessionStart prints one line saying capture is off and why.
   Hooks still exit 0.
4. **Launcher parity**: `weighin.cmd` picks the newer of msvc and gnu, as the `sh` launcher does (B-05).
5. Bump to 0.3.0 and release.

**Done 2026-10-02** (0.3.0; released when Ryan pushes `v0.3.0`). Each step landed as its own PR:
[#31](https://github.com/Wet-Ink-Corporation/weigh-in/pull/31) launcher parity (#10),
[#32](https://github.com/Wet-Ink-Corporation/weigh-in/pull/32) ledger schema version and 0.1.x/0.2.2
upgrade fixtures (#8, [B-10](decisions/build-decisions.md#b-10)),
[#33](https://github.com/Wet-Ink-Corporation/weigh-in/pull/33) SessionStart says when capture is off
(#9, [B-11](decisions/build-decisions.md#b-11)),
[#34](https://github.com/Wet-Ink-Corporation/weigh-in/pull/34) Weigh-In used here (#7; ledger at the
default location, [D-0014](decisions/build-decisions.md#d-0014); cloud sessions wired through the
repository because they do not install plugins, [B-12](decisions/build-decisions.md#b-12)), and
[#35](https://github.com/Wet-Ink-Corporation/weigh-in/pull/35) the 0.3.0 bump (#11). What differs from
the steps above: the 0.2.2 fixture is built in the test from event payloads a real 0.2.2 ledger
holds, rather than a committed database; and SessionStart reports through `systemMessage`, since
plain SessionStart output reaches only Claude.

### Phase 2 — Distribution from the internal repository (about 2 days) → 0.4.0

1. **Marketplace branch (D-0009)**: after publishing the release, `release.yml` force-pushes a
   single-commit `marketplace` branch. It holds `.claude-plugin/marketplace.json` and `plugin/`,
   with the Linux musl, Windows msvc and macOS arm64 binaries in `libexec/<target>/`, committed
   executable. Replacing the commit each release keeps old binaries out of the history that
   clones download. The workflow needs `contents: write`, which it already has. Exclude the
   branch from the `main` ruleset, and allow only the workflow to push it.
2. **Install from it**: `/plugin marketplace add Wet-Ink-Corporation/weigh-in#marketplace`, then
   `/plugin install weigh@weigh-in`, on each of Ryan's machines with `gh auth setup-git` done.
   Check that hooks fire with no toolchain installed, and that background update picks up the
   next release. The docs don't say whether a plugin keeps its executable bit in the cache; the
   launcher already sets it (`chmod +x`) when it is missing.
3. **Cowork binary**: keep a local checkout of the `marketplace` branch and connect it once to
   Cowork, so the skill's connected-folder fallback finds the Linux binary. Upload the release
   zip under Customize > Plugins. Releases stay `--no-download` (D-0012).
4. **Developer route unchanged**: `main`'s `marketplace.json` and `scripts/build-plugin-bins.*`
   stay for `--plugin-dir` and local builds.
5. **Install guide**, separate from the developer build steps in `README.md`: install, update,
   uninstall, where the ledger is, how to read `hook-errors.log`.
6. Bump to 0.4.0 and release.

**Done 2026-10-03** (0.4.0 and 0.4.1, both tagged by Ryan). Each step landed as its own PR:
[#40](https://github.com/Wet-Ink-Corporation/weigh-in/pull/40) the `marketplace` branch from
`release.yml` (#12, [B-14](decisions/build-decisions.md#b-14)),
[#41](https://github.com/Wet-Ink-Corporation/weigh-in/pull/41) the install guide (#14), and
[#42](https://github.com/Wet-Ink-Corporation/weigh-in/pull/42) the 0.4.0 bump (#15), after
[#39](https://github.com/Wet-Ink-Corporation/weigh-in/pull/39) fixed the cloud SessionStart that
temper's lint tables had stopped (#38, [B-13](decisions/build-decisions.md#b-13)). The installs (#13)
ran on Windows and in Cowork, which are Ryan's machines for v1; macOS and Linux were not run. They
found two problems, fixed in 0.4.1: a rerun release packed the failed attempt's assets into the
plugin ([#45](https://github.com/Wet-Ink-Corporation/weigh-in/pull/45), #43,
[B-15](decisions/build-decisions.md#b-15)), and Claude Code leaves auto-update off for this
marketplace, so the guide now turns it on
([#47](https://github.com/Wet-Ink-Corporation/weigh-in/pull/47), #46). With it on, Windows moved to
0.4.1 by itself. What differs from the steps above: no ruleset guards `marketplace` (Ryan's call in
B-14), and only the tag is pushed, since the workflow creates the release.

### Phase 3 — Verify every surface and run it for real (about 2 weeks elapsed, little effort) → 0.9.x

1. **Close V1–V7**, each with a dated note in its issue and in `roadmap.md`:
   V1 Cowork session id; V2 ArtifactData from Cowork; V4 editor links from the board; V5 `db` store
   quotas (push a large ledger and note where it fails); V6 the desktop Code tab and the Cowork
   question card; V7 the Windows binary from an installed plugin. Add the Windows-without-Git-Bash
   case, since hooks run under `sh`: either confirm Claude Code on Windows always provides it, or
   ship a `cmd`/PowerShell hook form.
2. **Eval seed**: label 20 items from five real sessions (the design's slice 1 eval set, not yet
   built), so marker capture precision can be measured.
3. **Sustained use**: at least two weeks (one, by D-0019) of real sittings across this repository and two of
   Ryan's own repos, plus one Cowork folder. Each week, record `weighin stats` (action rate,
   not-useful rate, overrides, time to decide) and the sitting cost in this plan's issue.
4. **Sitting cost**: either trim the batch JSON and card output until a sitting is under about
   $0.35, or record the measured cost as the v1 figure in a decision.
5. Fix what use finds, releasing 0.9.x release candidates through the same workflow.

**Progress at 0.5.1 (2026-10-04).** Landed, each as its own PR:
- **Step 1:** [#53](https://github.com/Wet-Ink-Corporation/weigh-in/pull/53) recorded V7 as verified
  (#21). [#57](https://github.com/Wet-Ink-Corporation/weigh-in/pull/57) moved the hooks to exec form
  in 0.5.0, so they need no Git Bash on Windows ([D-0015](decisions/build-decisions.md#d-0015),
  [B-16](decisions/build-decisions.md#b-16)), after the spike in
  [#56](https://github.com/Wet-Ink-Corporation/weigh-in/pull/56).
  [#61](https://github.com/Wet-Ink-Corporation/weigh-in/pull/61) capped the board's defaults at 200
  in 0.5.1, ahead of V5's run (#19). The checklists for V1, V2, V4, V5 and V6 are posted on #16–#20,
  and Ryan has not run them yet. #21 waits on one Windows session with Git Bash hidden.
- **Step 2:** [#55](https://github.com/Wet-Ink-Corporation/weigh-in/pull/55) added the scorer and a
  provisional scripted seed. [#66](https://github.com/Wet-Ink-Corporation/weigh-in/pull/66) added
  the real seed of 25 items from three sessions since 0.5.0, labels for review, with the 80% bar as a
  test. It passes on the one captured item, and recall is 1 of 7, because agents seldom write blocks
  (#22).
- **Step 3:** [#54](https://github.com/Wet-Ink-Corporation/weigh-in/pull/54) added
  `scripts/weekly-stats.sh`. No week has been posted on #23 yet, so the action and not-useful rates,
  and with them #25, are not yet measured.
- **Step 4:** done. [#59](https://github.com/Wet-Ink-Corporation/weigh-in/pull/59) recorded the
  measured cost as a formula ([D-0016](decisions/build-decisions.md#d-0016), #24).
- **Step 5:** [#58](https://github.com/Wet-Ink-Corporation/weigh-in/pull/58) stopped read-only
  commands from creating a ledger, which a Cowork folder cannot delete (0.5.1).
  [#62](https://github.com/Wet-Ink-Corporation/weigh-in/pull/62) covered the Windows `ENOENT` hook
  error in the install guide. No 0.9.x candidate yet.

**Progress at 0.5.2 (2026-10-05).**
- **Step 1:** no V-item results posted on #16–#21 since 0.5.1; #21 still waits on one Windows
  session with Git Bash hidden.
- **Step 2:** [#69](https://github.com/Wet-Ink-Corporation/weigh-in/pull/69) added the convention
  line "a list of calls for the user gets one block per call" (#68, wi-357345), released in
  [0.5.2](https://github.com/Wet-Ink-Corporation/weigh-in/pull/71). The seed grew to 28 items with
  three true negatives from the release session, which ran the 0.5.1 convention; recall is still
  1 of 7. No session had run with the 0.5.2 line yet, so its effect on recall is unmeasured. The
  80% bar still waits on 10 captured items (wi-a9df17).
- **Step 3:** no week posted on #23, so #25 is not triggered.
- **Step 5:** [#70](https://github.com/Wet-Ink-Corporation/weigh-in/pull/70) tightened the temper
  baseline to 721. No 0.9.x candidate yet.

**Since 0.5.2 (2026-10-05).**
- **Step 1:** still no V-item results on #16–#21; #21 waits on the Windows session with Git Bash
  hidden.
- **Step 2:** [#72](https://github.com/Wet-Ink-Corporation/weigh-in/pull/72) added the release
  session's turns (s7). [#73](https://github.com/Wet-Ink-Corporation/weigh-in/pull/73) recorded two
  sitting decisions: each cloud session saves its own end-of-turn messages to the seed before it
  ends (wi-48657b), and `.claude/settings.json` allows the launcher so the skill's injected ledger
  command runs in auto mode (wi-6b2fa5); it added s8, the first session on the 0.5.2 line. The
  next session added s9. The seed holds 59 items, 8 of 14 `capture` items captured (precision 8 of
  8); recall is 1 of 7 before 0.5.2 and 7 of 7 after (s8, s9, s10). Labels still wait on Ryan's review, and the 80% bar on 10 captured items (wi-a9df17).
- **Step 3:** no week posted on #23, so #25 is not triggered.
- **V-items, walked through with Ryan on 2026-10-05:** V2 deferred to slice 2
  ([B-17](decisions/build-decisions.md#b-17)). V1, V4 and V7's last check are Ryan's runs, V5 is
  run from a cloud session, and V6 is checked in the desktop app and, with V1, in Cowork. Weeks
  on #23 count from 2026-10-05.
- **V-items, results the same day:** V5 ran from the cloud session and closed (the document
  limit is exactly 262,144 bytes, and the capped board is 59,678 bytes at 1,000 defaults). V4 closed as
  not supported: an Artifact does not hand `vscode://` links to the OS. V6 passed in the desktop
  Code tab; its Cowork half comes with V1. A Windows install showed the `marketplace` branch's
  launcher checked out with CRLF line endings, but a trace on 2026-10-06 showed Git Bash runs it
  correctly, and `/weigh:weigh-in board` then published a board from a local Windows session on
  0.5.1. The earlier "no binary" failure
  ([#76](https://github.com/Wet-Ink-Corporation/weigh-in/issues/76)) was not tested by that run:
  it came from a claude.ai-synced copy of `main` at 0.5.2, which has no binaries, a different
  install ([B-18](decisions/build-decisions.md#b-18)). The board URL stays out of the
  tracked config ([B-20](decisions/build-decisions.md#b-20)).

Exit criteria. These are the design's slice 1 exit criteria (two weeks or 30 presented items,
whichever comes later), the same targets `weighin stats` prints, plus the release checks:

- Action rate at least 60%; not-useful dismissals at most 10%. If not-useful is above 10%,
  per-category suppression comes into v1 (P-6).
- Marker capture precision at least 80% on the eval seed, counted once the seed holds at least
  10 captured items (wi-a9df17).
- No "pending" item is ever reported as "done", and at least one sitting correctly returns
  "no decisions need you".
- Every V-item is closed or has a recorded decision to defer it past 1.0.
- No ledger corruption, and no unexplained `hook-errors.log` lines.
- Each of Ryan's machines goes from nothing to a working `/weigh-in` through the marketplace
  branch, without Rust; Cowork works through the connected checkout.
- A 0.2.2 ledger opens and reads correctly under the candidate.

### Phase 4 — v1.0 live (about 1 day) → 1.0.0

1. Freeze for review: run `/code-review` over the changes since 0.2.2 and fix what it finds.
2. Update `README.md` ("Measured in this build"), `roadmap.md` (slice 2 becomes the next
   milestone), `CHANGELOG.md`, and add a dated amendment to `docs/research/report.md`
   summarising what v1 shipped against the design.
3. Bump both versions to 1.0.0, merge, tag `v1.0.0`; the release workflow publishes the release
   and the marketplace.
4. Update each of Ryan's machines from the `marketplace` branch, refresh the connected Cowork
   checkout and upload the zip to claude.ai; remove any local-build installs so only one version runs (B-05).
5. Record "v1 live" in `build-decisions.md` with the exit-criteria results, and open the
   `v1.1` / slice 2 milestone.

## After v1

Slice 2 as in `roadmap.md`: the Sonnet grounder and stability check, `weighin serve --wait` for
client repos, per-category suppression, just-in-time surfacing, reliance drills (wi-2b90f3), the
door classifier for unattended runs (wi-4d8b6a, which also settles the open model-key question),
the board from Cowork, the owner default taken from git config (needed once anyone but Ryan
uses it), going public (D-0012), and the move into redkiln-rs with kb-ingest of the intake keys
(wi-e15c77, wi-b4c1d8).
