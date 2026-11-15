---
id: "kb-decision-9e9cf2e77a"
title: "Keep session, cite verdicts on PR"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"How should the rest of Move 2 step 5 meet the review bar while temper's Stop hook cannot record reviewer verdicts in this session (its base 8f2bd9c predates #99-#111, so it wants merged files in the reviewer's files_read)?\", facing temper's Stop hook demands a recorded reviewer verdict, but the honest reviewer cannot list files from already-merged PRs it never read, so no verdict records in this session, we decided for Keep session, cite verdicts on PR and neglected Fresh session per PR stretch; Fix temper's evidence scoping, on the premise that the PR citation is the merge bar you care about, accepting that if wrong: the hook keeps nagging each turn; nothing records in temper's cache."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-11-15"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-f464f850d4"
question: "How should the rest of Move 2 step 5 meet the review bar while temper's Stop hook cannot record reviewer verdicts in this session (its base 8f2bd9c predates #99-#111, so it wants merged files in the reviewer's files_read)?"
door: two-way
blast_radius: system
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-11-15T17:47:47Z"
tree_hash: "44b48638ac6e120cd1c3bc4197d3a3c41ff5099e"
recommended: "A"
flip_condition: "the hook blocking tool calls or merges, not only Stop"
---

# Keep session, cite verdicts on PR

## Context and problem statement

How should the rest of Move 2 step 5 meet the review bar while temper's Stop hook cannot record reviewer verdicts in this session (its base 8f2bd9c predates #99-#111, so it wants merged files in the reviewer's files_read)?

temper's Stop hook demands a recorded reviewer verdict, but the honest reviewer cannot list files from already-merged PRs it never read, so no verdict records in this session.

Raised by an agent (sweep) as a question and captured by Weigh-In as `wi-f464f850d4`. Anchor: `.claude/temper/temper.mjs`.

## Decision drivers

- Config change
- Cost

## Considered options

### A · Keep session, cite verdicts on PR (chosen)

Status quo: reviewer per PR, merge on an unpadded APPROVED verdict cited on the PR; turns end Status: WIP.

- Holds if the PR citation is the merge bar you care about.
- If wrong: the hook keeps nagging each turn; nothing records in temper's cache.

### B · Fresh session per PR stretch

Start a new thread on current main so the hook base is fresh; memory carries the context.

- Holds if base staleness is the only cause.
- If wrong: the base goes stale again after the next merge; lost context costs time.

### C · Fix temper's evidence scoping

Scope the hook to the diff since merge-base with main; a temper change (frozen until Rust 2.0) you would approve.

- Holds if you want it fixed now rather than at Rust 2.0.
- If wrong: edits a frozen plugin and the gate everyone runs.

## Evidence

- `Stop hook`: current tree has no approving independent review (PROC-08) ... diff since 8f2bd9cdd16a9daeb0da942c354cf35cd9ebb12a
- `temper AGENTS.md, Unattended sessions`: an APPROVED verdict from a fresh-context reviewer, cited on the PR

## Decision outcome

Chosen option: **Keep session, cite verdicts on PR**, the recommended option.

Decider's note: Ryan, decision card 2026-11-15: keep this session; merge on an unpadded APPROVED fresh-context verdict cited on the PR; end turns Status: WIP

### Consequences

- Good, because it holds if the PR citation is the merge bar you care about.
- Bad, because if wrong: the hook keeps nagging each turn; nothing records in temper's cache.

### Confirmation

Revisit when: the hook blocking tool calls or merges, not only Stop

## Why this might be wrong

if you treat temper's recorded verdict, not the PR citation, as the bar

## Provenance

- Decided 2026-11-15T17:47:47Z by human:ryan (user-approved), via cli.
- Raised in session `22af9544-95b0-5794-a45f-15714e38e099`.
- Verbatim: "How should the rest of Move 2 step 5 meet the review bar while temper's Stop hook cannot record reviewer verdicts in this session (its base 8f2bd9c predates #99-#111, so it wants merged files in the reviewer's files_read)?"
