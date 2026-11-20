---
id: "kb-decision-2e9a34a3f4"
title: "#125 D2: accept that anything able to run a binary can feed `weighin hook PostToolUse` a forged payload (document it, add a TR-09 guard tripwire), or also cross-check transcript_path, or record nothing witnessed: A"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"#125 D2: accept that anything able to run a binary can feed `weighin hook PostToolUse` a forged payload (document it, add a TR-09 guard tripwire), or also cross-check transcript_path, or record nothing witnessed?\", facing the question an agent raised, we decided for A."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-11-20"
reversibility: low
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-6a1b0885b0"
question: "#125 D2: accept that anything able to run a binary can feed `weighin hook PostToolUse` a forged payload (document it, add a TR-09 guard tripwire), or also cross-check transcript_path, or record nothing witnessed?"
door: one-way
blast_radius: system
decider: "human:ryan"
provenance: user-directed
decided_at: "2026-11-20T20:53:08Z"
tree_hash: "3ab09e4bbc3dcb2ae11f8ea64c118a534a72004e"
---

# #125 D2: accept that anything able to run a binary can feed `weighin hook PostToolUse` a forged payload (document it, add a TR-09 guard tripwire), or also cross-check transcript_path, or record nothing witnessed: A

## Context and problem statement

#125 D2: accept that anything able to run a binary can feed `weighin hook PostToolUse` a forged payload (document it, add a TR-09 guard tripwire), or also cross-check transcript_path, or record nothing witnessed?

Raised by an agent (sweep) as a question and captured by Weigh-In as `wi-6a1b0885b0`. Anchor: `docs/plan/move-2-bench.md:1`.

## Decision outcome

Chosen option: **A**.

Decider's note: Ryan, decision card 2026-11-20: accept the residual as TR-03 frames it, document it (hooks.md, charter, trust docs), file a TR-09 guard rule-pack follow-up denying an agent's Bash run of weighin hook

## Provenance

- Decided 2026-11-20T20:53:08Z by human:ryan (user-directed), via chat.
- Raised in session `b0c6d687-7060-5156-b4f0-1cfd743a63f9`.
- Verbatim: "kind: question"
