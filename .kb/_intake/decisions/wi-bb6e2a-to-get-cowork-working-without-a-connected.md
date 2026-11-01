---
id: "kb-decision-wi-bb6e2a"
title: "Skill copies the binary"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"To get Cowork working without a connected checkout, should the skill copy the Linux binary from Cowork's plugin copy into each folder's .weighin/ (about 4.6 MB each, OneDrive and client folders included)?\", facing cowork runs weighin through the computer's shell, so the binary must be on the computer; today that needs a connected marketplace checkout in each chat, we decided for Skill copies the binary and neglected Status quo: connected checkout; Public releases now, on the premise that 4.6 MB in each folder, OneDrive and client folders included, is acceptable, accepting that if wrong: an executable sits in client folders; remove the copy step."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-05"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-bb6e2a"
question: "To get Cowork working without a connected checkout, should the skill copy the Linux binary from Cowork's plugin copy into each folder's .weighin/ (about 4.6 MB each, OneDrive and client folders included)?"
door: two-way
blast_radius: external
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-05T16:27:01Z"
tree_hash: "8b56c841f92ecbfa68905ae68813e9735010462c"
recommended: "A"
flip_condition: "client folders refusing executables, or releases going public"
---

# Skill copies the binary

## Context and problem statement

To get Cowork working without a connected checkout, should the skill copy the Linux binary from Cowork's plugin copy into each folder's .weighin/ (about 4.6 MB each, OneDrive and client folders included)?

Cowork runs weighin through the computer's shell, so the binary must be on the computer; today that needs a connected marketplace checkout in each chat.

Raised by an agent (marker) as a question and captured by Weigh-In as `wi-bb6e2a`. Anchor: `docs/decisions/build-decisions.md:117`.

## Decision drivers

- Per-chat step
- Footprint in folders
- Reversible

## Considered options

### A · Skill copies the binary (chosen)

From Cowork's plugin copy into F/.weighin/, once per folder and version.

- Holds if 4.6 MB in each folder, OneDrive and client folders included, is acceptable.
- If wrong: an executable sits in client folders; remove the copy step.

### B · Status quo: connected checkout

Connect the marketplace checkout in each chat.

- Holds if Cowork use stays occasional.
- If wrong: Weigh-In stays unusable in most Cowork chats.

### C · Public releases now

Make release assets public so the pinned-SHA download runs; reverses D-0012 early.

- Holds if publishing the binary is fine before v1.
- If wrong: internal code is public; can't be undone.

## Evidence

- `docs/decisions/build-decisions.md:120`: Cowork uses the uploaded zip and a connected checkout of that branch
- `plugin/skills/weigh-in/SKILL.md`: falls back to a copy in a connected folder

## Decision outcome

Chosen option: **Skill copies the binary**, the recommended option.

Decider's note: Skill copies the Linux binary into F/.weighin/

### Consequences

- Good, because it holds if 4.6 MB in each folder, OneDrive and client folders included, is acceptable.
- Bad, because if wrong: an executable sits in client folders; remove the copy step.

### Confirmation

Revisit when: client folders refusing executables, or releases going public

## Why this might be wrong

client folders may be a bad place for an executable

## Provenance

- Decided 2026-10-05T16:27:01Z by human:ryan (user-approved), via chat.
- Raised in session `cf1aaf47-19e1-571d-ba4e-d8b3943fc609`.
- Verbatim: "kind: question"
