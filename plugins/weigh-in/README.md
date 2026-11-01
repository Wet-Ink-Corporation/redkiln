# Weigh-In

A decision ledger for agent sessions. Agents mark what they assumed, deferred or could not
decide; hooks capture those marks and the open questions they write into documents; `/weigh-in`
filters them hard and settles the ones that need you in one sitting, at the end of the chat
that raised them. Every decision lands as a `.kb` atom that passes Redkiln's `validate --kb`;
the atoms become Knowledge's format (`knowledge-contract`) with the move into the
`redkiln` monorepo (B-36, superseding B-22's move into redkiln-rs).

Slice 1 of the design in *Settle agent questions while the context is warm* (2026-09-26).
It stands on its own until it moves into the `redkiln` monorepo as `rk-weigh-in` 2.0 (Move 1).

## How it works

| Stage | Where | Model tokens |
|---|---|---|
| Capture: `weigh-in` blocks in the agent's last message | `Stop` / `SubagentStop` command hook | 0 |
| Capture: "Open questions" bullets and new TODOs in written files | `PostToolUse` command hook | 0 |
| Past decisions anchored at an edited file, at most three lines per edit | `PostToolUse` command hook | ~40 per line shown |
| Convention and open counts | `SessionStart` command hook | ~150 per session |
| Sweep, five filters, grounding, cards | `/weigh-in` skill, in the chat that raised them | one sitting, 3–4 requests |
| Record | `weighin` CLI: event, atom in `.kb/_intake/`, source annotation | 0 |
| Ledger band and `/wi` commands, Claude Code only | the mod, its own plugin `weigh-mod` (`plugin-mod/`, B-35) | 0 |
| Local board pane: decide, defer, dismiss, reopen or ack grounded cards and defaults | the mod, through `board export --stdout` and `board apply --surface pane` | 0 |
| Toast for past decisions at an edited file, for the person | the mod's `tool.call` hook | 0 |

The ledger is an event log in happenstance's SQLite schema at
`<git-common-dir>/weighin/ledger.sqlite3`: shared by every worktree of the repo, never
committed. Writes are conditioned (DCB): raising an item is idempotent, and two sessions
cannot both decide the same item. Decisions reach git as atoms and as a
`Weigh-In: <id> decided <X>` commit trailer.

Where the ledger lives, first match wins: `--store <file>`; `$WEIGHIN_STORE`; `store` in
`.kb/_ledger/config.json` (set with `init --ledger <path>`, relative to the folder or absolute);
the git common dir; outside git, `<folder>/.weighin/`. It runs under a TRUNCATE journal, not
happenstance's WAL: WAL and SQLite's default both delete files beside the database, and Cowork's
connected folders refuse deletes. For a folder synced by OneDrive or Dropbox and used from more
than one computer, point `store` outside the synced tree.

## Cowork

Cowork runs skills and hooks in a cloud workspace whose project folder is scratch, deleted with
the session; your files are on your computer, behind its shell. So in Cowork:

- The skill runs `weighin` through your computer's shell, against the connected folder you
  worked in. With public releases, it downloads the Linux binary of its own release from
  GitHub on first use in a chat, into the shell's per-chat scratch space (outside every
  connected folder), and checks it against the SHA-256 stamped into the skill. Otherwise (1.0.0
  and earlier, packaged while this repository was internal, B-33), it uses the copy in a connected folder holding a checkout
  of the `marketplace` branch (`plugin/libexec/`, [install](docs/install.md#cowork)); a checkout
  of `main` has no binaries (B-18). Failing both, it settles in chat and hands you Y-statements.
- Hooks write nothing (`CLAUDE_CODE_ENTRYPOINT=remote_cowork`): `SessionStart` still teaches the
  marker convention and `Stop` still nudges on one-way markers. Edits to your folder go through
  the computer's shell, so `PostToolUse` never sees them; the sitting scans the files the
  conversation changed instead.
- Decisions land as atoms in `<folder>/.kb/_intake/decisions/`, and in the folder's own ruling
  or decision log when it keeps one.

## Docs

- [`docs/research/report.md`](docs/research/report.md): the research and the design, with dated amendments.
- [`docs/decisions/`](docs/decisions/README.md): every decision so far, including the board sitting and the calls made while building.
- [`docs/install.md`](docs/install.md): installing, updating and removing a release.
- [`docs/roadmap.md`](docs/roadmap.md): what is not yet verified, known gaps, and slice 2.

## Install

To use Weigh-In, install a release: [`docs/install.md`](docs/install.md) covers Claude Code
(from the `marketplace` branch, no Rust needed), Cowork, updates, uninstalling, where the ledger
is and what to do when capture is off. The rest of this section builds it from source.

```sh
# Linux / macOS / cloud: static binaries into plugin/libexec/<target>/
./scripts/build-plugin-bins.sh            # linux (musl) and windows (mingw) cross-builds
# Windows, native:
pwsh scripts/build-plugin-bins.ps1        # x86_64-pc-windows-msvc
```

Then in Claude Code: `/plugin marketplace add <path to this repo>` and
`/plugin install weigh@weigh-in`, plus `/plugin install weigh-mod@weigh-in` for the ledger band. Type `/weigh-in` everywhere. Cowork and claude.ai list plugin
skills by the skill's own name (`/weigh-in`); Claude Code lists them as `/<plugin>:<skill>`
(`/weigh:weigh-in`) but also resolves the bare name. A repo needs no setup; run
`/weigh-in init client` first in client repos, which sets the board to `local`. From Claude Code's `!` prompt on Windows, which is Git Bash, use
forward slashes: `! sh D:/repos/weigh-in/plugin/libexec/weighin init --class own`.

After pulling a new version, rebuild every binary you keep: the launcher prefers the native
`x86_64-pc-windows-msvc` build over the `gnu` one unless the `gnu` one is newer, and an old
build next to a new one would reopen the ledger under its own journal mode.

Cowork and claude.ai take plugins from your account, not from a local marketplace, and the
binaries are not in git. An organization that syncs `Wet-Ink-Corporation/weigh-in-plugin`, which
each release fills (B-19), gets every release; otherwise upload `weigh-in-plugin-<version>.zip`
from the GitHub release under Customize > Plugins. A plugin saved to your account also shows up
in Claude Code; if you then see `/weigh-in` twice, remove one of the two installs.

## Releasing

Bump `version` in `Cargo.toml`, `plugin/.claude-plugin/plugin.json` and `plugin-mod/.claude-plugin/plugin.json`, move `## Unreleased` in
`CHANGELOG.md` to the new version, merge to `main`, then push the tag `v<version>`. Push only the
tag: the workflow creates the GitHub release itself, refuses a tag that is not on `main` or has no
changelog entry, and a rerun updates the release it made. The release workflow tests, builds the
Linux (musl), Windows (msvc) and macOS binaries, packages the plugin with its skill stamped to that
Linux build's SHA-256, publishes the release with the zip, the raw binaries and `SHA256SUMS`, and
then replaces the `marketplace` branch with one commit holding that zip's plugin and
`marketplace.json`, which Claude Code installs from (D-0009, `scripts/marketplace-branch.sh`), and
pushes the same commit to the default branch of `Wet-Ink-Corporation/weigh-in-plugin` for the
organization sync (B-19). The zip and the Linux binary must come from the same build, which is why
the release is made only there; `scripts/package-plugin.sh` run locally gives a zip that matches
only a local build. The download needs public release assets: the workflow stamps the checksum only when the
repository is public, as it is since 2026-10-30 (B-33). A local build has no matching release, so
package it with `scripts/package-plugin.sh --no-download`; the skill then uses the binary in a
connected checkout of the `marketplace` branch.

## Commands

```
weighin [--store <file>] [--dir <folder>] [--session <id>] <command>
weighin prime                            the marker convention and open counts
weighin init [--class own|client] [--board artifact|local|off] [--board-url <url>] [--owner <who>]
             [--ledger <path>]       a first init takes the owner from `git config user.name`
                                     (else the email's local part, else `ryan`); later ones keep it
weighin open [--repo | --all] [--with-context] [--top <n>] [--json]
                                         open items, ranked (one-way, blocking, authority first);
                                         `--all` lists closed ones too
weighin show <id> [--json]               one item in full
weighin context <id> [--json]            script-built evidence, ≤600 tokens
weighin raise --q <question> … | --markers
                                         raise by hand, or every `weigh-in` block on stdin
weighin batch [--file <ops.json>]        raise / triage / ground / present / decide / defer /
                                         default / dismiss / reopen / supersede / link / ack
weighin decide <id> <option> [--note] [--via <channel>]
                                         record a decision, write the atom, annotate the source
weighin defer <id> --flip --trigger --by --default [--reason]
weighin dismiss <id> <why> [--note]      not-useful | recoverable | moot | already-decided:<ref> |
                                         duplicate-of:<id> | not-my-call:<who>
weighin reopen <id> [--note]             reopen a decided, defaulted or dismissed item
weighin digest [--json]                  defaults taken without you
weighin stats [--json]                   action rate, not-useful rate, overrides, time to decide (ledger + committed journals)
weighin journal                          every ledger event as JSON lines; sessions keep their own in
                                         .kb/_ledger/journal/ (B-28)
weighin ci-check --base <ref> [--json]   CI report: open items, decisions since <ref>, append-only
                                         journals (schema weighin.ci-check/1)
weighin trailer <id>                     the commit trailer for a decided item
weighin decisions --path <file> [--line <n>] [--json]
                                         decisions still standing that are anchored at a file,
                                         nearest the line first, else newest first
weighin scan <path>…                     backfill from existing documents
weighin board export [--out <file> | --stdout]
                                         write the board state document; `--stdout` prints it
                                         under a `local` or `artifact` board and writes no file
weighin board apply [<path> | -] [--surface board|pane]
                                         apply board answers (a file, a directory, or stdin);
                                         `--surface pane` records them as the local pane's
weighin hook                             hook entry point (reads the hook JSON on stdin)
```

### Decisions on edits

When a `Write`, `Edit` or `MultiEdit` touches a file that a decision still standing is anchored
to (its `at:`, or the file it was scanned from), the `PostToolUse` hook adds that decision to
the model's context, one line each and at most three per edit:
`<id> decided <choice>: <question> (path:line)`. After an `Edit` the decisions nearest the edited
line come first; after a `Write`, the newest. Only decided items count: one reopened,
superseded or dismissed since is not shown, and neither is a default (the agent's own call, or
one taken on expiry), which the digest reviews. The hook still raises exactly what it did
before and nothing more; a failure here goes to `hook-errors.log` and adds nothing. It works on
every surface where hooks run, cloud sessions included, but not in Cowork, whose hooks run in a
scratch workspace (#116). `weighin decisions --path <file>` lists the same decisions.

### Session journals and CI

In an initialised git repository, every session keeps its own events in
`.kb/_ledger/journal/<session>.jsonl`. The binary appends them after each capture hook and each
command that changes the ledger, append-only and each line once, so a pull request that commits
`.kb/` carries what its session raised, deferred, dismissed and decided (B-28). Set
`"journal": false` in `.kb/_ledger/config.json` to opt out. A journal that cannot be written
leaves the command's result alone: the CLI warns on stderr, and hooks log it to
`hook-errors.log`.

`weighin ci-check --base <ref> --json` reads only git and the checkout, never a ledger. It reports
`open_items` (each with its triaged `door` and `authority`, or `null` before triage, and
`blocks`: `merge`, `release`, `none` for a date deadline, or `unknown`), the `decisions` the
branch adds with their option, commit and `sources` (`journal`, `atom`, `trailer`), and
`journal.ok` with the `violations` of the append-only rule. It exits 0 whenever it writes a
report, whatever the report finds, and 1 only on its own errors: a `--base` that names no
commit, no git repository, git or I/O failing, or a broken config. CI must have the base
commit, so check out with `fetch-depth: 0` or fetch the base first.

## Guards the CLI enforces

- A card has 2–4 options; the recommendation cannot fail a MUST or be flagged evasive.
- One-way and unsure doors are never defaulted; authority-bound items default only on expiry.
- A deferral names the fact it waits for, a trigger, a resolve-by and a default.
- An authority-sounding item (compliance, isolation, retention, security, migration, …) is
  dismissed as "already decided" only against an accepted `kb-…` atom or the user's own words.
- An agent's two-way "judgment" marker with authority words stays open instead of defaulting.
- `board export` writes a board only where `.kb/_ledger/config.json` chose the `artifact` board:
  it refuses a `local` or `off` mode, a folder never initialised (run `weighin init --class own`
  or `--class client` first) and a config that does not parse (wi-7c21e4).
- `board export --stdout`, for the local board pane, prints the state under a `local` or
  `artifact` board and never writes or publishes it; it refuses `off` and a folder never
  initialised. `board apply --surface pane` records decisions with surface `pane`, leaves items
  no sitting grounded read-only, and fails when an answer is refused (D-0017, B-30).

## Measured in this build

Measured at 1.0.0 (2026-10-19), against the v1 exit criteria ([v1 plan](docs/v1-plan.md),
Phase 3). Every criterion is met; macOS and Linux installs are unverified.

- **Tests and lints.** 297 tests at 1.0.0 (113 core unit tests, 22 core property
  tests, 7 core doctests, 93 CLI unit tests, 62 CLI integration tests including the eval scorer;
  one more end-to-end test runs on Windows only). CI runs them, fmt and rustdoc on Linux, Windows
  and macOS; `cargo deny`, the plugin package check and temper's lint
  ratchet run on Linux. The temper baseline is 0 findings (721 remediated in #82). Atoms pass
  Redkiln's `validate --kb`, with a rejected negative control.
- **Reviews.** An independent review at 0.1 found eleven defects; all are fixed, each with a
  regression test. The `/code-review` of everything since 0.2.2 (#26) found 14 candidates.
  Seven were verified and fixed: two in the CLI, five in scripts and workflows. One was refuted
  by the B-16 spike, and six were left with reasons on #26. A second pass at 0.5.4 found 34 and
  fixed 24 (#93 to #95); a third at 0.6.2 found 17 and fixed 5, with the rest left with
  reasons on #26.
- **Upgrades.** A 0.1.x WAL ledger and a 0.2.2 TRUNCATE ledger open, are stamped with the schema
  version and read back (fixture tests, B-10). Cowork's refused deletes, reproduced with a shim
  that refuses `unlink`, break 0.1.1 with `SQLITE_IOERR_DELETE` (2570). From 0.1.2 the store
  creates, raises and decides with no delete or rename in the folder.
- **Capture precision** (eval seed, #22): 110 real end-of-turn messages from cloud sessions on
  this repository, 27 labelled `capture`, 15 captured. Precision is 15 of 15 (100%, bar 80%, in
  force since 10 items are captured, wi-a9df17); recall is 15 of 27. At 72 items, recall was 1 of
  7 before the 0.5.2 convention line and 12 of 13 after it. Ryan reviewed every label up to the
  0.6.1 session; later items are `by: claude-draft`.
- **Sitting cost** (D-0016): about 3 × the session's context at the cache-read price, plus about
  5,600 output tokens of cards. That is about $0.10–0.15 on Sonnet 5.5 after a short session and
  about $0.47 after a 585k-token one. Earlier live runs (`--plugin-dir`) cost $0.42–0.57, against
  the design's $0.28.
- **Surfaces.**
  - Windows: installed from the `marketplace` branch with no toolchain, hooks fired and a sitting
    decided an item (V7, #13). Exec-form hooks run with or without Git Bash (B-16, spiked on all
    three platforms).
  - Board: in the desktop Code tab (V6). The `db` store's document limit is 262,144 bytes, and a
    board capped at 200 defaults measures 59,678 bytes at 1,000 (V5). Editor links from the
    board are not supported (V4).
  - Cowork: the zip-synced plugin keeps the launcher's executable bit and shows the SessionStart
    line.
  - Releases: since 0.5.4, each release reaches claude.ai's organization sync through
    `weigh-in-plugin` (B-19), with Linux musl, Windows msvc and macOS arm64 binaries and
    `libexec/weighin.exe`. A copy synced from the development tree has none (#76); from 0.5.4
    the launcher says so (B-18).
  - The synced `weigh-in-plugin` copy shows its SessionStart line with binaries on Windows and in
    Cowork (#76, closed 2026-10-13).
  - Ryan's checks on 2026-10-13: the Cowork session id is real (V1) and hooks capture on Windows
    without Git Bash (V7). They also found cards printed as text and the ledger left in the cloud
    in Projects tasks (#126), and a Windows desktop sitting that could not reach the binary
    (#127); 0.6.2 fixes both (D-0018, B-26).
  - Ryan's re-checks on 0.9.0 (2026-10-18): the Windows desktop sitting reaches the ledger with
    and without Git Bash (#127, #21), and a Projects task with a connected folder shows a
    tappable one-way card, records nothing before the tap, and writes the decision and its atom
    into the folder (#126, #16, #20). That run found the connected-folder binary fallback took
    any version; from 1.0.0 it only runs this release's (#143).
- **Use** (#23, one week by D-0019): 2026-10-12 to 10-18 across six folders (this repository,
  four of Ryan's other repositories and a connected folder). 90 items presented, action rate
  73% (target at least 60%), not-useful 0% (at most 10%, so per-category suppression, #25,
  stays out), 9 overrides of 66 decided, 0 hook errors. `weighin stats` and
  `scripts/weekly-stats.sh` produce these.

## Open items

Verification spikes (V1–V7), known gaps and slice 2 are tracked in [`docs/roadmap.md`](docs/roadmap.md).
