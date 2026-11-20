# Orchestration: fan-out, Clef D2 and the weigh-in bridge

`/anneal:orchestrate` (`plugin/skills/orchestrate/SKILL.md`, user-invoked because fanning out
costs about 15 times the tokens of one session) runs an approved plan's ready tickets across
subagents. Its deterministic steps are `anneal orchestrate`, `anneal decide --point D2`, and the
brief and round commands of [brief.md](brief.md).

Rulings, not stalls: a reversible call is ruled on and logged; a one-way call is parked in
Weigh-In as a decision request with its options, and the rest of the run moves on. Bench never
stores a decision itself: Weigh-In owns parked requests, rulings and their answers. Since Move 2
step 7 a park (PR 7.5) and a ruling (PR 7.6, Bench call F) write Weigh-In's
`weighin.request_raised` through `weighin-contract` (DR-01 to DR-05); Bench's `weighin_parked`,
`bridge.unavailable` and `orchestrate.ruled` are retired (decision D6 B, [ledger.md](ledger.md)).

```text
anneal orchestrate fan-out [--tracker markdown|github] [--dir PATH] [--repo OWNER/NAME] [--issue N]
bench orchestrate park --ticket <id> --q <question>
                       (--option <label> --holds <when> --cost <if wrong>){2,4}
                       [--recommend a|b|c|d] --at <path:line> --resolve-by <YYYY-MM-DD>
                       [--decision <D2 id>] [--session <id>]
anneal orchestrate rule --ticket <id> --what <text> --why <text> --cost <text>
                        [--decision <D2 id>] [--session <id>]
```

Exit codes: 0 fan out (park: parked; rule: raised), 3 `fan-out`: fewer than 3 independent
tickets, 4 `park`: Weigh-In cannot take the call (nothing recorded), rule instead; 2 for refused arguments, 70 when the
ledger or the tickets cannot be read ([exit-codes.md](exit-codes.md)).

## `fan-out`

It reads the tickets as `anneal dag frontier` does (same tracker flags, [dag.md](dag.md)) and
exits 0 only when the frontier holds 3 or more tickets. Frontier tickets are independent: each
has every blocker done, so none waits on another. Otherwise it exits 3 with an
`orchestrate/no-fan-out` finding, and the skill builds the tickets one at a time. A graph that
fails its check (a cycle, a dangling reference) lists no tickets, so it is refused too, with
the check's findings. `data` is `{tracker, fan_out, independent, required, tickets}`. It writes
nothing.

## Clef D2 and the call

For each judgment call the skill asks `anneal decide --point D2 --question noul --state <call>`
([decide.md](decide.md)): `one_way_door` at 0.50 or above is `park`, below it `rule`. D2 starts
in shadow, where it logs only: the skill parks only on `park` with `data.enforce: true`, and
rules otherwise (shadow, `rule`, Clef off with exit 3, no answer with exit 5, or `park` exit 4).

## `park`: a decision request

**Finding `weighin`.** Weigh-In ships its CLI inside its own plugin
(`${CLAUDE_PLUGIN_ROOT}/libexec/weighin` there), which Bench cannot see. Bench uses
`$WEIGHIN_BIN` when it is set and not empty (it must name a file), else the first `weighin` on
`PATH` (`weighin.exe`, `weighin.cmd` or `weighin.bat` on Windows).

**Arguments.** Every argument is checked first, before the store is opened or `weighin` runs:
the ticket as `bench round` takes it; each text one non-blank line of at most 1,024 characters
for `--q` and 512 for the rest, with no control, default-ignorable, format (Cf, bidi controls
among them), line or paragraph separator character; 2 to 4 options, each with exactly one
`--holds` and one `--cost` (counted, so unequal counts are refused), ids `a` to `d` in order;
`--recommend` one of them (default `a`); `--resolve-by` a `YYYY-MM-DD` date that exists
(`2026-02-31` and `2026-02-29` are refused, `2028-02-29` is not) and is after today, since the
request is due at the end of that UTC day (`T23:59:59Z`) and Weigh-In's contract refuses a
deadline already past; `--decision` a decision id; `--session` a ledger tag.

**Asking Weigh-In first** (decision D3 A). The park runs `weighin status --json` in the working
directory, with nothing on stdin, 10 s and each output stream capped at 4 MiB. It goes on only
when the answer is a `weighin.status/1` envelope with `requests: true` (a Weigh-In that folds
decision requests, step 7 or later) and its `store` is the store Bench writes (compared after
resolving the folder, so another spelling of the same path is the same store). It asks
before the store is opened, so otherwise no store is created, no anneal ledger imported and
nothing written.

