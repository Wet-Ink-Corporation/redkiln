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

/// The most git calls an import makes after its target's deadline passes.
///
/// No fetch, listing, ancestry check, source or store write starts after it, and no run of
/// calls between two checks of it is longer than this (checking for the remote and the first
/// fetch; listing one file at a commit and reading it). Each call is bounded by the target's
/// `timeout`, so an import ends by `deadline + MAX_IMPORT_CALLS_PAST_DEADLINE × timeout`, or by
/// the deadline plus the store's wait for one write begun before it, plus its store reads.
pub const MAX_IMPORT_CALLS_PAST_DEADLINE: u32 = 2;

/// Whether `ctx`'s deadline has passed, noted in `report`: nothing more is started.
fn late(ctx: &Ctx<'_>, report: &mut RefsImport) -> bool {
    report.late |= ctx.late();
    report.late
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
    let importer = Importer {
        ctx: &ctx,
        store,
        ts,
        author,
    };
    import_through(&importer, target.remote)
}

/// [`import_refs`] through `importer`, fetching from `remote` when it is configured.
fn import_through(importer: &Importer<'_>, remote: &str) -> Result<RefsImport, RefsError> {
    let ctx = importer.ctx;
    let mut report = RefsImport::default();
    if late(ctx, &mut report) {
        return Ok(report);
    }
    if ctx.repo.has_remote(remote)? {
        fetch_all(ctx, remote, &mut report)?;
    }
    let last = last_imported(importer.store)?;
    if late(ctx, &mut report) {
        return Ok(report);
    }
    for (refname, tip) in listed(ctx, JOURNAL_REFS, &mut report)? {
        let Some(name) = named(&refname, JOURNAL_REFS, &mut report) else {
            continue;
        };
        if let Some(previous) = last.get(&refname) {
            if *previous == tip {
                continue;
            }
            if late(ctx, &mut report) {
                return Ok(report);
            }
            if ctx.repo.descends(previous, &tip)? != Some(true) {
                report.rewritten.push(refname.clone());
            }
        }
        importer.source(&mut report, (refname, tip), &name.file_name())?;
    }
    if late(ctx, &mut report) {
        return Ok(report);
    }
    import_broken(ctx, &last, &mut report, importer)?;
    if late(ctx, &mut report) {
        return Ok(report);
    }
    for (local, tip) in listed(ctx, INBOX_REFS, &mut report)? {
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
            Some(file) => {
                self.record(report, (source, &commit), file)?;
            }
            None => report.passed_over.push(source),
        }
        Ok(())
    }

    /// Imports `file`, read from `source` at `commit`, into `report`, unless the deadline passed
    /// while it was read: no store write starts after it, and a later run reads it again.
    /// Whether it was imported.
    fn record(
        &self,
        report: &mut RefsImport,
        (source, commit): (String, &Oid),
        file: Vec<u8>,
    ) -> Result<bool, RefsError> {
        if self.late(report) {
            return Ok(false);
        }
        report.sources.push(self.one(source, commit, file)?);
        Ok(true)
    }

    /// Whether the deadline has passed, noted in `report`: no more sources are started.
    fn late(&self, report: &mut RefsImport) -> bool {
        late(self.ctx, report)
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
            // The commits left are read from the last one recorded by a later run.
            if importer.late(report) {
                return Ok(());
            }
            // A broken tree may hold other files: only the journal's own is read, and a commit
            // without a readable one is passed over, since the ref as a whole was set aside.
            if let Some(file) = file_at(ctx, &commit, &name.file_name())? {
                // Each commit's record names the ref it was read from.
                if !importer.record(report, (refname.clone(), &commit), file)? {
                    return Ok(());
                }
                tip_read |= commit == tip;
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
    if late(ctx, report) {
        return Ok(());
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
/// context's cap: two git calls, the listing and the read.
fn file_at(ctx: &Ctx<'_>, commit: &Oid, path: &str) -> Result<Option<Vec<u8>>, RefsError> {
    match ctx.repo.entries(commit, Some(path))?.as_deref() {
        Some([entry]) if entry.mode == REGULAR && entry.kind == "blob" && entry.path == path => {
            ctx.repo.file(entry, ctx.max_bytes)
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

#[cfg(test)]
#[cfg(unix)]
mod tests {
    use std::cell::RefCell;
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::time::{Duration, Instant};

    use super::*;
    use crate::envelope::{Actor, Writer};
    use crate::journal::refs::git::GIT;
    use crate::store::Wait;
    use crate::trust::Trust;

    thread_local! {
        /// The call log of the run [`clock`] reads, and the call during which its deadline passes.
        static WATCHED: RefCell<Option<(PathBuf, usize)>> = const { RefCell::new(None) };
    }

    /// A deadline an hour away, which [`clock`] passes once the watched call has started.
    fn deadline() -> Instant {
        Instant::now()
            .checked_add(Duration::from_secs(3600))
            .expect("an hour from now")
    }

    /// The real clock until the watched call starts, then two hours ahead of it.
    fn clock() -> Instant {
        let passed = WATCHED.with_borrow(|watched| {
            watched
                .as_ref()
                .is_some_and(|(log, call)| calls(log).len() >= *call)
        });
        let now = Instant::now();
        if passed {
            now.checked_add(Duration::from_secs(7200))
                .expect("two hours from now")
        } else {
            now
        }
    }

    /// The git calls a wrapper logged, each by its first argument.
    fn calls(log: &Path) -> Vec<String> {
        std::fs::read_to_string(log)
            .unwrap_or_default()
            .lines()
            .map(str::to_owned)
            .collect()
    }

    fn git(dir: &Path, args: &[&str]) -> String {
        // Spawned as Core spawns, never while another test's store opener holds its lock.
        let _forking = crate::git::forking();
        let out = Command::new(GIT)
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

    /// A commit in `seed` holding exactly `files`, on `parent` when given.
    fn commit(seed: &Path, files: &[(&str, String)], parent: Option<&str>) -> String {
        git(seed, &["rm", "-rqf", "--ignore-unmatch", "."]);
        for (path, content) in files {
            std::fs::write(seed.join(path), content).expect("write");
        }
        git(seed, &["add", "-A"]);
        let tree = git(seed, &["write-tree"]);
        let mut args = vec!["commit-tree", tree.as_str(), "-m", "c"];
        args.extend(parent.into_iter().flat_map(|parent| ["-p", parent]));
        git(seed, &args)
    }

    /// The journal refs moved since the store's import, and the broken ref.
    const MOVED: [&str; 4] = ["a1", "b2", "c3", "d4"];
    const BROKEN: &str = "e5";

    /// A remote whose journal refs `MOVED` each moved on by one commit since `refs/old/`, and
    /// whose broken ref holds three commits, the oldest without its journal file.
    fn remote(dir: &Path) -> PathBuf {
        let seed = dir.join("seed");
        std::fs::create_dir_all(&seed).expect("dir");
        git(&seed, &["init", "-q", "-b", "main"]);
        git(
            &seed,
            &["-c", "user.name=t", "-c", "user.email=t@example.invalid"]
                .into_iter()
                .chain(["commit", "-q", "--allow-empty", "-m", "seed"])
                .collect::<Vec<_>>(),
        );
        for (position, name) in (1..).zip(MOVED) {
            let file = format!("{name}.jsonl");
            let old = commit(&seed, &[(&file, line(position))], None);
            let both = line(position) + &line(position + 100);
            let new = commit(&seed, &[(&file, both)], Some(&old));
            git(
                &seed,
                &["update-ref", &format!("refs/old/journal/{name}"), &old],
            );
            git(
                &seed,
                &["update-ref", &format!("{JOURNAL_REFS}{name}"), &new],
            );
        }
        let file = format!("{BROKEN}.jsonl");
        let without = commit(&seed, &[("other.txt", String::new())], None);
        let one = commit(&seed, &[(&file, line(50))], Some(&without));
        let two = commit(&seed, &[(&file, line(50) + &line(51))], Some(&one));
        git(
            &seed,
            &["update-ref", &format!("{BROKEN_REFS}{BROKEN}/x"), &two],
        );
        seed
    }

    /// A wrapper around git that logs each call's first argument to `log`.
    fn wrapper(dir: &Path, log: &Path) -> PathBuf {
        use std::os::unix::fs::PermissionsExt as _;
        let path = dir.join("logging-git");
        let script = format!(
            "#!/bin/sh\nprintf '%s\\n' \"$1\" >> '{}'\nexec git \"$@\"\n",
            log.display()
        );
        std::fs::write(&path, script).expect("write");
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
        path
    }

    fn author() -> Author {
        Author {
            actor: Actor::Agent,
            trust: Trust::asserted(),
            writer: Writer::new("weighin", "2.0.0").expect("valid"),
        }
    }

    /// The git calls of an import into a clone of `seed` whose store last imported the
    /// journal refs at `refs/old/`, each by its first argument, and its report; the deadline
    /// passes during call `call` (`usize::MAX`: never).
    fn run(dir: &Path, seed: &Path, call: usize) -> (Vec<String>, RefsImport) {
        let run = dir.join(format!("run-{call}"));
        std::fs::create_dir_all(&run).expect("dir");
        let url = seed.to_string_lossy().into_owned();
        git(&run, &["clone", "-q", &url, "work"]);
        let work = run.join("work");
        git(
            &work,
            &[
                "fetch",
                "-q",
                "origin",
                "+refs/old/journal/*:refs/redkiln/journal/*",
            ],
        );
        let store = Store::open(&run.join("store.sqlite3"), Wait::Command).expect("store");
        let ts = Timestamp::parse("2026-11-02T10:00:00Z").expect("valid");
        let author = author();
        let target = RefsTarget {
            repo: &work,
            remote: "origin",
            timeout: Duration::from_secs(30),
            deadline: None,
        };
        let ctx = Ctx::local(&target);
        let importer = Importer {
            ctx: &ctx,
            store: &store,
            ts: &ts,
            author: &author,
        };
        // The store's earlier import, of the old tips, from no remote.
        let earlier = import_through(&importer, "none").expect("ran");
        assert_eq!(earlier.sources.len(), MOVED.len(), "{earlier:?}");
        let log = run.join("calls");
        let program = wrapper(&run, &log);
        let mut ctx = Ctx::local(&target);
        ctx.repo.program = program.as_os_str();
        ctx.deadline = Some(deadline());
        ctx.now = clock;
        let importer = Importer {
            ctx: &ctx,
            ..importer
        };
        WATCHED.set(Some((log, call)));
        let report = import_through(&importer, "origin");
        let (log, _) = WATCHED.take().expect("watched");
        let report = report.expect("ran");
        assert!(report.fetch_failed.is_empty(), "{report:?}");
        (calls(&log), report)
    }

    /// For a deadline passing during each call of the full import (fetches, ancestry checks,
    /// a broken ref's history, file listings and reads): at most the bound of calls, the one in
    /// flight included, run past it; no ancestry check or file listing starts after it; and a
    /// file being read as it passes is not imported.
    #[test]
    fn a_deadline_passing_during_any_call_of_an_import_bounds_what_follows() {
        let dir = tempfile::tempdir().expect("tempdir");
        let seed = remote(dir.path());
        let (full, _) = run(dir.path(), &seed, usize::MAX);
        for step in ["fetch", "merge-base", "rev-list", "ls-tree", "cat-file"] {
            assert!(full.iter().any(|call| call == step), "{step}: {full:?}");
        }
        let bound = usize::try_from(MAX_IMPORT_CALLS_PAST_DEADLINE).expect("small");
        for call in 1..=full.len() {
            let (calls, report) = run(dir.path(), &seed, call);
            let (before, from) = calls.split_at_checked(call - 1).expect("the call ran");
            let (in_flight, after) = from.split_first().expect("the call ran");
            let at = format!("deadline during call {call} ({in_flight})");
            assert!(from.len() <= bound, "bound: {at}: {from:?}");
            for step in ["merge-base", "ls-tree"] {
                assert!(
                    !after.iter().any(|started| started == step),
                    "{step} started: {at}: {after:?}"
                );
            }
            if in_flight == "cat-file" {
                let read = before
                    .iter()
                    .filter(|started| *started == "cat-file")
                    .count();
                assert_eq!(
                    report.sources.len(),
                    read,
                    "store write after the deadline: {at}: {:?}",
                    report.sources
                );
            }
        }
    }
}
