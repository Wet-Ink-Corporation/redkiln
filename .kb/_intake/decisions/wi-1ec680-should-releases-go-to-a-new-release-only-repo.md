---
id: "kb-decision-wi-1ec680"
title: "Release-only repo"
kind: decision
status: accepted
authority_tier: decision
summary: "In the context of \"Should releases go to a new release-only repo (Wet-Ink-Corporation/weigh-in-plugin) that the org syncs, reversing D-0012's \"no second repository\"?\", facing claude.ai's org sync reads only the default branch and fires only on pushes to it, so a release on the marketplace branch never reaches users by itself, we decided for Release-only repo and neglected #79 pointer on main; Status quo: upload zip, on the premise that a deploy key or App token can be set up once, accepting that if wrong: delete the repo and switch the sync back; nothing in main changes."
depends_on: []
related: []
source_paths: []
last_reviewed: "2026-10-05"
reversibility: high
phase: null
supersedes: []
superseded_by: null
weighin_item: "wi-1ec680"
question: "Should releases go to a new release-only repo (Wet-Ink-Corporation/weigh-in-plugin) that the org syncs, reversing D-0012's \"no second repository\"?"
door: two-way
blast_radius: system
decider: "human:ryan"
provenance: user-approved
decided_at: "2026-10-05T16:27:01Z"
tree_hash: "8b56c841f92ecbfa68905ae68813e9735010462c"
recommended: "A"
flip_condition: "the sync honouring a ref and firing on marketplace-branch pushes"
---

# Release-only repo

## Context and problem statement

Should releases go to a new release-only repo (Wet-Ink-Corporation/weigh-in-plugin) that the org syncs, reversing D-0012's "no second repository"?

claude.ai's org sync reads only the default branch and fires only on pushes to it, so a release on the marketplace branch never reaches users by itself.

Raised by an agent (marker) as a question and captured by Weigh-In as `wi-1ec680`. Anchor: `docs/decisions/build-decisions.md:117`.

## Decision drivers

- Manual step per release
- Unverified dependencies
- main's dev loop

## Considered options

### A · Release-only repo (chosen)

New private repo weigh-in-plugin; release.yml pushes the one-commit build to its default branch; org syncs it with auto-sync and installed-by-default.

- Holds if a deploy key or App token can be set up once.
- If wrong: delete the repo and switch the sync back; nothing in main changes.

### B · #79 pointer on main

main's entry is git-subdir ref=marketplace; Sync now after each release.

- Holds if the sync follows a ref to another branch (unverified).
- If wrong: synced plugin stays the dev copy; close #79.

### C · Status quo: upload zip

Remove the org sync of weigh-in; upload each release's zip by hand (D-0012 stands).

- Holds if releases are rare enough that a manual upload is fine.
- If wrong: forgotten uploads leave users on old versions.

## Evidence

- `docs/decisions/build-decisions.md:119`: No second repository and no public assets.
- `https://claude.com/docs/plugins/org-sync`: reads the marketplace repository's default branch; Sync automatically syncs each time someone pushes to the default branch
- `.github/workflows/release.yml:118`: push --force ... HEAD:refs/heads/marketplace

## Decision outcome

Chosen option: **Release-only repo**, the recommended option.

Decider's note: Release-only repo; supersedes D-0012's no-second-repository clause

### Consequences

- Good, because it holds if a deploy key or App token can be set up once.
- Bad, because if wrong: delete the repo and switch the sync back; nothing in main changes.

### Confirmation

Revisit when: the sync honouring a ref and firing on marketplace-branch pushes

## Why this might be wrong

if Anthropic adds branch selection to org sync, a second repo is overhead

## Provenance

- Decided 2026-10-05T16:27:01Z by human:ryan (user-approved), via chat.
- Raised in session `cf1aaf47-19e1-571d-ba4e-d8b3943fc609`.
- Verbatim: "kind: question"
