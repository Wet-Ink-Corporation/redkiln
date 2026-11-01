---
id: "kb-decision-wi-52e471"
title: "Skip markers inside quotes"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Should the scanner stop raising TODOs from comment markers inside string literals (and tighten the prose hints \"temporary\", \"for now\"), given this session's ledger got 212 noise items that way from test fixtures?\", facing the scanner raises TODOs from comment markers inside string literals; this session got 212 noise items that way from test fixtures, we decided for Skip markers inside quotes and neglected Skip test files; Keep as is, on the premise that most real TODOs are in comments that do not follow an open string, accepting that if wrong: a TODO after a line with a stray quote (an apostrophe in prose) is missed."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-08"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-52e471"
question: "Should the scanner stop raising TODOs from comment markers inside string literals (and tighten the prose hints \"temporary\", \"for now\"), given this session's ledger got 212 noise items that way from test fixtures?"
door: two-way
blast_radius: module
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-08T04:57:19Z"
tree_hash: "32f5d5b7eb426f8b2803b71234d641befe5df595"
recommended: "A"
flip_condition: "a seed item shows a real TODO lost to a stray apostrophe"
---

# Skip markers inside quotes

## Context and problem statement

Should the scanner stop raising TODOs from comment markers inside string literals (and tighten the prose hints "temporary", "for now"), given this session's ledger got 212 noise items that way from test fixtures?

The scanner raises TODOs from comment markers inside string literals; this session got 212 noise items that way from test fixtures.

Raised by an agent (marker) as a question and captured by Weigh-In as `wi-52e471`. Anchor: `crates/weighin-core/src/scan.rs:276`.

## Decision drivers

- This session's 212

## Considered options

### A · Skip markers inside quotes (chosen)

a marker after an odd number of unescaped quotes on its line is not a comment; prose hints unchanged

- Holds if most real TODOs are in comments that do not follow an open string.
- If wrong: a TODO after a line with a stray quote (an apostrophe in prose) is missed.

### B · Skip test files

never scan files under tests/ or named *_test.* / test_*

- Holds if noise mostly comes from fixtures.
- If wrong: real TODOs in tests are missed, and fixtures in src/ (as here) still raise.

### C · Keep as is

leave the scanner; dismiss noise in sittings

- Holds if this session was a one-off.
- If wrong: not-useful stays high and may trigger #25.

## Evidence

- `crates/weighin-core/src/scan.rs:276`: comment_start finds // and # anywhere on the line
- `ledger 2026-10-08`: 212 scan items, all from string literals in crates/weighin/src/annotate.rs and tests/it/annotate.rs

## Decision outcome

Chosen option: **Skip markers inside quotes**, the recommended option.

Decider's note: skip markers in quotes

### Consequences

- Good, because it holds if most real TODOs are in comments that do not follow an open string.
- Bad, because if wrong: a TODO after a line with a stray quote (an apostrophe in prose) is missed.

### Confirmation

Revisit when: a seed item shows a real TODO lost to a stray apostrophe

## Why this might be wrong

apostrophes in prose comments are common in some codebases

## Provenance

- Decided 2026-10-08T04:57:19Z by human:ryan (user-approved), via chat.
- Raised in session `ca6d9b4d-821f-5563-bf7e-5c243bdbde79`.
- Verbatim: "kind: question"
