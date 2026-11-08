# Crate charter: `xtask`

## Purpose
Repository tasks for the redkiln workspace, run as `cargo xtask <task>` (alias in `.cargo/config.toml`). It is the blueprint's `tools/xtask`: a dev tool, never shipped in a plugin. It has three tasks: `layers`, the structural test of the layer rules in the root `AGENTS.md`; `plugins`, the structural test of its CLI-first plugin rules; and `ids`, the check of `.kb/` against the knowledge-base id rules (Knowledge call C). Packaging into `libexec/`, version agreement, changelog and marketplace assembly join it as the moves need them.

## Layer
Tool (`tools/`). A library (`src/lib.rs`, the tasks) plus a thin binary (`src/main.rs`, argument parsing and exit codes).

## Allowed dependencies
- Workspace crates: Core, kits and contract crates. Today `knowledge-contract`, for the atom kinds and the frontmatter reader `ids` uses. Never a plugin's internal crates.
- Third-party: `serde` (derive, for the `cargo metadata` and `hooks.json` DTOs), `serde_json` (to read them), `thiserror` (the `ReadError`s). All three are on `.claude/temper.json`'s pre-approved list.
- Never: `cargo_metadata` or other crates for what `cargo metadata` and serde already give; `anyhow` in `src/lib.rs`.

## Public surface
`xtask::layers`: `Package`, `Dependency`, `Kind`, `Violation`, `check`, `read_workspace`, `ReadError`. `xtask::plugins`: `Plugin`, `PluginFile`, `FileKind`, `Violation`, `check`, `read_plugins`, `ReadError`, and the reader's limits `MAX_FILES`, `MAX_DEPTH`, `MAX_TEXT`. `xtask::ids`: `AtomFile`, `Violation`, `check`, `read_kb`, `ReadError` and its limits `MAX_FILES`, `MAX_DEPTH`, `MAX_TEXT`. Workspace-internal; nothing is published.

## Invariants
- Each `check` is pure: the same input gives the same violations, in member or plugin order.
- `read_plugins` never follows a symlink, `plugins/` itself included, and caps what it reads: 5,000 files per plugin (claude.ai's limit) and entries per folder, 16 folders deep, 1 MiB per text file it reads (H-13). Tests pin each limit at its boundary.
- `ids::check` fails on two files with one `id`, an atom outside `.kb/<kind>/` for its id's kind, an id that is not `kb-<kind>-<key>`, a file in a kind's folder without an `id`, and one `wi-` id standing for two questions (a decision's `weighin_item` and `question`, an open question's `tracks` and `title`). Other Markdown without an `id` is not an atom. `read_kb` reads only `*.md` files, passes over `.kb/_ledger/`, refuses a symlink or other non-regular entry anywhere else (it could hide an atom or show one twice), and caps 10,000 Markdown files in all, 10,000 entries per folder, 8 folders and 1 MiB a file (H-13). Tests pin each limit at its boundary.
- The layer of a member comes only from its directory (and, under `crates/<plugin>/`, from a `-contract` name suffix). The plugin folder list (`PLUGINS`) is the one place a new plugin is admitted; changing it is a crate-boundary change (H-16).
- Dev and build dependencies are held to the same layer rules as normal ones.

## Errors
`ReadError`: `Spawn` (cargo could not start), `Cargo` (it failed; carries its status and stderr), `Parse` (output was not the metadata expected), `OutsideWorkspace` and `NonUtf8` (a member's directory cannot be placed). `plugins::ReadError`: `Io`, `RootNotFolder`, `NonUtf8`, `NonUtf8Text`, `TooMany`, `TooManyEntries`, `TooDeep`, `TooLarge`. `ids::ReadError`: `Io`, `RootNotFolder`, `NonUtf8`, `NonUtf8Text`, `TooMany`, `TooManyEntries`, `NotRegular`, `TooDeep`, `TooLarge`. Violations are values (`Violation`), not errors.

## Test strategy
Integration tests in `tests/it/` against synthetic workspaces, plugins and atoms, one per rule, plus trees written under `CARGO_TARGET_TMPDIR` for the plugin reader and one test each that reads this repository. Run with `cargo test -p xtask`.

## Do not
- Do not relax a rule here to admit a crate or a plugin file; change `AGENTS.md`'s layer rules first, as an H-16 decision, then the code and its tests.

## Decisions
| Date | Decision | Rule | Decided by |
|---|---|---|---|
| 2026-10-30 | Core crates may depend on other Core crates (the blueprint's "nothing internal" read as "nothing outside Core"), so `redkiln-calibrate` and `redkiln-capacity` can use `redkiln-core` | ARC-02 | phase 0 skeleton, Ryan to confirm |
