---
id: "kb-decision-wi-f6f07a"
title: "Plugin-tree check only"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"May .github/workflows change as proposed: a weekly-stats smoke run in ci.yml (#23) and a check-plugin-tree.sh step in ci.yml and release.yml that asserts every binary, weighin.exe, 100755 modes and the skill stamp (#76)?\", facing two CI additions were proposed; .github edits need your approval, we decided for Plugin-tree check only and neglected Both; Neither until 1.0.0, on the premise that release safety (#76) is what matters before 1.0.0, accepting that if wrong: weekly-stats regressions surface on 2026-10-12 instead of in CI."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-08"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-f6f07a"
question: "May .github/workflows change as proposed: a weekly-stats smoke run in ci.yml (#23) and a check-plugin-tree.sh step in ci.yml and release.yml that asserts every binary, weighin.exe, 100755 modes and the skill stamp (#76)?"
door: two-way
blast_radius: system
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-08T04:25:34Z"
tree_hash: "6afc146aaed963738fe8b9f357053f56fa41d07d"
recommended: "B"
flip_condition: "the weekly-stats script breaks in a way only CI would catch"
---

# Plugin-tree check only

## Context and problem statement

May .github/workflows change as proposed: a weekly-stats smoke run in ci.yml (#23) and a check-plugin-tree.sh step in ci.yml and release.yml that asserts every binary, weighin.exe, 100755 modes and the skill stamp (#76)?

Two CI additions were proposed; .github edits need your approval.

Raised by an agent (marker) as a question and captured by Weigh-In as `wi-f6f07a`. Anchor: `.github/workflows/ci.yml:96`.

## Decision drivers

- Release safety
- CI cost

## Considered options

### A · Both

add check-plugin-tree to ci.yml and release.yml, and a weekly-stats smoke run

- Holds if you want CI to cover the stats script too.
- If wrong: one more CI job to maintain.

### B · Plugin-tree check only (chosen)

add scripts/check-plugin-tree.sh and wire it into ci.yml and release.yml

- Holds if release safety (#76) is what matters before 1.0.0.
- If wrong: weekly-stats regressions surface on 2026-10-12 instead of in CI.

### C · Neither until 1.0.0

keep CI as is

- Holds if you want CI frozen for the release.
- If wrong: a release can again ship without weighin.exe (#76).

## Evidence

- `.github/workflows/ci.yml:96`: today: grep checks for launcher and binary modes
- `#76`: proposal: scripts/check-plugin-tree.sh asserting weighin.exe, every target binary, 100755 modes, skill stamp
- `#23`: proposal: weekly-stats.sh smoke run in CI

## Decision outcome

Chosen option: **Plugin-tree check only**, the recommended option.

Decider's note: plugin-tree check only

### Consequences

- Good, because it holds if release safety (#76) is what matters before 1.0.0.
- Bad, because if wrong: weekly-stats regressions surface on 2026-10-12 instead of in CI.

### Confirmation

Revisit when: the weekly-stats script breaks in a way only CI would catch

## Why this might be wrong

week 1 on 2026-10-12 may expose a stats bug a smoke run would have caught

## Provenance

- Decided 2026-10-08T04:25:34Z by human:ryan (user-approved), via chat.
- Raised in session `ca6d9b4d-821f-5563-bf7e-5c243bdbde79`.
- Verbatim: "kind: question"
