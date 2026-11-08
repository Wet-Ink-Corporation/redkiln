---
id: "kb-decision-wi-56bd73"
title: "Trim and update goldens"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"May the atom golden tests change so y_statement trims trailing periods (\"small, … conflict.\" instead of \"small., … conflict..\")?\", facing y_statement renders small., and conflict.. when holds_if or if_wrong end with a period; fixing it changes pinned golden values (H-12), we decided for Trim and update goldens and neglected Keep as is, on the premise that atoms should read cleanly; existing atoms are not regenerated, accepting that if wrong: none known; old atoms keep their text."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-08"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-56bd73"
question: "May the atom golden tests change so y_statement trims trailing periods (\"small, … conflict.\" instead of \"small., … conflict..\")?"
door: two-way
blast_radius: module
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-08T04:25:34Z"
tree_hash: "6afc146aaed963738fe8b9f357053f56fa41d07d"
recommended: "A"
flip_condition: "kb-ingest matches atoms by summary text"
---

# Trim and update goldens

## Context and problem statement

May the atom golden tests change so y_statement trims trailing periods ("small, … conflict." instead of "small., … conflict..")?

y_statement renders small., and conflict.. when holds_if or if_wrong end with a period; fixing it changes pinned golden values (H-12).

Raised by an agent (marker) as a question and captured by Weigh-In as `wi-56bd73`. Anchor: `crates/weighin-core/src/atom.rs:160`.

## Decision drivers

- Atom text
- Test change

## Considered options

### A · Trim and update goldens (chosen)

trim trailing periods as Consequences already does; update the two golden values

- Holds if atoms should read cleanly; existing atoms are not regenerated.
- If wrong: none known; old atoms keep their text.

### B · Keep as is

leave the doubled punctuation

- Holds if the golden values must never change after release.
- If wrong: every rich decision atom keeps the doubled punctuation.

## Evidence

- `crates/weighin-core/src/atom.rs:160`: y_statement appends holds_if and if_wrong untrimmed
- `crates/weighin-core/src/atom.rs:606`: GOLDEN_DECISION_RICH summary: small., ... conflict..

## Decision outcome

Chosen option: **Trim and update goldens**, the recommended option.

Decider's note: trim and update goldens

### Consequences

- Good, because it holds if atoms should read cleanly; existing atoms are not regenerated.
- Bad, because if wrong: none known; old atoms keep their text.

### Confirmation

Revisit when: kb-ingest matches atoms by summary text

## Why this might be wrong

something downstream may compare summaries byte for byte

## Provenance

- Decided 2026-10-08T04:25:34Z by human:ryan (user-approved), via chat.
- Raised in session `ca6d9b4d-821f-5563-bf7e-5c243bdbde79`.
- Verbatim: "kind: question"
