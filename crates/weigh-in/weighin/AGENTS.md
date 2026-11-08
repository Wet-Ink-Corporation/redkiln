# Crate charter: `weighin`

## Purpose
The `weighin` CLI, and the single writer of the Weigh-In decision ledger. Every write, from a command, a hook or the `batch` entry point, goes through `ops::Session`; read-only commands use `ops::read_ledger`, which never creates the store, and `ops::Session` creates it only with its first append, so a command that fails before writing leaves none behind (B-01). That write path reads the ledger, validates, appends under a guard, and then performs the side effects of a decision, such as writing atoms and annotating sources.

## Layer
binary. The composition root holds the store, git, context packs, environment discovery and the Claude Code hook entry point. Output is written for a model to read at the lowest token cost: short lines, and `--json` where a structure is needed.

## Allowed dependencies
- Workspace crates: `weighin-core`, `knowledge-contract` (reading atoms' frontmatter in `ci`), and `redkiln-core`, the suite's store, its location and the envelope (since #17 PR 5a).
- Third-party:
  - `clap` (CLI), `anyhow` (binary errors), `time` (timestamps), `tempfile` (temporary indexes and files on the temp disk);
  - `serde` and `serde_json`;
  - dev only: `jsonschema` (D2 C1).
- Never: a second writer of the ledger, `happenstance-*`, `rusqlite` or `tokio` (only Core opens the store), or a git library. Git goes through the command line (`git.rs`).

## Public surface
None. This is a binary crate; its modules are private (`annotate`, `ci`, `cli_envelope`, `commands`, `context`, `env`, `git`, `hooks`, `journal`, `legacy_import`, `ops`, `pane`, `recall`, `shell_bin`, `store`, `text`).

## Invariants
- **One write path.** Every change to the ledger goes through `ops::Session`; imports go through Core's.
- **Conditioned writes (DCB).** Raising an item fails if that item already exists. Every later event for an item fails if the item changed since the writer read it, so two sessions cannot both decide the same item (`store.rs`).
- **The store is Core's.** weighin opens, locates and writes it only through `redkiln-core` (`Store::open`, `open_existing`, `append`, `read`), as `weighin.<type>` envelopes, version 1, `asserted` (`store/wire.rs`, which moves to `weighin-contract` at step 5); a read never creates it. It lives at `<git-common-dir>/redkiln/events.sqlite3`, the state dir at `redkiln/weighin/` beside it (D4). A weighin event this build cannot use is skipped and counted; another namespace's never conflicts a write (EV-08 to EV-10).
- **Old ledgers are never opened.** A 1.x ledger (`<common>/weighin/ledger.sqlite3`, `.weighin/`, or the one `$WEIGHIN_STORE` or config.json's `store` names, the store going beside it, D2) is imported once from Core's copy, before 1.x journals, by any command but `ci-check` (`legacy_import`); a store path naming one is refused. Nothing is deleted beside the store, and temporary files stay on the temp disk (B-01).
- **Hooks never fail a session.** Errors are appended to `<state dir>/hook-errors.log`, and the hook exits 0 with no error output (`hooks.rs`).
- **`PostToolUse` only adds context beyond capture.** After the scan, it adds the decisions still standing that are anchored at the edited file, at most three lines, as `additionalContext`, and raises nothing more; it reads the ledger once per edit and never writes for this, nor touches a journal (`recall.rs`, `hooks.rs`, #116).
- **weighin's own git is read-only, apart from a tree hash on a temporary index.** Nothing in `git.rs` commits, moves refs or runs hooks; journal refs and inbox branches are written by `redkiln-core` only.
- **Only the Stop hook, `journal checkpoint` and `journal pre-push` export journals**, in an `own` repository (ST-13, D8 A), inside a budget that ends before the hook timeout (`journal/refs.rs`, D3, D11), the Stop over its session's journal first, then only the journals past its repository's watermark (`journal/watermark.rs`); the Stop hook logs only what needs a person (D12). SessionStart imports journal sources before it primes, inside `Budget::SESSION_START` (D10). Nothing writes under `.kb/_ledger/journal/`; 1.x committed journals there are read only: imported once (`legacy_import`), and read by `stats` (an event held twice counted once) and `ci-check` (`journal.rs`, B-21).
- **The class and owner are suite.toml's** (`.redkiln/suite.toml`, read through Core's `Suite` whatever chose the store): `repo_class`, below it config.json's with a warning (M1-F), else `client`; `owner`, then config.json's, then git's. `init` writes the class only to a new suite.toml, never to config.json (#17 PR 5b).
- **`ci-check` never opens the ledger.** It reads git and the checkout only, and exits 0 whenever it writes a report; its findings are data (`ci.rs`, B-28).
- **Context packs stay bounded** by `caps.context_chars`, about 600 tokens (`context.rs`).

## Errors
`anyhow` throughout, with `Context` on I/O, store and git failures. Domain rejections from `weighin-core` (`CardError`, `AnswerError`) are reported to the caller as they are. A `.kb/_ledger/config.json` that exists but cannot be read or parsed (`env::ConfigError`) stops every command but `hook`, which runs on the defaults and reports it in every `SessionStart` message, never in `hook-errors.log`; `board export` also refuses a folder with no config or a board mode other than `artifact` (wi-7c21e4), and a repository that is not `own` (ST-13), and `board export --stdout` and `board apply --surface pane`, the local pane's, refuse no config or mode `off` (B-30). Exit codes: 0 on success; any error prints `weighin: <error chain>` to stderr and exits 1, except a closed stdout (`weighin open | head`): a broken pipe on stdout, anywhere in the chain, exits 0 with nothing on stderr (a command that changes the ledger still applies every operation first, and only its receipts stop), while a broken pipe on stderr or elsewhere is an error like any other. The `journal checkpoint`, `import`, `relay` and `pre-push` subcommands follow SPEC CLI-01 to CLI-08 instead (`cli_envelope`, `commands/journal.rs`): exit 0, 1 (`relay`: inbox left in place), 2 (usage, or no inbox branch), 70 (could not run) or 74 (output failed), and `weighin: error:` or `weighin: warning:` on stderr; `pre-push` always exits 0. `weighin hook` always exits 0, even on a panic, and logs its own output failures to `hook-errors.log`, and its input and parse failures there only inside a git repository; outside git those leave no trace, and in Cowork hooks write nothing (`main.rs`, `hooks.rs`).

## Test strategy
See [`TESTING.md`](TESTING.md).

## Do not
- Write to the ledger outside `ops::Session`.
- Delete files beside the ledger (B-01): Cowork's connected folders refuse deletes. Overwrite a file there instead (as `hook-errors.seen` is, B-11), and set a stale one aside by renaming it (as Core's `Store::open` does with a `-wal`).
- Let a hook return a non-zero exit code or print an error to the session.
- Ship binaries in a top-level `plugin/bin/` (B-03), or keep an old binary next to a rebuilt one (B-05).

## Decisions
| Date | Decision | Rule | Decided by |
|---|---|---|---|
| 2026-10-19 | The invariant "Journals are read, never written, by the CLI" becomes "every command that appends, and every capture hook, appends the session's own events to its journal" (B-28, #145). `ci` joins the modules. | ARC-08 | session lead (Claude), open to Ryan |
| 2026-11-06 | weighin writes through `redkiln-core` (#17 PR 5a, D1 to D7, D9, D10): `happenstance-*` and `tokio` leave its dependencies, "No deletes beside the ledger" becomes "The store is Core's" and "Old ledgers are never opened", and `redkiln-core` joins the workspace crates | ARC-08, H-16 | Ryan (decision card, #17 PR 5 plan, 2026-11-06) |
| 2026-11-07 | Class and owner from suite.toml (#17 PR 5b, D8 A): a `client` repository keeps no journal and exports no board; `init` writes suite.toml | ARC-08, ST-13 | Ryan (decision card, 2026-11-06) |
| 2026-11-07 | Journal refs (step 7 PR B, D3, D6, D11, D12): the committed-journal writer (`journal/append.rs`) is retired; only Stop, `journal checkpoint` and `journal pre-push` export, to refs through `redkiln-core`; "Git is read-only" names weighin's own git | ARC-08, JR-07, JR-08 | owner (decision card, 2026-11-07) |
| 2026-11-08 | Step 8: 1.x sources imported once (`legacy_import`, H5) | ARC-08, H-16 | Ryan (decision cards, 2026-11-08) |
