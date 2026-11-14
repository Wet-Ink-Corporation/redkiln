# Crate charter: `weighin`

## Purpose
The `weighin` CLI, and the single writer of the Weigh-In decision ledger. Every write, from a command, a hook or the `batch` entry point, goes through `ops::Session`; read-only commands use `ops::read_ledger`, which never creates the store.

The full charter (invariants, errors, do-nots and its decisions log; layer, dependencies and hooks are repeated here, so change both) is [`docs/weigh-in/crates/weighin.md`](../../../docs/weigh-in/crates/weighin.md), moved there whole so the charter chains fit the 24 KiB budget (`tools/charters/check.sh`), as Bench's was at its import. Read the parts a change touches before making it; its rules hold here, and new decisions are added to its log.

## Layer
binary. The composition root holds the store, git, context packs, environment discovery and the hooks, behind one adapter per harness (`harness::claude_code`, the only module that speaks Claude Code's protocol, CLI-13). Output is written for a model to read at the lowest token cost: short lines, and `--json` where a structure is needed.

## Allowed dependencies
- Workspace crates: `weighin-core`, `weighin-contract` with `core` (payloads, `open` and `ci-check` JSON, the default guard; step 5), `knowledge-contract` (reading atoms' frontmatter in `ci`), and `redkiln-core`, the suite's store, its location and the envelope (since #17 PR 5a).
- Third-party:
  - `clap` (CLI), `anyhow` (binary errors), `time` (timestamps), `tempfile` (temporary indexes and files on the temp disk);
  - `serde` and `serde_json`;
  - dev only: `jsonschema` (D2 C1).
- Never: a second writer of the ledger, `happenstance-*`, `rusqlite` or `tokio` (only Core opens the store), or a git library. Git goes through the command line (`git.rs`).

## Invariants in short
Among them: one write path (`ops::Session`); conditioned writes; the store is Core's, reached only through `redkiln-core`; old ledgers are imported once and never opened; hooks never fail a session; `ci-check` never opens a store or fetches; `status` imports nothing. All twelve are in the full charter.

## Hooks
The plugin's hook rules until Move 2 puts them in the SPEC: [`docs/weigh-in/hooks.md`](../../../docs/weigh-in/hooks.md), with how each is met and what is not yet. In short: every `hooks.json` entry is exec-form `hook <event>` and fails open (CLI-09); a guard fails closed (CLI-10); no build lets every hook through (CLI-11; not on Windows, where the missing `.exe` fails to spawn); never off without a word (HK-2); a guard is linear-time (HK-4); each hook budgets its own timeout and keeps a heartbeat (HK-5); stdin capped, every field optional (HK-6); a Stop hook never blocks, or has honest exits and a loop breaker (HK-7); denies name the route (HK-8). Weigh-In has no guard, and its Stop hook never blocks.

## Test strategy
See [`TESTING.md`](TESTING.md).
