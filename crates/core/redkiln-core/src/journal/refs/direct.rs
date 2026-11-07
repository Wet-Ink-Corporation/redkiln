//! The two ways a journal is written (SPEC JR-07): straight into its ref, or into its inbox
//! branch for the relay. Each merges by `origin` and retries a rejected push at most
//! [`RETRIES`] times, fetching first.

use super::git::{Fetched, Pushed};
use super::{
    BASE_REF, Breach, Ctx, InboxCopy, Oid, RETRIES, RefsError, State, inbox_branch, inbox_path,
    inbox_ref, journal_ref, read_inbox, read_journal,
};
use crate::journal::merge::{Merged, merge};
use crate::journal::name::JournalName;

/// How a direct write ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Direct {
    /// The ref already held every line.
    UpToDate,
    /// The ref moved, gaining this many lines.
    Moved(u64),
    /// The ref breaks JR-05, so it was left alone.
    Broken(Breach),
    /// The merged file would be larger than the cap every reader holds to, so nothing was
    /// written: a ref no reader can read would stop every later write.
    TooLarge,
    /// Every retry was rejected.
    Spent,
    /// The deadline passed before the write.
    Late,
    /// The push or fetch failed for another reason, with what git said.
    Failed(String),
}

/// Merges `incoming` into `name`'s journal ref and pushes it to the context's remote, or, with
/// none, moves the local ref only.
pub(crate) fn merge_direct(
    ctx: &Ctx<'_>,
    name: &JournalName,
    incoming: &[u8],
) -> Result<Direct, RefsError> {
    let refname = journal_ref(name);
    for attempt in 0..=RETRIES {
        if ctx.late() {
            return Ok(Direct::Late);
        }
        let (parent, current) = match current_journal(ctx, name)? {
            Ok(current) => current,
            Err(done) => return Ok(done),
        };
        let merged = merge(&current, incoming);
        if merged.added == 0 {
            return Ok(Direct::UpToDate);
        }
        if over(ctx, &merged) {
            return Ok(Direct::TooLarge);
        }
        let blob = ctx.repo.hash_blob(&merged.bytes)?;
        let tree = ctx.repo.single_file_tree(&name.file_name(), &blob)?;
        let message = format!("redkiln journal {name} [skip ci]");
        let commit = ctx.repo.commit(&tree, parent.as_ref(), &message)?;
        let Some(remote) = ctx.remote else {
            // With no remote the local ref is the ref of record: moved only from the commit
            // read, or merged again on top of whatever another writer moved it to.
            if ctx.repo.move_ref(&refname, &commit, parent.as_ref())? {
                return Ok(Direct::Moved(merged.added));
            }
            continue;
        };
        match ctx
            .repo
            .push(remote, &format!("{commit}:{refname}"), None)?
        {
            Pushed::Done => {
                // The clone's copy follows unless another writer already moved it past the
                // commit read; it is never moved back.
                ctx.repo.move_ref(&refname, &commit, parent.as_ref())?;
                return Ok(Direct::Moved(merged.added));
            }
            Pushed::Failed(said) => return Ok(Direct::Failed(said)),
            Pushed::Rejected if attempt == RETRIES => return Ok(Direct::Spent),
            Pushed::Rejected => {
                if let Some(failed) = refetch(ctx, remote, &refname, &refname)? {
                    return Ok(Direct::Failed(failed));
                }
            }
        }
    }
    Ok(Direct::Spent)
}

/// A journal ref as a write starts from: its tip (`None`: a root commit comes next) and file.
type Current = (Option<Oid>, Vec<u8>);

/// The clone's journal ref of `name`: its tip and file, or the outcome that ends the write.
/// A copy that breaks JR-05 is fetched again first, since a person may have moved the remote
/// ref aside to `refs/redkiln/broken/` (then it is absent, and the next commit is a root).
fn current_journal(
    ctx: &Ctx<'_>,
    name: &JournalName,
) -> Result<Result<Current, Direct>, RefsError> {
    let mut state = read_journal(ctx, name)?;
    if let (State::Broken(_), Some(remote)) = (&state, ctx.remote) {
        let refname = journal_ref(name);
        if let Some(failed) = refetch(ctx, remote, &refname, &refname)? {
            return Ok(Err(Direct::Failed(failed)));
        }
        state = read_journal(ctx, name)?;
    }
    Ok(match state {
        State::Absent => Ok((None, Vec::new())),
        State::Good { tip, file } => Ok((Some(tip), file)),
        State::Broken(breach) => Err(Direct::Broken(breach)),
    })
}

