---
id: "kb-decision-wi-48b36e"
title: "Close V3"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Is V3 (sibling worktrees' ledgers) closed as moot under wi-6e2a47, citing the worktree end-to-end test (B-24)?\", facing worktrees share the common-dir ledger (wi-6e2a47), and the worktree end-to-end test covers it, so V3 is moot (B-24), we decided for Close V3 and neglected Keep V3 open, on the premise that every worktree shares the common-dir store, accepting that if wrong: a setup without a shared store loses items."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-11"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-48b36e"
question: "Is V3 (sibling worktrees' ledgers) closed as moot under wi-6e2a47, citing the worktree end-to-end test (B-24)?"
door: two-way
blast_radius: file
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-11T15:29:52Z"
tree_hash: "9e3e3fc007fd5d8039adc588809019b0cc665302"
recommended: "A"
flip_condition: "you use worktrees that do not share .git"
---

# Close V3

## Context and problem statement

Is V3 (sibling worktrees' ledgers) closed as moot under wi-6e2a47, citing the worktree end-to-end test (B-24)?

Worktrees share the common-dir ledger (wi-6e2a47), and the worktree end-to-end test covers it, so V3 is moot (B-24).

Raised by an agent (sweep) as a call and captured by Weigh-In as `wi-48b36e`. Anchor: `docs/decisions/build-decisions.md:570`.

## Considered options

### A · Close V3 (chosen)

close V3 as moot, citing the worktree test

- Holds if every worktree shares the common-dir store.
- If wrong: a setup without a shared store loses items.

### B · Keep V3 open

keep checking sibling ledgers by hand

- Holds if you use worktrees that do not share .git.
- If wrong: an open gate with nothing left to check.

## Evidence

- `docs/decisions/build-decisions.md:570`: recorded decision

## Decision outcome

Chosen option: **Close V3**, the recommended option.

Decider's note: Accepted on the project thread card (Accept all).

### Consequences

- Good, because it holds if every worktree shares the common-dir store.
- Bad, because if wrong: a setup without a shared store loses items.

### Confirmation

Revisit when: you use worktrees that do not share .git

## Why this might be wrong

a setup without a shared store loses items

## Provenance

- Decided 2026-10-11T15:29:52Z by human:ryan (user-approved), via chat.
- Raised in session `ed0ca560-5ce5-5ac9-bb2b-9577c222272c`.
- Verbatim: "kind: call"
