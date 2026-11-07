//! Importing journals from git (SPEC JR-05, JR-07, JR-10 to JR-13): the journal refs, refs set
//! aside under `refs/redkiln/broken/`, and inbox branches not yet relayed.
//!
//! Each source is fetched with an explicit refspec (a plain clone fetches none of them), then
//! read from the clone's copy and imported through [`crate::journal::import`], which is
//! idempotent by identity (JR-11). A source already imported at its commit, by the
//! `core.import_completed` records, is not read again. A journal ref whose commit does not
//! descend from the one last imported was rewritten (JR-05): it is reported, and its new lines
//! are still imported.

use std::collections::HashMap;
use std::io::Cursor;
use std::num::NonZeroUsize;

use super::git::Fetched;
use super::{
    Ctx, INBOX_BRANCHES, INBOX_REFS, JOURNAL_REFS, Oid, REGULAR, RefsError, RefsTarget, inbox_path,
};
use crate::envelope::{Author, Payload, Timestamp, decode_all};
use crate::journal::import::{ImportCompleted, ImportReport, ImportSource, Recording, import};
use crate::journal::name::JournalName;
use crate::store::{Filter, Store};

/// Where refs set aside as breaking JR-05 live: `refs/redkiln/broken/<name>/<sha>`.
pub const BROKEN_REFS: &str = "refs/redkiln/broken/";

/// The most bytes of ref or commit listing read at once (H-13).
const MAX_LISTING_BYTES: u64 = 16 * 1024 * 1024;

/// Records read from the store per page.
const PAGE: NonZeroUsize = match NonZeroUsize::new(512) {
    Some(page) => page,
    None => NonZeroUsize::MIN,
};

/// What an import from git did.
#[derive(Debug, Default)]
#[must_use]
pub struct RefsImport {
    /// Each source read, in the order read.
    pub sources: Vec<ImportedRef>,
    /// Journal refs whose commit does not descend from the one last imported (JR-05).
    pub rewritten: Vec<String>,
    /// Sources passed over: a name that is no journal's, a listing longer than this reader
    /// takes, or a journal file that is missing, not a regular file or larger than
    /// [`crate::journal::MAX_JOURNAL_BYTES`] (for a broken ref, at its tip).
    pub passed_over: Vec<String>,
    /// What git said when a fetch failed; the clone's copies were read anyway.
    pub fetch_failed: Vec<String>,
    /// The target's deadline passed, so some sources (and perhaps the fetch) were left for a
    /// later run.
    pub late: bool,
}

/// One source imported at one commit.
#[derive(Debug)]
pub struct ImportedRef {
    /// The ref read: a journal ref, a broken ref, or an inbox branch as the remote names it.
    pub source: String,
    /// The commit read.
    pub commit: String,
    /// What its import did.
    pub report: ImportReport,
}

/// Imports every journal source not already imported at its commit into `store` (JR-10).
///
/// Sources are fetched from the target's remote first, when one is configured; a fetch that
/// fails is reported and the clone's own copies are read. Each import is recorded as written
/// at `ts` by `author` (JR-13). After the target's deadline, nothing more is fetched or read.
///
/// # Errors
/// [`RefsError::Store`] when the store cannot be read, [`RefsError::Import`] when an import
/// stops (those recorded before it stay), and [`RefsError`] when git cannot run or fails at a
/// local step.
pub fn import_refs(
    store: &Store,
    target: &RefsTarget<'_>,
    ts: &Timestamp,
    author: &Author,
) -> Result<RefsImport, RefsError> {
    let ctx = Ctx::local(target);
    let mut report = RefsImport::default();
    if ctx.late() {
        report.late = true;
        return Ok(report);
    }
    if ctx.repo.has_remote(target.remote)? {
        fetch_all(&ctx, target.remote, &mut report)?;
    }
    let last = last_imported(store)?;
    let importer = Importer {
        ctx: &ctx,
        store,
        ts,
        author,
    };
    for (refname, tip) in listed(&ctx, JOURNAL_REFS, &mut report)? {
        let Some(name) = named(&refname, JOURNAL_REFS, &mut report) else {
            continue;
        };
        if let Some(previous) = last.get(&refname) {
            if *previous == tip {
                continue;
            }
            if ctx.repo.descends(previous, &tip)? != Some(true) {
                report.rewritten.push(refname.clone());
            }
        }
        importer.source(&mut report, (refname, tip), &name.file_name())?;
    }
    import_broken(&ctx, &last, &mut report, &importer)?;
    for (local, tip) in listed(&ctx, INBOX_REFS, &mut report)? {
        let Some(name) = named(&local, INBOX_REFS, &mut report) else {
            continue;
        };
        let source = format!("{INBOX_BRANCHES}{name}");
        if last.get(&source) != Some(&tip) {
            importer.source(&mut report, (source, tip), &inbox_path(&name))?;
        }
    }
    Ok(report)
}

