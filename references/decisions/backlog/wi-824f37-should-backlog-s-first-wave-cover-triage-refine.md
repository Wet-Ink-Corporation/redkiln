---
id: "kb-decision-wi-824f37"
title: "Planned wave 1"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Should Backlog's first wave cover triage, refine, split, lint, next, sweep, import and board/sync, leaving prioritisation and forecasting for later?\", facing backlog is the domain most likely to grow into its own product, we decided for Planned wave 1 and neglected Smaller; Larger, on the premise that Loop picks by the Priority field alone, accepting that if wrong: pull scoring forward."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-19"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-824f37"
question: "Should Backlog's first wave cover triage, refine, split, lint, next, sweep, import and board/sync, leaving prioritisation and forecasting for later?"
door: two-way
blast_radius: module
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-19T04:28:35Z"
recommended: "A"
flip_condition: "Loop needs scoring before wave 2"
---

# Planned wave 1

## Context and problem statement

Should Backlog's first wave cover triage, refine, split, lint, next, sweep, import and board/sync, leaving prioritisation and forecasting for later?

Backlog is the domain most likely to grow into its own product.

Raised by an agent (sweep) as a question and captured by Weigh-In as `wi-824f37`. Anchor: `https://claude.ai/code/artifact/6a5d3746-3d34-4c36-8776-2206b0aaf526`.

## Decision drivers

- Commands

## Considered options

### A · Planned wave 1 (chosen)

triage, refine, split, lint, next, sweep, import, board/sync

- Holds if Loop picks by the Priority field alone.
- If wrong: pull scoring forward.

### B · Smaller

lint, next, import, board only

- Holds if you groom by hand for now.
- If wrong: grooming stays manual.

### C · Larger

also scoring, iterations and forecasting

- Holds if you need forecasts now.
- If wrong: Backlog grows before its core works.

## Evidence

- `strategy/suite-structure.md:198`: Prioritisation, estimation, roadmaps and sprints are each a product

## Decision outcome

Chosen option: **Planned wave 1**, the recommended option.

### Consequences

- Good, because it holds if Loop picks by the Priority field alone.
- Bad, because if wrong: pull scoring forward.

### Confirmation

Revisit when: Loop needs scoring before wave 2

## Why this might be wrong

Without scoring, ties in the ready queue fall back to issue number

## Provenance

- Decided 2026-10-19T04:28:35Z by human:ryan (user-approved), via cli.
- Raised in session `740d340f-d7bd-5e81-b390-5a65c8fd6e86`.
- Verbatim: "Should Backlog's first wave cover triage, refine, split, lint, next, sweep, import and board/sync, leaving prioritisation and forecasting for later?"
