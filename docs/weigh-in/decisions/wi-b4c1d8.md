---
id: wi-b4c1d8
title: "Which runtime should the standalone Weigh-In CLI use, now that redkiln-rs is its eventual home?"
status: accepted; Cowork half amended by D-0002; destination (redkiln-rs) superseded by B-36
date: 2026-09-27
decision: "A - Rust crate and CLI on redkiln-rs's toolchain"
door: two-way
authority: authority
blast: system
provenance: user-approved (matched the recommendation)
decided_on: Weigh-In Board sitting, 2026-09-26/27
reopens: wi-5f0a19
blocks: [wi-6e2a47]
---

# Which runtime should the standalone Weigh-In CLI use, now that redkiln-rs is its eventual home?

## Context and problem statement

The CLI is the single writer of the ledger and the piece redkiln-rs will absorb. Writing it in redkiln-rs's language decides whether integration adopts code or re-implements it.

Door: two-way. A small CLI can be rewritten, but every week on the wrong runtime adds code that is thrown away at integration.

Timing: Decide now. Slice 1 cannot start without it, and the only unknown (whether a prebuilt binary runs in each surface) is a 20-minute spike that the recommendation already schedules.

Resolve by: before slice 1 starts (it is the first line of code).

This reopened the default wi-5f0a19. Ryan's note:

> redkiln has a newer version (redkiln-rs), written in Rust, which is still in active development. This will be the eventual home of the weigh-in system - but it needs to be able to stand on it's own until redkiln-rs is ready to receive it

Evidence:

- [your reopen note] redkiln-rs will be the eventual home of Weigh-In; Weigh-In must stand on its own until then.
- [redkiln-rs/Cargo.toml:21,25] Rust, edition 2024, rust-version 1.98.1, pinned exactly in rust-toolchain.toml.
- [operator-surface-proposal.md §12.1, §12.3] happenstance 0.3.2 (your DCB event store, MSRV 1.97.1) is proposed as redkiln-rs's tier-2 store adapter, with git staying the tier-1 authority.
- [ADR-0077] redkiln-rs reads git through gix and shells out for every git write, the same split a Weigh-In CLI needs.
- [plugin platform support] Cowork cannot install a plugin's bin/ executables; a binary inside a skill folder run through Bash is unverified.
- [ADR-0164] Your box runs core.autocrlf=true; redkiln-rs forces LF in every checkout, so whatever writes ledger files must write LF.

## Considered options

### A. Rust crate and CLI on redkiln-rs's toolchain (chosen)

A weighin-core crate plus a weighin binary, pinned to Rust 1.98.1, edition 2024, events stored through happenstance. Prebuilt binaries for Windows x64 and Linux x64 ship in the plugin. Cowork, which has no repo, keeps its ledger in the board's store and needs no binary.

- Holds if: a prebuilt binary runs from the plugin folder on Windows and in Claude Code cloud sessions (20-minute spike)
- If wrong: you maintain release builds for two targets for a small tool; the core crate is still what redkiln-rs adopts

### B. Node single file now, port to Rust at integration

The original default: one dependency-free Node file, run the same way everywhere Node runs.

- Holds if: the data contract, not the code, is what carries into redkiln-rs
- If wrong: about 1,500 lines written twice, and two implementations of the ID and fold rules while both are live

### C. Rust core compiled to WebAssembly behind a Node shim

One .wasm artifact runs wherever Node runs; the same crate links natively into redkiln-rs.

- Holds if: the shared logic is pure: git and process calls stay in the shim
- If wrong: git, rg and file watching live in JavaScript anyway, splitting the logic you meant to share; node:wasi is still experimental

| Criterion | A | B | C |
|---|---|---|---|
| Code redkiln-rs can adopt | The crate | None | The core |
| Runs in Cowork | Board store, no binary | Yes, via Node | Yes, via Node |
| Build and release work | Two targets | None | One wasm plus shim |
| Same stack as redkiln-rs | Yes | No | Partly |

## Decision outcome

Chosen: **A. Rust crate and CLI on redkiln-rs's toolchain**, decided 2026-09-27T04:28:36Z.

The card's recommendation was A (recommended and pre-selected).
Trade-off: You are trading release plumbing for two targets for code that redkiln-rs adopts rather than rewrites.
What would flip it: The spike shows a prebuilt binary will not run from a plugin in Claude Code cloud sessions or on your Windows install. Then B, keeping the event schema identical so the port is mechanical.
Why it might be wrong: redkiln-rs's own ports may change shape before integration. A crate written now against happenstance may still need an adapter, so adopting the crate could shrink to adopting its data.
Stability: 4 of 4 (Opus pick plus 3 Sonnet samples, each with the option order permuted and the recommendation removed). All samples judged options written by the same author, so this checks order and framing bias, not authorship bias.

## Later changes

The Cowork half ("board store, no binary") is amended by [D-0002](build-decisions.md#d-0002): when the computer's shell can reach the binary, Cowork runs it there against the connected folder; the board stays the fallback.

## More information

Card as presented and answered: [`2026-09-27-board-sitting/state.json`](2026-09-27-board-sitting/state.json). Design context: [the report](../research/report.md).
