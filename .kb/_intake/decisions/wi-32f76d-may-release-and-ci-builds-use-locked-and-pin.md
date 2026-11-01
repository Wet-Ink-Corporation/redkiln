---
id: "kb-decision-wi-32f76d"
title: "Lock and pin"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"May release and CI builds use --locked and pin the remaining actions to commit SHAs, leaving happenstance's requirement unpinned (PR #121 commit 3)?\", facing release builds resolved dependencies fresh and used moving action tags, so a release could build code CI never tested, we decided for Lock and pin and neglected Leave unpinned, on the premise that Cargo.lock plus --locked is enough to hold happenstance, accepting that if wrong: a dependency moves only through a reviewed lockfile change."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-11"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-32f76d"
question: "May release and CI builds use --locked and pin the remaining actions to commit SHAs, leaving happenstance's requirement unpinned (PR #121 commit 3)?"
door: two-way
blast_radius: system
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-11T14:22:17Z"
tree_hash: "3ecee07cdae74527d16a1007e82348a986ee50fd"
recommended: "A"
flip_condition: "convenience outweighs supply-chain risk"
---

# Lock and pin

## Context and problem statement

May release and CI builds use --locked and pin the remaining actions to commit SHAs, leaving happenstance's requirement unpinned (PR #121 commit 3)?

Release builds resolved dependencies fresh and used moving action tags, so a release could build code CI never tested.

Raised by an agent (manual) as a question and captured by Weigh-In as `wi-32f76d`. Anchor: `.github/workflows/release.yml:53`.

## Considered options

### A · Lock and pin (chosen)

--locked builds and tests, actions pinned to SHAs; happenstance requirement unpinned

- Holds if Cargo.lock plus --locked is enough to hold happenstance.
- If wrong: a dependency moves only through a reviewed lockfile change.

### B · Leave unpinned

keep moving tags and unlocked builds

- Holds if convenience outweighs supply-chain risk.
- If wrong: a release can build untested dependencies.

## Evidence

- `.github/workflows/release.yml:53`: changed on PR #121

## Decision outcome

Chosen option: **Lock and pin**, the recommended option.

Decider's note: Approved on the decision card in the project thread (Merge all).

### Consequences

- Good, because it holds if Cargo.lock plus --locked is enough to hold happenstance.
- Bad, because if wrong: a dependency moves only through a reviewed lockfile change.

### Confirmation

Revisit when: convenience outweighs supply-chain risk

## Why this might be wrong

a dependency moves only through a reviewed lockfile change

## Provenance

- Decided 2026-10-11T14:22:17Z by human:ryan (user-approved), via chat.
- Raised in session `ed0ca560-5ce5-5ac9-bb2b-9577c222272c`.
- Verbatim: "May release and CI builds use --locked and pin the remaining actions to commit SHAs, leaving happenstance's requirement unpinned (PR #121 commit 3)?"
