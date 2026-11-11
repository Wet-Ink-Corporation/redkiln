# Crate charter: `bench`

## Purpose
The `bench` binary (anneal's `anneal-cli`, renamed at the Move 2 door): the deterministic layer behind the `rk-bench` plugin's hooks and skills and the `rk-bench-cockpit` mod. It is the composition root, so argument parsing, output formatting and exit codes live here.

The full charter as imported from anneal (public surface, invariants, errors and its decisions log) is [`docs/bench/crates/bench.md`](../../../docs/bench/crates/bench.md), moved there whole so this chain fits the 24 KiB charter budget (`tools/charters/check.sh`). Read the parts a change touches before making it; its rules still hold, with the names below.

## Layer
Plugin (bench), binary `bench`. Process concerns: arguments, stdout and stderr, exit codes. The code lives in an internal library target (`bench`, `src/lib.rs`) so tests reach it; `src/main.rs` passes the process's arguments and streams to `cli::run`.

## Allowed dependencies
- Workspace crates: `bench-contract` (with `core`: the `bench.*` events; the command list), `redkiln-core` (the suite store, its location, envelopes, ids, journals, approvals and the suite owner; since step 3 of `docs/plan/move-2-bench.md`, which removed `bench-ledger`). Bench reaches the store only through `src/store.rs` and its submodules (the 0.x envelope there is converted to and from Core's), and the location only through `src/paths/location.rs`.
- Third-party: `clap` (the workspace entry), `serde`, `serde_json`, `thiserror`, `sha2`, `ureq` (3, `rustls` with bundled roots: Clef's transport and `verdict set`'s commit status; M2-A, used by this crate only); dev: `tempfile`. The old-ledger fixtures in `tests/fixtures/anneal/` are read only from copies, pinned by `tests/it/fixtures.rs`.
- Never: anything that needs a network at build time.

## Names at the door (M2-B)
- Binary, clap command and launcher copies: `bench`; crates `bench`, `bench-contract`; install ids `rk-bench`, `rk-bench-cockpit`.
- Since step 3: events are `bench.<name>` in the suite store (`<git-common-dir>/redkiln/events.sqlite3`, `$REDKILN_STORE`); `$ANNEAL_STORE` still read, below it, naming an anneal ledger the store goes beside; anneal ledgers are found and warned about, never opened. `bench ledger append|query|tail` keep the 0.x short names until step 4; `bench ledger journal` prints JR-02 lines. Unchanged until step 5: `.anneal/`, `~/.anneal` and `$ANNEAL_HOME`. Old names in the imported charter and in `docs/bench/` are anneal's.
- `bench_contract::cli` lists every subcommand path; `tests/it/cli_pin.rs` pins it against clap.

## Harness
Claude Code's protocol (hook payload and response keys, `CLAUDE_*` names) lives only in `src/harness.rs` and `src/harness/` (CLI-13, `cargo xtask plugins` check (b)); every other module and every out-of-line test reaches it through `harness`.

## Test strategy
Unit tests beside the code; integration tests in `tests/it/` run the built binary (`core_store.rs`: old ledgers untouched, reads create no store, journal lines import). `cargo nextest run -p bench`. Tests that read repository data use the monorepo paths (`plugins/bench/`, `docs/bench/`, `evals/bench/`).
