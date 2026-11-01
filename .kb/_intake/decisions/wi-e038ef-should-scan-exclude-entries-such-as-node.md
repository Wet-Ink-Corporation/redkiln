---
id: "kb-decision-wi-e038ef"
title: "Bare names at any depth"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Should scan.exclude entries such as node_modules/ and target/ match at any depth, the way .gitignore treats a bare name, instead of only as repository-root prefixes?\", facing scan.exclude entries (target/, node_modules/) match only as repository-root prefixes, so nested ones in a monorepo are scanned, while context.rs already excludes them at any depth, we decided for Bare names at any depth and neglected Keep root-only, on the premise that nobody needs a bare name excluded only at the root, accepting that if wrong: a nested folder sharing a name with an excluded one is skipped."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-08"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-e038ef"
question: "Should scan.exclude entries such as node_modules/ and target/ match at any depth, the way .gitignore treats a bare name, instead of only as repository-root prefixes?"
door: two-way
blast_radius: module
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-08T05:02:43Z"
tree_hash: "8cf80029551cd98c97139ab9b4b27a400669af5d"
recommended: "A"
flip_condition: "someone relies on root-only exclusion of a nested path"
---

# Bare names at any depth

## Context and problem statement

Should scan.exclude entries such as node_modules/ and target/ match at any depth, the way .gitignore treats a bare name, instead of only as repository-root prefixes?

scan.exclude entries (target/, node_modules/) match only as repository-root prefixes, so nested ones in a monorepo are scanned, while context.rs already excludes them at any depth.

Raised by an agent (marker) as a question and captured by Weigh-In as `wi-e038ef`. Anchor: `crates/weighin/src/ops.rs:1065`.

## Decision drivers

- Matches .gitignore intuition

## Considered options

### A · Bare names at any depth (chosen)

a single-segment entry (node_modules/) matches at any depth, as .gitignore does; multi-segment entries stay anchored

- Holds if nobody needs a bare name excluded only at the root.
- If wrong: a nested folder sharing a name with an excluded one is skipped.

### B · Keep root-only

leave prefix matching

- Holds if monorepos are not in use.
- If wrong: vendored READMEs and TODOs raise noise.

## Evidence

- `crates/weighin/src/ops.rs (scannable)`: exclude entries compared as prefixes of the repo-relative path
- `#26 finding 26`: reproduced: packages/app/node_modules/x/README.md is scanned

## Decision outcome

Chosen option: **Bare names at any depth**, the recommended option.

Decider's note: bare names at any depth

### Consequences

- Good, because it holds if nobody needs a bare name excluded only at the root.
- Bad, because if wrong: a nested folder sharing a name with an excluded one is skipped.

### Confirmation

Revisit when: someone relies on root-only exclusion of a nested path

## Why this might be wrong

a repository may have a real source folder named build/ nested under a package

## Provenance

- Decided 2026-10-08T05:02:43Z by human:ryan (user-approved), via chat.
- Raised in session `ca6d9b4d-821f-5563-bf7e-5c243bdbde79`.
- Verbatim: "kind: question"
