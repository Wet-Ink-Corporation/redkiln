---
id: "kb-decision-wi-fa1571"
title: "Require init"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Should `weighin board export` refuse in a folder with no `.kb/_ledger/config.json` until `init --class own|client` is run, instead of defaulting to the artifact board (B-25)?\", facing a folder never initialised used to publish a board as if it were your own repo; 0.6.1 refuses until init (B-25), we decided for Require init and neglected Default to own, on the premise that one init per own repo is acceptable, accepting that if wrong: own repos need one init before the board works."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-11"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-fa1571"
question: "Should `weighin board export` refuse in a folder with no `.kb/_ledger/config.json` until `init --class own|client` is run, instead of defaulting to the artifact board (B-25)?"
door: two-way
blast_radius: module
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-11T15:29:52Z"
tree_hash: "9e3e3fc007fd5d8039adc588809019b0cc665302"
recommended: "A"
flip_condition: "client repos are never left uninitialised"
---

# Require init

## Context and problem statement

Should `weighin board export` refuse in a folder with no `.kb/_ledger/config.json` until `init --class own|client` is run, instead of defaulting to the artifact board (B-25)?

A folder never initialised used to publish a board as if it were your own repo; 0.6.1 refuses until init (B-25).

Raised by an agent (sweep) as a call and captured by Weigh-In as `wi-fa1571`. Anchor: `crates/weighin/src/main.rs:915`.

## Considered options

### A · Require init (chosen)

export refuses without config.json, as shipped in 0.6.1

- Holds if one init per own repo is acceptable.
- If wrong: own repos need one init before the board works.

### B · Default to own

export with no config counts as own and publishes

- Holds if client repos are never left uninitialised.
- If wrong: a client repo can publish a board.

## Evidence

- `crates/weighin/src/main.rs:915`: recorded decision

## Decision outcome

Chosen option: **Require init**, the recommended option.

Decider's note: Accepted on the project thread card (Accept all).

### Consequences

- Good, because it holds if one init per own repo is acceptable.
- Bad, because if wrong: own repos need one init before the board works.

### Confirmation

Revisit when: client repos are never left uninitialised

## Why this might be wrong

own repos need one init before the board works

## Provenance

- Decided 2026-10-11T15:29:52Z by human:ryan (user-approved), via chat.
- Raised in session `ed0ca560-5ce5-5ac9-bb2b-9577c222272c`.
- Verbatim: "kind: call"
