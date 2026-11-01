---
id: "kb-decision-wi-10c645"
title: "Tighten now"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Should the temper baseline be tightened from 729 to 727 (temper baseline --tighten), locking in the two findings\", facing the gate counts 727 findings against a recorded 729, so two fixed findings could return unnoticed until the baseline is tightened, we decided for Tighten now and neglected Tighten at releases, on the premise that the ratchet should always sit at the current count, accepting that if wrong: one extra PR now; no behaviour changes."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-03"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-10c645"
question: "Should the temper baseline be tightened from 729 to 727 (temper baseline --tighten), locking in the two findings"
door: two-way
blast_radius: module
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-03T17:55:35Z"
tree_hash: "2edd9586052a96bb12d42c99557151b2ec9a838f"
recommended: "A"
flip_condition: "you tighten only at release points"
---

# Tighten now

## Context and problem statement

Should the temper baseline be tightened from 729 to 727 (temper baseline --tighten), locking in the two findings

The gate counts 727 findings against a recorded 729, so two fixed findings could return unnoticed until the baseline is tightened.

Raised by an agent (marker) as a question and captured by Weigh-In as `wi-10c645`. Anchor: `.claude/temper-baseline.json:1`.

## Decision drivers

- Slack in the ratchet
- Work

## Considered options

### A · Tighten now (chosen)

Run temper baseline --tighten and merge it as its own small PR.

- Holds if the ratchet should always sit at the current count.
- If wrong: one extra PR now; no behaviour changes.

### B · Tighten at releases

Leave the slack; tighten as part of each release PR.

- Holds if you prefer fewer config PRs.
- If wrong: a regression of up to two findings can pass in between.

## Evidence

- `.claude/temper-baseline.json:1`: "temper_baseline": 1,
- `PR #57 temper report`: 727 legacy findings ... within the baseline of 729. 2 findings fixed

## Decision outcome

Chosen option: **Tighten now**, the recommended option.

### Consequences

- Good, because it holds if the ratchet should always sit at the current count.
- Bad, because if wrong: one extra PR now; no behaviour changes.

### Confirmation

Revisit when: you tighten only at release points

## Why this might be wrong

if tightening churns often, batching at releases is less noise

## Provenance

- Decided 2026-10-03T17:55:35Z by human:ryan (user-approved), via chat.
- Raised in session `c3033c21-fb39-5c9f-99d0-12436b203ca1`.
- Verbatim: "kind: question"
