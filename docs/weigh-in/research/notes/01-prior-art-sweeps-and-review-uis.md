# Prior art: end-of-session sweeps of open questions/decisions, and visual out-of-terminal review surfaces for coding agents (as of 2026-09-26)

Scope note for the report writer: this builds on the 2026-09-12 "Weigh-In" brief (00_prior_weigh-in_brief_2026-09-12.html). Items that brief already covered (AskUserQuestion / request_user_input / ask_followup_question schemas, spec-kit clarify rules, ce-plan triage, BMAD [ASSUMPTION], agent-decision-log, adr-gen, approval-fatigue and HCI literature) are not repeated. Everything below is either new since 2026-09-12 or was missed by that brief. Primary sources (repo files, official docs, changelogs) were read directly unless marked "(unverified)". Repo files were read from shallow clones at HEAD on 2026-09-26 (compound-engineering HEAD 2026-09-25; superpowers v6.4.2 2026-09-25; claude-plugins-official HEAD 2026-09-25; plannotator HEAD 2026-09-26).

## Q1. Which Claude Code skills/plugins/commands do end-of-session or end-of-task sweeps of open questions, decisions, assumptions, follow-ups or handoffs — what do they output, where do they store it, how do they present it?

### Takeaway
Nothing found does exactly what Ryan wants: a sweep that extracts open questions, decisions and assumptions into a persistent per-repo ledger and presents them for decision. What exists falls into four families: handoff documents (HumanLayer, compound-engineering ce-handoff, softaworks), "learnings" sweeps that edit CLAUDE.md or a solutions folder (revise-claude-md, ce-compound, claude-wrap-up), recap/wrap-up summaries, and Anthropic's own project-artifact status page, whose "Risks & open questions", "Decisions/FAQ" and "Attention" tabs come closest to a visual ledger. The most reusable new pattern since 2026-09-12 is in compound-engineering. It records which decisions the user actually settled (with provenance), never re-asks them, and runs a judgment-bound question past a cross-model panel before it can reach the human (added 2026-09-22).

