# Calibration records

A producer that asks a model a question whose answer may one day decide something (Bench's Clef points, later Review and Loop) records each answer and, later, what turned out right. Calibration scores the recorded probabilities against those outcomes, and a person reads the scores before letting a point act on its own. This section fixes the two record types, where they live and how a reader takes them alongside anneal's v1 rows. `redkiln-calibrate` implements the types and the reader (`redkiln_calibrate::record`, `redkiln_calibrate::rows`); the goldens in [`fixtures/`](fixtures/README.md) pin both.

## Records

**CA-01. Record before scoring.** A judgement that may one day drive control flow is recorded when it is made, before anything scores it (blueprint design rule 8). Scores and the promotion bar are only reported: a point leaves shadow mode only through a human edit of the producer's configuration.

**CA-02. Types.** Two `core.*` types, both durable (EV-14), each versioned under EV-06 and described by its schema (`schemas/core.judgement_recorded-1.schema.json`, `schemas/core.judgement_labelled-1.schema.json`).

`core.judgement_recorded` version 1, one answer (or the lack of one):

| Key | Type | Meaning |
| --- | --- | --- |
| `producer` | string: lowercase ASCII letters, digits and `-`, not first | The namespace whose judgement it is (EV-03), also its user-scope folder name (ST-17) |
| `id` | string, 1 to 64 of `[A-Za-z0-9_.:-]` | Unique per producer; labels name it |
| `point` | string, same characters | The decision point, in the producer's vocabulary |
| `question` | string, same characters | The question type |
| `model` | string | The model asked |
| `mode` | `shadow` or `enforce` | The mode in effect when it was asked |
| `probability` | number from 0 to 1, or absent | P(yes), or P(top option) for a choice; absent only with `failure` |
| `options` | object of string to number from 0 to 1, or absent | The probability of each option of a choice |
| `top` | string, or absent | The top option |
| `action` | string, or absent | What the producer's thresholds made of it, in its vocabulary |
| `asked` | boolean, present exactly when `probability` is | Whether the action handed the decision to a human |
| `failure` | `{"kind": string, "reason"?: string}`, or absent | Why no answer came |
| `subject` | `<method>:<hash>` (ID-09), or absent | The content the judgement binds to |
| `detail` | object, or absent | The producer's own fields; Core reads none of them |

Exactly one of `probability` and `failure` is present. `options`, `top` and `action` come only with `probability`.

`core.judgement_labelled` version 1, an outcome given later:

| Key | Type | Meaning |
| --- | --- | --- |
| `producer` | string, as above | As above |
| `judgement` | string, as `id` above | The `id` of the judgement it labels |
| `outcome` | string, 1 to 64 of `[A-Za-z0-9_.:-]` | What turned out right, in the producer's vocabulary |
| `by` | `{"actor"?: "human" \| "model", "via"?: string}`, or absent | Who gave it and through what, as asserted |

A judgement may be labelled more than once; the latest label in reading order is its outcome. A label may name a judgement recorded in either line form (CA-04).

**CA-03. Where they live.** In the **user scope** (ST-17), a producer's records are lines of `<root>/calibrate/<producer>/decisions.jsonl` and its rotations, pooled across repositories: a line is at most 8 KiB, the live file rotates at 16 MiB and four rotated files are kept, so the oldest lines past about 80 MiB per producer are dropped (H-13). Files are read oldest first. In **repository scope**, the same types are durable events in the store (ST-01), tagged `plugin:core`, `trust:<level>`, `core.producer:<producer>`, and `subject:<method>:<hash>` when `data.subject` is set (EV-12). SPEC 1.0 allows repository scope; no binary writes it yet.

**CA-04. Line forms.** A line of a user-scope file is one of:

- a **record**: an EV-01 envelope of one of the CA-02 types, without `origin` and without tags, at most 8 KiB with its newline;
- an **anneal v1 row**: `{"kind": "decision", …}` or `{"kind": "label", …}`, the rows anneal and then Bench wrote before these types, read forever and mapped onto the CA-02 types by the table below.

A reader takes both forms alike. It skips and counts, never fatally (EV-08 to EV-10): a line of neither form; a record of another type, of a `version` above the one it knows, whose envelope or `data` does not decode, or longer than 8 KiB; a record whose `producer` is not the folder's; and a v1 row the table cannot map. A torn last line is such a line.

**CA-05. Trust.** A record is `asserted` unless a TR-03 or TR-04 code path wrote it (TR-05). A label does not change its judgement's trust. A label given through a mod pane is still a CLI call, so it is `asserted`; `by.via` keeps the surface as data.

**CA-06. Vocabulary.** Points, questions, actions and outcomes stay the producer's vocabulary. Core scores a probability against an outcome only as the producer maps that outcome to right or wrong, and reads none of `detail`.

## Appendix: the v1 field map

The reader upcasts each v1 row (EV-07's spirit). A v1 key that is `null` is absent. The v1 producer is `bench`: anneal's rows became Bench's.

`{"kind": "decision"}` to `core.judgement_recorded`:

| v1 key | Record |
| --- | --- |
| `ts_ms` | the envelope's `ts` (milliseconds since the Unix epoch) |
| `id`, `point`, `question` | `id`, `point`, `question`; a row whose value does not fit CA-02 is skipped |
| `model` | `model`; `""` when absent |
| `mode` | `enforce` when `enforce`, else `shadow` (no other mode acted on an answer); any other value is kept in `detail.mode` |
| `probability` | `probability`; out of 0..=1 the row is skipped |
| `probabilities` | `options` beside a `probability`, where an option value outside 0..=1 skips the row; beside no `probability`, kept in `detail.probabilities` as it is |
| `top`, `action` | `top`, `action` |
| (from `action`) | `asked`: the action is `ask` or starts `ask_` |
| `error`, `error_reason` (no `probability`) | `failure.kind`, `failure.reason`; `kind` is `unknown` when `error` is absent |
| every other key (`subject`, `state_sha256`, `input`, `thresholds`, `threshold`, `configured_mode`, `latency_ms`, `outcome`, `unreachable`, and `error`, `error_reason` beside a `probability`) | the same key in `detail` |

v1's `subject` is the rule's subject text, not an ID-09 digest, so it goes to `detail`, and the record's `subject` is absent.

`{"kind": "label"}` to `core.judgement_labelled`:

| v1 key | Record |
| --- | --- |
| `ts_ms` | the envelope's `ts` |
| `id` | `judgement` |
| `outcome` | `outcome`; a row whose outcome does not fit CA-02 is skipped |
| `actor`, `via` | `by.actor`, `by.via`; `by` is absent when both are; an `actor` other than `human` or `model` is not kept, and the row still reads |
| any other key | not kept: the type has no `detail` |
