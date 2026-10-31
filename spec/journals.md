# Journals

A store lives as long as its clone, and a cloud container's clone dies with the container. A journal is the export that outlives it: the store's durable facts as JSON lines, imported idempotently into any other store. It is the one answer to cloud containers losing `.git/`, replacing weigh-in's committed journal and temper's telemetry branch (Blueprint, Data and state).

## Format

**JR-01.** A journal is a UTF-8 file with no byte order mark, one event per line, each line ended by `\n`.

**JR-02 (one-way, OW-12).** Each line is one envelope exactly as EV-01 defines it, with `origin` set to the event's identity (ID-02) and `trust` as stored. A line carries no wrapper and no header, so `jq` and a plain line reader both work, and a line copied between journals stays the same fact.

```text
{"type":"weighin.item_raised","version":1,"ts":"2026-10-31T04:12:40Z","actor":"agent","trust":{"level":"asserted"},"writer":{"binary":"weighin","version":"2.0.0","spec":"1.0.0"},"session":"8b6fb94b-0975-5275-b434-6c9e60544265","worktree":"wt-5d41402abc4b","origin":"0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f/40","data":{"item":"wi-3fa9c01b2e","question":"Which journal location?"}}
```

**JR-03.** A reader tolerates one `\r` before the `\n`, and skips blank lines. A last line with no `\n` is incomplete, the mark of a writer that stopped mid-line: a reader ignores it, and the next writer starts a new line after it rather than appending to it (weigh-in `journal.rs`).

**JR-04.** A journal holds durable events only (EV-14). Telemetry never leaves the machine except as aggregates.

**JR-05.** A journal is append-only. A complete line, once written, is never changed or removed, and a branch's journals at its head hold every complete line they held at its base. CI checks this, as `weighin ci-check` does today.

**JR-06.** Each session writes its own file, `<session>.jsonl`, with the session id reduced to `[A-Za-z0-9._-]` (any other character becomes `_`). Events with no session go to `store-<store-id>.jsonl`. One file per session keeps concurrent sessions from conflicting in a merge.

## Export

**JR-07 (one-way, OW-13).** Where journals live is pressure-test call A, and it is open: append-only refs, one per store (`refs/redkiln/journal/<store-id>`, one file per session in its tree, fetched with an explicit refspec), or committed files on the code branch. Refs keep unreviewed agent output off code branches and away from fork pull requests, but whether cloud sessions and Actions may push a ref other than their own branch is the spike in phase 0. Until it reports, the draft writes committed files at `.redkiln/journal/<session>.jsonl`, the fallback either way. Review's CI runs export as workflow artifacts in the same format.

weigh-in's `.kb/_ledger/journal/` becomes this location at the rename at the door; its existing files are imported once (ST-15) and left in place.

**JR-08.** Export is one checkpoint, run at Stop and from a `pre-push` git hook, never at SessionEnd, whose hooks share a 1.5-second budget (pressure test, Run modes). It is idempotent: it appends to the session's file only the durable events whose `origin` the file does not already hold, in store order.

**JR-09.** In a `client` repository the checkpoint writes nothing (ST-13).

## Import

**JR-10.** Import reads a journal line by line and appends each complete, well-formed envelope to the store, whether or not the importing binary knows its type: the log is shared, and a sibling may. A malformed line is skipped and reported with its file and line number, never fatal.

**JR-11.** Import is idempotent by identity. A line whose `origin` names the importing store's own id is skipped, since the store already holds it. Any other line is a conditioned append that fails if an event tagged with the same `origin` is already present, so importing the same journal twice, or two journals holding the same line after a cherry-pick, appends each fact once.

**JR-12.** Every imported event is stored at `asserted` trust, with the line's level kept in `trust.claimed` (TR-06). A journal from a fork pull request therefore cannot satisfy a gate.

**JR-13.** Each import is recorded as one `core.import_completed` event naming the source (a path, a ref and commit, or an artifact), the lines read, imported, skipped as present and skipped as malformed.

**JR-14.** Import never deletes, truncates or rewrites its source.
