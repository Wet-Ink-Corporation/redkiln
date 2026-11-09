//! Reading the local journal sources without a store (SPEC JR-05, JR-07): the journal refs, the
//! clone's inbox copies and the refs set aside as broken, as a gate reads them.
//!
//! Nothing is fetched and nothing is written: every git call reads local objects and refs. Each
//! journal ref is checked whole against JR-05, every commit of its first-parent chain, where a
//! writer checks only its tip; inbox copies (replaced by design) and broken refs (already set
//! aside) are read and never checked.

use std::path::Path;
use std::time::Duration;

use super::git::Resolved;
use super::read::{BROKEN_REFS, MAX_LISTING_BYTES, broken_name, file_at};
use super::{
    Breach, Ctx, INBOX_REFS, InboxCopy, JOURNAL_REFS, Oid, RefsError, RefsTarget, journal_file_at,
    read_inbox, unended,
};
use crate::journal::name::JournalName;

/// The most commits read from one journal or broken ref; a longer history is passed over.
pub const MAX_HISTORY_COMMITS: usize = 4096;

/// Which kind of local journal source.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SourceKind {
    /// A journal ref, `refs/redkiln/journal/<name>`: checked whole.
    Journal,
    /// The clone's copy of an inbox branch, `refs/redkiln/inbox/<name>`: its tip only.
    Inbox,
    /// A ref set aside as breaking JR-05, `refs/redkiln/broken/<name>/<sha>`: every commit.
    Broken,
}

/// How one commit of a journal ref breaks JR-05.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flaw {
    /// What a writer finds at a tip, found at this commit ([`Breach::Merge`] at each merge
    /// commit reachable from the tip).
    Breach(Breach),
    /// This commit's file does not start with the previous readable commit's file.
    Rewritten,
}

/// One flaw at one commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainBreak {
    /// The commit at fault, its full object id.
    pub commit: String,
    /// How it breaks JR-05.
    pub flaw: Flaw,
}

/// One local source read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalSource {
    /// The full ref name (`refs/redkiln/journal/s1`, `refs/redkiln/inbox/s1`,
    /// `refs/redkiln/broken/s1/<sha>`).
    pub refname: String,
    /// Which kind of source it is.
    pub kind: SourceKind,
    /// The tip's full object id.
    pub tip: String,
    /// Commits read: the first-parent chain of a journal ref (0 when it names no commit), every
    /// commit reachable from a broken ref, 1 for an inbox.
    pub commits: usize,
    /// JR-05 flaws, merges first then the chain's, each oldest first; journal refs only (empty
    /// for inbox and broken refs).
    pub breaks: Vec<ChainBreak>,
}

/// What [`read_local`] found.
#[derive(Debug, Default)]
#[must_use]
pub struct LocalJournals {
    /// Each source read: journal refs, then inboxes, then broken refs, each by ref name.
    pub sources: Vec<LocalSource>,
    /// Refs not read: a name that is no journal's, a broken ref naming no commit, a history
    /// longer than [`MAX_HISTORY_COMMITS`], an inbox file past the cap. A prefix whose listing
    /// is past the cap is named itself, as [`super::import_refs`] names it.
    pub passed_over: Vec<String>,
}

/// One journal file read, handed to the caller as it is read, so Core keeps at most two files.
#[derive(Debug, Clone, Copy)]
pub struct LocalFile<'a> {
    /// The ref it was read from.
    pub refname: &'a str,
    /// Which kind of source that ref is.
    pub kind: SourceKind,
    /// The commit it was read at, its full object id.
    pub commit: &'a str,
    /// The file's bytes.
    pub bytes: &'a [u8],
}

