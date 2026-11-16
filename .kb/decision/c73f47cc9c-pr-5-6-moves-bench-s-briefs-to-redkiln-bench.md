---
id: "kb-decision-c73f47cc9c"
title: "PR 5.6 moves Bench's briefs to .redkiln/bench/briefs, warns on an .anneal/ fallback read, and moves per-user state to ~/.redkiln/bench. May the 32 Bench tests pinning the old locations or empty stderr be rewritten to the new values (plus moving 2 snapshot tests whose old-path !exists check went vacuous), and may Bench's now-unused paths::anneal_home_from and its 3 tests be deleted, as skillcheck's were: A"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"PR 5.6 moves Bench's briefs to .redkiln/bench/briefs, warns on an .anneal/ fallback read, and moves per-user state to ~/.redkiln/bench. May the 32 Bench tests pinning the old locations or empty stderr be rewritten to the new values (plus moving 2 snapshot tests whose old-path !exists check went vacuous), and may Bench's now-unused paths::anneal_home_from and its 3 tests be deleted, as skillcheck's were?\", facing the question an agent raised, we decided for A."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-11-16"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-3ad82cba39"
question: "PR 5.6 moves Bench's briefs to .redkiln/bench/briefs, warns on an .anneal/ fallback read, and moves per-user state to ~/.redkiln/bench. May the 32 Bench tests pinning the old locations or empty stderr be rewritten to the new values (plus moving 2 snapshot tests whose old-path !exists check went vacuous), and may Bench's now-unused paths::anneal_home_from and its 3 tests be deleted, as skillcheck's were?"
door: two-way
blast_radius: file
decider: "human:ryan"
provenance: user-directed
decided_at: "2026-11-16T06:58:55Z"
tree_hash: "a408759f7f2e8b782c590727af5cf3229bf9e817"
---

# PR 5.6 moves Bench's briefs to .redkiln/bench/briefs, warns on an .anneal/ fallback read, and moves per-user state to ~/.redkiln/bench. May the 32 Bench tests pinning the old locations or empty stderr be rewritten to the new values (plus moving 2 snapshot tests whose old-path !exists check went vacuous), and may Bench's now-unused paths::anneal_home_from and its 3 tests be deleted, as skillcheck's were: A

## Context and problem statement

PR 5.6 moves Bench's briefs to .redkiln/bench/briefs, warns on an .anneal/ fallback read, and moves per-user state to ~/.redkiln/bench. May the 32 Bench tests pinning the old locations or empty stderr be rewritten to the new values (plus moving 2 snapshot tests whose old-path !exists check went vacuous), and may Bench's now-unused paths::anneal_home_from and its 3 tests be deleted, as skillcheck's were?

Raised by an agent (sweep) as a question and captured by Weigh-In as `wi-3ad82cba39`. Anchor: `crates/bench/bench/tests/it/brief.rs:51`.

## Decision outcome

Chosen option: **A**.

Decider's note: Ryan, decision card 2026-11-16: rewrite the 32 held tests as one class, move the 2 vacuous snapshot checks, delete Bench's anneal_home_from and its 3 tests

## Provenance

- Decided 2026-11-16T06:58:55Z by human:ryan (user-directed), via cli.
- Raised in session `22af9544-95b0-5794-a45f-15714e38e099`.
- Verbatim: "PR 5.6 moves Bench's briefs to .redkiln/bench/briefs, warns on an .anneal/ fallback read, and moves per-user state to ~/.redkiln/bench. May the 32 Bench tests pinning the old locations or empty stderr be rewritten to the new values (plus moving 2 snapshot tests whose old-path !exists check went vacuous), and may Bench's now-unused paths::anneal_home_from and its 3 tests be deleted, as skillcheck's were?"