/// How an inbox write ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Inboxed {
    /// The inbox would not change, so nothing was pushed.
    Same,
    /// The inbox branch was pushed; this many of the checkpoint's lines were new to it.
    Pushed(u64),
    /// The default branch already holds the inbox's path, or a file where one of its folders
    /// goes, so no commit can only add it.
    Occupied,
    /// The inbox branch's file is larger than the cap, so it cannot be merged.
    InboxTooLarge,
    /// The merged file would be larger than the cap, so nothing was pushed.
    TooLarge,
    Spent,
    Late,
    Failed(String),
}

/// Where inbox branches go: the remote, and its default branch's tip, every inbox commit's
/// parent.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Inbox<'a> {
    pub(crate) remote: &'a str,
    pub(crate) base: &'a Oid,
}

/// Pushes `name`'s inbox branch: one commit on the base adding `.redkiln/inbox/<name>.jsonl`,
/// the merge of the journal ref's file, the inbox's file and `own`, under a lease on the inbox
/// tip it read.
pub(crate) fn write_inbox(
    ctx: &Ctx<'_>,
    inbox_to: Inbox<'_>,
    name: &JournalName,
    own: &[u8],
) -> Result<Inboxed, RefsError> {
    let Inbox { remote, base } = inbox_to;
    let path = inbox_path(name);
    if occupied(ctx, base, &path)? {
        return Ok(Inboxed::Occupied);
    }
    let branch = inbox_branch(name);
    let local = inbox_ref(name);
    for attempt in 0..=RETRIES {
        if ctx.late() {
            return Ok(Inboxed::Late);
        }
        let (expected, merged) = match inbox_content(ctx, name, own)? {
            Content::Same => return Ok(Inboxed::Same),
            Content::InboxTooLarge => return Ok(Inboxed::InboxTooLarge),
            Content::New(expected, merged) => (expected, merged),
        };
        if over(ctx, &merged) {
            return Ok(Inboxed::TooLarge);
        }
        let blob = ctx.repo.hash_blob(&merged.bytes)?;
        let tree = ctx.repo.tree_with(base, &path, &blob)?;
        let message = format!("redkiln inbox {name} [skip ci]");
        let commit = ctx.repo.commit(&tree, Some(base), &message)?;
        let lease = format!("{branch}:{}", expected.as_ref().map_or("", Oid::as_str));
        match ctx
            .repo
            .push(remote, &format!("{commit}:{branch}"), Some(&lease))?
        {
            Pushed::Done => {
                // As for journal refs, the clone's copy is never moved back.
                ctx.repo.move_ref(&local, &commit, expected.as_ref())?;
                return Ok(Inboxed::Pushed(merged.added));
            }
            Pushed::Failed(said) => return Ok(Inboxed::Failed(said)),
            Pushed::Rejected if attempt == RETRIES => return Ok(Inboxed::Spent),
            Pushed::Rejected => {
                let refname = journal_ref(name);
                for (from, to) in [(branch.as_str(), local.as_str()), (&refname, &refname)] {
                    if let Some(failed) = refetch(ctx, remote, from, to)? {
                        return Ok(Inboxed::Failed(failed));
                    }
                }
            }
        }
    }
    Ok(Inboxed::Spent)
}

/// Whether `merged` is larger than the context's cap.
fn over(ctx: &Ctx<'_>, merged: &Merged) -> bool {
    u64::try_from(merged.bytes.len()).map_or(true, |len| len > ctx.max_bytes)
}

/// Whether `base` holds `path`, or a file where one of its folders goes: either way no commit
/// on `base` can only add `path`.
fn occupied(ctx: &Ctx<'_>, base: &Oid, path: &str) -> Result<bool, RefsError> {
    let folders = path.match_indices('/').filter_map(|(at, _)| path.get(..at));
    for folder in folders {
        match ctx.repo.entries(base, Some(folder))?.as_deref() {
            Some([]) => return Ok(false),
            Some([entry]) if entry.kind == "tree" && entry.path == folder => {}
            _ => return Ok(true),
        }
    }
    Ok(ctx
        .repo
        .entries(base, Some(path))?
        .is_none_or(|entries| !entries.is_empty()))
}

/// What the inbox should hold.
enum Content {
    /// The inbox would not change.
    Same,
    /// The inbox's file is larger than the cap.
    InboxTooLarge,
    /// The inbox tip read (`None`: no inbox) and the merge of the journal ref's file, the
    /// inbox's and the checkpoint's own.
    New(Option<Oid>, Merged),
}

