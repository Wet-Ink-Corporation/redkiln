# Bench (`rk-bench`)

> **Imported from anneal (Move 2).** anneal moved into the redkiln monorepo as Bench: the plugin
> installs as `rk-bench`, its cockpit mod as `rk-bench-cockpit`, and the CLI is `bench`. The
> event names, the ledger (`<git-common-dir>/anneal/ledger.sqlite3`), `.anneal/`, `~/.anneal`,
> the `/anneal` cockpit pane and the `$ANNEAL_*` variables keep their anneal names until later
> steps of the move (`docs/plan/move-2-bench.md`).

Bench is a Claude Code plugin that turns prose SDLC gates into enforced checks. It has three layers:

1. **Skills** for each phase of the work (`grill`, `spec`, `plan`, `build`, `diagnose`, `verify`, `ship`) and a few user-invoked ones (`/rk-bench:orchestrate`, `/rk-bench:handoff`, `/rk-bench:correct`, `/rk-bench:write-skill`, `/rk-bench:calibrate`).
2. **A deterministic CLI**, `bench`, behind hooks on Claude Code events, backed by an append-only event ledger.
3. **Calibrated judgement calls** (Clef decision points) that run in shadow mode until their calibration says they may enforce.

Status: not released. `rk-bench` ships 0.1.0 from the suite's marketplace once it runs on the catalog Loop brings (Move 4). Until then, anneal 0.0.5 keeps installing from the archived anneal repository's `marketplace` branch.

## Install

Requires Claude Code **2.1.295 or later**.

Once `rk-bench` is released, install it from the Redkiln suite's marketplace, as the other `rk-*` plugins are installed:

```
/plugin install rk-bench@redkiln
```

For the cockpit in Claude Code (a status band above the prompt and an `/anneal` pane, see `docs/bench/mods.md`), also install its plugin from the same marketplace:

```
/plugin install rk-bench-cockpit@redkiln
```

Before the release, run it from a checkout: build the CLI (`cargo build --release -p bench`), put `target/release/bench` on your PATH, and start Claude Code with `claude --plugin-dir plugins/bench` (add `--plugin-dir plugins/bench-cockpit` for the cockpit).

The cockpit is a separate plugin because the Claude desktop app's chats turn off a whole plugin that ships a mod, skills included. Kept apart, `rk-bench` loads its skills there and the cockpit is installed only where it draws. Every option is optional: with none set, Bench runs its deterministic gates and Clef stays off (see Clef below).

### How the CLI is found

The hooks run the `bench` CLI through the plugin's `libexec/bench` launcher (`bench.cmd` on Windows), by path; it is the suite's shared launcher (`tools/launcher/README.md`). Your shell's PATH is not changed. For the session's own Bash calls, where the skills run `bench` commands, the session-start hook appends one line to the script Claude Code sources before each Bash command (`$CLAUDE_ENV_FILE`), `export PATH='<plugin>/libexec':"$PATH"`, so `bench` there is the launcher. The line is written once per file. A plugin directory holding a control character or a `:` (a Windows drive path among them) is not written; the session start notes it instead, and `bench` must then be run by its full path. The launcher takes the first of:

1. a build in the plugin's `libexec/<target>/`;
2. a `bench` on PATH, skipping the launcher's own directory.

If neither works, Bench is off for the session: every hook exits quietly, the guards (`guard`, `merge-gate`, `preconditions`, `brief`) included, and the session start says so. Once a binary runs, a guard that fails or times out still blocks. anneal's download-and-verify fallback is not part of the shared launcher yet (M2-E).

### Where it works

| Surface | Skills | Hooks and gates | Clef |
|---|---|---|---|
| Claude Code CLI and desktop (2.1.295 or later) | yes | yes | yes |
| Claude Code on the web (cloud sessions) | yes | yes, with `bench` on PATH in the environment's setup script | no: Clef's client ignores proxy settings, and cloud sessions reach the internet only through a proxy, so every call is unreachable and fails open |
| claude.ai chat and Cowork | yes | no | no |

Hooks run only in Claude Code, so in claude.ai chat and Cowork nothing enforces the gates, and a skill's `bench` commands work only where the CLI can run. The cockpit (`rk-bench-cockpit`) draws only in the Claude Code CLI and the desktop Code tab; the desktop app's chats turn off a plugin that ships a mod, so that plugin stays separate from `rk-bench`.

### Status line (once, in user settings)

Bench's context monitor warns at 60% and stops at 75% of the auto-compact window, so long sessions hand off before compaction loses their state. Hooks receive no context usage, so the monitor reads what `bench statusline` saves. Plugins cannot set the main status line, so wire it once in `~/.claude/settings.json`. On macOS and Linux, this runs the newest installed Bench through its launcher, so it survives plugin updates:

```json
{
  "statusLine": {
    "type": "command",
    "command": "sh -c 'exec \"$(ls -td \"$HOME\"/.claude/plugins/cache/*/rk-bench/*/libexec/bench | head -n 1)\" statusline'"
  }
}
```

