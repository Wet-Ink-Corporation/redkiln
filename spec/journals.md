# Journals

A store lives as long as its clone, and a cloud container's clone dies with the container. A journal is the export that outlives it: the store's durable facts as JSON lines, imported idempotently into any other store. It is the one answer to cloud containers losing `.git/`, replacing weigh-in's committed journal and temper's telemetry branch (Blueprint, Data and state).

## Format

**JR-01.** A journal is a UTF-8 file with no byte order mark, one event per line, each line ended by `\n`.

**JR-02 (one-way, OW-12).** Each line is one envelope exactly as EV-01 defines it, with `origin` set to the event's identity (ID-02) and `trust` as stored. A line carries no wrapper and no header, so `jq` and a plain line reader both work, and a line copied between journals stays the same fact.

```text
{"type":"weighin.item_raised","version":1,"ts":"2026-10-31T04:12:40Z","actor":"agent","trust":{"level":"asserted"},"writer":{"binary":"weighin","version":"2.0.0","spec":"1.0.0"},"session":"8b6fb94b-0975-5275-b434-6c9e60544265","worktree":"wt-5d41402abc4b","origin":"0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:40","data":{"item":"wi-3fa9c01b2e","question":"Which journal location?"}}
```

**JR-03.** A reader tolerates one `\r` before the `\n`, and skips blank lines. A last line with no `\n` that does not decode is incomplete, the mark of a writer that stopped mid-line: a reader passes over it, and the next writer to that file drops it and appends its event again, as weigh-in does (`journal.rs`). An incomplete line is not a complete line, so JR-05 does not hold it in place. A journal ref's or inbox's commit is written whole, so it never holds an incomplete line; the reader rule still applies to legacy committed journals and workflow artifacts.

**JR-04.** A journal holds durable events only (EV-14). Telemetry never leaves the machine except as aggregates.

**JR-05.** A journal is append-only. A complete line, once written, is never changed or removed. A journal ref (JR-07) only moves forward: its first commit is a root commit, each later commit has the previous one as its parent, and each commit's file holds every complete line of the previous one, in the same order, at its start. Writers and the relay refuse to move a ref any other way. They cannot stop someone with write access who sets out to rewrite a ref (rulesets do not cover custom refs), so import is the check: it remembers the commit it last imported from each `refs/redkiln/journal/*` ref (JR-13; inbox branches are replaced by design and read by content only), and when a fetched ref does not descend from it, it reports the rewrite and still ingests the new lines, which identity makes safe (JR-11). The legacy committed journals JR-07 names keep CI's check that a branch's journals at its head hold every complete line they held at its base, as `weighin ci-check` does today.

**JR-06.** Each session writes its own journal, named after its session id, lowercased, with every character outside `[a-z0-9_-]` replaced by `_`, so a name is always a valid git ref component. Events with no session go to the journal `store-<store-id>`. One journal per session means two concurrent sessions never update the same session journal; only the store journal is shared. The mapping can make two ids share a name, and one session can write from two clones; both are harmless, because every write to a journal ref merges by `origin` (JR-07) and identity removes duplicates (JR-11).

## Export

**JR-07 (one-way, OW-13).** Journals live in append-only refs, one per journal: `refs/redkiln/journal/<name>` (JR-06) points to a commit whose tree holds exactly one file, `<name>.jsonl`. The refs stay off code branches, out of a default clone and out of fork pull requests: a fork can push neither the upstream's refs nor its branches, and a fork pull request's `GITHUB_TOKEN` is read-only. Journals are never committed to a code branch.

Every write to a journal ref is a **merge**: the new file is the ref's current file (empty when the ref is absent, and then the new commit is a root commit) followed by the complete lines of the incoming journal that decode as envelopes with an `origin` the ref's file does not hold, each `origin` once (first occurrence wins), in the incoming order. A merge that adds nothing moves nothing. This keeps JR-05 by construction, whoever else wrote to the ref in between. A writer that finds a ref breaking JR-05 (not a single-parent chain, a tree with another file, or a non-empty file whose last byte is not `\n`) writes nothing to it and reports it; a person recovers by moving the ref aside to `refs/redkiln/broken/<name>/<sha>`, after which the next write starts a new root commit, which merging by `origin` makes safe. Every push of a ref or an inbox branch is a fast-forward without force unless stated, uses `--no-verify` (so the `pre-push` checkpoint, JR-08, does not run again), and is retried at most three times after a rejection (git's `[rejected]`: not a fast-forward, or a stale lease), fetching first; after that the events stay in the store for the next checkpoint. A writer tries the direct way first; any other failure to push `refs/redkiln/*` (the cloud proxy's 403 among them) sends it straight to the relay. Two ways in:

