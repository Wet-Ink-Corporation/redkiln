# Crate charter: `weighin-core`

## Purpose
The ledger domain of Weigh-In: the `weigh-in` marker parser, document and TODO scanning, the projection fold, `.kb` atom rendering and the board exchange format. The event vocabulary, `wi-` item ids and the decision card are `weighin-contract`'s since Move 1 step 5, re-exported here at their old paths. It is its own crate so that it carries no I/O and can be linked into redkiln-rs unchanged (`lib.rs`, wi-b4c1d8).

## Layer
domain. Pure: nothing here reads a clock, a file, a process or the environment, and there is no async runtime.

## Allowed dependencies
- Workspace crates: `weighin-contract` without its `core` feature, so pure (events, ids, cards, `open` JSON; Move 1 step 5); `knowledge-contract`, Knowledge's atom contract (pure), through which `atom` writes frontmatter, ids and paths (Move 1 step 4).
- Third-party: `serde` and `serde_json` (events, cards and board documents are JSON); `thiserror` (typed errors). Item ids hash in `weighin-contract` since step 5.
- Never: `weighin`, happenstance, tokio, clap, anyhow, `time` or anything that does I/O. Timestamps arrive as values from the caller.

## Public surface
The modules `answered`, `atom`, `board`, `marker`, `projection` and `scan`, with `card`, `event` and `id` re-exported from `weighin-contract`, re-exporting `Card`, `EventEnvelope`, `LedgerEvent`, `ItemId`, `Item`, `Ledger` and `Status` at the root. Workspace-internal, never published (`publish = false`, D-0007).

