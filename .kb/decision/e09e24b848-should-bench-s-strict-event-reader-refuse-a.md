---
id: "kb-decision-e09e24b848"
title: "Refuse repeated keys"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Should Bench's strict event reader refuse a repeated key inside a record's data object, as assay's does? That changes how stored events read (#135's last item).\", facing assay's strict reader refuses a repeated key in a record's data object; Bench's does not, so the two readers disagree on the same bytes, we decided for Refuse repeated keys and neglected Keep accepting, on the premise that no stored bench.* event has duplicates, accepting that if wrong: an old event becomes unreadable and needs a repair import."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-11-21"
reversibility: low
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-c72eab2538"
question: "Should Bench's strict event reader refuse a repeated key inside a record's data object, as assay's does? That changes how stored events read (#135's last item)."
door: one-way
blast_radius: module
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-11-21T04:44:09Z"
tree_hash: "dcd694bbf8be1896c5f3e900883bae2e1a6fc349"
recommended: "A"
flip_condition: "a stored event with a duplicate key turns up"
---

# Refuse repeated keys

## Context and problem statement

Should Bench's strict event reader refuse a repeated key inside a record's data object, as assay's does? That changes how stored events read (#135's last item).

assay's strict reader refuses a repeated key in a record's data object; Bench's does not, so the two readers disagree on the same bytes.

Raised by an agent (marker) as a question and captured by Weigh-In as `wi-c72eab2538`. Anchor: `crates/bench/bench`.

## Decision drivers

- Old events
- Reader parity

## Considered options

### A · Refuse repeated keys (chosen)

Bench's reader refuses a duplicate key in data, as assay's does

- Holds if no stored bench.* event has duplicates.
- If wrong: an old event becomes unreadable and needs a repair import.

### B · Keep accepting

Leave the reader as is; close the #135 item as won't-do

- Holds if no hand-written or foreign events reach Bench.
- If wrong: an ambiguous record reads differently in Bench and assay.

## Evidence

- `#135`: last open item: strict reader repeated-key parity with assay

## Decision outcome

Chosen option: **Refuse repeated keys**, the recommended option.

### Consequences

- Good, because it holds if no stored bench.* event has duplicates.
- Bad, because if wrong: an old event becomes unreadable and needs a repair import.

### Confirmation

Revisit when: a stored event with a duplicate key turns up

## Why this might be wrong

an external writer may already emit duplicates

## Provenance

- Decided 2026-11-21T04:44:09Z by human:ryan (user-approved), via chat.
- Raised in session `d55e53e3-870a-42df-88f6-aa37684d49c2`.
- Verbatim: "kind: question"
