---
id: "kb-decision-0eed63741d"
title: "PR 5.4: should skillcheck's eval tmp folder stay under ~/.anneal/tmp until 5.6, or move to ~/.redkiln/tmp now (changing 3 expected values): B"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"PR 5.4: should skillcheck's eval tmp folder stay under ~/.anneal/tmp until 5.6, or move to ~/.redkiln/tmp now (changing 3 expected values)?\", facing the question an agent raised, we decided for B."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-11-16"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-6b797a85db"
question: "PR 5.4: should skillcheck's eval tmp folder stay under ~/.anneal/tmp until 5.6, or move to ~/.redkiln/tmp now (changing 3 expected values)?"
door: two-way
blast_radius: file
decider: "human:ryan"
provenance: user-directed
decided_at: "2026-11-16T03:51:18Z"
tree_hash: "f27e861819bd2c1b6ca7bf3883737a448c98ff40"
---

# PR 5.4: should skillcheck's eval tmp folder stay under ~/.anneal/tmp until 5.6, or move to ~/.redkiln/tmp now (changing 3 expected values): B

## Context and problem statement

PR 5.4: should skillcheck's eval tmp folder stay under ~/.anneal/tmp until 5.6, or move to ~/.redkiln/tmp now (changing 3 expected values)?

Raised by an agent (sweep) as a question and captured by Weigh-In as `wi-6b797a85db`. Anchor: `tools/skillcheck/src/eval/uplift/args.rs`.

## Decision outcome

Chosen option: **B**.

Decider's note: Ryan, decision card 2026-11-16: move skillcheck's eval tmp to <redkiln root>/tmp (UserScope) now in 5.4; 3 expected values change

## Provenance

- Decided 2026-11-16T03:51:18Z by human:ryan (user-directed), via cli.
- Raised in session `22af9544-95b0-5794-a45f-15714e38e099`.
- Verbatim: "PR 5.4: should skillcheck's eval tmp folder stay under ~/.anneal/tmp until 5.6, or move to ~/.redkiln/tmp now (changing 3 expected values)?"
