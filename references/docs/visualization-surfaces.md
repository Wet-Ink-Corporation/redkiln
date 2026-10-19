<!-- Exported from the Claude Doc https://claude.ai/code/artifact/60bdf8c0-9f83-4592-b481-3445a9a84bee (revision 11) on 2026-10-19 as part of the project handover. The Doc is the original; this copy is frozen. Diagrams (1) could not be exported and show as [embedded content] placeholders; open the Doc to see them. -->

# Redkiln visualization surfaces: options, rubric and crate shape

Oct 14, 2026 · @Ryan

## Summary

Build two renderers over one read model: a **Claude Code mod** for the person at the keyboard, and a **Claude dashboard Artifact** for the person away from it (phone, claude.ai, cloud sessions). They score 71 and 73 out of 100 on the rubric below; a Ratatui terminal UI scores 63 and a self-hosted web UI 53.

- **Mods win the in-flow moment.** A band above the prompt and a pane, live, with zero-token commands. But they draw only in the terminal, JetBrains and the Desktop Code tab, never in cloud sessions, `claude -p`, CI or on a phone, and the API is early access.
- **Artifacts win the away moment.** They open on any signed-in surface, including a phone, and can take an answer back through their `db` capability; Weigh-In's board already works this way. But the data leaves the machine, so client repos can never use them (wi-7c21e4).
- **Ratatui is a conditional third,** for an operator watching a headless Loop over SSH. redkiln-rs already designed it; build it only if its pre-registered spike passes.
- **Self-hosted web loses for now.** It needs a server, auth and a second stack, and the problems it would solve are covered by the two winners. Revisit when Loop becomes a hosted, multi-user service.

**Crates:** one shared Rust crate for the read model, not one per UI. The mod is TypeScript and the Artifact is HTML, so neither is a crate. Only a Rust renderer (Ratatui, or later a web server) gets its own binary crate, so its dependencies never reach the plugins.

