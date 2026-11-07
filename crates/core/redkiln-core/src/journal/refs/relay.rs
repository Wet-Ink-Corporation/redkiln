//! The relay (SPEC JR-07): what a repository's relay workflow runs for each push to an inbox
//! branch. It reads the branch's tip, refuses any commit other than one adding exactly
//! `.redkiln/inbox/<name>.jsonl` to its single parent, merges that file into the journal ref,
//! and deletes the branch, with a lease on the commit it processed, only once the merge was
//! pushed or added nothing.
//!
//! It keeps honest writers honest; anyone with write access can change the workflow itself.

use super::direct::{Direct, merge_direct, refetch};
use super::git::{Fetched, Pushed};
use super::{
    Breach, Ctx, INBOX_BRANCHES, Oid, RefsError, RefsTarget, inbox_branch, inbox_path, inbox_ref,
    journal_ref,
};
use crate::journal::export::MAX_JOURNAL_BYTES;
use crate::journal::name::JournalName;

/// What a relay run did.
#[derive(Debug, Clone, PartialEq, Eq)]
#[must_use]
pub enum Relayed {
    /// The inbox branch is gone: an earlier run relayed it.
    Gone,
    /// The inbox commit is not one the relay moves; the branch is left for a person.
    Refused(Refusal),
    /// The journal ref breaks JR-05; the branch is left in place.
    Broken(Breach),
    /// Every retry of a rejected push was rejected; the branch is left in place.
    RetriesSpent,
    /// A fetch or push failed, with what git said; the branch is left in place.
    Failed(String),
    /// The inbox was merged into the ref, which gained `added` lines (`0`: it held them all).
    Merged {
        /// Lines the ref gained.
        added: u64,
        /// What became of the inbox branch.
        deleted: Deleted,
    },
}

/// What became of an inbox branch after its file was merged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Deleted {
    /// It was deleted.
    Yes,
    /// A newer inbox commit arrived meanwhile, so the lease refused the delete; the run that
    /// commit's push started relays it.
    NewerArrived,
    /// The delete failed for another reason, with what git said.
    Failed(String),
}

/// Why an inbox commit is refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// It has this many parents, not one.
    Parents(usize),
    /// It changes something other than adding the one inbox file: what git listed.
    Changes(String),
    /// Its file is larger than [`MAX_JOURNAL_BYTES`].
    TooLarge,
}

/// Relays the inbox branch `branch` (`redkiln/inbox/<name>`, or with `refs/heads/` before it) of
/// the target's remote into its journal ref.
///
/// # Errors
/// [`RefsError::NotInbox`] when `branch` names no inbox, and [`RefsError`] when git cannot run
/// or fails at a local step.
pub fn relay(target: &RefsTarget<'_>, branch: &str) -> Result<Relayed, RefsError> {
    let name = inbox_name(branch)?;
    let mut ctx = Ctx::local(target);
    let remote = target.remote;
    ctx.remote = Some(remote);
    let full = inbox_branch(&name);
    let local = inbox_ref(&name);
    match ctx
        .repo
        .fetch(remote, &[format!("+{full}:{local}")], false)?
    {
        Fetched::Done => {}
        Fetched::Missing => return Ok(Relayed::Gone),
        Fetched::Failed(said) => return Ok(Relayed::Failed(said)),
    }
    let Some(tip) = ctx.repo.commit_of(&local)? else {
        return Ok(Relayed::Gone);
    };
    let file = match inbox_file(&ctx, &name, &tip)? {
        Ok(file) => file,
        Err(refusal) => return Ok(Relayed::Refused(refusal)),
    };
    merged(&ctx, remote, &name, (&tip, &file))
}

/// Merges the inbox `file` read at `tip` into `name`'s journal ref, then deletes the inbox
/// branch with a lease on `tip`.
fn merged(
    ctx: &Ctx<'_>,
    remote: &str,
    name: &JournalName,
    (tip, file): (&Oid, &[u8]),
) -> Result<Relayed, RefsError> {
    let refname = journal_ref(name);
    if let Some(said) = refetch(ctx, remote, &refname, &refname)? {
        return Ok(Relayed::Failed(said));
    }
    let full = inbox_branch(name);
    let added = match merge_direct(ctx, name, file)? {
        Direct::UpToDate => 0,
        Direct::Moved(added) => added,
        Direct::Broken(breach) => return Ok(Relayed::Broken(breach)),
        Direct::Spent => return Ok(Relayed::RetriesSpent),
        Direct::Failed(said) => return Ok(Relayed::Failed(said)),
        Direct::Late => return Ok(Relayed::Failed("out of time".to_owned())),
    };
    let lease = format!("{full}:{tip}");
    let deleted = match ctx.repo.push(remote, &format!(":{full}"), Some(&lease))? {
        Pushed::Done => Deleted::Yes,
        Pushed::Rejected => Deleted::NewerArrived,
        Pushed::Failed(said) => Deleted::Failed(said),
    };
    Ok(Relayed::Merged { added, deleted })
}

