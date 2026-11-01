---
id: "kb-decision-wi-486cfc"
title: "Yes"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Should the mod's local board pane replace weighin serve --wait for client repos in Claude Code, amending wi-7c21e4?\", facing wi-7c21e4 called for a local board; serve --wait was its planned form, we decided for Yes and neglected No, on the premise that client work runs in the terminal or desktop, accepting that if wrong: build serve --wait after all."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-11"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-486cfc"
question: "Should the mod's local board pane replace weighin serve --wait for client repos in Claude Code, amending wi-7c21e4?"
door: two-way
blast_radius: module
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-11T14:17:21Z"
tree_hash: "f81993f6723ef0895287e8171b003c754b86b6ca"
recommended: "A"
flip_condition: "VS Code panel used with client repos"
---

# Yes

## Context and problem statement

Should the mod's local board pane replace weighin serve --wait for client repos in Claude Code, amending wi-7c21e4?

wi-7c21e4 called for a local board; serve --wait was its planned form.

Raised by an agent (sweep) as a question and captured by Weigh-In as `wi-486cfc`. Anchor: `docs/decisions/wi-7c21e4.md:1`.

## Considered options

### A · Yes (chosen)

Pane is the local board; serve --wait only for the VS Code panel

- Holds if client work runs in the terminal or desktop.
- If wrong: build serve --wait after all.

### B · No

Build serve --wait as planned

- Holds if mods are blocked or unused.
- If wrong: two local boards to keep.

## Evidence

- `docs/decisions/wi-7c21e4.md:66`: Per-repo board mode; client repos default to local

## Decision outcome

Chosen option: **Yes**, the recommended option.

Decider's note: Ryan in the mods thread, 2026-10-11: yes (D-0017)

### Consequences

- Good, because it holds if client work runs in the terminal or desktop.
- Bad, because if wrong: build serve --wait after all.

### Confirmation

Revisit when: VS Code panel used with client repos

## Why this might be wrong

mods API still moving

## Provenance

- Decided 2026-10-11T14:17:21Z by human:ryan (user-approved), via chat.
- Raised in session `72702aeb-7823-5243-8308-ccb0e2581026`.
- Verbatim: "Should the mod's local board pane replace weighin serve --wait for client repos in Claude Code, amending wi-7c21e4?"
