# Redkiln Rust 2.0 runbook: temper (draft)

temper becomes Redkiln Rust, `rk-rust` 2.0. It differs from the other moves: temper is a released product (1.x, Node), so it keeps shipping 1.x from its own repository until 2.0 cuts over, and its Rust port is finished here rather than imported finished. temper finished its move preparation on 2026-11-08 and has been frozen by Ryan since then.

temper wrote what only it knows in its import manifest, temper `docs/plans/redkiln-move.md`: where each path goes, what stays behind, what this root must provide, the state imported once and its issue triage. This runbook does not repeat it. It adds what this repository decides: when the import happens, what the import PR must solve here, and the calls still open. It is a **draft**, re-checked when the import is next.

Facts below were checked on 2026-11-08 against temper `main` at `ab78a31` (1.6.0 released; `2.0.0-alpha.2` built from the port) and redkiln `main` at `d80e7fb`.

## Decided (Ryan, 2026-11-08, temper's Weigh-In sitting)

The open flag from the merge briefing (the port was being built in temper's repository, not here as the migration plan said) is settled:

| Call | Decision | Record |
| --- | --- | --- |
| Where the port lands | Read-tree import of temper `main`, history kept; the port's four crates come as they are after Phase 3 | temper wi-187aa7 |
| The port's progress | Paused after Phase 3 (temper #211); Phases 4 to 8 are built here on Core 1.0, after Move 2 | temper wi-14cf95 |
| Crate split (D3) | The language-neutral half goes to the kits: completion gate, verdict validation, fingerprint, baseline ratchet, gate cache to `redkiln-craft`; shell reader and protected globs to `redkiln-guard`. Scanner, lexer, manifest rules and cargo steps stay in Rust's crates. temper's `cargo xtask check-seams` keeps those modules reaching only what the kit will hold | temper wi-59cb6e (#213) |
| Install (D1, D13, D14, D17) | npm retired; builds in `libexec/<target>/` behind the shared launcher | temper wi-02195d, wi-12d8ef |
| crates.io (D2) | Not published; names kept | temper wi-0eea27 |
| MSRV (D4) | 1.98.1 from the import | temper wi-a2240a |
| Dependencies (D10) | The suite's set (`redkiln-core`, `clap`, `time`) from the import. The `toml` and `sha2` version splits are settled separately: the root steps up before Move 2 (Ryan, 2026-11-08) | temper wi-fa59d1 |
| Old state (Invariant 3) | 2.0 reads 1.x state until it imports it once into the shared log | temper wi-f4c3ac |

These amend the migration plan's "Not here: any Rust code for 2.0. The port starts in the monorepo" and "the Node 1.x is not imported as code": the whole of temper `main` is imported, and the Node engine lands at `tools/rust/engine-1x/` as the parity oracle until the cutover deletes it (D11).

## When

The import's gate is Core 1.0, which freezes at Move 2. **Decided (Ryan, 2026-11-08): the import comes right after Move 2, before Move 3** (`docs/decisions.md`). Even so, Rust's move has a long middle: the import (steps 1 to 6) is quick, then Phases 4 to 8 are built here, and only the cutover releases `rk-rust` 2.0.0, transfers the issues and archives temper (steps 7 and 8). "One product in motion" applies to the import itself; the port's phases run beside the next move, as the migration plan always intended ("the port runs alongside moves 3 and 4").

## What the import PR must solve here

| # | Item | Default (two-way unless marked) |
| --- | --- | --- |
| R-1 | **Hooks fail `cargo xtask plugins`.** temper's `hooks/hooks.json` runs `node ${CLAUDE_PLUGIN_ROOT}/scripts/temper.mjs hook <event>`, and the manifest places `scripts/` at `tools/rust/engine-1x/`. At `plugins/rust/` that is a hook the check refuses and a path that does not exist | The door commit points every hook at `${CLAUDE_PLUGIN_ROOT}/libexec/temper hook <event>` with a launcher copy. Nothing ships `rk-rust` before 2.0, and with no build the launcher fails open. The port's binary answers every hook but `stop` and `subagent-stop` today (`RUST_READY` in temper's `scripts/launch.mjs`; both graduate in Phase 4) |
| R-2 | **Root dependency gaps** (manifest, "What the redkiln root must provide"): `toml` 1.1 against 0.9.12, `sha2` 0.11 against 0.10, no `regex`, `wait-timeout` or `proptest`, `serde_json`'s `float_roundtrip` (which Cargo unifies for every crate) | One-way (H-16). The version splits are settled: the root steps up to `toml` 1.1 and `sha2` 0.11 before Move 2 (Ryan, 2026-11-08). The rest are rows in the import PR |
| R-3 | **1.x after the import.** temper keeps shipping 1.x, but its tree is now also here as the oracle | From the import on, 1.x takes fixes only; each fix lands in temper and, in the same sitting, in `tools/rust/engine-1x/`, so the oracle never lags the release users run |
| R-4 | **The root's vendored gate.** This repository runs temper 1.4.0 at `.claude/temper/`; temper is at 1.6.0 | Ryan's call, since it changes migration call E (open calls in [README.md](README.md)). Recommended: bump the root copy to the latest 1.x in its own PR, after Move 1 closes and before Move 2's re-baseline, so Bench is baselined once under the newer engine |
| R-5 | **Kits.** `redkiln-guard` is first cut at Move 2 (from anneal); `redkiln-craft` is born with Rust | The kit-bound modules move out after the import is green, one PR per kit, never in the import |
| R-6 | **Lens plugin.** `plugins/temper-lens/` becomes `plugins/rust-lens/`, a mod plugin with its own launcher copy (as `rk-weigh-in-mod`) | As the manifest says |

## Open questions temper left (Weigh-In, temper repository)

temper is frozen, so these are answered at the import, each through Weigh-In:

| Id | Question | Bears on |
| --- | --- | --- |
| wi-b1bfb5 | What replaces the `temper-telemetry` branch? The plan's answer is SPEC journals; the open part is whether 1.x's branch is kept anywhere | Rust's events |
| wi-39f19a | temper #193: a GHCR artifact for assay's pinned gates, or Review pins temper inside the monorepo | Move 3, code work step 4 |
| wi-0924e6 | temper #172 and #173 (the `check` subcommands and `gate --base --findings-out` Review calls): built in 1.x, or in 2.0 | Review's temper adapter, which re-implements them today |

## Done when

As the migration plan: at parity, `rk-rust` 2.0.0 ships from this repository, the root gate switches from the vendored temper to it, the Node oracle and `tools/rust/parity/` are deleted, the last 1.x release prints a SessionStart notice naming `rk-rust`, and temper is archived with its keepers transferred.
