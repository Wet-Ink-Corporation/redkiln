---
id: "kb-decision-wi-b51069"
title: "Keep #74 as one docs PR"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Should B-17 (#17) and the V5 results (#19) ride on PR #74, opened as Refs #22, despite the one-PR-per-issue rule?\", facing ryan's ground rule is one PR per issue, and #74 now carries docs for #22, #17 and #19, we decided for Keep #74 as one docs PR and neglected Split into three PRs, on the premise that the rule is about code changes, not doc-only status notes, accepting that if wrong: #17 and #19 history is mixed into a #22 PR."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-05"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-b51069"
question: "Should B-17 (#17) and the V5 results (#19) ride on PR #74, opened as Refs #22, despite the one-PR-per-issue rule?"
door: two-way
blast_radius: module
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-05T04:53:56Z"
tree_hash: "f96a7b40423bef9378270a68e5062f430dd4d407"
recommended: "A"
flip_condition: "Ryan wants per-issue traceability even for doc notes"
---

# Keep #74 as one docs PR

## Context and problem statement

Should B-17 (#17) and the V5 results (#19) ride on PR #74, opened as Refs #22, despite the one-PR-per-issue rule?

Ryan's ground rule is one PR per issue, and #74 now carries docs for #22, #17 and #19.

Raised by an agent (sweep) as a call and captured by Weigh-In as `wi-b51069`. Anchor: `docs/decisions/build-decisions.md:398`.

## Decision drivers

- Follows the ground rule
- Merge work

## Considered options

### A · Keep #74 as one docs PR (chosen)

Retitle and add Refs #17 and #19 to the body; one review, no roadmap.md conflicts.

- Holds if the rule is about code changes, not doc-only status notes.
- If wrong: #17 and #19 history is mixed into a #22 PR.

### B · Split into three PRs

Move e7f218e and 6c125e9 to their own branches and PRs off main.

- Holds if each issue's record must be reviewable and revertible alone.
- If wrong: three PRs touch roadmap.md and v1-plan.md, so two need conflict merges.

## Evidence

- `PR #74`: opened Refs #22: seed rs9-01 and docs since 0.5.2
- `commit e7f218e`: B-17: defer V2 to slice 2 (Refs #17)
- `commit 6c125e9`: Roadmap: V5 run results (Refs #19)

## Decision outcome

Chosen option: **Keep #74 as one docs PR**, the recommended option.

### Consequences

- Good, because it holds if the rule is about code changes, not doc-only status notes.
- Bad, because if wrong: #17 and #19 history is mixed into a #22 PR.

### Confirmation

Revisit when: Ryan wants per-issue traceability even for doc notes

## Why this might be wrong

If the rule exists so each issue's PR shows its whole story, A hides #17 and #19 inside #22's.

## Provenance

- Decided 2026-10-05T04:53:56Z by human:ryan (user-approved), via chat.
- Raised in session `4739eb43-c237-535b-9ba1-71a4f560592c`.
- Verbatim: "Should B-17 (#17) and the V5 results (#19) ride on PR #74, opened as Refs #22, despite the one-PR-per-issue rule?"