/// Imports sources from one clone into one store, recording each import (JR-13).
struct Importer<'a> {
    ctx: &'a Ctx<'a>,
    store: &'a Store,
    ts: &'a Timestamp,
    author: &'a Author,
}

impl Importer<'_> {
    /// Imports the file `path` at `commit` as `source`, or reports the source passed over when
    /// there is none there or it is too large.
    fn source(
        &self,
        report: &mut RefsImport,
        (source, commit): (String, Oid),
        path: &str,
    ) -> Result<(), RefsError> {
        if self.late(report) {
            return Ok(());
        }
        match file_at(self.ctx, &commit, path)? {
            Some(file) => report.sources.push(self.one(source, &commit, file)?),
            None => report.passed_over.push(source),
        }
        Ok(())
    }

    /// Whether the deadline has passed, noted in `report`: no more sources are started.
    fn late(&self, report: &mut RefsImport) -> bool {
        report.late |= self.ctx.late();
        report.late
    }

    /// Imports `file`, read from `source` at `commit`.
    fn one(&self, source: String, commit: &Oid, file: Vec<u8>) -> Result<ImportedRef, RefsError> {
        // The store's record and the caller's report each keep the source and the commit.
        let recording = Recording {
            source: ImportSource::Ref {
                name: source.clone(),
                commit: commit.to_string(),
            },
            ts: self.ts.clone(),
            author: self.author.clone(),
        };
        let report = import(self.store, Cursor::new(file), recording).map_err(RefsError::Import)?;
        Ok(ImportedRef {
            source,
            commit: commit.to_string(),
            report,
        })
    }
}

/// The refs set aside under [`BROKEN_REFS`]: `<name>.jsonl` at each commit reachable from each,
/// oldest first, unless the ref's tip was the last commit imported from it.
fn import_broken(
    ctx: &Ctx<'_>,
    last: &HashMap<String, Oid>,
    report: &mut RefsImport,
    importer: &Importer<'_>,
) -> Result<(), RefsError> {
    for (refname, tip) in listed(ctx, BROKEN_REFS, report)? {
        if importer.late(report) {
            return Ok(());
        }
        let name = refname
            .strip_prefix(BROKEN_REFS)
            .and_then(|rest| rest.split_once('/'))
            .and_then(|(name, _)| JournalName::parse(name).ok());
        let Some(name) = name else {
            report.passed_over.push(refname);
            continue;
        };
        // Only the commits after the last one this store imported from the ref, so a tip with
        // no readable journal file is not read, and recorded, again on every run.
        let since = match last.get(&refname) {
            Some(previous) if *previous == tip => continue,
            Some(previous) if ctx.repo.descends(previous, &tip)? == Some(true) => Some(previous),
            _ => None,
        };
        let Some(history) = ctx.repo.history(&tip, since, MAX_LISTING_BYTES)? else {
            report.passed_over.push(refname);
            continue;
        };
        let mut tip_read = false;
        for commit in history {
            // A broken tree may hold other files: only the journal's own is read, and a commit
            // without a readable one is passed over, since the ref as a whole was set aside.
            if let Some(file) = file_at(ctx, &commit, &name.file_name())? {
                tip_read |= commit == tip;
                // Each commit's record names the ref it was read from.
                report
                    .sources
                    .push(importer.one(refname.clone(), &commit, file)?);
            }
        }
        if !tip_read {
            report.passed_over.push(refname);
        }
    }
    Ok(())
}

