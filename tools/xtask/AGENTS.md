# Crate charter: `xtask`

## Purpose
Repository tasks for the redkiln workspace, run as `cargo xtask <task>` (alias in `.cargo/config.toml`). It is the blueprint's `tools/xtask`: a dev tool, never shipped in a plugin. It has three check tasks: `layers`, the structural test of the layer rules in the root `AGENTS.md`; `plugins`, the structural test of its CLI-first plugin rules, of every subcommand a prompt names existing (`check_commands`), and of harness protocol staying in each binary's adapter module (`harness`, CLI-13); and `ids`, the check of `.kb/` against the knowledge-base id rules (Knowledge call C). The release tasks (`release`) build every plugin binary into `libexec/<target>/` (`build`), stage the plugins as a release ships them (`package`), write them into the release repository (`marketplace`) and check a release's version sites and changelog (`release-check`); `.github/workflows/release.yml` runs them.

## Layer
Tool (`tools/`). A library (`src/lib.rs`, the tasks) plus a thin binary (`src/main.rs`, argument parsing and exit codes).

## Allowed dependencies
- Workspace crates: Core, kits and contract crates. Today `knowledge-contract`, for the atom kinds and the frontmatter reader `ids` uses, and `weighin-contract` (pure, no features), for `cli`'s command lists `main.rs` passes to `check_commands`. Never a plugin's internal crates.
- Third-party: `serde` (derive, for the `cargo metadata` and `hooks.json` DTOs), `serde_json` (to read them), `thiserror` (the `ReadError`s), `sha2` (the Cowork fetch script's stamp). All four are on `.claude/temper.json`'s pre-approved list.
- Never: `cargo_metadata` or other crates for what `cargo metadata` and serde already give; `anyhow` in `src/lib.rs`.

## Public surface
`xtask::layers`: `Package`, `Dependency`, `Kind`, `Violation`, `check`, `read_workspace`, `ReadError`. `xtask::plugins`: `Plugin`, `PluginFile`, `FileKind`, `Violation`, `check`, `Cli`, `check_commands`, `read_plugins`, `ReadError`, and the reader's limits `MAX_FILES`, `MAX_DEPTH`, `MAX_TEXT`. `xtask::harness`: `SourceFile`, `Violation`, `check`, `read_sources`, `ReadError`, `TOKENS` and the reader's limits `MAX_FILES`, `MAX_DEPTH`, `MAX_TEXT`. `xtask::ids`: `AtomFile`, `Violation`, `check`, `read_kb`, `ReadError` and its limits `MAX_FILES`, `MAX_DEPTH`, `MAX_TEXT`. `xtask::release`: `build`, `package`, `Download`, `Packaged`, `stamp`, `assemble`, `release_check`, `changelog_section`, `ReleaseError`, `Tree`. Workspace-internal; nothing is published.

## Invariants
- Each `check` is pure: the same input gives the same violations, in member or plugin order.
- `read_plugins` never follows a symlink, `plugins/` itself included, and caps what it reads: 5,000 files per plugin (claude.ai's limit) and entries per folder, 16 folders deep, 1 MiB per text file it reads (H-13). Tests pin each limit at its boundary.
- `ids::check` fails on two files with one `id`, an atom outside `.kb/<kind>/` for its id's kind, an id that is not `kb-<kind>-<key>`, a file in a kind's folder without an `id`, and one `wi-` id standing for two questions (a decision's `weighin_item` and `question`, an open question's `tracks` and `title`). Other Markdown without an `id` is not an atom. `read_kb` reads only `*.md` files, passes over `.kb/_ledger/`, refuses a symlink or other non-regular entry anywhere else (it could hide an atom or show one twice), and caps 10,000 Markdown files in all, 10,000 entries per folder, 8 folders and 1 MiB a file (H-13). Tests pin each limit at its boundary.
- `harness::check` fails on a protocol token (`TOKENS`, or `CLAUDE_` then an uppercase letter, as a whole identifier) in a `.rs` file under `crates/`, outside a crate's `tests/`, the `src/harness.rs` and `src/harness/` of a crate with `src/main.rs`, `//` lines and a top-level `#[cfg(test)]` module as rustfmt writes it. `read_sources` never follows a symlink, skips `target/` folders and caps 10,000 `.rs` files and entries per folder, 16 folders and 1 MiB a file (H-13), each pinned at its boundary.
- `plugins::check_commands` reads only code (fenced block lines and inline spans) in the Markdown and text of skills, agents and commands: after a shipped binary's name, its `libexec/` path or the file's first alias (`` `W` below means `...` ``), value options and their values and other `-` tokens are skipped, and the next lowercase word (two, when the list has paths under the first and a second word follows) must be in the binary's list. A shipped binary with no list fails (D5b); one not shipped is `check`'s `UnknownBinary`. The mod's TypeScript argv is not read.
- The layer of a member comes only from its directory (and, under `crates/<plugin>/`, from a `-contract` name suffix). The plugin folder list (`PLUGINS`) is the one place a new plugin is admitted; changing it is a crate-boundary change (H-16).
- Dev and build dependencies are held to the same layer rules as normal ones.
- `package` writes only into a new folder and never changes `plugins/`; it copies only regular files, gives every plugin that ships a binary's launcher every build of it, and refuses a package that breaks the plugin rules. `assemble` replaces only `plugins/<name>/` for each packaged plugin and its own `marketplace.json` entry, leaving every other folder and entry, and refuses a symlink where a plugin folder goes.

## Errors
`ReadError`: `Spawn` (cargo could not start), `Cargo` (it failed; carries its status and stderr), `Parse` (output was not the metadata expected), `OutsideWorkspace` and `NonUtf8` (a member's directory cannot be placed). `plugins::ReadError`: `Io`, `RootNotFolder`, `NonUtf8`, `NonUtf8Text`, `TooMany`, `TooManyEntries`, `TooDeep`, `TooLarge`. `harness::ReadError` and `ids::ReadError`: `Io`, `RootNotFolder`, `NonUtf8`, `NonUtf8Text`, `TooMany`, `TooManyEntries`, `NotRegular`, `TooDeep`, `TooLarge`. `release::ReleaseError`: `Read` (a plugin tree, naming which: the source plugins, the staged package or the packages to assemble), `Io`, `Json`, `Spawn`, `Failed`, `NoHost`, `OutExists`, `BadName`, `NoBuild`, `VersionsDiffer`, `VersionMismatch`, `NoChangelogSection`, `Plugin`, `Stamp`, `Marketplace`, `Rules`. Violations are values (`Violation`), not errors.

## Test strategy
Integration tests in `tests/it/` against synthetic workspaces, plugins and atoms, one per rule, plus trees written under `CARGO_TARGET_TMPDIR` for the plugin reader and one test each that reads this repository. Run with `cargo test -p xtask`.

## Do not
- Do not relax a rule here to admit a crate or a plugin file; change `AGENTS.md`'s layer rules first, as an H-16 decision, then the code and its tests.

## Decisions
| Date | Decision | Rule | Decided by |
|---|---|---|---|
| 2026-10-30 | Core crates may depend on other Core crates (the blueprint's "nothing internal" read as "nothing outside Core"), so `redkiln-calibrate` and `redkiln-capacity` can use `redkiln-core` | ARC-02 | phase 0 skeleton, Ryan to confirm |
