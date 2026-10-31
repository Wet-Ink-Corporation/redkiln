# Ids, digests and hash methods

Ids are minted where the data is made, with no coordinator: parallel sessions, worktrees and clones all mint them. So ids are store-assigned or content hashes, never sequence numbers (weigh-in `id.rs`: sequential ids are what collided in beads and Backlog.md).

## Stores and events

**ID-01.** A store's id is happenstance's `StoreId`: 16 bytes, written as 32 lowercase hex characters with no dashes, minted when the store is created. happenstance requires a new id when a store's state is restored or cloned, so that two copies never issue the same event identity to different events. Core records the store's canonical path at creation and re-mints the id when it opens the store at another path (a copied `.git`, a restored backup). Events already in the store keep the identity they were written with, which is correct: both copies hold those same facts.

**ID-02 (one-way, OW-6).** An event's identity is happenstance's `EventId`: the id of the store that first accepted it and its position there, written `<store-id>/<position>` (`0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f/41`). It never changes. An event written locally takes its identity from the store; an exported event carries it as the envelope's `origin` (EV-01); an import keeps it, so the same fact imported twice, from two journals or two clones, is one event (JR-10).

**ID-03.** An event ref, in `cites` and in tags, is an event's identity in the same text form.

## Content-hash ids

**ID-04 (one-way, OW-8).** A content-hash id is a registered prefix, a dash and the first *n* lowercase hex characters of a sha256 over a domain-separated input. New ids use *n* = 10. A writer that finds the same id already naming a different input extends to 12, then 16. Ids already written are read forever at their length: today's 6-hex and 8-hex `wi-` ids stay valid, and nothing renumbers them.

The input is the domain string followed by each field, each written as its UTF-8 byte length as a 64-bit big-endian unsigned integer and then its bytes. The domain names the id kind and its derivation version (`redkiln.id/weighin.item/1`), so a changed derivation never mints an id that collides with an old one.

| Prefix | Names | Derived from | Owner |
| --- | --- | --- | --- |
| `wi-` | A Weigh-In item | A captured item: session and normalised question, as today. A decision request: the raiser's namespace and correlation key (DR-02) | Weigh-In |
| `kb-<kind>-` | A Knowledge atom (`kb-decision-…`) | Defined by Knowledge (Knowledge call C) | Knowledge |
| `wt-` | A worktree | ID-06 | Core |

Ten hex characters keep the chance of two clones minting the same id below one in a million at a thousand items, where six hex characters gave about 3% (pressure test, Git scenarios). Because ids are minted in separate clones, uniqueness is also checked where the clones meet: Knowledge's `validate` checks every id in the merged `.kb` in CI.

**ID-05.** A session id is the Claude Code session id as the hook input or `CLAUDE_SESSION_ID` or `CLAUDE_CODE_SESSION_ID` gives it. An empty value, or a `${…}` a surface did not substitute, is no session (weigh-in `env.rs`). A CI writer uses `gha:<repo_id>/<run_id>/<run_attempt>` (assay's run uid).

**ID-06 (one-way, OW-7).** A worktree id is `wt-` and the first 12 hex characters of the sha256 of the worktree's normalised top-level path: the absolute path `git rev-parse --show-toplevel` prints, with `/` separators and no trailing slash, ASCII-lowercased on Windows and macOS. Every per-worktree projection keys on it.

Today the two ledgers spell the worktree two ways: anneal writes the normalised path, weigh-in only its last directory name, which two clones can share (pressure test, Git scenarios). The SPEC takes anneal's normalisation and hashes it, so a committed journal never carries a local absolute path.

## Digests

**ID-07.** A digest is written `<method>:<lowercase hex>`. A file's bytes are `sha256:<64 hex>`, the form anneal's plan approvals use.

**ID-08.** The canonical digest of a JSON value is the sha256 of its RFC 8785 (JCS) serialisation, written `sha256:<64 hex>`. A payload's canonical digest is taken over `data` alone, never over the envelope or tags (EV-13). Loop's `canon.rs` is compared with this at Move 4; if they differ, Loop's digests keep their own domain and the SPEC names both.

## Hash methods

**ID-09.** An approval, a verdict or a gate result binds to the hash of its subject, taken by a named method. Core implements each method once, and each method survives different git operations (pressure test, Approvals and hash methods). A subject tag is `<method>:<hash>`.

| Method | Hash of | Form | Survives a rebase onto a moved base | Survives a squash merge |
| --- | --- | --- | --- | --- |
| `sha256` | A file's bytes | `sha256:<64 hex>` | Yes | Yes |
| `git-commit` | A commit | `git-commit:<40 or 64 hex>` | No | No |
| `git-tree` | The worktree's tree: the current index plus tracked modifications, untracked files excluded, written on a temporary index (Loop ADR-0105 clause 4, weigh-in `git.rs`) | `git-tree:<40 or 64 hex>` | No | No |
| `patch-id` | `git patch-id --verbatim` of `git diff <base>...<commit>`, recorded with its base | `patch-id:<40 or 64 hex>` | Yes, when the rebase is clean | Yes |
| `rust-fingerprint` | Redkiln Rust's change fingerprint over Rust pathspecs, byte-identical with temper 1.x | `rust-fingerprint:<hex>` | Only when the base changed no Rust file | Only when the base changed no Rust file |

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
