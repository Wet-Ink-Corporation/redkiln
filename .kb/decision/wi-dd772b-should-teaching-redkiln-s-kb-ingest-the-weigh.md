---
id: "kb-decision-wi-dd772b"
title: "Slice 3"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Should teaching Redkiln's kb-ingest the Weigh-In intake keys (wi-e15c77's second half) move to slice 3, as B-22 records?\", facing teaching Redkiln's kb-ingest the intake keys is redkiln-rs work outside this repository's v1 scope, we decided for Slice 3 and neglected Ship in v1, on the premise that v1 only needs atoms written to .kb/_intake, accepting that if wrong: atoms wait in _intake until slice 3."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-11"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-dd772b"
question: "Should teaching Redkiln's kb-ingest the Weigh-In intake keys (wi-e15c77's second half) move to slice 3, as B-22 records?"
door: two-way
blast_radius: module
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-11T15:29:52Z"
tree_hash: "9e3e3fc007fd5d8039adc588809019b0cc665302"
recommended: "A"
flip_condition: "you want atoms promoted automatically at v1"
---

# Slice 3

## Context and problem statement

Should teaching Redkiln's kb-ingest the Weigh-In intake keys (wi-e15c77's second half) move to slice 3, as B-22 records?

Teaching Redkiln's kb-ingest the intake keys is redkiln-rs work outside this repository's v1 scope.

Raised by an agent (sweep) as a call and captured by Weigh-In as `wi-dd772b`. Anchor: `docs/decisions/build-decisions.md:535`.

## Considered options

### A · Slice 3 (chosen)

move kb-ingest support to slice 3, as B-22 records

- Holds if v1 only needs atoms written to .kb/_intake.
- If wrong: atoms wait in _intake until slice 3.

### B · Ship in v1

teach kb-ingest the keys before 1.0.0

- Holds if you want atoms promoted automatically at v1.
- If wrong: 1.0.0 waits on redkiln-rs changes.

## Evidence

- `docs/decisions/build-decisions.md:535`: recorded decision

## Decision outcome

Chosen option: **Slice 3**, the recommended option.

Decider's note: Accepted on the project thread card (Accept all).

### Consequences

- Good, because it holds if v1 only needs atoms written to .kb/_intake.
- Bad, because if wrong: atoms wait in _intake until slice 3.

### Confirmation

Revisit when: you want atoms promoted automatically at v1

## Why this might be wrong

atoms wait in _intake until slice 3

## Provenance

- Decided 2026-10-11T15:29:52Z by human:ryan (user-approved), via chat.
- Raised in session `ed0ca560-5ce5-5ac9-bb2b-9577c222272c`.
- Verbatim: "kind: call"