/// What `name`'s inbox should hold once `own` is merged in.
fn inbox_content(ctx: &Ctx<'_>, name: &JournalName, own: &[u8]) -> Result<Content, RefsError> {
    // A ref breaking JR-05 counts as empty here; the inbox is still pushed (JR-07).
    let journal = match read_journal(ctx, name)? {
        State::Good { file, .. } => file,
        State::Absent | State::Broken(_) => Vec::new(),
    };
    Ok(match read_inbox(ctx, name)? {
        InboxCopy::TooLarge => Content::InboxTooLarge,
        InboxCopy::Read { tip, file } => {
            let merged = merge(&merge(&journal, &file).bytes, own);
            if merged.bytes == file {
                Content::Same
            } else {
                Content::New(Some(tip), merged)
            }
        }
        InboxCopy::Absent => {
            let merged = merge(&journal, own);
            if merged.added > 0 {
                Content::New(None, merged)
            } else {
                Content::Same
            }
        }
    })
}

/// Fetches the remote ref `from` into the local `to`, deleting `to` when the remote has no
/// `from`; what git said when the fetch failed otherwise.
pub(crate) fn refetch(
    ctx: &Ctx<'_>,
    remote: &str,
    from: &str,
    to: &str,
) -> Result<Option<String>, RefsError> {
    match ctx.repo.fetch(remote, &[format!("+{from}:{to}")], false)? {
        Fetched::Done => Ok(None),
        Fetched::Missing => ctx.repo.delete_ref(to).map(|()| None),
        Fetched::Failed(said) => Ok(Some(said)),
    }
}

