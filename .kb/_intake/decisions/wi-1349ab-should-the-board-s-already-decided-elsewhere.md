---
id: "kb-decision-wi-1349ab"
title: "Trust board answers"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Should the board's \"already decided elsewhere\" answer count as your own words and pass the papering-over guard on authority-word items, or should the board hide that option on such cards?\", facing your own board answer \"already decided elsewhere\" is refused by the papering-over guard on any item whose text contains an authority word, and board apply exits 0 with only a ✗ line, we decided for Trust board answers and neglected Hide the option; Keep as is, on the premise that only the owner can write board answers (the db write rule is admin), accepting that if wrong: a mis-click on the board settles an authority item without its record."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-08"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-1349ab"
question: "Should the board's \"already decided elsewhere\" answer count as your own words and pass the papering-over guard on authority-word items, or should the board hide that option on such cards?"
door: two-way
blast_radius: module
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-08T04:57:19Z"
tree_hash: "32f5d5b7eb426f8b2803b71234d641befe5df595"
recommended: "A"
flip_condition: "board answers can be written by someone other than the owner"
---

# Trust board answers

## Context and problem statement

Should the board's "already decided elsewhere" answer count as your own words and pass the papering-over guard on authority-word items, or should the board hide that option on such cards?

Your own board answer "already decided elsewhere" is refused by the papering-over guard on any item whose text contains an authority word, and board apply exits 0 with only a ✗ line.

Raised by an agent (marker) as a question and captured by Weigh-In as `wi-1349ab`. Anchor: `crates/weighin/src/main.rs:1047`.

## Decision drivers

- Owner answer applied

## Considered options

### A · Trust board answers (chosen)

a board answer counts as the owner's words: record it as already-decided with the board note, past the guard

- Holds if only the owner can write board answers (the db write rule is admin).
- If wrong: a mis-click on the board settles an authority item without its record.

### B · Hide the option

the board does not offer "already decided elsewhere" on authority-bound cards

- Holds if authority items should always get an explicit decision.
- If wrong: you must decide again something already decided elsewhere.

### C · Keep as is

leave the refusal; board apply keeps printing ✗

- Holds if it never happens in practice.
- If wrong: your answer is silently lost on each apply.

## Evidence

- `crates/weighin/src/main.rs:1047`: already-decided:board is refused for authority-sounding items
- `#26 finding 7`: reproduced: "What retention period for audit logs?" → ✗ … authority-bound, applied: none

## Decision outcome

Chosen option: **Trust board answers**, the recommended option.

Decider's note: trust board answers

### Consequences

- Good, because it holds if only the owner can write board answers (the db write rule is admin).
- Bad, because if wrong: a mis-click on the board settles an authority item without its record.

### Confirmation

Revisit when: board answers can be written by someone other than the owner

## Why this might be wrong

the guard may be there to catch your own reflexive clicks too

## Provenance

- Decided 2026-10-08T04:57:19Z by human:ryan (user-approved), via chat.
- Raised in session `ca6d9b4d-821f-5563-bf7e-5c243bdbde79`.
- Verbatim: "kind: question"