### Cited Findings
**Handoff-style sweeps (end of session, for the next agent)**
- HumanLayer `/create_handoff` writes `thoughts/shared/handoffs/[TICKET]/YYYY-MM-DD_HH-MM-SS_[TICKET]_description.md` with YAML frontmatter (date, researcher, git_commit, branch, repository, topic, tags, status, last_updated, type) and fixed sections "Task(s)", "Critical References", "Recent changes" (file:line), "Learnings", "Artifacts", "Action Items & Next Steps", "Other Notes". There is no dedicated Open Questions or Decisions section. — [humanlayer create_handoff.md](https://github.com/humanlayer/humanlayer/blob/main/.claude/commands/create_handoff.md)
- HumanLayer `/resume_handoff` reads the handoff "FULLY" plus the linked plans/research. It picks the most recent file in `thoughts/shared/handoffs/ENG-XXXX/` by timestamp, then "propose[s] a course of action to the user and confirm[s]". — [humanlayer resume_handoff.md](https://github.com/humanlayer/humanlayer/blob/main/.claude/commands/resume_handoff.md)
- The public HumanLayer repo's README now says "the code here is pretty much all deprecated - you can try the rebuild of humanlayer at https://humanlayer.com"; the last commit is 2026-06-18. The prior brief cited HumanLayer as a model, so this is new context. — [humanlayer/humanlayer README](https://github.com/humanlayer/humanlayer)
- The rebuilt HumanLayer stores artifacts in `.humanlayer/tasks/<task-slug>/` with numbered prefixes. It says "Design decisions remain open until you clearly settle them" in a design-discussion phase and "feedback on an outline or plan revises the document — it never starts implementation". It ships `/rpi:review-artifact-comments` for handling cloud comments on saved artifacts. — [HumanLayer docs: skills & workflows](https://docs.humanlayer.com/reference/skills-workflows)
- compound-engineering `ce-handoff` (present at HEAD 2026-09-25; introduction date not verified):
  - It writes one immutable Markdown snapshot to a managed private store `/tmp/compound-engineering-<uid>/ce-handoff/<repo-namespace>/<topic>.md` (umask 077, no timestamp in the path) with flat frontmatter `artifact_contract: "ce-handoff/v1"`, created_at, title, summary, keywords, cwd, and optionally resume_focus/repository/repo_root_sha/branch/head/worktree_path. It forbids "mutable lifecycle fields".
  - Its body guidance lists "Decisions, constraints, and rejected alternatives", "Unfinished work, blockers, dependencies" and "Failed approaches already abandoned".
  - It adds an attribution rule: "wherever the next agent would otherwise take a statement of intent or a decision as the user's, say whether it was the user's, your inference, or your own call."
  - It ends with one fenced `/ce-handoff resume <path>` command.
  - Resume "MUST stop and ask the user to select a candidate" and "MUST stop without acting until the user confirms". — [ce-handoff SKILL.md](https://github.com/EveryInc/compound-engineering-plugin/blob/main/skills/ce-handoff/SKILL.md); [ce-handoff references/create.md](https://github.com/EveryInc/compound-engineering-plugin/blob/main/skills/ce-handoff/references/create.md)
- softaworks `session-handoff` has ten sections, including "Decisions Made" (with rationale), "Pending Work", "Potential Gotchas" and "Handoff Chain" (links to previous/next handoffs). It stores files at `.claude/handoffs/YYYY-MM-DD-HHMMSS-[slug].md`. It ships scripts `validate_handoff.py` (flags `[TODO: ...]` placeholders and secrets) and `check_staleness.py` (time elapsed, commits, file changes, branch divergence). It triggers proactively at ">80% full" context or "after substantial work (5+ file edits, complex debugging, architecture decisions)". — [softaworks session-handoff README](https://github.com/softaworks/agent-toolkit/blob/main/skills/session-handoff/README.md)

**Learnings and wrap-up sweeps**
- Anthropic's official `claude-md-management` plugin ships `/revise-claude-md` ("Update CLAUDE.md with learnings from this session"). It reflects on missing context (commands, style, testing, quirks, gotchas), routes each item to `CLAUDE.md` or `.claude.local.md`, and asks for "one line per concept. CLAUDE.md is part of the prompt, so brevity matters." It captures no questions or decisions. — [claude-plugins-official revise-claude-md.md](https://github.com/anthropics/claude-plugins-official/blob/main/plugins/claude-md-management/commands/revise-claude-md.md)
- compound-engineering `ce-compound` writes one qualifying "durable repo learning" under `<root>/solutions/` and writes nothing when no learning qualifies. On 2026-09-24 (PR #1772) learnings gained an optional `retire_when` field: "The change outside the repo that would retire this learning, and how a refresh can check for it without changing the repo". `ce-compound-refresh` then checks whether each learning still applies. — [ce-compound SKILL.md](https://github.com/EveryInc/compound-engineering-plugin/blob/main/skills/ce-compound/SKILL.md); [PR #1772 retirement condition](https://github.com/EveryInc/compound-engineering-plugin/pull/1772)
- stephanbuettig/claude-wrap-up `/wrap-up`:
  - It classifies session signals (Corrections, Repetitions, Preferences, Successes, Insights) as HIGH ("Explicit correction or ≥2 occurrences"), MEDIUM or LOW.
  - It routes them to CLAUDE.md (≤200 lines), `.claude/rules/[domain].md`, CLAUDE.local.md or `.claude/sessions/YYYY-MM-DD-[topic].md`, with a user confirmation gate before writing.
  - It uses a Stop hook and renders its output in the terminal only.
  - 1 star, v1.0.1 (2026-03-20). — [claude-wrap-up](https://github.com/stephanbuettig/claude-wrap-up)
- RyanPenn-coder/wrap-skill "Checks the claims against the files" (verifies work landed), saves progress, decisions and reasoning to the project's existing records ("never rewrites earlier entries"), "Turns follow-ups into tasks", and emits a first message for the next session. It installs on claude.ai/desktop/Cowork by skill upload and in Claude Code via `~/.claude/skills/`. 0 stars. — [wrap-skill](https://github.com/RyanPenn-coder/wrap-skill)
- tothomas69/wrapup writes one Markdown file per session (<500 words) to `docs/wrapup/`. It includes "Problems worked through, including what was rejected and why", next steps and the verbatim final exchange. It is "fully automatic" and "never invents", and has no open-questions section. — [wrapup](https://github.com/tothomas69/wrapup)
- A "decision-log" skill was proposed on 2026-09-08 (issue only, not built): user-invoked "log our decisions", compiles session decisions into MADR files `decisions/NNNN-kebab-title.md`, validated against git history, with a "don't fabricate" rule: "an invented alternative misrepresents the actual record". — [ismailkhan313/skills issue #3](https://github.com/ismailkhan313/skills/issues/3)

**Anthropic official plugins and skills that overlap (claude-plugins-official / anthropics/skills)**
- `project-artifact` publishes an "opinionated, tabbed status page" through the built-in Artifact tool to a private claude.ai page.
  - Tabs appear only when they have content: Overview, Workstreams, **Attention (Waiting on)**, Background, Plan, **Risks & open questions** ("Risk register ... plus the unresolved questions the project hasn't answered yet") and **Decisions / FAQ** ("the decisions made + rationale").
  - Attention items are "numbered, priority order, each item the exact action (a paste-ready message or a one-word decision) plus one sentence on what it unblocks".
  - A per-project config at `${CLAUDE_PLUGIN_DATA}/artifacts/<slug>/config.md` makes "refresh the artifact" redeploy the same URL and "report only the delta". It embeds a state block so the next run can compute changes.
  - It needs a claude.ai login. — [project-artifact SKILL.md](https://github.com/anthropics/claude-plugins-official/blob/main/plugins/project-artifact/skills/project-artifact/SKILL.md)
- `playground` builds single-file HTML explorers. Its `document-critique` template shows suggestions with Approve / Reject / Comment and a bottom "Prompt output (approved + commented items)" with a **Copy Prompt** button. The return path to the agent is copy-paste. — [playground document-critique template](https://github.com/anthropics/claude-plugins-official/blob/main/plugins/playground/skills/playground/templates/document-critique.md); [playground README](https://github.com/anthropics/claude-plugins-official/blob/main/plugins/playground/README.md)
- `session-report` and `receipts` mine `~/.claude/projects` transcripts for usage and impact reports as self-contained HTML. They do not extract questions or decisions, but they are official examples of the "template HTML + JSON blob + 3–5 agent-written findings" pattern. — [session-report SKILL.md](https://github.com/anthropics/claude-plugins-official/blob/main/plugins/session-report/skills/session-report/SKILL.md); [receipts README](https://github.com/anthropics/claude-plugins-official/blob/main/plugins/receipts/README.md)
- anthropics/skills `discernment-nudge` (last changed 2026-09-24) fires after substantive advice or drafts. It appends "2-3 short follow-up questions, each tied to something specific in what you just produced, that help the user check key facts, probe the reasoning or assumptions", "at most once per conversation". — [discernment-nudge SKILL.md](https://github.com/anthropics/skills/blob/main/skills/discernment-nudge/SKILL.md)

**Built-in Claude Code features that overlap**
- The Claude Code docs for Projects suggest a manual assumptions sweep prompt: "For every open thread, list what you asked it to do, what it assumed or couldn't reach, and what it's waiting on." — [Claude Code docs: Projects](https://code.claude.com/docs/en/claude-projects)
- Claude Code `/recap` and the away summary (added v2.1.108, 2026-04-14): the format is "Under 40 words, 1-2 plain sentences, no markdown. Lead with the overall goal and current task, then the one next action". It is toggled by `awaySummaryEnabled`, and there is a request (2026) to bring it to the desktop app. Recap text was capped at 400 characters in v2.1.236 (2026-08-19) after "runaway session recaps". — [claude-code issue #91573](https://github.com/anthropics/claude-code/issues/91573); [Claude Code changelog](https://code.claude.com/docs/en/changelog)

**compound-engineering patterns relevant to "don't paper over or defer what can be settled now"**
- **Session-settled decisions** protocol (present at HEAD; byte-duplicated in ce-plan and ce-brainstorm, parity-tested):
  - It classifies conversation decisions as **Settled** ("a tradeoff, alternative, or risk was raised ... and the user chose with it in view"), **Directive** ("the user asserted a choice no one examined ... receives exactly one in-pipeline challenge") or **Unlabeled** (agent-inferred).
  - It has two provenance classes, `user-directed` and `user-approved`, and the rule "**No self-settling.** An agent never labels its own unexamined proposal, and never upgrades bare assent to `user-directed`."
  - Annotation: `(session-settled: user-directed — chosen over <alternative>: <one-line reason>)`.
  - "Never re-ask a settled decision." Research may contradict a settled decision "only on evidence".
  - "No sidecar files, no frontmatter registry, no numeric weights, no lifecycle field", with worst-case degradation described as "today's re-opening of the decision". — [ce-plan references/settled-decisions.md](https://github.com/EveryInc/compound-engineering-plugin/blob/main/skills/ce-plan/references/settled-decisions.md)
- **Adjudicate before escalating** (2026-09-22, PR #1765, in ce-resolve-pr-feedback):
  - Split: "**Authority-bound** — the decision needs authority or knowledge this run does not hold: it touches security, auth, billing, data retention, a migration, or another system; it is a product or business call; ... or only the user knows the intent. This is `needs-human`." versus "**Judgment-bound** — competent engineers could reasonably disagree, and every fact the decision turns on is in the repository, the PR, and the review thread".
  - Judgment-bound items go to `ce-pov` with a cross-model "oracle" panel first. Only a blocked result, a verdict with no lean, or an out-of-envelope action becomes `needs-human`, "richer than the one you started with".
  - "'this might be intentional' is not evidence, and uncertainty resolves to fixing (or a brief `replied` question), not escalating."
  - An autonomous call is announced "so the author can reverse it after the fact". — [PR #1765](https://github.com/EveryInc/compound-engineering-plugin/pull/1765); [evaluation-rubric.md](https://github.com/EveryInc/compound-engineering-plugin/blob/main/skills/ce-resolve-pr-feedback/references/evaluation-rubric.md)
- "fix(ce-plan): run a Bake-off unasked when a costly choice stays open" (2026-09-14, PR #1717). By the title, the planner now builds competing options itself instead of leaving a costly open question; diff body not read. — [PR #1717](https://github.com/EveryInc/compound-engineering-plugin/pull/1717)
- `ce-sweep` is a feedback sweep (Slack/GitHub Issues/email), not a session sweep, but its mechanics transfer:
  - `scripts/sweep-state.py` is "the **only** writer of sweep state", with a lease and cursor.
  - Items that can't be actioned become `ack_deferred`.
  - A decision round "2h" is "Interactive only. For items needing a product call, ask the user, grouped by category with one blocking question per category"; in non-interactive runs "the deferrals are already in the plan's Outstanding Questions".
  - "Fail safe. With no usable blocking-question tool, behave as non-interactive". — [ce-sweep SKILL.md](https://github.com/EveryInc/compound-engineering-plugin/blob/main/skills/ce-sweep/SKILL.md); [ce-sweep references/run.md](https://github.com/EveryInc/compound-engineering-plugin/blob/main/skills/ce-sweep/references/run.md)
- ce-plan supports `output:html` (exclusive: markdown OR HTML, never both). HTML invariants include "Single self-contained HTML5 file", "All metadata appears as visible text — single source of truth", "No hidden machine-readable copy in any form", and "Stable IDs as anchor IDs AND visible text". Downstream agents (`ce-doc-review`, `ce-work`) read the HTML directly. — [html-rendering.md](https://github.com/EveryInc/compound-engineering-plugin/blob/main/skills/ce-plan/references/html-rendering.md); [output-mode.md](https://github.com/EveryInc/compound-engineering-plugin/blob/main/skills/ce-plan/references/output-mode.md)
- obra/superpowers `finishing-a-development-branch` is the end-of-task step: verify tests → detect environment → "present exactly these 3 options" (merge locally / push and PR / keep as-is). It does not sweep questions or decisions. — [superpowers finishing-a-development-branch](https://github.com/obra/superpowers/blob/main/skills/finishing-a-development-branch/SKILL.md)

### Inferences
- The ledger slot is empty. Handoffs record decisions as prose for the next agent, and learnings sweeps record rules for CLAUDE.md. None keeps items with status across sessions or asks the human to decide them. project-artifact is the closest official shape: tabs for open questions and decisions, a delta-only refresh, and per-project config in plugin data.
- For Ryan's "don't paper over, don't defer what can be settled" requirement, three recent primitives can be adopted almost verbatim:
  - CE's settled / directive / unlabeled taxonomy with the no-self-settling rule.
  - CE's authority-bound vs judgment-bound split, where only authority-bound items reach the human by default.
  - ce-handoff's attribution rule: user's decision vs agent inference vs agent's own call.
- CE's `retire_when` and softaworks' `check_staleness.py` are two cheap expiry mechanisms: a named retirement condition, and staleness measured by git drift.
- Most sweep skills are tiny, single-author projects (0–1 stars). The widely installed ones (compound-engineering, superpowers, official plugins) put handoff and learning capture inside larger workflows rather than shipping a standalone "open questions" skill. That weakly suggests the standalone form has not found traction.

### Gaps
- Not read: `rvdbreemen/adr-kit`, `arthurglaizal/session-recap`, `madappgang` adr-documentation, and the mcpmarket "session wrap-up" listings (seen only in search results).
- Introduction dates for ce-handoff, ce-sweep and settled-decisions could not be pinned (they predate the 2026-09-10 shallow history); only the 2026-09-14/22/24 changes are dated.
- No install or usage numbers for any end-of-session sweep skill were found.

## Q2. Which tools give coding agents a visual review/approval surface outside the terminal, and how does the human's answer get back to the agent?

### Takeaway
Return channels fall into six families:
1. **Blocking hook plus local server** (Plannotator).
2. **Background local server that the agent blocks on or polls** (ce-prototype, superpowers visual companion, Anthropic's fakechat channel).
3. **Hosted document with comments the agent reads through an API or watch** (Claude Code artifacts, Proof, Antigravity, the HumanLayer rebuild).
4. **Batched diff comments sent as one chat message** (Claude Desktop, Vibe Kanban, Conductor, Antigravity CLI).
5. **Copy-as-prompt** (playground, the artifacts docs' own recommended pattern).
6. **Protocol-level mechanisms**: MCP URL-mode elicitation (Claude Code 2.1.281, 2026-09-23) and channels (research preview).

Only families 3 and 5 work unchanged in Claude desktop/Cowork non-code sessions as well as Claude Code.

### Cited Findings
**Plannotator (backnotprop)**
- Described as "a local, browser-based review surface for AI coding agents", with 8.9k stars and 677 forks when fetched.
- Plugin hooks (`apps/hook/hooks/hooks.json`):
  - `PermissionRequest` matcher `ExitPlanMode` runs command `plannotator` with `"timeout": 345600` (96 hours).
  - `PreToolUse` matcher `EnterPlanMode` runs `plannotator improve-context` (5 s).
- Flow: "Agent calls ExitPlanMode -> PermissionRequest hook fires -> Local server reads plan from hook input -> Browser opens with review UI -> You annotate and approve/deny".
- Approve returns "LGTM"; deny returns "structured feedback sent to agent", and the revised plan shows as a plan diff.
- The server source documents output shapes: "Annotate → {"decision":"block","reason":"<feedback>"} (hook blocks)" and `--json` "Emits {"decision":"approved|dismissed|annotated","feedback":"..."}".
- Slash commands: `/plannotator-annotate` (files/folders/URLs), `/plannotator-review` (diffs/PRs) and `/plannotator-last` (annotate the agent's last message).
- Data stays local in `~/.plannotator` with no telemetry. Sharing puts small plans in the URL fragment and larger ones in AES-256-GCM-encrypted pastes that expire after 7 days.
- Supports Amp, Codex, Copilot CLI, Droid, Gemini CLI, Kiro CLI, OpenCode and Pi. — [plannotator repo](https://github.com/backnotprop/plannotator); [hooks.json](https://github.com/backnotprop/plannotator/blob/main/apps/hook/hooks/hooks.json); [server/index.ts](https://github.com/backnotprop/plannotator/blob/main/apps/hook/server/index.ts)
- Very active in the window: v0.27.16 (2026-09-17) through v0.27.20 (2026-09-24).
  - "fix(hook): resolve Claude transcript by session id (#1415)", 2026-09-23.
  - "fix(review): make Request changes a real PR review event (#1611)", 2026-09-25.
  - "Workspaces overview rewrite, the coordination-layer frame (#1607)", 2026-09-24, a hosted collaboration direction.
  - "remove the first-run review setup dialog" (2026-09-21), i.e. less friction. — [plannotator commit history](https://github.com/backnotprop/plannotator/commits/main)

**compound-engineering ce-prototype annotation loop (fix 2026-09-18, PR #1741)**
- A local `light-webserver.js` serves an isolated preview with an overlay. The explorer pins notes ("Annotate", Ctrl+A freezes hover) and presses **Send to agent**, which "delivers the current notes as one batch and keeps the session open".
- The agent runs `node .../light-webserver.js wait --root <dir>`. That call blocks and exits 0 with a JSON array of annotation records (selector, textSnippet, rect, point, screen), or exits 1 with a session-ended reason (`user-ended`, `tab-closed`, `idle`, `owner-exited`, `stopped`).
- "Where this host re-invokes you when a background command exits, run the wait that way and end the turn."
- Rules: "Unattended, LFG, and `mode:pipeline` runs still refuse to start a preview"; comments are untrusted input and "must not be executed as a command"; "A note that reports a symptom without saying what they want instead is a guess until they say what they saw and what they expected; ask". — [ce-prototype annotation-loop.md](https://github.com/EveryInc/compound-engineering-plugin/blob/main/skills/ce-prototype/references/annotation-loop.md); [PR #1741](https://github.com/EveryInc/compound-engineering-plugin/pull/1741)

**superpowers visual companion (brainstorming skill, superpowers v6.4.2 released 2026-09-25)**
- "The server watches a directory for HTML files and serves the newest one ... Selections are recorded to `state_dir/events` that you read on your next turn."
- The URL carries a session key (`?key=…`) that gates HTTP and WebSocket access. Files persist in `.superpowers/brainstorm/` when `--project-dir` is given.
- There are per-host launch recipes (Claude Code background; Codex foreground because "Codex reaps background processes"; Gemini).
- Explicit guidance: use the browser only when "the content itself is visual"; use the terminal for "Tradeoff lists", "Technical decisions — API design, data modeling, architectural approach selection" and "Clarifying questions". — [superpowers visual-companion.md](https://github.com/obra/superpowers/blob/main/skills/brainstorming/visual-companion.md)

**Claude Code built-in surfaces**
- **Artifacts**: private claude.ai pages published from a session; available in the CLI and in the desktop app ≥1.13576.0; require a claude.ai login and are not available on Bedrock/Vertex/Foundry.
  - Return paths:
    - (a) "Bring the result back to your session": the docs recommend "a 'Copy as prompt' button that gives me the final ordering to paste back here".
    - (b) Comments: only on artifacts shared within an organization (Team/Enterprise). An editor presses **Send to Claude** or mentions `@claude`. The session "watches that artifact for comments for as long as the session runs" (v2.1.228+) and auto-replies or auto-edits depending on permission mode. It pauses in plan mode and caps at "60 sent comments or thread activations ... within an hour".
    - (c) Connector calls from the page (v2.1.209+) run under the viewer's account.
  - Pro/Max accounts can only share publicly, and "viewers can't comment" on public artifacts. — [Claude Code docs: artifacts](https://code.claude.com/docs/en/artifacts)
- Artifact **database** reads and writes exist and changed during the window: 2.1.269 (2026-09-11) "artifact database reads that save into the session scratchpad no longer stop for working-folder approval"; 2.1.273 (2026-09-15) "an update can now remove a single field"; 2.1.283 (2026-09-25) paged ordered queries. — [Claude Code changelog](https://code.claude.com/docs/en/changelog)
- **MCP URL-mode elicitation**: 2.1.281 (2026-09-23) "Added MCP URL-mode elicitation on 2026-07-28 protocol connections, so servers can ask Claude Code to open a browser-based flow; no waiting dialog is left on screen when the server has no way to confirm completion". This postdates the 2026-09-12 brief, which treated URL mode as auth-only. — [Claude Code changelog](https://code.claude.com/docs/en/changelog)
- **Channels** (research preview): "an MCP server that pushes events into your running Claude Code session"; two-way via a reply tool.
  - The demo `fakechat` "runs a chat UI on localhost" (http://localhost:8787); typed messages arrive as `<channel source="plugin:fakechat:fakechat">` events.
  - Custom channels need `--dangerously-load-development-channels` during the preview. Pro/Max users "skip these checks"; Team/Enterprise need an admin to enable channels.
  - Under `-p`, "multiple-choice questions and plan mode approval, are disabled". — [Claude Code docs: channels](https://code.claude.com/docs/en/channels)
- **Desktop diff review**: "click any line in the diff to open a comment box ... submit all comments at once (Cmd+Enter)"; "Claude reads your comments and makes the requested changes". Side chats (`/btw`) ask questions without adding to the main thread. — [Claude Code docs: desktop](https://code.claude.com/docs/en/desktop)
- Hook changes in the window:
  - 2.1.280 (2026-09-22): "Changed `PermissionRequest` hooks: an agent-type hook no longer runs there ... now shows an error pointing to command or http hooks". This matters for Plannotator-style gates, which must be command or http hooks.
  - 2.1.274 (2026-09-17): "Fixed Stop prompt hooks re-sending their whole prompt on every block ... repeat blocks now name the condition with a 500-character label" (a token cost fix).
  - 2.1.274: AskUserQuestion preview-note fixes.
  - 2.1.281: "[VSCode] Fixed Claude attaching option previews to multiple-choice questions in the chat panel, where the question card never shows them". — [Claude Code changelog](https://code.claude.com/docs/en/changelog)

**Other agents**
- **Google Antigravity (IDE)**: review policies "Request Review (Recommended)" (the agent "always halts and requests your explicit approval") vs "Always Proceed". Users can "add inline comments" on plan artifacts. — [Antigravity docs: artifact review](https://antigravity.google/docs/artifact-review/). Comments are Google-Docs-style on the Implementation Plan, Task List, diffs and Walkthrough (screenshots, recordings), and "make sure you remember to submit the comment. This is what triggers Antigravity to update its plans" (Dec 2025 post). — [Mete Atamel, 2025-12-10](https://atamel.dev/posts/2025/12-10_antigravity_provide_feedback/)
- **Antigravity CLI**: Ctrl+R opens an Artifact Picker; `c` comments on a line; `y`/`n` approve or reject a file; `Shift+A` / `Shift+R` bulk approve or reject; Esc "submit[s] approvals/rejections back to the agent thread". — [Antigravity docs: CLI artifacts](https://antigravity.google/docs/cli/artifacts/)
- **Cursor**: Plan Mode asks clarifying questions, researches, then writes a plan. "Plans are saved by default in your home directory. Click 'Save to workspace'"; the plan is edited "through chat or markdown files". The question UI is not documented on the page. — [Cursor docs: plan mode](https://cursor.com/docs/agent/plan-mode)
- **Cursor Projects** (2026-09-10): a coordinator agent "plans the work, delegates it to agents ... and brings the finished work back to you to check"; shared context files sync across machines. No question-inbox mechanism is documented. — [Cursor changelog: Projects](https://cursor.com/changelog/projects)
- **OpenAI Codex**:
  - v0.156.0 (2026-09-22): "Restore Plan mode when resuming sessions"; "Side chats now carry over attachments, selected text, and review comments"; "Delay automatic recaps and compact their TUI layout"; "Preserve conversation context and separate next actions in recaps". — [Codex changelog](https://learn.chatgpt.com/docs/changelog)
  - Desktop app users asked (2026-03-12) to edit plans directly instead of regenerating them; the issue is closed. — [openai/codex #14463](https://github.com/openai/codex/issues/14463)
- **Vibe Kanban**: tasks move to **In Review** when the agent finishes. Line comments via "+" are "not submitted individually. They are collected and sent as a complete review", "combined into a single message for the coding agent". The task then returns to In Progress. — [Vibe Kanban docs](https://vibekanban.com/docs/core-features/reviewing-code-changes)
- **Conductor**:
  - v0.87.0 (2026-09-18) added a "Workspace status matrix ... distinct states for routine runs, attention, errors, running pull request checks"; v0.86.0 (2026-09-16) fixed "review comments appearing unsent after submitting a message". — [Conductor changelog](https://www.conductor.build/changelog)
  - Claude's own line comments come from an in-process MCP server (`createSdkMcpServer`); humans "add these comments to the chat" to send them back (2026-01-07). — [Conductor blog: diff tools](https://www.conductor.build/blog/diff-tools)
- **Proof (Every)** is a hosted collaborative markdown editor that CE uses for review handoffs. The agent reads `comments[]` and `suggestions[]` from `GET /api/agent/<slug>/v3/document` as "the review state" and replies, resolves, accepts or rejects by id via `v3/edit`. — [ce-proof SKILL.md](https://github.com/EveryInc/compound-engineering-plugin/blob/main/skills/ce-proof/SKILL.md)
- **Kiro**: requirements → design → tasks, with a "Confirm when requirements meet your needs" step per phase. To change requirements, edit `requirements.md` and ask Kiro to update the design. — [Kiro docs: requirements-first](https://kiro.dev/docs/specs/feature-specs/requirements-first/). A 2025 issue (older than the window) reports Kiro ignoring manual edits to `requirements.md` and generating designs with deleted requirements; closed as not planned. — [kirodotdev/Kiro #2587](https://github.com/kirodotdev/Kiro/issues/2587)

### Inferences
- For a skill that must run in both Claude Code and Cowork, the only return channels that need no local process are:
  - (1) in-chat structured questions;
  - (2) an artifact with a "Copy as prompt" export, which works on every plan;
  - (3) artifact comments via Send to Claude, which require Team/Enterprise and org sharing;
  - (4) the artifact database, which the page writes and Claude reads back. Its existence is confirmed by the changelog; its public docs were not found.

  Local-server patterns (Plannotator, ce-prototype, superpowers, fakechat) are proven in Claude Code but cannot reach a Cowork cloud session.
- The ce-prototype pattern (a background `wait` that exits with a JSON batch, plus the host re-invoking on background exit) is the most token-frugal local loop found. The agent spends no tokens while the human reviews, and gets one batch per "Send".
- Batching review comments into one submission is universal: Vibe Kanban, Claude Desktop Cmd+Enter, Antigravity "submit", Plannotator deny/annotate. No tool sends per-item round trips.
- superpowers' guidance that technical decisions and trade-off lists belong in the terminal is a real counterpoint. It supports rendering the ledger visually but deciding simple two-to-four-option items with native in-chat questions, and reserving the page for items with evidence or comparisons.

### Gaps
- Not researched for lack of budget: Imbue Sculptor, Omnara, Crystal/Nimbalyst, Factory Droids, Amp, Jules, and the GitHub Copilot coding agent's review UI.
- Antigravity's "Agent Manager inbox" and any "Agent Decides" policy are unverified; the docs page fetched showed only Request Review / Always Proceed.
- Cursor's clarifying-question UI format and how answers flow into the plan are not documented on the pages fetched.
- Public documentation of the artifact runtime "db" capability (as opposed to changelog mentions) was not found.
- Whether Ryan's claude.ai account is Team/Enterprise, which comment-based return needs, is unknown.

## Q3. Are there "decision inbox" / "needs your input" queues for agents, and how do they rank, batch and expire items?

### Takeaway
Every orchestrator now has a "waiting on you" lane: Claude Projects Overview, Devin Agent Command Center, Linear `awaitingInput`, Conductor's status matrix, Vibe Kanban's In Review, and `claude agents` "Needs your input". These queues are all per session or thread, with state-based grouping, not ranking. None ranks individual questions by impact, and none keeps questions as items separate from the session that raised them. Expiry exists only coarsely: Claude Projects auto-resolves threads after a week idle, and Linear marks sessions `stale`.

### Cited Findings
- **Claude Projects** (public beta, Pro/Max; Overview pane):
  - Threads are grouped into "Ready for review", "**Waiting on you** — Threads that need your reply or approval, or that failed", "Working", "Landing", "Idle" and "Resolved". Resolved means marked done "by you ..., by Claude once you've taken the last step ..., or **automatically after a week with no activity**".
  - A dot appears on the Overview button, and desktop notifications fire "when Claude posts in the conversation, a thread hits an error, or a thread needs your input".
  - Approvals must be answered inside the thread: "Telling Claude in the project conversation to go ahead doesn't reach it."
  - Project memory holds "requirements, decisions, and pitfalls" as files, with a `MEMORY.md` index that every thread reads. — [Claude Code docs: Projects](https://code.claude.com/docs/en/claude-projects)
- The `claude agents` session list gained statuses in 2.1.196 (2026-06-29): "completed rows no longer flip between 'Done' and 'Needs your input', stalled agents are now labeled 'Needs attention'". 2.1.269 (2026-09-11) mentions a contextual "what Claude needs" turn-end notification text in Claude Desktop sessions. — [Claude Code changelog](https://code.claude.com/docs/en/changelog)
- **Devin Desktop Agent Command Center**: "Agents are organized into columns by status so you can quickly tell what is in flight, what needs your attention, and what is finished". "a single native OS notification when an agent session finishes or needs your input" is "off by default" (`devin.agentNotifications`). "Add to chat" routes selected transcript text back. — [Devin docs: Agent Command Center](https://docs.devin.ai/desktop/agent-command-center)
- **Linear Agent Interaction SDK**:
  - Session states `pending`, `active`, `error`, `awaitingInput`, `complete`, `stale`.
  - Activity types thought / **elicitation** ("Requests clarification or confirmation from the user") / action / response / error.
  - A user reply arrives as a webhook with action `prompted`, and the agent must "insert that message into the conversation history".
  - Agents must respond within 10 s to `created` (otherwise marked unresponsive); webhooks must return within 5 s.
  - An Agent Plan API (preview) carries checklist items with pending/inProgress/completed/canceled. — [Linear developers: agent interaction](https://linear.app/developers/agent-interaction)
- **GitHub Agent HQ mission control** (Dec 2025, older than the window) advises steering through session logs and "grouping similar reviews together ... to reduce context-switching". — [GitHub blog, 2025-12-01](https://github.blog/ai-and-ml/github-copilot/how-to-orchestrate-agents-using-mission-control/)
- **Conductor** 0.87.0 (2026-09-18): the "Workspace status matrix" shows distinct states including "attention". — [Conductor changelog](https://www.conductor.build/changelog)
- **Vibe Kanban**: an "In Review" column is the inbox. — [Vibe Kanban docs](https://vibekanban.com/docs/core-features/reviewing-code-changes)
- A Claude Code feature request (2026-07-19) asks for a persistent **sidebar of open/unresolved questions** with "Auto-dismiss on resolution" and "Click-to-jump navigation". The motivation is "questions Claude asks — especially ones that need a decision before Claude can proceed — can get buried in the scrollback and missed"; it should work "whether the question is asked via plain text or via a structured question/decision tool". — [anthropics/claude-code #79078](https://github.com/anthropics/claude-code/issues/79078)

### Inferences
- The industry unit of attention is the thread or session, not the question. Ryan's per-item ledger (per session and per repo) sits below what any inbox offers, which is where issue #79078 points too.
- The expiry defaults observed are one week idle (Claude Projects) and `stale` (Linear). None expires individual questions; ce-compound's `retire_when` (Q1) is the only item-level retirement condition found.
- Ranking is state-based everywhere: needs-you / working / done. Impact × uncertainty ranking (spec-kit, from the prior brief) remains unique to spec-time clarification.

### Gaps
- No public details on how Jules, Factory, Amp or Cursor background agents rank or batch pending questions.
- The Claude Projects docs do not say whether "Waiting on you" items are ordered within the group.

## Q4. What was released or announced between 2026-09-12 and 2026-09-26 that is relevant?

### Takeaway
There were no new first-party "decision ledger" features in the window. The relevant changes are enabling plumbing:
- Claude Code: URL-mode elicitation (Sep 23), skill/plugin sync from claude.ai to the terminal (Sep 17), PermissionRequest hooks limited to command/http (Sep 22), Stop-prompt-hook token fix (Sep 17), artifact database changes (Sep 15 and 25).
- compound-engineering: adjudicate-before-escalate (Sep 22), bake-off-unasked (Sep 14), learning retirement conditions (Sep 24).
- Plannotator: weekly releases.
- Codex: recap changes and the user backlash they drew (Sep 22).
- Cursor Projects arrived on Sep 10, just before the window.

### Cited Findings
- Claude Code releases 2.1.275 (Sep 17) through 2.1.283 (Sep 25). — [Claude Code changelog](https://code.claude.com/docs/en/changelog)
  - 2.1.275 (2026-09-17): "Added syncing of the skills and plugins enabled on your claude.ai account to terminal sessions signed in with it; opt out with `syncClaudeAiSkills: false` or `syncClaudeAiPlugins: false`." A skill enabled once on claude.ai (Cowork) now also appears in Claude Code.
  - 2.1.281 (2026-09-23): MCP URL-mode elicitation to open a browser-based flow.
  - 2.1.280 (2026-09-22): agent-type hooks no longer run on `PermissionRequest`; "[VSCode] Added a typed `/plan`"; the plan approval card offers "Yes, and use auto mode".
  - 2.1.274 (2026-09-17): Stop prompt hook repeat-block token fix; AskUserQuestion preview fixes; Monitor tool notifications merged "saving a model turn".
  - 2.1.275: "Fixed a resumed cloud session leaving an unanswered question open in the transcript after a queued message superseded it".
- Official plugins: no dated change for Q&A features in the window. `discernment-nudge` in anthropics/skills last changed 2026-09-24. — [anthropics/skills discernment-nudge](https://github.com/anthropics/skills/blob/main/skills/discernment-nudge/SKILL.md)
- compound-engineering: PR #1717 (2026-09-14) Bake-off unasked; PR #1765 (2026-09-22) adjudicate judgment calls before escalating; PR #1772 (2026-09-24) `retire_when`; PR #1741 (2026-09-18) annotation sessions kept alive. — [compound-engineering commits](https://github.com/EveryInc/compound-engineering-plugin/commits/main)
- Plannotator v0.27.16–v0.27.20 (2026-09-17 → 2026-09-24): transcript-by-session-id hook fix, first-class Mistral Vibe support, Workspaces "coordination-layer" framing. — [plannotator commits](https://github.com/backnotprop/plannotator/commits/main)
- superpowers v6.4.2 (2026-09-25), "leaner plans from writing-plans". — [obra/superpowers](https://github.com/obra/superpowers)
- Codex v0.156.0 (2026-09-22): recap and plan-mode resume changes (see Q2), plus a user "Feature un-request: Claude-style recap" filed 2026-09-22 (see Q5). — [Codex changelog](https://learn.chatgpt.com/docs/changelog); [openai/codex #47306](https://github.com/openai/codex/issues/47306)
- Conductor v0.86.0 (2026-09-16) and v0.87.0 (2026-09-18): review-comment and attention-state changes. — [Conductor changelog](https://www.conductor.build/changelog)
- Cursor 2026-09-23 "Security Review" posts "one review comment reporting exploitable bugs"; Cursor Projects landed 2026-09-10. — [Cursor changelog](https://cursor.com/changelog)
- Decision-log skill idea filed 2026-09-08, just before the window. — [ismailkhan313/skills #3](https://github.com/ismailkhan313/skills/issues/3)

### Inferences
- The claude.ai → terminal skill sync (2.1.275) makes the "one skill, both surfaces" requirement easier to meet. The skill can be authored once, enabled on claude.ai for Cowork, and it then appears in Claude Code. The risk is that runtime capabilities differ (local files and git exist in Claude Code; Cowork cloud sessions differ).
- URL-mode elicitation (2.1.281) gives the prior brief's option C (registry MCP server that opens a local review page) a first-party launch path. It still needs a server process, so it stays Claude Code-only.

### Gaps
- The Antigravity, Kiro, Devin, Jules and GitHub Copilot changelogs for 2026-09-12..26 were not checked.
- No Anthropic announcement of a first-party decision or question register was found; absence is not proof.

## Q5. What do users say makes these question/review UIs get used versus ignored? Any abandoned attempts and why?

### Takeaway
The freshest and most direct evidence is the September 2026 backlash against Codex's automatic recaps. Users rejected recaps that fire unasked, spend hidden tokens, and misreport status: "pending approval" was turned into "verified complete" because the recap saw only a truncated 900-byte window. A manual `/recap` was fine. On the positive side, users ask for open questions pinned out of the scrollback (#79078) and for open questions to block "start" (#64178). Plannotator's durable adoption (8.9k stars, weekly releases) comes from hooking the one moment users already stop at, plan approval, rather than adding a new ritual. One notable abandonment: the open-source HumanLayer/CodeLayer code is now declared deprecated.

### Cited Findings
- "Feature un-request: Claude-style recap" (2026-09-22): "I would like it to not happen automatically. A `/recap` command does not bother me because I can just not do that. Automated recap does bother me." No setting to disable it was found at the time. — [openai/codex #47306](https://github.com/openai/codex/issues/47306)
- Codex recap accuracy bug (reported for v0.151.0 and reproduced on v0.153.4):
  - The recap claimed an approval-gated test was "verified and complete" when the agent had said it was "still pending" and asked for authorization: "A recap must not convert 'pending approval / still needs verification' into 'verified complete.'"
  - Root cause: the recap prompt gets "a truncated recent-conversation window (900-byte limit) without explicit markers indicating truncation or unresolved states".
  - A second reporter saw the reverse, verified work downgraded to pending by UTF-8 truncation.
  - Another user called recaps "misleading" and "unrelated nonsense". — [openai/codex #41626](https://github.com/openai/codex/issues/41626)
- The cost and consent objection: automatic recaps make real API calls "not represented by the visible thread counter"; "Codex should not silently add model calls merely because the terminal lost focus"; the requester asked for opt-in with the manual `/recap` kept. The issue was consolidated into #41622 rather than rejected. — [openai/codex #41896](https://github.com/openai/codex/issues/41896); [openai/codex #41622](https://github.com/openai/codex/issues/41622)
- Claude Code's own recap needed guardrails: capped at 400 characters after "occasional runaway session recaps" (2.1.236, 2026-08-19); "Fixed session recap auto-firing while composing unsent text" (2.1.113, 2026-04-17). A power user running 12+ concurrent sessions wants recap in desktop because "Every switch costs the same tax: scroll up, find the last few tool calls, work out what the session was doing and what it was about to do next." — [Claude Code changelog](https://code.claude.com/docs/en/changelog); [anthropics/claude-code #91573](https://github.com/anthropics/claude-code/issues/91573)
- Demand signals for surfacing questions:
  - #79078 (2026-07-19): questions "get buried in the scrollback and missed", leading to "wasted work (Claude proceeding on assumptions) or stalled work"; the stated workaround "doesn't solve the underlying UI gap". — [anthropics/claude-code #79078](https://github.com/anthropics/claude-code/issues/79078)
  - #64178 (v2.1.154): Claude presents plan start options "even when there are unresolved open questions remaining at the bottom of the plan"; the request is to prompt to answer them or explicitly skip. — [anthropics/claude-code #64178](https://github.com/anthropics/claude-code/issues/64178)
- Plannotator reception:
  - A Show HN about three months ago got 5 points and 2 comments; its pitch is replacing approval of "a wall of text in the terminal", and the comments were positive but light ("plannotator is really nice!"). — [HN 48495970](https://news.ycombinator.com/item?id=48495970)
  - On Plannotator's "guided code reviews" Show HN (~65 days ago), a critic said "AI is adding more stuff for us to read, it's accelerating the code production but slowing down the code review" and asked for tools that "just point you to what you need to focus on and skim the noise"; the maintainer said guided review lets users "skim the code versus read the full diff". — [HN 48797645](https://news.ycombinator.com/item?id=48797645)
  - The repo shows 8.9k stars and continued weekly releases, including removing a first-run setup dialog (2026-09-21). — [plannotator repo](https://github.com/backnotprop/plannotator); [plannotator commits](https://github.com/backnotprop/plannotator/commits/main)
- Abandonment:
  - The HumanLayer OSS repo (source of `create_handoff` / `resume_handoff` and the CodeLayer IDE) says "the code here is pretty much all deprecated"; the team is rebuilding as a hosted product with `.humanlayer/tasks/` artifacts and cloud artifact comments. — [humanlayer/humanlayer](https://github.com/humanlayer/humanlayer); [HumanLayer docs](https://docs.humanlayer.com/reference/skills-workflows)
  - Kiro's 2025 issue shows another failure mode: the "edit the file to answer" channel silently failed (manual `requirements.md` edits ignored) and was closed as not planned. — [kirodotdev/Kiro #2587](https://github.com/kirodotdev/Kiro/issues/2587)
- Design choices that reduce ignoring, as encoded by the tool authors:
  - CE requires "Never re-ask a settled decision".
  - CE's evaluation rubric says "uncertainty resolves to fixing ... not escalating".
  - Devin ships needs-input notifications off by default and sends "a single" notification.
  - Claude artifacts cap auto-replies at 60/hour.
  - superpowers uses the browser only for inherently visual questions.
  - discernment-nudge fires "at most once per conversation". — [settled-decisions.md](https://github.com/EveryInc/compound-engineering-plugin/blob/main/skills/ce-plan/references/settled-decisions.md); [evaluation-rubric.md](https://github.com/EveryInc/compound-engineering-plugin/blob/main/skills/ce-resolve-pr-feedback/references/evaluation-rubric.md); [Devin docs](https://docs.devin.ai/desktop/agent-command-center); [Claude Code docs: artifacts](https://code.claude.com/docs/en/artifacts); [superpowers visual-companion.md](https://github.com/obra/superpowers/blob/main/skills/brainstorming/visual-companion.md); [discernment-nudge](https://github.com/anthropics/skills/blob/main/skills/discernment-nudge/SKILL.md)
- Token frugality has official tooling:
  - `claude plugin details <name>` shows each plugin's "Always-on" token cost (the names plus descriptions of skills, agents and commands).
  - `/plugin` highlights the `Every turn:` cost when it is ≥2,000 tokens.
  - A plugin moves under "Not used recently" after ≥14 days and 10 sessions unused.
  - `/skill-doctor` flags skills never invoked; the `claude_code.skill_activated` OTel event and the Enterprise Analytics API count invocations across Claude Code and Cowork. — [Claude Code docs: measure plugin cost and usage](https://code.claude.com/docs/en/plugins/measure)

### Inferences
- The Codex recap episode maps directly onto Ryan's end-of-session sweep. It argues for:
  - (a) explicit invocation by default, with any automatic trigger opt-in;
  - (b) disclosed token cost;
  - (c) extraction from the full session, or from explicit markers, rather than a truncated tail;
  - (d) never collapsing "pending/unverified/assumed" into "done". Status markers must be carried through verbatim.

  This mirrors CE's no-self-settling rule and ce-handoff's attribution rule.
- Tools that stick attach to an existing stop point (plan approval for Plannotator, "In Review" for Vibe Kanban, the end of a PR thread for CE). That supports running the sweep at chat end, as Ryan intends, as the one natural breakpoint, rather than as a new standalone ritual.
- The #79078 and #64178 requests show unmet demand for the in-session view, not just a cross-session register: pinned open questions, and blocking "go" until they are answered or explicitly skipped.

### Gaps
- No quantitative adoption or abandonment data (usage rates, retention) was found for any question or review UI. Evidence is issues, stars and HN comments only.
- Reddit threads were not reached (searches returned no relevant Reddit results); HN item 47625467 returned HTTP 429 and was not read.
- No postmortem of an abandoned "open questions register" for coding agents was found, consistent with the prior brief.
