# Crate charter: `weighin-contract`

## Purpose
Weigh-In's contract (blueprint, Contract crate; SPEC DR): what another plugin needs to raise, read and answer Weigh-In items without reaching into Weigh-In's internals. It holds the event payloads and their `weighin.*` names with versions, `wi-` ids, the vocabulary (kind, door, authority), the decision card, the shared guards, the JSON of `open --json` and `ci-check --json`, the `data` of `status --json` (CLI-12), and, with the `core` feature, the decision request (DR-01 to DR-04) and its answer (DR-05, DR-06). Born at Move 1 step 5.

## Layer
contract (Weigh-In's). No file, clock, process or environment access.

## Allowed dependencies
- Workspace crates: `redkiln-core`, only behind the `core` feature: its `Payload` impls, the request's envelope, ids, subjects and trust levels. Without `core` the crate is pure, so weighin-core links it without Core.
- Third-party: `serde`, `serde_json`, `sha2` (item ids), `thiserror`. Dev: `tempfile` (a store in a test).
- Never: another plugin's crates, `weighin-core` or `weighin`, anything that does I/O in its own code.

## Public surface
Modules `vocab`, `id`, `card`, `event`, `names`, `guard`, `open`, `ci_check`, `status`, and with `core` `request` and `answer`; `Card`, `EventEnvelope`, `LedgerEvent` and `ItemId` at the root. Workspace-internal, never published.

## Invariants
- `ci_check::Report` is `weighin.ci-check/1` byte for byte; a change is a `/2` with assay told first (`docs/plan/move-1-weigh-in.md`).
- `status::Status` is `weighin.status/1`'s `data`, field order wire order, every `Option` written `null`; it reads a later key (CLI-04), and `spec/fixtures/status/` pins it.
- `open::OpenItem` writes what weighin 1.2.0's `open --json` wrote; weighin-core's test pins it against its projection's `Item`.
- Every payload is version 1 and durable under `weighin.<serde tag>` (`names`).
- A request is refused, and nothing written, when it breaks a DR-04 guard or a per-field shape of `decision-request-1.schema.json` (`Request::check`, run by `raise_event`); its item id is ID-04 over `redkiln.id/weighin.request/1`, its namespace and key (DR-03).
- A one-way or unsure item is never defaulted (`guard::may_route_to_default` for triage's route, `Request::check` for a request).

## Errors
`card::CardError`, `guard::DefaultRefused`, `request::RequestError`, `answer::AnswerRefused` `ci_check::ReadError` (a report of another shape or schema) and `status::ReadError` are domain refusals; `RequestError` also carries Core's id, tag and envelope refusals.

## Test strategy
Unit tests beside each module; `request` reads the SPEC's decision-request goldens (`spec/fixtures/decision-request/`) and `status` its status goldens (`spec/fixtures/status/valid/`) and writes a request through a temporary Core store. Run `cargo test -p weighin-contract --all-features`, and without features for the pure half.

## Do not
- Fold, project or render here: that is weighin-core's.
- Change a payload's bytes without a new version (EV-06).
