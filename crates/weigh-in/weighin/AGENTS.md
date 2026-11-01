# Crate charter: `weighin`

## Purpose
The `weighin` CLI, and the single writer of the Weigh-In decision ledger. Every write, from a command, a hook or the `batch` entry point, goes through `ops::Session`; read-only commands use `ops::read_ledger`, which never creates the store, and `ops::Session` creates it only with its first append, so a command that fails before writing leaves none behind (B-01). That write path reads the ledger, validates, appends under a guard, and then performs the side effects of a decision, such as writing atoms and annotating sources.

## Layer
binary. The composition root holds the store, git, context packs, environment discovery and the Claude Code hook entry point. Output is written for a model to read at the lowest token cost: short lines, and `--json` where a structure is needed.

## Allowed dependencies
- Workspace crates: `weighin-core`.
- Third-party:
  - `happenstance-core` and `happenstance-sqlite`, the event store;
  - `tokio` with `rt` only, which happenstance's API needs;
  - `clap` (CLI), `anyhow` (binary errors), `time` (timestamps), `tempfile` (temporary indexes and files on the temp disk);
  - `serde` and `serde_json`.
- Never: a second writer of the ledger, or a git library. Git goes through the command line (`git.rs`).

## Public surface
None. This is a binary crate; its modules are private (`annotate`, `ci`, `commands`, `context`, `env`, `git`, `hooks`, `journal`, `ops`, `pane`, `recall`, `shell_bin`, `store`, `text`).

## Invariants
- **One write path.** Every change to the ledger goes through `ops::Session`.
- **Conditioned writes (DCB).** Raising an item fails if that item already exists. Every later event for an item fails if the item changed since the writer read it, so two sessions cannot both decide the same item (`store.rs`).
- **No deletes beside the ledger.** The store runs under a TRUNCATE journal and never through happenstance's WAL open, and temporary files stay on the temp disk (B-01).
- **Hooks never fail a session.** Errors are appended to `<state dir>/hook-errors.log`, and the hook exits 0 with no error output (`hooks.rs`).
- **`PostToolUse` only adds context beyond capture.** After the scan, it adds the decisions still standing that are anchored at the edited file, at most three lines, as `additionalContext`, and raises nothing more; it reads the ledger once per edit, for this and the session's journal together, and never writes for this (`recall.rs`, `hooks.rs`, #116).
- **Git is read-only, apart from a tree hash on a temporary index.** Nothing here commits, moves refs or runs hooks (`git.rs`).
- **Every command that appends, and every capture hook, appends the session's own events to its journal**, `.kb/_ledger/journal/<session>.jsonl`: append-only, deduped by line, only in an initialised git worktree, under a lock in the temp dir that the cloud hook script shares, and never failing the command or hook (`journal/append.rs`, B-28). `journal` prints the ledger's events in the same line format, and `stats` folds every journal with the ledger, counting an event held twice once (`journal.rs`, B-21).
- **`ci-check` never opens the ledger.** It reads git and the checkout only, and exits 0 whenever it writes a report; its findings are data (`ci.rs`, B-28).
- **Context packs stay bounded** by `caps.context_chars`, about 600 tokens (`context.rs`).

## Errors
`anyhow` throughout, with `Context` on I/O, store and git failures. Domain rejections from `weighin-core` (`CardError`, `AnswerError`) are reported to the caller as they are. A `.kb/_ledger/config.json` that exists but cannot be read or parsed (`env::ConfigError`) stops every command but `hook`, which runs on the defaults and reports it in every `SessionStart` message, never in `hook-errors.log`; `board export` also refuses a folder with no config or a board mode other than `artifact` (wi-7c21e4), and `board export --stdout` and `board apply --surface pane`, the local pane's, refuse no config or mode `off` (B-30). Exit codes: 0 on success; any error prints `weighin: <error chain>` to stderr and exits 1, except a closed stdout (`weighin open | head`): a broken pipe on stdout, anywhere in the chain, exits 0 with nothing on stderr (a command that changes the ledger still applies every operation first, and only its receipts stop), while a broken pipe on stderr or elsewhere is an error like any other; `weighin hook` always exits 0, even on a panic, and logs its own output failures to `hook-errors.log`, and its input and parse failures there only inside a git repository; outside git those leave no trace, and in Cowork hooks write nothing (`main.rs`, `hooks.rs`).

## Test strategy
Unit tests sit beside the code in `annotate`, `ci` (atoms, trailers, the append-only check and the report, all pure), `commands`, `env`, `store`, `context`, `git`, `hooks`, `journal` (with the writer's plan and lock), `ops`, `pane`, `recall`, `text` and `main`; the ones that spawn processes or open SQLite carry `#[cfg_attr(miri, ignore = "...")]`. Integration tests in `tests/it/` compile as one binary over shared helpers in `support.rs`. The end-to-end tests in `cli.rs` (and the source write-back in `annotate.rs`) cover:
- a scratch repository;
- hooks fed the JSON Claude Code sends;
- a linked worktree;
- a sitting done through `batch`;
- the board round trip.

`journal_write.rs` covers the session journals the binary writes (gates, dedupe beside the script's file, a cut-short line, failures), and `ci_check.rs` covers `ci-check` on real branches, over the `scratch.rs` repository, which gives each test its own temp dir. `owner.rs` covers the owner taken from git where there is no config, with git reading only a config each test writes (B-31). `pane.rs` covers what the local board pane needs (the state on stdout, one answer on stdin as surface `pane`, raw items read-only, mode `off`, no config). `recall.rs` covers the decisions an edit brings back (a match, no match, a reopened decision, the cap of three, a `Write`, a ledger that cannot be read, logged once) and that capture beside it is unchanged. `eval_seed.rs` scores marker capture against the eval seed (`eval/`, #22); `cargo test -p weighin --test it eval_seed -- --nocapture` prints the scores.

Run with `cargo test -p weighin`.

## Do not
- Write to the ledger outside `ops::Session`.
- Delete files beside the ledger (B-01): Cowork's connected folders refuse deletes. Overwrite a file there instead (as `hook-errors.seen` is, B-11), and set a stale one aside by renaming it (as `Store::open` does with a `-wal`).
- Let a hook return a non-zero exit code or print an error to the session.
- Ship binaries in a top-level `plugin/bin/` (B-03), or keep an old binary next to a rebuilt one (B-05).

## Decisions
| Date | Decision | Rule | Decided by |
|---|---|---|---|
| 2026-10-19 | The invariant "Journals are read, never written, by the CLI" becomes "every command that appends, and every capture hook, appends the session's own events to its journal" (B-28, #145). `ci` joins the modules. | ARC-08 | session lead (Claude), open to Ryan |
