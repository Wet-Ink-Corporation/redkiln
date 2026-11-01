---
id: "kb-decision-wi-7c3aa0"
title: "Release 0.6.0 before 10-12"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Should the capture-changing fixes (wi-64c8ea, wi-52e471, wi-e038ef) ship in a release during the #23 pilot, or merge to main and wait for release until the two weeks close?\", facing three recorded decisions change what capture raises; when they reach the pilot decides whether #23's two weeks measure one behaviour or two, we decided for Release 0.6.0 before 10-12 and neglected Merge to main, hold the release; Hold the PRs until #23 closes, on the premise that the window still opens on 10-12 and the fixes land in two days, accepting that if wrong: the window slips a few days, or week 1 runs on 0.5.4 and is reported as such."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-10"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-7c3aa0"
question: "Should the capture-changing fixes (wi-64c8ea, wi-52e471, wi-e038ef) ship in a release during the #23 pilot, or merge to main and wait for release until the two weeks close?"
door: two-way
blast_radius: system
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-10T08:02:32Z"
tree_hash: "4f21a6235a8dd38512a1cb5856530978fe4cbca4"
recommended: "A"
flip_condition: "the window opening before the fixes can land, then C"
---

# Release 0.6.0 before 10-12

## Context and problem statement

Should the capture-changing fixes (wi-64c8ea, wi-52e471, wi-e038ef) ship in a release during the #23 pilot, or merge to main and wait for release until the two weeks close?

Three recorded decisions change what capture raises; when they reach the pilot decides whether #23's two weeks measure one behaviour or two.

Raised by an agent (marker) as a question and captured by Weigh-In as `wi-7c3aa0`. Anchor: `crates/weighin-core/src/marker.rs:166`.

## Decision drivers

- Pilot measures one behaviour
- Your time
- Known noise in the pilot

## Considered options

### A · Release 0.6.0 before 10-12 (chosen)

An AFK session builds the three capture fixes first; you tag 0.6.0 and update machines before the window opens.

- Holds if the window still opens on 10-12 and the fixes land in two days.
- If wrong: the window slips a few days, or week 1 runs on 0.5.4 and is reported as such.

### B · Merge to main, hold the release

Your installed machines stay on 0.5.4; cloud sessions on this repo pick up the new capture at once.

- Holds if cloud-session journals are a small share of #23.
- If wrong: #23 mixes two capture behaviours and needs a split date in the stats.

### C · Hold the PRs until #23 closes

Build them on branches, merge after the two weeks.

- Holds if the scanner noise is rare outside this repo.
- If wrong: noise inflates not-useful in the pilot and could pull #25 into v1 for the wrong reason.

## Evidence

- `docs/audits/2026-10-06-status-audit.md:27`: The #23 window opens on 2026-10-12, with week 2 on or after 2026-10-19
- `CLAUDE.md`: In cloud sessions ... the plugin is wired through .claude/settings.json and .claude/hooks/session-start.sh, because cloud sessions do not install plugins (B-12)
- `.kb/_intake/decisions/wi-52e471-should-the-scanner-stop-raising-todos-from.md`: the scanner raises TODOs from comment markers inside string literals; this session got 212 noise items that way
- `crates/weighin-core/src/marker.rs:166`: .is_some_and(|d| d.trim().eq_ignore_ascii_case("blocked"))

## Decision outcome

Chosen option: **Release 0.6.0 before 10-12**, the recommended option.

### Consequences

- Good, because it holds if the window still opens on 10-12 and the fixes land in two days.
- Bad, because if wrong: the window slips a few days, or week 1 runs on 0.5.4 and is reported as such.

### Confirmation

Revisit when: the window opening before the fixes can land, then C

## Why this might be wrong

if two days is too tight, a rushed release could itself add hook errors to the pilot

## Provenance

- Decided 2026-10-10T08:02:32Z by human:ryan (user-approved), via chat.
- Raised in session `355e0caf-9396-568f-9148-439277874085`.
- Verbatim: "kind: question"
