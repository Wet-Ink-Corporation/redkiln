---
id: "kb-decision-7374ca1a0c"
title: "Confirm"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Do you confirm the H-12 test moves in #155 (the Bench guard and SessionStart record assertions moved in process onto a frozen clock, with \"the anneal ledger\" check added to the existing test)?\", facing #155 moved two budget-dependent binary assertions in process on a frozen clock to end the Windows flake, we decided for Confirm and neglected Restore binary tests, on the premise that the binary path stays covered by its other tests, accepting that if wrong: a binary-only wiring bug slips past."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-11-21"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-50c70c5bf6"
question: "Do you confirm the H-12 test moves in #155 (the Bench guard and SessionStart record assertions moved in process onto a frozen clock, with \"the anneal ledger\" check added to the existing test)?"
door: two-way
blast_radius: module
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-11-21T04:44:09Z"
tree_hash: "dcd694bbf8be1896c5f3e900883bae2e1a6fc349"
recommended: "A"
flip_condition: "you want binary-level record assertions back"
---

# Confirm

## Context and problem statement

Do you confirm the H-12 test moves in #155 (the Bench guard and SessionStart record assertions moved in process onto a frozen clock, with "the anneal ledger" check added to the existing test)?

#155 moved two budget-dependent binary assertions in process on a frozen clock to end the Windows flake.

Raised by an agent (marker) as a call and captured by Weigh-In as `wi-50c70c5bf6`. Anchor: `docs/decisions.md:163`.

## Decision drivers

- Flake

## Considered options

### A · Confirm (chosen)

Keep the in-process assertions on a frozen clock

- Holds if the binary path stays covered by its other tests.
- If wrong: a binary-only wiring bug slips past.

### B · Restore binary tests

Put the binary-level assertions back with a longer or injected budget

- Holds if a budget can be made reliable on slow runners.
- If wrong: the flake returns.

## Evidence

- `docs/decisions.md:162`: #131 fixed as #90, #92 and M2-J precedent

## Decision outcome

Chosen option: **Confirm**, the recommended option.

### Consequences

- Good, because it holds if the binary path stays covered by its other tests.
- Bad, because if wrong: a binary-only wiring bug slips past.

### Confirmation

Revisit when: you want binary-level record assertions back

## Why this might be wrong

the binary's own wiring of the record path is less directly pinned

## Provenance

- Decided 2026-11-21T04:44:09Z by human:ryan (user-approved), via chat.
- Raised in session `d55e53e3-870a-42df-88f6-aa37684d49c2`.
- Verbatim: "kind: call"