/// Fetches the journal and broken refs in one fetch, and the inbox branches, pruned, in a fetch
/// of their own, so the prune never touches the other refs (JR-07).
fn fetch_all(ctx: &Ctx<'_>, remote: &str, report: &mut RefsImport) -> Result<(), RefsError> {
    let refs = [JOURNAL_REFS, BROKEN_REFS].map(|prefix| format!("+{prefix}*:{prefix}*"));
    if let Fetched::Failed(said) = ctx.repo.fetch(remote, &refs, false)? {
        report.fetch_failed.push(said);
    }
    let inboxes = [format!("+{INBOX_BRANCHES}*:{INBOX_REFS}*")];
    if let Fetched::Failed(said) = ctx.repo.fetch(remote, &inboxes, true)? {
        report.fetch_failed.push(said);
    }
    Ok(())
}

/// The local refs under `prefix`; none, and the prefix reported, when the listing is too long.
fn listed(
    ctx: &Ctx<'_>,
    prefix: &str,
    report: &mut RefsImport,
) -> Result<Vec<(String, Oid)>, RefsError> {
    Ok(ctx
        .repo
        .list_refs(prefix, MAX_LISTING_BYTES)?
        .unwrap_or_else(|| {
            report.passed_over.push(prefix.to_owned());
            Vec::new()
        }))
}

/// The journal name of `refname` under `prefix`, or `None`, reported.
fn named(refname: &str, prefix: &str, report: &mut RefsImport) -> Option<JournalName> {
    let name = refname
        .strip_prefix(prefix)
        .and_then(|name| JournalName::parse(name).ok());
    if name.is_none() {
        report.passed_over.push(refname.to_owned());
    }
    name
}

/// The file `path` at `commit`, or `None` when there is no such blob or it is larger than the
/// context's cap.
fn file_at(ctx: &Ctx<'_>, commit: &Oid, path: &str) -> Result<Option<Vec<u8>>, RefsError> {
    match ctx.repo.entries(commit, Some(path))?.as_deref() {
        Some([entry]) if entry.mode == REGULAR && entry.kind == "blob" && entry.path == path => {
            ctx.repo.blob(&entry.oid, ctx.max_bytes)
        }
        _ => Ok(None),
    }
}

/// The commit last imported from each ref, by the `core.import_completed` records this store
/// wrote itself.
fn last_imported(store: &Store) -> Result<HashMap<String, Oid>, RefsError> {
    let filter = Filter::all()
        .of_type(ImportCompleted::TYPE)
        .map_err(RefsError::Store)?;
    let mine = store.store_id();
    let mut last = HashMap::new();
    let mut after = 0;
    loop {
        let page = store.read(&filter, after, PAGE).map_err(RefsError::Store)?;
        let Some(end) = page.last().map(|stored| stored.position) else {
            break;
        };
        // Only this store's own records: those ingested from another clone's journal name what
        // that store read, not what this one holds. A record that does not decode is passed
        // over; its ref is then read again, which the idempotent import makes harmless.
        let own = page.iter().filter(|stored| stored.id.store() == mine);
        for envelope in decode_all::<ImportCompleted>(own).events {
            // A record whose commit is not an object name was not written by this reader.
            if let ImportSource::Ref { name, commit } = envelope.data.source
                && let Ok(commit) = Oid::parse(&commit)
            {
                last.insert(name, commit);
            }
        }
        if page.len() < PAGE.get() {
            break;
        }
        after = end;
    }
    Ok(last)
}
