# Ids, digests and hash methods

Ids are minted where the data is made, with no coordinator: parallel sessions, worktrees and clones all mint them. So ids are store-assigned or content hashes, never sequence numbers (weigh-in `id.rs`: sequential ids are what collided in beads and Backlog.md).

## Stores and events

**ID-01.** A store's id is happenstance's `StoreId`: 16 bytes, written as 32 lowercase hex characters with no dashes, minted when the store is created. happenstance requires a new id when a store's state is restored or cloned, so that two copies never issue the same event identity to different events. Core records the store's canonical path at creation and re-mints the id when it opens the store at another path (a copied `.git`, a restored backup). Events already in the store keep the identity they were written with, which is correct: both copies hold those same facts.

**ID-02 (one-way, OW-6).** An event's identity is happenstance's `EventId`: the id of the store that first accepted it and its position there, written as happenstance writes it, `<store-id>:<position>` (`0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:41`). It never changes. happenstance keeps it in the store's `origin_store` and `origin_position` columns, unique together. An event written locally takes its identity from the store; an exported event carries it as the envelope's `origin` (EV-01); an import stores it in those same columns through happenstance's ingest path, so the same fact imported twice, from two journals or two clones, is one event (JR-11). Core keeps identity nowhere else; the pinned happenstance must offer that ingest call (ST-04).

**ID-03.** An event ref, in `cites` and in tags, is an event's identity in the same text form.

## Content-hash ids

**ID-04 (one-way, OW-8, OW-15).** A content-hash id is a registered prefix, a dash and the first *n* lowercase hex characters of a sha256 over a domain-separated input. New ids use *n* = 10. A writer that finds the same id already naming a different input extends to 12, then 16. Ids already written are read forever at their length: today's 6-hex and 8-hex `wi-` ids stay valid, and nothing renumbers them.

The input is the domain string followed by each field, each written as its UTF-8 byte length as a 64-bit big-endian unsigned integer and then its bytes. The domain names the id kind and its derivation version (`redkiln.id/weighin.item/1`), so a changed derivation never mints an id that collides with an old one. One exception keeps today's ids stable: Weigh-In's captured items keep their current input, `sha256(session \n normalised question)` with no domain or length prefixes, so a 10-hex id is the 6-hex id it always was, extended.

| Prefix | Names | Derived from | Owner |
| --- | --- | --- | --- |
| `wi-` | A Weigh-In item | A captured item: session and normalised question, as today (the exception above). A decision request: the raiser's namespace and correlation key (DR-02) | Weigh-In |
| `kb-<kind>-` | A Knowledge atom (`kb-decision-…`) | Defined by Knowledge (Knowledge call C) | Knowledge |
| `wt-` | A worktree | ID-06 | Core |

Ten hex characters keep the chance of two clones minting the same id below one in a million at a thousand items, where six hex characters gave about 3% (pressure test, Scenarios). Because ids are minted in separate clones, uniqueness is also checked where the clones meet: Knowledge's `validate` checks every id in the merged `.kb` in CI.

