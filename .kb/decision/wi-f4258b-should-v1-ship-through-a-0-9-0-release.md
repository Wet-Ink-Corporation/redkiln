---
id: "kb-decision-wi-f4258b"
title: "0.9.0 first"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Should v1 ship through a 0.9.0 release candidate with 1.0.0 adding no code, rather than straight to 1.0.0 (B-23)?\", facing a 0.9.0 candidate gives the v1 gates a release to check before 1.0.0 (B-23), we decided for 0.9.0 first and neglected Straight to 1.0.0, on the premise that your Cowork, Windows and org-sync checks run on 0.9.0, accepting that if wrong: one extra release."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-11"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-f4258b"
question: "Should v1 ship through a 0.9.0 release candidate with 1.0.0 adding no code, rather than straight to 1.0.0 (B-23)?"
door: two-way
blast_radius: system
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-11T15:29:52Z"
tree_hash: "9e3e3fc007fd5d8039adc588809019b0cc665302"
recommended: "A"
flip_condition: "the checks pass on 0.6.x already"
---

# 0.9.0 first

## Context and problem statement

Should v1 ship through a 0.9.0 release candidate with 1.0.0 adding no code, rather than straight to 1.0.0 (B-23)?

A 0.9.0 candidate gives the v1 gates a release to check before 1.0.0 (B-23).

Raised by an agent (sweep) as a call and captured by Weigh-In as `wi-f4258b`. Anchor: `docs/decisions/build-decisions.md:552`.

## Considered options

### A · 0.9.0 first (chosen)

ship 0.9.0 as the candidate; 1.0.0 adds no code

- Holds if your Cowork, Windows and org-sync checks run on 0.9.0.
- If wrong: one extra release.

### B · Straight to 1.0.0

skip the candidate

- Holds if the checks pass on 0.6.x already.
- If wrong: a gate failure lands in 1.0.x.

## Evidence

- `docs/decisions/build-decisions.md:552`: recorded decision

## Decision outcome

Chosen option: **0.9.0 first**, the recommended option.

Decider's note: Accepted on the project thread card (Accept all).

### Consequences

- Good, because it holds if your Cowork, Windows and org-sync checks run on 0.9.0.
- Bad, because if wrong: one extra release.

### Confirmation

Revisit when: the checks pass on 0.6.x already

## Why this might be wrong

one extra release

## Provenance

- Decided 2026-10-11T15:29:52Z by human:ryan (user-approved), via chat.
- Raised in session `ed0ca560-5ce5-5ac9-bb2b-9577c222272c`.
- Verbatim: "kind: call"
