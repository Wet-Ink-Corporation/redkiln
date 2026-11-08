# Hook rules for `plugins/weigh-in/`

Weigh-In's hooks ship in Move 1, before the SPEC's fuller hook section, which arrives with Move 2 (field guide [T3](../guide/README.md#tensions-with-the-plan); Move 1 step 9, #24). Until then these are the plugin's charter rules: SPEC CLI-09 to CLI-11 as written (`spec/cli.md`), and the field guide's [HK-2](../guide/hooks.md#hk-2-never-off-without-a-word) and HK-4 to HK-8 adopted for this plugin. The SPEC wins where the wording differs, and Move 2 lifts the HK rules into it. The `weighin` crate charter (`crates/weigh-in/weighin/AGENTS.md`, Hooks) carries the short form; this page has each rule, how Weigh-In meets it today, and what is not yet met.

Weigh-In's hooks are all observers: SessionStart primes, Stop and SubagentStop capture `weigh-in` blocks, and PostToolUse scans what `Write`, `Edit` and `MultiEdit` wrote and recalls decisions anchored at that file (`hooks.json`, `src/hooks.rs`, behind the adapter `src/harness/claude_code.rs`). None is a guard or a gate. A rule that speaks of guards, denies or Stop gates binds any such hook the plugin adds; it is written here so the hook arrives already under it.

## The rules

**CLI-09. Exec form, `hook <event>`, fail open.** As written: "A hook is `<binary> hook <event>`, called by path in exec form from `hooks.json` (`${CLAUDE_PLUGIN_ROOT}/libexec/<binary>`), never through a shell. Hooks fail open: a hook notes its failure in its output, or in one stderr line when stdout cannot be written, and exits 0, even on a bug. Its usage errors still exit 2, so a test runs every `hooks.json` entry's arguments through the binary."
- *Met:* exec form by path, `hook <event>` on every entry, exit 0 even on a panic (`run_hook_guarded`), usage errors exit 2 (clap). `cargo xtask plugins` checks exec form, the launcher copy and `hook` as the first argument. `every_hooks_json_entry_is_hook_event_runs_and_never_blocks` (`tests/it/cli.rs`) checks that every entry's args are `hook <event>` and runs each through the binary with its event's payload. Step 9 made Stop, SubagentStop and PostToolUse pass their event name; the binary still reads the event from stdin (`hook_event_name`), and the name on the command line is for the launcher and Windows (B-16).
- *Diverges:* Weigh-In does not note a failure in the hook's output, and nothing goes to stderr. Every failure, a stdout write failure included, is appended to `hook-errors.log` and reported to the person at the next SessionStart (HK-2), because a hook's error output would reach the session (`harness/claude_code.rs`, `respond`). Step 6's adapter module brings this in line with the SPEC, or Move 2's hook section takes the deferred report as an allowed form.

**CLI-13. Harness protocol at the edge.** Hook payloads and responses, transcript formats and `CLAUDE_*` variables are read and written in one adapter module per binary (root `AGENTS.md`, CLI first, harness second).
- *Met* (Move 1 step 6a): `src/harness/claude_code.rs` reads the payload into `hooks::HookInput`, runs the hook, and renders its `hooks::HookOutput` per event; `hooks`, `recall`, `env` and every command take plain values, and output is byte for byte what it was (every end-to-end hook test unchanged, render goldens per form). `cargo xtask plugins` fails on the protocol's tokens in any `.rs` file under `crates/` outside a binary's `src/harness.rs` and `src/harness/`, its integration tests, comment lines and a top-level unit test module as rustfmt writes it (`tools/xtask/src/harness.rs`).

**CLI-10. A guard fails closed.** It denies what it cannot judge with a deny decision on stdout and exit 0, and exits 2 with one stderr line only when it cannot write that decision.
- *Not applicable today:* Weigh-In ships no guard. A guard added later declares `onFailure: "block"` and carries a manifest test for it (field guide HK-1).

**CLI-11. No build, no block.** With no `weighin` build present at all, the launcher copies (`libexec/weighin`, `libexec/weighin.cmd`, from `tools/launcher/`) let every hook through, and `hook SessionStart` says once that the plugin is off (not in Cowork).
- *Met on Linux and macOS* by the sh launcher and its tests (`sh tools/launcher/test.sh`).
- *Not met on Windows.* An exec-form hook there resolves `libexec/weighin` only to `libexec/weighin.exe`, a build packaged at release (M1-E). A `.cmd` cannot be spawned that way (`EINVAL`, B-16), so `weighin.cmd` serves calls through a shell, such as the skill's, and never a hook. With no build at all there is no `.exe`: the spawn fails with `ENOENT`, no launcher runs, and no "off" message is printed. Claude Code reports a hook error, so the plugin is not off silently, but CLI-11's promise does not hold. A guard declared `onFailure: "block"` may then block every call; whether `onFailure` fires on a failed spawn is unverified (M1-E, field guide T9). Weigh-In has no guard, so it is not exposed, but a guard must not be added here until this is settled.

**HK-2. Never off without a word.** Every hook error is appended to `hook-errors.log` in the state dir (`<git-common-dir>/redkiln/weighin/`), and the hook exits 0. The next SessionStart counts the lines past the offset in `hook-errors.seen` and tells the person through `systemMessage`, since plain SessionStart stdout reaches only the model. A launcher with no build prints its fixed JSON line itself.
- *Met* (`hooks.rs`, module doc). Two places stay silent by design: input failures outside a git repository, where logging would create a state dir in the user's folder (field guide HK-10), and Cowork, where hooks write nothing.

**HK-4. A guard is linear-time by construction.** A PreToolUse hook that times out lets the call through, so a slow match is a bypass. A guard matches in linear time (no backtracking), caps its input, and carries a timing test at the cap.
- *Not applicable today* (no guard). PostToolUse is not a guard: a timeout there loses a capture or a recall line, never lets something through.

**HK-5. Budget time inside the hook.** Each hook plans against its own `hooks.json` timeout and measures as it goes; sub-steps get explicit slices.
- *Met* for the two hooks that do bounded network or store work: Stop's journal export, `Budget::STOP` (18 s worst case in its 20 s timeout), and SessionStart's import, `Budget::SESSION_START` (8 s in 10 s, store wait included); `hooks_json_timeouts_match_the_budgets` pins both timeouts. The store wait in every hook is `Wait::Hook` (2 s). *Not yet:* the guide's heartbeat, which lets the next hook tell a timeout from an interruption.

**HK-6. Cap stdin, read only the fields used, every field optional.** Caps are per event; a payload field is read as optional and unknown fields are ignored, so a new Claude Code field never breaks a hook; hook state files are read only when they are regular files.
- *Met:* fields are read by name from the payload, each with a fallback, and unknown fields are ignored. *Not yet:* stdin is read whole with no cap, and `hook-errors.seen` and the log are opened without a regular-file check. Both land in step 6b, in the adapter module that now reads hook payloads (`src/harness/claude_code.rs`).

**HK-7. The Stop gate has honest exits and a loop breaker.** A Stop hook that can block runs only when the turn changed what it gates, stands down on a final `Status: WIP` or `Status: BLOCKED`, names that exit in every block message, and breaks a repeated block of the same tree for the same reason.
- *Met by never blocking:* Weigh-In's Stop and SubagentStop hooks never block and never stop the session (no `decision` or `continue` in any response; pinned by the test under CLI-09), so they spend none of the shared Stop budget. Stop's export spends only its own time, within `Budget::STOP`, without waiting on a sibling hook. A nudge about a one-way question is a `systemMessage`, never a block, and never starts a sitting.

**HK-8. Deny messages name the sanctioned route.** A refusal says what to do instead and that a retry will be refused again.
- *Not applicable today* (no deny). Weigh-In's warnings follow its spirit: each names what to fix or run (the store path, the config file, `/weigh-in`).

## Not yet met

| Rule | Gap | Where it lands |
|---|---|---|
| CLI-09 | Failures go to `hook-errors.log` and the next SessionStart, not the hook's output or one stderr line | Step 6's adapter module, or Move 2's hook section |
| CLI-11 | On Windows with no build, the exec-form spawn fails (`ENOENT`): no launcher runs and no "off" message is printed | Move 2, with the SPEC's hook section and before Bench brings guards (M1-E) |
| HK-5 | No heartbeat to tell a timed-out hook from an interrupted one | Step 6, with the adapter module |
| HK-6 | Stdin read uncapped; state files not checked as regular files | Step 6, with the adapter module |
