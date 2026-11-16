---
id: "kb-decision-0c9f04eb9a"
title: "After 5.4, should a relative $ANNEAL_HOME still stop skillcheck's eval uplift (it reads nothing there now), and should skillcheck's unused anneal_home_from and its tests be removed? Both change or delete tests.: A"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"After 5.4, should a relative $ANNEAL_HOME still stop skillcheck's eval uplift (it reads nothing there now), and should skillcheck's unused anneal_home_from and its tests be removed? Both change or delete tests.\", facing the question an agent raised, we decided for A."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-11-16"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-ad764d8531"
question: "After 5.4, should a relative $ANNEAL_HOME still stop skillcheck's eval uplift (it reads nothing there now), and should skillcheck's unused anneal_home_from and its tests be removed? Both change or delete tests."
door: two-way
blast_radius: file
decider: "human:ryan"
provenance: user-directed
decided_at: "2026-11-16T05:56:05Z"
tree_hash: "7bfa9f247a08d15154dd638f9ad5bf5a083de2e2"
---

# After 5.4, should a relative $ANNEAL_HOME still stop skillcheck's eval uplift (it reads nothing there now), and should skillcheck's unused anneal_home_from and its tests be removed? Both change or delete tests.: A

## Context and problem statement

After 5.4, should a relative $ANNEAL_HOME still stop skillcheck's eval uplift (it reads nothing there now), and should skillcheck's unused anneal_home_from and its tests be removed? Both change or delete tests.

Raised by an agent (sweep) as a deferral and captured by Weigh-In as `wi-ad764d8531`. Anchor: `crates/core/redkiln-calibrate/src/scope.rs:114`.

## Decision outcome

Chosen option: **A**.

Decider's note: Ryan, decision card 2026-11-16: fold F5 (relative ANNEAL_HOME no longer stops skillcheck; its test's expectation changes) and F6 (delete unused anneal_home_from and its tests) into 5.6

## Provenance

- Decided 2026-11-16T05:56:05Z by human:ryan (user-directed), via cli.
- Raised in session `22af9544-95b0-5794-a45f-15714e38e099`.
- Verbatim: "After 5.4, should a relative $ANNEAL_HOME still stop skillcheck's eval uplift (it reads nothing there now), and should skillcheck's unused anneal_home_from and its tests be removed? Both change or delete tests."
