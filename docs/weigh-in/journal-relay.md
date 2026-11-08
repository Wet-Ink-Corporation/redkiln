# The journal relay

`.github/workflows/redkiln-journal-relay.yml` moves the inbox branches of Claude Code cloud
sessions into their journal refs (SPEC JR-07). A cloud session's proxy refuses
`refs/redkiln/journal/*`, so its Stop hook pushes `redkiln/inbox/<session>` instead, a branch
whose one commit adds `.redkiln/inbox/<session>.jsonl`. The workflow runs on each such push,
builds `weighin` from the default branch, and runs:

```sh
target/debug/weighin journal relay --json "$GITHUB_REF_NAME"
```

It merges the inbox file into `refs/redkiln/journal/<session>` by `origin` (JR-05) and deletes
the branch under a lease. Ryan approved the file under D8 A (build `weighin` from the default
branch on every run, restoring the Rust cache but never saving it; about 3 to 6 Actions minutes
per inbox push, coalesced per branch) and D13 (the file as drafted: `GITHUB_TOKEN` with
`contents: write` only, no secret).

Because it builds `weighin` from source, the workflow works only in a repository that holds the
`weighin` crate: this one. D8 C, a released binary fetched by digest once `rk-weigh-in` 2.0.0 is
published, is what would let another repository run it.

## What a run tells you

The run's last step prints the `weighin.journal-relay/1` envelope (SPEC CLI-01); `data.outcome`
says what happened.

| `outcome` | Exit | Meaning | What to do |
|---|---|---|---|
| `merged`, `deleted: "yes"` | 0 | The inbox is in the journal ref and the branch is gone | Nothing |
| `merged`, `deleted: "newer-arrived"` | 0 | Merged; a newer inbox commit arrived, so the branch stays for its own run | Nothing |
| `gone` | 0 | An earlier run already relayed it | Nothing |
| `merged`, `deleted: "failed"` | 1 | Merged, but the branch delete was refused | Allow `GITHUB_TOKEN` to delete `redkiln/inbox/*` under the rulesets, then delete the branch |
| `failed`, `retries-spent` | 1 | A fetch or push failed, or every retry was rejected; the branch is left in place | A refused push of `refs/redkiln/journal/*` means a ruleset blocks `GITHUB_TOKEN`: allow it, then rerun the run |
| `broken` | 1 | The journal ref breaks JR-05 | Move it aside to `refs/redkiln/broken/<session>/<sha>` (JR-05), then rerun |
| `refused`, `too-large` | 1 | The inbox commit is not one the relay moves, or merging it would make the journal too large | Read the finding; import by hand (`weighin journal import`) or delete the branch |

Exit 2, 70 or 74 is usage, could not run, or output failed (SPEC CLI-05): read the log and rerun.

An inbox left in place is not lost: `weighin journal import` also reads inbox branches not yet
relayed (SPEC JR-07, "Readers"), and the next push to the branch starts another run.

## Behaviour to know

- The relay's own pushes use `GITHUB_TOKEN`, which starts no workflow, so it never triggers
  itself; its branch delete is skipped by the job's `if`.
- Runs for one branch queue and never cancel each other (`concurrency` on the ref,
  `cancel-in-progress: false`), so a second push waits for the first relay.
- `actions/checkout` leaves `GITHUB_TOKEN` in the clone's local git config, which Core's git
  reads: Core drops only the `GIT_DIR` family and command-line config from git's environment.

## To verify on the first real inbox push

- GitHub starts the run for a branch the cloud proxy pushed under the session's identity.
- `GITHUB_TOKEN` may push `refs/redkiln/journal/*` and delete the branch under the repository's
  rulesets.
