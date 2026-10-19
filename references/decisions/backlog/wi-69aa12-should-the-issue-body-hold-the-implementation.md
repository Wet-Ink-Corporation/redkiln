---
id: "kb-decision-wi-69aa12"
title: "Contract in body"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Should the issue body hold the implementation contract (requirements, AC, test cases, checklist) with long specs as committed files it links?\", facing agents need requirements, AC and tests visible on the issue while they build, we decided for Contract in body and neglected All in body; All in files, on the premise that contracts fit in a body, accepting that if wrong: oversized contracts move to files case by case."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-19"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-69aa12"
question: "Should the issue body hold the implementation contract (requirements, AC, test cases, checklist) with long specs as committed files it links?"
door: two-way
blast_radius: system
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-19T04:28:17Z"
recommended: "A"
flip_condition: "contracts regularly exceed the body limit"
---

# Contract in body

## Context and problem statement

Should the issue body hold the implementation contract (requirements, AC, test cases, checklist) with long specs as committed files it links?

Agents need requirements, AC and tests visible on the issue while they build.

Raised by an agent (sweep) as a question and captured by Weigh-In as `wi-69aa12`. Anchor: `https://claude.ai/code/artifact/6a5d3746-3d34-4c36-8776-2206b0aaf526`.

## Decision drivers

- Visible on issue

## Considered options

### A · Contract in body (chosen)

Body holds R, AC, TC and checklist; long specs as committed files it links

- Holds if contracts fit in a body.
- If wrong: oversized contracts move to files case by case.

### B · All in body

Specs and designs live in the issue too

- Holds if specs are short.
- If wrong: no PR review of designs, 65,536-char cap.

### C · All in files

Issue links to spec files only

- Holds if everyone works from the repo.
- If wrong: issue shows nothing useful during work.

## Evidence

- `redkiln-rs .github/ISSUE_TEMPLATE/task.yml`: Problem, Evidence, Acceptance criteria, Doc links, Out of scope

## Decision outcome

Chosen option: **Contract in body**, the recommended option.

### Consequences

- Good, because it holds if contracts fit in a body.
- Bad, because if wrong: oversized contracts move to files case by case.

### Confirmation

Revisit when: contracts regularly exceed the body limit

## Why this might be wrong

Two places can drift if the spec changes after the body

## Provenance

- Decided 2026-10-19T04:28:17Z by human:ryan (user-approved), via cli.
- Raised in session `740d340f-d7bd-5e81-b390-5a65c8fd6e86`.
- Verbatim: "Should the issue body hold the implementation contract (requirements, AC, test cases, checklist) with long specs as committed files it links?"
