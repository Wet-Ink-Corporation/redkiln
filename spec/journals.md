# Journals

A store lives as long as its clone, and a cloud container's clone dies with the container. A journal is the export that outlives it: the store's durable facts as JSON lines, imported idempotently into any other store. It is the one answer to cloud containers losing `.git/`, replacing weigh-in's committed journal and temper's telemetry branch (Blueprint, Data and state).

## Format

**JR-01.** A journal is a UTF-8 file with no byte order mark, one event per line, each line ended by `\n`.

**JR-02 (one-way, OW-12).** Each line is one envelope exactly as EV-01 defines it, with `origin` set to the event's identity (ID-02) and `trust` as stored. A line carries no wrapper and no header, so `jq` and a plain line reader both work, and a line copied between journals stays the same fact.

```text
{"type":"weighin.item_raised","version":1,"ts":"2026-10-31T04:12:40Z","actor":"agent","trust":{"level":"asserted"},"writer":{"binary":"weighin","version":"2.0.0","spec":"1.0.0"},"session":"8b6fb94b-0975-5275-b434-6c9e60544265","worktree":"wt-5d41402abc4b","origin":"0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:40","data":{"item":"wi-3fa9c01b2e","question":"Which journal location?"}}
```

**JR-03.** A reader tolerates one `\r` before the `\n`, and skips blank lines. A last line with no `\n` that does not decode is incomplete, the mark of a writer that stopped mid-line: a reader passes over it, and the next writer to that file drops it and appends its event again, as weigh-in does (`journal.rs`). An incomplete line is not a complete line, so JR-05 does not hold it in place.

**JR-04.** A journal holds durable events only (EV-14). Telemetry never leaves the machine except as aggregates.

**JR-05.** A journal is append-only. A complete line, once written, is never changed or removed. A journal ref (JR-07) only moves forward: each commit has the previous one as its parent, and its file begins with the previous file's bytes. The writer and the relay refuse anything else, and import reports a ref that breaks it. Committed journal files from before JR-07 keep CI's check that a branch's journals at its head hold every complete line they held at its base, as `weighin ci-check` does today.

**JR-06.** Each session writes its own journal, named after the session id reduced to `[A-Za-z0-9._-]` (any other character becomes `_`). Events with no session go to the journal `store-<store-id>`. One journal per session means two concurrent sessions never update the same ref.

## Export

**JR-07 (one-way, OW-13).** Journals live in append-only refs, one per journal: `refs/redkiln/journal/<name>` (JR-06) points to a commit whose tree holds exactly one file, `<name>.jsonl`. They stay off code branches, out of a default clone and out of fork pull requests: a fork can push neither the upstream's refs nor its branches, and a fork pull request's `GITHUB_TOKEN` is read-only. Readers fetch them with an explicit refspec, `+refs/redkiln/journal/*:refs/redkiln/journal/*`. Two ways in:

- **Direct.** Where the writer may push a ref (a local clone, the desktop app, a GitHub Actions job), the checkpoint pushes the ref itself, fast-forward only.
- **Relay.** Claude Code cloud sessions' git proxy refuses any push outside `refs/heads/*`, and any branch deletion (refs spike, 2026-11-01). There the checkpoint pushes the branch `redkiln/inbox/<name>`: one commit on the default branch's tip that adds only `.redkiln/inbox/<name>.jsonl`, the whole journal. The repository's relay workflow (on push to `redkiln/inbox/**`, with `contents: write` and `GITHUB_TOKEN` only) refuses an inbox commit that changes anything else or does not extend the ref's file (JR-05), appends a commit to the ref, and deletes the inbox branch only if it has not moved since. The workflow is the repository owner's opt-in: without it a cloud checkpoint exports nothing, keeps its events in the store and says so.

A push that fails leaves the events in the store, and the next checkpoint exports them again (JR-08). Import reads every journal source it finds in this format (JR-10): these refs, workflow artifacts (Review's CI runs export that way), and committed `.jsonl` files from before this decision, weigh-in's `.kb/_ledger/journal/` among them, which are imported once (ST-15) and left in place. Which way a writer exports is a setting it may change; where journals live and how they are read is the one-way part (Ryan, 2026-11-01, refs spike).

**JR-08.** Export is one checkpoint, run at Stop and from a `pre-push` git hook, never at SessionEnd, whose hooks share a 1.5-second budget (pressure test, Run modes). It is idempotent: it appends to the session's journal only the durable events whose `origin` the journal does not already hold, in store order.

**JR-09.** In a `client` repository the checkpoint writes nothing (ST-13).

## Import

**JR-10.** Import reads a journal line by line and appends each complete, well-formed envelope to the store, whether or not the importing binary knows its type: the log is shared, and a sibling may. A malformed line is skipped and reported with its file and line number, never fatal.

**JR-11.** Import is idempotent by identity. Each line is ingested with its `origin` as its stored identity (ID-02), and happenstance refuses a second event with the same identity, so importing the same journal twice, two journals holding the same line after a cherry-pick, or a journal written by this store before its id was re-minted (ID-01), appends each fact once.

**JR-12.** Every imported event is stored at `asserted` trust, with the line's level kept in `trust.claimed` (TR-06). A journal from a fork pull request therefore cannot satisfy a gate.

**JR-13.** Each import is recorded as one `core.import_completed` event naming the source (a path, a ref and commit, or an artifact), the lines read, imported, skipped as present and skipped as malformed.

**JR-14.** Import never deletes, truncates or rewrites its source.
