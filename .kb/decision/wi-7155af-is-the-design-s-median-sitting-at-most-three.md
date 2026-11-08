---
id: "kb-decision-wi-7155af"
title: "Drop it for v1"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Is the design's \"median sitting at most three requests and about 8k fresh tokens\" criterion dropped for v1, given the v1 plan's exit criteria leave it out?\", facing the design's \"median sitting at most three requests and about 8k fresh tokens\" is not among the v1 plan's exit criteria, and no record drops it, we decided for Drop it for v1 and neglected Keep and measure, on the premise that the skill keeps its three-request budget, accepting that if wrong: sittings drift past three requests unnoticed."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-08"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-7155af"
question: "Is the design's \"median sitting at most three requests and about 8k fresh tokens\" criterion dropped for v1, given the v1 plan's exit criteria leave it out?"
door: two-way
blast_radius: external
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-08T05:02:43Z"
tree_hash: "8cf80029551cd98c97139ab9b4b27a400669af5d"
recommended: "A"
flip_condition: "a sitting on #23 takes more than three requests"
---

# Drop it for v1

## Context and problem statement

Is the design's "median sitting at most three requests and about 8k fresh tokens" criterion dropped for v1, given the v1 plan's exit criteria leave it out?

The design's "median sitting at most three requests and about 8k fresh tokens" is not among the v1 plan's exit criteria, and no record drops it.

Raised by an agent (marker) as a question and captured by Weigh-In as `wi-7155af`. Anchor: `docs/research/report.md:627`.

## Decision drivers

- Measurement cost

## Considered options

### A · Drop it for v1 (chosen)

record that the skill's three-request budget enforces the request count, and D-0016 covers tokens

- Holds if the skill keeps its three-request budget.
- If wrong: sittings drift past three requests unnoticed.

### B · Keep and measure

add it to the exit criteria and measure it on #23's sittings

- Holds if you want the 8k fresh-token figure checked.
- If wrong: a measurement nobody has a tool for yet.

## Evidence

- `docs/research/report.md:627`: Not among the v1 plan's exit criteria; no decision record drops it
- `D-0016`: sitting cost recorded as a formula: 3 × context at cache-read price + ~5,600 output tokens
- `SKILL.md`: Budget: three requests

## Decision outcome

Chosen option: **Drop it for v1**, the recommended option.

Decider's note: drop it for v1

### Consequences

- Good, because it holds if the skill keeps its three-request budget.
- Bad, because if wrong: sittings drift past three requests unnoticed.

### Confirmation

Revisit when: a sitting on #23 takes more than three requests

## Why this might be wrong

fresh tokens per sitting may grow with ledger size, which D-0016's formula only covers indirectly

## Provenance

- Decided 2026-10-08T05:02:43Z by human:ryan (user-approved), via chat.
- Raised in session `ca6d9b4d-821f-5563-bf7e-5c243bdbde79`.
- Verbatim: "kind: question"