- **Direct.** Where the writer may push a ref (a local clone, the desktop app, a GitHub Actions job), the checkpoint fetches the ref, merges its own journal into it, and pushes it.
- **Relay.** Claude Code cloud sessions' git proxy refuses any push outside `refs/heads/*`, and any branch deletion (refs spike, 2026-11-01). There the checkpoint writes the inbox branch `redkiln/inbox/<name>`: one commit whose parent is the remote default branch's tip, fetched at checkpoint time, and which adds only `.redkiln/inbox/<name>.jsonl`. That file is the merge (as above) of the journal ref's file, the inbox branch's file if the branch exists, and the checkpoint's own journal, so an inbox written by another clone under the same name is kept. The push replaces any earlier inbox commit with `--force-with-lease=<branch>:<sha it fetched>` (empty when the branch was absent), which the proxy allows, and is retried under the rule above. An inbox branch is pushed whether or not the repository has a relay: import reads unrelayed inboxes, so the events outlive the container either way. Its parent is the default branch's tip so that the push event finds the relay workflow. An inbox branch is never merged into a code branch; without a relay, inbox branches stay until a person or a job deletes them, since cloud sessions cannot.

  The relay workflow, `.github/workflows/redkiln-journal-relay.yml` (on push to `redkiln/inbox/**`, `contents: write`, `GITHUB_TOKEN` only, one run at a time per `<name>`), is the repository owner's opt-in to tidying them into refs. A run reads the inbox branch's tip when it starts, and does nothing if the branch is gone. It refuses an inbox commit that changes anything else, and leaves that branch for a person to look at. Otherwise it merges the inbox file into the ref. It deletes the inbox branch only after that push succeeded or the merge added nothing, and then only with a lease on the commit it processed (`--force-with-lease=<branch>:<sha>`); a rejected lease means a newer inbox commit arrived for the run its push started, and the delete is not retried. In every other case (a ref breaking JR-05, retries spent) it leaves the branch in place and reports.

The relay keeps honest writers honest; it is not a defence against someone with write access, who can change the workflow itself (JR-05). That includes a cloud agent: an inbox commit can carry an edited relay and run it with the relay's token, as any branch push can carry an edited workflow.

Readers fetch journal refs with an explicit refspec, `+refs/redkiln/journal/*:refs/redkiln/journal/*`, and compare each with the commit they last imported (JR-05). Import reads every journal source it finds in this format (JR-10): these refs; inbox branches not yet relayed, fetched with `+refs/heads/redkiln/inbox/*:refs/redkiln/inbox/*` and `--prune`; workflow artifacts (Review's CI runs export that way); and the legacy committed journals, weigh-in's `.kb/_ledger/journal/*.jsonl`, which are imported once and left in place. Which way a writer exports is a setting it may change; where journals live and how they are read is the one-way part (Ryan, 2026-11-01, refs spike).

**JR-08.** Export is one checkpoint, run at Stop and from a `pre-push` git hook (which does nothing when every ref being pushed is a journal ref or an inbox branch), never at SessionEnd, whose hooks share a 1.5-second budget (pressure test, Run modes). It writes every journal for which the store holds a durable event that journal lacks, imported events included: events go to their session's journal (JR-06), and events with no session to `store-<store-id>`, so a checkpoint run with no session of its own (a `pre-push` from a terminal) still exports every session's events. It is idempotent: each journal gains only the durable events whose `origin` it does not already hold, in store order.

**JR-09.** In a `client` repository the checkpoint writes nothing (ST-13).

## Import

**JR-10.** Import reads a journal line by line and appends each complete, well-formed envelope to the store, whether or not the importing binary knows its type: the log is shared, and a sibling may. A malformed line is skipped and reported with its file and line number, never fatal.

**JR-11.** Import is idempotent by identity. Each line is ingested with its `origin` as its stored identity (ID-02), and happenstance refuses a second event with the same identity, so importing the same journal twice, two journals holding the same line after a cherry-pick, or a journal written by this store before its id was re-minted (ID-01), appends each fact once.

**JR-12.** Every imported event is stored at `asserted` trust, with the line's level kept in `trust.claimed` (TR-06). A journal from a fork pull request therefore cannot satisfy a gate.

**JR-13.** Each import is recorded as one `core.import_completed` event naming the source (a path, a ref and commit, or an artifact), the lines read, imported, skipped as present and skipped as malformed.

**JR-14.** Import never deletes, truncates or rewrites its source.
