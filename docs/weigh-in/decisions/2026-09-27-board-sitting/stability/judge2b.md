# Decision cards

Constraints in force (settled decisions, never re-open): the mechanism is a Redkiln-native decision item type built on a portable Claude Code plugin core; decided records land in the same .kb layout in every repo; judge models are Anthropic models only; the decider's agent work is about half interactive, half unattended; board mode is per repo with client repos local by default; unattended runs default two-way items and block one-way items; the decider has stated that redkiln-rs (a Rust rewrite of his backlog tool, still in development) will be Weigh-In's eventual home and Weigh-In must stand on its own until then without creating source-control friction for that merge. The decider is a very senior solo architect (Wet Ink Corporation, and a consulting firm, IQBG, with federal/state/local and commercial clients). He prefers long-term value and quality over shortcuts and dislikes recommendations that paper over problems or defer what can be settled now.


## wi-b4c1d8: Which runtime should the standalone Weigh-In CLI use, now that redkiln-rs is its eventual home?

Why it matters: The CLI is the single writer of the ledger and the piece redkiln-rs will absorb. Writing it in redkiln-rs's language decides whether integration adopts code or re-implements it.

Door: two-way (A small CLI can be rewritten, but every week on the wrong runtime adds code that is thrown away at integration.). Blast radius: system. Resolve by: before slice 1 starts (it is the first line of code).

Evidence:
- [your reopen note] redkiln-rs will be the eventual home of Weigh-In; Weigh-In must stand on its own until then.
- [redkiln-rs/Cargo.toml:21,25] Rust, edition 2024, rust-version 1.98.1, pinned exactly in rust-toolchain.toml.
- [operator-surface-proposal.md §12.1, §12.3] happenstance 0.3.2 (your DCB event store, MSRV 1.97.1) is proposed as redkiln-rs's tier-2 store adapter, with git staying the tier-1 authority.
- [ADR-0077] redkiln-rs reads git through gix and shells out for every git write, the same split a Weigh-In CLI needs.
- [plugin platform support] Cowork cannot install a plugin's bin/ executables; a binary inside a skill folder run through Bash is unverified.
- [ADR-0164] Your box runs core.autocrlf=true; redkiln-rs forces LF in every checkout, so whatever writes ledger files must write LF.

Options:
- **P. Node single file now, port to Rust at integration** — The original default: one dependency-free Node file, run the same way everywhere Node runs. Holds if the data contract, not the code, is what carries into redkiln-rs. If wrong: about 1,500 lines written twice, and two implementations of the ID and fold rules while both are live.
- **Q. Rust core compiled to WebAssembly behind a Node shim** — One .wasm artifact runs wherever Node runs; the same crate links natively into redkiln-rs. Holds if the shared logic is pure: git and process calls stay in the shim. If wrong: git, rg and file watching live in JavaScript anyway, splitting the logic you meant to share; node:wasi is still experimental.
- **R. Rust crate and CLI on redkiln-rs's toolchain** — A weighin-core crate plus a weighin binary, pinned to Rust 1.98.1, edition 2024, events stored through happenstance. Prebuilt binaries for Windows x64 and Linux x64 ship in the plugin. Cowork, which has no repo, keeps its ledger in the board's store and needs no binary. Holds if a prebuilt binary runs from the plugin folder on Windows and in Claude Code cloud sessions (20-minute spike). If wrong: you maintain release builds for two targets for a small tool; the core crate is still what redkiln-rs adopts.


## wi-6e2a47: Where should Weigh-In keep its live ledger so that moving into redkiln-rs later adds no source-control friction?

Why it matters: redkiln-rs keeps git as the authority for decisions and a derived store outside the working tree. A Weigh-In ledger committed as files would be a second shape that redkiln-rs has to import or ignore.

Door: two-way (Ledger files already pushed to your repos stay in history; everything else can move.). Blast radius: system. Resolve by: before slice 1 starts.

Evidence:
- [crates/rk-store/src/conn.rs:72] redkiln-rs keeps one store per repository at <git-common-dir>/redkiln/store.sqlite3, shared by every worktree and never committed.
- [ADR-0018] Rows are keyed by SHA or tree hash, never branch or path; rebase and amend emit rewrite events; only the run-owned worktree writes.
- [ADR-0105 cl. 3-4] An answer is recorded as a trailer commit on the run's branch, with the tree hash taken from a temporary index (untracked files excluded).
- [operator-surface-proposal.md §12.3] git stays the tier-1 authority; the store is derived and rebuildable. F19 rebuilt 174 of 174 records from git.
- [ADR-0129] In Phase 1 only a CLI answer reconciles; a web view is not an answer channel. Board answers must therefore be applied by the CLI, as Weigh-In already does.
- [settled · wi-a93f02] You chose C: commit event files in your repos, keep them local in client repos. This card would supersede the committing half.

Excerpt crates/rk-store/src/conn.rs:72:
```
/// Where the store lives: `<git-common-dir>/redkiln/`.
///
/// **One store per repository** (AC-178). Resolved from the common dir, which on arm a's **go** is
/// what gix reports from any worktree — so opening from a linked worktree yields the main
/// repository's store rather than a second one beside it.
#[must_use]
pub fn store_dir(common_dir: &Path) -> PathBuf {
    common_dir.join("redkiln")
}
```

Options:
- **P. Keep committed per-session event files (wi-a93f02 as decided)** — Event files under .kb/_ledger/events in your repos; write an importer into redkiln-rs's store at integration. Holds if redkiln-rs is happy to import and then ignore those files. If wrong: your repos carry ledger history redkiln-rs never reads, and two ledgers drift apart while both run.
- **Q. Commit open items as open_question atoms; no store** — Every open item is an atom in .kb/_intake from the moment it is raised; git holds everything. Holds if open items change status rarely. If wrong: each triage, defer and default becomes a commit, and kb-ingest's handling of frequently changing open_question atoms is still unverified.
- **R. Mirror redkiln-rs: git for decisions, a common-dir store for open items** — Open items live in <git-common-dir>/weighin/, shared by every worktree and never committed. Decisions reach git as .kb/_intake atoms plus a Weigh-In trailer on the enacting commit. Your repos mirror open items to the board for other machines. Holds if you never need undecided items from a fresh clone of a client repo. If wrong: open items in client repos stay on one machine, as wi-7c21e4 already accepts; a lost clone loses open items not yet decided.