**ID-05.** A session id is the Claude Code session id as the hook input or `CLAUDE_SESSION_ID` or `CLAUDE_CODE_SESSION_ID` gives it. An empty value, or a `${…}` a surface did not substitute, is no session (weigh-in `env.rs`). A CI writer uses `gha:<repo_id>/<run_id>/<run_attempt>` (assay's run uid).

**ID-06 (one-way, OW-7).** A worktree id is `wt-` and the first 12 hex characters of the sha256 of the worktree's normalised top-level path: the absolute path `git rev-parse --show-toplevel` prints, with `/` separators and no trailing slash, ASCII-lowercased on Windows and macOS. Every per-worktree projection keys on it.

Today the two ledgers spell the worktree two ways: anneal writes the normalised path, weigh-in only its last directory name, which two clones can share (pressure test, Scenarios). The SPEC takes anneal's normalisation and hashes it, so a journal never carries a local absolute path.

## Digests

**ID-07.** A digest is written `<method>:<lowercase hex>`. A file's bytes are `sha256:<64 hex>`, the form anneal's plan approvals use.

**ID-08.** The canonical digest of a JSON value is the sha256 of its RFC 8785 (JCS) serialisation, written `sha256:<64 hex>`. A payload's canonical digest is taken over `data` alone, never over the envelope or tags (EV-13). Loop's `canon.rs` is compared with this at Move 4; if they differ, Loop's digests keep their own domain and the SPEC names both.

## Hash methods

**ID-09 (one-way, OW-14).** An approval, a verdict or a gate result binds to the hash of its subject, taken by a named method. Core implements each method once, and each method survives different git operations (pressure test, Which hash an approval binds to). A subject tag is `<method>:<hash>`.

| Method | Hash of | Form | Survives a rebase onto a moved base | Survives a squash merge |
| --- | --- | --- | --- | --- |
| `sha256` | A file's bytes | `sha256:<64 hex>` | Yes | Yes |
| `git-commit` | A commit | `git-commit:<40 or 64 hex>` | No | No |
| `git-tree` | The worktree's tree: the current index plus tracked modifications, untracked files excluded, written on a temporary index (Loop ADR-0105 clause 4, weigh-in `git.rs`) | `git-tree:<40 or 64 hex>` | No | No |
| `patch-id` | `git patch-id --verbatim` of the pinned diff below, from `<base>...<commit>`, recorded with its base | `patch-id:<40 or 64 hex>` | Yes, when the rebase is clean | Yes |
| `rust-fingerprint` | Redkiln Rust's change fingerprint over Rust pathspecs, byte-identical with temper 1.x | `rust-fingerprint:<hex>` | Only when the base changed no Rust file | Only when the base changed no Rust file |

The `patch-id` diff is pinned so that its id depends only on the change: not on git configuration (colour, renames, prefixes, context, diff algorithm, indent heuristic, path quoting, submodule format, external diff and textconv drivers), not on attributes from any source (`core.attributesFile`, `.git/info/attributes`, a checked-out `.gitattributes`, `diff=` drivers, `binary`, `-diff`), and not on replace refs, grafts or a shallow boundary. It runs with `GIT_DIFF_OPTS` and `GIT_EXTERNAL_DIFF` unset, since the first overrides `-U`, and `GIT_GRAFT_FILE` pointing at `/dev/null`, and needs git 2.39 or later:

```sh
(
  unset GIT_DIFF_OPTS GIT_EXTERNAL_DIFF
  GIT_GRAFT_FILE=/dev/null; export GIT_GRAFT_FILE
  git -c core.quotePath=true -c diff.suppressBlankEmpty=false -c core.useReplaceRefs=false \
    -c advice.graftFileDeprecated=false \
    diff --no-color --no-ext-diff --no-textconv --no-renames --no-relative \
    --ignore-submodules=none --submodule=short --text --full-index -U3 \
    --inter-hunk-context=0 --diff-algorithm=myers --indent-heuristic \
    --src-prefix=a/ --dst-prefix=b/ -O/dev/null <base>...<commit>
) | git patch-id --verbatim
```

`--text` diffs every file as text, so no attribute can turn a file into "Binary files differ" (binary content still counts). `--verbatim` keeps whitespace, which is code in Python, YAML and Makefiles, and since git 2.39 implies `--stable`, so file order does not matter. The id is the first field of patch-id's output. `--full-index`, `-O/dev/null` and the hunk headers' function context (`diff=cpp`, `xfuncname`) change only the diff's bytes; patch-id ignores all three, and the first two pin those bytes for anyone who reads them.

There is an id only when all of these hold; otherwise the subject has none, and nothing is bound to `patch-id:` with an empty or partial hash:
- The repository is not shallow (`git rev-parse --is-shallow-repository` prints `false`): a shallow boundary acts as a graft that `GIT_GRAFT_FILE` does not turn off, and it can move the merge base to an older commit and so give a wrong id rather than none. A shallow clone deepens or unshallows first.
- `git merge-base --all <base> <commit>`, run under the same environment and `core.useReplaceRefs=false`, names exactly one commit, the base `...` diffs against. A criss-cross history has no id, even when a replace ref makes it look linear.
- `git diff` and `git patch-id` both exit 0. A diff that dies part way, on a missing object in a partial clone or a corrupt pack, prints the id of a smaller change, so an implementation checks each process's status rather than run the pipeline as written.
- patch-id prints one line. An empty diff has no id.

The id's length follows the repository's object format, 40 hex for SHA-1 and 64 for SHA-256, and ids from the two formats never compare equal. anneal's ids, taken with `--binary` and without `--text`, differ from these for any change with a binary file, so they are not `patch-id` subjects.

`spec/fixtures/patch-id/check.sh` is the conformance fixture: a change sensitive to each pinned input, the id taken under each hostile setting, attribute file, environment variable, replace ref and graft in turn, with a control per case showing that a plain `git diff` under it gives another id, and the cases that must give no id: a missing object, no change, a criss-cross history (also behind a replace ref) and a shallow boundary that would move the base. CI runs it. A change to this recipe changes what every recorded `patch-id` means, so from SPEC 1.0 on it is a new method name, never an edit (OW-14); before then, an edit needs the owner's decision and is recorded as a decisions row, as on 2026-11-02 (`docs/decisions.md`).

The `git-tree` temporary index lives on the local temporary disk, never beside the store, since git replaces it by rename and Cowork refuses deletes.

**ID-10.** Which method binds what:

| Subject | Method |
| --- | --- |
| A plan, spec, design or brief that was approved | `sha256` of the file |
| A review verdict | `patch-id` plus its base |
| A gate result | The gate's declared method: `rust-fingerprint` for Redkiln Rust's gate, `git-tree` for Loop's verify gate |
| A gate on exactly this snapshot | `git-tree` |
| What CI ran | `git-commit` |

So merge queues and auto-rebase re-run gates without voiding human approvals. A new method is a SPEC minor.
