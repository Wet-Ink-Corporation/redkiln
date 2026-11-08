---
id: "kb-decision-wi-c34fb3"
title: "Cap defaults"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"How should the board keep its state document under the db store's 256 KiB cap, given defaults are unbounded (about 700 overflow it)?\", facing board export writes every open default into one db document, capped at 256 KiB, so about 700 defaults stop the board syncing, we decided for Cap defaults and neglected One doc per default; Wait for V5, on the premise that nobody reviews more than 200 defaults on the board, accepting that if wrong: older defaults are reachable only through /weigh-in digest."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-03"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-c34fb3"
question: "How should the board keep its state document under the db store's 256 KiB cap, given defaults are unbounded (about 700 overflow it)?"
door: two-way
blast_radius: external
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-03T17:55:35Z"
tree_hash: "2edd9586052a96bb12d42c99557151b2ec9a838f"
recommended: "A"
flip_condition: "you review old defaults on the board, not in the digest"
---

# Cap defaults

## Context and problem statement

How should the board keep its state document under the db store's 256 KiB cap, given defaults are unbounded (about 700 overflow it)?

board export writes every open default into one db document, capped at 256 KiB, so about 700 defaults stop the board syncing.

Raised by an agent (sweep) as a deferral and captured by Weigh-In as `wi-c34fb3`. Anchor: `crates/weighin-core/src/board.rs:196`.

## Decision drivers

- Work
- Defaults on the board

## Considered options

### A · Cap defaults (chosen)

Export the newest 200 unacknowledged defaults plus a total count; the digest covers the rest.

- Holds if nobody reviews more than 200 defaults on the board.
- If wrong: older defaults are reachable only through /weigh-in digest.

### B · One doc per default

Write defaults as their own documents in a collection the board reads.

- Holds if the board must show every default.
- If wrong: more ArtifactData writes per sync, and per-collection count caps instead.

### C · Wait for V5

Change nothing until the V5 run confirms the limit.

- Holds if the documented limit turns out not to apply.
- If wrong: the board fails at about 700 defaults, silently from your side.

## Evidence

- `crates/weighin-core/src/board.rs:196`: let defaults = ledger.items.values().filter(|i| i.status == Status::Defaulted)
- `issue #19`: 1,000 defaults export to a 369 KB board-state.json
- `artifact runtime db.d.ts:137`: a document over 256 KiB ... invalid_argument

## Decision outcome

Chosen option: **Cap defaults**, the recommended option.

### Consequences

- Good, because it holds if nobody reviews more than 200 defaults on the board.
- Bad, because if wrong: older defaults are reachable only through /weigh-in digest.

### Confirmation

Revisit when: you review old defaults on the board, not in the digest

## Why this might be wrong

if you do browse old defaults on the board, the cap hides them

## Provenance

- Decided 2026-10-03T17:55:35Z by human:ryan (user-approved), via chat.
- Raised in session `c3033c21-fb39-5c9f-99d0-12436b203ca1`.
- Verbatim: "How should the board keep its state document under the db store's 256 KiB cap, given defaults are unbounded (about 700 overflow it)?"
