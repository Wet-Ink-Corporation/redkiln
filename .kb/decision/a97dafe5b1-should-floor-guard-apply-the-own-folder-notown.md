---
id: "kb-decision-a97dafe5b1"
title: "Fix floor-guard now, file the rsync/tar gap"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Should floor-guard apply the own-folder (NotOwn) check so a symlinked .redkiln with x/bench/ratchets.json is judged, and should the guard parse rsync/tar/unzip/interpreters writing into config folders?\", facing two gaps remain after #116: floor-guard does not apply the own-folder check to a symlinked .redkiln ratchets file, and the guard does not parse rsync, tar, unzip or interpreters writing into config folders, we decided for Fix floor-guard now, file the rsync/tar gap and neglected Both as issues for later; Fix both now, on the premise that floor-guard should stand alone without skip-rule, accepting that if wrong: one small PR of work."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-11-16"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-3e8eb7a535"
question: "Should floor-guard apply the own-folder (NotOwn) check so a symlinked .redkiln with x/bench/ratchets.json is judged, and should the guard parse rsync/tar/unzip/interpreters writing into config folders?"
door: two-way
blast_radius: module
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-11-16T15:43:42Z"
tree_hash: "79d9905a3cde0ce08ce92570fa198dd56beb6d4f"
recommended: "A"
flip_condition: "seeing agents write config via rsync or tar"
---

# Fix floor-guard now, file the rsync/tar gap

## Context and problem statement

Should floor-guard apply the own-folder (NotOwn) check so a symlinked .redkiln with x/bench/ratchets.json is judged, and should the guard parse rsync/tar/unzip/interpreters writing into config folders?

Two gaps remain after #116: floor-guard does not apply the own-folder check to a symlinked .redkiln ratchets file, and the guard does not parse rsync, tar, unzip or interpreters writing into config folders.

Raised by an agent (sweep) as a deferral and captured by Weigh-In as `wi-3e8eb7a535`. Anchor: `crates/bench/bench/src/floor_guard.rs:1`.

## Considered options

### A · Fix floor-guard now, file the rsync/tar gap (chosen)

Small PR adds NotOwn to floor-guard; a guard-kit issue tracks other writers.

- Holds if floor-guard should stand alone without skip-rule.
- If wrong: one small PR of work.

### B · Both as issues for later

File two issues, no code now.

- Holds if full review via skip-rule is enough meanwhile.
- If wrong: a ratchet slips if review misses it.

### C · Fix both now

Add NotOwn and parse rsync/tar/unzip in the guard.

- Holds if agents do use those tools on config.
- If wrong: guard parser work for little gain.

## Evidence

- `crates/bench/bench/src/floor_guard.rs`: ratchets judged from diff paths; working-tree read lacks NotOwn

## Decision outcome

Chosen option: **Fix floor-guard now, file the rsync/tar gap**, the recommended option.

Decider's note: Ryan, card 2026-11-16: fix floor-guard's own-folder (NotOwn) check now; file a guard-kit issue for rsync/tar/unzip/interpreter writes into config folders

### Consequences

- Good, because it holds if floor-guard should stand alone without skip-rule.
- Bad, because if wrong: one small PR of work.

### Confirmation

Revisit when: seeing agents write config via rsync or tar

## Why this might be wrong

the guard gap may matter more than floor-guard

## Provenance

- Decided 2026-11-16T15:43:42Z by human:ryan (user-approved), via cli.
- Raised in session `22af9544-95b0-5794-a45f-15714e38e099`.
- Verbatim: "Should floor-guard apply the own-folder (NotOwn) check so a symlinked .redkiln with x/bench/ratchets.json is judged, and should the guard parse rsync/tar/unzip/interpreters writing into config folders?"
