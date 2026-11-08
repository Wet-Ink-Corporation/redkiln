---
id: "kb-decision-wi-48657b"
title: "Session saves its own"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Since cloud transcripts can vanish before they're read (the 0.5.2 release session did), should each session save its own end-of-turn messages for the seed before it closes, rather than a later session reading them back with list_events?\", facing the 0.5.2 release session left the sessions API before its turns were read; only 3 were recovered, we decided for Session saves its own and neglected Next session reads back; Both, on the premise that sessions end with /weigh-in, accepting that if wrong: a session that skips the close loses its turns."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-05"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-48657b"
question: "Since cloud transcripts can vanish before they're read (the 0.5.2 release session did), should each session save its own end-of-turn messages for the seed before it closes, rather than a later session reading them back with list_events?"
door: two-way
blast_radius: module
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-05T03:16:12Z"
tree_hash: "a38d454ee657f965c1136909d7588b111ebda300"
recommended: "A"
flip_condition: "the vanishing was transient"
---

# Session saves its own

## Context and problem statement

Since cloud transcripts can vanish before they're read (the 0.5.2 release session did), should each session save its own end-of-turn messages for the seed before it closes, rather than a later session reading them back with list_events?

The 0.5.2 release session left the sessions API before its turns were read; only 3 were recovered.

Raised by an agent (marker) as a question and captured by Weigh-In as `wi-48657b`. Anchor: `eval/README.md:44`.

## Decision drivers

- Loss risk

## Considered options

### A · Session saves its own (chosen)

Each session adds its end-of-turn messages to eval/seed.jsonl at its close, from its own transcript.

- Holds if sessions end with /weigh-in.
- If wrong: a session that skips the close loses its turns.

### B · Next session reads back

Status quo: list_events from a later session.

- Holds if vanishing was a one-off.
- If wrong: more sessions lost.

### C · Both

Self-save plus read-back.

- Holds if both are cheap.
- If wrong: duplicates to dedupe.

## Evidence

- `eval/README.md:44`: the transcript is the only record

## Decision outcome

Chosen option: **Session saves its own**, the recommended option.

Decider's note: Session saves its own

### Consequences

- Good, because it holds if sessions end with /weigh-in.
- Bad, because if wrong: a session that skips the close loses its turns.

### Confirmation

Revisit when: the vanishing was transient

## Why this might be wrong

a session that ends abruptly saves nothing

## Provenance

- Decided 2026-10-05T03:16:12Z by human:ryan (user-approved), via chat.
- Raised in session `2b46cfdc-83e0-50b2-8dc4-18b1835adc3f`.
- Verbatim: "kind: question"
