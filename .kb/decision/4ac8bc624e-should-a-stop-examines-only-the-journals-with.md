---
id: "kb-decision-4ac8bc624e"
title: "Leave as residual"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Should a_stop_examines_only_the_journals_with_events_since_the_last_clean_export stay a binary test on the 4 s clock, or get a seam passing GIT_TRACE to the spawned git so it can move in process?\", facing one Stop export test still depends on the 4 s deadline, we decided for Leave as residual and neglected Build a GIT_TRACE seam now, on the premise that it keeps passing, accepting that if wrong: one flaky Windows run, then the seam."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-11-13"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-0409b31bb8"
question: "Should a_stop_examines_only_the_journals_with_events_since_the_last_clean_export stay a binary test on the 4 s clock, or get a seam passing GIT_TRACE to the spawned git so it can move in process?"
door: two-way
blast_radius: file
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-11-13T16:28:40Z"
tree_hash: "556d26c4551febf6003aae7f1f85a36910f631cc"
recommended: "A"
flip_condition: "it failing once on Windows"
---

# Leave as residual

## Context and problem statement

Should a_stop_examines_only_the_journals_with_events_since_the_last_clean_export stay a binary test on the 4 s clock, or get a seam passing GIT_TRACE to the spawned git so it can move in process?

One Stop export test still depends on the 4 s deadline.

Raised by an agent (sweep) as a deferral and captured by Weigh-In as `wi-0409b31bb8`. Anchor: `crates/weigh-in/weighin/tests/it/journal_hooks.rs:221`.

## Considered options

### A · Leave as residual (chosen)

Fix only if it flakes

- Holds if it keeps passing.
- If wrong: one flaky Windows run, then the seam.

### B · Build a GIT_TRACE seam now

Pass trace env to spawned git in tests, move it in process

- Holds if you want zero clock-bound tests.
- If wrong: a small seam nobody needed.

## Evidence

- `crates/weigh-in/weighin/tests/it/journal_hooks.rs:221`: counts git calls through GIT_TRACE

## Decision outcome

Chosen option: **Leave as residual**, the recommended option.

Decider's note: Ryan chose on the decision card, 2026-11-13: leave it, build the GIT_TRACE seam only if it flakes

### Consequences

- Good, because it holds if it keeps passing.
- Bad, because if wrong: one flaky Windows run, then the seam.

### Confirmation

Revisit when: it failing once on Windows

## Why this might be wrong

Windows runners are slow often enough to hit it

## Provenance

- Decided 2026-11-13T16:28:40Z by human:ryan (user-approved), via chat.
- Raised in session `128f4cc5-5a63-5e51-ae0f-472143607eac`.
- Verbatim: "Should a_stop_examines_only_the_journals_with_events_since_the_last_clean_export stay a binary test on the 4 s clock, or get a seam passing GIT_TRACE to the spawned git so it can move in process?"
