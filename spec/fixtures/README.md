# Conformance fixtures

Golden files for every JSON document the SPEC defines, and the JSON Schemas (Draft 2020-12) beside them in [`../schemas/`](../schemas/). They belong to the SPEC, not to one crate: kits, contract crates and code in other languages (the suite mod) read them. `redkiln-core`'s tests read every file here (`crates/core/redkiln-core/tests/it/conformance.rs` and `conformance/`), and a file that no table there lists fails `every_fixture_and_schema_is_listed`. `patch-id/` is ID-09's recipe check, run by CI as a script (`patch-id/check.sh`).

A golden is a conforming example, not the only conforming document. The CLI error golden, for example, is weighin's exit-70 form (`data: {}`, one `internal` finding), which conforms; it does not make `internal` a rule for every binary (decisions row D5, 2026-11-08).

## Bytes

Each file is the exact bytes. A `.json` golden is one compact JSON line ended by `\n`. A `.jsonl` file is a journal, every line ended by `\n` unless its scenario says otherwise. Everything is LF (`.gitattributes`); the CRLF case of JR-03 is built in the test from the LF golden. Where the SPEC gives an example, its golden is that example byte for byte (the text in the fenced block, plus `\n`), and `conformance/spec_text.rs` checks it: EV-01, AP-01, JR-02, DR-02 and CLI-02.

## Classes

| Class | Schema | Core |
| --- | --- | --- |
| `valid/` | passes | reads it; where Core writes the shape, writes these bytes back |
| `lenient/` | fails | reads it: a writer must not write it, a reader takes it (EV-01 `null`, EV-09, JR-10) |
| `invalid/` | fails at exactly the `(instance path, keyword)` pairs its table row lists | refuses it, where Core reads the shape |
| `invalid-data/` | the `data` of Core's own types, failing that type's schema | refuses the `core.import_completed` ones; Core has no reader of `core.approval_recorded` yet |
| `scenario/` | not checked as a whole | journals of several lines, some malformed or torn on purpose |

A schema describes what a writer writes (EV-05): optional keys absent, never `null`, and no key the SPEC does not define. `data` stays an open object in `envelope-1` and `cli-envelope-1`, since its type's owner defines it; Core's own types have a schema for their `data`.

Two writer conventions in the schemas go beyond the SPEC's text (D4): `envelope-1` gives `cites` `minItems: 1`, since a writer leaves out an empty list as it leaves out any absent optional key (Core writes no `cites` when there are none), and `cli-envelope-1` limits `command` to printable characters with no control character. Each may be dropped if the SPEC says otherwise.

`status-1` is the one schema that allows keys it does not define (`additionalProperties: true`): CLI-12 has each plugin add its own keys beside the common ones, and a reader must take them (decision D7, Move 1 step 6d).

## Files

