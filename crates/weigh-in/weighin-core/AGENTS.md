# Crate charter: `weighin-core`

## Purpose
The ledger domain of Weigh-In: the event vocabulary, `wi-` item ids, the `weigh-in` marker parser, document and TODO scanning, the grounded decision card, the projection fold, `.kb` atom rendering and the board exchange format. It is its own crate so that it carries no I/O and can be linked into redkiln-rs unchanged (`lib.rs`, wi-b4c1d8).

## Layer
domain. Pure: nothing here reads a clock, a file, a process or the environment, and there is no async runtime.

## Allowed dependencies
- Workspace crates: none.
- Third-party: `serde` and `serde_json` (events, cards and board documents are JSON); `sha2` (item ids hash the session and the normalised question); `thiserror` (typed errors).
- Never: `weighin`, happenstance, tokio, clap, anyhow, `time` or anything that does I/O. Timestamps arrive as values from the caller.

## Public surface
The modules `atom`, `board`, `card`, `event`, `id`, `marker`, `projection` and `scan`, re-exporting `Card`, `EventEnvelope`, `LedgerEvent`, `ItemId`, `Item`, `Ledger` and `Status` at the root. Workspace-internal, never published (`publish = false`, D-0007).

## Invariants
- Current state is a fold of events in store-position order. Nothing trusts timestamps for ordering (`projection`).
- An item id is `wi-` plus lowercase hex of `sha256(session \n normalised question)`, so parallel sessions and worktrees mint ids without coordination (`id`).
- Rendered atoms validate against Redkiln's `KbFrontmatter` (`atom`).
- A card has 2 to 4 options with unique ids, and it never recommends an option that fails a MUST or is flagged as evasive (`card::CardError`).
- Card field names are the board's, so `board.html` renders a card as-is (`card`).

## Errors
- `card::CardError`: a card that breaks the invariants above. These are domain rejections.
- `board::AnswerError`: a board answer the ledger cannot accept, such as an unknown or non-open item, an unknown or ineligible option, an incomplete deferral, or an unknown dismiss reason. These are domain rejections.
- There are no infrastructure failures in this crate, because it has no I/O.

## Test strategy
Unit tests sit beside the code in each module: id, marker, scan, card, atom, board, projection and event, and the public docs carry doctests. Property tests live in one integration binary, `tests/it/` (TST-02, TST-14): a fixed-seed splitmix64 PRNG and adversarial-text generators in `tests/it/support.rs` (std only, no property-testing dependency) drive the public API of `id`, `marker`, `projection` and `atom` through a few thousand cases each. Run with `cargo test -p weighin-core`.

## Do not
- Add I/O, a clock read or an environment read. Those belong in `weighin` (the "no I/O" rule in `lib.rs`).
- Order by timestamp in the projection.
- Rename card fields without changing the board, which renders them by name.

## Decisions
| Date | Decision | Rule | Decided by |
|---|---|---|---|
| 2026-10-19 | `event::Surface` names the spellings of `item_presented.surface` and `item_decided.via`, `pane` among them; the stored fields stay strings. Only `as_str` is public; `weighin stats --json` counts standing decisions by surface in the binary, so `Health` is unchanged (B-30, #115). Additive public API. | API-14, H-16 | session lead (Claude), open to Ryan |
| 2026-11-01 | Decision ageing (D-0022, #173): `Decision::reviewed_ts` (serialised only once set), `Item::reviews` and `Item::reopened_after_review` (not serialised), and `Item::last_reviewed`, `Item::flip_condition`, `Item::due_for_review` and `Ledger::due_for_review`, which take `now` from the caller. `item_acknowledged` on a decided item records a review; on a default it folds as before. `Health` is unchanged: `weighin stats` counts reviews in the binary. Additive public API. | API-14, H-16 | Ryan Britton (D-0022) |
