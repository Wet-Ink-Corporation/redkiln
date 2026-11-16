---
id: "kb-decision-e3528d031b"
title: "Ignore an unreadable old cache, repoint the 2 tests"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"After 5.6, an unreadable old ~/.anneal/<file> (e.g. a directory) makes bench's status line and snapshot saves to ~/.redkiln/bench fail, and the two tests that pin 'cannot be saved' now pass through that read instead of an unwritable new path. Should the fallback read ignore an unreadable old cache and the two tests be repointed at <home>/bench/<file> (a test rewrite)?\", facing since #116, an unreadable old ~/.anneal/<file> stops Bench saving its status line and snapshot to ~/.redkiln/bench, and the two tests meant to pin an unwritable save now pass through that read, we decided for Ignore an unreadable old cache, repoint the 2 tests and neglected Keep as merged, on the premise that old caches are disposable, accepting that if wrong: a real read error on the old cache goes unreported (caches only)."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-11-16"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-b394c39445"
question: "After 5.6, an unreadable old ~/.anneal/<file> (e.g. a directory) makes bench's status line and snapshot saves to ~/.redkiln/bench fail, and the two tests that pin 'cannot be saved' now pass through that read instead of an unwritable new path. Should the fallback read ignore an unreadable old cache and the two tests be repointed at <home>/bench/<file> (a test rewrite)?"
door: two-way
blast_radius: module
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-11-16T15:43:30Z"
tree_hash: "79d9905a3cde0ce08ce92570fa198dd56beb6d4f"
recommended: "A"
flip_condition: "the old cache holding anything worth an error"
---

# Ignore an unreadable old cache, repoint the 2 tests

## Context and problem statement

After 5.6, an unreadable old ~/.anneal/<file> (e.g. a directory) makes bench's status line and snapshot saves to ~/.redkiln/bench fail, and the two tests that pin 'cannot be saved' now pass through that read instead of an unwritable new path. Should the fallback read ignore an unreadable old cache and the two tests be repointed at <home>/bench/<file> (a test rewrite)?

Since #116, an unreadable old ~/.anneal/<file> stops Bench saving its status line and snapshot to ~/.redkiln/bench, and the two tests meant to pin an unwritable save now pass through that read.

Raised by an agent (sweep) as a deferral and captured by Weigh-In as `wi-b394c39445`. Anchor: `crates/bench/bench/src/paths/state_home.rs:98`.

## Considered options

### A · Ignore an unreadable old cache, repoint the 2 tests (chosen)

Fallback read treats an unreadable old file as absent; the two tests aim at <home>/bench/<file>.

- Holds if old caches are disposable.
- If wrong: a real read error on the old cache goes unreported (caches only).

### B · Keep as merged

Leave behaviour and tests as in #116.

- Holds if nobody has a broken ~/.anneal cache.
- If wrong: status line saves fail until the old file is fixed.

## Evidence

- `crates/bench/bench/src/paths/state_home.rs`: read new-else-old; an error other than not-found on the old file is returned

## Decision outcome

Chosen option: **Ignore an unreadable old cache, repoint the 2 tests**, the recommended option.

Decider's note: Ryan, card 2026-11-16: fallback read treats an unreadable old cache as absent; repoint the two save-failure tests at <home>/bench/<file>

### Consequences

- Good, because it holds if old caches are disposable.
- Bad, because if wrong: a real read error on the old cache goes unreported (caches only).

### Confirmation

Revisit when: the old cache holding anything worth an error

## Why this might be wrong

hiding a read error goes against H-10 if the cache matters

## Provenance

- Decided 2026-11-16T15:43:30Z by human:ryan (user-approved), via cli.
- Raised in session `22af9544-95b0-5794-a45f-15714e38e099`.
- Verbatim: "After 5.6, an unreadable old ~/.anneal/<file> (e.g. a directory) makes bench's status line and snapshot saves to ~/.redkiln/bench fail, and the two tests that pin 'cannot be saved' now pass through that read instead of an unwritable new path. Should the fallback read ignore an unreadable old cache and the two tests be repointed at <home>/bench/<file> (a test rewrite)?"
