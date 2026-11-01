---
id: "kb-decision-wi-8d37cf"
title: "Resolve against the root"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Should a relative $WEIGHIN_STORE resolve against the repository root like the config's store, or be refused?\", facing a relative $WEIGHIN_STORE resolves against the process cwd, so hooks started in different directories open different ledgers; the config's relative store resolves against the repository root, we decided for Resolve against the root and neglected Refuse relative values; Keep as is, on the premise that one ledger per repository is the intent everywhere, accepting that if wrong: someone relying on cwd-relative paths gets a different ledger."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-08"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-8d37cf"
question: "Should a relative $WEIGHIN_STORE resolve against the repository root like the config's store, or be refused?"
door: two-way
blast_radius: module
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-08T05:02:43Z"
tree_hash: "8cf80029551cd98c97139ab9b4b27a400669af5d"
recommended: "A"
flip_condition: "anyone relies on a cwd-relative WEIGHIN_STORE"
---

# Resolve against the root

## Context and problem statement

Should a relative $WEIGHIN_STORE resolve against the repository root like the config's store, or be refused?

A relative $WEIGHIN_STORE resolves against the process cwd, so hooks started in different directories open different ledgers; the config's relative store resolves against the repository root.

Raised by an agent (marker) as a question and captured by Weigh-In as `wi-8d37cf`. Anchor: `crates/weighin/src/env.rs:304`.

## Decision drivers

- Same rule as the config

## Considered options

### A · Resolve against the root (chosen)

a relative WEIGHIN_STORE resolves against the repository root, like the config's store

- Holds if one ledger per repository is the intent everywhere.
- If wrong: someone relying on cwd-relative paths gets a different ledger.

### B · Refuse relative values

a relative WEIGHIN_STORE is an error naming the variable

- Holds if explicit absolute paths are acceptable.
- If wrong: a working relative setup stops with an error.

### C · Keep as is

leave cwd-relative behaviour

- Holds if nobody sets a relative value.
- If wrong: split ledgers and hook-error logs, silently.

## Evidence

- `crates/weighin/src/env.rs:302`: state_dir: store_path().parent(), empty for a bare file name
- `#26 finding 21`: reproduced: WEIGHIN_STORE=ledger.sqlite3 writes hook-errors.log in each starting cwd

## Decision outcome

Chosen option: **Resolve against the root**, the recommended option.

Decider's note: resolve against root

### Consequences

- Good, because it holds if one ledger per repository is the intent everywhere.
- Bad, because if wrong: someone relying on cwd-relative paths gets a different ledger.

### Confirmation

Revisit when: anyone relies on a cwd-relative WEIGHIN_STORE

## Why this might be wrong

WEIGHIN_STORE outside a repository has no root to resolve against

## Provenance

- Decided 2026-10-08T05:02:43Z by human:ryan (user-approved), via chat.
- Raised in session `ca6d9b4d-821f-5563-bf7e-5c243bdbde79`.
- Verbatim: "kind: question"
