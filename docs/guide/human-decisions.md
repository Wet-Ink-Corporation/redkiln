# Human decisions

Weigh-In owns decisions in the suite (blueprint design rule 4), and every other product raises its human gates there (decision requests, SPEC `decision-requests.md`). This chapter is about the behaviour around those records: when an agent may decide, how authority is bound, and how records stay true.

## Guardrails

### HD-1 Decide two-way calls, block on one-way ones, and record both

An agent with no human present takes reversible calls, records each one with what would flip it, and keeps going; one-way calls (new dependencies, MSRV raises, API breaks, SPEC one-way items, releases) block, and the agent does independent work meanwhile. "Make assumptions and keep going delegates the work, not authority-bound decisions."

- **Evidence.** This project's rules; temper `AGENTS.md`, Unattended sessions; weigh-in `plugin/skills/weigh-in/SKILL.md` §2; blueprint, Decision requests (wi-4d8b6a).

### HD-2 Decision quality lives in types, not only prompts

The card type refuses a recommendation that fails a must or is flagged evasive, enforces two to four options, never defaults a one-way or unsure door, and requires flip condition, trigger, resolve-by, default and owner on a deferral. "No decisions need you" is a correct outcome.

- **Evidence.** weigh-in `crates/weighin-core/src/card.rs:103-187`, `board.rs:616`.

### HD-3 Provisional authority has a ratification date

When an unattended agent takes a one-way call in its most reversible form, the record says provisional and names when the human will confirm it. Silence is never confirmation, and nothing is built on a provisional ruling without saying so.

- **Why.** Under redkiln-rs's AFK rule, agents took one-way calls provisionally, often decided by LLM judge panels; several ADRs flipped to accepted on "the operator has not objected". An audit found twenty rulings with no recorded confirmation and code already depending on them; all twenty needed a ratification sitting.
- **Evidence.** redkiln-rs #28, D21-105, D21-156.

### HD-4 An approval is bound to the content it approved

An approval names the hash of what was approved and the method that took it, and is void when the hash moves. A gate answer is `(token, tree_hash, deadline)`: if the tree moved, the answer is void and the gate re-opens with the drift attached, and every gate has a terminal expiry path (fail, escalate or default), never a hang.

