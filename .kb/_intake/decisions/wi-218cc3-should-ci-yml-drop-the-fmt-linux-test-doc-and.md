---
id: "kb-decision-wi-218cc3"
title: "Drop the duplicates from ci.yml"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Should ci.yml drop the fmt, Linux test, doc and deny steps that rust.yml already runs, so each pull request runs them once?\", facing ci.yml and rust.yml both run fmt, the Linux tests, docs and cargo deny on every PR, we decided for Drop the duplicates from ci.yml and neglected Keep both, on the premise that rust.yml's checks are required in the ruleset too, accepting that if wrong: a check stops being required for merge."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-06"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-218cc3"
question: "Should ci.yml drop the fmt, Linux test, doc and deny steps that rust.yml already runs, so each pull request runs them once?"
door: two-way
blast_radius: system
decider: "human:ryan"
provenance: user-directed
decided_at: "2026-10-06T04:54:33Z"
tree_hash: "f59525a31975c6ffd86d27d6f0751813f0745387"
recommended: "A"
flip_condition: "CI queue time or minutes becoming a cost"
---

# Drop the duplicates from ci.yml

## Context and problem statement

Should ci.yml drop the fmt, Linux test, doc and deny steps that rust.yml already runs, so each pull request runs them once?

ci.yml and rust.yml both run fmt, the Linux tests, docs and cargo deny on every PR.

Raised by an agent (marker) as a question and captured by Weigh-In as `wi-218cc3`. Anchor: `.github/workflows/ci.yml:102`.

## Decision drivers

- PR wall time
- Ruleset change

## Considered options

### A · Keep both

status quo; the ruleset's required checks stay as named

- Holds if CI minutes are not a cost you track.
- If wrong: each PR waits on duplicate Linux runs.

### B · Drop the duplicates from ci.yml (chosen)

remove ci.yml's deny job and the Linux fmt/doc steps; keep the three-platform test matrix and the plugin job

- Holds if rust.yml's checks are required in the ruleset too.
- If wrong: a check stops being required for merge.

## Evidence

- `.github/workflows/ci.yml:102`: deny job: EmbarkStudios/cargo-deny-action@v2
- `docs/v1-plan.md`: ruleset requires check (ubuntu-latest), check (windows-latest)

## Decision outcome

Chosen option: **Drop the duplicates from ci.yml**, overriding the recommendation (A).

Decider's note: Drop duplicates

### Consequences

- Good, because it holds if rust.yml's checks are required in the ruleset too.
- Bad, because if wrong: a check stops being required for merge.

### Confirmation

Revisit when: CI queue time or minutes becoming a cost

## Why this might be wrong

the duplicate runs may already slow every merge

## Provenance

- Decided 2026-10-06T04:54:33Z by human:ryan (user-directed), via chat.
- Raised in session `560d7069-4907-54f9-a6f7-42616a78677a`.
- Verbatim: "kind: question"
