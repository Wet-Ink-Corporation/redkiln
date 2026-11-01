---
id: "kb-decision-wi-a01b14"
title: "Return empty"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"May clip(s, 0) return an empty string instead of an ellipsis, changing the two pinned test values (H-12, PR #121 commit 1)?\", facing clip(s, 0) returns one character, breaking its 'at most n characters' contract; fixing it changes two pinned test values (H-12), we decided for Return empty and neglected Keep the ellipsis, on the premise that no caller relies on the ellipsis at n = 0, accepting that if wrong: none known; no caller passes 0."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-11"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-a01b14"
question: "May clip(s, 0) return an empty string instead of an ellipsis, changing the two pinned test values (H-12, PR #121 commit 1)?"
door: two-way
blast_radius: module
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-11T14:22:17Z"
tree_hash: "3ecee07cdae74527d16a1007e82348a986ee50fd"
recommended: "A"
flip_condition: "the pinned values must not change"
---

# Return empty

## Context and problem statement

May clip(s, 0) return an empty string instead of an ellipsis, changing the two pinned test values (H-12, PR #121 commit 1)?

clip(s, 0) returns one character, breaking its 'at most n characters' contract; fixing it changes two pinned test values (H-12).

Raised by an agent (manual) as a question and captured by Weigh-In as `wi-a01b14`. Anchor: `crates/weighin/src/text.rs:5`.

## Considered options

### A · Return empty (chosen)

clip(s, 0) returns ""; update the two pinned values

- Holds if no caller relies on the ellipsis at n = 0.
- If wrong: none known; no caller passes 0.

### B · Keep the ellipsis

leave clip as is

- Holds if the pinned values must not change.
- If wrong: clip keeps breaking its documented limit at n = 0.

## Evidence

- `crates/weighin/src/text.rs:5`: changed on PR #121

## Decision outcome

Chosen option: **Return empty**, the recommended option.

Decider's note: Approved on the decision card in the project thread (Merge all).

### Consequences

- Good, because it holds if no caller relies on the ellipsis at n = 0.
- Bad, because if wrong: none known; no caller passes 0.

### Confirmation

Revisit when: the pinned values must not change

## Why this might be wrong

none known; no caller passes 0

## Provenance

- Decided 2026-10-11T14:22:17Z by human:ryan (user-approved), via chat.
- Raised in session `ed0ca560-5ce5-5ac9-bb2b-9577c222272c`.
- Verbatim: "May clip(s, 0) return an empty string instead of an ellipsis, changing the two pinned test values (H-12, PR #121 commit 1)?"
