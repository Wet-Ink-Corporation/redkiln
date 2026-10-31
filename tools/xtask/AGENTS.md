# Crate charter: `xtask`

## Purpose
Repository tasks for the redkiln workspace, run as `cargo xtask <task>` (alias in `.cargo/config.toml`). It is the blueprint's `tools/xtask`: a dev tool, never shipped in a plugin. Today it has one task, `layers`, the structural test of the layer rules in the root `AGENTS.md`. Packaging into `libexec/`, version agreement, changelog and marketplace assembly join it as the moves need them.

## Layer
Tool (`tools/`). A library (`src/lib.rs`, the tasks) plus a thin binary (`src/main.rs`, argument parsing and exit codes).

## Allowed dependencies
- Workspace crates: Core, kits and contract crates, once they exist. Never a plugin's internal crates.
- Third-party: `serde` (derive, for the `cargo metadata` DTOs), `serde_json` (to read that output), `thiserror` (`ReadError`). All three are on `.claude/temper.json`'s pre-approved list.
- Never: `cargo_metadata` or other crates for what `cargo metadata` and serde already give; `anyhow` in `src/lib.rs`.

## Public surface
`xtask::layers`: `Package`, `Dependency`, `Kind`, `Violation`, `check`, `read_workspace`, `ReadError`. Workspace-internal; nothing is published.

## Invariants
- `check` is pure: the same packages give the same violations, in member order.
- The layer of a member comes only from its directory (and, under `crates/<plugin>/`, from a `-contract` name suffix). The plugin folder list (`PLUGINS`) is the one place a new plugin is admitted; changing it is a crate-boundary change (H-16).
- Dev and build dependencies are held to the same layer rules as normal ones.

## Errors
`ReadError`: `Spawn` (cargo could not start), `Cargo` (it failed; carries its status and stderr), `Parse` (output was not the metadata expected), `OutsideWorkspace` and `NonUtf8` (a member's directory cannot be placed). Layer violations are values (`Violation`), not errors.

## Test strategy
Integration tests in `tests/it/` against synthetic workspaces, one per rule, plus one that reads this workspace through `cargo metadata`. Run with `cargo test -p xtask`.

## Do not
- Do not relax a rule here to admit a crate; change `AGENTS.md`'s layer rules first, as an H-16 decision, then the code and its tests.

## Decisions
| Date | Decision | Rule | Decided by |
|---|---|---|---|
| 2026-10-30 | Core crates may depend on other Core crates (the blueprint's "nothing internal" read as "nothing outside Core"), so `redkiln-calibrate` and `redkiln-capacity` can use `redkiln-core` | ARC-02 | phase 0 skeleton, Ryan to confirm |