- **Evidence.** blueprint design rule 9; SPEC AP; redkiln-rs ADR-0017, ADR-0129; anneal `plan approve` (sha256 of the plan's bytes).

### HD-5 An approval names a human the agent cannot impersonate

A human approval is recorded from a surface the agent cannot drive (a decision card answered in the app, a signed command) and records that surface. An approval that a model could have produced through the same CLI, account or file does not count as human.

- **Why.** The agent's GitHub identity is the owner's daily account (assay decision 0005), so GitHub approvals cannot tell owner from agent; SPEC TR-10 makes every GitHub review and comment command `asserted` meanwhile. anneal denies `--actor human` from a model's Bash; `approver: human` in the Node Redkiln was opt-in to auto-approval, and the AI reviewer held a veto over whether the human gate ever fired.
- **Evidence.** assay decision 0005, R3-01, #269; anneal wi-10f284; redkiln-old A3-03, B1-15. See [Tension T5](README.md#tensions-with-the-plan).

### HD-6 A verdict call bounds its walk

A command that advances state on an approval stops at the next human gate. "Forward only" plus "walk until a gate" plus a stage with no gate means an approval can carry work past the human, irreversibly.

- **Why.** A bare `advance --verdict approved --commit` in the Node Redkiln walked items through `report` into `done` at least 17 times in five days, each repaired with a hand `git revert`; the guard was a paragraph of prose. A command gate must also guard both sides of its stage: `advance --to` skipped intermediate blocking gates because entry and exit semantics disagreed.
- **Evidence.** redkiln-old reverts `59268262` … `a14813fe`, `references/2026-07-21-advance-to-stage-bypasses-blocking-gates.md`, A9-07.

### HD-7 A recorded decision is never edited; it is superseded or corrected in errata

If a correction leaves every decision sentence standing, it goes in errata; anything that moves a decision is a new record that says what it supersedes, in part or in full. Keep a census of what is currently true, because partial supersessions pile up (61 of 198 Loop ADRs carry one).

- **Evidence.** redkiln-rs ADR-0092, `references/adr/ERRATA.md`; weigh-in `CLAUDE.md` ("Settled decisions are not reopened without new evidence").

### HD-8 Freeze behaviour while a pilot measures it

When a pilot measures a behaviour, behaviour-changing PRs wait until its report, so the pilot measures one thing. Exit criteria may shorten on evidence, recorded as an amendment.

- **Evidence.** temper wi-b63bd6; weigh-in wi-7c3aa0, D-0019.

## Patterns worth copying

- **One convention line in the end-of-turn grammar moved capture from 1/7 to 12/13** ("a list of calls for the user gets one block per call") (weigh-in #68).
- **The end-of-turn block is forgiving everywhere except its key field**: values case-insensitive with aliases and defaults, a block without `q:` is not an item, a `#` comment needs a following space so `#123` survives, a block inside an outer example fence is not captured (weigh-in `crates/weighin-core/src/marker.rs`).
- **Measure whether the rendering was enough**: Loop's `answer` requires `--explain-sufficed` and `--toml-opened` on every gate answer (redkiln-rs D21-96).
- **Budget a decision sitting in requests, not tokens**: three requests re-reading a 585k-token context made up 60-75% of a sitting's cost (weigh-in D-0016).
- **The control loop working means it halted correctly**: a halt on an irreducibly human action is success, not a stuck pipeline; model constraints that nothing encodes (a shared design budget) as data (redkiln-old readiness ruling §1, C1-01).

## Gotchas

- **A board button recorded the recommendation as the owner's choice** when nothing was picked (weigh-in CHANGELOG 0.6.0). A default is never an answer.
- **Atoms titled by the chosen option are unreadable** ("Yes", "Tighten now"), and atoms do not record the chosen option at all (weigh-in `crates/weighin-core/src/atom.rs:230-240`, HITL-1).
- **A one-way ruling recorded in the wrong product's ledger blurs domain ownership** (anneal wi-1154f8 records orchestrator rulings in anneal's ledger).
- **Settings granted to stop prompts during an AFK run outlive it** ("allowlist everything", anneal wi-f701c2).

## Open problems

- **Decisions are never re-examined**; D-0022 (due 90 days after the newest decide or review, two-way decisions with a flip condition only) is designed, not built (weigh-in #173).
- **No approver identity** distinct from the agent (assay #41, #269).

## Prior art

- Bezos's Type 1 and Type 2 decisions; the failure is using the heavy process for most decisions. **Agree; adopt** tracking the one-way share of ledger items. [Farnam Street](https://fs.blog/reversible-irreversible-decisions/)
- Nygard ADRs: never delete, supersede; record Consequences. **Agree; adopt** a consequences field in decision atoms. [Documenting Architecture Decisions](https://cognitect.com/blog/2011/11/15/documenting-architecture-decisions)
- MADR records the considered options. **Adopt**: atoms keep rejected options, which cards already carry. [JabRef ADR-0000](https://jabref.readthedocs.io/en/latest/adr/0000-use-markdown-architectural-decision-records/)
- LangGraph's `interrupt()` pauses on a checkpointer until a human resumes. **We differ**: ours is asynchronous, so the session keeps doing independent work. [LangGraph human-in-the-loop](https://docs.langchain.com/oss/python/langgraph/human-in-the-loop)
- GitHub dismisses stale reviews on push and can require the last push to be approved by someone else. **Agree, and we are ahead**: hash binding survives a no-op rebase and voids on any change. [Copilot: the requester cannot approve](https://docs.github.com/en/copilot/concepts/agents/cloud-agent/risks-and-mitigations)
