---
id: "kb-decision-wi-b4c07d"
title: "Keep committed journals"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Should cloud sessions commit a per-session event journal under .kb/_ledger/journal/ so stats survive the container (B-21), rather than counting decided atoms only?\", facing cloud sittings lost all evidence except decided atoms, so #23's action and not-useful rates were unmeasurable here, we decided for Keep committed journals and neglected Atoms only, on the premise that event text in the repo is acceptable, as atoms already are, accepting that if wrong: revert #104's hook wiring; journals stay in history."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-09"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-b4c07d"
question: "Should cloud sessions commit a per-session event journal under .kb/_ledger/journal/ so stats survive the container (B-21), rather than counting decided atoms only?"
door: two-way
blast_radius: system
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-09T04:52:38Z"
tree_hash: "ef4f1136eaf287a5afb6c08a75db1348d2620acc"
recommended: "A"
flip_condition: "you would rather not commit ledger event text"
---

# Keep committed journals

## Context and problem statement

Should cloud sessions commit a per-session event journal under .kb/_ledger/journal/ so stats survive the container (B-21), rather than counting decided atoms only?

Cloud sittings lost all evidence except decided atoms, so #23's action and not-useful rates were unmeasurable here.

Raised by an agent (marker) as a call and captured by Weigh-In as `wi-b4c07d`. Anchor: `docs/decisions/build-decisions.md:516`.

## Decision drivers

- Pilot evidence
- Repo churn

## Considered options

### A · Keep committed journals (chosen)

As merged in #104/#105

- Holds if event text in the repo is acceptable, as atoms already are.
- If wrong: revert #104's hook wiring; journals stay in history.

### B · Atoms only

Revert to D-0014 as it was

- Holds if the pilot can be judged on decided items alone.
- If wrong: not-useful rate unmeasurable; #25 cannot be judged.

## Evidence

- `docs/decisions/build-decisions.md:516`: B-21: cloud sessions commit per-session event journals
- `.kb/_ledger/journal/a30b6003-c528-5711-b5a2-08effa8e852d.jsonl:1`: first committed journal, 8 lines

## Decision outcome

Chosen option: **Keep committed journals**, the recommended option.

Decider's note: keep committed journals

### Consequences

- Good, because it holds if event text in the repo is acceptable, as atoms already are.
- Bad, because if wrong: revert #104's hook wiring; journals stay in history.

### Confirmation

Revisit when: you would rather not commit ledger event text

## Why this might be wrong

if ledger text is ever sensitive, it is now in git history

## Provenance

- Decided 2026-10-09T04:52:38Z by human:ryan (user-approved), via chat.
- Raised in session `a30b6003-c528-5711-b5a2-08effa8e852d`.
- Verbatim: "kind: call"