## Invariants
- Current state is a fold of events in store-position order. Nothing trusts timestamps for ordering (`projection`).
- An item id is `wi-` plus lowercase hex of `sha256(session \n normalised question)`, so parallel sessions and worktrees mint ids without coordination (`id`).
- Atoms are written through `knowledge-contract` (`atom`): frontmatter, ids and paths are Knowledge's; the MADR body is Weigh-In's rendering. Decision atoms of 6- and 8-hex items are byte for byte what 1.x wrote.
- A card has 2 to 4 options with unique ids, and it never recommends an option that fails a MUST or is flagged as evasive (`card::CardError`).
- Card field names are the board's, so `board.html` renders a card as-is (`card`).
- A pick is witnessed only when the person resolved the dialog, the question is single-select, the item's card passes `Card::validate` (cards folded from the store or imported ledgers are not otherwise re-validated), no option title marks itself recommended (`recommended`, in any ASCII case, anywhere after the title's first `(` or `[`, #159) and no two options can be shown under the same label, the question is the item's own (`<id> <question>`, with ` (one-way: options first)` exactly when the item's door is not two-way), no label was shown twice and no option under two labels, every eligible option was shown under its `<id>: <title>` label (the recommended option may carry one ` (Recommended)` suffix, only when the item's door is two-way and `Card::recommendation_earned` holds, as SKILL.md step 4 presents it), and the one label selected names an eligible option (`answered`). The module takes plain values; the harness payload stays in the binary's adapter.
- The door comes from the item (`ItemCard::door`, i.e. `Item::door`), never from the text the model wrote. An `Unsure` door counts as one-way: a sitting must present it options first, with the suffix and no marked recommendation, or nothing it shows is witnessed (`answered`).
- Question matching is byte-exact, and an item with a blank question never matches: a sitting must show the stored `Item::question` verbatim after the id (`answered`).
- Accepted residual (#125 review): `answered` does not see option descriptions or the question header, so a dialog could describe an option differently from its card. The payload carries them, but PR 1b does not compare them: a sitting writes each description from the card's `holds_if` and `if_wrong` in its own words (SKILL.md step 4), so there is no stored text to match byte for byte.
- `AskUserQuestion` answers are keyed by question text, so the binary records nothing for a question whose text the dialog showed twice, without calling `answered::witness` (`weighin`'s `hooks/answered.rs`, #125 PR 1b). Accepted residual (#159): a title spelling `recommended` with Unicode lookalikes passes; catching it needs a confusables table, a dependency (H-16).

## Errors
- `card::CardError`: a card that breaks the invariants above. These are domain rejections.
- `board::AnswerError`: a board answer the ledger cannot accept, such as an unknown or non-open item, an unknown or ineligible option, an incomplete deferral, or an unknown dismiss reason. These are domain rejections.
- `answered::Unwitnessed`: why an answered dialog does not witness a pick of one option (not resolved by the person, multi-select, an invalid card, a title claiming the recommendation, two options sharing a label, not the item's own question, a label shown twice, an option under two labels, an eligible option not shown, not exactly one shown label selected, a label naming no option, an ineligible option). These are domain rejections: the caller records nothing `witnessed`.
- There are no infrastructure failures in this crate, because it has no I/O.

## Test strategy
Unit tests sit beside the code in each module: id, marker, scan, card, atom, board, answered, projection and event, and the public docs carry doctests. Property tests live in one integration binary, `tests/it/` (TST-02, TST-14): a fixed-seed splitmix64 PRNG and adversarial-text generators in `tests/it/support.rs` (std only, no property-testing dependency) drive the public API of `id`, `marker`, `projection`, `atom` and `answered` through a few thousand cases each. Run with `cargo test -p weighin-core`.

## Do not
- Add I/O, a clock read or an environment read. Those belong in `weighin` (the "no I/O" rule in `lib.rs`).
- Order by timestamp in the projection.
- Rename card fields without changing the board, which renders them by name.

## Decisions
| Date | Decision | Rule | Decided by |
|---|---|---|---|
| 2026-10-19 | `event::Surface` names the spellings of `item_presented.surface` and `item_decided.via`, `pane` among them; the stored fields stay strings. Only `as_str` is public; `weighin stats --json` counts standing decisions by surface in the binary, so `Health` is unchanged (B-30, #115). Additive public API. | API-14, H-16 | session lead (Claude), open to Ryan |
| 2026-11-01 | Decision ageing (D-0022, #173): `Decision::reviewed_ts` (serialised only once set), `Item::reviews` and `Item::reopened_after_review` (not serialised), and `Item::last_reviewed`, `Item::flip_condition`, `Item::due_for_review` and `Ledger::due_for_review`, which take `now` from the caller. `item_acknowledged` on a decided item records a review; on a default it folds as before. `Health` is unchanged: `weighin stats` counts reviews in the binary. Additive public API. | API-14, H-16 | Ryan Britton (D-0022) |
| 2026-11-20 | #125 PR 1a (D7 A): new public module `answered`, pure: `witness(&ShownQuestion, &Answer, &ItemCard) -> Result<Choice, Unwitnessed>`, `label(&CardOption)` (`<id>: <title>`), `ONE_WAY_SUFFIX`, `RECOMMENDED_SUFFIX`, and the plain input types `ShownQuestion`, `Answer`, `Resolution`, `ItemCard` (id, question, card and door). No harness protocol: PR 1b's `weighin` adapter maps the `AskUserQuestion` payload onto these types and writes `witnessed` (TR-03, DR-06); every other path stays `asserted` (TR-05). Additive public API. | TR-03, TR-05, DR-06, API-14, H-16 | Ryan (D7 A, #125) |
| 2026-11-20 | #159: `Unwitnessed::TitleMarksRecommended` catches any option title with `recommended`, in any ASCII case, anywhere after its first `(` or `[`, not only the exact ` (Recommended)` ending. Chosen over trimming and case-folding the suffix because it fails closed on more variants (`(Recommended) T`, `T (recommended option)`, `T ((x) recommended)`, `T [Recommended]`; the last two from review): a false refusal costs only the witness, since the sitting records the answer `asserted`, while a false acceptance records a witnessed pick under a label that misled the person. No API change | TR-03, H-16 | two-way, delegated to the implementer (#125 PR 1b) |
