---
id: "kb-decision-wi-77b8b6"
title: "Yes, grounded cards only"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"May grounded cards be decided from a Claude Code mod pane outside a sitting, with raw items read-only?\", facing the pane moves the moment of decision away from the sitting, we decided for Yes, grounded cards only and neglected No, on the premise that the filters ran in a sitting, accepting that if wrong: pane decisions are revisited."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-11"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-77b8b6"
question: "May grounded cards be decided from a Claude Code mod pane outside a sitting, with raw items read-only?"
door: two-way
blast_radius: module
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-11T14:17:21Z"
tree_hash: "f81993f6723ef0895287e8171b003c754b86b6ca"
recommended: "A"
flip_condition: "pilot shows pane decisions are worse"
---

# Yes, grounded cards only

## Context and problem statement

May grounded cards be decided from a Claude Code mod pane outside a sitting, with raw items read-only?

The pane moves the moment of decision away from the sitting.

Raised by an agent (sweep) as a question and captured by Weigh-In as `wi-77b8b6`. Anchor: `docs/decisions/build-decisions.md:185`.

## Considered options

### A · Yes, grounded cards only (chosen)

Pane decides grounded cards; raw items read-only

- Holds if the filters ran in a sitting.
- If wrong: pane decisions are revisited.

### B · No

Pane is read-only

- Holds if decisions need the warm context.
- If wrong: each decision keeps costing a turn.

## Evidence

- `https://github.com/Wet-Ink-Corporation/weigh-in/issues/115`: Raw items read-only; grounded cards decidable

## Decision outcome

Chosen option: **Yes, grounded cards only**, the recommended option.

Decider's note: Ryan in the mods thread, 2026-10-11: yes (D-0017)

### Consequences

- Good, because it holds if the filters ran in a sitting.
- Bad, because if wrong: pane decisions are revisited.

### Confirmation

Revisit when: pilot shows pane decisions are worse

## Why this might be wrong

context may matter more than expected

## Provenance

- Decided 2026-10-11T14:17:21Z by human:ryan (user-approved), via chat.
- Raised in session `72702aeb-7823-5243-8308-ccb0e2581026`.
- Verbatim: "May grounded cards be decided from a Claude Code mod pane outside a sitting, with raw items read-only?"
