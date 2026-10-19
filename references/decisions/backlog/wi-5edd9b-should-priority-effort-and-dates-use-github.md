---
id: "kb-decision-wi-5edd9b"
title: "Issue fields"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Should priority, effort and dates use GitHub issue fields instead of prio/* and size/* labels?\", facing issue fields are a native, typed home for priority, effort and dates, we decided for Issue fields and neglected Keep labels, on the premise that plugin tokens can read and write fields, accepting that if wrong: fall back to labels for those values."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-19"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-5edd9b"
question: "Should priority, effort and dates use GitHub issue fields instead of prio/* and size/* labels?"
door: two-way
blast_radius: system
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-19T04:28:06Z"
recommended: "A"
flip_condition: "tokens cannot access fields"
---

# Issue fields

## Context and problem statement

Should priority, effort and dates use GitHub issue fields instead of prio/* and size/* labels?

Issue fields are a native, typed home for priority, effort and dates.

Raised by an agent (sweep) as a question and captured by Weigh-In as `wi-5edd9b`. Anchor: `https://claude.ai/code/artifact/6a5d3746-3d34-4c36-8776-2206b0aaf526`.

## Decision drivers

- Roadmap dates

## Considered options

### A · Issue fields (chosen)

Use the four default org fields; drop prio/* and size/*

- Holds if plugin tokens can read and write fields.
- If wrong: fall back to labels for those values.

### B · Keep labels

prio/* and size/* labels as redkiln-rs has them

- Holds if field access stays blocked.
- If wrong: dates have no label form and the roadmap lacks them.

## Evidence

- `https://github.blog/changelog/2026-07-02-issue-fields-are-now-generally-available`: four default fields: Priority, Effort, Start date, Target date

## Decision outcome

Chosen option: **Issue fields**, the recommended option.

### Consequences

- Good, because it holds if plugin tokens can read and write fields.
- Bad, because if wrong: fall back to labels for those values.

### Confirmation

Revisit when: tokens cannot access fields

## Why this might be wrong

This cloud session's GitHub integration got 403 on org issue fields

## Provenance

- Decided 2026-10-19T04:28:06Z by human:ryan (user-approved), via cli.
- Raised in session `740d340f-d7bd-5e81-b390-5a65c8fd6e86`.
- Verbatim: "Should priority, effort and dates use GitHub issue fields instead of prio/* and size/* labels?"