With `bench` on your PATH (and on Windows), the command is just `bench statusline`. If you keep another status line, have your script pipe its input to `bench statusline` as well; it prints one line (`anneal ctx 120k/200k 60%`) and saves the reading to `~/.anneal/context-usage.json`.

The window is `CLAUDE_CODE_AUTO_COMPACT_WINDOW` when set, else the model's context window, else 200,000 tokens. Until a status line saves a reading the monitor stays silent, and the session start shows a one-time setup hint. The status line is the fallback for the cockpit mod, which saves the same reading through `bench snapshot write --json` (to `~/.anneal/context-usage-mod.json`); when both save one, the monitor reads the fresher, and while the mod has saved one in the last 10 minutes the setup hint stays away. See `docs/bench/handoff.md`.

`bench status` shows the rest of the state Bench holds for a worktree in at most 12 lines: its mode, the plan's approval, tasks done, gates that blocked, the verdict for `HEAD`, Clef's points, the context reading and the newest ledger events. With `--json` it is one versioned document a UI can render without logic of its own. See `docs/bench/status.md`.

### Clef (optional)

Clef answers Bench's judgement calls: seven decision points (D1 to D7), such as "is this Bash command destructive?" (the guard) or "is this question a decision for the human?" (`grill`). Each call goes to a model on Cloudflare Workers AI, its probability is checked against the point's thresholds (built in: `bench playbook show clef-thresholds`), and the decision is logged under `~/.anneal/decisions/`. Set it in the plugin's options (`userConfig`):

| Option | Meaning |
|---|---|
| `cf_account_id` | Cloudflare account that runs Clef. Empty (the default): Clef is off and every point uses its deterministic layer alone. |
| `cf_api_token` | Workers AI token, stored as a secret. It is never written to a log, an error or a decision row. |
| `clef_mode` | `off`, `shadow` (the default) or `enforce`, optionally followed by per-point overrides: `shadow,D1=off,D3=enforce`. |

**Shadow** asks Clef and logs what it would decide, but no caller acts on it. **Enforce** acts on Clef's answer, and only after the point has 50 answered decisions; until then it stays in shadow. Promote a point only when `bench calibrate` (or `/rk-bench:calibrate`) shows it meets the bar: at least 50 labelled decisions, a Brier score of 0.15 or less and an ask rate under 20%. Then set it per point (`shadow,D1=enforce`). See `docs/bench/decide.md` and `docs/bench/calibrate.md`.

## Skills and evals

Model-invoked skills (`grill`, `spec`, `plan`, `build`, `diagnose`, `verify`, `ship`) load when the request fits. User-invoked skills run only when you type them: `/rk-bench:handoff` (at a phase boundary or when the context monitor warns), `/rk-bench:orchestrate` (fan an approved plan out across subagents), `/rk-bench:correct` (turn a mistake into a check), `/rk-bench:write-skill` (write a skill test-first) and `/rk-bench:calibrate` (score the Clef points and propose thresholds). See `docs/bench/skills.md`.

Skills are tested in four tiers (`evals/bench/README.md`):

| Tier | What | Where it runs |
|---|---|---|
| 0 | Static checks: `bench lint skills`, `claude plugin validate --strict` | CI; `cargo test` runs the skill lint |
| 1 | Lexical routing: 100% rank-1 on each skill's fixtures (`bench eval routing`) | CI, through `cargo test` |
| 2 | Real routing under Claude Code's router (`bench eval routing --router claude`) | locally before a release, with `claude` signed in |
| 3 | Behaviour uplift with and without each skill (`bench eval uplift`) | locally before a release, with `claude` signed in; `cargo test` checks the suites (`--check`) |

Tiers 2 and 3 run `claude plugin eval`, so they spend money or plan usage (capped by `--max-cost-usd`, 25 US dollars by default) and run the plugin's hooks on the host: run them in a container.

## Where things are

- `crates/bench/bench/`: the `bench` binary; `crates/bench/bench-ledger/`: its ledger; `crates/bench/bench-contract/`: what other crates may name of it.
- `plugins/bench/`: the `rk-bench` plugin as installed (`skills/`, `hooks/`, `libexec/` launcher copies); the templates, schemas and tables the skills use are built into the binary (`bench playbook show <name>`, `docs/bench/playbook.md`). It ships no mod.
- `plugins/bench-cockpit/`: the `rk-bench-cockpit` plugin, the cockpit mod; its type check and tests are in `tools/bench/cockpit-check/`. See `docs/bench/mods.md`.
- `evals/bench/`: skill evaluation suites (not packaged).
- `docs/bench/`: anneal's records, imported with their series.

The repository's commands (format, lint, tests, the layer and plugin checks) are in the root `AGENTS.md`.

## Licence

Licensed under either of [Apache License, Version 2.0](../../LICENSE-APACHE) or [MIT license](../../LICENSE-MIT) at your option.
