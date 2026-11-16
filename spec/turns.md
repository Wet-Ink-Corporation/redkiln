# The end of a turn

An agent ends a turn with a report a program can read: a `Status:` line, the section lines after it, and any fenced blocks a plugin asks for. One grammar serves every plugin, so Bench's `SubagentStop` hook, Clef's D4, temper's Stop gates and Weigh-In's capture read the same message the same way. `redkiln-core` implements the report (`redkiln_core::turn`, ET-02 to ET-07); Weigh-In's marker parser implements block framing (ET-08). The fixtures in [`fixtures/turn/`](fixtures/README.md) pin both.

## Reading

**ET-01. Scope.** The grammar reads the final assistant message of a turn, or a subagent's report as it is handed back, capped by the reading hook (HK-6). Reading never fails: a message with no report, or with a malformed one, is a value (ET-09).

**ET-02. Normalised line.** Keys and words are compared on a normalised line: every `*` and `` ` `` in the line is removed, then the leading run of whitespace, `-`, `+`, `>` and `#`. So `- **Status:** `DONE``, `> Status: DONE` and `## Status: DONE` all read as `Status: DONE`. Section text is taken from the normalised line. `_` is not removed, so `DONE_WITH_CONCERNS` survives, and a key in `_` emphasis (`__Status:__`) is not a key.

**ET-03. Keys.** A key is `<Key>:` at the start of a normalised line, matched ASCII case-insensitively, with nothing between the key and its colon. A status line is one whose key is `Status`.

**ET-04. Words.** A status line's word is the first whitespace-separated word after `Status:`, with trailing `.`, `,`, `;`, `:` and `!` trimmed, matched ASCII case-insensitively against this closed set:

| Word | Meaning |
| --- | --- |
| `DONE` | Done and verified |
| `DONE_WITH_CONCERNS` | Done, with concerns the caller should read |
| `NEEDS_CONTEXT` | Cannot go on without context the caller has |
| `BLOCKED` | Needs a decision it cannot take |
| `WIP` | Paused, unfinished, with the reason given |

The rest of the line is free text, the reason or the question (`Status: WIP: paused for CI`). `WIP` and `BLOCKED` are the honest exits a Stop gate stands down on (HK-7). Adding a word is a SPEC minor; a reader that meets a word it does not know reads a malformed report (ET-05), never fails.

**ET-05. Which line is the report.** The last status line in the message is the report, whatever follows its colon. If its word is not in the set (a prose line such as `Status: waiting on CI`), the message has a malformed report and no status; an earlier status line does not stand in for it. A reader that gates treats a malformed report as neither `WIP` nor `BLOCKED`.

**ET-06. Fences.** A status line counts inside or outside a code fence, so a report quoted in a ```` ```text ```` fence, as temper's template writes it, is read.

**ET-07. Sections.** The registered keys are, in report order, `Gate`, `Review`, `Changed`, `Decisions needed`, `Declined` and `Not verified`. A section counts only on a line after the report's status line, malformed or not, and its first occurrence there is the one read. Its text is the rest of that line, then the following lines up to a blank line, a fence line (one whose first non-blank characters are three backticks or three tildes), a status line or any registered key, joined by single spaces. A key present with nothing after it is present and empty. None is required by the SPEC: a plugin that requires some says which in its docs (Bench requires all but `Declined`). Adding a key is a SPEC minor; an unregistered key is text (ET-09).

## Plugin blocks

**ET-08. Framing.** A plugin block is a fenced code block whose opening fence has three or more backticks or tildes, indented at most three spaces, and that closes as CommonMark says: a line of the same mark, at least as many, and nothing else but whitespace. A backtick fence's info string holds no backtick. The info string's first word names a registered block, matched ASCII case-insensitively. Any other fence is consumed whole, so a block shown inside it (a ````` ````markdown ````` example) is its content. A fence with no closer is read as an ordinary line, so a stray or truncated example does not swallow a block after it. A line indented four or more spaces is not a fence. Blocks may appear anywhere in the message; by convention they follow the report.

| Block | Aliases | Owner | Body |
| --- | --- | --- | --- |
| `weigh-in` | `weighin`, `weigh_in` | Weigh-In | Weigh-In's marker fields (`q:` required) |

Adding a block name is a SPEC minor. Each block's body grammar belongs to its owner.

## Leniency

**ET-09. Lenient.** A reader never fails or blocks on a message's form. An absent report, a malformed one or a missing section is recorded or reported as the reader's own rules say. Unknown keys are text.

## Example

**ET-10. Example.** One message: a report in a ```` ```text ```` fence, with every registered section, then one `weigh-in` block. It reads as `DONE_WITH_CONCERNS` with six sections and one block, and it is `fixtures/turn/et-10-example.md` byte for byte.

````markdown
The retention fix is in; one call below waits on the owner.

```text
Status: DONE_WITH_CONCERNS
Gate: fmt ok | clippy ok | test ok (42 passed) | doc ok
Review: approved
Changed: crates/retention/src/policy.rs (+40/-2)
Decisions needed: retention per series or per record (the block below)
Declined: none
Not verified: the Windows leg
```

```weigh-in
kind: assumption
door: one-way
authority: authority
q: Should retention overrides attach per record or per series?
did: per-series, matching RetentionPolicy
flip: a tenant needs per-record legal hold
at: crates/retention/src/policy.rs:88
blocks: none
```
````

## Where this comes from

The grammar reconciles three readers that grew apart:

| Point | temper 1.x (`parseStatus`) | Bench (`hook/status/block.rs`) | Weigh-In (`marker.rs`) | SPEC |
| --- | --- | --- | --- | --- |
| Words | `DONE`, `WIP`, `BLOCKED` | The blueprint's five | — | The five (ET-04) |
| Case | Insensitive | Insensitive | Insensitive (info string) | Insensitive, ASCII |
| Prefixes | Leading `*`, `_`, `` ` ``, `>`, `-` | Leading whitespace and `- + > #`; every `*` and `` ` `` removed | — | Bench's (ET-02) |
| Which line | The last line with a known word | The last `Status:` line; prose there is no report | — | Bench's (ET-05) |
| Fences | Read through | Read through | Other fences consumed whole; unclosed is a plain line; indent at most 3 | Status read through (ET-06); blocks as Weigh-In (ET-08) |
| Sections | — | Five required | — | Six registered, none required (ET-07) |

Ryan chose the last status line as the report and the module `redkiln_core::turn` (decision cards, 2026-11-16); the normalisation details and the word set being two-way are the plan's defaults (`docs/decisions.md`).
