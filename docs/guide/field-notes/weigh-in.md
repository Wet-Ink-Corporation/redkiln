<!-- Research note gathered 2026-11-01 for the field guide (docs/guide/README.md). Kept as the evidence base behind the guide; it is not maintained, and the guide wins where they differ. -->

# weigh-in (+ weigh-in-plugin): hard-won lessons

Mined 2026-11-01 at `main` = `f0e553e` (1.1.3 plus Unreleased; 140 commits) and `weigh-in-plugin` = `59d0a76` (one commit, "Weigh-In 1.1.3 marketplace"). This file covers what the audit (`references/strategy/audit/weigh-in.md`, written at 0.6.1 on 2026-10-13) does not: everything from 0.6.2 to 1.1.3 (B-26 to B-35, D-0018 to D-0020, #126/#127/#143/#113/#169/#171/#173) and the reasons behind the rules. Unless a path says otherwise, decision ids are in `docs/decisions/build-decisions.md`.

## Guardrails

- **[plugin-packaging] No hooks module (`modules`) in a plugin that ships skills.** On 1.1.2, a desktop chat connected to a folder turned off the whole `weigh` plugin, skills included ("this plugin entry is off here: it or its plugin comes with hooks other than the ones this session registered …"). temper failed the same way. Ryan bisected with two uploaded copies: one with every command hook and no mod, one with no hooks at all. Both loaded, so the mod was the cause. Anthropic's docs say chat ignores hooks "without an error". The mod moved to its own plugin, `weigh-mod`, and `scripts/marketplace-branch.sh` now refuses a `weigh` whose `hooks.json` contains `"modules"`. redkiln needs the same check per plugin, enforced in CI. Evidence: B-35; #113 comments of 2026-10-31; `scripts/marketplace-branch.sh` (the `grep -q '"modules"'` line); upstream anthropics/claude-code#103029.

- **[plugin-packaging] No `plugin/bin/`, enforced at package time and again at branch build.** claude.ai rejects hosted plugins with a top-level `bin/`, because Claude Code puts it on PATH outside the admin approval surface. Binaries live in `libexec/<target>/` behind a launcher that is called by path. Both `package-plugin.sh` and `marketplace-branch.sh` refuse a `plugin/bin`. Evidence: B-03; `scripts/marketplace-branch.sh` (`[ ! -e plugin/bin ]`); comment block in `plugin/libexec/weighin:3-4`.

- **[plugin-packaging] `libexec/` takes an allowlist, not "whatever is there".** A rerun of the v0.4.0 `release` job downloaded every artifact of the run into `plugin/libexec/`, including the failed first attempt's `release-assets`. The zip and the `marketplace` branch came out at 32 MB instead of about 13. The job now downloads only artifacts named like a target (`*-*-*`). `package-plugin.sh` refuses anything except `weighin`, `weighin.cmd` and `<target>/weighin[.exe]`, and CI checks that refusal on every PR. Evidence: B-15; #43; CHANGELOG 0.4.1.

- **[events-store] Nothing beside the ledger is ever deleted, and read-only commands create nothing.** Cowork's connected folders refuse `unlink`, so creating a ledger under WAL or DELETE journals failed with `SQLITE_IOERR_DELETE` (2570). The fix was confirmed with strace and an unlink-refusing shim. Several rules follow from it. The store runs under TRUNCATE, and a stale `-wal` is renamed aside rather than deleted (`crates/weighin/src/store.rs:200` `set_aside_wal`). Read-only commands never create a ledger (`4b8f28d`, 0.5.1: `weighin --dir <folder> stats` used to leave an undeletable `.weighin/`). Writers create the store lazily on their first write (0.6.0, #95). Offset files are overwritten, never removed (B-11). Lock files live in the temp dir (B-28). Evidence: B-01; CHANGELOG 0.5.1, 0.5.4, 0.6.0.

- **[events-store] The schema version lives in `PRAGMA user_version`, and a newer ledger is refused untouched.** happenstance keeps its own counter in `store_meta` and never touches `user_version`, which leaves the pragma free for the product's version. Old ledgers (0.1.x WAL, 0.2.x TRUNCATE) migrate step by step inside one `BEGIN IMMEDIATE`. A ledger stamped higher than the build knows fails with "upgrade weighin" and is not written. The tests build real 0.1.x and 0.2.2 ledgers from copied payloads. Evidence: B-10; `crates/weighin/src/store.rs:349-387`.

- **[cross-platform] Only one binary version may touch a ledger.** Builds differ in journal mode, so an old build can reopen a ledger under WAL. The launcher picks the newest build among the listed targets (B-05). The Cowork fallback once took the first `weighin` in any connected folder, and in the 0.9.0 re-check that was a 0.2.1 build. It now runs a candidate only if `--version` matches the stamped `V`, and names the copies it skipped. Evidence: B-05; #143; `plugin/skills/weigh-in/SKILL.md` (Cowork step 1); CHANGELOG 1.0.0.

- **[hooks] Hooks never fail a session, and never fail silently.** Every hook error goes to `<state dir>/hook-errors.log`, and the hook exits 0. The next SessionStart counts the lines past an offset kept in `hook-errors.seen` and tells the user through `systemMessage`. With no binary at all, the sh launcher prints a fixed JSON "capture is off" line itself. That rule came from two silent outages, B-13 and D-0015 (see history). A further bug: after the log was truncated, the offset skipped new errors (fixed in 0.5.4). Evidence: B-11; `crates/weighin/src/hooks.rs:1-15,312-356`; CHANGELOG 0.3.0, 0.5.4.

- **[skills-prompts] The sitting never reports a write the CLI did not print.** In the Windows desktop app every `W` call failed, yet the model "dismissed" and "decided" items in prose, so the user believed they were recorded. The skill now has a hard budget rule ("never report a raise, dismiss, default, decision or deferral that the CLI did not print as written"). A **Without the binary** route says once that nothing was recorded, shows the command and its error, and keeps every item open ("would dismiss as recoverable"). This was an exit criterion of v1. Evidence: #127; `plugin/skills/weigh-in/SKILL.md` ("Budget", "Without the binary"); B-27 table row 3.

- **[trust-security] Client-data isolation is enforced in the CLI, not in skill wording.** wi-7c21e4 keeps client repositories' items off claude.ai, but at first only the skill's prose enforced it. `board export` printed a note and exported anyway, and a folder never initialised counted as an own repository with a publishing board. Now export refuses unless the config exists, parses and chooses `artifact`, and a broken config stops every command. A typo, `--class clinet`, was still stored with the publishing board (#169, Unreleased). `init` now refuses unknown classes and board modes. Evidence: B-25; #169; CHANGELOG 0.6.1 and Unreleased; wi-7c21e4.

- **[process] Capture behaviour is frozen while a pilot measures it.** From 0.6.0 to 1.0.0, every release note states that "capture … is unchanged, so the #23 pilot still measures 0.6.0's capture behaviour". 1.0.0 was the 0.9.0 candidate with no new Rust, so the build that earned the exit criteria is the build that shipped. Evidence: wi-7c3aa0; B-23; CHANGELOG 0.6.1, 0.6.2, 0.9.0, 1.0.0.

- **[github-ci] A release cannot ship without notes, off `main`, or half-published.** CI fails when the current version has no `CHANGELOG.md` section (`scripts/changelog-section.sh`). A release tag must be on `main`. The workflow refuses to publish anything when the deploy-key secret is missing. Reruns are idempotent: `gh release upload --clobber` runs when the release already exists. `release.yml` pins GitHub's ed25519 host key with `StrictHostKeyChecking=yes`. Evidence: B-09; B-19; `.github/workflows/release.yml:124-151`.

- **[distribution] The development tree is never org-synced, and a binary-less copy names itself.** claude.ai org sync of the dev repository's `main` delivered a copy with no binaries and a skill stamped `V=dev`. On Windows the exec-form hooks then failed with `ENOENT`. The fix: sync only the release repository, and the launcher now says "this copy has no binaries" when no `<target>/` build exists. Evidence: B-18; #76; `plugin/libexec/weighin` (the `nobuild` block).

- **[testing] Session setup never gates on lint.** temper's `[workspace.lints]` fail plain `cargo clippy` on baselined legacy findings. Under `set -e` that stopped the cloud `session-start.sh` before it built the binary, so capture was off in every cloud session without a word. Lints are judged by the gate (`baseline --check`) and never by setup; setup runs clippy only to warm the cache. Evidence: B-13; #38; CHANGELOG 0.4.0.

## Patterns

- **[hooks] Exec-form hooks run with no shell and reach a Windows `.exe` through one extensionless path.** Each hook is `{"command": "${CLAUDE_PLUGIN_ROOT}/libexec/weighin", "args": [...]}`. On Linux and macOS the kernel runs the sh launcher by its shebang, so it must be mode 100755, or the hook fails with `EACCES`. On Windows the same path resolves to `libexec/weighin.exe`, with or without Git on PATH. A `.cmd` cannot be spawned this way (`EINVAL`). The package therefore ships the msvc build twice: at `libexec/weighin.exe` for hooks and per-target for the sh launcher. Rejected: a second hook entry with `"shell": "powershell"` (runs twice where both shells exist) and `commandWindows` (ignored). Evidence: B-16; PR #56 spike under Bun; `plugin/hooks/hooks.json`.

- **[plugin-packaging] A small, careful sh launcher.** `plugin/libexec/weighin` does four things worth copying. It picks the newest build among the listed targets (`-nt`), so a stale native build cannot shadow a fresh cross-build. It falls back to a `weighin` on PATH, using `WEIGHIN_LAUNCHER=1` so it never re-execs itself. It drops the event name that `hooks.json` passes (`hook SessionStart`), which it needs to know when to speak but older binaries would refuse. With no binary it prints a fixed JSON line with only `uname` interpolated, so nothing needs escaping. The `.cmd` twin compares file ages with `xcopy /D /L` and matches the file name instead of the localised summary, and it names `System32\find.exe` by path so Git's Unix `find` is never picked up. Evidence: `plugin/libexec/weighin`, `plugin/libexec/weighin.cmd`; B-05; B-11.

- **[events-store] The library's schema is built on the temp disk and copied into a TRUNCATE file.** happenstance's `SqliteEventStore::open` assumes WAL, so `ensure_schema` builds a template database with it in a `tempfile::tempdir()`. It then `ATTACH`es that template and copies every table and index DDL, plus the store-identity rows, into `main` inside one `BEGIN IMMEDIATE`, stamping `user_version` in the same transaction. Concurrent creators therefore end with one store identity. The connection is then handed to `SqliteEventStore::new(conn)`. A failed `DETACH` is an error, not ignored (0.5.4). Evidence: `crates/weighin/src/store.rs:215-317`; B-01; CHANGELOG 0.5.4.

- **[events-store] Conditional appends, position-ordered folds and content-hash ids.** Every write carries a DCB guard: `Guard::NotYetRaised(id)` makes a raise idempotent, and `ItemUnchangedSince(id, pos)` detects a race. The projection folds by store position and "nothing here trusts timestamps for order". Ids are `wi-` plus the first 6 hex characters of `sha256(session \n normalised question)`, extended to 8 on collision. They are deterministic and need no coordinator, so a hook and the skill can raise the same item twice safely. Evidence: `crates/weighin/src/store.rs:63-75,154-186,435-504`; `crates/weighin-core/src/projection.rs:1-2`; `crates/weighin-core/src/id.rs`.

- **[observability] Committed per-session journals make history survive ephemeral containers.** After every capture hook and every command that writes, the binary appends the session's own events to `.kb/_ledger/journal/<session>.jsonl`. Lines are deduped (a trailing `\r` is ignored), and a complete line is never moved or removed. The only bytes ever dropped are an unterminated last line that does not decode (a run killed mid-write); its event is re-appended in the same write. Writers lock `<tmp>/weigh-in-journal-<session>.lock` with a 3 s wait and skip if busy. Rust's `File::lock` and `flock(1)` both use `flock(2)`, so the old shell hook and the binary take turns. Journals are written only when the session id is a safe file name, the folder is an initialised git worktree, and there is something new. Evidence: B-21; B-28; `crates/weighin/src/journal/append.rs:134-243`.

- **[cli-contract] `ci-check` reads only git and the checkout, never the local store.** `weighin ci-check --base <ref> --json` (schema `weighin.ci-check/1`) reports `open_items`, the `decisions` a branch adds (from journals, atoms and `Weigh-In:` trailers, each with the commit that added it) and a `journal` append-only check (`removed`, `missing`, `altered`, `undecodable`). Ordering problems are settled by commit order in the range, not timestamps. It reads trailers on every message line, because a squash merge puts trailers mid-body. It exits 0 whenever it writes a report and 1 only on its own errors. What blocks a merge is CI's policy. Evidence: B-28; `crates/weighin/src/ci.rs`, `crates/weighin/src/ci/report.rs`; #145.

- **[skills-prompts] The end-of-turn block grammar: forgiving everywhere except `q:`.** The fenced `weigh-in` block uses `key: value` lines (`kind`, `door`, `authority`, `q`, `did`, `flip`, `at`, `blocks`). Values are case-insensitive, take aliases (`todo` means deferral, `decision` means call) and fall back to defaults when unknown (question, unsure). A block without `q:` is not an item. Real use forced these refinements:
  - A comment is ` #` followed by a space or the end of the line, so `#123` issue references survive.
  - A block inside an outer four-backtick example fence is not captured.
  - `did: blocked; …` with an explanation stays open; before 0.6.0 only a bare `did: blocked` did (wi-64c8ea).
  - Question normalisation keeps a trailing `#`, `+` or `%`, so `C#?` and `C++?` no longer merge.

  Evidence: `crates/weighin-core/src/marker.rs:1-18,40-80`; CHANGELOG 0.6.0; `crates/weighin-core/tests/it/marker_props.rs`.

- **[skills-prompts] One line in the convention fixed recall.** SessionStart injects a convention of about 150 tokens (`CONVENTION`, `crates/weighin/src/hooks.rs`). Agents wrote "Decisions for you" lists as prose, which capture cannot see: 1 of 7 calls in the seed was captured. 0.5.2 added one sentence, "A list of calls for the user … gets one block per call", and recall rose to 12 of 13 on the following sessions. Another rule in the same text: "If you cannot ask the user, take no one-way path: `did: blocked`, then continue with independent work." Evidence: #68; CHANGELOG 0.5.2; `docs/roadmap.md` "Known gaps".

- **[human-decisions] Decision-quality rules live in types, not only in prompts.** `Card::validate` refuses a recommendation that has `fails_must` or is flagged evasive (`CardError::RecommendsEvasion`), and the CLI enforces 2 to 4 options. It also never defaults a one-way or unsure door, and it requires flip, trigger, resolve-by, default and owner on a deferral. The skill holds the rest:
  - Five cheap-first filters: settled, recoverable, authority and door, decide-now, evasion.
  - At most four cards, with no pre-selection on one-way doors.
  - "'No decisions need you' is a correct outcome."
  - "Make assumptions and keep going delegates the work, not authority-bound decisions."

  Evidence: `crates/weighin-core/src/card.rs:103-187`; `crates/weighin-core/src/board.rs:616`; `plugin/skills/weigh-in/SKILL.md` §2 and §4.

- **[human-decisions] Answers from a UI are untrusted input, and each one is validated against the card revision it was read from.** Board and pane answers go through `validate_answer`. A choice made against an older card (a stale `card_rev`) is refused. The pane sends one answer at a time, and `board apply --surface pane` fails the whole command if any answer was refused, after applying the rest. A write that timed out says "may have been applied" and re-reads the board. Decisions carry their surface (`chat|board|local|pane|cli`), so stats keep pane and chat apart. Evidence: B-30; B-32; `crates/weighin-core/src/board.rs`; `plugin-mod/hooks/mod/pane.tsx`.

- **[distribution] The marketplace branch and the release repository are one regenerated commit each.** `scripts/marketplace-branch.sh` unpacks the release zips into a fresh repository and commits it as the only commit. It uses `git add -f`, so a global gitignore cannot drop binaries, and `git update-index --chmod=+x` on the launcher and every binary, whatever the zip kept. It then force-pushes. Old binaries never pile up in history, and Claude Code re-clones a marketplace when it sees new commits. The same tree goes to `weigh-in-plugin`'s default branch over SSH with a deploy key, because org sync reads only a default branch. CI builds the same tree from every PR. Evidence: B-14; B-19; `scripts/marketplace-branch.sh`; `weigh-in-plugin` at `59d0a76`.

- **[distribution] One release repository can carry several plugins through org sync (now verified).** The audit listed multi-plugin org sync as unverified. `weigh-in-plugin`'s `.claude-plugin/marketplace.json` lists both `weigh` (`./plugin`) and `weigh-mod` (`./plugin-mod`). Ryan's check D on 1.1.3 saw `hooks module weigh-mod@synced loaded`, and `/wi open` answered. Each plugin needs its own copy of `libexec/`, because a plugin cannot reach another plugin's files, so the binaries appear twice. Evidence: `weigh-in-plugin/.claude-plugin/marketplace.json`; `git ls-files -s` there (identical blob ids under `plugin/libexec` and `plugin-mod/libexec`); #113 comment of 2026-10-31; B-35.

- **[cross-platform] When the skill's shell cannot see the plugin folder, SessionStart copies the binary to where it can.** In the Windows desktop app, hooks start `libexec\weighin.exe` but the skill's shell cannot see the plugin install folder (`…\local-agent-mode-sessions\…\rpm\plugin_…`). SessionStart therefore copies the running binary to `<git common dir>/weighin/bin/<version>/weighin.exe`, through a `.part` file and a rename. It prints `Weigh-In binary for the shell: <path>` into context, and the skill falls back to that path, using `& ` in PowerShell. Copies are kept per version and never deleted. Only the running version's path is printed, so an older copy is never run. Evidence: B-26; #127; `crates/weighin/src/hooks.rs:263-284`.

- **[testing] A labelled eval seed, scored through the real capture path, with a bar that applies only from a minimum sample.** `eval/seed.jsonl` holds real end-of-turn messages labelled `capture` or `skip`, `expect.status` `raised|defaulted`, and `by: claude-draft|ryan`. `tests/it/eval_seed.rs` runs each one through the real Stop or PostToolUse path in a scratch repository. The 80% precision bar is "in force" only once 10 items are captured (wi-a9df17), which avoids a 1-of-1 "100%". Sessions save their own turns before ending, because transcripts leave the sessions API before a later session can read them (s7 lost all but its last page). `scripts/seed.py save|review` (#170) automates this. Evidence: `eval/README.md`; wi-48657b; wi-a9df17; wi-100d6d.

- **[cost-capacity] What a sitting costs is set by the session's length, not by the cards.** The measurement: three requests re-reading a context of 585k tokens made up 60 to 75% of the $0.59 a sitting cost on Opus. Trimming the card output would have saved a cent or two. The v1 figure is therefore a formula: about 3 × the context at the cache-read price, plus about 5,600 output tokens. The design's $0.28 constant was dropped. Any end-of-session skill in redkiln should budget in requests, not tokens. Evidence: D-0016; #24.

## Gotchas

- **[skills-prompts] A failed `!` injection blocks the whole command on some surfaces.** In a desktop chat connected to a folder, the skill's injected `weighin open` failed and the command refused to load ("This command couldn't be loaded"). The old Cowork never ran the injection. The fix, `… 2>&1 || echo "(weighin did not run here)"`, makes the injection always succeed, and the skill treats that line as a placeholder. Related cases:
  - Cowork replaces `!` lines with a placeholder.
  - claude.ai-synced skills run `!` in cloud sessions, but in local terminal sessions Claude sees the line as literal text.
  - In cloud sessions the auto-mode classifier gave "no verdict" on the injected command until `.claude/settings.json` allowed the launcher.

  Evidence: CHANGELOG 1.1.1; #163; `docs/research/report.md:252`; wi-6b2fa5; `.claude/settings.json` allow rules.

- **[skills-prompts] `disable-model-invocation: true` makes desktop chats refuse the command.** Since Chat and Cowork merged, a folder chat answers user-only plugin commands (`/weigh:weigh-in`, temper's `/temper:gate`) with "This command couldn't be loaded". The flag is frontmatter, read before the skill runs, and a synced plugin is identical on every surface, so it cannot vary per surface. The skill dropped it and now says "only when the user asks" in its description. A `claude -p` check confirmed that "run a weigh-in" loads the skill and an unrelated prompt does not. The cost: the description is listed in every session. Evidence: B-34; CHANGELOG 1.1.2; commit `82f3b4b`.

- **[hooks] Surfaces differ, and `CLAUDE_CODE_ENTRYPOINT` is the only cheap signal.** The surfaces seen so far:
  - Cowork is `remote_cowork`: hooks run in a scratch cloud workspace, the user's files are behind the device shell, and `PostToolUse` never sees edits made there.
  - Claude Projects threads are `remote_projects`: hooks run and capture into a ledger that dies with the task. There is no `AskUserQuestion`, only `mcp__hearthbot__ask_decision`, one card per call, with each answer arriving as a new turn.
  - Desktop folder chats run no plugin hooks at all.

  `env::cowork()` still matches only `remote_cowork`, so Projects is routed by the skill's prose. Evidence: B-04; D-0002; D-0018; #126; B-34; `crates/weighin/src/env.rs:337-339`.

- **[headless-agents] Cloud sessions and plugins: repository wiring and org sync can collide.** Cloud sessions ignore a repository's `enabledPlugins` and `extraKnownMarketplaces`, but they do run its `.claude/settings.json` hooks and load `.claude/skills/`. Weigh-In is therefore wired through the repository there: it builds the binary, copies the skill with `${CLAUDE_PLUGIN_ROOT}` rewritten, and runs SessionStart after the build. Org-synced plugins, however, also load in cloud sessions, so this checkout may capture twice. That has been noted since #76 and never checked. Evidence: B-12; `.claude/hooks/session-start.sh`; #126 comment; #143; `docs/roadmap.md:32`.

- **[hooks] Plain SessionStart stdout reaches only Claude.** To tell the user something, emit JSON with `systemMessage`, and move the prime into `hookSpecificOutput.additionalContext` in the same object. Evidence: B-11; `crates/weighin/src/hooks.rs:230-251`.

- **[distribution] Rules of the marketplace and org-sync plumbing:**
  - Org sync reads only a repository's default branch, and "Sync automatically" fires on a push to it (B-19).
  - Claude Code leaves auto-update off for marketplaces that are not official, so the install guide turns it on (0.4.1).
  - A session started before a newly synced plugin arrived does not have it; start a new session (#113 check D).
  - Synced plugins get no npm dependency install.
  - Executable bits survive upload and sync (observed, undocumented).
  - Plugin limits are 200 MB uncompressed and 5,000 files.
  - A ruleset cannot name the workflow's `GITHUB_TOKEN` as a bypass actor, so the force-pushed `marketplace` branch has no ruleset (B-14).

  Evidence: B-14; B-19; CHANGELOG 0.4.1; `docs/research/shipping-native-binaries.md:16-24`; #113.

- **[cross-platform] Windows traps met in tests and use:**
  - `timeout` on PATH can resolve to `System32\TIMEOUT.EXE`, so tests enforce deadlines in-process (`ab89c71`).
  - A Windows runner ran about 5× slower and broke a 5 s deadline; "catch an infinite loop" bounds are now 30 s (`ae3ccb0`).
  - PowerShell 5's `Set-Content -Encoding UTF8` writes a BOM that broke config parsing (0.9.0).
  - Without Git for Windows, shell-form hooks run under PowerShell, where `sh` is missing, and capture was silently off (D-0015).
  - `\` must be refused in session ids used as file names (B-28).

  Evidence: commits `ab89c71`, `ae3ccb0`; CHANGELOG 0.9.0; D-0015; B-28.

- **[github-ci] "Require linear history" plus "merge commits only" leaves no merge button.** Phase 0 had turned on linear history. Once merge commits became the only allowed method, GitHub offered only the bypass and showed "This branch must not contain merge commits" on every PR. Ryan merged #166 through the bypass, then turned linear history off. Merge commits keep each PR's `Weigh-In:` trailers where they were written. Evidence: D-0020; `283c63f`.

- **[github-ci] Moving CI to another runner provider cost a round trip.** CI moved to Blacksmith runners while hosted minutes were exhausted (#154). When Blacksmith's free minutes also ran out, it moved back and paused the Windows and macOS legs behind an org variable, `CI_NON_LINUX` (#157). The matrix default kept `check (ubuntu-latest)` reporting, so the required check stayed green. Making the repository public made hosted minutes free, and #159 restored all three platforms. Evidence: B-33; PRs #154, #157, #159.

- **[trust-security] Auto mode gates workflow edits that cross repository boundaries.** The auto-mode permission check refused adding a `release.yml` step that pushes to another repository with a deploy key. It needed an explicit human allow (wi-dba31a). Separately, `ci.yml` and `release.yml` install the latest `@anthropic-ai/claude-code` from npm in jobs holding a write token, a risk accepted as "leave unpinned" (wi-497dc5). redkiln should revisit that, since a single release repository would hold every plugin's deploy key. Evidence: `.kb/_intake/decisions/wi-dba31a-*.md`, `wi-497dc5-*.md`.

- **[headless-agents] temper's SubagentStart/SubagentStop hooks never fired in cloud sessions.** No reviewer verdict was recorded across six runs, so temper's review check could not pass there, and an issue was filed against temper. Weigh-In's own `SubagentStop` capture is subject to the same doubt in cloud sessions. Evidence: `.kb/_intake/decisions/wi-680975-*.md`.

- **[git] A board URL in tracked config points every clone at one board.** Publishing the board writes its URL into `.kb/_ledger/config.json`. Committed, it would make every cloud container, each with its own short-lived ledger, overwrite the same board. The workaround is `git update-index --skip-worktree`; a per-machine location was never built. Evidence: B-20; `docs/roadmap.md` "Known gaps".

## Lessons from history

- **[observability] Capture was silently off three times, which is why "never off without a word" is a rule.**
  - In every cloud session, after temper's lints arrived (B-13, #38).
  - On every Windows machine without Git Bash, where shell-form hooks ran under PowerShell (D-0015, #21).
  - On a claude.ai-synced copy of the dev tree, which had no binaries (B-18).

  Each was found by a human noticing absent output, not by a check. SessionStart's `systemMessage` and the launcher's no-binary line came out of these. Evidence: B-11; B-13; B-18; D-0015.

- **[testing] A check that "did not reproduce" ran on a different install.** On 2026-10-06 the Windows "no binary" failure was recorded as not reproducing (B-20). In fact that run used the `marketplace` branch install, while the failure came from the org-synced copy of `main`. B-18 corrected the record: not reproducing it on another install showed nothing. Verification notes must name the exact install path and surface. Evidence: B-20 ("Corrected 2026-10-06 by B-18"); B-18.

- **[plugin-packaging] B-29 to B-35: a spike defaulted to the convenient placement and missed the failing surface.** The mod spike (#113) gathered evidence that org sync accepts `modules` (temper 1.4.0 loaded with one). The agent half defaulted to shipping the mod inside `weigh`, leaving the check on chat and Cowork to a later human run. That run, on the merged desktop Chat and Cowork app, found the plugin turned off entirely. It took three patch releases in one day (1.1.1, 1.1.2, 1.1.3), each fixing one layer: the injection, the invocation flag, then the module. Evidence: B-29; B-35; #113 comments; CHANGELOG 1.1.1 to 1.1.3.

- **[skills-prompts] Scanner noise needed several narrowing passes.**
  - One session's ledger got 212 TODO items from comment markers inside string fixtures (wi-52e471); markers inside double quotes are now ignored.
  - A bare `node_modules/` exclude did not match at depth (wi-e038ef).
  - The authority acronyms (SLA, PII …) matched inside Slack and slash.
  - An apostrophe was read as a VB comment inside `"can't connect"`.
  - `decide` appended its decision note to source lines of any language and could break code; it now annotates only markdown and `.txt`.

  Evidence: CHANGELOG 0.6.0; wi-52e471; wi-e038ef.

- **[cli-contract] A positional reference in a batch shifted when an earlier op failed.** In `batch`, `@N` refers to the Nth raise. Before 0.6.0, a raise that failed or did not parse dropped out of the numbering, so every later reference moved to the next item and an op could act on the wrong item. Now a failed raise keeps its slot, and a reference to it is refused. Any batch format with back-references in redkiln needs this rule. Evidence: CHANGELOG 0.6.0; #95.

- **[human-decisions] The board's single state document overflowed at about 700 defaults.** The Artifact `db` holds at most 262,144 bytes per document (V5). The board now carries the newest 200 unacknowledged defaults plus a total, and acknowledged defaults leave it. Evidence: #19; CHANGELOG 0.5.1; `docs/research/report.md` (V5).

- **[human-decisions] A board button recorded the recommendation as the owner's choice.** "Copy as prompt" with nothing picked copied the recommended option as a decision. Its defer text passed the trigger as `--flip` and omitted the required `--trigger`. Dismiss copied a decide request. All three were fixed in 0.6.0, and the button is now disabled until what Record needs is present. Evidence: CHANGELOG 0.6.0.

- **[process] `init` without flags silently reset a client repository to `own` with the publishing board.** Any `init` without `--class` used to reset class and board, for example `init --ledger` on a client repository. Cowork advice also suggested `--class own`. Now `init` changes only what its flags name, and the Cowork advice keeps the folder's class or defaults to `client`/`local`. Evidence: CHANGELOG 0.6.0; `54dbc70`; #169.

- **[process] Pilot exit criteria shortened on evidence and recorded as an amendment.** Week 1 met every target (90 presented, 73% action rate, 0% not-useful, 0 hook errors), so D-0019 closed v1 after one week instead of two. The not-useful trigger for per-category suppression (#25) never fired. Evidence: D-0019; B-27.

## Open problems

- **[events-store] Journals are not replayed into a new container's ledger.** Cloud sessions keep history in journals, but open, deferred and defaulted items do not come back in the next container, so cloud sessions must still settle everything before they end. Evidence: B-21 ("Not done"); B-28 ("Not done").

- **[hooks] Possible double capture when org sync and repository wiring both load.** This has been carried from #76 to #126 to #143 and is "left open on purpose". Evidence: `docs/roadmap.md:32`; #143.

- **[skills-prompts] Routing across six surfaces lives in untested skill prose.** `SKILL.md` carries the Claude Code, Cowork, Projects, Windows and no-binary routes, plus a one-line shell script that downloads and checks the binary. Every surface fix (B-26, B-34, D-0018) added a branch. Ryan approved a design on 2026-11-01, to be recorded as D-0021, that is not built yet:
  - `plugin/libexec/cowork-fetch.sh`, tested against a fake `$HOME/mnt`, with the version stamp moving into it.
  - A SessionStart `route: … · W=…` line for the skill to read.
  - Per-route `claude plugin eval` cases, run by hand before each release.

  Evidence: #171.

- **[human-decisions] Decisions are never re-examined.** Atoms carry `last_reviewed`, `reversibility` and `flip_condition`, but nothing reads them back. Ryan approved a design to be recorded as D-0022: a decision comes due 90 days after its newest decide or review, "still holds" widens `item_acknowledged`, and only two-way decisions with a flip condition come due. Automatic flip checks were dropped because flip conditions are free text. Not built. Evidence: #173.

- **[human-decisions] Atoms are hard to read by title, and they lack the chosen option.** An atom's `title` is the chosen option's title, so the knowledge base lists titles like "Yes", "Tighten now", "Lock and pin" and "Remove it". Atoms also record the recommended option but not the chosen one; `ci-check` reports `option: null` for decisions that exist only as atoms. A `weighin_option` key is proposed (HITL-1) and still awaits Ryan. Evidence: `crates/weighin-core/src/atom.rs:230-240`; titles in `.kb/_intake/decisions/*.md`; B-28 (HITL-1).

- **[cross-platform] Why the Windows desktop shell cannot see the plugin folder is still unknown.** The Claude Code docs say native Windows runs commands unsandboxed. B-26 works around the problem without explaining it. Evidence: B-26; #127.

- **[llm-judgement] Recall, not precision, limits capture.** Precision is 15 of 15, but recall is 15 of 27. Agents still state calls in prose without a block, and the sitting's sweep is the backstop. The Sonnet grounder and its 3-sample stability check, which were the real calibration piece, were never built. Evidence: `docs/research/report.md` (2026-10-19 amendment); #22; #27.

- **[distribution] Coverage gaps that were never closed:**
  - macOS and Linux installs are unverified.
  - The Cowork download, on since 1.1.0, is "untried" in Cowork.
  - Journals and `ci-check` reports are unsigned.
  - No board runs from Cowork (V2 deferred).

  Evidence: `docs/roadmap.md` "Known gaps"; B-17; B-28; B-33.
