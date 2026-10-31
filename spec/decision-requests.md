# Decision requests

Any plugin that needs the human raises a Weigh-In item: Bench parking a one-way call, Loop at a human gate, Review needing an owner's acknowledgement, Redkiln Rust asking to approve a lint exception (Blueprint, Decision requests). Weigh-In owns the decision domain; the SPEC owns the record's shape, so that every plugin's human gate asks and is answered the same way (Weigh-In call A).

## The record

**DR-01.** A decision request is an item that arrives already grounded: the raiser supplies the card, and Weigh-In validates it and presents it, skipping triage (weigh-in.md, The decision model). It is written through `weighin-contract` as the event that raises an item, with the channel tag `weighin.channel:request`; the event's name is Weigh-In's to set in that crate. Its `data` holds the request record below, and nothing in the record changes the meaning of a field weigh-in already stores.

**DR-02 (one-way, OW-11).** The request record:

| Key | Type | Meaning | From |
| --- | --- | --- | --- |
| `question` | string | The question, whole | Today's `Raised` |
| `options` | array of 2 to 4 objects | Each `{id, label, holds_when, cost_if_wrong}`; ids are `a` to `d` in order | Today's `CardOption` and its 2-to-4 guard |
| `recommendation` | string | The id of one option. It may not fail a MUST or be evasive | Today's CLI guards |
| `door` | string | `one-way`, `two-way` or `unsure`; `unsure` is treated as `one-way` everywhere | Today's triage fields |
| `authority` | string | `authority` (the owner's call) or `judgment` (a call anyone competent could make) | Today's triage fields |
| `blast` | string or absent | `file`, `module`, `system` or `external` | Today's triage fields |
| `default` | string or absent | The option an unattended run takes on a two-way item. Never set on `one-way` or `unsure` | wi-4d8b6a |
| `deadline` | string or absent | RFC 3339 UTC: when the raiser stops waiting and takes the default or gives up | Loop's gate record (ADR-0017, ADR-0198) |
| `subject` | object or absent | What the decision is about, in the approval record's subject shape (AP-01) | The pressure test's hash table |
| `blocks` | string | What waits on it: `merge`, `release`, `run`, `none` or `unknown` | assay's D-WI enum, plus `run` for Loop; `weighin.ci-check/1` |
| `trust_needed` | string | `witnessed` or `attested` (TR-07) | Pressure-test call C |
| `raiser` | object | `{namespace, key, run?, session?}`: the raising plugin, its own correlation key, and the run or session that asked | Today's content-hash id, keyed on the raiser's key |
| `attachments` | array or absent | Each `{kind, locator, hash?}`: a file, diff or log the human should see, by locator and digest | Loop ADR-0009's typed attachments |

**DR-03 (one-way, OW-15).** A request's item id is a `wi-` content-hash id (ID-04) over the domain `redkiln.id/weighin.request/1`, the raiser's namespace and its key. Raising the same key again finds the same item, and the raise is a conditioned append (not yet raised), so a raiser that retries after a crash never raises twice. A raiser resumes from its own checkpoint keyed by that id (LangGraph's interrupt pattern, weigh-in.md, Prior art).

```json
{"question":"Add the `jiff` crate for time zone parsing?","options":[{"id":"a","label":"Add jiff 0.2","holds_when":"Time zones are needed this release","cost_if_wrong":"One more dependency to audit"},{"id":"b","label":"Parse offsets only","holds_when":"UTC offsets cover every input","cost_if_wrong":"Named zones fail at runtime"}],"recommendation":"b","door":"one-way","authority":"authority","blocks":"merge","trust_needed":"attested","raiser":{"namespace":"rust","key":"h16/dependency/jiff","session":"gha:812345/9912/1"},"subject":{"kind":"dependency","method":"rust-fingerprint","hash":"rust-fingerprint:4be1…","locator":"Cargo.toml"}}
```

## Guards

**DR-04.** `weighin-contract` refuses a record that breaks any of these, and nothing is written:

- two to four options, with distinct ids in order;
- `recommendation` names one of them;
- `default`, when present, names one of them, and the door is `two-way`;
- `deadline`, when present, is later than the event's `ts`;
- a raiser in CI (a `gha:` session, ID-05) asks for `attested`, since no witnessed surface reaches it.

The guards run inside the contract crate, so a sibling cannot raise a malformed request by skipping Weigh-In's CLI (Weigh-In call B).

## Answers

**DR-05.** An answer is Weigh-In's decision event for the item. It names the chosen option, cites the raising event, and carries `applies`, `once` or `always` (Knowledge call G): an answer that always applies becomes a constraint, and may name its `confirmation`, the check that enforces it. The decider's trust is the event's envelope ([trust.md](trust.md)).

**DR-06.** A raiser accepts an answer only when its trust is at least the request's `trust_needed`, its option is one of the request's, and, when the request has a `subject`, the raiser recomputes the subject's hash and gets the same value (as AP-02 does). An answer about content that has since changed is stale; the raiser raises again with a new key.

**DR-07.** With nobody present, a run takes the `default` of a two-way item at its deadline and records that it did as Weigh-In's defaulted event, at `asserted` trust. A defaulted decision yields a proposed atom flagged for review, never an accepted one. A one-way item is never defaulted: the run stops and waits (wi-4d8b6a). Review defaults nothing; its check stays pending.

**DR-08.** Where an unattended request travels so that it can be answered elsewhere (a Decision issue, a ledger ref, or only the local machine for client work) is pressure-test call B, a two-way call Weigh-In owns. The SPEC requires only that an answer imported from any rendezvous keeps its `origin` (ID-02) and is `asserted` until a gate verifies its attestation (TR-06).
