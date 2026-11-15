---
id: "kb-decision-293df74320"
title: "Approve all six"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Approve the six test moves and assertion changes in redkiln PR #105 (Windows hook timing flakes, M2-J option A), so it can merge?\", facing h-12 makes moved tests and changed assertions Ryan's decision, and #105 holds six, we decided for Approve all six and neglected Approve five, Core fix for appends; Reject; raise budgets instead, on the premise that pinned behaviour unchanged, only timing removed, accepting that if wrong: a moved test misses a binary-level regression; revert one test."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-11-15"
reversibility: low
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-3181d055cf"
question: "Approve the six test moves and assertion changes in redkiln PR #105 (Windows hook timing flakes, M2-J option A), so it can merge?"
door: one-way
blast_radius: module
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-11-15T06:47:04Z"
tree_hash: "1f469fde413eea288929c8153a10cc6792cd93d9"
recommended: "A"
flip_condition: "you want concurrent first creation pinned through 20 processes"
---

# Approve all six

## Context and problem statement

Approve the six test moves and assertion changes in redkiln PR #105 (Windows hook timing flakes, M2-J option A), so it can merge?

H-12 makes moved tests and changed assertions Ryan's decision, and #105 holds six.

Raised by an agent (sweep) as a question and captured by Weigh-In as `wi-3181d055cf`. Anchor: `.temper/plans/windows-hook-timing-flakes.md:7`.

## Decision drivers

- Production change

## Considered options

### A · Approve all six (chosen)

Merge #105 as built

- Holds if pinned behaviour unchanged, only timing removed.
- If wrong: a moved test misses a binary-level regression; revert one test.

### B · Approve five, Core fix for appends

Keep twenty first-opens; Core waiters re-check a settled store and open without the lock

- Holds if you want concurrent first creation pinned at process level.
- If wrong: Core behaviour change, hard to test deterministically; more days.

### C · Reject; raise budgets instead

Keep tests as they were, lengthen production budgets

- Holds if hook deadlines are too tight in production too.
- If wrong: slower hooks for every user; contradicts M2-J A.

## Evidence

- `.temper/plans/windows-hook-timing-flakes.md:7`: Six changes: weighin 1.x import gets the hook budget seam; two tests move in process; e2e measures the prime; bench twenty-appends pre-creates the store; Core telemetry bound uses Wait::Command
- `.temper/plans/windows-hook-timing-flakes.md:18`: 20 first opens serialize on Core's open lock past the 10 s wait on Windows (13 of 20 timed out)

## Decision outcome

Chosen option: **Approve all six**, the recommended option.

Decider's note: chosen on the decision card, 2026-11-15

### Consequences

- Good, because it holds if pinned behaviour unchanged, only timing removed.
- Bad, because if wrong: a moved test misses a binary-level regression; revert one test.

### Confirmation

Revisit when: you want concurrent first creation pinned through 20 processes

## Why this might be wrong

Core's concurrent-creation test may not cover what 20 real processes do

## Provenance

- Decided 2026-11-15T06:47:04Z by human:ryan (user-approved), via chat.
- Raised in session `72cd9cff-beae-5dc7-8553-d2e008d540f3`.
- Verbatim: "Approve the six test moves and assertion changes in redkiln PR #105 (Windows hook timing flakes, M2-J option A), so it can merge?"
