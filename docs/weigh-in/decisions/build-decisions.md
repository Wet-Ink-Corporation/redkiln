# Decisions taken while building and shipping, 2026-09-27 to 2026-10-02

These came up in chat rather than on the board. `D-` entries are Ryan's calls; `B-` entries are
build calls Claude made, either forced by a platform constraint or as two-way choices, and are
open to challenge like any other default.

## D-0002

**Cowork runs the ledger through the computer's shell.** 2026-09-28 · amends the Cowork half of
[wi-b4c1d8](wi-b4c1d8.md) · provenance: Ryan passed on a Cowork session's findings and asked for
them to be addressed; built as proposed.

Context. In Cowork, skills and hooks run in a cloud workspace whose project folder is scratch and
is deleted with the session; the user's files sit behind the computer's shell. wi-b4c1d8 had
Cowork use the board store with no binary, because no binary was thought reachable there. The
shell on the computer can run the Linux binary against a connected folder.

Decision. When the skill can reach a `weighin` binary from the computer's shell, Cowork runs it
there against the folder the conversation worked in, so a Cowork folder keeps the same ledger,
guards and `.kb` atoms as a repo in Claude Code. Without a binary the skill settles in chat and
hands over Y-statements; the board remains the fallback. The sitting scans the files the
conversation changed, because `PostToolUse` never sees edits made through the computer's shell.
Decisions also go into the folder's own ruling or decision log when it keeps one.

## D-0003

**Cowork gets the binary by downloading it from a GitHub release.** 2026-09-28 · provenance: Ryan
chose option 1 of 3.

Options weighed: (1) download from a GitHub release into the shell's per-chat scratch space,
checked against a SHA-256 stamped into the skill; (2) copy the binary into each work folder,
which puts a 4.5 MB executable into synced client folders; (3) one dedicated Weigh-In folder on
the computer, requested through the folder-access prompt each chat.

Decision. Option 1. `scripts/package-plugin.sh` stamps the version and the Linux binary's
SHA-256 into the zipped skill; the release workflow builds, stamps and publishes from one build.
If the download fails or does not match, the skill uses a copy in a connected folder.