| File | Covers | Read by (in `conformance/`) |
| --- | --- | --- |
| `envelope/valid/ev-01-item-decided.json` | EV-01's example: witnessed, every optional key but `origin` | `envelope.rs`, `spec_text.rs` |
| `envelope/valid/minimal-asserted.json` | EV-01 with only the required keys, `asserted` with no `via` (TR-01) | `envelope.rs` |
| `envelope/valid/ap-01-approval-recorded.json` | AP-01's example | `envelope.rs`, `spec_text.rs` |
| `envelope/valid/approval-attested-signed-commit.json` | AP-01 with every optional `data` key, `attested` through `signed-commit` (TR-04), a `patch-id` subject with its base | `envelope.rs` |
| `envelope/valid/approval-github-review-asserted.json` | `github-review` written `asserted`, keeping `via` and `by` (TR-10) | `envelope.rs` |
| `envelope/valid/import-completed-path.json` | JR-13, a path source: what `journal::import` writes for `journal/valid/jr-02-item-raised.jsonl` | `records.rs` |
| `envelope/valid/import-completed-ref.json`, `-artifact.json` | JR-13, a ref and commit, an artifact | `records.rs` |
| `envelope/valid/import-completed-legacy.json` | ST-15's old source with its store id: what `journal::import_mapped` writes for one unmapped record | `records.rs` |
| `envelope/lenient/null-optional-keys.json` | EV-01: optional keys and trust evidence as `null` read as absent | `envelope.rs`, `schema.rs` |
| `envelope/lenient/unknown-keys.json` | EV-09: unknown keys in the envelope and in `data` | `envelope.rs`, `schema.rs` |
| `envelope/invalid/*.json` | OW-3 (`data-flattened`), EV-01 forms (`version-zero`, `ts-offset`, `data-not-object`), TR-01 (`trust-*`), ID-06, ID-03, a key named twice | `envelope.rs`, `schema.rs` |
| `envelope/invalid-data/*.json` | AP-01's verdict and subject hash, JR-13's one source and ID-01's store id form | `schema.rs` |
| `journal/valid/<line>.jsonl` | JR-02: the example, an EV-01 line with `origin` before `cites`, a line with caller `tags` | `journal.rs` |
| `journal/valid/<line>.imported.json` | JR-12, TR-06, EV-02: the stored envelope, `asserted` with the line's level in `claimed`, without `origin` or `tags` | `journal.rs` |
| `journal/valid/<line>.reexported.jsonl` | that event exported again: a fixed point of import then export | `journal.rs` |
| `journal/lenient/future-namespace.jsonl` | JR-10: an unregistered namespace is imported, tagged `plugin:acme` | `journal.rs` |
| `journal/invalid/*.jsonl` | JR-11 (`no-origin`), JR-02 (`tags-*`, and `unsorted-tags` and `duplicate-tags`: a line's tags are sorted, each once, or the line is malformed), JR-10 (`origin-past-max`) | `journal.rs`, `schema.rs` |
| `journal/scenario/ev-08-foreign-and-unusable.jsonl` | EV-08's foreign-events-are-skipped fixture, and EV-10's count | `journal.rs` |
| `journal/scenario/jr-10-malformed.jsonl` | JR-10: malformed lines skipped and reported by number | `journal.rs` |
| `journal/scenario/jr-03-torn-tail.jsonl` | JR-03: a torn last line, and the same journal in CRLF | `journal.rs` |
| `cli/valid/gate-failed.json` | CLI-02's example | `cli.rs`, `spec_text.rs` |
| `cli/valid/ok-with-warning.json` | CLI-03, CLI-06: a report with a warning finding and its `at` | `cli.rs` |
| `cli/valid/internal-error.json` | CLI-05: weighin's exit-70 envelope | `cli.rs` |
| `cli/invalid/*.json` | CLI-02, CLI-03 | `schema.rs`, `cli.rs` |
| `decision-request/valid/dr-02.json` | DR-02's example | `decision_request.rs`, `spec_text.rs`; `weighin-contract`'s `request` tests |
| `decision-request/invalid/*.json` | DR-02: options, option ids, door, `default` on a one-way item, subject hash form | `schema.rs`; `weighin-contract`'s `request` tests |
| `status/valid/weighin-ok.json` | CLI-12: weighin's `status --json` with a located store and one finished `Stop` hook; weighin's own keys (`hook_errors`, `hooks`) beside the common ones | `status.rs`, `schema.rs`; `weighin-contract`'s `status` tests |
| `status/valid/weighin-off.json` | CLI-12: a store that cannot be located, its keys `null` and one error finding (decision D7) | `status.rs`, `schema.rs` |
| `status/invalid/*.json` | CLI-12 `data` only: `health` missing or unknown, `store_schema` not `<major>.<minor>` | `schema.rs` |

The tables in `conformance/schema.rs` give each `invalid/` and `lenient/` file's exact violations. weighin's integration tests (`crates/weigh-in/weighin/tests/it/cli_conformance.rs`) validate its real `--json` output against `cli-envelope-1`.

## Schemas

| Schema | Describes | Ids |
| --- | --- | --- |
| `envelope-1.schema.json` | the envelope and its trust object | EV-01, EV-03, TR-01, TR-03, TR-04, TR-10, ID-02, ID-03, ID-05, ID-06 |
| `journal-line-1.schema.json` | a journal line: the envelope with `origin` required and `tags` | JR-02 |
| `core.approval_recorded-1.schema.json` | the `data` of `core.approval_recorded` | AP-01, ID-09 |
| `core.import_completed-1.schema.json` | the `data` of `core.import_completed` | JR-13, ST-15 |
| `decision-request-1.schema.json` | the decision request record | DR-02 |
| `cli-envelope-1.schema.json` | the CLI result envelope | CLI-02, CLI-03 |
| `status-1.schema.json` | the `data` of `status --json` | CLI-12 |

Each file is self-contained: no `$ref` crosses files, since the validator resolves none. `journal-line-1` copies `envelope-1`'s `$defs` and properties, and `decision-request-1` copies AP-01's `subject`; `shared_defs_are_identical` checks the copies. The closed lists (namespaces, `via` values and the levels they may back, hash methods, the tag keys Core sets) are checked against Core's registries. `-1` is the shape's version: until SPEC 1.0 freezes at Move 2 a draft schema may be edited in place, and after that a new version is a new file (EV-06, CLI-04).

## What a schema cannot say

These are pinned by Core's tests, and by fixtures a schema passes and Core refuses:

- a key named twice (`envelope/invalid/duplicate-key.json`): JSON Schema sees the parsed object, so the last value wins; Core refuses the envelope;
- a position past 2^63 - 1 in an event ref (`journal/invalid/origin-past-max.jsonl`);
- tags sorted (`journal/invalid/unsorted-tags.jsonl`; duplicates are the schema's `uniqueItems`), and `tags` and `origin` before `data`;
- a `ts` that is a real moment (a 30 February passes the pattern);
- byte lengths: EV-16's 64 KiB, EV-03's 64-byte type and EV-11's 255-byte tag are counted in characters by `maxLength`, which equals bytes only for ASCII;
- DR-04's cross-field guards (a text that is only whitespace, `recommendation` and `default` name an option, option ids in order, `deadline` after the event's `ts`, a CI raiser asks for `attested`), which are `weighin-contract`'s (`Request::check`);
- AP-02's checks, which a gate makes when it runs.
