# Trust and security

redkiln's plugins sit between a model that will do whatever its context rewards, untrusted content (PR text, diffs, comments), and authority that matters (merges, approvals, verdicts, money). This chapter is what the repositories learned about where a control can actually hold.

## Guardrails

### TS-1 A guard is a tripwire; the boundary is where every route passes

A hook that reads a shell command can never be complete. Guards exist to stop honest mistakes cheaply and early; every guard names the authoritative control behind it: a required GitHub check, a sandbox, a permission rule, or a CI gate. Spend fail-closed strictness where it is cheap (temper's read-only reviewer denies anything it cannot read exactly) and keep other agents best-effort.

- **Why.** anneal's shell guard has 38 hardening commits since mid-October and six open bypass issues; Ryan ruled word splitting and `git push "$branch"` documented gaps. temper's #151 took nine review rounds, each finding another route (`gpatch`, `busybox`, `$'...'`, `env -S`, git aliases, `trap`, awk `system()`). Both repositories reached the same conclusion: the hook is a tripwire, and the real gate is a GitHub required status (anneal's `anneal/verdict`).
- **Evidence.** anneal `docs/guard.md:349-430`, #116, `@c34e565`; temper wi-9792d6, wi-0e226b, `scripts/lib/shell.mjs:1511-1603`. See [Tension T4](README.md#tensions-with-the-plan).

### TS-2 Every artefact a gate trusts is protected on every write route

If a gate reads a file (verdicts, gate records, baselines, ledgers, approvals), that file is unwritable from the agent through Edit, Write **and** Bash, including redirects, `cp`, `tee`, interpreters, `cd` into the directory and variables set earlier. Keep such state under `<git-common-dir>/` and protect the path, not the filename.

- **Why.** temper's audit found a shell redirect could forge a green gate record or an approving verdict, and Stop trusted it. anneal's `human-only` rule still cannot see a model that appends `"actor": "human"` straight to its JSONL.
- **Evidence.** temper audit engine-1-6, `@c6eaac2`, `scripts/lib/hooks.mjs:737`; anneal `docs/guard.md:409-411`.

### TS-3 A model cannot record a human act

A command that records a human approval or label refuses when run by the model (`--actor human` from a model's Bash is denied), and a gate counts a human approval only when the actor is recorded as human. Raising a baseline is a human act enforced in three places: the CLI refuses under `CLAUDECODE=1` unless the human sets an override, the hook asks, and CI flags every raise.

- **Evidence.** anneal wi-10f284, `@504cc93`, `docs/verdict.md:40-46`; temper wi-d23383, `scripts/temper.mjs:271`.

### TS-4 Make model text inert by construction; never parse the attacker's language

Do not try to recognise dangerous Markdown, HTML or Rust attributes in model or PR text. Escape every ASCII punctuation character, turn `@` and `#` into fullwidth forms, defang `://`, `www.` and `gh-<n>`, drop leading whitespace and control characters, and let only publisher-built links and a closed list of fence info strings exist (no `suggestion`). Write down the accepted residuals.

- **Why.** assay's mechanical validation took eight review rounds, each finding a new gap between one CommonMark reading and GitHub's renderer; its temper lexer went through at least four rounds of desyncs. The by-construction sanitiser ended the series.
- **Evidence.** assay #175, #88, #86, #63, `crates/assay-app/src/publish/sanitise.rs`.

### TS-5 Untrusted input is constrained, not detected

Once an agent has read untrusted content, it cannot trigger a consequential action: it emits a document that deterministic code validates and publishes. Strip HTML comments and invisible characters from PR bodies and comments before they reach a prompt, keep PR titles, branch names and commit messages out of instructions, and drop comments from users without write access.

- **Evidence.** assay's reviewer pipeline (publication by `decide` and `publish`); prior art below (lethal trifecta, the design-patterns paper, Copilot cloud agent's input filtering, Comment-and-Control).

### TS-6 A fixer never writes what it is judged by

The pass that repairs work never edits the acceptance criteria, labels, rubric or verdict it will be judged against. Make those paths trust paths that only a human change can touch.

- **Why.** A Node Redkiln repair pass reworded an acceptance sentence to match what shipped, and both review layers passed it (C2-03); a fix story's provenance was the commit that narrowed its own criteria (C2-01); the fix pass was told to rewrite the review record (B5-04). assay rediscovered it and made its corpus a trust path.
- **Evidence.** redkiln-old C2-01, C2-03, B5-04; assay decisions log 2026-10-14 (#230).

### TS-7 Fail closed is a named policy, briefed up front

Every security control's open gaps are settled toward less eligibility, and the policy has a name agents can cite (assay's wi-b5affa, "refuse rather than model"): absent lists are empty, an unreadable lease is never reaped, a 404 counts as absent only once the token can see the repository. Brief every security story with it before it starts.

- **Evidence.** assay `docs/handoff-2026-10-12.md`, wi-3981eb, wi-850eb7, wi-67ab73.

### TS-8 Redact by allowlist, not by denylist

When command text or prompts leave the machine (telemetry, a judge), render only what a fixed table names (program names, options, separators) and replace every value; or drop every quoted string before applying secret patterns. Denylist redaction leaked in both repositories that tried it.

- **Evidence.** anneal `@d2e340a` (#120, after #94 and #95 leaked cookie pairs); temper wi-f4e663, `scripts/lib/project.mjs:491-545`.

### TS-9 Client data stays local, enforced in the binary

Repo class is checked by the CLI, not by skill wording: export refuses unless the config exists, parses and allows it; a broken config stops every command; unknown classes are refused at `init`; and `init` changes only what its flags name.

- **Why.** weigh-in's board export printed a note and exported anyway; an uninitialised folder counted as an own repository with a publishing board; `--class clinet` was stored; `init --ledger` silently reset a client repository to `own`.
- **Evidence.** weigh-in B-25, #169, `@54dbc70`; blueprint design rule 10.

## CI trust boundaries

These come from assay's three adversarial rounds and its owner's decisions; they apply to every workflow redkiln ships, and to Review's deployment first.

- **Re-runs are forbidden twice.** Every secret-bearing job carries `if: github.run_attempt == 1` from the first template, and the binary refuses `GITHUB_RUN_ATTEMPT != 1` before reading any token. A re-run reuses earlier jobs' outputs, launders flakes, and brings back an old workflow with today's secrets (assay R3-05).
- **A public repository makes fork PRs live.** Every `pull_request_target` workflow refuses a head from another repository before any secret-bearing job, as a completed `failure` with a reason, never a skip (assay decision 0006). GitHub will enforce its default `pull_request_target` policy on public repositories from 2026-12-02; settle each such workflow before then (prior-art/agents.md §4).
- **Commands are events, not state.** An edited comment can only revoke a command, never grant one: parse commands from the triggering `created` payload, record them with the body hash, and void the record if the comment changed (assay R3-02).
- **A neutral conclusion passes a required check, per SHA, across bases.** Never conclude `neutral` on a security condition; re-fetch the base on every decision (assay R3-03).
- **Concurrency groups evict before job filters run**, silently dropping an owner's command; derive the group key from the filter (assay R3-10).
- **`pull_request_review` and `labeled` run the PR's own workflow file**; `pull_request_target` runs the default branch's on github.com. Environment deployment-branch policies, not the trigger, guard secrets (assay R2-1, R3-04).
- **Workflow text can read differently to a parser than to GitHub** (bare carriage returns, tabs); refuse those forms and treat undeclared `permissions` as write (assay #284, wi-5c7803, wi-d4c1d4).
- **The Actions cache is a poisoning channel** for decision-bearing jobs; a repository test allowlists cache-free actions per job (assay ADR-21, `crates/assay-cli/tests/it/workflow_cache.rs`).

## Patterns worth copying

- **The ledger accepts only records byte-identical to what it writes.** A hand edit or serde leniency could smuggle in a record that parses but was never written; reads refuse anything that does not round-trip through the writer, and contract reads refuse duplicate keys and oversize documents (assay wi-0351d8, #145).
- **Every write cap is sized so the file can be read back** (ledger files 4 MiB so the largest blob fits the 8 MiB response cap) (assay wi-c8f4d6).
- **Hardened git for PR objects**: no system or global config, no hooks, `https` only, fsck on fetch, `attr.tree` set to the empty tree, `--no-ext-diff --no-textconv`, each process its own group ([GT-8](git.md#gt-8-git-over-untrusted-objects-is-hardened)).
- **`gh` expands `{owner}`, `:repo` and `:branch` anywhere in a path**, so refuse those in attacker-influenced paths before gh runs (assay `crates/assay-forge/AGENTS.md:46,102`).

## Gotchas

- **The agent's GitHub identity is the owner's daily account**, so GitHub cannot tell an agent's approval from Ryan's, and the account is an org owner an agent can reach (assay decision 0005, R3-01). See [HD-5](human-decisions.md#hd-5-an-approval-names-a-human-the-agent-cannot-impersonate).
- **The reviewer's "no write access" was nominal**: `tools: [Read, Bash]` with Bash unscoped; `Bash(git commit:*)` permits `--no-verify`; `Bash(redkiln:*)` pre-approved every state mutation (redkiln-old B5-02, B1-14, B1-08).
- **Auto mode gates workflow edits that cross repository boundaries**; a release step pushing with a deploy key needed an explicit human allow (weigh-in wi-dba31a). Installing the latest `@anthropic-ai/claude-code` in a job that holds a write token was accepted unpinned; a single release repository holding every plugin's deploy key should revisit that (weigh-in wi-497dc5).
- **Cloud sessions sit behind a TLS-terminating proxy**; a Rust client with bundled roots fails. Honour `HTTPS_PROXY` and `SSL_CERT_FILE`, and name variables in errors, never their values (anneal `@d0b6692`, wi-077990).
- **A plugin's hooks under `claude plugin eval` run on the host with your rights** (anneal `evals/README.md:92`).
- **Case-insensitive file systems let `RM -rf /` past name matching** (anneal `docs/guard.md`, Limits).

## Open problems

- **Nothing outside a session checks that an approving review existed for the merged tree** (temper audit §1). A CI-visible verdict status, as anneal posts, is the shape of the fix.
- **No approver identity** distinct from the agent's yet (assay #41, #269).
- **A compiler-enforced backstop for lint exceptions** (`-F <lint>` after macro expansion) waits on the owner (assay #172).
- **Waivers carry too rarely**, because the binding covers whole gate documents (assay #292).

## Prior art

- The lethal trifecta: private data, untrusted content and external communication together make an agent exploitable, and 95% detection "is very much a failing grade". **Agree**: remove the exfiltration leg rather than trust detection. [Simon Willison](https://simonwillison.net/2025/Jun/16/the-lethal-trifecta/)
- Once an agent has read untrusted input, constrain it so the input cannot trigger consequential actions (Plan-Then-Execute, Dual LLM, Map-Reduce). **Agree; adopt** naming the pattern in the threat model and one isolated reviewer per shard for large diffs. [Design Patterns for Securing LLM Agents (arXiv 2506.08837)](https://arxiv.org/abs/2506.08837)
- CaMeL extracts control flow from the trusted query only and enforces capabilities on tool calls. **Agree in spirit**; a cheap step toward it is tagging each published string with its provenance. [Defeating Prompt Injections by Design (arXiv 2503.18813)](https://arxiv.org/abs/2503.18813)
- OWASP LLM01 and LLM06: constrain behaviour, validate outputs deterministically, least privilege, human approval for high-impact actions, adversarial testing. **Agree; adopt** an injection suite as a must-not-regress set. [LLM01](https://genai.owasp.org/llmrisk/llm01-prompt-injection/), [LLM06](https://genai.owasp.org/llmrisk/llm062025-excessive-agency/)
- GitHub Security Lab on pwn requests: artifacts from untrusted PR data are themselves untrusted; label gating is racy; `persist-credentials: false`. **Agree; adopt** a check that no privileged job persists credentials. [Preventing pwn requests](https://securitylab.github.com/resources/github-actions-preventing-pwn-requests)
- Copilot's cloud agent filters hidden characters from input, shows the agent no comments from non-writers, and cannot approve or merge its own work. **Adopt** the input filtering. [Copilot cloud agent risks and mitigations](https://docs.github.com/en/copilot/concepts/agents/cloud-agent/risks-and-mitigations)
- Hooks are best-effort; "use the permission system rather than a hook to enforce a hard allow or deny". **Agree**: that is TS-1. [Hooks reference](https://code.claude.com/docs/en/hooks)
