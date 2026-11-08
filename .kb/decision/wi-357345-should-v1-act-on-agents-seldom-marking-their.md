---
id: "kb-decision-wi-357345"
title: "Sharpen the convention text"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Should v1 act on agents seldom marking their own calls (1 block in 51 turns since 0.5.0; seed recall 1 of 7), and how?\", facing six of seven calls in the seed were written as prose ('Decisions for you:', 'Your calls on the PR:') with no block, so the ledger never saw them, we decided for Sharpen the convention text and neglected Leave it; the sweep catches them; Capture the prose too, on the premise that agents follow an explicit instruction better than an implied one, accepting that if wrong: one release spent; recall stays low."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-04"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-357345"
question: "Should v1 act on agents seldom marking their own calls (1 block in 51 turns since 0.5.0; seed recall 1 of 7), and how?"
door: two-way
blast_radius: system
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-04T15:09:56Z"
tree_hash: "dc3cc0b7f1052c6b640c125db4ad4989a0477183"
recommended: "B"
flip_condition: "weekly stats show the sweep already catches the misses"
---

# Sharpen the convention text

## Context and problem statement

Should v1 act on agents seldom marking their own calls (1 block in 51 turns since 0.5.0; seed recall 1 of 7), and how?

Six of seven calls in the seed were written as prose ('Decisions for you:', 'Your calls on the PR:') with no block, so the ledger never saw them.

Raised by an agent (sweep) as a question and captured by Weigh-In as `wi-357345`. Anchor: `crates/weighin/src/hooks.rs:192`.

## Decision drivers

- Size
- Risk to precision

## Considered options

### A · Leave it; the sweep catches them

No change in v1; the sitting's sweep raises what markers miss. Already recorded as a gap.

- Holds if you run /weigh-in at every session's end.
- If wrong: calls in sessions without a sitting are lost.

### B · Sharpen the convention text (chosen)

Add one line to the SessionStart convention: a list of calls for the user ('Decisions for you') gets one block per call. Ships in 0.5.2.

- Holds if agents follow an explicit instruction better than an implied one.
- If wrong: one release spent; recall stays low.

### C · Capture the prose too

A Stop-hook rule raises items from headings like 'Decisions for you:' with no block.

- Holds if those headings are reliable.
- If wrong: new noise lowers precision; a new capture channel to test.

## Evidence

- `crates/weighin/src/hooks.rs:192`: When you assume, defer, make a call the user did not examine, or cannot decide, end your message with:
- `eval/seed.jsonl rs6-01`: Decisions for you:
- **Version bump:** I didn't bump the version

## Decision outcome

Chosen option: **Sharpen the convention text**, the recommended option.

Decider's note: Sharpen convention text

### Consequences

- Good, because it holds if agents follow an explicit instruction better than an implied one.
- Bad, because if wrong: one release spent; recall stays low.

### Confirmation

Revisit when: weekly stats show the sweep already catches the misses

## Why this might be wrong

agents may already read the text and still not write blocks

## Provenance

- Decided 2026-10-04T15:09:56Z by human:ryan (user-approved), via chat.
- Raised in session `6fda78c2-2ead-56e4-8e03-7d278eddd909`.
- Verbatim: "Should v1 act on agents seldom marking their own calls (1 block in 51 turns since 0.5.0; seed recall 1 of 7), and how?"
