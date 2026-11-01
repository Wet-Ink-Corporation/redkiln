---
id: "kb-decision-wi-a3dcba"
title: "Add proptest"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"May weighin-core add proptest as a dev-dependency (shrinking, regression files), replacing\", facing proptest would replace the hand-rolled PRNG in crates/weighin-core/tests/it/support.rs with shrinking and regression files, we decided for Add proptest and neglected Stay std-only, on the premise that you want shrinking for future failures, accepting that if wrong: one more dev-dependency and a port of 22 tests for no failure seen yet."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-08"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-a3dcba"
question: "May weighin-core add proptest as a dev-dependency (shrinking, regression files), replacing"
door: two-way
blast_radius: module
decider: "human:ryan"
provenance: user-directed
decided_at: "2026-10-08T04:25:34Z"
tree_hash: "6afc146aaed963738fe8b9f357053f56fa41d07d"
recommended: "B"
flip_condition: "a property fails with a case too large to read"
---

# Add proptest

## Context and problem statement

May weighin-core add proptest as a dev-dependency (shrinking, regression files), replacing

proptest would replace the hand-rolled PRNG in crates/weighin-core/tests/it/support.rs with shrinking and regression files.

Raised by an agent (marker) as a question and captured by Weigh-In as `wi-a3dcba`. Anchor: `crates/weighin-core/Cargo.toml:1`.

## Decision drivers

- Dependencies
- Failure readability

## Considered options

### A · Add proptest (chosen)

dev-dependency of weighin-core; port the 22 properties

- Holds if you want shrinking for future failures.
- If wrong: one more dev-dependency and a port of 22 tests for no failure seen yet.

### B · Stay std-only

keep the hand-rolled generators

- Holds if the properties keep finding nothing that needs shrinking.
- If wrong: a future failure is reported unminimised and takes longer to read.

## Evidence

- `.claude/temper.json`: proptest is on the preapprovedDependencies list
- `crates/weighin-core/tests/it/support.rs`: splitmix64 with fixed seeds, case index in every assertion

## Decision outcome

Chosen option: **Add proptest**, overriding the recommendation (B).

Decider's note: add proptest

### Consequences

- Good, because it holds if you want shrinking for future failures.
- Bad, because if wrong: one more dev-dependency and a port of 22 tests for no failure seen yet.

### Confirmation

Revisit when: a property fails with a case too large to read

## Why this might be wrong

redkiln-rs may standardise on proptest, making B a later port anyway

## Provenance

- Decided 2026-10-08T04:25:34Z by human:ryan (user-directed), via chat.
- Raised in session `ca6d9b4d-821f-5563-bf7e-5c243bdbde79`.
- Verbatim: "kind: question"
