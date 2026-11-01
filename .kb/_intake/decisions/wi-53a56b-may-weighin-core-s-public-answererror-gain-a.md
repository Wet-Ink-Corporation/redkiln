---
id: "kb-decision-wi-53a56b"
title: "Add NoDecider"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"May weighin-core's public AnswerError gain a NoDecider variant so a not-my-call answer without a decider reports that, instead of a bad dismiss reason?\", facing a not-my-call answer with no decider is reported as BadReason(\"not-my-call without a decider\"), whose message talks about dismiss reasons, we decided for Add NoDecider and neglected Reword BadReason, on the premise that weighin-core is unpublished and redkiln-rs has not adopted it, accepting that if wrong: a downstream exhaustive match breaks."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-08"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-53a56b"
question: "May weighin-core's public AnswerError gain a NoDecider variant so a not-my-call answer without a decider reports that, instead of a bad dismiss reason?"
door: two-way
blast_radius: module
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-08T04:57:19Z"
tree_hash: "32f5d5b7eb426f8b2803b71234d641befe5df595"
recommended: "A"
flip_condition: "redkiln-rs adopts weighin-core before 1.0.0"
---

# Add NoDecider

## Context and problem statement

May weighin-core's public AnswerError gain a NoDecider variant so a not-my-call answer without a decider reports that, instead of a bad dismiss reason?

A not-my-call answer with no decider is reported as BadReason("not-my-call without a decider"), whose message talks about dismiss reasons.

Raised by an agent (marker) as a question and captured by Weigh-In as `wi-53a56b`. Anchor: `crates/weighin-core/src/board.rs:673`.

## Decision drivers

- API change
- Clear message

## Considered options

### A · Add NoDecider (chosen)

a new AnswerError::NoDecider variant with its own message; update the one test expectation

- Holds if weighin-core is unpublished and redkiln-rs has not adopted it.
- If wrong: a downstream exhaustive match breaks.

### B · Reword BadReason

keep the enum; make BadReason's message neutral ("bad dismiss reason or hand-off")

- Holds if you want no API change before 1.0.0.
- If wrong: callers still cannot tell the two cases apart.

## Evidence

- `crates/weighin-core/src/board.rs:673`: return Err(AnswerError::BadReason("not-my-call without a decider".into()))
- `crates/weighin-core/src/board.rs:497`: pub enum AnswerError (exhaustive, crate unpublished)

## Decision outcome

Chosen option: **Add NoDecider**, the recommended option.

Decider's note: add NoDecider

### Consequences

- Good, because it holds if weighin-core is unpublished and redkiln-rs has not adopted it.
- Bad, because if wrong: a downstream exhaustive match breaks.

### Confirmation

Revisit when: redkiln-rs adopts weighin-core before 1.0.0

## Why this might be wrong

1.0.0 may be the moment to freeze the enum

## Provenance

- Decided 2026-10-08T04:57:19Z by human:ryan (user-approved), via chat.
- Raised in session `ca6d9b4d-821f-5563-bf7e-5c243bdbde79`.
- Verbatim: "kind: question"