The finding that matters most: **four repos already plan four separate mods** (temper lens #107, anneal Cockpit #56, assay #123 to #132, Weigh-In #112). Installed together they would stack four bands above one prompt. They should become one suite mod.

## Who looks at what, and where

People meet Redkiln in two moments: **at the keyboard, mid-task** ("what state am I in, why was I stopped?") and **away from it** ("what happened, and what does it need from me?"). The second moment is mostly one action: answering a gate or a Weigh-In card. Every plugin already treats that answer as the one write a view may make (redkiln-rs C2/C3, anneal #56, assay ADR-23, Weigh-In's `board apply`).

| Surface | Who, when | What they need to see | What they need to do |
| --- | --- | --- | --- |
| Local Claude Code terminal (and JetBrains) | Developer mid-task; most interactive time | Gate green or red, review verdict, plan and task N of M, open decisions, context % | Approve a plan, label an outcome, decide a grounded card |
| Claude Desktop Code tab | Same person, desktop app | Same as the terminal | Same |
| Cowork | Knowledge work in a connected folder; skills only, no hooks (anneal audit) | Open decisions | Decide from the board |
| Claude Code cloud sessions (web, mobile app, Projects) | Async work, like this project | What a session did, open decisions | Decide, approve |
| CI (GitHub Actions: Review, gates) | No human in the loop; output is read later on GitHub | Check runs, PR comments, job summary | A reviewer's `/assay` waive or override |
| Headless Loop, local or hosted | Operator away; Loop runs `claude -p` sessions unattended | Run HUD, DAG, gate queue, cost and limits | **Answer a gate from a phone** (redkiln-rs Phase 3 exit criterion 1) |
| Client repositories | Any of the above, but data must stay on the machine (wi-7c21e4) | Same as their surface | Same, locally only |

Two things follow. The keyboard moment wants something **inside Claude Code**, not a second window: the landscape research in redkiln-rs says observability, not beauty, is the recurring complaint, and that people abandoned dashboards to go "back to tmux panes". The away moment wants something that **opens on a phone**, which no terminal UI can do.

## The four options, technically

All four are renderers. None of them should hold state or enforce anything: each reads a versioned JSON document from a plugin CLI and writes only by calling that CLI. Every plugin that has designed a view already landed on that rule.

### Claude dashboard Artifacts

- **What it is.** A single HTML page hosted on claude.ai, private by default, shareable. Runtime capabilities let it keep a shared `db`, know who is viewing, read connected data and ask Claude a question.
- **How data gets in and out.** A Claude session pushes state into the page's `db` with the ArtifactData tool, and reads answers back the same way. Weigh-In's board works exactly so: `board.html` (694 lines) publishes `BoardState`, people answer on the page, `board apply` validates each `Answer` as untrusted and checks `card_rev` for staleness.
- **Where it shows.** claude.ai on the web, the mobile app, the desktop app, and `/artifacts` in the terminal. It can be pinned to the sidebar.
- **Limits seen so far.** A 262,144-byte document cap (Weigh-In caps the board at 200 defaults for it). Publishing needs a Claude session, so a page is only as fresh as the last session that wrote to it. Not reachable from Cowork for Weigh-In (B-17).

### Claude Code mods

- **What it is.** A TypeScript hooks module listed under `"modules"` in a plugin's `hooks/hooks.json`. Mods launched 2026-10-01 and are early access; they need Claude Code 2.1.287 in the terminal or 2.1.286 on the desktop.
- **What it can draw.** A band above the prompt (`AbovePrompt`), panes (`$.ui.open`) with tabs, buttons with hotkeys and fields, toasts, a spinner suffix, a line under a permission dialog (`$.ui.notice`), and `Raster` (terminal) or `Svg` (desktop) pictures for charts.
- **What it can do.** Register `/commands` that run with no model turn (`command.run`), run a host command by argv (`$.process.run`, with the CLI at `$.plugin.root`), read `$.session.usage()`, observe tool calls and turns, and fill the prompt box (`$.prompt.fill`).
- **What it cannot do.** Change the permission prompt; draw in cloud sessions, the VS Code chat panel, `claude -p` or the SDK. A hook that throws is skipped, and organisations can block mods (`allowManagedModsOnly`), so mods must never enforce (all four repo designs agree).
- **Proven so far.** temper's spike: `claude plugin validate --strict` passes with a module added, on 2.1.286, 2.1.287 and 2.1.296. Whether claude.ai org sync and Cowork accept a plugin carrying a module is still unchecked (anneal #57, Weigh-In #113).

### Ratatui terminal UI

- **What it is.** A separate Rust binary (redkiln-rs's proposal calls it `redkiln-view`) running in its own terminal pane. It cannot be a `redkiln` subcommand, because "no verb is a mode" (redkiln-rs C1).
- **Cost.** `ratatui` 0.30.2, MIT, MSRV 1.88 against our 1.98.1 pin, three required crates, backends optional. 0.x, so pinned exactly.
- **What it can draw.** Tables, gauges, sparklines, bar and line charts, braille `Canvas` for the activity DAG, modals. The proposal finds nothing in its six views the terminal cannot draw.
- **What it does.** Reads the event store read-only, folds with Core's code, and writes only by `exec`-ing `redkiln answer`. No daemon, nothing to survive (C4). It works over SSH to a host running Loop.

### Self-hosted web UI

- **What it is.** A Rust server (axum, with SSE or htmx) serving pages from the read model, on localhost or on a host. redkiln-rs reserved it as Phase 3's web view; Weigh-In planned `weighin serve --wait` (#27) as a smaller local cousin for client repos.
- **What it needs.** A process that stays up, which breaks redkiln-rs's C4 ("nothing may depend on a process surviving") for local use. Hosted, it needs auth, TLS, a write endpoint for gate answers, and somewhere to run.
- **What it gives.** Any browser, a phone when hosted, full charting, multi-user views.

## Matrix: features, pros, cons, gotchas

Each option is strongest in one moment and weak in the other, which is why the answer is a pair rather than a single winner.

|  | Artifacts | Mods | Ratatui TUI | Self-hosted web |
| --- | --- | --- | --- | --- |
| **Best at** | The away moment: board, gate answers, rich run reports | The keyboard moment: live state above the prompt, zero-token actions | Watching a headless Loop from a terminal or over SSH | Team-wide, multi-user views of a hosted Loop |
| **Features** | HTML, JS and charts; shared `db`; viewer identity; pinning; sharing | Band, panes, tabs, buttons, hotkeys, toasts, `/commands`, `Raster`/`Svg`, session usage | Tables, gauges, sparklines, charts, braille DAG, keyboard navigation | Anything a browser draws; links; SSE live updates |
| **Pros** | Opens on a phone; no server; already proven by Weigh-In's board; answers are typed and validated | Inside Claude Code, no context switch; button presses are provably human (`actor: human, via: mod`); 0 tokens per action | Pure Rust, small deps, same language and gate as the rest; no daemon; any terminal | Most capable; works without a Claude account; one link for a whole team |
| **Cons** | Fresh only after a session writes to it; a separate window from the work; one page per artifact | Early access, API moves between releases; TypeScript is a second language and needs the `claude` CLI in CI | A second pane to keep open; no phone; whole app to build and maintain | A server to run, secure and keep up; auth and TLS; a front-end stack |
| **Gotchas** | Client repos may never publish (wi-7c21e4); 262 KB doc cap; not reachable from Cowork (B-17); page content returned by people is untrusted | Does not draw in cloud sessions, VS Code panel, `-p` or CI; org can block mods; four repos each plan their own band | "No verb is a mode" forces a separate binary; a live view of a store being written raises SQLite reader questions (redkiln-rs OSV-4); 0.x churn | Breaks C4 locally; a write endpoint for gate answers is an attack surface; landscape research calls local desktop apps "a proven graveyard" |
| **Prior work in our repos** | Weigh-In board (`board.html`, `weighin-core::board`) | temper #107 (held PRs #117 to #121, #123), anneal #56, assay #123 to #132 and ADR-23, Weigh-In #112 | redkiln-rs operator-surface proposal: six views, wireframes, three lints, a spike with kill criteria | redkiln-rs Phase 3 row; Weigh-In `serve --wait` (#27), unbuilt |

## Which option reaches which surface

Mods and Artifacts together cover every surface a person uses except CI, and CI already has its own surface on GitHub.

| Surface | Artifacts | Mods | Ratatui TUI | Self-hosted web |
| --- | --- | --- | --- | --- |
| Local terminal, JetBrains | Partly: `/artifacts`, opens a browser | **Yes**, in-flow | Yes, second pane | Partly: localhost, a browser |
| Desktop Code tab | Yes | **Yes** (no terminal-only elements) | No | Partly |
| Cowork | No for Weigh-In today (B-17) | Unknown, needs the spike | No | No |
| Cloud sessions (web, mobile, Projects) | **Yes** | No | No | Only if hosted |
| CI | No: CI writes check runs and PR comments instead | No | No | Only if hosted |
| Headless Loop, operator away | **Yes, phone** (if Loop's sessions can publish; unverified) | No | Over SSH only | **Yes**, if hosted |
| Client repositories | **Never** (wi-7c21e4) | **Yes**, all local | Yes | Yes on localhost |

The two gaps in the pair are worth naming. Client repos away from the keyboard get no phone view; that is the price of keeping their data local, and Weigh-In already accepted it. A hosted Loop's sessions are `claude -p` runs, and whether they can write to an Artifact is unverified; if they cannot, the Loop's phone view needs a short Claude session to sync it, or the tracker projection (GitHub) carries gates instead.

## Rubric and scores

Artifacts (73) and Mods (71) win, and they win on opposite criteria, so together they cover each other's weakest scores. Ratatui (63) is the best fit for architecture and trust but reaches too few surfaces. Self-hosted web (53) loses on cost and on fit.

Scores run from 1 (poor) to 5 (excellent). A criterion's points are its weight × score ÷ 5.

| Criterion | Weight | Artifacts | Mods | Ratatui | Web |
| --- | --- | --- | --- | --- | --- |
| Reach, weighted by where people spend time | 20 | 4 | 3 | 2 | 3 |
| Answers a gate or card where the person is, phone included | 15 | 5 | 3 | 2 | 4 |
| Glanceable in flow, no context switch, live | 15 | 2 | 5 | 3 | 2 |
| Fits the architecture: read-only, CLI is the single writer, no daemon, client data stays local | 15 | 3 | 4 | 5 | 2 |
| Build and maintenance cost (languages, deps, CI) | 10 | 4 | 3 | 4 | 1 |
| Platform maturity and risk | 10 | 3 | 2 | 4 | 3 |
| Visual richness (charts, DAG, tables) | 5 | 5 | 3 | 3 | 5 |
| Reuse of designs and code already in our repos | 10 | 4 | 5 | 3 | 2 |
| **Total out of 100** | **100** | **73** | **71** | **63** | **53** |

How the close calls were made:

- **Mods' reach is 3, not 2,** because the local terminal and the Desktop Code tab are where most interactive time is spent, even though mods miss four other surfaces.
- **Artifacts' fit is 3,** because data leaves the machine (ruled out for client repos) and freshness depends on a session writing to the page.
- **Mods' maturity is 2:** launched 2026-10-01, early access, the API "moves between releases", and an org setting can turn them off.
- **Ratatui's gate score is 2:** it answers gates at the keyboard and over SSH, never from a phone.

What would change the result: if the mods API stabilises and Cowork draws mods, Mods pass Artifacts. If Loop becomes a hosted, multi-user service, web's reach and gate scores rise and its fit stops mattering, because a hosted service is a process that is meant to stay up.

## Crate structure

One shared crate for the read model, not one crate per UI. Most renderers are not Rust at all, so a crate per UI does not even apply to them.

[embedded content: proposed layout · 1 shared crate, 4 renderers]

- **`redkiln-view`, one library crate.** It holds the typed view documents (status, gate, card, finding, run), their JSON schemas, and the badge vocabulary: every badge is an exhaustive `match` over a closed enum, so a new state fails the build instead of showing a grey "unknown" (redkiln-rs §4.1). It has no I/O, no clock and no UI dependencies, so each plugin CLI links it to emit `status --json` and text.
- **The mod is a TypeScript package** in the monorepo (for example `mods/lens/`, shipped inside a plugin's `hooks/hooks.json`). It calls CLIs through `$.process.run` and never opens a store.
- **The Artifact is an HTML asset** (Weigh-In's `board.html`, grown into the suite board). It is published by a skill, not built by Cargo.
- **Rust renderers get their own binary crates:** `redkiln-view-tui` (ratatui) and, if ever, `redkiln-view-web` (axum). Separate binaries keep `ratatui`, `crossterm` and `axum` out of every plugin's dependency graph and out of the plugin zips.

One crate with a feature per UI was considered and rejected. Feature-gated renderers inflate the feature powerset CI checks (`cargo hack`), let UI dependencies leak into libraries through feature unification, and still could not hold the TypeScript or HTML renderers.

## Sequencing and open calls

The read model comes first, because every renderer depends on it and it is useful with no UI at all (redkiln-rs option B: "the JSON is the contract").

1. **Read model.** Each plugin keeps a versioned `status --json` (temper's `temper_status: 1` exists; anneal #58, assay `assay.status/1`, Weigh-In `open --json`). Core's SPEC adds one suite envelope and the badge vocabulary, and `redkiln-view` implements it. Text output from the same documents serves every surface that draws nothing.
2. **One suite mod.** Fold temper lens, anneal Cockpit, the assay mod and the Weigh-In mod into one band and one `/redkiln` pane with a tab per installed plugin. Start after temper's pilot freeze (about 2026-10-18) and Weigh-In 1.0 (2026-10-20 at the earliest), since both freezes forbid hook behaviour changes. First, the packaging spike all four repos still owe: do claude.ai org sync and Cowork accept a plugin carrying a module?
3. **One suite board Artifact.** Grow Weigh-In's board into a Redkiln board: decisions, Loop gates and Review owner actions, all through the same untrusted-answer path.
4. **Ratatui, behind its spike.** Run redkiln-rs's S-VIEW with its kill criteria once Loop records tags and times; build `redkiln-view-tui` only if it passes.
5. **Web UI: off the roadmap** until Loop is a hosted, multi-user service.

### Calls for you

Decided 2026-10-19: all five calls as recommended (wi-d5b04b, wi-7de964, wi-ab2df0, wi-ec4bc2, wi-6e750e).

| # | Call | Recommendation | Door |
| --- | --- | --- | --- |
| A | One suite mod, or one mod per plugin? | One suite mod, shipped as its own optional plugin, reading each installed plugin's `status --json` | Two-way |
| B | One suite board Artifact, or one per plugin? | One, grown from Weigh-In's board | Two-way |
| C | Is a Ratatui view in scope for Loop? | Yes, gated on the S-VIEW spike | Two-way |
| D | Drop the self-hosted web UI until Loop is a hosted, multi-user service? | Yes | Two-way |
| E | Approve TypeScript and the `claude` CLI in the monorepo's CI for the mod? | Yes; anneal already approved it for itself (wi-08319f) | One-way (H-16: a new language in CI) |
