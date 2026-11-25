---
id: "kb-decision-b559d96955"
title: "Fix + Windows lint leg"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"How should a Windows contributor get a green gate, given about 85 clippy findings that only appear on Windows (#158): box the large error types, or add a Windows clippy leg to CI?\", facing about 85 clippy findings appear only on Windows (result_large_err and others), so the local gate is red there while CI's Linux lint is green, we decided for Fix + Windows lint leg and neglected Baseline Windows; Linux is authoritative, on the premise that the fixes are mechanical, accepting that if wrong: a few days of churn across Core and kits."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-11-21"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-f06c27e875"
question: "How should a Windows contributor get a green gate, given about 85 clippy findings that only appear on Windows (#158): box the large error types, or add a Windows clippy leg to CI?"
door: two-way
blast_radius: system
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-11-21T04:44:09Z"
tree_hash: "dcd694bbf8be1896c5f3e900883bae2e1a6fc349"
recommended: "A"
flip_condition: "the fixes are not mechanical"
---

# Fix + Windows lint leg

## Context and problem statement

How should a Windows contributor get a green gate, given about 85 clippy findings that only appear on Windows (#158): box the large error types, or add a Windows clippy leg to CI?

About 85 clippy findings appear only on Windows (result_large_err and others), so the local gate is red there while CI's Linux lint is green.

Raised by an agent (marker) as a deferral and captured by Weigh-In as `wi-f06c27e875`. Anchor: `crates/core/redkiln-core/src/journal/refs`.

## Decision drivers

- Gate agreement
- Cost

## Considered options

### A · Fix + Windows lint leg (chosen)

Box the large error types and add a Windows clippy leg to CI

- Holds if the fixes are mechanical.
- If wrong: a few days of churn across Core and kits.

### B · Baseline Windows

Record the findings in temper's baseline for Windows

- Holds if they are platform noise.
- If wrong: real size regressions hide in the baseline.

### C · Linux is authoritative

Document that the local gate on Windows may be red; CI decides

- Holds if few contributors gate on Windows.
- If wrong: every Windows turn ends red.

## Evidence

- `#158`: Windows-only clippy findings

## Decision outcome

Chosen option: **Fix + Windows lint leg**, the recommended option.

### Consequences

- Good, because it holds if the fixes are mechanical.
- Bad, because if wrong: a few days of churn across Core and kits.

### Confirmation

Revisit when: the fixes are not mechanical

## Why this might be wrong

boxing errors touches public error types (API change)

## Provenance

- Decided 2026-11-21T04:44:09Z by human:ryan (user-approved), via chat.
- Raised in session `d55e53e3-870a-42df-88f6-aa37684d49c2`.
- Verbatim: "kind: deferral"
