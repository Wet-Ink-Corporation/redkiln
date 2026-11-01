---
id: "kb-decision-wi-a9df17"
title: "Count it once 10 are captured"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Does the exit criterion 'capture precision at least 80% on the eval seed' count as met when the real seed's bar rests on one captured item (1 of 1)?\", facing the bar passes on a single captured item, so it cannot tell 80% precision from 100% or 0%, we decided for Count it once 10 are captured and neglected Count it as met now; Judge noise by not-useful, on the premise that blocks become common enough to reach 10 within the two weeks, accepting that if wrong: at 1 block per 50 turns, v1 waits weeks on the seed."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-04"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-a9df17"
question: "Does the exit criterion 'capture precision at least 80% on the eval seed' count as met when the real seed's bar rests on one captured item (1 of 1)?"
door: two-way
blast_radius: module
decider: "human:ryan"
provenance: user-directed
decided_at: "2026-10-04T15:09:56Z"
tree_hash: "dc3cc0b7f1052c6b640c125db4ad4989a0477183"
recommended: "C"
flip_condition: "10+ captured items during #23 makes A cheap"
---

# Count it once 10 are captured

## Context and problem statement

Does the exit criterion 'capture precision at least 80% on the eval seed' count as met when the real seed's bar rests on one captured item (1 of 1)?

The bar passes on a single captured item, so it cannot tell 80% precision from 100% or 0%.

Raised by an agent (sweep) as a question and captured by Weigh-In as `wi-a9df17`. Anchor: `docs/v1-plan.md:252`.

## Decision drivers

- Delay to v1
- What it measures

## Considered options

### A · Count it once 10 are captured (chosen)

Keep the test; add blocks from #23's weeks to seed.jsonl; the criterion counts only when 10+ items are captured.

- Holds if blocks become common enough to reach 10 within the two weeks.
- If wrong: at 1 block per 50 turns, v1 waits weeks on the seed.

### B · Count it as met now

1 of 1 meets the bar; close #22 after the label review.

- Holds if noise is not the risk, recall is.
- If wrong: a capture change that raises noise goes unseen until not-useful climbs.

### C · Judge noise by not-useful

Replace the precision criterion with #23's not-useful rate (at most 10%), which measures noise in what reaches you; keep the seed test as a regression check.

- Holds if not-useful already measures what precision guards.
- If wrong: loses a capture-only signal, separate from triage.

## Evidence

- `eval/README.md`: precision is 100% on one captured item and recall is 14%
- `docs/v1-plan.md:252`: Marker capture precision at least 80% on the eval seed.
- `session s5, 2026-10-03T17:12`: This session's ledger is empty so far because I haven't been marking my own calls.

## Decision outcome

Chosen option: **Count it once 10 are captured**, overriding the recommendation (C).

Decider's note: Count once 10 captured

### Consequences

- Good, because it holds if blocks become common enough to reach 10 within the two weeks.
- Bad, because if wrong: at 1 block per 50 turns, v1 waits weeks on the seed.

### Confirmation

Revisit when: 10+ captured items during #23 makes A cheap

## Why this might be wrong

if capture noise matters apart from triage, C hides it

## Provenance

- Decided 2026-10-04T15:09:56Z by human:ryan (user-directed), via chat.
- Raised in session `6fda78c2-2ead-56e4-8e03-7d278eddd909`.
- Verbatim: "Does the exit criterion 'capture precision at least 80% on the eval seed' count as met when the real seed's bar rests on one captured item (1 of 1)?"
