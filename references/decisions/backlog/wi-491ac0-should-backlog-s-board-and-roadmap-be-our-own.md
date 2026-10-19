---
id: "kb-decision-wi-491ac0"
title: "Our own views"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Should Backlog's board and roadmap be our own projections over Issues rather than a GitHub Project?\", facing gitHub Projects needs a project scope or stored app token that automation cannot grant, we decided for Our own views and neglected GitHub Project, on the premise that we will maintain the views, accepting that if wrong: add a GitHub Project sync later."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-19"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-491ac0"
question: "Should Backlog's board and roadmap be our own projections over Issues rather than a GitHub Project?"
door: two-way
blast_radius: system
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-19T04:28:22Z"
recommended: "A"
flip_condition: "Actions token gains project write"
---

# Our own views

## Context and problem statement

Should Backlog's board and roadmap be our own projections over Issues rather than a GitHub Project?

GitHub Projects needs a project scope or stored app token that automation cannot grant.

Raised by an agent (sweep) as a question and captured by Weigh-In as `wi-491ac0`. Anchor: `https://claude.ai/code/artifact/6a5d3746-3d34-4c36-8776-2206b0aaf526`.

## Decision drivers

- Extra permission

## Considered options

### A · Our own views (chosen)

Board and roadmap on redkiln-view over Issues; roll-ups in bodies

- Holds if we will maintain the views.
- If wrong: add a GitHub Project sync later.

### B · GitHub Project

Project as the board, set up with the project scope

- Holds if granting scopes is acceptable.
- If wrong: every user and Action needs extra permission.

## Evidence

- `https://docs.github.com/en/rest/projects/views`: org endpoint needs Projects write permission

## Decision outcome

Chosen option: **Our own views**, the recommended option.

### Consequences

- Good, because it holds if we will maintain the views.
- Bad, because if wrong: add a GitHub Project sync later.

### Confirmation

Revisit when: Actions token gains project write

## Why this might be wrong

Colleagues without the suite see only the issues list

## Provenance

- Decided 2026-10-19T04:28:22Z by human:ryan (user-approved), via cli.
- Raised in session `740d340f-d7bd-5e81-b390-5a65c8fd6e86`.
- Verbatim: "Should Backlog's board and roadmap be our own projections over Issues rather than a GitHub Project?"
