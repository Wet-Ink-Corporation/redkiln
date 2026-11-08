---
id: "kb-decision-wi-90ff13"
title: "Approve both edits as pushed"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Approve #85's edits to .github/workflows/release.yml and .claude/hooks/session-start.sh, which went through the shell so the approval prompt your settings route those paths to never fired?\", facing settings.json routes .github/** and .claude/** edits to you; #85's edits went through the shell, so no prompt fired, we decided for Approve both edits as pushed and neglected Revert the workflow and hook parts, on the premise that the two diffs read right to you, accepting that if wrong: a re-run release clobbers assets you wanted kept."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-06"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-90ff13"
question: "Approve #85's edits to .github/workflows/release.yml and .claude/hooks/session-start.sh, which went through the shell so the approval prompt your settings route those paths to never fired?"
door: two-way
blast_radius: system
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-06T04:54:33Z"
tree_hash: "f59525a31975c6ffd86d27d6f0751813f0745387"
recommended: "A"
flip_condition: "a line in either diff you would have refused"
---

# Approve both edits as pushed

## Context and problem statement

Approve #85's edits to .github/workflows/release.yml and .claude/hooks/session-start.sh, which went through the shell so the approval prompt your settings route those paths to never fired?

settings.json routes .github/** and .claude/** edits to you; #85's edits went through the shell, so no prompt fired.

Raised by an agent (sweep) as a question and captured by Weigh-In as `wi-90ff13`. Anchor: `.github/workflows/release.yml:117`.

## Decision drivers

- Fixes

## Considered options

### A · Approve both edits as pushed (chosen)

merge #85 once CI is green

- Holds if the two diffs read right to you.
- If wrong: a re-run release clobbers assets you wanted kept.

### B · Revert the workflow and hook parts

keep only the scripts in #85

- Holds if you want those files changed only through prompted edits.
- If wrong: the two findings stay unfixed.

## Evidence

- `.github/workflows/release.yml:117`: gh release view || create; else upload --clobber and edit notes
- `.claude/hooks/session-start.sh:19`: warmers non-fatal; failed build removes the stale binary and says capture is off

## Decision outcome

Chosen option: **Approve both edits as pushed**, the recommended option.

Decider's note: Approve as pushed

### Consequences

- Good, because it holds if the two diffs read right to you.
- Bad, because if wrong: a re-run release clobbers assets you wanted kept.

### Confirmation

Revisit when: a line in either diff you would have refused

## Why this might be wrong

the prompt exists so these paths never change unseen

## Provenance

- Decided 2026-10-06T04:54:33Z by human:ryan (user-approved), via chat.
- Raised in session `560d7069-4907-54f9-a6f7-42616a78677a`.
- Verbatim: "Approve #85's edits to .github/workflows/release.yml and .claude/hooks/session-start.sh, which went through the shell so the approval prompt your settings route those paths to never fired?"
