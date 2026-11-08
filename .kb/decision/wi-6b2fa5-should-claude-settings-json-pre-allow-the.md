---
id: "kb-decision-wi-6b2fa5"
title: "Pre-allow the launcher"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Should .claude/settings.json pre-allow the weighin launcher so /weigh-in's injected ledger command runs in cloud sessions, where the auto-mode permission check gave no verdict?\", facing the skill's ! injection failed: the auto-mode classifier gave no verdict, we decided for Pre-allow the launcher and neglected Leave it, on the premise that the permission check was the only blocker, accepting that if wrong: the injection still fails; one line to drop."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-05"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-6b2fa5"
question: "Should .claude/settings.json pre-allow the weighin launcher so /weigh-in's injected ledger command runs in cloud sessions, where the auto-mode permission check gave no verdict?"
door: two-way
blast_radius: module
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-05T03:16:12Z"
tree_hash: "a38d454ee657f965c1136909d7588b111ebda300"
recommended: "A"
flip_condition: "the injection still fails with the rule"
---

# Pre-allow the launcher

## Context and problem statement

Should .claude/settings.json pre-allow the weighin launcher so /weigh-in's injected ledger command runs in cloud sessions, where the auto-mode permission check gave no verdict?

The skill's ! injection failed: the auto-mode classifier gave no verdict.

Raised by an agent (sweep) as a question and captured by Weigh-In as `wi-6b2fa5`. Anchor: `.claude/settings.json:68`.

## Decision drivers

- Cost

## Considered options

### A · Pre-allow the launcher (chosen)

Add the weighin launcher to permissions.allow.

- Holds if the permission check was the only blocker.
- If wrong: the injection still fails; one line to drop.

### B · Leave it

The skill falls back to Bash.

- Holds if one extra call per sitting is fine.
- If wrong: every cloud sitting starts with an error.

## Evidence

- `.claude/settings.json:68`: permissions.allow

## Decision outcome

Chosen option: **Pre-allow the launcher**, the recommended option.

Decider's note: Pre-allow the launcher

### Consequences

- Good, because it holds if the permission check was the only blocker.
- Bad, because if wrong: the injection still fails; one line to drop.

### Confirmation

Revisit when: the injection still fails with the rule

## Why this might be wrong

auto mode may ignore allow rules for injected commands

## Provenance

- Decided 2026-10-05T03:16:12Z by human:ryan (user-approved), via chat.
- Raised in session `2b46cfdc-83e0-50b2-8dc4-18b1835adc3f`.
- Verbatim: "Should .claude/settings.json pre-allow the weighin launcher so /weigh-in's injected ledger command runs in cloud sessions, where the auto-mode permission check gave no verdict?"
