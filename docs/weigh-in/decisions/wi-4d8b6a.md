---
id: wi-4d8b6a
title: "May unattended runs decide two-way, judgment-bound questions themselves and report them in a digest?"
status: accepted
date: 2026-09-26
decision: "A - Yes for two-way items; one-way items block"
door: two-way
authority: authority
blast: system
provenance: user-approved (matched the recommendation)
decided_on: Weigh-In Board sitting, 2026-09-26/27
---

# May unattended runs decide two-way, judgment-bound questions themselves and report them in a digest?

## Context and problem statement

About half your agent work runs unattended. Without a rule, those runs either stall on every question or make silent assumptions, which are the buried questions this skill exists to surface.

Door: two-way. Any default can be reopened from the digest; reopening supersedes it.

Timing: Decide now. Nothing arrives by waiting; the digest's override rate will show later whether to tighten.

Resolve by: before slice 1 ships.

Evidence:

- [settled · Q1] Your agent work is about half interactive, half unattended.
- [ce-sweep SKILL.md] "With no usable blocking-question tool, behave as non-interactive"; only authority-bound items are escalated to the human.
- [docs › sdk user input] AskUserQuestion is unavailable to subagents and in claude -p runs.
- [Amazon 2015 letter] Two-way-door decisions "can and should be made quickly by high judgment individuals or small groups".

## Considered options

### A. Yes for two-way items; one-way items block (chosen)

Unattended runs take a logged default on two-way, judgment-bound items and list it in the digest. One-way items are marked did: blocked and the run continues with independent work.

- Holds if: runs classify door type correctly most of the time
- If wrong: a misclassified one-way item gets a default; the digest shows it and a reopen supersedes it

### B. No; every item waits for you

Unattended runs park each question until you answer, using the SDK defer loop.

- Holds if: unattended runs can wait hours for you
- If wrong: runs stall, and the slice 3 defer loop becomes a prerequisite for slice 1

### C. Yes for every item, one-way included

Unattended runs decide everything and report it all in the digest.

- Holds if: you are willing to review one-way calls after the fact
- If wrong: an irreversible choice is made with nobody asked
- Fails a constraint in force: One-way doors always get a human (principle 1 of the 12 September brief)

| Criterion | A | B | C |
|---|---|---|---|
| Unattended runs keep moving | Yes | No | Yes |
| One-way calls made without you | Never | Never | Yes |
| Build needed first | Slice 1 | Slice 3 defer loop | Slice 1 |

## Decision outcome

Chosen: **A. Yes for two-way items; one-way items block**, decided 2026-09-26T18:13:44Z.

The card's recommendation was A (recommended and pre-selected).
Trade-off: You are trading a small chance of a misclassified default for runs that finish.
What would flip it: The slice 1 eval shows the unattended door classifier calling one-way items two-way more than about 1 in 20 times. Then B for those repos until it improves.
Why it might be wrong: Door classification is the weak link. If the unattended run misjudges one-way items often, A drifts into C by accident; the slice 1 eval must measure that miss rate before you rely on it.
Stability: 4 of 4 (Opus pick plus 3 Sonnet samples, each with the option order permuted and the recommendation removed). All samples judged options written by the same author, so this checks order and framing bias, not authorship bias.

## Later changes

Ryan's note on this card asked about using a separately held model key for the classification. Answered in chat: any key is read from an environment variable at run time and never written to the ledger, and judges stay Anthropic-only ([Q3](README.md#settled-before-the-design)). Which provider that key is for is still open ([roadmap](../roadmap.md)).

## More information

Card as presented and answered: [`2026-09-27-board-sitting/state.json`](2026-09-27-board-sitting/state.json). Design context: [the report](../research/report.md).
