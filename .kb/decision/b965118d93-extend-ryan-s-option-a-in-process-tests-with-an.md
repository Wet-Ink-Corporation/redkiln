---
id: "kb-decision-b965118d93"
title: "Confirm"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Extend Ryan's option A (in-process tests with an injected budget) from the SessionStart import tests to the Stop hook's export tests, as #92 did?\", facing a test rewrite is an H-16 call, and #92 merged under the SessionStart approval, we decided for Confirm and neglected Revert to binary tests, on the premise that the same reasoning as SessionStart applies, accepting that if wrong: a small PR moves them back."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-11-13"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-5394130514"
question: "Extend Ryan's option A (in-process tests with an injected budget) from the SessionStart import tests to the Stop hook's export tests, as #92 did?"
door: two-way
blast_radius: module
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-11-13T16:28:34Z"
tree_hash: "556d26c4551febf6003aae7f1f85a36910f631cc"
recommended: "A"
flip_condition: "wanting binary-level export coverage"
---

# Confirm

## Context and problem statement

Extend Ryan's option A (in-process tests with an injected budget) from the SessionStart import tests to the Stop hook's export tests, as #92 did?

A test rewrite is an H-16 call, and #92 merged under the SessionStart approval.

Raised by an agent (sweep) as a call and captured by Weigh-In as `wi-5394130514`. Anchor: `docs/decisions.md:124`.

## Considered options

### A · Confirm (chosen)

Keep the in-process Stop tests

- Holds if the same reasoning as SessionStart applies.
- If wrong: a small PR moves them back.

### B · Revert to binary tests

Use a test-only budget variable instead

- Holds if you want end-to-end binary coverage.
- If wrong: Windows flakes return.

## Evidence

- `docs/decisions.md:124`: agent, extending Ryan's option A (2026-11-12); owner to confirm

## Decision outcome

Chosen option: **Confirm**, the recommended option.

Decider's note: Ryan confirmed on the decision card, 2026-11-13

### Consequences

- Good, because it holds if the same reasoning as SessionStart applies.
- Bad, because if wrong: a small PR moves them back.

### Confirmation

Revisit when: wanting binary-level export coverage

## Why this might be wrong

the in-process path could diverge from the binary's

## Provenance

- Decided 2026-11-13T16:28:34Z by human:ryan (user-approved), via chat.
- Raised in session `128f4cc5-5a63-5e51-ae0f-472143607eac`.
- Verbatim: "Extend Ryan's option A (in-process tests with an injected budget) from the SessionStart import tests to the Stop hook's export tests, as #92 did?"