/// Fetches the remote default branch's tip into [`BASE_REF`], and the inbox branches, pruned,
/// in a fetch of their own (JR-07): the tip, or what went wrong.
pub(crate) fn prepare_relay(ctx: &Ctx<'_>, remote: &str) -> Result<Result<Oid, String>, RefsError> {
    let branch = match ctx.repo.default_branch(remote)? {
        Ok(branch) => branch,
        Err(said) => return Ok(Err(said)),
    };
    let base = format!("+refs/heads/{branch}:{BASE_REF}");
    match ctx.repo.fetch(remote, &[base], false)? {
        Fetched::Done => {}
        Fetched::Missing => return Ok(Err(format!("the remote has no branch {branch}"))),
        Fetched::Failed(said) => return Ok(Err(said)),
    }
    let inboxes = format!("+{}*:{}*", super::INBOX_BRANCHES, super::INBOX_REFS);
    if let Fetched::Failed(said) = ctx.repo.fetch(remote, &[inboxes], true)? {
        return Ok(Err(said));
    }
    match ctx.repo.commit_of(BASE_REF)? {
        Some(tip) => Ok(Ok(tip)),
        None => Ok(Err("the default branch was not fetched".to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::process::Command;
    use std::time::Duration;

    use super::*;
    use crate::journal::export::MAX_JOURNAL_BYTES;
    use crate::journal::refs::RefsTarget;

    fn git(dir: &Path, args: &[&str]) -> String {
        // Spawned as Core spawns, never while another test's store opener holds its lock.
        let _forking = crate::git::forking();
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git");
        assert!(
            out.status.success(),
            "{args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout)
            .expect("utf-8")
            .trim()
            .to_owned()
    }

    fn line(position: u32) -> String {
        format!(
            r#"{{"type":"weighin.item_raised","version":1,"ts":"2026-10-31T04:12:40Z","actor":"agent","trust":{{"level":"asserted"}},"writer":{{"binary":"weighin","version":"2.0.0","spec":"1.0.0"}},"origin":"0f3c9a1e6b7d4c2a8e5f1b3d7c9a2e4f:{position}","data":{{"item":"wi-{position}"}}}}"#
        ) + "\n"
    }

    fn ctx<'a>(target: &RefsTarget<'a>) -> Ctx<'a> {
        let mut ctx = Ctx::local(target);
        ctx.remote = Some(target.remote);
        ctx
    }

    /// A bare remote whose `main` has one commit (holding `files`), and clones of it.
    fn remote_with_clones<const N: usize>(
        dir: &Path,
        files: &[&str],
        names: [&str; N],
    ) -> [std::path::PathBuf; N] {
        let remote = dir.join("remote.git");
        std::fs::create_dir_all(&remote).expect("dir");
        git(&remote, &["init", "-q", "--bare", "-b", "main"]);
        let seed = dir.join("seed");
        std::fs::create_dir_all(&seed).expect("dir");
        git(&seed, &["init", "-q", "-b", "main"]);
        for file in files.iter().chain(&["README.md"]) {
            let path = seed.join(file);
            std::fs::create_dir_all(path.parent().expect("a parent")).expect("dir");
            std::fs::write(path, "").expect("write");
        }
        git(&seed, &["add", "-A"]);
        let identity = ["-c", "user.name=t", "-c", "user.email=t@example.invalid"];
        git(
            &seed,
            &[&identity[..], &["commit", "-q", "-m", "seed"]].concat(),
        );
        let url = remote.to_string_lossy().into_owned();
        git(&seed, &["push", "-q", &url, "main"]);
        names.map(|name| {
            git(dir, &["clone", "-q", &url, name]);
            dir.join(name)
        })
    }

    fn target(repo: &Path) -> RefsTarget<'_> {
        RefsTarget {
            repo,
            remote: "origin",
            timeout: Duration::from_secs(30),
            deadline: None,
        }
    }

    /// `name`'s inbox written from `repo` with `own`, the relay prepared first.
    fn inbox(repo: &Path, name: &JournalName, own: &str) -> Inboxed {
        let target = target(repo);
        let ctx = ctx(&target);
        let base = prepare_relay(&ctx, "origin").expect("ran").expect("a base");
        let to = Inbox {
            remote: "origin",
            base: &base,
        };
        write_inbox(&ctx, to, name, own.as_bytes()).expect("ran")
    }

    #[test]
    fn an_inbox_push_under_a_stale_lease_is_retried_after_a_fetch_and_keeps_both_inboxes() {
        let dir = tempfile::tempdir().expect("tempdir");
        let [first, second] = remote_with_clones(dir.path(), &[], ["first", "second"]);
        let name = JournalName::parse("a1").expect("name");
        assert_eq!(inbox(&first, &name, &line(1)), Inboxed::Pushed(1));
        // The second clone fetches the first inbox, which then moves on before its push.
        let second_target = target(&second);
        let second_ctx = ctx(&second_target);
        let base = prepare_relay(&second_ctx, "origin")
            .expect("ran")
            .expect("a base");
        assert_eq!(inbox(&first, &name, &line(3)), Inboxed::Pushed(1));
        let to = Inbox {
            remote: "origin",
            base: &base,
        };
        let outcome = write_inbox(&second_ctx, to, &name, line(2).as_bytes()).expect("ran");
        assert_eq!(outcome, Inboxed::Pushed(1));
        let remote = dir.path().join("remote.git");
        let file = git(
            &remote,
            &["show", "redkiln/inbox/a1:.redkiln/inbox/a1.jsonl"],
        ) + "\n";
        assert_eq!(file, line(1) + &line(3) + &line(2));
    }

    #[test]
    fn a_default_branch_already_holding_the_inbox_file_defers_the_journal() {
        let dir = tempfile::tempdir().expect("tempdir");
        let [work] = remote_with_clones(dir.path(), &[".redkiln/inbox/a1.jsonl"], ["work"]);
        let name = JournalName::parse("a1").expect("name");
        assert_eq!(inbox(&work, &name, &line(1)), Inboxed::Occupied);
    }

    #[test]
    fn a_local_ref_moves_only_from_the_commit_the_writer_read() {
        let dir = tempfile::tempdir().expect("tempdir");
        let [work] = remote_with_clones(dir.path(), &[], ["work"]);
        let repo = target(&work);
        let repo = Ctx::local(&repo).repo;
        let head = Oid::parse(&git(&work, &["rev-parse", "HEAD"])).expect("oid");
        let tree = Oid::parse(&git(&work, &["rev-parse", "HEAD^{tree}"])).expect("oid");
        let other = repo.commit(&tree, None, "other").expect("commit");
        let name = "refs/redkiln/journal/a1";
        assert!(repo.move_ref(name, &head, None).expect("ran"), "created");
        assert!(
            !repo.move_ref(name, &other, None).expect("ran"),
            "exists already"
        );
        assert!(
            !repo.move_ref(name, &other, Some(&other)).expect("ran"),
            "not at other"
        );
        assert_eq!(repo.commit_of(name).expect("ran"), Some(head.clone()));
        assert!(
            repo.move_ref(name, &other, Some(&head)).expect("ran"),
            "moved"
        );
        assert_eq!(repo.commit_of(name).expect("ran"), Some(other));
    }

    #[test]
    fn a_push_that_is_not_a_fast_forward_is_retried_after_a_fetch_and_keeps_both_writers_lines() {
        let dir = tempfile::tempdir().expect("tempdir");
        let remote = dir.path().join("remote.git");
        std::fs::create_dir_all(&remote).expect("dir");
        git(&remote, &["init", "-q", "--bare"]);
        let url = remote.to_string_lossy().into_owned();
        let [first, second] = ["first", "second"].map(|name| {
            git(dir.path(), &["init", "-q", name]);
            let repo = dir.path().join(name);
            git(&repo, &["remote", "add", "origin", &url]);
            repo
        });
        let name = JournalName::parse("a1").expect("name");
        let target = |repo| RefsTarget {
            repo,
            remote: "origin",
            timeout: Duration::from_secs(30),
            deadline: None,
        };
        let (first_target, second_target) = (target(&first), target(&second));
        assert_eq!(
            merge_direct(&ctx(&first_target), &name, line(1).as_bytes()).expect("ok"),
            Direct::Moved(1)
        );
        // The second clone has not fetched: its push is not a fast-forward.
        assert_eq!(
            merge_direct(&ctx(&second_target), &name, line(2).as_bytes()).expect("ok"),
            Direct::Moved(1)
        );
        let file = git(&remote, &["show", "refs/redkiln/journal/a1:a1.jsonl"]) + "\n";
        assert_eq!(file, line(1) + &line(2));
    }

    /// A repository with no remote, a commit `HEAD` and the journal ref `a1` set by `setup`.
    fn local_with(dir: &Path, setup: &[&[&str]]) -> std::path::PathBuf {
        let [work] = remote_with_clones(dir, &[], ["work"]);
        git(&work, &["remote", "remove", "origin"]);
        for args in setup {
            git(&work, args);
        }
        work
    }

    /// `merge_direct` of `line(9)` into `a1` from `repo`, with no remote, reading at most `cap`
    /// bytes of journal.
    fn direct_with_cap(repo: &Path, cap: u64) -> Direct {
        let target = target(repo);
        let mut ctx = Ctx::local(&target);
        ctx.max_bytes = cap;
        let name = JournalName::parse("a1").expect("name");
        merge_direct(&ctx, &name, line(9).as_bytes()).expect("ran")
    }

    /// The commit of a tree holding `file` as `a1.jsonl`, with `parents`.
    fn journal_commit(repo: &Path, file: &str, parents: &[&str]) -> String {
        let forking = crate::git::forking();
        let blob = Command::new("git")
            .args(["hash-object", "-w", "--stdin"])
            .current_dir(repo)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write as _;
                child
                    .stdin
                    .take()
                    .expect("stdin")
                    .write_all(file.as_bytes())?;
                child.wait_with_output()
            })
            .expect("hash-object");
        let blob = String::from_utf8(blob.stdout).expect("utf-8");
        let tree = Command::new("git")
            .args(["mktree"])
            .current_dir(repo)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write as _;
                let entry = format!("100644 blob {}\ta1.jsonl\n", blob.trim());
                child
                    .stdin
                    .take()
                    .expect("stdin")
                    .write_all(entry.as_bytes())?;
                child.wait_with_output()
            })
            .expect("mktree");
        // Released before `git` takes its own: a second read while a writer waits could block.
        drop(forking);
        let tree = String::from_utf8(tree.stdout).expect("utf-8");
        let mut args = vec![
            "-c",
            "user.name=t",
            "-c",
            "user.email=t@example.invalid",
            "commit-tree",
            tree.trim(),
            "-m",
            "j",
        ];
        for parent in parents {
            args.extend(["-p", parent]);
        }
        git(repo, &args)
    }

    #[test]
    fn a_journal_ref_naming_a_blob_or_an_annotated_tag_breaks_its_tree() {
        let dir = tempfile::tempdir().expect("tempdir");
        let work = local_with(dir.path(), &[]);
        let blob = git(&work, &["rev-parse", "HEAD:README.md"]);
        git(&work, &["update-ref", "refs/redkiln/journal/a1", &blob]);
        assert_eq!(
            direct_with_cap(&work, MAX_JOURNAL_BYTES),
            Direct::Broken(Breach::Tree)
        );
        let commit = journal_commit(&work, &line(1), &[]);
        let tag = git(
            &work,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@example.invalid",
                "tag",
                "-a",
                "-m",
                "t",
                "t1",
                &commit,
            ],
        );
        assert_eq!(tag, "");
        let tag = git(&work, &["rev-parse", "refs/tags/t1"]);
        git(&work, &["update-ref", "refs/redkiln/journal/a1", &tag]);
        assert_eq!(
            direct_with_cap(&work, MAX_JOURNAL_BYTES),
            Direct::Broken(Breach::Tree)
        );
    }

    #[test]
    fn a_journal_ref_with_a_merge_in_its_chain_or_an_unended_file_is_broken() {
        let dir = tempfile::tempdir().expect("tempdir");
        let work = local_with(dir.path(), &[]);
        let first = journal_commit(&work, &line(1), &[]);
        let second = journal_commit(&work, &line(2), &[]);
        let merged = journal_commit(&work, &(line(1) + &line(2)), &[&first, &second]);
        git(&work, &["update-ref", "refs/redkiln/journal/a1", &merged]);
        assert_eq!(
            direct_with_cap(&work, MAX_JOURNAL_BYTES),
            Direct::Broken(Breach::Merge)
        );
        let unended = journal_commit(&work, line(1).trim_end(), &[]);
        git(&work, &["update-ref", "refs/redkiln/journal/a1", &unended]);
        assert_eq!(
            direct_with_cap(&work, MAX_JOURNAL_BYTES),
            Direct::Broken(Breach::Unended)
        );
    }

    #[test]
    fn a_journal_ref_over_the_cap_is_broken_and_a_merge_over_it_is_not_written() {
        let dir = tempfile::tempdir().expect("tempdir");
        let work = local_with(dir.path(), &[]);
        let commit = journal_commit(&work, &line(1), &[]);
        git(&work, &["update-ref", "refs/redkiln/journal/a1", &commit]);
        let one = u64::try_from(line(1).len()).expect("small");
        assert_eq!(
            direct_with_cap(&work, one - 1),
            Direct::Broken(Breach::TooLarge)
        );
        assert_eq!(direct_with_cap(&work, one + 1), Direct::TooLarge);
        assert_eq!(
            git(&work, &["rev-parse", "refs/redkiln/journal/a1"]),
            commit
        );
    }

    #[test]
    fn an_inbox_over_the_cap_or_a_merge_past_it_defers_the_journal() {
        let dir = tempfile::tempdir().expect("tempdir");
        let [work] = remote_with_clones(dir.path(), &[], ["work"]);
        let name = JournalName::parse("a1").expect("name");
        assert_eq!(inbox(&work, &name, &line(1)), Inboxed::Pushed(1));
        let target = target(&work);
        let mut ctx = ctx(&target);
        let base = prepare_relay(&ctx, "origin").expect("ran").expect("a base");
        let to = Inbox {
            remote: "origin",
            base: &base,
        };
        let one = u64::try_from(line(1).len()).expect("small");
        ctx.max_bytes = one - 1;
        let outcome = write_inbox(&ctx, to, &name, line(2).as_bytes()).expect("ran");
        assert_eq!(outcome, Inboxed::InboxTooLarge);
        ctx.max_bytes = one + 1;
        let outcome = write_inbox(&ctx, to, &name, line(2).as_bytes()).expect("ran");
        assert_eq!(outcome, Inboxed::TooLarge);
    }

    #[test]
    fn a_default_branch_holding_a_file_where_the_inbox_folder_goes_defers_the_journal() {
        for file in [".redkiln", ".redkiln/inbox"] {
            let dir = tempfile::tempdir().expect("tempdir");
            let [work] = remote_with_clones(dir.path(), &[file], ["work"]);
            let name = JournalName::parse("a1").expect("name");
            assert_eq!(inbox(&work, &name, &line(1)), Inboxed::Occupied, "{file}");
        }
    }

    #[test]
    fn past_the_deadline_nothing_is_written() {
        let dir = tempfile::tempdir().expect("tempdir");
        git(dir.path(), &["init", "-q"]);
        let target = RefsTarget {
            repo: dir.path(),
            remote: "origin",
            timeout: Duration::from_secs(30),
            deadline: Some(std::time::Instant::now()),
        };
        let name = JournalName::parse("a1").expect("name");
        let outcome = merge_direct(&Ctx::local(&target), &name, line(1).as_bytes()).expect("ok");
        assert_eq!(outcome, Direct::Late);
        assert_eq!(git(dir.path(), &["for-each-ref", "refs/redkiln"]), "");
    }
}
