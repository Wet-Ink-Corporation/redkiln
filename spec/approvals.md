# The approval record

Core holds one approval record, so that an approval given in one plugin can answer a gate in another without either reading the other's internals. A Bench plan approval answers Loop's planning gate when Loop recomputes the same hash before acting (Blueprint, Approvals). Approvals are bound to content: an approval names the hash of what was approved and is void when that hash moves (design rule 9).

## The record

**AP-01 (one-way, OW-10).** An approval is a `core.approval_recorded` event, version 1, class durable. Any plugin writes it through `redkiln-core`. Its `data`:

| Key | Type | Meaning |
| --- | --- | --- |
| `subject` | object | What was approved (below) |
| `verdict` | string | `approved`, `rejected` or `withdrawn` |
| `scope` | string | Where it holds: `worktree`, `branch` or `repository`. Default `repository` |
| `gates` | array of strings, or absent | The gates that may accept it, as `<namespace>.<gate>` (`bench.plan`, `loop.planning`). Absent: any gate whose subject matches |
| `decider` | string | Who decided: `github:<login>` or `human:<name>`. For a `witnessed` or `attested` event it equals `trust.by` |
| `expires` | string or absent | RFC 3339 UTC; absent means it does not expire |
| `request` | string or absent | The `wi-` item it answers, when it answers a decision request |

`subject`:

| Key | Type | Meaning |
| --- | --- | --- |
| `kind` | string | What sort of thing: `plan`, `spec`, `design`, `brief`, `change`, `pull_request`, `dependency`, `lint_exception`, `test_removal`, or a kind a namespace registers as `<namespace>.<kind>` |
| `method` | string | A hash method from ID-09 |
| `hash` | string | The digest in the method's form (`sha256:…`, `patch-id:…`) |
| `base` | string or absent | For `patch-id`, the base commit the diff was taken against |
| `locator` | string | Where the subject is: a worktree-relative path with `/` separators, `pr:<number>`, or a commit |

The time is the envelope's `ts`, and the decider's trust is the envelope's `trust` ([trust.md](trust.md)). The event carries the tags `subject:<method>:<hash>` and, when `request` is set, `item:<id>`.

```json
{"type":"core.approval_recorded","version":1,"ts":"2026-11-02T10:04:00Z","actor":"human:ryan","trust":{"level":"witnessed","via":"ask-user-question","by":"github:Ryan-Britton","ref":"toolu_01AbC"},"writer":{"binary":"bench","version":"0.1.0","spec":"1.0.0"},"session":"4c1d…","worktree":"wt-5d41402abc4b","data":{"subject":{"kind":"plan","method":"sha256","hash":"sha256:9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08","locator":"docs/plans/retention.md"},"verdict":"approved","scope":"repository","gates":["bench.plan","loop.planning"],"decider":"github:Ryan-Britton"}}
```

## Using one

**AP-02.** A gate accepts an approval only when all of these hold, checked when the gate runs:

1. the subject's `kind` and `method` are the ones the gate binds to (ID-10);
2. the gate recomputes the subject's hash by that method and gets the same value;
3. the verdict is `approved`, and no later `rejected` or `withdrawn` record for the same `kind` and `locator`, at the same or a higher trust level, follows it (later by position in this store, or by `cites`);
4. it has not expired;
5. its trust level is at least the gate's (TR-07);
6. its scope covers the gate's worktree or branch, and `gates`, when present, names the gate.

**AP-03.** An approval is never edited or deleted. When the file changes, its hash no longer matches and the approval simply stops applying; approving the new content is a new record.

**AP-04.** Recording an approval is a conditioned append (ST-10): it succeeds only if no record for the same subject landed after the one the writer read, so of two racing approvals of one plan exactly one is first (anneal `plan approve`).

**AP-05.** A plugin's own events may say more about an approval (Bench's grill confirmation and its quoted words, Loop's gate token) and cite the `core.approval_recorded` event, but the approval itself is the Core record. At Move 2, anneal's `plan_approved` imports as `core.approval_recorded` with a `sha256` subject and `asserted` trust, its writer's `actor` becomes `decider`, and anneal's `by` object (who gave it, through what) stays in Bench's own event.
