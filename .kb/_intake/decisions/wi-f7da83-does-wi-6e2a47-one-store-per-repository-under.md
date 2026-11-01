---
id: "kb-decision-wi-f7da83"
title: "Record V3 as moot"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Does wi-6e2a47 (one store per repository under the git common dir) close V3, so a dated record can say so before 1.0.0?\", facing every V-item must be closed or deferred by a record before 1.0.0; V3 has no record, we decided for Record V3 as moot and neglected Run V3 anyway, on the premise that every worktree opens the same common-dir store (tested in tests/it, linked worktree), accepting that if wrong: a worktree case reads a different store and nobody checked."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-08"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-f7da83"
question: "Does wi-6e2a47 (one store per repository under the git common dir) close V3, so a dated record can say so before 1.0.0?"
door: two-way
blast_radius: external
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-08T04:25:34Z"
tree_hash: "6afc146aaed963738fe8b9f357053f56fa41d07d"
recommended: "A"
flip_condition: "the linked-worktree end-to-end test does not exercise two worktrees reading one item"
---

# Record V3 as moot

## Context and problem statement

Does wi-6e2a47 (one store per repository under the git common dir) close V3, so a dated record can say so before 1.0.0?

Every V-item must be closed or deferred by a record before 1.0.0; V3 has no record.

Raised by an agent (marker) as a question and captured by Weigh-In as `wi-f7da83`. Anchor: `docs/research/report.md:636`.

## Decision drivers

- Effort
- Evidence

## Considered options

### A · Record V3 as moot (chosen)

a B-21 entry: wi-6e2a47 makes one store per repository, so V3 needs no check

- Holds if every worktree opens the same common-dir store (tested in tests/it, linked worktree).
- If wrong: a worktree case reads a different store and nobody checked.

### B · Run V3 anyway

a manual check in two worktrees before 1.0.0

- Holds if you distrust the linked-worktree test.
- If wrong: a manual run for a property already tested.

## Evidence

- `docs/research/report.md:561`: V3: whether a worktree session can read sibling worktrees .kb/_ledger
- `docs/decisions/wi-6e2a47.md:58`: Open items live in <git-common-dir>/weighin/, shared by every worktree

## Decision outcome

Chosen option: **Record V3 as moot**, the recommended option.

Decider's note: record V3 as moot

### Consequences

- Good, because it holds if every worktree opens the same common-dir store (tested in tests/it, linked worktree).
- Bad, because if wrong: a worktree case reads a different store and nobody checked.

### Confirmation

Revisit when: the linked-worktree end-to-end test does not exercise two worktrees reading one item

## Why this might be wrong

the test may cover raise in a worktree but not reading from a sibling

## Provenance

- Decided 2026-10-08T04:25:34Z by human:ryan (user-approved), via chat.
- Raised in session `ca6d9b4d-821f-5563-bf7e-5c243bdbde79`.
- Verbatim: "kind: question"
