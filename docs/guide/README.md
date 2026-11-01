# The redkiln field guide

Guardrails, patterns, gotchas and prior art for building the `rk-*` suite, distilled from the six repositories it grows out of and compared with published practice.

The blueprint (`references/docs/blueprint.md`) says what the suite is. The migration plan says in what order it arrives. The SPEC (`spec/`) says what its shared data means. This guide says **how its code has to behave**, because each lesson below was paid for once already in weigh-in, anneal, assay, temper, redkiln-rs or the original Node Redkiln. It complements the plan and does not re-plan it: where a lesson pulls against a decision, the decision stands and the tension is listed in [Tensions with the plan](#tensions-with-the-plan) for Ryan.

Written 2026-11-01 from the repositories at these heads: weigh-in `f0e553e`, weigh-in-plugin `59d0a76`, anneal `7f41aa2`, assay `34cce61`, temper `82d3176`, redkiln-rs `build/phase-0-to-1` at `b3b28a2`, redkiln-old `e61b68f`.

## How to use it

- **Before a move**, read the chapters for what the product touches, and turn each guardrail that applies into a check, a test or a charter line in the move's PR.
- **When reviewing**, cite a guardrail by its id (`HK-3`, `HA-5`) the way a rust-craft rule is cited. Ids are stable: a withdrawn guardrail keeps its id and is marked withdrawn.
- **When a lesson is relearned**, add it here with its evidence. A guardrail with no evidence does not belong in this guide.

The guide is guidance, not normative text. Where a guardrail should bind every plugin, the place for it is the SPEC (the hook conventions and end-of-turn grammar sections arrive with Move 2) or a structural check such as `cargo xtask layers`. Several chapters end with candidates for exactly that.

## Chapters

| Chapter | Ids | Covers |
| --- | --- | --- |
| [Hooks](hooks.md) | HK | Failure policy, time and input budgets, the Stop gate and honest exits, deny messages, what hooks receive |
| [Plugins and surfaces](plugins-and-surfaces.md) | PS | Packaging, the launcher, mods, versions, marketplaces and org sync, and the surfaces a plugin actually runs on |
| [Headless agents](headless-agents.md) | HA | Supervising `claude -p`: environment, flags proved by observation, structured output, stream-json, processes, version pins, sandboxes |
| [Judgement and evals](judgement-and-evals.md) | JE | Model judgement that does not drive control flow until scored; verdicts, findings, corpora, statistics, paid evals |
| [Trust and security](trust-and-security.md) | TS | Guards as tripwires, protected artefacts, prompt injection, CI trust boundaries, fixers and the specs they are judged by |
| [State and storage](state-and-storage.md) | SS | The event log, SQLite under synced folders, schema versions, tiers of authority, journals, locks |
| [Git](git.md) | GT | One wrapper, refs and compare-and-set, trailers, worktrees, hardened git for untrusted objects, fixtures |
| [CLI contracts](cli-contracts.md) | CL | `--json`, exit codes, render never parse, typed outcomes, strict arguments |
| [Human decisions](human-decisions.md) | HD | One-way and two-way calls, provisional authority, approvals bound to content, the decision record |
| [Cross-platform](cross-platform.md) | XP | Windows, line endings, paths, processes, proxies |
| [CI and release](ci-and-release.md) | CR | Workflows, required checks, supply chain, versions, release surfaces |
| [Working practices](working-practices.md) | WP | Testing that can fail, first contact with the real thing, adversarial review, keeping process light |

Supporting material:

- [`field-notes/`](field-notes/): the per-repository research notes this guide was distilled from, about 300 cited lessons. Read them when a guardrail's evidence line is not enough.
- [`prior-art/`](prior-art/): the two research notes comparing our practices with published practice (agent tooling; Rust, git and CI), each bullet marked agree, adopt or we differ, with its URL.

## The twelve that matter most

If you read nothing else, these are the lessons that cost the most to learn and that every product in the suite will meet.

1. **A control that cannot fail visibly is not a control.** Weigh-In's capture was silently off three times; the old audit's own scorer published 66.7 from 5 of 33 dimensions; eight checkpoint greps in redkiln-rs passed on empty input. Every check gets a positive control, and every "off" says so out loud ([WP-1](working-practices.md#wp-1-every-check-has-a-positive-control), [HK-2](hooks.md#hk-2-never-off-without-a-word)).
2. **Hooks fail open, guards fail closed, a missing install warns.** A synced plugin with no binaries blocked every tool call until anneal PR 140 ([HK-1](hooks.md#hk-1-hooks-fail-open-guards-fail-closed-a-missing-install-warns)).
3. **A guard is a tripwire; the boundary is where every route passes.** anneal's shell guard has 38 hardening commits and six open bypasses; temper's took nine review rounds. The authoritative gate is a required GitHub check or a sandbox ([TS-1](trust-and-security.md#ts-1-a-guard-is-a-tripwire-the-boundary-is-where-every-route-passes)).
4. **A timed-out PreToolUse hook lets the command run.** So a slow regex is a bypass, and guards are linear-time by construction ([HK-4](hooks.md#hk-4-a-guard-is-linear-time-by-construction)).
5. **No mod in a plugin that ships skills.** The desktop app's chats turn the whole plugin off. It bit temper, weigh-in and anneal in the same week ([PS-2](plugins-and-surfaces.md#ps-2-no-mod-in-a-plugin-that-ships-skills-or-commands)).
6. **Test on every surface a person uses.** CI runs Claude Code and cloud sessions only; three of the worst bugs of October lived in desktop chats and Cowork ([PS-6](plugins-and-surfaces.md#ps-6-a-release-is-checked-on-every-surface-a-person-uses)).
7. **Prove every `claude -p` flag took effect from the child's own `system/init`.** An ignored flag looks exactly like an honoured one; exit 0 does not mean structured output arrived ([HA-2](headless-agents.md#ha-2-prove-every-flag-from-the-childs-own-systeminit), [HA-3](headless-agents.md#ha-3-exit-0-is-not-a-completed-structured-dispatch)).
8. **A transport failure is never a verdict.** One HTTP 529 became a committed `changes-requested` seal in the Node Redkiln ([CL-3](cli-contracts.md#cl-3-a-transport-failure-is-never-a-verdict)).
9. **Model judgement does not drive control flow until it is scored, and the verdict string is the least stable thing a reviewer emits.** Findings are stable; verdicts flip ([JE-1](judgement-and-evals.md#je-1-judgement-ships-in-shadow-and-is-promoted-on-numbers), [JE-3](judgement-and-evals.md#je-3-policy-over-typed-findings-not-the-verdict-string)).
10. **A fixer never writes the spec, the labels or the verdict it is judged by.** Repair passes reworded acceptance criteria to fit what shipped, and both review layers passed it ([TS-6](trust-and-security.md#ts-6-a-fixer-never-writes-what-it-is-judged-by)).
11. **Make the next action derivable from durable state, and be re-entrant from it.** The deleted-database test, the two-axis trailers and every prose-orchestrated stage that could not resume after a partial run ([SS-4](state-and-storage.md#ss-4-the-deleted-database-test-decides-what-is-authoritative)).
12. **Provisional authority needs a scheduled ratification.** Twenty unattended one-way rulings in redkiln-rs became policy by silence before anyone confirmed them ([HD-3](human-decisions.md#hd-3-provisional-authority-has-a-ratification-date)).

## Citation format

Evidence lines name the repository and then the most durable reference it has:

| Form | Means |
| --- | --- |
| `weigh-in B-35`, `weigh-in D-0015` | weigh-in's build decisions (`docs/decisions/build-decisions.md`) and decision records |
| `anneal PR 140`, `assay #175` | a pull request or issue in that repository |
| `temper@84d2a0f` | a commit |
| `assay R3-05` | a finding in assay's adversarial review rounds (`docs/review/`) |
| `redkiln-rs ADR-0109`, `redkiln-rs D21-167`, `redkiln-rs F20` | an ADR, a ruling in `spikes/DECISIONS-2026-09-21.md`, a finding in `spikes/FINDINGS.md` |
| `redkiln-old A5-01` | a finding in `references/audit/2026-08-30-state-of-the-system/findings/_index.md` |
| `wi-xxxxxx` | a Weigh-In decision, in the repository named beside it |
| `path:line` | a file in the repository named beside it, at the head listed above |

## Tensions with the plan

None of these overrides a decision. Each is a place where the evidence gathered here pulls against the plan, the SPEC or a repository's own charter, written down so Ryan can decide whether it matters.

| # | Tension | Where | Suggested handling |
| --- | --- | --- | --- |
| T1 | **Environment policy for `claude -p` children.** assay builds the child environment from nothing (an allowlist) and pins the CLI; redkiln-rs inherits everything minus a denylist, because Claude Code reads undocumented variables and an allowlist broke it. Both refuse a stray API key. `redkiln-dispatch` has to pick one | blueprint, Kits (`redkiln-dispatch`); [HA-1](headless-agents.md#ha-1-build-the-childs-environment-on-purpose-and-refuse-conflicting-credentials) | A two-way call for the dispatch kit at the move that creates it: allowlist in CI against a pinned CLI, inherit-minus-denylist on a developer machine, one denylist in both |
| T2 | **`--bare` for headless runs.** Anthropic's headless docs recommend `--bare` for scripted calls and say it will become the default for `-p`; assay bans it because it ignores the subscription OAuth token | [HA-6](headless-agents.md#ha-6-headless-isolation-is-chosen-per-auth-mode-and-checked-by-canary) | Watch for the default change; it would silently switch subscription runs to API billing or break them. Canary the auth path on every CLI pin move |
| T3 | **Hook conventions are not in the SPEC until Move 2**, but Move 1 ships Weigh-In's hooks into the suite | `spec/README.md`, Not in this draft | Move 1 adopts [Hooks](hooks.md) HK-1 to HK-8 as charter rules for `plugins/weigh-in/`, and Move 2 lifts them into the SPEC |
| T4 | **Design rule 7 says guards fail closed**; the guard chapters of anneal and temper show a guard cannot be complete. The rule is right, but read alone it suggests a guard is a security boundary | blueprint, Design rules 7 | When the SPEC's hook section is written, say that guards are tripwires and name the boundary each one is backed by ([TS-1](trust-and-security.md#ts-1-a-guard-is-a-tripwire-the-boundary-is-where-every-route-passes)) |
| T5 | **Approvals bound to a decider** (SPEC `approvals.md`, `trust.md`) assume a human identity distinct from the agent's. Today the agent's GitHub identity is Ryan's own account (assay decision 0005), so an approval "by Ryan-Britton" on GitHub cannot tell owner from agent | SPEC TR, AP; [HD-5](human-decisions.md#hd-5-an-approval-names-a-human-the-agent-cannot-impersonate) | Keep human approvals on surfaces the agent cannot drive (Weigh-In cards answered in the app, a signed command), and record the surface; an owner-only identity question for Ryan before Review moves |
| T6 | **Root charters are long.** Claude Code advises under 200 lines per `CLAUDE.md` with imports counted, and Codex truncates the combined `AGENTS.md` chain at 32 KiB, which would cut the crate charter first. assay's root charter is mostly its decisions log | blueprint design rule for harness-agnosticism; [WP-8](working-practices.md#wp-8-charters-stay-small-enough-to-load-whole) | As products move, keep the decisions log in `docs/<plugin>/decisions.md` with a pointer from the charter, and add a size check |
| T7 | **temper's 2.0 port still publishes through npm**, against the blueprint's `libexec/` bundling. Already reopened by the blueprint (D1, D6, D13); listed so the port's next PR sees it | temper `CHANGELOG.md`, `release-2.yml`; blueprint, What the audit changed | No action here; the port decides inside the monorepo |
| T8 | **Loop asserts WAL**; Core's store is TRUNCATE. Already handled by R-SQLITE in the blueprint; the load half of R-SQLITE has never run | redkiln-rs `crates/rk-store/src/conn.rs`; blueprint, Data and state | Run R-SQLITE's load half before Move 4, as the blueprint says |

## Related work in this project

- The **harness-agnostic proposal** (thread "Keep redkiln harness-agnostic", 2026-11-01) covers CLI first, a thin plugin layer, a `cargo xtask plugins` check and a model runner behind a port. This guide's [Plugins and surfaces](plugins-and-surfaces.md) and [Headless agents](headless-agents.md) chapters assume that direction and do not repeat it; where they name a check, it belongs in that proposal's `xtask plugins` once it lands.
- The **refs spike** (OW-13, redkiln PR #9) settled where journals live; [State and storage](state-and-storage.md) cites it rather than re-arguing it.
- The **org-sync spike** (wi-f041fb) settled packaging for claude.ai; [Plugins and surfaces](plugins-and-surfaces.md) builds on its findings.
- The **October audits** (`references/strategy/audit/`) describe each repository's architecture. The field notes here start where they stop.
