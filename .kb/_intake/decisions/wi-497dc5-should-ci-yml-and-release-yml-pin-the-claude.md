---
id: "kb-decision-wi-497dc5"
title: "Leave unpinned"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Should ci.yml and release.yml pin the Claude Code version they install with npm (they run it in jobs that hold a write token), and if so who bumps the pin?\", facing ci.yml and release.yml install the latest @anthropic-ai/claude-code from npm in jobs holding a contents:write token, later the deploy key, we decided for Leave unpinned and neglected Pin an exact version, bumped in each release PR, on the premise that you trust each npm release in a job with a write token, accepting that if wrong: a bad release fails or abuses the release job."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-06"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-497dc5"
question: "Should ci.yml and release.yml pin the Claude Code version they install with npm (they run it in jobs that hold a write token), and if so who bumps the pin?"
door: two-way
blast_radius: system
decider: "human:ryan"
provenance: user-directed
decided_at: "2026-10-06T04:54:33Z"
tree_hash: "f59525a31975c6ffd86d27d6f0751813f0745387"
recommended: "A"
flip_condition: "validate needing a newer Claude Code for a new manifest rule"
---

# Leave unpinned

## Context and problem statement

Should ci.yml and release.yml pin the Claude Code version they install with npm (they run it in jobs that hold a write token), and if so who bumps the pin?

ci.yml and release.yml install the latest @anthropic-ai/claude-code from npm in jobs holding a contents:write token, later the deploy key.

Raised by an agent (marker) as a question and captured by Weigh-In as `wi-497dc5`. Anchor: `.github/workflows/release.yml:99`.

## Decision drivers

- Supply-chain exposure
- Upkeep

## Considered options

### A · Pin an exact version, bumped in each release PR

@anthropic-ai/claude-code@2.1.291 in both workflows; the release PR author bumps it

- Holds if validate keeps working on a pinned version for weeks.
- If wrong: a manifest rule claude.ai adds is missed until the pin moves.

### B · Leave unpinned (chosen)

status quo

- Holds if you trust each npm release in a job with a write token.
- If wrong: a bad release fails or abuses the release job.

## Evidence

- `.github/workflows/release.yml:99`: npm install -g @anthropic-ai/claude-code
- `.github/workflows/ci.yml:82`: npm install -g @anthropic-ai/claude-code

## Decision outcome

Chosen option: **Leave unpinned**, overriding the recommendation (A).

Decider's note: Leave unpinned

### Consequences

- Good, because it holds if you trust each npm release in a job with a write token.
- Bad, because if wrong: a bad release fails or abuses the release job.

### Confirmation

Revisit when: validate needing a newer Claude Code for a new manifest rule

## Why this might be wrong

the org sync may enforce rules a pinned validate does not know

## Provenance

- Decided 2026-10-06T04:54:33Z by human:ryan (user-directed), via chat.
- Raised in session `560d7069-4907-54f9-a6f7-42616a78677a`.
- Verbatim: "kind: question"
