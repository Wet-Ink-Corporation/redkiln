---
id: "kb-decision-wi-ee47e6"
title: "Recreate now in claude.ai"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Is Monday 06:52 UTC a good time for the weekly #23 report, and can a routine created without connectors post to GitHub?\", facing the weekly #23 report was created without connectors or a repo source; the tool warned its sessions may lack GitHub tools, we decided for Recreate now in claude.ai and neglected Keep and see; Drop it, on the premise that you have 2 minutes now, accepting that if wrong: nothing lost."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-09"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-ee47e6"
question: "Is Monday 06:52 UTC a good time for the weekly #23 report, and can a routine created without connectors post to GitHub?"
door: two-way
blast_radius: external
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-09T04:52:38Z"
tree_hash: "ef4f1136eaf287a5afb6c08a75db1348d2620acc"
recommended: "B"
flip_condition: "cloud sessions turn out to have GitHub access without a connector"
---

# Recreate now in claude.ai

## Context and problem statement

Is Monday 06:52 UTC a good time for the weekly #23 report, and can a routine created without connectors post to GitHub?

The weekly #23 report was created without connectors or a repo source; the tool warned its sessions may lack GitHub tools.

Raised by an agent (marker) as a assumption and captured by Weigh-In as `wi-ee47e6`. Anchor: `scripts/weekly-stats.sh:1`.

## Decision drivers

- Your effort

## Considered options

### A · Keep and see

Leave it; recreate only if nothing posts on the 12th

- Holds if cloud sessions get GitHub access without a connector.
- If wrong: week 1 posts late, by hand or a recreated routine.

### B · Recreate now in claude.ai (chosen)

Routines page, with GitHub connector and this repo as source; delete this one

- Holds if you have 2 minutes now.
- If wrong: nothing lost.

### C · Drop it

Run weekly-stats.sh and paste by hand

- Holds if you prefer manual control.
- If wrong: back to the manual step you wanted gone.

## Evidence

- `routine trig_01MizfvJ5YYkz7PzWemKKQLF`: Mondays 06:52 UTC; sources: []; mcp_connections: []

## Decision outcome

Chosen option: **Recreate now in claude.ai**, the recommended option.

Decider's note: recreate the routine now on claude.ai with the GitHub connector and this repo

### Consequences

- Good, because it holds if you have 2 minutes now.
- Bad, because if wrong: nothing lost.

### Confirmation

Revisit when: cloud sessions turn out to have GitHub access without a connector

## Why this might be wrong

A may well work, making B unnecessary

## Provenance

- Decided 2026-10-09T04:52:38Z by human:ryan (user-approved), via chat.
- Raised in session `a30b6003-c528-5711-b5a2-08effa8e852d`.
- Verbatim: "kind: assumption"
