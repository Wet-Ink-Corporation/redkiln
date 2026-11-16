# Hook conventions

Every `rk-*` plugin reaches Claude Code through hooks that call its binary, and several plugins' hooks run on the same events in one session. This section is what each hook owes the person and its siblings. [cli.md](cli.md) fixes the core of the contract in CLI-09 to CLI-11 (exec form, hooks fail open, guards fail closed, no build lets every hook through) and CLI-13 (all of it is one harness's protocol, read and written in one adapter per binary); the rules here add what those leave out.

The rules come from four repositories that ran hooks in production for a month or more, distilled in the field guide's [Hooks](../docs/guide/hooks.md) chapter. A rule lifted from it keeps the guide's number (HK-2, HK-4 to HK-8), so citations written before this section still resolve; the guide's HK-1 and HK-3 restate CLI-09 to CLI-11 and are not repeated, and HK-9 and HK-10 stay guidance (below). Rules new here start at HK-11. The SPEC wins where the guide's wording differs.

Everything here describes Claude Code's hook protocol (CLI-13): its payload on stdin, its response on stdout, exit 2 read as "block", `onFailure`, the shared Stop allowance. A second harness gets these rules restated for its own protocol when it is supported.

## Words

- A **hook** is one `hooks.json` entry: `<binary> hook <event>` (CLI-09).
- A **guard** is a `PreToolUse` or `PermissionRequest` hook that can refuse a call. It passes `--fail-closed` (CLI-10).
- A **Stop gate** is a `Stop` or `SubagentStop` hook that can block the end of a turn.
- Every other hook is an **observer**: it records, injects or reports, and never refuses.

## Failing open and closed

**HK-2. Never off without a word.** A hook that fails notes the failure where the person will see it, never only where the model will. Every hook error is appended to a log in the plugin's state directory and the hook exits 0 (CLI-09). The next `SessionStart` counts the lines added since the last report and tells the person through `systemMessage`, since plain `SessionStart` stdout reaches only the model. A plugin's `status --json` (CLI-12) counts the same lines between sessions. A launcher with no build prints one fixed JSON line itself, with nothing interpolated (CLI-11). Where writing a log would itself break a rule (outside a repository that opted in, or in Cowork, where hooks write nothing to disk), the hook says so in its own output instead. Work a hook leaves undone because its budget ran out (HK-5) is not an error; it is recorded and reported at the next `SessionStart` the same way, so a hook that is always late is not silent either.

**HK-11. A guard fails closed inside its binary, not through `onFailure`.** A guard's own process denies what it cannot judge (CLI-10): input over its cap or not JSON, a state it cannot read, a budget it is about to miss. Its `hooks.json` entry MUST NOT set `onFailure: "block"`. Claude Code treats a hook that cannot start (its executable missing) as a failure, so `onFailure: "block"` on a guard blocks every call it matches whenever the binary is absent, which is exactly the case CLI-11 lets through: on Windows an exec-form hook resolves only to `libexec/<binary>.exe`, and a copy with no build has none. What a guard gives up is a block when its binary is killed or exceeds its `timeout`; HK-4 and HK-5 make those rare, and HK-12 says why a guard is not the last line. A guard that exits 2 still blocks: exit 2 is read as "block" whatever `onFailure` says.

**HK-12. A guard is a tripwire.** A guard reads one tool call's input; it cannot see every route to what it protects, and a determined model or person can go around it. Guards exist to stop honest mistakes early and cheaply. Each guard's documentation names the control that is authoritative for what it protects: a required GitHub check, a branch ruleset, a sandbox, a permission rule, or CI. A guard is never described, in a skill, a message or a document, as the thing that makes an action impossible (field guide TS-1, T4).

## Time

**HK-4. A guard is linear-time by construction.** A guard that times out lets the call through (HK-11), so a slow match is a bypass, not a delay. A guard matches in linear time, without backtracking (the `regex` crate), caps its input (HK-6), and carries a timing test at its cap, with a looser bound on Windows.

