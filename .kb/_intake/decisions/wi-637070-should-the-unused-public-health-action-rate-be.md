---
id: "kb-decision-wi-637070"
title: "Remove it"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Should the unused public Health::action_rate be kept (converted losslessly) rather than removed?\", facing health::action_rate is public in weighin-core, has no caller, and was the only source of its cast findings, we decided for Remove it and neglected Keep it, on the premise that no consumer needs it, accepting that if wrong: re-adding it is a one-function, additive change."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-06"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-637070"
question: "Should the unused public Health::action_rate be kept (converted losslessly) rather than removed?"
door: two-way
blast_radius: module
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-06T01:04:55Z"
tree_hash: "180f2a2aad39d71cf697152bbb32bb9f903cdd39"
recommended: "B"
flip_condition: "redkiln-rs or the board page needs the float rate"
---

# Remove it

## Context and problem statement

Should the unused public Health::action_rate be kept (converted losslessly) rather than removed?

Health::action_rate is public in weighin-core, has no caller, and was the only source of its cast findings.

Raised by an agent (marker) as a call and captured by Weigh-In as `wi-637070`. Anchor: `crates/weighin-core/src/projection.rs:509`.

## Decision drivers

- API surface

## Considered options

### A · Keep it

Keep the saturating f64 version as it is in PR 82.

- Holds if a consumer will want a float rate.
- If wrong: one dead public fn to maintain.

### B · Remove it (chosen)

Delete it in PR 82; the CLI never used it.

- Holds if no consumer needs it.
- If wrong: re-adding it is a one-function, additive change.

## Evidence

- `crates/weighin-core/src/projection.rs:509`: pub fn action_rate(&self) -> Option<f64>
- `crates/weighin/src/main.rs`: stats computes the percentage itself with integer rounding

## Decision outcome

Chosen option: **Remove it**, the recommended option.

Decider's note: remove it

### Consequences

- Good, because it holds if no consumer needs it.
- Bad, because if wrong: re-adding it is a one-function, additive change.

### Confirmation

Revisit when: redkiln-rs or the board page needs the float rate

## Why this might be wrong

if a consumer appears, it is re-added

## Provenance

- Decided 2026-10-06T01:04:55Z by human:ryan (user-approved), via chat.
- Raised in session `f8018889-e862-5ea0-9a26-8b6a120b3d19`.
- Verbatim: "kind: call"
