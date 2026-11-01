---
id: "kb-decision-wi-727fbf"
title: "Mark allow-invalid"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"May clippy.toml mark the tokio::sync::watch::Ref await-holding entry allow-invalid, silencing the unreachable-path warning (H-05, PR #121 commit 2)?\", facing every clippy run warns that tokio::sync::watch::Ref is unreachable, because tokio is built with rt only; silencing it edits lint config (H-05), we decided for Mark allow-invalid and neglected Keep the warning, on the premise that the entry should stay ready for when sync is enabled, accepting that if wrong: the warning no longer reminds anyone tokio lacks sync."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-11"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-727fbf"
question: "May clippy.toml mark the tokio::sync::watch::Ref await-holding entry allow-invalid, silencing the unreachable-path warning (H-05, PR #121 commit 2)?"
door: two-way
blast_radius: file
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-11T14:22:17Z"
tree_hash: "3ecee07cdae74527d16a1007e82348a986ee50fd"
recommended: "A"
flip_condition: "the warning is useful as a reminder"
---

# Mark allow-invalid

## Context and problem statement

May clippy.toml mark the tokio::sync::watch::Ref await-holding entry allow-invalid, silencing the unreachable-path warning (H-05, PR #121 commit 2)?

Every clippy run warns that tokio::sync::watch::Ref is unreachable, because tokio is built with rt only; silencing it edits lint config (H-05).

Raised by an agent (manual) as a question and captured by Weigh-In as `wi-727fbf`. Anchor: `clippy.toml:36`.

## Considered options

### A · Mark allow-invalid (chosen)

add allow-invalid = true, as the lazy_static entry has

- Holds if the entry should stay ready for when sync is enabled.
- If wrong: the warning no longer reminds anyone tokio lacks sync.

### B · Keep the warning

leave clippy.toml as is

- Holds if the warning is useful as a reminder.
- If wrong: every clippy run stays noisy.

## Evidence

- `clippy.toml:36`: changed on PR #121

## Decision outcome

Chosen option: **Mark allow-invalid**, the recommended option.

Decider's note: Approved on the decision card in the project thread (Merge all).

### Consequences

- Good, because it holds if the entry should stay ready for when sync is enabled.
- Bad, because if wrong: the warning no longer reminds anyone tokio lacks sync.

### Confirmation

Revisit when: the warning is useful as a reminder

## Why this might be wrong

the warning no longer reminds anyone tokio lacks sync

## Provenance

- Decided 2026-10-11T14:22:17Z by human:ryan (user-approved), via chat.
- Raised in session `ed0ca560-5ce5-5ac9-bb2b-9577c222272c`.
- Verbatim: "May clippy.toml mark the tokio::sync::watch::Ref await-holding entry allow-invalid, silencing the unreachable-path warning (H-05, PR #121 commit 2)?"
