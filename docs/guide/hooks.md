# Hooks

Every `rk-*` plugin reaches Claude Code through hooks that call its binary. Four repositories have run hooks in production for a month or more; this chapter is what they agree on, and where they learned it. The SPEC already fixes the core of the hook contract in CLI-09 to CLI-11 (`spec/cli.md`): exec form by path, hooks fail open but still exit 2 on usage errors, a guard denies with a deny decision and exit 0 (exit 2 only when it cannot write one), and no build lets every hook through. Where this chapter and the SPEC differ, the SPEC wins; HK-1 to HK-8 add what it does not yet say, and are candidate text for the fuller hook section that arrives with Move 2.

## Guardrails

### HK-1 Hooks fail open, guards fail closed, a missing install warns

This restates SPEC CLI-09 to CLI-11, which win where the wording differs. A hook that records, injects or observes exits 0 on any failure of its own (its usage errors still exit 2, CLI-09). A guard (a PreToolUse hook that refuses something) denies what it cannot judge with a deny decision and exit 0, and blocks when its binary runs and fails or times out (CLI-10): declare `onFailure: "block"` on the hook entry and keep the binary's own `--fail-closed` handling. With **no binary anywhere**, every hook exits 0, guards included, and SessionStart says that the plugin is off and why.

- **Why.** claude.ai syncs plugins into cloud sessions without their `libexec/` builds. anneal's old launcher made every fail-closed guard exit 2 when it found no binary, so every Bash, Edit, Write and Agent call was blocked and the only way out was to remove the plugin. Ryan's call was "warn, don't block".
- **Enforce.** A manifest test that `onFailure: "block"` appears exactly when the args carry `--fail-closed`, every entry is exec form with an explicit timeout, and every hook id is a subcommand the binary accepts (anneal's `hooks_manifest`). The launcher's no-binary path is a tested branch, not an accident.
- **Evidence.** anneal PR 140, `plugin/libexec/anneal:21-25,236-247`, `crates/anneal-cli/tests/it/hooks_manifest.rs`; Claude Code hooks reference (timed-out PreToolUse hooks do not block unless `onFailure: "block"`).

### HK-2 Never off without a word

Every hook error goes to a log in the plugin's state directory and the hook exits 0. The next SessionStart counts new lines past a stored offset and tells the **user** through `systemMessage`; plain SessionStart stdout reaches only the model. A launcher that finds no binary prints a fixed JSON line itself, with nothing interpolated that would need escaping.

- **Why.** Weigh-In's capture was silently off three times, each found by a human noticing absent output: in every cloud session after lints arrived (setup ran under `set -e`), on every Windows machine without Git Bash, and on an org-synced copy of the dev tree with no binaries.
- **Evidence.** weigh-in B-11, B-13, B-18, D-0015; `crates/weighin/src/hooks.rs:1-15,312-356`.

### HK-3 Hooks are exec form and call one binary by path

Each hook is `{"command": "${CLAUDE_PLUGIN_ROOT}/libexec/<binary>", "args": [...]}`: no shell, no `bin/` on PATH, no logic in the hook entry. On Linux and macOS the kernel runs the sh launcher by its shebang (mode `100755`, or the hook fails with `EACCES`); on Windows the same extensionless path resolves to `<binary>.exe`. A `.cmd` cannot be spawned this way (`EINVAL`), so weigh-in ships the Windows build at `libexec/<binary>.exe` as well as per target. The shared launcher (`tools/launcher/README.md`) ships only `libexec/<binary>` and `<binary>.cmd` today, so on Windows the exec-form hook has nothing to run; see [Tension T9](README.md#tensions-with-the-plan).

- **Evidence.** weigh-in B-16, `plugin/hooks/hooks.json`; anneal `docs/hooks.md:34-41`; redkiln `AGENTS.md`, Plugin rules.

### HK-4 A guard is linear-time by construction

A PreToolUse hook that times out lets the tool call through, so a slow match is a bypass, not a delay. Guards use linear-time matching (the `regex` crate, no backtracking), cap their input, and carry timing tests at the cap (1 MiB inputs, with a looser bound on Windows, which ran about 2x slower).

- **Why.** temper #155: a 64K-space input took 27 s against a 15 s budget; 16K `write_text` took 16 s and 32K `sed ` over 20 s, because each pattern rescanned the segment from every keyword. The port chose `regex` for exactly this reason (wi-e9386f).
- **Evidence.** temper #155, `CHANGELOG.md:27-29`, `@84d2a0f`, wi-779016.

### HK-5 Budget time inside the hook, not by trusting `timeout`

Each hook plans against its own timeout and measures as it goes. Sub-steps get explicit slices (anneal's guard gives each ledger statement 16 of its 100 ms, so a locked ledger still lets the deny out). A long Stop gate stops starting new uncached work before the hook timeout can cut it off (temper blocks before another gate after 540 s of a 1200 s hook). A heartbeat lets the next hook tell a timeout from an interruption.

- **Why.** With default timeouts, a stuck writer could cost an anneal bootstrap about 100 s; 7 of 16 temper Stop gates in one redkiln-rs session ended with no recorded outcome.
- **Evidence.** anneal `CHANGELOG.md:63` (#89, #108), `crates/anneal-cli/AGENTS.md`; temper wi-4521a7, #137, `scripts/lib/hooks.mjs:1305-1400,1621-1648`.

### HK-6 Cap stdin, read only the fields you use, and make every field optional

Caps are per event (anneal: 64 KiB for SessionStart, 1 MiB for PreToolUse guards, 4 MiB for Stop, 8 MiB for PostToolBatch). Input structs are `#[serde(default)] Option<…>` with unknown fields ignored, so a new Claude Code field never breaks a hook; a guard denies oversized or non-JSON input. Hook state files are refused unless they are regular files (`symlink_metadata`).

- **Evidence.** anneal `docs/hooks.md:62,135,184,209,266,389`, `crates/anneal-cli/src/hook/completeness.rs:52,70-81`.

### HK-7 The Stop gate has honest exits and a loop breaker

A Stop gate runs only when the turn changed what it gates. It reads the last `Status:` line of `last_assistant_message`; `WIP` and `BLOCKED` stand it down with a notice, and every block message names that exit. A loop breaker records, per agent context, which tree was blocked for which reason; when `stop_hook_active` is set and the same tree is blocked for the same reason again, the turn ends with a loud notice instead of looping. Each plugin keeps its own block cap (anneal uses 2; Claude Code's own cap is 8 blocks with no tool call between) and resets it only on real progress. When the harness forces an end, record it as an unhonest end so the next SessionStart can surface it.

- **Why.** Without exits, an honest agent with a genuinely blocked task can only lie or loop. Without a breaker, two red worktrees alternated forever (temper #135).
- **Evidence.** temper `scripts/lib/hooks.mjs:1264-1286,1623-1712`; anneal `docs/hooks.md:371-380`; Claude Code best practices (`CLAUDE_CODE_STOP_HOOK_BLOCK_CAP`).

### HK-8 Deny messages name the sanctioned route

A refusal says what to do instead and that a retry will be refused again. A deny that only says no gets retried.

- **Why.** Workflow subagents retried denied `git apply` patches (28 of 42 interim denials); the read-only reviewer tried to apply patches 13 times. Naming Edit/Write, or commit-then-cherry-pick, stopped it.
- **Evidence.** temper #140, `@e33efa4`, `CHANGELOG.md:39`.

### HK-9 Gate every checkout an agent touches, not the cwd

Subagents edit in `.claude/worktrees/agent-*`. A gate keyed on the session's cwd never sees them. Record every checkout an agent touches with its starting tree, and gate each one still changed at Stop.

- **Evidence.** temper #135, #157, `scripts/lib/hooks.mjs:1432-1620` (redkiln-rs had 9 reviews on one fingerprint and no Stop row).

### HK-10 A hook never writes into a repository that did not opt in

A hook checks for the plugin's own marker (`.redkiln/suite.toml`, or the plugin's config) before it creates anything in the working tree. Read-only commands never create a store.

- **Evidence.** redkiln-old `CHANGELOG.md:3116-3121` (telemetry scaffolded and `git add`ed into any repository); weigh-in `@4b8f28d` (`stats` left an undeletable `.weighin/` in a Cowork folder).

## Patterns worth copying

- **Scan the text as it will be after the edit, and refuse only new hits.** temper rebuilds the file post-edit, scans before and after with the same lexer, and denies only what is new. Legacy escape hatches never block, a new `unwrap` is refused before it reaches disk, and the same diff turns a removed test or an added `#[ignore]` into an ask rather than a deny (temper `scripts/lib/hooks.mjs:251-372`). Refusing at edit time beats discovering at stop time; SWE-agent's ablation found the lint-gated edit action its most important component (prior-art/agents.md §1).
- **Put the plugin's `libexec/` on PATH through `CLAUDE_ENV_FILE`.** Skills that run the bare binary need it on PATH; SessionStart appends one single-quoted `export PATH=` line to the env file, once, refusing a directory with control characters or `:`, and joining with `/` as text because `Path::join` inserted `\` on Windows (anneal `@dfb5cb0`, `@209ebd3`).
- **Mirror a hook gate as a GitHub status.** anneal's merge gate posts `anneal/verdict` from its own evaluation, so branch protection enforces what the hook could only trip on (anneal `@c34e565`, #116). See [TS-1](trust-and-security.md#ts-1-a-guard-is-a-tripwire-the-boundary-is-where-every-route-passes).
- **Guards say deny or nothing, never allow.** A PreToolUse `allow` widens the user's permission rules; silence does not approve. Use `ask` where a human can answer in place (removed tests, one-way edits) (Claude Code hooks reference; temper's test-count ask).
- **SessionStart injects facts, not instructions, and less when siblings are present.** `additionalContext` is capped at 10,000 characters and re-runs on resume. The blueprint's suite context budget depends on each plugin trimming itself (blueprint, One session, several plugins; prior-art/agents.md §6).

## Gotchas

- **`/clear` starts a new session id**, so state keyed by session must be re-injected (anneal wi-6f8350).
- **Hooks receive no Claude Code version**, and `CLAUDE_CODE_VERSION` was seen stale; no version check is possible from inside a hook (anneal wi-319ca0).
- **A hook that never fires in CLI smoke runs may be the only path elsewhere.** temper removed its `SubagentHandback` hook because smoke runs never called it; in cloud sessions it is the only route for a subagent's report, so every cloud session that changed Rust ended `BLOCKED` until it was restored. Test on every surface before removing a hook (temper wi-103aa0, #97).
- **Settings hooks fire inside `claude -p` children**, so an in-session plugin's hooks run in headless children unless the child is isolated (redkiln-rs F2). See [HA-6](headless-agents.md#ha-6-headless-isolation-is-chosen-per-auth-mode-and-checked-by-canary).
- **The Stop hook can change the reply an eval grades**: anneal's completeness check blocked in eval homes whose dotfiles looked like changed source (anneal `CHANGELOG.md:24`).
- **Claude Code's Bash sandbox mounts `/dev/null` over protected dotfiles**, so `git status` shows them as untracked character devices and a "tree is clean" precondition fails everywhere (anneal `@116424a`).
- **Desktop folder chats run no plugin hooks at all**, and Cowork runs them in a scratch cloud workspace where `PostToolUse` never sees edits made through the device shell (weigh-in B-04, D-0002, B-34).
- **Generated files are not source, but code under cache directories still is.** A "source changed" check tripped on `__pycache__` and `*.log`; a review then caught that `*.py` under `.cache` must stay source (anneal `@0b7f89e`, `@2b3cc9d`).
- **Facts about the hook protocol are checked against the installed binary's schema and recorded with the version**, never from memory (anneal `crates/anneal-cli/AGENTS.md:229-267`, checked at 2.1.296).

## Open problems

- **Regex claim detection over prose keeps growing.** anneal's completeness hook carries a hand-built grammar that still has named false positives and judges only the final message; any green test run satisfies it (anneal `docs/hooks.md:270-347`, #135).
- **The end-of-turn grammar diverged.** anneal records temper's `WIP` as a violation; the blueprint's one Core grammar fixes this at Move 2 (anneal `docs/hooks.md:394-400`).
- **Double capture** when org sync and repository wiring both load a plugin in a cloud session has never been checked (weigh-in `docs/roadmap.md:32`, #143).

## Prior art

- Claude Code's hooks reference: exit 2 blocks, exit 1 only warns; hooks fail open by default; `onFailure: "block"` makes a guard fail closed; silence is not approval. **Agree**, and we adopt `onFailure: "block"` on every guard. [Hooks reference](https://code.claude.com/docs/en/hooks)
- Anthropic: "If something has to happen every time, make it a hook", and keep the enforced set small. **Agree, with a caution**: each guard should map to a zero-exception rule. [Best practices for Claude Code](https://code.claude.com/docs/en/best-practices)
- `prompt` and `agent` hooks ask another model and are not deterministic. **Agree**: verdicts go through the binary as validated records. [Best practices](https://code.claude.com/docs/en/best-practices)
- Codex pairs approvals with an OS sandbox rather than hook scripts. **We differ, deliberately**: hooks are our quality layer; unattended lanes still need a sandbox. [Codex sandbox](https://developers.openai.com/codex/sandbox)
