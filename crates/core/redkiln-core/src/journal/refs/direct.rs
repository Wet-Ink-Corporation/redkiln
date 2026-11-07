//! The two ways a journal is written (SPEC JR-07): straight into its ref, or into its inbox
//! branch for the relay. Each merges by `origin` and retries a rejected push at most
//! [`RETRIES`] times, fetching first.

use super::git::{Fetched, Pushed};
use super::{
    BASE_REF, Breach, Ctx, Oid, RETRIES, RefsError, State, inbox_branch, inbox_path, inbox_ref,
    journal_ref, read_inbox, read_journal,
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
        let (parent, current) = match read_journal(ctx.repo, name)? {
            State::Absent => (None, Vec::new()),
            State::Good { tip, file } => (Some(tip), file),
            State::Broken(breach) => return Ok(Direct::Broken(breach)),
        };
        let merged = merge(&current, incoming);
        if merged.added == 0 {
            return Ok(Direct::UpToDate);
        }
        let blob = ctx.repo.hash_blob(&merged.bytes)?;
        let tree = ctx.repo.single_file_tree(&name.file_name(), &blob)?;
        let message = format!("redkiln journal {name} [skip ci]");
        let commit = ctx.repo.commit(&tree, parent.as_ref(), &message)?;
        let Some(remote) = ctx.remote else {
            ctx.repo.set_ref(&refname, &commit)?;
            return Ok(Direct::Moved(merged.added));
        };
        match ctx
            .repo
            .push(remote, &format!("{commit}:{refname}"), None)?
        {
            Pushed::Done => {
                ctx.repo.set_ref(&refname, &commit)?;
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

/// How an inbox write ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Inboxed {
    /// The inbox would not change, so nothing was pushed.
    Same,
    /// The inbox branch was pushed; this many of the checkpoint's lines were new to it.
    Pushed(u64),
    /// The default branch already holds the inbox's path, so no commit can only add it.
    Occupied,
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
    if !ctx.repo.entries(base, Some(&path))?.is_empty() {
        return Ok(Inboxed::Occupied);
    }
    let branch = inbox_branch(name);
    let local = inbox_ref(name);
    for attempt in 0..=RETRIES {
        if ctx.late() {
            return Ok(Inboxed::Late);
        }
        let Some((expected, merged)) = inbox_content(ctx, name, own)? else {
            return Ok(Inboxed::Same);
        };
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
                ctx.repo.set_ref(&local, &commit)?;
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

/// The inbox tip read (`None`: no inbox) and the file the inbox should hold: the merge of the
/// journal ref's file, the inbox's and `own`; `None` when the inbox would not change.
fn inbox_content(
    ctx: &Ctx<'_>,
    name: &JournalName,
    own: &[u8],
) -> Result<Option<(Option<Oid>, Merged)>, RefsError> {
    // A ref breaking JR-05 counts as empty here; the inbox is still pushed (JR-07).
    let journal = match read_journal(ctx.repo, name)? {
        State::Good { file, .. } => file,
        State::Absent | State::Broken(_) => Vec::new(),
    };
    if let Some((tip, file)) = read_inbox(ctx.repo, name)? {
        let merged = merge(&merge(&journal, &file).bytes, own);
        return Ok((merged.bytes != file).then_some((Some(tip), merged)));
    }
    let merged = merge(&journal, own);
    Ok((merged.added > 0).then_some((None, merged)))
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
    match ctx.repo.resolve(BASE_REF)? {
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
    use crate::journal::refs::RefsTarget;

    fn git(dir: &Path, args: &[&str]) -> String {
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

    #[test]
    fn a_push_rejected_as_stale_is_retried_after_a_fetch_and_keeps_both_writers_lines() {
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
