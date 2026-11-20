---
id: "kb-decision-c233f6f0d8"
title: "#125 D1: should Ryan capture live AskUserQuestion PostToolUse payloads (person-answered, idle-timeout, model-prefilled tool_input.answers) before the witnessed write (PR 1b) merges: A"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"#125 D1: should Ryan capture live AskUserQuestion PostToolUse payloads (person-answered, idle-timeout, model-prefilled tool_input.answers) before the witnessed write (PR 1b) merges?\", facing the question an agent raised, we decided for A."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-11-20"
reversibility: low
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-c4c2e70481"
question: "#125 D1: should Ryan capture live AskUserQuestion PostToolUse payloads (person-answered, idle-timeout, model-prefilled tool_input.answers) before the witnessed write (PR 1b) merges?"
door: one-way
blast_radius: system
decider: "human:ryan"
provenance: user-directed
decided_at: "2026-11-20T20:52:58Z"
tree_hash: "3ab09e4bbc3dcb2ae11f8ea64c118a534a72004e"
---

# #125 D1: should Ryan capture live AskUserQuestion PostToolUse payloads (person-answered, idle-timeout, model-prefilled tool_input.answers) before the witnessed write (PR 1b) merges: A

## Context and problem statement

#125 D1: should Ryan capture live AskUserQuestion PostToolUse payloads (person-answered, idle-timeout, model-prefilled tool_input.answers) before the witnessed write (PR 1b) merges?

Raised by an agent (sweep) as a question and captured by Weigh-In as `wi-c4c2e70481`. Anchor: `docs/plan/move-2-bench.md:1`.

## Decision outcome

Chosen option: **A**.

Decider's note: Ryan, decision card 2026-11-20: capture live payloads first; PR 1b merges only if a model-prefilled answer cannot answer for him

## Provenance

- Decided 2026-11-20T20:52:58Z by human:ryan (user-directed), via chat.
- Raised in session `b0c6d687-7060-5156-b4f0-1cfd743a63f9`.
- Verbatim: "kind: question"