**The request.** One `weighin.request_raised` (DR-02) is appended to the suite store, written by
Bench: the question and options, the recommendation, `door: one-way`, `authority`, no default,
the deadline, `blocks: run` (the fan-out run waits on it), `trust_needed: witnessed`
(`attested` for a `gha:` session, DR-04), the call's `path:line` as its `anchor` attachment and
the D2 decision id, when given, as a `bench.clef_decision` attachment. The raiser is Bench with
the key `orchestrate/park/<ticket>/<16 hex of sha256(question)>`, so the same question on the
same ticket is the same item. The item id is the contract's 10-hex id, extended to 12 and then
16 hex when an item of another raiser holds it; all three taken is an internal error (70).

The append is conditioned on no event already holding the item (DR-03), so a park run again (a
retried hook, a resumed session), or two parks racing, writes one request: the second reports
the item with `data.recorded: false` and the first one's `data.position`. `data` is
`{ticket, parked: true, item_id, recorded, position}` (`bench.orchestrate-park/2`; version 1's `position` was
a `weighin_parked` event's).

**Failing open.** With no `weighin`, or one that cannot start, times out, exits non-zero,
answers what is not a status, cannot take requests or keeps another store, the park writes
nothing and exits 4 with an `orchestrate/weigh-in-unavailable:<ticket>` finding and `data`
`{ticket, parked: false, reason}`: the orchestrator rules on the call instead.

Bench never writes `weigh-in` marker blocks for a parked call: Weigh-In's Stop hook would
capture them too and raise the call twice.

## `rule`: a two-way request, already defaulted

A ruling is logged in Weigh-In as the orchestrator's own two-way call (Bench call F, decision D4
B). Its arguments are checked as a park's are (`--what`, `--why` and `--cost` one line of at most
512 characters each), then one `weighin.request_raised` is appended: the question
`Ticket <ticket>: <what>`, option `a` what was ruled (holds when `--why`, costs `--cost` if
wrong), option `b` "Undo the ruling" (holds when the reason given does not hold, costs the ruled
work undone for nothing), recommended and default `a`, `door: two-way`, `authority: judgment`,
no deadline, `blocks: none`, the trust a park needs, and the D2 decision id, when given, as a
`bench.clef_decision` attachment. The raiser key is
`orchestrate/rule/<ticket>/<16 hex of sha256(what)>`, so the same ruling on the same ticket is
one item, run again with `data.recorded: false`.

Weigh-In folds a two-way judgment request with a default and no deadline as already defaulted
to that option (SPEC DR-07): it shows in `weighin digest`'s unacknowledged defaults, to be
acknowledged or reopened there, and nobody is asked. A ruling is a record, not an ask, so it
does not ask Weigh-In first: the request waits in the store for a `weighin` that folds it, and
with no `weighin` found the envelope adds an `orchestrate/weigh-in-absent:<ticket>` finding and
still exits 0. `data` is `{ticket, item_id, recorded, position}` (`bench.orchestrate-rule/2`;
version 1's `position` was an `orchestrate.ruled` event's).

## Reading rulings back (D6)

The `ruled-before` hook offers Clef D6 the `.out-of-scope/` rulings, weigh-in's accepted
decision atoms (`.kb/_intake/decisions/*.md`), and the items `weighin open --all --json --top 4096`
lists as decided whose atoms are not written yet ([hooks.md](hooks.md)). A ruling recorded with
`weighin decide` on a parked item is therefore surfaced on a later prompt. `--top` is explicit
because weigh-in's `open` lists only 12 items by default, in id order, also with `--json`; a
listing of 4,096 items or more may have been cut, so it is refused as a failure rather than
read in part.

Labelling the D2 decision from that answer is not automatic yet: the request keeps the D2
decision id (its `bench.clef_decision` attachment) so it can be, and until then it is labelled
by hand with `bench decide label <id> <outcome>`.

## Tests against weigh-in

Unit tests drive `status --json` through the `Runner` port with a fake on every platform; the
integration tests run the binary with Weigh-In absent (every platform) and with a fake
`weighin` shell script answering `status --json` (unix). `orchestrate::the_pinned_weigh_in_parks_once_and_its_ruling_reaches_d6`
and `orchestrate::a_raised_ruling_shows_as_an_unacknowledged_default_in_weighin_digest` run
against a real Weigh-In when `WEIGHIN_BIN` names one that folds requests (Move 2 step 7 or
later; the CI step that builds it is PR 7.7, decision D8); without it each test prints that it
skipped. The first also raises 13 more items, decides the one whose id
sorts last and removes the decision atoms, so D6 can find that ruling only past `open`'s
default 12.