**HK-5. A hook budgets its own time.** A hook plans against its own `hooks.json` `timeout`, measured from its start on a monotonic clock, and gives each sub-step an explicit slice of it: a store wait (ST-09: at most 2 seconds for a hook's durable write), a git call, a network call. It stops starting work it cannot finish before the timeout, and leaves time to write its output. A hook whose work can outlive one run keeps a heartbeat, so the next hook of its event can tell a run its timeout killed from one that was interrupted, and reports the difference (HK-2). Each binary pins its budgets against its `hooks.json` timeouts in a test.

**HK-13. Latency budget.** Every hook of every installed plugin runs on each matching event, in parallel, so their costs add up. A hook that asks no model and makes no network call SHOULD finish, start to end and the launcher included, within 500 ms at the 95th percentile on Windows, the slowest platform for starting a process; a guard SHOULD finish within 200 ms. A hook that does network or model work states its budget in its documentation, and its `timeout` bounds it (HK-5). The conformance kit measures these on Windows; until it does, a plugin's own tests may.

## Input

**HK-6. Cap stdin, read only the fields used, every field optional.** A hook reads at most a fixed number of bytes of its payload, set per event by its plugin. A cap is never below 64 KiB for `SessionStart`, 1 MiB for a guard, or 4 MiB for `Stop` and `SubagentStop`, and never above 8 MiB for any event. It reads only the fields it uses, each optional with a fallback, and ignores unknown fields, so a new Claude Code field never breaks it. A payload over its cap or not JSON is a failure (HK-2): an observer dispatches nothing, and a guard denies (CLI-10). A hook reads, appends or overwrites its own state files only when they are regular files or absent, and its state directory only when it is a directory, not a link.

## The shared Stop allowance

**HK-7. A Stop gate has honest exits, a loop breaker and its own cap.** Claude Code ends the turn anyway after eight Stop blocks in a row with no tool call between them (`CLAUDE_CODE_STOP_HOOK_BLOCK_CAP`), and every installed plugin's Stop gates draw on that one allowance. So a Stop gate:

- runs only when the turn changed what it gates;
- reads the end-of-turn `Status:` line (the end-of-turn grammar, a later SPEC section until it lands) and stands down on `WIP` or `BLOCKED` with a notice, and names that exit in every block message, so an agent with a genuinely blocked task has an honest way out;
- does nothing when `stop_hook_active` is true and the same tree is blocked for the same reason again, and says so loudly instead of looping;
- blocks at most **2** times in a row per session, resetting only on real progress (a tool call, or a stop with nothing to block), so that four plugins' gates together stay under the shared eight;
- treats an end the harness forced as an unhonest end, recorded so the next `SessionStart` can report it (HK-2).

A plugin that needs more than two blocks takes it as a SPEC change, because it spends its siblings' share. An observer's `Stop` hook never blocks, so it spends none of the allowance.

**HK-14. `SessionEnd` is not a checkpoint.** `SessionEnd` hooks share about a second and a half between them. Nothing that must happen (exporting a journal, pushing a ref, recording an outcome) waits for `SessionEnd`; it happens at `Stop`, or before the push that needs it.

## Independence

**HK-15. No hook waits on a sibling.** Matching hooks of every installed plugin run in parallel, in no fixed order, and a deny from any guard beats ask beats allow. So each hook is independent and idempotent: it reads no other plugin's hook output, waits on no other plugin's process or file lock beyond the store's own bounded wait (ST-09), and gives the same answer when run twice. A plugin that needs a sibling's fact reads it from the store, through the sibling's contract crate or `--json` CLI (blueprint design rule 4), and treats its absence as "not known yet".

## Messages

**HK-8. A refusal names the sanctioned route.** A guard's deny, and a Stop gate's block, says what to do instead, and that a retry of the same call will be refused again. A deny that only says no gets retried.

## Guidance, not rules

The field guide's HK-9 (a gate checks every checkout an agent touched, not only the session's `cwd`) and HK-10 (a hook writes nothing into a repository that did not opt in) stay guidance in [docs/guide/hooks.md](../docs/guide/hooks.md): HK-9 binds only a plugin whose gate follows worktrees, and HK-10 is already required where it matters, by ST-01 (the store is created only where its location says) and ST-13 (a `client` repository publishes nothing).
