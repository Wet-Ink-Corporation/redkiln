---
id: "kb-decision-wi-64c8ea"
title: "Blocked prefix keeps it raised"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Should capture keep an item raised, not defaulted, when its block's did starts with \"blocked\" followed by more text (as in \"blocked; left the guard as it is\")?\", facing capture logs a block as a default when did starts with \"blocked; …\" instead of exactly \"blocked\", so questions the agent could not act on are filed as calls it made, we decided for Blocked prefix keeps it raised and neglected Exact \"blocked\" only, on the premise that agents explain what they did instead after \"blocked\", accepting that if wrong: an agent writing \"blocked\" for something it did act on leaves an extra open item."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-08"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-64c8ea"
question: "Should capture keep an item raised, not defaulted, when its block's did starts with \"blocked\" followed by more text (as in \"blocked; left the guard as it is\")?"
door: two-way
blast_radius: system
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-08T04:57:19Z"
tree_hash: "32f5d5b7eb426f8b2803b71234d641befe5df595"
recommended: "A"
flip_condition: "a seed item shows \"blocked …\" written for a call the agent did make"
---

# Blocked prefix keeps it raised

## Context and problem statement

Should capture keep an item raised, not defaulted, when its block's did starts with "blocked" followed by more text (as in "blocked; left the guard as it is")?

Capture logs a block as a default when did starts with "blocked; …" instead of exactly "blocked", so questions the agent could not act on are filed as calls it made.

Raised by an agent (marker) as a question and captured by Weigh-In as `wi-64c8ea`. Anchor: `crates/weighin-core/src/marker.rs:1`.

## Decision drivers

- Questions lost to defaults
- Change

## Considered options

### A · Blocked prefix keeps it raised (chosen)

did that starts with the word blocked (any punctuation after) keeps the item raised; add the seed cases as tests

- Holds if agents explain what they did instead after "blocked".
- If wrong: an agent writing "blocked" for something it did act on leaves an extra open item.

### B · Exact "blocked" only

keep today: only did: blocked keeps it raised; tell the convention to write nothing after it

- Holds if agents can be trained to write the bare word.
- If wrong: questions keep landing as defaults, unseen.

## Evidence

- `eval/seed.jsonl:70`: rs12-10: 21 blocks, 9 with did: blocked; …
- `ledger 2026-10-08`: 8 of those 9 were logged as defaulted, 1 stayed raised

## Decision outcome

Chosen option: **Blocked prefix keeps it raised**, the recommended option.

Decider's note: blocked prefix stays open

### Consequences

- Good, because it holds if agents explain what they did instead after "blocked".
- Bad, because if wrong: an agent writing "blocked" for something it did act on leaves an extra open item.

### Confirmation

Revisit when: a seed item shows "blocked …" written for a call the agent did make

## Why this might be wrong

"blocked" may appear as a verb in a did that describes real action

## Provenance

- Decided 2026-10-08T04:57:19Z by human:ryan (user-approved), via chat.
- Raised in session `ca6d9b4d-821f-5563-bf7e-5c243bdbde79`.
- Verbatim: "kind: question"
