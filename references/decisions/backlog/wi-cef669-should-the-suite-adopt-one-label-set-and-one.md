---
id: "kb-decision-wi-cef669"
title: "One suite set"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Should the suite adopt one label set and one set of org issue types for every repo, replacing each repo's own labels?\", facing five repos run five overlapping label sets, and every plugin's queries depend on one meaning, we decided for One suite set and neglected Per-repo, mapped; New repos only, on the premise that repos are ours and agents share queries, accepting that if wrong: a repo needing its own labels gets a config override later."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-19"
reversibility: low
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-cef669"
question: "Should the suite adopt one label set and one set of org issue types for every repo, replacing each repo's own labels?"
door: one-way
blast_radius: external
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-19T04:27:59Z"
recommended: "A"
flip_condition: "a client repo must keep its labels"
---

# One suite set

## Context and problem statement

Should the suite adopt one label set and one set of org issue types for every repo, replacing each repo's own labels?

Five repos run five overlapping label sets, and every plugin's queries depend on one meaning.

Raised by an agent (sweep) as a question and captured by Weigh-In as `wi-cef669`. Anchor: `https://claude.ai/code/artifact/6a5d3746-3d34-4c36-8776-2206b0aaf526`.

## Decision drivers

- Migration work

## Considered options

### A · One suite set (chosen)

One labels.yml and org issue types; migrate all five repos with one manifest each

- Holds if repos are ours and agents share queries.
- If wrong: a repo needing its own labels gets a config override later.

### B · Per-repo, mapped

Each repo keeps its labels; Backlog maps them through config

- Holds if repos must keep local conventions.
- If wrong: every query and skill carries a mapping layer.

### C · New repos only

One set in the monorepo; existing repos untouched

- Holds if old repos are archived soon.
- If wrong: two conventions live side by side until migration.

## Evidence

- `redkiln-rs docs/BACKLOG.md (build/phase-0-to-1)`: area/*, source/*, status/*, needs/*, prio/*, size/* label families
- `temper .kb wi-7c3212`: P0/P1/P2 labels

## Decision outcome

Chosen option: **One suite set**, the recommended option.

### Consequences

- Good, because it holds if repos are ours and agents share queries.
- Bad, because if wrong: a repo needing its own labels gets a config override later.

### Confirmation

Revisit when: a client repo must keep its labels

## Why this might be wrong

If client repos adopt Backlog, they may refuse our labels

## Provenance

- Decided 2026-10-19T04:27:59Z by human:ryan (user-approved), via cli.
- Raised in session `740d340f-d7bd-5e81-b390-5a65c8fd6e86`.
- Verbatim: "Should the suite adopt one label set and one set of org issue types for every repo, replacing each repo's own labels?"