/// The journal name of an inbox branch.
fn inbox_name(branch: &str) -> Result<JournalName, RefsError> {
    let short = INBOX_BRANCHES.trim_start_matches("refs/heads/");
    let bare = branch.strip_prefix("refs/heads/").unwrap_or(branch);
    bare.strip_prefix(short)
        .and_then(|name| JournalName::parse(name).ok())
        .ok_or_else(|| RefsError::NotInbox(branch.to_owned()))
}

/// The inbox file at `tip`, when `tip` only adds it to its single parent.
fn inbox_file(
    ctx: &Ctx<'_>,
    name: &JournalName,
    tip: &Oid,
) -> Result<Result<Vec<u8>, Refusal>, RefsError> {
    let parents = ctx.repo.parents(tip)?;
    let [parent] = parents.as_slice() else {
        return Ok(Err(Refusal::Parents(parents.len())));
    };
    let changes = match ctx.repo.changes(parent, tip) {
        Ok(changes) => changes,
        // A diff too large to list cannot be the one added file.
        Err(RefsError::Git {
            source: crate::git::GitError::TooMuchOutput,
            ..
        }) => {
            return Ok(Err(Refusal::Changes("more than git lists".to_owned())));
        }
        Err(e) => return Err(e),
    };
    let Some(blob) = only_added(&changes, &inbox_path(name)) else {
        return Ok(Err(Refusal::Changes(
            String::from_utf8_lossy(&changes).replace('\0', " "),
        )));
    };
    Ok(ctx
        .repo
        .blob(&blob, MAX_JOURNAL_BYTES)?
        .ok_or(Refusal::TooLarge))
}

/// The blob of `path` when `changes` (`diff-tree --raw -z`) is exactly that path added as a
/// regular file.
fn only_added(changes: &[u8], path: &str) -> Option<Oid> {
    let text = std::str::from_utf8(changes).ok()?;
    let records: Vec<&str> = text.split('\0').collect();
    let [meta, changed_path, ""] = records.as_slice() else {
        return None;
    };
    let fields: Vec<&str> = meta.split(' ').collect();
    let [old_mode, new_mode, _old, new, status] = fields.as_slice() else {
        return None;
    };
    let added = *old_mode == ":000000" && *new_mode == "100644" && *status == "A";
    if added && *changed_path == path {
        Oid::parse(new).ok()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_inbox_branch_names_its_journal() {
        for branch in ["redkiln/inbox/abc_1", "refs/heads/redkiln/inbox/abc_1"] {
            assert_eq!(inbox_name(branch).expect(branch).as_str(), "abc_1");
        }
        for branch in [
            "main",
            "redkiln/inbox/",
            "redkiln/inbox/a/b",
            "redkiln/inbox/A",
        ] {
            assert!(
                matches!(inbox_name(branch), Err(RefsError::NotInbox(b)) if b == branch),
                "{branch}"
            );
        }
    }

    #[test]
    fn only_one_added_regular_file_at_the_inbox_path_is_accepted() {
        let blob = "b".repeat(40);
        let zero = "0".repeat(40);
        let path = ".redkiln/inbox/x.jsonl";
        let record = |old: &str, new: &str, status: &str, at: &str| {
            format!(":{old} {new} {zero} {blob} {status}\0{at}\0")
        };
        let good = record("000000", "100644", "A", path);
        assert_eq!(
            only_added(good.as_bytes(), path),
            Some(Oid::parse(&blob).expect("oid"))
        );
        let refused = [
            record("000000", "100755", "A", path),
            record("100644", "100644", "M", path),
            record("000000", "120000", "A", path),
            record("000000", "100644", "A", ".redkiln/inbox/y.jsonl"),
            format!("{good}{}", record("000000", "100644", "A", "README.md")),
            String::new(),
        ];
        for changes in refused {
            assert_eq!(only_added(changes.as_bytes(), path), None, "{changes:?}");
        }
    }
}
