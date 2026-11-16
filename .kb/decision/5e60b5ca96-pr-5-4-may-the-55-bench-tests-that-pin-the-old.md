---
id: "kb-decision-5e60b5ca96"
title: "PR 5.4: may the 55 Bench tests that pin the old decision-log location (<home>/decisions/...), 3 goldens' log paths, and empty stderr be rewritten as one class to <home>/calibrate/bench/ and the D1 warning line, as D1 implies: A"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"PR 5.4: may the 55 Bench tests that pin the old decision-log location (<home>/decisions/...), 3 goldens' log paths, and empty stderr be rewritten as one class to <home>/calibrate/bench/ and the D1 warning line, as D1 implies?\", facing the question an agent raised, we decided for A."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-11-16"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-f3ba2ae601"
question: "PR 5.4: may the 55 Bench tests that pin the old decision-log location (<home>/decisions/...), 3 goldens' log paths, and empty stderr be rewritten as one class to <home>/calibrate/bench/ and the D1 warning line, as D1 implies?"
door: two-way
blast_radius: module
decider: "human:ryan"
provenance: user-directed
decided_at: "2026-11-16T03:51:09Z"
tree_hash: "f27e861819bd2c1b6ca7bf3883737a448c98ff40"
---

# PR 5.4: may the 55 Bench tests that pin the old decision-log location (<home>/decisions/...), 3 goldens' log paths, and empty stderr be rewritten as one class to <home>/calibrate/bench/ and the D1 warning line, as D1 implies: A

## Context and problem statement

PR 5.4: may the 55 Bench tests that pin the old decision-log location (<home>/decisions/...), 3 goldens' log paths, and empty stderr be rewritten as one class to <home>/calibrate/bench/ and the D1 warning line, as D1 implies?

Raised by an agent (sweep) as a question and captured by Weigh-In as `wi-f3ba2ae601`. Anchor: `crates/bench/bench/src/decide/log.rs`.

## Decision outcome

Chosen option: **A**.

Decider's note: Ryan, decision card 2026-11-16: rewrite the 55 tests as one class (paths to <home>/calibrate/bench/, re-bless 3 goldens, expect D1 warning); any test pinning more than location/warning comes back

## Provenance

- Decided 2026-11-16T03:51:09Z by human:ryan (user-directed), via cli.
- Raised in session `22af9544-95b0-5794-a45f-15714e38e099`.
- Verbatim: "PR 5.4: may the 55 Bench tests that pin the old decision-log location (<home>/decisions/...), 3 goldens' log paths, and empty stderr be rewritten as one class to <home>/calibrate/bench/ and the D1 warning line, as D1 implies?"