Status. Built and tested against a local copy of a release; dormant. The repository is internal
([D-0007](#d-0007)) and the computer's shell has no GitHub login, so zips are packaged with
`--no-download` and Cowork uses the copy in a connected `weigh-in` folder.

## D-0004

**The command is `/weigh-in`: plugin `weigh`, skill `weigh-in`.** 2026-09-28 · provenance: Ryan.

History. `/weigh-in:weigh-in` was unwieldy in Claude Code. `/weigh:in` (plugin `weigh`, skill
`in`) read well in Claude Code but showed as a bare `/in` in Cowork, which lists plugin skills by
the skill's own name.

Decision. Name the skill so it stands alone, as other plugins do (`inbox-zero` with `iz-rules`,
`create-cowork-plugin`, `pdf`). Cowork shows `/weigh-in`; Claude Code lists `/weigh:weigh-in` and
also resolves the bare `/weigh-in` (checked with `claude -p`). The install id stays
`weigh@weigh-in`.

## D-0005

**One skill with arguments; no split yet.** 2026-09-28 · provenance: Ryan.

The occasional routes (`init`, `digest`, `board`, `sweep`, `stats`) stay arguments of
`/weigh-in` rather than separate `/weigh-in-digest`-style skills. A split would make them visible
in both menus and cheaper to run, since they would not load the full sitting instructions; it is
not wanted now.

## D-0006

**Distribute through the local marketplace for now.** 2026-09-28 · provenance: Ryan.

Claude Code installs from the marketplace in this repository (`/plugin install weigh@weigh-in`).
Cowork, which only takes plugins from the account, gets zips uploaded under Customize > Plugins.

## D-0007

**Weigh-In lives in `Wet-Ink-Corporation/weigh-in`, internal.** 2026-10-01 · provenance: Ryan
created the repository and a local clone.

Consequence. Release assets are not public, so the Cowork download ([D-0003](#d-0003)) stays off
until the repository or its releases are public; see [B-06](#b-06).

## D-0008

**v1 means daily use by Ryan.** 2026-10-02 · provenance: Ryan, answering P-1 of the
[v1 plan](../v1-plan.md#decisions).

v1 is live when Ryan uses `/weigh-in` daily on his own machines and in Cowork. Installing for
other people, a public plugin, and an owner default other than `ryan` are not v1 work.

## D-0009

**Claude Code installs from a built marketplace in this internal repository.** 2026-10-02 ·
amends [D-0006](#d-0006) · provenance: Ryan, answering P-2 of the [v1 plan](../v1-plan.md#decisions).

The release workflow publishes the built plugin, binaries included, to a `marketplace` branch
here, and Claude Code installs from `Wet-Ink-Corporation/weigh-in#marketplace` with the
machine's git credentials. The local marketplace on `main` stays for development. Cowork still
gets the uploaded zip.

## D-0010

**This repository uses Weigh-In on itself.** 2026-10-02 · provenance: Ryan, answering P-4 of
the [v1 plan](../v1-plan.md#decisions).

`init --class own` here, `.kb/` committed, and new decisions go through `/weigh-in`. The
hand-kept records in this folder stay as the history before that.

## D-0011

**Open work lives in GitHub Issues.** 2026-10-02 · provenance: Ryan, answering P-5 of the
[v1 plan](../v1-plan.md#decisions).

Verification items, known gaps and plan tasks are issues under a `v1.0` milestone;
`docs/roadmap.md` summarises and links them.

## D-0012

**Release assets stay internal for v1.** 2026-10-02 · provenance: Ryan, answering P-3 of the
[v1 plan](../v1-plan.md#decisions).

No second repository and no public assets. Claude Code installs from the internal `marketplace`
branch (D-0009); Cowork uses the uploaded zip and a connected checkout of that branch, so the
Cowork download ([D-0003](#d-0003)) stays dormant. Going public is after v1, by cleaning up the
research and making this repository public; there is still one repository.

Amended 2026-10-05 by [B-19](#b-19): releases are also published to a release-only repository,
`Wet-Ink-Corporation/weigh-in-plugin`, which the organization sync reads. Assets stay internal.

## D-0013

**No slice 2 work in v1, unless the not-useful rate misses.** 2026-10-02 · provenance: Ryan,
answering P-6 of the [v1 plan](../v1-plan.md#decisions).

v1 is slice 1 made dependable. If two weeks of daily use show more than 10% of items dismissed
`not-useful`, per-category suppression comes into v1, since it is the design's fix for that miss.

## D-0014

**This repository's ledger stays at the default location.** 2026-10-02 · provenance: Ryan,
choosing between keeping the default, a home-directory ledger, committing the ledger, and
deferring [D-0010](#d-0010) to Phase 2.

`.kb/_ledger/config.json` sets no `store`, so the ledger is `<git common dir>/weighin/` ([B-02](#b-02)).
On Ryan's clones it lasts; in Claude Code cloud sessions it goes with the container. Decisions
survive either way as committed `.kb` atoms; what a container takes with it is the event history
and anything left open, deferred or defaulted, so cloud sessions settle everything before they
end. Accepted until Phase 2's installs, and noted as a known gap.

Amended 2026-10-09 by [B-21](#b-21): the event history now survives too, as a committed journal
per cloud session, because the pilot is judged on more than decisions.

## D-0015

**Hooks work on Windows without Git Bash in v1.** 2026-10-03 · provenance: Ryan, choosing between
exec-form hooks now, requiring Git Bash for v1, and exec-form hooks with a shell-neutral skill ·
closes the hook half of [#21](https://github.com/Wet-Ink-Corporation/weigh-in/issues/21).

Claude Code without Git for Windows runs shell-form hooks under PowerShell, where `sh` is not
found, so capture was silently off. The hooks move to exec form ([B-16](#b-16)) in 0.5.0. The
sitting still runs through `sh`, so `/weigh-in` on Windows still needs Git Bash; a shell-neutral
skill was not chosen.

## D-0016

**v1 records the measured sitting cost instead of trimming the output.** 2026-10-03 · provenance:
Ryan, choosing between measuring then recording, trimming and recording, and trimming only ·
closes [#24](https://github.com/Wet-Ink-Corporation/weigh-in/issues/24).

A sitting's cost is set by the session it ends, not by the cards. Measured from the transcript of
the session that chose this (0.5.0, Opus 5.5, three requests, two cards, four swept items):

| Request | Cache read | Cache write | Output |
|---|---:|---:|---:|
| Batch 1 (sweep, triage, two cards) | 583,694 | 4,750 | 4,392 |
| AskUserQuestion | 588,444 | 4,807 | 972 |
| Batch 2 (decisions) | 588,444 | 15,516 | 224 |

That is about $0.59 at Opus 5.5's prices ($4 in, $20 out, $0.20 cache read; cache writes taken as
1.25 × input) and about $0.47 at Sonnet 5.5's ($2, $10, $0.20): about 60–75% of it is the three
requests re-reading a 585k-token context, which no trim touches. The cards and batch JSON the
model writes come to about 5,600 output tokens, as estimated in #24; trimming them would save about
1,400 tokens, a cent or two. So the v1 figure is a formula, not a constant: **about 3 × the
session's context at the cache-read price, plus about 5,600 output tokens**, which is about $0.10–0.15
on Sonnet 5.5 after a short session (40k context) and about $0.47 after a long one. The design's
$0.28 and the $0.35 target are dropped.

## D-0017

**After 1.0, a Claude Code mod's local board pane decides grounded cards, and replaces
`serve --wait` for client repositories in Claude Code.** 2026-10-11 · provenance: Ryan, answering
the two calls in [#112](https://github.com/Wet-Ink-Corporation/weigh-in/issues/112) yes, each as
recommended · amends [wi-7c21e4](wi-7c21e4.md)'s local route · nothing changes before 1.0.0
([D-0013](#d-0013), [B-23](#b-23)).

1. **Decisions outside a sitting.** The pane ([#115](https://github.com/Wet-Ink-Corporation/weigh-in/issues/115))
   may decide, defer, dismiss and reopen cards a sitting has already grounded, so the five
   filters have run on them. Items no sitting has grounded stay read-only there, to be settled
   in a sitting. Pane decisions carry their own surface, so `weighin stats` keeps them apart
   from chat and board decisions.
2. **The local board for client repositories.** In Claude Code (terminal and the desktop Code
   tab) the pane is the local board that wi-7c21e4's `local` mode called for: it runs on the
   machine and never publishes. `serve --wait` is kept only for the VS Code chat panel, where
   mods do not draw, and only if that surface is used with client repositories. Since
   [B-25](#b-25), `board export` refuses outside an `artifact` board, so the pane needs a
   local-only export path that never publishes.

## D-0018

**In a Claude Projects task, the ledger follows the work.** 2026-10-13 · provenance: Ryan, on a
decision card, choosing "follow the work" over "always cloud" and "always folder" · applies
[D-0002](#d-0002) to Projects tasks · from [#126](https://github.com/Wet-Ink-Corporation/weigh-in/issues/126).

Projects thread sessions run in a cloud workspace (`CLAUDE_CODE_ENTRYPOINT=remote_projects`,
not Cowork's `remote_cowork`), where the hooks run and capture into a ledger that ends with the
task; when a computer is linked, its connected folders are reached through the device shell, as
in Cowork. Ryan's check on 2026-10-13 found a Projects task working in a connected Windows folder
still recorded its sitting in the cloud ledger, so the decisions never reached the folder.
Now, when the task's work was in a connected folder, the sitting takes the Cowork route against
that folder, and this session's items from the cloud ledger are raised again there as swept
items. When the work was in a cloud checkout, the cloud ledger stays, and the journal is
committed as for any cloud session ([B-21](#b-21)). In the same change, Projects tasks ask cards
with their own decision card tool (`ask_decision`) instead of printing them as text.

## D-0019

**One week of daily use is enough for v1.** 2026-10-18 · Ryan, in the project thread, after the
week 1 numbers · amends the "two weeks" in [D-0008](#d-0008)'s exit criteria and `docs/v1-plan.md`
· from [#23](https://github.com/Wet-Ink-Corporation/weigh-in/issues/23).

The design's slice 1 exit asks for two weeks or 30 presented items, whichever comes later. Week 1
(2026-10-12 to 10-18) already met every measured target across six folders: 90 presented,
action rate 73%, not-useful 0%, no hook errors, no ledger corruption, and a sitting that said no
decisions needed Ryan. A second week would mostly re-measure the same capture behaviour, so v1
closes on week 1's numbers. #25 (suppression) stays out, since not-useful is under 10%.

## D-0020

**Pull requests land on `main` as merge commits.** 2026-10-31 · Ryan's call, in the project
thread · supersedes the "squash or rebase merges only so history stays linear" part of Phase 0
in [`v1-plan.md`](../v1-plan.md) and the squash merge in [`releasing.md`](../releasing.md).

The `main` ruleset and the repository's General settings allow merge commits only, so each
PR's commits reach `main` unchanged, with their `Weigh-In:` trailers where they were written.
The ruleset's "Require linear history" was still on from Phase 0. It forbids the merge commit
that the only allowed method creates, so GitHub offered no merge button, only the bypass, and
showed "This branch must not contain merge commits" on every PR. Ryan merged #166 through the
bypass (`283c63f`, a merge commit), then turned "Require linear history" off.

The ruleset still requires a pull request, the six status checks and resolved review threads,
and blocks force pushes and deletion. Ryan remains the only bypass actor.

## D-0021

**The Cowork fetch script becomes its own stamped, tested file.** 2026-11-01 · Ryan's call, in
the project thread, each on the recommended option · for
[#171](https://github.com/Wet-Ink-Corporation/weigh-in/issues/171), from the 2026-11-01 audit ·
amends the "one stamp in the skill" rule in `CLAUDE.md` and how [D-0003](#d-0003)'s download is
carried.

1. The one-line script `SKILL.md` sends to the device shell in Cowork moves to
   `plugin/libexec/cowork-fetch.sh`, readable and tested against a fake `$HOME/mnt` tree. The
   skill Reads it and sends its text unchanged. The device shell runs on the user's computer and
   cannot read the plugin, so the text passes through the model either way; a `weighin`
   subcommand cannot do the job, since the binary is what is being fetched.
2. The `V=…; S=…;` stamp moves with it: `package-plugin.sh` stamps that file, and the one-stamp
   rule applies to it instead of `SKILL.md`.
3. Per-route `claude plugin eval` cases run by hand before each release, not in CI, which would
   need an API key secret and spend per run.

Considered and not taken: keeping the script inline with a test that extracts it, and leaving it
untested. The extra Read call in a Cowork sitting is confirmed in one real Cowork run before
release.

The route line the design pass also proposed (SessionStart printing the `W` to use) is not
built: on Windows SessionStart already prints the binary's path, so the line would restate it
and add a real-device check. #171 closes with the fetch script (Ryan, 2026-11-01).

Confirmed in Cowork on 2026-11-01 (Ryan): a test build of `main` at e12823e, uploaded as
`weigh-rc` with the script stamped `V=1.1.3; S=none;`, in a task connected to one scratch folder
holding that build's Linux binary. `/weigh-in-rc` Read `libexec/cowork-fetch.sh`, sent it to the
device shell, which found the folder's binary, and the sitting presented a card and wrote the
decision atom to the folder's `.kb/`. The download branch could not run without a release; its
logic is unchanged from 1.1.3, and the first Cowork sitting on 1.2.0 exercises it.

## D-0022

**Standing decisions come due for review after 90 days.** 2026-11-01 · Ryan's call, in the
project thread, each on the recommended option · for
[#173](https://github.com/Wet-Ink-Corporation/weigh-in/issues/173), from the 2026-11-01 audit ·
widens `item_acknowledged`, which meant only "a default was seen and left standing".

1. A decision comes due 90 days after its newest decide or review. `caps.review_age_days` in
   `.kb/_ledger/config.json` changes it per repository; 0 turns ageing off.
2. Leaving a due decision standing is recorded with `item_acknowledged`, widened to decided
   items. No new event type, so an older binary beside a newer one still reads every event
   (B-05). A 1.1.x fold sets `acknowledged` only on an item with a default, so it ignores a
   review; a fixture test checks that the current fold reads 1.1.x events as before and that a
   review is written in the 1.1.x shape.
3. Only two-way decisions with a flip condition come due. One-way decisions and those with no
   flip condition never do, though any decision can still be reopened by hand.

Automatic flip-condition checks, which the audit proposed, are dropped: flip conditions are free
text. Considered and not taken: 30 or 180 days, an age by reversibility, a new `item_reviewed`
event, and recording reviews only in the atom.

## B-01

**The ledger runs under a TRUNCATE journal.** 2026-09-28 · forced by a platform constraint.

Cowork's connected folders refuse deletes, and both happenstance's WAL and SQLite's default
DELETE journal delete files beside the database: creating a ledger there failed with
`SQLITE_IOERR_DELETE` (2570). `Store::open` therefore opens its own connection under
`journal_mode=TRUNCATE` (`synchronous=FULL`) and copies happenstance's schema from a template
built on the local temporary disk, inside one `BEGIN IMMEDIATE`. A WAL ledger from 0.1.x converts
on first open; a stale `-wal` is renamed aside rather than deleted. Reproduced with a shim that
refuses `unlink`, and confirmed with strace on the computer's mount. Consequence: happenstance is
held at 0.3.2, and a happenstance schema change will need a migration path ([roadmap](../roadmap.md)).

## B-02

**The ledger location is settable.** 2026-09-28 · Claude's call, two-way.

First match wins: `--store`, `$WEIGHIN_STORE`, `store` in `.kb/_ledger/config.json`
(`init --ledger`), the git common dir, then `<folder>/.weighin/` outside git. The in-folder default
is safe on one computer; a synced folder used from several computers should point `store`
outside the synced tree.

## B-03

**Plugin binaries live in `libexec/`, not `bin/`.** 2026-09-28 · forced: claude.ai rejects hosted
plugins with a top-level `bin/`, because Claude Code adds it to PATH outside the admin approval
surface. Hooks and the skill call the launcher by path; `package-plugin.sh` refuses a `plugin/bin`.

## B-04

**Hooks write nothing in Cowork.** 2026-09-28 · Claude's call, two-way.

Under `CLAUDE_CODE_ENTRYPOINT=remote_cowork` the project folder is scratch, so `SessionStart`
prints the marker convention, `Stop` nudges on one-way markers, and nothing is written; the
sitting's sweep is the capture.

## B-05

**One binary version at a time.** 2026-09-28 · Claude's call, two-way.

The launcher prefers the first target listed unless a later one is a newer build, so a stale
native Windows build cannot shadow a fresh cross-build. The stale 0.1.1 msvc build was removed
from the local marketplace, since versions differ in journal mode.

## B-06

**The repository was seeded as one commit on top of GitHub's initial commit.** 2026-10-01 ·
Claude's call, two-way.

The pre-GitHub history stays only in the local branch `archive/pre-github`: it named a client's
Azure DevOps organisation in a test fixture and carried a stray telemetry file. GitHub's single
MIT `LICENSE` is replaced by `LICENSE-MIT` and `LICENSE-APACHE`, matching
`license = "MIT OR Apache-2.0"`. The release workflow packages with `--no-download` unless the
repository is public, so a release from this internal repository never points Cowork at a
download it cannot reach.

## B-07

**Every push to `main` and every pull request runs the gate in CI.** 2026-10-01 · Claude's call,
two-way.

Until now only a release tag ran tests and clippy, so a broken `main` surfaced at release time;
rustdoc under `-D warnings` was failing unnoticed. `.github/workflows/ci.yml` runs `cargo fmt
--check`, clippy with `-D warnings`, the tests and rustdoc with `-D warnings`, on Linux and
Windows, and the release workflow calls it instead of keeping its own copy. Dependabot opens
weekly update PRs for actions and Cargo minor and patch versions; Cargo majors (H-16) and
happenstance ([B-01](#b-01)) are held back.

## B-08

**A path is in the repository when its resolved form is under the resolved toplevel.**
2026-10-01 · Claude's call, two-way.

Hooks and agents spell paths as their surface does: 8.3 short names on Windows, symlinked temp
dirs on macOS, `..` components, and files not written yet. `Env::relative` resolves the nearest
existing directory, folds the rest (`..`, `.`) itself, and compares that with git's toplevel;
anchors keep their `:line:col`. When both sides resolve, that answer stands, so a file under an
in-repo directory symlink that points out of the repository is outside, as git treats it: it is
not scanned and nothing is annotated through the link. The file name itself is never resolved,
so an in-repo file symlink keeps its own name.

## B-09

**Phase 0 repository setup.** 2026-10-02 · Claude's call, two-way.

Release notes come from `CHANGELOG.md` (`scripts/changelog-section.sh`), and CI fails when the
current version has no entry, so a bump cannot ship without notes. A release tag must be on
`main`. CI adds macOS, a `plugin` job (stamp, versions, changelog, a packaged zip,
`claude plugin validate`) and `cargo deny` with a permissive licence list (MIT, Apache-2.0,
Unicode-3.0, Unlicense). Both crates are `publish = false`, which cargo-deny needs to accept the
workspace path dependency, and which matches D-0007. An `archive/*` pre-push guard lives in
`.githooks/`, opted into per clone. Claude Code cloud sessions run a synchronous SessionStart
hook that installs the toolchain and pre-builds the tests.

## B-10

**The ledger's schema version is `PRAGMA user_version`; newer ledgers are refused.** 2026-10-02 ·
Claude's call, two-way · closes the migration gap [B-01](#b-01) left.

happenstance keeps its own migration counter in `store_meta` and never touches `user_version`,
so the pragma is free for Weigh-In's own version: 1 is happenstance 0.3.2's schema under
TRUNCATE. A new ledger is stamped in the transaction that copies its schema. A 0.1.x or 0.2.x
ledger reads 0 and is migrated step by step in one `BEGIN IMMEDIATE` (0 → 1 changes nothing);
a ledger already current is not written to. A ledger stamped higher than the build knows is
refused with "upgrade weighin" and left untouched, rather than read by a build that may
misunderstand it. A happenstance schema change becomes one more step, which is the way past
0.3.2. Tests build a 0.1.x WAL ledger and a 0.2.2 TRUNCATE ledger in the test, the latter from
event payloads copied out of a ledger the 0.2.2 release wrote; older builds ignore the pragma,
so a 0.2.2 binary still opens a stamped ledger.

## B-11

**SessionStart tells the user when capture is off.** 2026-10-02 · Claude's call, two-way.

The message goes in `systemMessage`, which Claude Code shows to the user; plain SessionStart
stdout reaches only Claude's context, so the prime moves to `hookSpecificOutput.additionalContext`
whenever there is something to say, and stays plain stdout otherwise. Two cases:

- No binary. The `sh` launcher cannot parse the hook's JSON, so `hooks.json` passes the event
  name (`hook SessionStart`); the launcher drops it before running a binary, since the binary
  reads the event from stdin and an older one would refuse the extra argument. With no binary it
  prints a fixed JSON line (only `uname` output is interpolated, so nothing needs escaping).
- Hook errors. SessionStart counts `hook-errors.log` lines past the offset kept in
  `hook-errors.seen` beside it, reports the count and the last line, and moves the offset, so each
  line is reported once. A SessionStart that fails itself is logged first and reported in the same
  message, and Claude still gets the convention. The offset file is overwritten, never deleted
  ([B-01](#b-01)), and nothing is written when there is no log.

Under `CLAUDE_CODE_ENTRYPOINT=remote_cowork` neither is said ([B-04](#b-04)): Cowork captures
through the sitting's sweep, not hooks, so "capture is off" would be wrong there.

## B-12

**Cloud sessions on this repository get Weigh-In through the repository, not the plugin.**
2026-10-02 · Claude's call, two-way · implements [D-0010](#d-0010) for cloud sessions, as Ryan
chose; replaces enabling the plugin in `.claude/settings.json`, which cloud sessions ignore.

Claude Code's cloud sessions do not install plugins a repository enables (`enabledPlugins`,
`extraKnownMarketplaces`), but they do run its `.claude/settings.json` hooks and load its
`.claude/skills/`. So, in cloud sessions only (`CLAUDE_CODE_REMOTE=true`):
`.claude/hooks/session-start.sh` builds `weighin` into `plugin/libexec/<arch>-unknown-linux-gnu/`,
copies the plugin's skill into `.claude/skills/weigh-in/` (ignored by git) with
`${CLAUDE_PLUGIN_ROOT}` rewritten to the checkout's `plugin/`, and runs Weigh-In's SessionStart
itself, after the build, asking Claude Code to re-scan skills. A separate SessionStart hook would
run alongside the build and find no binary. Stop, SubagentStop and PostToolUse go through
`.claude/hooks/weigh-in.sh`, which runs the plugin's launcher. Both do nothing outside cloud
sessions, where the installed plugin captures, so nothing is captured twice.

## B-13

**The cloud SessionStart's lint step only warms the cache.** 2026-10-03 · Claude's call, two-way ·
fixes [B-12](#b-12) after temper's lint tables (#37) · closes
[#38](https://github.com/Wet-Ink-Corporation/weigh-in/issues/38).

temper's `[workspace.lints]` deny findings that `.claude/temper-baseline.json` records, so plain
`cargo clippy` fails on the legacy code, and under `set -e` that stopped `session-start.sh`
before it built `weighin`, installed the skill or ran Weigh-In's SessionStart: capture was off
in every cloud session, without a word. The hook now runs clippy with `--keep-going` and goes on
when it fails, saying the gate is `temper.mjs baseline --check`. Lints are judged by the gate,
never by session setup.

## B-14

**The `marketplace` branch is one commit built from the release zip, and has no ruleset.**
2026-10-03 · Claude's call, two-way, except the ruleset, which is Ryan's · implements
[D-0009](#d-0009) · closes [#12](https://github.com/Wet-Ink-Corporation/weigh-in/issues/12).

`scripts/marketplace-branch.sh` unpacks the zip the release uploads into `plugin/` beside
`main`'s `.claude-plugin/marketplace.json`, and commits it as the only commit of a fresh
repository, with the launcher and every binary set to mode 100755 whatever the zip kept. So
Claude Code installs exactly the plugin Cowork is given, skill stamped `V=<version>; S=none;`
while releases are internal ([D-0012](#d-0012)). `release.yml` builds and validates the tree
before publishing anything, then force-pushes it to `marketplace` after the release is created;
old binaries never stay in the history clones download, and Claude Code's update re-clones a
marketplace when it finds new commits, so the rewritten branch updates cleanly. CI builds the same tree from
its own zip on every pull request and checks the modes, the stamp and `claude plugin validate`.

Ryan chose no ruleset for the branch: a ruleset cannot name the workflow's `GITHUB_TOKEN` as a
bypass actor, so guarding it would need a GitHub App or a deploy key, and the `main` ruleset
already covers only `main`. People with write access could push it; the next release replaces it.

## B-15

**A plugin package holds only the launchers and one binary per target.** 2026-10-03 · Claude's
call, two-way · closes [#43](https://github.com/Wet-Ink-Corporation/weigh-in/issues/43).

Rerunning the `release` job of v0.4.0 downloaded every artifact of the run into
`plugin/libexec/`, including the `release-assets` artifact the failed first attempt had
uploaded, so the zip and the `marketplace` branch carried a nested copy of the release (32 MB
instead of about 13). The job now downloads only artifacts named like a target (`*-*-*`), and
`package-plugin.sh` refuses a `libexec/` holding anything but `weighin`, `weighin.cmd` and
`<target>/weighin[.exe]`, so a stray file fails the release instead of shipping. CI checks the
refusal on every pull request. The first attempt failed because a GitHub release for the tag
already existed; the README now says to push only the tag.

## B-16

**Hooks start the launcher directly (exec form), with the Windows build at `libexec/weighin.exe`.**
2026-10-03 · Claude's call, two-way · implements [D-0015](#d-0015) · amends [B-11](#b-11) for
Windows.

Each hook in `hooks.json` is `{"command": "${CLAUDE_PLUGIN_ROOT}/libexec/weighin", "args":
["hook", …]}`. With `args`, Claude Code (2.1.139 and later) spawns the command with no shell on
every platform, Git Bash or not. A spike under Bun, Claude Code's runtime
([#56](https://github.com/Wet-Ink-Corporation/weigh-in/pull/56)), settled the rest:

- On Linux and macOS the kernel runs the `sh` launcher by its shebang, so it must be executable.
  The `marketplace` branch commits it 100755 ([B-14](#b-14)); with the bit cleared the hook fails
  with `EACCES`.
- On Windows the same extensionless path resolves to `libexec/weighin.exe`, with forward or back
  slashes and with no Git on `PATH`; the hook JSON reaches stdin. A `.cmd` cannot be spawned this
  way (`EINVAL`), so `weighin.cmd` cannot stand in.
- `package-plugin.sh` copies the Windows build (msvc, else gnu) to `libexec/weighin.exe` in the
  package, and the local build scripts do the same, so the newest build is there ([B-05](#b-05)).
  The release ships the msvc build twice, once per path; the per-target copy stays for the `sh`
  launcher the skill uses.
- Nothing strips the event name on Windows, so `weighin hook` takes it as an optional argument
  and still reads the event from stdin. Hooks of 0.5.0 need a 0.5.0 binary, which the same
  package ships.

What it costs: on Windows a missing `weighin.exe` is a hook error (`ENOENT`), not [B-11](#b-11)'s
"capture is off" message, which still covers Linux and macOS. Claude Code older than 2.1.139 has
no exec form. Unverified: whether plugins synced from the uploaded zip (Cowork, a claude.ai
upload) keep the launcher's executable bit, which `sh launcher` never needed; if they don't, hooks
there fail with `EACCES`, and Cowork loses its SessionStart convention. To check in Cowork before
0.5.0 is tagged. Rejected: a second hook entry with
`"shell": "powershell"`, which fails on macOS and Linux without `pwsh` and runs twice where both
shells exist; one shell-form string for both shells, which neither parses; `commandWindows`, which
Claude Code ignores.


## B-17

**V2, ArtifactData from Cowork, is deferred to slice 2.** 2026-10-05 · Ryan's call, two-way ·
defers [#17](https://github.com/Wet-Ink-Corporation/weigh-in/issues/17) past 1.0.

V2 asks whether the board's data store can be reached from a Cowork chat, including setting the
board state from a file that `board export` writes on the computer, behind the device shell. In
v1 the board does not run from Cowork: the skill's Cowork route uses in-chat cards ("Board: not
from Cowork yet", `SKILL.md`), and the board from Cowork is slice 2
([#27](https://github.com/Wet-Ink-Corporation/weigh-in/issues/27)). V2 tests a path v1 does not
take, so it moves with that slice; the v1 exit criterion "every V-item is closed or has a
recorded decision to defer it past 1.0" is met for V2 by this record.

## B-18

**A copy synced from the development tree has no binaries; only the release repository is
synced, and the launcher names the case.** 2026-10-06 · Ryan's call, two-way (sync and
launcher message), recorded from
[#76](https://github.com/Wet-Ink-Corporation/weigh-in/issues/76) · cited by [B-19](#b-19).

On 2026-10-05 `/weigh:weigh-in board` failed on Windows with "no binary for MINGW64…" from a
plugin copy under `…/rpm/plugin_…/libexec` at 0.5.2 that held only `weighin` and `weighin.cmd`.
That copy was claude.ai's organization sync of this repository. The sync reads the default
branch, `main`, which is the development tree: the skill is stamped `V=dev`, and the binaries are
git-ignored and never committed. On such a copy, the exec-form hooks ([B-16](#b-16)) fail on
Windows with `ENOENT`, because no `libexec/weighin.exe` exists, so the launcher never runs. On
Linux and macOS they reach the launcher, which says capture is off. The 2026-10-06 check that
recorded the failure as "did not reproduce" ([B-20](#b-20), #83) ran on a different install,
from the `marketplace` branch.

Decided:
- **Sync only the release repository.** The organization syncs `weigh-in-plugin`
  ([B-19](#b-19)) and not this repository; Ryan removes `weigh-in` from the sync.
- **The launcher names the case.** When `libexec/` holds no build for any platform (no
  `<target>/weighin`, `<target>/weighin.exe` or `weighin.exe`) and none is on `PATH`, the
  launcher says "this copy has no binaries", and points to the marketplace branch, the release
  repository or the build script. The skill's error and SessionStart's capture-off message
  (Linux and macOS) carry it. On Windows the hooks still fail with `ENOENT`, which
  `docs/install.md` explains.

Checked 2026-10-06: `weigh-in-plugin` at 0.5.3 holds `libexec/weighin.exe`,
`libexec/x86_64-pc-windows-msvc/weighin.exe`, `libexec/x86_64-unknown-linux-musl/weighin` and
`libexec/aarch64-apple-darwin/weighin`, all 100755, with the skill stamped `V=0.5.3`.
Rejected: committing binaries to `main` (B-19's reasons); renaming the development plugin so
that a stray sync shows up separately, which would change the development route
([D-0009](#d-0009)) for a case that the sync setting already prevents.

## B-19

**Releases are published to a release-only repository that claude.ai's organization sync reads.**
2026-10-05 · Ryan's call, two-way (wi-1ec680, decided A; enacted under wi-dba31a) · amends
[D-0012](#d-0012) ("no second repository") and [D-0009](#d-0009) · replaces the proposal in
[#79](https://github.com/Wet-Ink-Corporation/weigh-in/pull/79).

claude.ai's organization sync reads only a repository's default branch, and with **Sync
automatically** on it syncs on each push to that branch
([docs](https://claude.com/docs/plugins/org-sync)). `main` here is the development tree (skill
stamped `V=dev`, no binaries), so a sync of this repository delivers an unreleased copy (B-18),
and a release that moves only the `marketplace` branch never reaches anyone without a manual
**Sync now** or zip upload. `main` cannot carry the build instead: about 13 MB of binaries per
release would stay in its history, every merge would ship before a release, and the release
workflow cannot merge to `main` past its ruleset.

So `release.yml` pushes the same one-commit tree that `scripts/marketplace-branch.sh` builds
([B-14](#b-14)) to the default branch (`main`) of `Wet-Ink-Corporation/weigh-in-plugin`, a
private repository holding nothing else. The push uses a write deploy key of that repository,
saved here as the secret `WEIGH_IN_PLUGIN_DEPLOY_KEY`, over SSH with GitHub's published host
key; the workflow refuses to publish anything when the secret is missing. The organization
syncs that repository, with Weigh-In set to installed by default, so a tag reaches Cowork and
the Claude Code sessions that load claude.ai plugins with no step of anyone's. The `marketplace`
branch here is still published, for existing `#marketplace` installs and the Cowork connected
checkout, until the Cowork binary copy (wi-bb6e2a) lands.

Rejected: #79's `git-subdir` pointer at `ref: marketplace`, because it is unverified whether the
sync follows a ref, and a release would still need **Sync now**; making `marketplace` the
default branch, which pull requests, Dependabot and CI follow. Unverified: the first sync of the
new repository, and that a force-pushed default branch re-syncs as cleanly as the
`marketplace` branch re-clones (B-14).

## B-20

**The board URL stays out of the tracked config.** 2026-10-06 · Ryan's call, two-way.

Publishing the board for the first time (`/weigh-in board`) writes its URL into
`.kb/_ledger/config.json`, which this repository tracks. Committed, that URL would point every
clone's ledger at one board, and every cloud session here has its own short-lived ledger
([D-0014](#d-0014)), so each `/weigh-in board` would overwrite the board with a different
ledger's state. The board mirrors one ledger: on Ryan's Windows checkout the change stays local
(`git update-index --skip-worktree .kb/_ledger/config.json`) and is never committed. Not decided:
a per-machine place for the URL outside the tracked config, which waits until a second machine
needs its own board.

Found on 2026-10-06, when `/weigh:weigh-in board` published the first board from a local
Windows session (0.5.1); the same run showed the earlier "no binary" failure on Windows does
not reproduce.

Corrected 2026-10-06 by [B-18](#b-18): that run was on a different install (the `marketplace`
branch). The earlier failure came from a claude.ai-synced copy of `main`, which has no binaries,
so not reproducing it on another install showed nothing about it.

## B-21

**Cloud sessions keep their pilot evidence in committed journals.** 2026-10-09 · Claude's call
in a cloud session, asked by Ryan to make sure the pilot's evidence is saved without his
intervention; two-way · amends [D-0014](#d-0014) for the event history.

D-0014 accepted that a cloud session's ledger goes with its container, on the view that the
decisions survive as atoms. The pilot ([#23](https://github.com/Wet-Ink-Corporation/weigh-in/issues/23))
is judged on more than decisions. Its action rate counts deferrals and `not-my-call` dismissals,
its not-useful rate counts `not-useful` dismissals, and it also tracks defaults, decision times
and `hook-errors.log`. None of those left an atom, so every cloud sitting on this repository,
where most sittings happen, was missing from `weighin stats`. A sitting whose items were all
dismissed left nothing to commit at all. Counting the atoms alone would have shown only decided
items, which inflates the action rate and hides the not-useful rate that decides #25.

Now `.claude/hooks/weigh-in-journal.sh` runs after every capture hook and every Bash call (a
sitting's `batch`) in a cloud session. It writes the ledger's events to
`.kb/_ledger/journal/<session>.jsonl`, using `weighin journal`, and copies `hook-errors.log`
beside it as `<session>.hook-errors.log`. Both files are append-only: a session resumed in a new
container keeps its id but starts with an empty ledger, and must not wipe what it already holds.
The session commits `.kb/` with its work. `weighin stats` folds the ledger with every committed
journal and counts an event held twice once, and `scripts/weekly-stats.sh` reads a folder that
has journals but no ledger. Local and Cowork ledgers persist already and write no journal.

Not done: replaying journals into a new container's ledger, so that open or deferred items
would also survive. Until then cloud sessions still settle everything before they end.

## B-22

**Teaching kb-ingest the intake keys moves to slice 3.** 2026-10-11 · Claude's call in the AFK
cloud session, recording where the v1 plan already put it; two-way, open to Ryan as a weigh-in
item · amends [wi-e15c77](wi-e15c77.md) for its kb-ingest half.

wi-e15c77 chose to reserve `.kb/_ledger/` in Redkiln's `.kb` contract and teach `kb-ingest`
Weigh-In's intake keys in slice 1. The first half holds: `validate --kb` already skips
`_`-prefixed directories, and Weigh-In's atoms pass it, with a rejected negative control. The
second half is a change in redkiln-rs, outside this repository, and the [v1 plan](../v1-plan.md)
has listed it under slice 3 since 2026-10-02 without a record (the
[status audit](../audits/2026-10-06-status-audit.md), finding A8). It moves with the rest of the
redkiln-rs work: v1 makes no claim that `kb-ingest` promotes Weigh-In's atoms, and the README
says so. The migration wi-e15c77 wanted to avoid stays small, because atoms already carry the
intake keys under `.passthrough()` frontmatter.

## B-23

**0.9.0 is the release candidate; 1.0.0 adds no code.** 2026-10-11 · Claude's call in the AFK
cloud session, which Ryan asked to schedule releases; two-way.

The v1 plan names Phase 3's releases 0.9.x, and the status audit asked whether to cut a
candidate or go straight to 1.0.0. While the two weeks of use run
([#23](https://github.com/Wet-Ink-Corporation/weigh-in/issues/23)), releases are 0.6.x patches
that do not change capture, so the pilot measures one capture behaviour (the reasoning of
wi-7c3aa0). 0.9.0 is cut once the agent-side work is merged and Ryan's Cowork, Windows and
organization-sync checks (#16, #20, #21, #76) pass; after it only fixes for what those checks
or the pilot find land, as 0.9.x. 1.0.0 is the last 0.9.x with the version, the changelog and
the closing documents, tagged once Ryan has ticked the exit criteria. That way the build that
earned the exit criteria is the build that ships, and "a 0.2.2 ledger opens under the
candidate" is checked on the code that becomes 1.0.0.

## B-24

**V3 is closed as moot.** 2026-10-11 · Claude's call in the AFK cloud session; two-way, open to
Ryan as a weigh-in item.

V3 asked whether a worktree session's script can read sibling worktrees' `.kb/_ledger`. Under
[wi-6e2a47](wi-6e2a47.md) there is one store per repository, under the git common dir, so every
worktree reads and writes the same ledger and there is no sibling ledger to read. The CLI's
end-to-end test checks it: a linked worktree lists both items raised in the main one, and
nothing is written into either working tree (`items_seen_from_a_worktree`,
`crates/weighin/tests/it/support.rs`).

## B-25

**Board export needs an explicit `init`; a broken config stops the CLI but not capture.**
2026-10-11 · Claude's call in the AFK cloud session, enforcing [wi-7c21e4](wi-7c21e4.md);
two-way, open to Ryan as weigh-in items · closes status-audit findings G1 and D9.

wi-7c21e4 keeps client repositories' items off claude.ai, but only the skill's wording enforced
it: `board export` printed a note and exported anyway, a folder never initialised counted as an
own repository with an Artifact board, and a config that did not parse was silently replaced by
those same defaults. Now `board export` of a stored ledger refuses unless
`.kb/_ledger/config.json` exists, parses and chooses the `artifact` board. A folder never
initialised needs `weighin init --class own` (or `client`) once before its first board; that
is the cost of the guard, since a client folder cannot be told apart from an own one without
it. A config that cannot be read or parsed stops every command with the file and the fault.
Hooks never fail a session, so they carry on with the defaults: capture still writes to the
default ledger (as before, so the pilot on #23 measures unchanged capture), and every
SessionStart says the config is broken and that items captured now go to the default ledger,
not a `store` the config names. Export with no ledger at all keeps its old behaviour, since an
empty board carries no items.

## B-26

**On Windows, SessionStart copies the binary into the repository's git dir for the skill's shell.**
2026-10-13 · Claude's call in the AFK cloud session, two-way · from
[#127](https://github.com/Wet-Ink-Corporation/weigh-in/issues/127) · amends the skill's `W`
for Windows.

In the Windows desktop app, Ryan's check on 2026-10-13 found the hooks start
`libexec/weighin.exe` from the plugin folder, but the shell the skill runs `W` in cannot see
that folder (Glob can), so every sitting recorded nothing. The cause is not documented; the
Claude Code docs say native Windows runs commands unsandboxed. Whatever the cause, the shell
does see the repository it works in. So on Windows, inside a git repository, `SessionStart`
copies the running binary to `<git common dir>/weighin/bin/<version>/weighin.exe`, and adds
`Weigh-In binary for the shell: <path>` to the session's context. The skill's `W` falls back to
that path, run directly with no `sh`. The copy goes to the git dir even when the config keeps the
ledger elsewhere, since that folder may be one the shell cannot see. It is written to a `.part`
name and renamed into place, and copies of older versions are left where they are (B-01: nothing
is deleted). When the copy fails but one of this version is already there (a rename fails while
another session runs it), that one is named and the failure logged. A copy is replaced only when
its length differs: copies are kept per version, and two builds of one version have the same
length only in development. The path printed is always the running version's, so an older copy
is never run against the ledger (B-05). Outside git, and on other platforms, nothing is copied.
Capture is unchanged.

## B-27

**v1 is live.** 2026-10-19 · Ryan's call · closes Phase 4 of the [v1 plan](../v1-plan.md)
([#26](https://github.com/Wet-Ink-Corporation/weigh-in/issues/26)).

Ryan tagged 1.0.0 on 2026-10-19, after one week of use ([D-0019](#d-0019)). The
[release workflow](https://github.com/Wet-Ink-Corporation/weigh-in/actions/runs/39191411957)
passed. It published the plugin zip, the Linux musl, Windows msvc and macOS arm64 binaries, and
SHA256SUMS, and it moved the `marketplace` branch and `weigh-in-plugin`'s `main` to 1.0.0, with all
three binaries under `libexec` ([B-19](#b-19)). These are the plan's exit criteria and their
results:

| Criterion | Result |
|---|---|
| Action rate ≥ 60%, not-useful ≤ 10% | Met: week 1 of [#23](https://github.com/Wet-Ink-Corporation/weigh-in/issues/23), six folders: 90 presented, 73% action rate, 0% not useful. [#25](https://github.com/Wet-Ink-Corporation/weigh-in/issues/25) stays out of v1 |
| Capture precision ≥ 80% on the seed | Met: 15 of 15 (110 items, 27 labelled `capture`; wi-a9df17), with labels reviewed ([#22](https://github.com/Wet-Ink-Corporation/weigh-in/issues/22)) |
| No "pending" item reported as "done"; one sitting returns "no decisions need you" | Met: sittings whose writes failed said nothing was recorded, and the sittings on 2026-10-18 returned "no decisions need you" |
| Every V-item closed or deferred past 1.0 | Met: V1, V6 and V7 passed their re-checks on 0.9.0 ([#16](https://github.com/Wet-Ink-Corporation/weigh-in/issues/16), [#20](https://github.com/Wet-Ink-Corporation/weigh-in/issues/20), [#21](https://github.com/Wet-Ink-Corporation/weigh-in/issues/21)); V2 deferred ([B-17](#b-17)), V3 moot ([B-24](#b-24)), V4 and V5 closed |
| No ledger corruption, no unexplained `hook-errors.log` lines | Met: 0 hook errors across six ledgers in week 1 |
| Each machine reaches a working `/weigh-in` without Rust; Cowork works | Met on Windows and in Cowork through the `marketplace` branch ([#13](https://github.com/Wet-Ink-Corporation/weigh-in/issues/13)) and the organization-synced copy ([#76](https://github.com/Wet-Ink-Corporation/weigh-in/issues/76)). macOS and Linux installs are unverified. Projects tasks use a connected checkout's binary only when it is this release's ([#143](https://github.com/Wet-Ink-Corporation/weigh-in/issues/143)) |
| A 0.2.2 ledger opens and reads under the candidate | Met: fixture tests pass at 1.0.0 ([B-10](#b-10)) |

Three code reviews since 0.2.2 are recorded on #26, with every finding fixed, refuted or left
with a reason. Left open on purpose: whether a cloud session in this checkout captures twice when
the organization-synced plugin also loads (#143), and the capture items held back for the pilot
(#26). Next: slice 2 ([#27](https://github.com/Wet-Ink-Corporation/weigh-in/issues/27)).

## B-28

**Every session keeps a journal; `ci-check` is the CI contract.** 2026-10-19 · Claude's call in
the AFK cloud session, two-way, open to Ryan as weigh-in items · amends [B-21](#b-21) · from
[#145](https://github.com/Wet-Ink-Corporation/weigh-in/issues/145).

B-21 kept journals only in this repository's cloud sessions, through
`.claude/hooks/weigh-in-journal.sh`, so a pull request from a desktop, CLI or Windows session
carried no record of what its session raised and settled. Now the binary keeps them itself.
After every capture hook (`Stop`, `SubagentStop`, `PostToolUse`) and every command that can
append events (`raise`, `batch`, `decide`, `defer`, `dismiss`, `reopen`, `scan`,
`board apply`), it appends the session's own events to `.kb/_ledger/journal/<session>.jsonl`,
in the line format `weighin journal` prints. It does so whatever the command's result, since a
batch that failed in part has still applied its other operations.

- **Gates.** Nothing is written unless the session id can be a file name (not empty, no `/` or
  `\`, not starting with `.`), the folder is a git worktree, `.kb/_ledger/config.json` exists and
  parses, its `journal` key is not `false` (it defaults to `true`), and the session has an event
  the file lacks. With nothing to add, no directory or file is created. Journals are kept in git
  worktrees only, not in a Cowork folder outside git: the journal exists to be committed.
  Refusing `\` (the Windows path separator) goes beyond the script's list.
- **Append-only, deduped by line.** A complete line already in the file stays where it is,
  whoever wrote it, and an event whose line is already there (a trailing `\r` ignored) is not
  appended again. The only bytes ever removed are a last line with no newline that does not
  decode, which a run killed mid-write cut short; its event is appended again in the same write.
  A last line with no newline that does decode is kept and ended, rather than dropped as the
  script drops it, since it may be another session's event.
- **Lock and coexistence.** Writers take `<temp dir>/weigh-in-journal-<session>.lock`, the file
  the script locks, waiting at most 3 s (the script's `flock -w 3`); a writer that cannot get it
  skips, and the next run catches up. On Linux and macOS `flock(1)` and Rust's `File::lock` both
  use `flock(2)`, so the binary and the script take turns. The script still appends the whole
  ledger's dump to the same file, and the dedupe means neither writes a line twice. The lock
  file is never deleted and never sits beside the ledger ([B-01](#b-01)).
- **Failures.** A journal failure never changes a command's exit code or stdout: the CLI prints
  `weighin: warning: session journal not updated: …` on stderr. In hooks it goes to
  `hook-errors.log` like every other hook failure, so the next `SessionStart` says capture may
  be off. Never silent (H-10).

`weighin ci-check --base <ref> [--json]` is what CI reads. It uses only git and the checkout and
never opens the ledger. The JSON report (schema `weighin.ci-check/1`) has `base` and `head`
shas, `open_items`, `decisions` and `journal`. `open_items` are the items the journals at head
leave open, minus those an atom at head or a trailer in the range closes, unless a journal
reopened them later. "Later" is read in the range's commit order: a reopen keeps the item open
only when the commit that added its line comes after the one that added the atom or trailer
(an uncommitted reopen is latest, and one from before the range loses to a record in it). When
the atom and the reopen both predate the range, the journal stands, since an atom is never
deleted. Each one gives its triaged `door` and `authority` (null before triage, never the
raiser's hints) and a `blocks` value read from its deadline (the deferral's, else the
triage's):

| Deadline | `blocks` |
|---|---|
| `before-merge`, `before-merge:<…>` | `merge` |
| `before-release` | `release` |
| an ISO date | `none` |
| `blocks:<id>` | that item's, while it is open; else `unknown` (a cycle is `unknown`) |
| none, or anything else | `unknown` |

The raiser's free-text `blocks` field is not read. `decisions` are the decisions the branch
adds, from three sources: `item_decided` lines added to journals, decision atoms added under
`.kb`, and `Weigh-In:` trailers in the range's commits. Every message line is read, because a
squash merge puts trailers mid-body. Each entry has its option and the commit that added it, the
trailer's first, then the atom's, then the journal line's. Sources that name different options
give one entry each. `journal` checks that every complete line a journal held at base is still
in it at head, wherever it now stands (`removed`, `missing`, `altered`, `undecodable`). The
command exits 0 whenever it writes a report, whatever the report finds, and 1 only on its own
errors: a `--base` that names no commit (so CI must fetch the base), no git repository, git
failing, or I/O failing. A broken config stops it as it stops every command. What CI does with
the report is CI's policy.

These were Claude's calls, all two-way and open to Ryan:

- An atom-only decision reports `option: null`: decision atoms carry the recommended option but
  not the chosen one, and trailers and journals give it. The recommended follow-up, awaiting
  Ryan, is a `weighin_option` front-matter key, which would change the golden atom tests (HITL-1
  in the plan, option A).
- A date deadline is `blocks: none`, and `blocks:<id>` inherits through open items, else
  `unknown` (HITL-2).
- Journal failures in hooks go to `hook-errors.log` (HITL-3).
- Journals in git worktrees only, and `\` refused in session names (above).
- The skill's close-out asks for `.kb/_ledger/journal/` to be committed with the atoms, which
  is what makes a pull request carry its journal; the plugin version is not bumped here.

Not done: signing journals or reports, any policy on what blocks a merge, and replaying journals
into a new container's ledger (B-21).

## B-29

**The Claude Code mod ships inside `weigh`.** 2026-10-19 · Claude's call in the AFK cloud
session, from the spike on [#113](https://github.com/Wet-Ink-Corporation/weigh-in/issues/113);
two-way, open to Ryan · implements [D-0017](#d-0017)'s first step ([#114](https://github.com/Wet-Ink-Corporation/weigh-in/issues/114)).

The spike asked whether the plugin can carry a hooks module (`"modules"` in `hooks/hooks.json`
beside the command hooks) without breaking a route that works today. On Claude Code 2.1.303,
`claude plugin validate` and `package-plugin.sh --no-download` pass, and a `--plugin-dir` run
loads the module next to the command hooks. Claude Code 2.1.200 ignores `modules`: it validates
the zip, registers the plugin's four command hooks and runs SessionStart. The organization sync
already delivers a plugin with a module (temper 1.4.0 ships `./lens/register.tsx` and loads in
cloud sessions), so the sync does not refuse `modules` the way it refuses `plugin/bin/` ([B-03](#b-03)).
So the mod lives in `plugin/hooks/mod/register.tsx`, with its state contract in
`plugin/types/index.d.ts`, rather than in a second plugin. Not yet seen: the plugin loading in
Cowork with the module present, and `$.process.run` starting `weighin.exe` or `weighin.cmd` on
Windows; both are Ryan's runs on #113. A failure in either moves the mod to `weigh-mod@weigh-in`.

The mod reads `open --repo --json` and `digest --json` and writes only through `decide` and
`defer`, run by argv; it draws only in a terminal or the desktop app, and `WEIGHIN_MOD=0` turns
it off. Its tests (`plugin/hooks/mod/mod.test.ts`) run with `tests/mod-check.sh` (validate,
`claude plugin test`, strict `tsc`) and stay out of the zip. Running that script in CI is a CI
change, so it waits for Ryan.

## B-30

**The local board pane reads the board on stdout and answers through stdin, as surface `pane`.**
2026-10-19 · Claude's calls in the AFK cloud session, two-way, open to Ryan as weigh-in items ·
the CLI half of [#115](https://github.com/Wet-Ink-Corporation/weigh-in/issues/115) · follows
[D-0017](#d-0017) and keeps [B-25](#b-25)'s guard.

- **Export.** `weighin board export --stdout` prints the board state document (the same
  `BoardState` the Artifact board reads) as one JSON line, under a `local` or an `artifact`
  board. It writes no file, creates no ledger, and never publishes; Claude pushing a file to
  claude.ai is still the only way a board leaves the machine, and `--stdout` gives it no file.
  It refuses a folder never initialised, a broken config and board mode `off`, which shows
  nothing. Without `--stdout`, `board export` is unchanged and still refuses outside `artifact`.
  A flag rather than a `board state` subcommand, so one export path holds the guard.
- **Apply.** `weighin board apply` reads its answers from stdin when it is given `-` or no path,
  in the forms a file takes, at most 256 KiB, so the pane leaves no temporary file beside the
  ledger ([B-01](#b-01)). `--surface pane` (the default is `board`) records its decisions with
  `via: pane` and its other reasons as `in the pane`, refuses an answer for an item no sitting
  grounded (it has no card, so it is read-only in the pane), refuses under board mode `off` or
  with no config, and fails the command when any answer was refused, after applying the rest,
  since the pane sends one answer at a time and must know. A choice or deferral read from an older card
  (a stale `card_rev`) is refused by the answer validation the board already uses. From the board, `board apply` is unchanged.
- **Surface vocabulary.** `weighin_core::event::Surface` names the spellings of
  `item_presented.surface` and `item_decided.via`: `chat`, `board`, `local`, `pane`, `cli`. The
  stored fields stay strings, so no event changes shape. `weighin stats --json` gains
  `decided_via`, the decisions still standing by where they were taken, so pane, chat and board
  stay apart; the journal already keeps each event's surface. The text line of `stats` is
  unchanged.

Not done: the pane itself (the mod), and recording a presentation when the pane shows a card.

## B-31

**The default owner comes from git.** 2026-10-19 · Claude's call in the AFK cloud session,
two-way, open to Ryan as a weigh-in item · the [#27](https://github.com/Wet-Ink-Corporation/weigh-in/issues/27)
item "owner default from git config instead of `ryan`".

Decisions record `human:<owner>` and deferrals an owner, and the owner was `ryan` in every folder
whose config did not name one, which is wrong as soon as anyone else uses Weigh-In. Now, where
a folder has no usable config (none, or one that does not parse, which only hooks run on), the
owner is `git config user.name` as git reads it from the folder, made a slug: lower-case ASCII
letters and digits, any other ASCII character separating words, characters outside ASCII dropped,
words joined with `-` (`Ana Lima` becomes `ana-lima`). Without a name that leaves a word, it is
the local part of `user.email`, slugged the same way, and without that, `ryan` as before. A first
`weighin init` without `--owner` writes that owner into the new config; a config that exists
keeps its owner, and `--owner` still sets it. Folders with a config pay nothing: git is asked only
when there is none, and then only when an owner is needed (a decision, a deferral or `init`), so
hooks and reads never ask it.

## B-32

**The board pane writes through `board apply --surface pane`, and acks through `batch`.**
2026-10-19 · Claude's calls in the AFK cloud session, two-way, open to Ryan · the mod half of
[#115](https://github.com/Wet-Ink-Corporation/weigh-in/issues/115) and
[#116](https://github.com/Wet-Ink-Corporation/weigh-in/issues/116), over [B-30](#b-30)'s CLI.

- The pane (`plugin/hooks/mod/pane.tsx`) reads `board export --stdout` and `open --repo --json`,
  and sends each choice, deferral, dismissal or reopen as one Answer on stdin to
  `board apply --surface pane`, showing the CLI's receipt or refusal word for word. Raw items
  are listed read-only. Under board mode `off` or without a config it shows the export's one
  refusal line.
- The Answer format has no acknowledgement, so **Ack** on a default runs `weighin batch` with
  one `ack` operation, as the sitting's digest does. It is the pane's only write outside
  `board apply`.
- A deferral's flip is its trigger, as the CLI already records it, so the form asks for a
  trigger, a resolve-by and a default, not a separate flip.
- Where `$.ui.open` reports the pane is not placed, `/wi board` answers with the board as text
  and the band's Board button writes that text to the transcript.
- On a successful Write, Edit or MultiEdit the mod shows, as a toast and a band line, at most
  three of the decisions anchored at the file (`decisions --path --json`, newest first), each
  line clipped as `recall::Anchored::line` clips it. The PostToolUse hook gives the model at most
  three too, but ranked by the edited line, so where more stand the two can differ. An edit that
  lists none clears the band line; a failure to list them is told once a session. It never
  alters the tool call or its result.
- After review: one write at a time (a press while an answer is being written is ignored, so a
  double press is not refused as already answered); a write that timed out says it may have been
  applied and the board is read again; a pane waiting undrawn is not read at each turn end; a
  one-way door's deferral pre-selects no default; `/wi decide` and `/wi defer` pass typed text as
  `--flag=value` and the positionals after `--`; each load clears a status entry a previous load
  left up.

Not done: recording a presentation when the pane shows a card.

## B-33

**The repository is public, and CI runs on GitHub-hosted runners on all three platforms.**
2026-10-30 · Ryan's calls, two-way · amends [D-0012](#d-0012) (the repository half; release
packaging is unchanged) · records [#154](https://github.com/Wet-Ink-Corporation/weigh-in/pull/154),
[#157](https://github.com/Wet-Ink-Corporation/weigh-in/pull/157) and
[#159](https://github.com/Wet-Ink-Corporation/weigh-in/pull/159).

- Ryan made `Wet-Ink-Corporation/weigh-in` public after 1.1.0. `weigh-in-plugin`, the release
  repository the organization sync reads ([B-19](#b-19)), stays internal.
- CI moved to Blacksmith runners (#154) while hosted minutes were exhausted, and was moved back
  (#157), which paused the Windows and macOS `check` legs behind the `CI_NON_LINUX` variable.
  With the repository public, hosted minutes are free, and #159 runs the `check` matrix on
  `ubuntu-latest`, `windows-latest` and `macos-latest` again, which the `main` ruleset requires.
  No Blacksmith runner or action remains in the workflows.
- **The Cowork download is on, from 1.1.0.** Ryan chose to keep it on (2026-10-30), ending the
  dormancy [D-0012](#d-0012) put on [D-0003](#d-0003). No workflow change was needed:
  `release.yml` packages without `--no-download` when the repository is public, and the
  repository was already public when `v1.1.0` was tagged. The synced 1.1.0 skill is stamped
  `S=6c3e54e0…`, which is the SHA-256 of the public `weighin-x86_64-unknown-linux-musl` asset of
  v1.1.0 (checked 2026-10-30, correcting the first version of this entry, which said 1.1.0 could
  not download). CI keeps `--no-download`, because its zip has no matching release. A connected
  checkout of the `marketplace` branch stays the fallback when the download fails, and 1.0.0 and
  earlier still need it. Ryan checks the download in Cowork. The research clean-up is the rest of "going public" on
  [#27](https://github.com/Wet-Ink-Corporation/weigh-in/issues/27).

## B-35

**The Claude Code mod ships as its own plugin, `weigh-mod`.** 2026-10-31 · Ryan's call, two-way,
on a decision card in the project thread, after a test he ran · supersedes
[B-29](#b-29)'s placement of the mod inside `weigh`, as B-29 provided for · found by
[#113](https://github.com/Wet-Ink-Corporation/weigh-in/issues/113)'s check C.

On 1.1.2, in a desktop chat connected to a folder, Claude loaded `/weigh-in` and the chat then
refused it: "this plugin entry is off here: it or its plugin comes with hooks other than the ones
this session registered … or the plugin did not load cleanly". temper's skills failed the same
way. Two copies of 1.1.2, uploaded under test names, both loaded and ran in a new folder chat:
one with every command hook and without the mod, one with no hooks at all. So the hooks module
turns the plugin off there, not command hooks or the binary. Anthropic's docs say chat ignores
hooks "without an error" and that a mod may share a plugin with skills
([platform support](https://claude.com/docs/plugins/platform-support.md),
[build guide](https://claude.com/docs/plugins/build.md)); no documentation names this message.
The nearest upstream report is [anthropics/claude-code#103029](https://github.com/anthropics/claude-code/issues/103029).

So `plugin-mod/` is the `weigh-mod` plugin: `hooks/hooks.json` with only `modules`, the module,
its tests and its `types` contract, with state keyed `weigh-mod`. One plugin cannot reach
another's files, so `package-plugin.sh` gives it a copy of weigh's `libexec/` (launchers and
binaries from the same build); `plugin-mod/libexec/` never exists in the repository.
`marketplace.json` lists both, the `marketplace` branch and weigh-in-plugin carry both, the
release attaches both zips, the versions move together, and `marketplace-branch.sh` refuses a
`weigh` that ships `modules`. Claude Code users install `weigh-mod@weigh-in` beside `weigh`.

Trade-offs: a second install step in Claude Code, and the binaries twice in the release-only
repository. Not tried: an uploaded copy of `weigh` with its mod (the failing copy came through
organization sync), and macOS or Projects chats. If the app later ignores modules in chat as
documented, the mod can move back.

## B-34

**`/weigh-in` can be started by Claude, so it loads in desktop chats.** 2026-10-31 · Ryan's
call, two-way, on a decision card in the project thread · amends the research report's
`disable-model-invocation: true` for the skill (report §plugin layout) · found by
[#113](https://github.com/Wet-Ink-Corporation/weigh-in/issues/113)'s check C.

Since Chat and Cowork merged in the desktop app, a chat connected to a folder answers
`/weigh:weigh-in`, and temper's user-only `/temper:gate`, with "This command couldn't be loaded"
before anything in the skill runs; 1.1.1's guarded `!` line did not change that. The same chat
found the skill through Claude's Skill tool and was refused only for `disable-model-invocation`.
The flag is frontmatter, read before the skill runs, and the synced plugin is the same on every
surface, so it cannot be set per surface. The skill drops it; its description says to start a
sitting only when the user asks. The body already picks its route (binary, device shell,
Projects thread, or settling in chat without the binary).

Per Anthropic's plugin platform-support table, hooks are ignored in Chat, so SessionStart's
marker convention does not reach a chat and capture there rests on the sitting's sweep. Mods are
Claude Code only.

Trade-off: the description is now listed in every session's skill list, and Claude may start a
sitting unasked. Rejected: a second, Claude-startable skill beside a user-only `/weigh-in`, which
keeps Claude Code unchanged at the same listing cost and with two skills to keep in step.

## B-36

**Weigh-In moves into the `redkiln` monorepo, not redkiln-rs.** 2026-11-01 · records Ryan's suite
decision of 2026-10-13 (redkiln `references/decisions/README.md`: separately installable plugins
in one `redkiln` monorepo) · supersedes the redkiln-rs destination of
[wi-b4c1d8](wi-b4c1d8.md) and [B-22](#b-22).

Weigh-In arrives there as `rk-weigh-in` 2.0 in Move 1 (redkiln `docs/plan/move-1-weigh-in.md`),
with its store extracted into `redkiln-core` and `atom.rs` into `knowledge-contract`. wi-b4c1d8's
choice itself, a Rust crate and CLI on the shared 1.98.1 toolchain, still holds; only where the
code ends up changes. Loop's gates become Weigh-In requests (Blueprint call 7), replacing the
`decision` item type of slice 3.

