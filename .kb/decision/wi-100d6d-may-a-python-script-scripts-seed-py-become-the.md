---
id: "kb-decision-wi-100d6d"
title: "Add scripts/seed.py"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"May a Python script (scripts/seed.py) become the repository's second scripting language, to make label review and seed saves one command each?\", facing scripts/seed.py would make label review and each session's seed save one command, but adds Python as a second scripting language, we decided for Add scripts/seed.py and neglected Stay sh and PowerShell, on the premise that the seed is mostly maintained by cloud sessions, accepting that if wrong: running it on Windows needs Python installed."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-08"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-100d6d"
question: "May a Python script (scripts/seed.py) become the repository's second scripting language, to make label review and seed saves one command each?"
door: two-way
blast_radius: module
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-08T05:02:43Z"
tree_hash: "8cf80029551cd98c97139ab9b4b27a400669af5d"
recommended: "A"
flip_condition: "you want to run seed tooling on Windows without Python"
---

# Add scripts/seed.py

## Context and problem statement

May a Python script (scripts/seed.py) become the repository's second scripting language, to make label review and seed saves one command each?

scripts/seed.py would make label review and each session's seed save one command, but adds Python as a second scripting language.

Raised by an agent (marker) as a question and captured by Weigh-In as `wi-100d6d`. Anchor: `scripts/weekly-stats.sh:1`.

## Decision drivers

- Languages
- Seed save

## Considered options

### A · Add scripts/seed.py (chosen)

Python 3 stdlib only; run by cloud sessions (Linux) and optional locally

- Holds if the seed is mostly maintained by cloud sessions.
- If wrong: running it on Windows needs Python installed.

### B · Stay sh and PowerShell

sessions keep writing throwaway parsers; label review stays manual

- Holds if seed work stays rare.
- If wrong: each session re-derives the turn parser and can get it wrong.

## Evidence

- `#22 proposal comment`: review / apply --flip / from-transcript subcommands, stdlib only
- `CLAUDE.md wi-48657b`: each cloud session parses its transcript with a script

## Decision outcome

Chosen option: **Add scripts/seed.py**, the recommended option.

Decider's note: add scripts/seed.py

### Consequences

- Good, because it holds if the seed is mostly maintained by cloud sessions.
- Bad, because if wrong: running it on Windows needs Python installed.

### Confirmation

Revisit when: you want to run seed tooling on Windows without Python

## Why this might be wrong

jq-free sh might be enough if the format stays simple

## Provenance

- Decided 2026-10-08T05:02:43Z by human:ryan (user-approved), via chat.
- Raised in session `ca6d9b4d-821f-5563-bf7e-5c243bdbde79`.
- Verbatim: "kind: question"
