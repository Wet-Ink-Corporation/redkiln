---
id: "kb-decision-wi-b950a4"
title: "Keep capturing"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"With a broken .kb/_ledger/config.json, should capture hooks keep writing to the default ledger (and say so at SessionStart) rather than skip writing until the file is fixed?\", facing with a broken config.json, 0.6.1 hooks keep capturing into the default ledger and say so at SessionStart, we decided for Keep capturing and neglected Stop capturing, on the premise that losing captures is worse than a misplaced ledger, accepting that if wrong: items may land outside a synced folder's chosen store."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-11"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-b950a4"
question: "With a broken .kb/_ledger/config.json, should capture hooks keep writing to the default ledger (and say so at SessionStart) rather than skip writing until the file is fixed?"
door: two-way
blast_radius: module
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-11T15:29:52Z"
tree_hash: "9e3e3fc007fd5d8039adc588809019b0cc665302"
recommended: "A"
flip_condition: "the store override must never be bypassed"
---

# Keep capturing

## Context and problem statement

With a broken .kb/_ledger/config.json, should capture hooks keep writing to the default ledger (and say so at SessionStart) rather than skip writing until the file is fixed?

With a broken config.json, 0.6.1 hooks keep capturing into the default ledger and say so at SessionStart.

Raised by an agent (sweep) as a call and captured by Weigh-In as `wi-b950a4`. Anchor: `crates/weighin/src/env.rs:302`.

## Considered options

### A · Keep capturing (chosen)

capture into the default ledger and warn at SessionStart

- Holds if losing captures is worse than a misplaced ledger.
- If wrong: items may land outside a synced folder's chosen store.

### B · Stop capturing

skip capture until the config is fixed

- Holds if the store override must never be bypassed.
- If wrong: items raised while broken are lost.

## Evidence

- `crates/weighin/src/env.rs:302`: recorded decision

## Decision outcome

Chosen option: **Keep capturing**, the recommended option.

Decider's note: Accepted on the project thread card (Accept all).

### Consequences

- Good, because it holds if losing captures is worse than a misplaced ledger.
- Bad, because if wrong: items may land outside a synced folder's chosen store.

### Confirmation

Revisit when: the store override must never be bypassed

## Why this might be wrong

items may land outside a synced folder's chosen store

## Provenance

- Decided 2026-10-11T15:29:52Z by human:ryan (user-approved), via chat.
- Raised in session `ed0ca560-5ce5-5ac9-bb2b-9577c222272c`.
- Verbatim: "kind: call"
