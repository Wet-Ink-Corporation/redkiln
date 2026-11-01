---
id: "kb-decision-wi-680975"
title: "File the issue"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Should I attach Wet-Ink-Corporation/temper and file an issue that its SubagentStart/SubagentStop hooks never fire in cloud sessions, so reviewer verdicts are never recorded?\", facing temper's SubagentStart/SubagentStop hooks never fired here, so no reviewer verdict was recorded across six runs and its review check cannot pass, we decided for File the issue and neglected Don't file, on the premise that temper is meant to work in cloud sessions, accepting that if wrong: one closable issue."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-09"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-680975"
question: "Should I attach Wet-Ink-Corporation/temper and file an issue that its SubagentStart/SubagentStop hooks never fire in cloud sessions, so reviewer verdicts are never recorded?"
door: two-way
blast_radius: external
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-09T04:52:38Z"
tree_hash: "ef4f1136eaf287a5afb6c08a75db1348d2620acc"
recommended: "A"
flip_condition: "temper already tracks it"
---

# File the issue

## Context and problem statement

Should I attach Wet-Ink-Corporation/temper and file an issue that its SubagentStart/SubagentStop hooks never fire in cloud sessions, so reviewer verdicts are never recorded?

temper's SubagentStart/SubagentStop hooks never fired here, so no reviewer verdict was recorded across six runs and its review check cannot pass.

Raised by an agent (marker) as a question and captured by Weigh-In as `wi-680975`. Anchor: `.git/temper/telemetry.jsonl:1`.

## Decision drivers

- Cost

## Considered options

### A · File the issue (chosen)

Attach Wet-Ink-Corporation/temper and post the evidence

- Holds if temper is meant to work in cloud sessions.
- If wrong: one closable issue.

### B · Don't file

Accept in-chat approvals; leave temper as is

- Holds if cloud Rust sessions here are rare.
- If wrong: each one loops on reviewers and ends BLOCKED.

## Evidence

- `.git/temper/telemetry.jsonl:1`: no subagent or verdict events across six reviewer runs
- `.git/temper/verdicts/`: directory never created

## Decision outcome

Chosen option: **File the issue**, the recommended option.

Decider's note: file the temper issue

### Consequences

- Good, because it holds if temper is meant to work in cloud sessions.
- Bad, because if wrong: one closable issue.

### Confirmation

Revisit when: temper already tracks it

## Why this might be wrong

the cause may be the cloud host, not temper

## Provenance

- Decided 2026-10-09T04:52:38Z by human:ryan (user-approved), via chat.
- Raised in session `a30b6003-c528-5711-b5a2-08effa8e852d`.
- Verbatim: "kind: question"