/// Reads every local journal source of the repository at `repo`, never fetching and writing
/// nothing; `file` is called for each file whose lines a reader folds.
///
/// Of a journal or broken ref, the files handed over are each one a later commit's file does
/// not start with, then the last readable one (the tip's, when it is readable); of an inbox,
/// its tip's file when it is not empty. Each git call is bounded by `timeout` with its output
/// capped, and a run makes at most `refs × (3 × MAX_HISTORY_COMMITS + 6) + 3` calls.
///
/// # Errors
/// [`RefsError::Git`] when git cannot run or fails at a local step, [`RefsError::Answer`] when
/// it prints something unreadable.
pub fn read_local(
    repo: &Path,
    timeout: Duration,
    file: impl FnMut(LocalFile<'_>),
) -> Result<LocalJournals, RefsError> {
    let target = RefsTarget {
        repo,
        remote: "",
        timeout,
        deadline: None,
    };
    // `Ctx::local` names no remote, so no step that talks to one can run.
    read_local_with(&Ctx::local(&target), MAX_HISTORY_COMMITS, file)
}

/// [`read_local`] through `ctx`, reading at most `cap` commits of one ref.
pub(crate) fn read_local_with<F: FnMut(LocalFile<'_>)>(
    ctx: &Ctx<'_>,
    cap: usize,
    file: F,
) -> Result<LocalJournals, RefsError> {
    let mut reader = Reader {
        ctx,
        cap,
        file,
        found: LocalJournals::default(),
    };
    for (refname, listed) in reader.listed(JOURNAL_REFS)? {
        reader.journal(refname, listed)?;
    }
    for (refname, _) in reader.listed(INBOX_REFS)? {
        reader.inbox(refname)?;
    }
    for (refname, _) in reader.listed(BROKEN_REFS)? {
        reader.broken(refname)?;
    }
    Ok(reader.found)
}

/// One walk over the local sources.
struct Reader<'c, 'a, F> {
    ctx: &'c Ctx<'a>,
    cap: usize,
    file: F,
    found: LocalJournals,
}

impl<F: FnMut(LocalFile<'_>)> Reader<'_, '_, F> {
    /// The local refs under `prefix`; none, and the prefix passed over, past the listing cap.
    fn listed(&mut self, prefix: &str) -> Result<Vec<(String, Oid)>, RefsError> {
        let listed = self.ctx.repo.list_refs(prefix, MAX_LISTING_BYTES)?;
        Ok(listed.unwrap_or_else(|| {
            self.found.passed_over.push(prefix.to_owned());
            Vec::new()
        }))
    }

    /// The journal name of `refname` under `prefix`, or `None`, passed over.
    fn named(&mut self, refname: &str, prefix: &str) -> Option<JournalName> {
        let name = refname
            .strip_prefix(prefix)
            .and_then(|name| JournalName::parse(name).ok());
        if name.is_none() {
            self.found.passed_over.push(refname.to_owned());
        }
        name
    }

    /// `rev-list <flags> --max-count=<cap + 1> <tip>`, or `None` past the cap.
    fn commits(&self, tip: &Oid, flags: &[&str]) -> Result<Option<Vec<Oid>>, RefsError> {
        let max = format!("--max-count={}", self.cap.saturating_add(1));
        let mut all = flags.to_vec();
        all.push(&max);
        let listed = self.ctx.repo.commits(tip, &all, MAX_LISTING_BYTES)?;
        Ok(listed.filter(|commits| commits.len() <= self.cap))
    }

    /// A journal ref, checked at every commit of its first-parent chain.
    fn journal(&mut self, refname: String, listed: Oid) -> Result<(), RefsError> {
        let Some(name) = self.named(&refname, JOURNAL_REFS) else {
            return Ok(());
        };
        let tip = match self.ctx.repo.resolve(&refname)? {
            // Deleted since the listing.
            Resolved::Absent => return Ok(()),
            Resolved::NotACommit => {
                let breaks = vec![ChainBreak {
                    commit: listed.as_str().to_owned(),
                    flaw: Flaw::Breach(Breach::Tree),
                }];
                self.found.sources.push(LocalSource {
                    refname,
                    kind: SourceKind::Journal,
                    tip: listed.into_string(),
                    commits: 0,
                    breaks,
                });
                return Ok(());
            }
            Resolved::Commit(tip) => tip,
        };
        let chain = self.commits(&tip, &["--first-parent", "--reverse"])?;
        let merges = self.commits(&tip, &["--min-parents=2", "--reverse"])?;
        let (Some(chain), Some(merges)) = (chain, merges) else {
            self.found.passed_over.push(refname);
            return Ok(());
        };
        let mut source = LocalSource {
            refname,
            kind: SourceKind::Journal,
            tip: tip.into_string(),
            commits: chain.len(),
            breaks: merges
                .into_iter()
                .map(|commit| ChainBreak {
                    commit: commit.into_string(),
                    flaw: Flaw::Breach(Breach::Merge),
                })
                .collect(),
        };
        let flaws = self.walk(&source, &chain, |ctx, commit| {
            journal_file_at(ctx, &name, commit)
        })?;
        source.breaks.extend(flaws);
        self.found.sources.push(source);
        Ok(())
    }

    /// The clone's copy of an inbox branch: its tip's file, never checked.
    fn inbox(&mut self, refname: String) -> Result<(), RefsError> {
        let Some(name) = self.named(&refname, INBOX_REFS) else {
            return Ok(());
        };
        match read_inbox(self.ctx, &name)? {
            InboxCopy::Absent => {}
            InboxCopy::TooLarge => self.found.passed_over.push(refname),
            InboxCopy::Read { tip, file } => {
                if !file.is_empty() {
                    (self.file)(LocalFile {
                        refname: &refname,
                        kind: SourceKind::Inbox,
                        commit: tip.as_str(),
                        bytes: &file,
                    });
                }
                self.found.sources.push(LocalSource {
                    refname,
                    kind: SourceKind::Inbox,
                    tip: tip.into_string(),
                    commits: 1,
                    breaks: Vec::new(),
                });
            }
        }
        Ok(())
    }

    /// A broken ref: every commit reachable from it, as an import reads it, never checked.
    fn broken(&mut self, refname: String) -> Result<(), RefsError> {
        let Some(name) = broken_name(&refname) else {
            self.found.passed_over.push(refname);
            return Ok(());
        };
        let Some(tip) = self.ctx.repo.commit_of(&refname)? else {
            self.found.passed_over.push(refname);
            return Ok(());
        };
        let Some(history) = self.commits(&tip, &["--reverse"])? else {
            self.found.passed_over.push(refname);
            return Ok(());
        };
        let source = LocalSource {
            refname,
            kind: SourceKind::Broken,
            tip: tip.into_string(),
            commits: history.len(),
            breaks: Vec::new(),
        };
        let path = name.file_name();
        // A broken tree may hold other files: only the journal's own is read, and a commit
        // without a readable one is skipped. The ref is set aside, so its flaws are not kept.
        self.walk(&source, &history, |ctx, commit| {
            Ok(file_at(ctx, commit, &path)?.ok_or(Breach::Tree))
        })?;
        self.found.sources.push(source);
        Ok(())
    }

    /// Reads `source`'s file at each of `commits` with `read`, handing over each file a later
    /// one does not start with, then the last readable one; the chain's flaws, oldest first.
    fn walk(
        &mut self,
        source: &LocalSource,
        commits: &[Oid],
        read: impl Fn(&Ctx<'_>, &Oid) -> Result<Result<Vec<u8>, Breach>, RefsError>,
    ) -> Result<Vec<ChainBreak>, RefsError> {
        let (refname, kind) = (source.refname.as_str(), source.kind);
        let mut breaks = Vec::new();
        let flawed = |commit: &Oid, flaw| ChainBreak {
            commit: commit.as_str().to_owned(),
            flaw,
        };
        let mut last: Option<(&Oid, Vec<u8>)> = None;
        for commit in commits {
            let bytes = match read(self.ctx, commit)? {
                Ok(bytes) => bytes,
                Err(breach) => {
                    breaks.push(flawed(commit, Flaw::Breach(breach)));
                    continue;
                }
            };
            if unended(&bytes) {
                breaks.push(flawed(commit, Flaw::Breach(Breach::Unended)));
            }
            if let Some((previous, kept)) = &last
                && !bytes.starts_with(kept)
            {
                breaks.push(flawed(commit, Flaw::Rewritten));
                (self.file)(LocalFile {
                    refname,
                    kind,
                    commit: previous.as_str(),
                    bytes: kept,
                });
            }
            last = Some((commit, bytes));
        }
        if let Some((commit, bytes)) = &last {
            (self.file)(LocalFile {
                refname,
                kind,
                commit: commit.as_str(),
                bytes,
            });
        }
        Ok(breaks)
    }
}

#[cfg(test)]
#[cfg(unix)]
mod tests;
