# Git

Every redkiln product reads git, and Loop and Review write it: trailers, refs, worktrees, leases. Loop and assay both arrived at a hardened, single-wrapper git layer; this chapter is what they and the others learned.

## Guardrails

### GT-1 All git goes through one wrapper with a closed set of operations

The wrapper takes an operation enum, never a free string, so a new git write is a new variant that review can see. It adds the flags no call site may forget (`core.hooksPath` pointing at an empty directory, `trailer.ifexists=add` on commits). Git's stderr is kept for diagnosis and never parsed for control flow. CI bans `Command::new("git")` outside the wrapper, with a positive half: the wrapper itself must contain the call, or the ban guards nothing.

- **Evidence.** redkiln-rs `crates/rk-git/src/git_exec.rs`, `.github/workflows/ci.yml:500-558`, ADR-0077, ADR-0105, ADR-0193; blueprint (Loop's rule "git through one wrapper").

### GT-2 Shell out to the git CLI for writes and network; read with a library only where it is exact

gix cannot write trailers, and a hand-rolled trailer writer matched `git interpret-trailers` on only 3 of 5 cases. Credentials, hooks and user configuration come free with the CLI. Batch reads (`cat-file --batch`) where volume matters: the Node Redkiln cut about 1,100 spawns to about 22 that way, and a lock once held 160 s mostly in git spawns.

- **Evidence.** redkiln-rs S-GIX, ADR-0091; redkiln-old RFC §3.5, `src/store/ids.ts:196`; prior art below (jj, cargo).

### GT-3 Never classify git by its message text

Map outcomes from exit codes and porcelain output. Where only a message distinguishes cases, accept every known wording and test against the git versions you support.

- **Why.** git 2.56 changed "not a git repository" to "gitfile does not point to a valid repository", and anneal's classifier broke on macOS and Windows runners and three Dependabot PRs.
- **Evidence.** anneal `@af95565`.

### GT-4 Trailers are parsed from the final trailer paragraph, on every line of a squashed message

Use git's own rules (`interpret-trailers --parse`, or `%(trailers)`), limited to the commit's final trailer block. A squash merge puts trailers mid-body, so a reader that must see them across squash merges reads every message line, deliberately and with a test.

- **Evidence.** redkiln-rs #18; weigh-in `crates/weighin/src/ci/trailer.rs` (`ci-check` reads trailers on every line); redkiln `main` allows merge commits only, which keeps each PR's trailers where they were written (wi-e3e5af).

### GT-5 Compare-and-set only where the platform really provides it

`git update-ref <ref> <new> <old>` and `git push --force-with-lease=<ref>:<old>` are genuine compare-and-set; so are GitHub's Contents API PUT with a blob sha and `POST git/refs` on a new ref. `DELETE /git/refs` and check-run updates take no precondition, so delete through `--force-with-lease` and serialise check writes under a lock ref with a short TTL. When a lease and its state ref change together, update both in one `update-ref --stdin -z` transaction.

- **Evidence.** assay R3-06(d), R3-09, `crates/assay-forge/src/github/ledger_refs.rs:1-35`, `crates/assay-forge/src/git/push.rs:187-218`; redkiln-rs `GitOp::UpdateRef`; prior-art/platform.md §2.

### GT-6 Patch-id keeps a verdict fresh across a rebase, and nothing else

A verdict recorded for one head stays fresh for another when `git patch-id --verbatim` over a fully pinned diff matches, each taken against its own merge base, so a clean rebase or squash keeps it. Pin the diff so the id depends only on content, with SPEC ID-09's recipe: `--no-color --no-ext-diff --no-textconv --no-renames --no-relative --ignore-submodules=none --submodule=short --text --full-index -U3 --inter-hunk-context=0 --diff-algorithm=myers --indent-heuristic --src-prefix=a/ --dst-prefix=b/ -O/dev/null`, with `core.quotePath=true`, `diff.suppressBlankEmpty=false` and `core.useReplaceRefs=false`, `GIT_DIFF_OPTS` and `GIT_EXTERNAL_DIFF` unset and `GIT_GRAFT_FILE=/dev/null`. anneal's flags (`--binary` without `--text`) left attributes open: a `-diff` or `binary` attribute from `core.attributesFile`, `info/attributes` or the checked-out tree changed its id, and its ids differ from ID-09's for any change with a binary file. Check every process's exit status, and refuse an ambiguous merge base and a shallow repository: a diff that dies part way gives the id of a smaller change, and a shallow boundary can move the base. `--verbatim` keeps whitespace, which is code in Python, YAML and Makefiles.

- **Evidence.** anneal `docs/verdict.md:128-143`, `crates/anneal-cli/src/pr_diff.rs`, `@cdc9787`; SPEC ID-09 (`patch-id` hash method, flags pinned 2026-11-01, fixture `spec/fixtures/patch-id/`); [Tension T10](README.md#tensions-with-the-plan), resolved.

### GT-7 Worktree ownership comes from the store, never from the cwd

A function that writes to a worktree takes an ownership token minted from the store, not `current_dir`. A cwd-based version "passes the refusal test and the positive control by accident". Lanes live in a git-ignored directory (a nested worktree pollutes recursive greps), are locked with a reason that names pid and start time, and are reset on re-entry with `reset --hard` and `clean -fd` (never `-x`). Telemetry labels the worktree root, never `basename(cwd)`.

- **Evidence.** redkiln-rs `crates/rk-git/src/worktree.rs`, ADR-0180, ADR-0188, AC-529, D21-40; redkiln-old A7-03.

### GT-8 Git over untrusted objects is hardened

Treat PR objects as data: `GIT_CONFIG_NOSYSTEM`, `GIT_CONFIG_GLOBAL=/dev/null`, `core.hooksPath=/dev/null`, `https` only, fsck on fetch, `attr.tree` set to the empty tree (a bare repository otherwise reads HEAD's `.gitattributes`, and `-diff` turns numstat counts into `-` even with `--text`), `--no-ext-diff --no-textconv`, every process its own group. A checkout handed to a gate is verified against the fetched tree: kinds, execute bit, bytes and symlink targets.

- **Evidence.** assay `docs/architecture/03-contracts.md` §9, decisions log, #60 ST-7 and ST-8.

### GT-9 Fixture repositories are hermetic

Every fixture repository sets `init -b main`, a repo-local `user.name` and `user.email` (hosted runners have none), `maintenance.auto=false` and `gc.auto=0`. Background `git maintenance` deleted a planted worktree entry and put a lock file into a "mutates nothing" fingerprint, flaking a test on one trigger of the same SHA and not the other.

- **Evidence.** redkiln-rs `@f433854`, `@ef747da`; anneal `@82bb54c`.

### GT-10 Halt, do not repair, shared git state

When `.git/config`, the object store or refs shared by several worktrees look corrupt, an agent stops and asks. It does not guess a rewrite.

- **Evidence.** redkiln-old `@78c29dba` (519 NUL bytes in a `.git/config` shared by ten worktrees; a subagent attempted a raw rewrite).

## Patterns worth copying

- **Leases as parentless empty-tree commits whose message is the lease**, created with `POST git/refs` so creation is atomic, released by compare-and-set on the lease commit (assay `ledger_refs.rs`; blueprint, Capacity).
- **Resume from git trailers on two axes**, because a rejected slice and a passed one look the same in history (redkiln-old `Story:` and `Slice-Verdict:`; redkiln-rs `Story: <project>/<slug> <stage>`).
- **A committed generated bundle is `-merge`**, so two rebuilt bundles conflict instead of merging line by line into a file no build produced (redkiln-old `.gitattributes`, `test/bundle-merge-attr.test.ts`). A rebuilding merge driver was rejected because it fails open on a fresh clone.
- **`ci-check` reads only git and the checkout**, never the local store, and settles order by commit order in the range, not timestamps (weigh-in B-28, `crates/weighin/src/ci.rs`).

## Gotchas

- **Custom refs are not fetched by default**; document the refspec each ref family needs (git-appraise, prior art below). The Anthropic cloud git proxy allows only `refs/heads/*` pushes and blocks branch deletion, which is why OW-13 relays cloud journals through an inbox branch (refs spike).
- **"Require linear history" plus "merge commits only" leaves no merge button** (weigh-in D-0020).
- **Windows `canonicalize` returns `\\?\D:\…`**, which slash-flipped becomes `//?/D:/…`, a URL to git ([XP-3](cross-platform.md#xp-3-compare-paths-after-resolving-them-and-never-hand-git-a-verbatim-path)).
- **`core.autocrlf=true` at system level smudged 1,480 of 1,757 files** in a run-owned worktree ([XP-1](cross-platform.md#xp-1-lf-everywhere-pinned-first-in-gitattributes)).

## Prior art

- `update-ref` three-argument compare-and-set and `--stdin` start/prepare/commit transactions. **Agree; adopt** transactions when a lease and its state move together. [git-update-ref](https://git-scm.com/docs/git-update-ref)
- Gerrit NoteDb keeps all review state in refs beside the code, fuses code and meta updates into one batch, and retries on lock contention. **Agree; adopt** bounded retry with back-off on lock failure. [Gerrit NoteDb](https://gerrit.googlesource.com/homepage/+/613ad8f7599c754e1f1539b8b3ebebe29960f757/pages/gerrit/note-db.md)
- git-bug stores entities as operation logs in commit chains under custom refs, ordered by Lamport clocks. **Adopt** Lamport-clock-plus-id ordering if refs from two machines ever merge. [git-bug data model](https://www.github.com/MichaelMure/git-bug/blob/master/doc/model.md)
- jj moved fetch and push onto the git binary, using `--force-with-lease` as test-and-set, with a declared minimum git version. **Agree; adopt** a git version check at start-up. [jj-lib git_subprocess](https://docs.rs/jj-lib/latest/src/jj_lib/git_subprocess.rs.html)
- Cargo keeps `net.git-fetch-with-cli` because libgit2 lags on authentication. **Agree.** [cargo fetch_with_cli](https://doc.rust-lang.org/stable/nightly-rustc/cargo/sources/git/utils/fn.fetch_with_cli.html)
- Worktrees share every `refs/*` except `refs/worktree`, `refs/bisect` and `refs/rewritten`; `worktree add --lock` avoids the add-then-lock race. **Adopt** `refs/worktree/redkiln/` for lane-local state and `add --lock`. [git-worktree](https://git-scm.com/docs/git-worktree)
- Monorepo import by `merge -s ours` plus `read-tree --prefix`, never squashed. **Agree**: it is the migration plan's recipe. Tomas Votruba argues history is not worth keeping; **we differ**, because cited hashes must resolve. [tomono](https://github.com/partikus-archive/tomono/blob/master/tomono.sh), [Votruba](https://tomasvotruba.com/blog/2020/06/15/how-to-create-monorepo-from-existing-repositories-in-7-steps)
- Josh projects a monorepo subdirectory as a standalone repository, reversibly. **Later**: a candidate for per-plugin mirrors. [josh](https://github.com/josh-project/josh)
