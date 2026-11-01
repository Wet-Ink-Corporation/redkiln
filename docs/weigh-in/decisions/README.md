# Decisions

Every decision behind Weigh-In so far, with where it was taken and who took it. The design phase
predates the tool, so these are kept here by hand: board decisions as one MADR record each, named
by their ledger id, and decisions taken in chat in [`build-decisions.md`](build-decisions.md).
In other repositories Weigh-In writes its own records to `.kb/_intake/decisions/`.

Settled decisions are not reopened without new evidence; when one is, the record says what
changed it and links the decision that replaced it.

## Settled before the design

Taken with Ryan on 2026-09-12, before the research (provenance: user-directed).

| Id | Question | Answer |
|---|---|---|
| D-0001 | What shape should the HITL review mechanism take? | A Redkiln-native decision item type, built on a portable Claude Code plugin core |
| Q1 | How much of your agent work runs unattended? | About half and half |
| Q2 | Where do records land outside Redkiln repos? | The same `.kb` layout everywhere |
| Q3 | Which models may judge one-way recommendations? | Anthropic models only |
| Q4 | Does Weigh-In stay as the name? | Keep Weigh-In |

## Board sitting, 2026-09-26/27

Seven cards on the Weigh-In Board ([artifact](https://claude.ai/artifact/5MzvjfqR9ZvaPAgNd8641q),
private to Ryan). Each chose the card's recommendation. The raw state, as presented and answered,
is in [`2026-09-27-board-sitting/state.json`](2026-09-27-board-sitting/state.json); the inputs to
the recommendation stability check (three permuted judge prompts per round, with their answer
keys) are in [`stability/`](2026-09-27-board-sitting/stability/).

| Id | Question | Decision | Door, authority | Date |
|---|---|---|---|---|
| [wi-b4c1d8](wi-b4c1d8.md) | Which runtime should the standalone Weigh-In CLI use, now that redkiln-rs is its eventual home? | A. Rust crate and CLI on redkiln-rs's toolchain (Cowork half amended by D-0002) | two-way, authority | 2026-09-27 |
| [wi-7c21e4](wi-7c21e4.md) | May Weigh-In publish ledger items from IQBG client repos to a claude.ai Artifact board? | A. Per-repo board mode; client repos default to local | one-way, authority | 2026-09-26 |
| [wi-6e2a47](wi-6e2a47.md) | Where should Weigh-In keep its live ledger so that moving into redkiln-rs later adds no source-control friction? | A. Mirror redkiln-rs: git for decisions, a common-dir store for open items | two-way, authority | 2026-09-27 |
| [wi-a93f02](wi-a93f02.md) | Should the ledger's event files be committed to git? | C. Commit in your repos; client repos local by default (commit half superseded by wi-6e2a47) | two-way, authority | 2026-09-26 |
| [wi-4d8b6a](wi-4d8b6a.md) | May unattended runs decide two-way, judgment-bound questions themselves and report them in a digest? | A. Yes for two-way items; one-way items block | two-way, authority | 2026-09-26 |
| [wi-e15c77](wi-e15c77.md) | Should Redkiln's `.kb` contract reserve `.kb/_ledger/` and accept Weigh-In's intake keys in slice 1? | A. Yes: reserve it and teach kb-ingest now | two-way, authority | 2026-09-26 |
| [wi-2b90f3](wi-2b90f3.md) | Should Weigh-In run reliance drills, occasionally marking a deliberately weaker option as recommended? | A. Yes, two-way items only, 1 in 30 for 60 days | two-way, authority | 2026-09-26 |

## Defaults taken on the board

Two-way, judgment-bound items the board decided without asking, listed so they can be reopened.

| Id | Question | Taken | Why | Status |
|---|---|---|---|---|
| wi-c3e8d0 | Registry MCP server, or a CLI over JSONL files? | CLI, no MCP server | A file fold is enough below a few hundred items, beads' storage change broke its tooling, and the board already spans clients without an MCP app. | Storage part superseded by wi-6e2a47 (an SQLite store through happenstance); still no MCP server |
| wi-9d4c62 | Capture with a model classifier on every Stop, or with agent markers? | Agent-emitted markers read by command hooks, zero model tokens | Prompt hooks call a model on every firing; automatic recaps elsewhere drew complaints about hidden cost and mislabelled status. | In force |
| wi-06e5bb | Hide every recommendation behind a reveal, or pre-select it when earned? | Pre-select only when earned; options first for one-way or unstable items | Pre-selected defaults stay strong whether disclosed or not, a bare verdict drew the lowest agreement, and options-first display costs about seven times the decision time. | In force |
| wi-5f0a19 | Which runtime for the CLI? | Node 20 or later, one dependency-free file | | Reopened by Ryan; superseded by wi-b4c1d8 |
| wi-81b7aa | How is the ledger stored? | One event file per session, ordered by Lamport clock, timestamp and event id | | Reopened by Ryan; superseded by wi-6e2a47 |

## After the sitting, 2026-09-27 to 2026-10-02

Details in [`build-decisions.md`](build-decisions.md).

| Id | Decision | Date | Provenance |
|---|---|---|---|
| [D-0002](build-decisions.md#d-0002) | Cowork runs the ledger through the computer's shell against the connected folder | 2026-09-28 | Ryan (amends wi-b4c1d8) |
| [D-0003](build-decisions.md#d-0003) | Cowork downloads the binary from a GitHub release, checked against a pinned SHA-256 (dormant while the repo is internal) | 2026-09-28 | Ryan, option 1 of 3 |
| [D-0004](build-decisions.md#d-0004) | The command is `/weigh-in`: plugin `weigh`, skill `weigh-in` | 2026-09-28 | Ryan |
| [D-0005](build-decisions.md#d-0005) | One skill with arguments; no split yet | 2026-09-28 | Ryan |
| [D-0006](build-decisions.md#d-0006) | Distribute through the local marketplace for now; Cowork gets uploaded zips | 2026-09-28 | Ryan |
| [D-0007](build-decisions.md#d-0007) | Weigh-In lives in `Wet-Ink-Corporation/weigh-in`, internal | 2026-10-01 | Ryan |
| [D-0008](build-decisions.md#d-0008) | v1 means daily use by Ryan | 2026-10-02 | Ryan |
| [D-0009](build-decisions.md#d-0009) | Claude Code installs from a built `marketplace` branch in this internal repository (amends D-0006) | 2026-10-02 | Ryan |
| [D-0010](build-decisions.md#d-0010) | This repository uses Weigh-In on itself | 2026-10-02 | Ryan |
| [D-0011](build-decisions.md#d-0011) | Open work lives in GitHub Issues | 2026-10-02 | Ryan |
| [D-0012](build-decisions.md#d-0012) | Release assets stay internal for v1; one repository | 2026-10-02 | Ryan |
| [D-0013](build-decisions.md#d-0013) | No slice 2 work in v1, unless not-useful exceeds 10% | 2026-10-02 | Ryan |
| [B-01](build-decisions.md#b-01) | The ledger runs under a TRUNCATE journal | 2026-09-28 | Forced by Cowork's refused deletes |
| [B-02](build-decisions.md#b-02) | The ledger location is settable | 2026-09-28 | Claude, two-way |
| [B-03](build-decisions.md#b-03) | Plugin binaries live in `libexec/`, not `bin/` | 2026-09-28 | Forced by claude.ai's plugin validation |
| [B-04](build-decisions.md#b-04) | Hooks write nothing in Cowork | 2026-09-28 | Claude, two-way |
| [B-05](build-decisions.md#b-05) | One binary version at a time | 2026-09-28 | Claude, two-way |
| [B-06](build-decisions.md#b-06) | Seeded as one commit on GitHub's initial commit; dual licence files; no download stamp unless public | 2026-10-01 | Claude, two-way |
| [B-07](build-decisions.md#b-07) | CI runs the gate on every push to `main` and every pull request; Dependabot for actions and Cargo | 2026-10-01 | Claude, two-way |
| [B-08](build-decisions.md#b-08) | A path is in the repository when its resolved form is under the resolved toplevel | 2026-10-01 | Claude, two-way |
| [B-09](build-decisions.md#b-09) | Phase 0 repository setup: changelog-driven releases, macOS, plugin and cargo-deny CI | 2026-10-02 | Claude, two-way |
