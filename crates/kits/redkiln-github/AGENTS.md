# Crate charter: `redkiln-github`

## Purpose
The GitHub kit (blueprint, Kits): what a plugin needs to read tickets, pull request diffs and the state of a pull request, a commit's checks or a deployment from GitHub and git. Moved from Bench in Move 2 step 5 with its history: tickets, the tracker port and its GitHub Issues adapter, the ticket graph, the estimate and the PR diff engine (PR 5.3a), and the `watch` engine (PR 5.3b). Bench is the only user today.

## Layer
Kit. I/O only through `gh` and `git` child processes, each bounded; no harness, no environment. Time only as a deadline, and in `watch` behind its `Clock` port, whose real `SystemClock` sleeps the calling thread between polls.

## Allowed dependencies
- Core crates only (none used yet), `serde`, `serde_json`, `thiserror`. Dev: `tempfile` (scratch git repositories and ticket roots). Any other is H-16.
- Never a plugin crate, a contract crate or `anyhow`.

## Public surface
Only what a plugin calls is `pub` (API-14).
- `ticket`: `Ticket`, `TicketId`, `Status`. The id order is `TicketId`'s `Ord`.
- `tracker`: `Tracker` (an associated `Error`, so each adapter keeps its typed error; a plugin dispatching several closes them in its own enum), `Tickets`, `MAX_TICKETS`; `tracker::github`: `GitHub<G>`, `GitHubError`, `Repo`, `RepoError`, the ports `GhApi` and `GhGraphql`, `GhCli`, `GhError`, `Field`, and its limits.
- `graph`: `Graph`, `Check`, `Dangling`, `ExternalBlocker`, `GraphError`. A check reports what it found; its exit code is the plugin's.
- `estimate`: `estimate`, `Estimate`, `FileCost`, `Basis`, `DEFAULT_BUDGET` and the file limits.
- `diff`: `Budget`, `DiffError`, `GitError`, `TextLimits`, `resolve_commit`, `default_base`, `patch_id`, `numstat` (raw bytes; parsing is the caller's), `is_ancestor`, `diff_text`, `is_object_id`, `DEFAULT_BASES`, `MAX_NUMSTAT_BYTES`.
- `watch`: `watch` (the long poll), the ports `Clock` (real: `SystemClock`), `Source` and `DeployProvider`, `Schedule`, `Watched`, `Snapshot`, `Blocker`, `Pending`, the sources `PullRequestWatch`, `CommitWatch` and `DeployWatch` with `GitHubDeployments`, `DeployState`, `RepoRef`, `unresolved_threads`, `WatchError`, `DEFAULT_TIMEOUT_SECS`, and the merge-gate seam `HeadGate` with `GateBlock`: a pull request watch asks the plugin's gate last, once nothing else blocks or is pending. What a blocker or `WatchError` means as an exit code or finding is the plugin's.

## Invariants
- No harness protocol: no `CLAUDE_*` name and no hook payload or response key, tests included (`cargo xtask plugins`, check (b)).
- Every `gh` and `git` call has a timeout and an output cap (H-13): `GH_TIMEOUT`, `MAX_GH_OUTPUT`, a `Budget` or an explicit timeout per diff call. A capped answer that may be cut short is an error, never a partial result.
- `watch` only reads (REST `GET`, one GraphQL query); at most 100 check runs, statuses and review threads, past which a poll fails; its loop never sleeps past the deadline.
- Trackers are read-only and bounded: at most `MAX_TICKETS` tickets, `MAX_PAGES` pages and `MAX_EXTERNAL` outside blockers; past a limit the read fails.
- The diff's options are fixed so one change always has one patch id.
- `process` and `git` are private copies of Bench's waiting half and git runner until `redkiln-dispatch` (Move 4) absorbs them; `graph::cycle` copies Bench's cycle finder.
- Error enums are closed (workspace-internal, so not `#[non_exhaustive]`).

## Test strategy
Unit tests beside the code, moved from Bench with assertions unchanged: the GitHub tracker against recorded JSON through a fake `GhApi` (`tests/fixtures/github/`), `GhCli` against a fake `gh` script, the diff against scratch repositories (`src/test_repo.rs`), the estimate, the cycle finder and the process waiter, and the watch's loop, sources and threads against recorded JSON through a fake `gh`, gate and clock (`src/watch/fake.rs`, `tests/fixtures/watch/`; no sleep, no network). The graph's own tests stay in Bench (`dag::check::tests`), and so do the watch tests that assert an exit code or finding (`watch::blocker`, `watch::code::tests`); the graph's module doc test runs here. `cargo nextest run -p redkiln-github`.

## Decisions
In `docs/decisions.md` (2026-11-15 rows, PRs 5.3a and 5.3b).
