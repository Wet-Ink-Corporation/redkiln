---
id: "kb-decision-73e69a5d25"
title: "Say it at next SessionStart"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"When SessionStart or Stop runs past its budget and skips the journal import or export, should the hook say so (a SessionStart line or a logged note) instead of staying silent per D12?\", facing a late hook skips the journal import or export without a word (D12), we decided for Say it at next SessionStart and neglected Keep silent, on the premise that skips are rare and worth knowing, accepting that if wrong: a noisy line on slow machines."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-11-13"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-6ab291e7fb"
question: "When SessionStart or Stop runs past its budget and skips the journal import or export, should the hook say so (a SessionStart line or a logged note) instead of staying silent per D12?"
door: two-way
blast_radius: system
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-11-13T16:28:45Z"
tree_hash: "556d26c4551febf6003aae7f1f85a36910f631cc"
recommended: "A"
flip_condition: "skips proving routine on normal machines"
---

# Say it at next SessionStart

## Context and problem statement

When SessionStart or Stop runs past its budget and skips the journal import or export, should the hook say so (a SessionStart line or a logged note) instead of staying silent per D12?

A late hook skips the journal import or export without a word (D12).

Raised by an agent (sweep) as a question and captured by Weigh-In as `wi-6ab291e7fb`. Anchor: `crates/weigh-in/weighin/src/journal/refs.rs:1`.

## Considered options

### A · Say it at next SessionStart (chosen)

Log the skip; next SessionStart reports it once

- Holds if skips are rare and worth knowing.
- If wrong: a noisy line on slow machines.

### B · Keep silent

D12 as is; the next Stop retries

- Holds if the retry always catches up.
- If wrong: a lost export goes unseen.

## Evidence

- `docs/decisions.md:123`: the hook is silent by D12

## Decision outcome

Chosen option: **Say it at next SessionStart**, the recommended option.

Decider's note: Ryan chose on the decision card, 2026-11-13: log a skipped import/export and report it once at the next SessionStart

### Consequences

- Good, because it holds if skips are rare and worth knowing.
- Bad, because if wrong: a noisy line on slow machines.

### Confirmation

Revisit when: skips proving routine on normal machines

## Why this might be wrong

D12 chose silence to keep hooks quiet

## Provenance

- Decided 2026-11-13T16:28:45Z by human:ryan (user-approved), via chat.
- Raised in session `128f4cc5-5a63-5e51-ae0f-472143607eac`.
- Verbatim: "When SessionStart or Stop runs past its budget and skips the journal import or export, should the hook say so (a SessionStart line or a logged note) instead of staying silent per D12?"
