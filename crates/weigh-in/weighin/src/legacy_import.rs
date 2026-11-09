//! The one-time import of Weigh-In 1.x sources into the Core store (Move 1 step 8, SPEC ST-15,
//! JR-07, M1-A): every 1.x ledger location finds ([`Env::old_ledgers`]), then the committed 1.x
//! journals under `.kb/_ledger/journal/`.
//!
//! Each source is imported once: its import is recorded as one `core.import_completed` naming
//! it (`{"legacy": {path, store_id}}`), and a source this store recorded itself is never read
//! again (decision H3). Ledgers go first, and while a found ledger is not imported no journal is,
//! so a journal line equal to a ledger event is stored under the ledger's identity (M1-A, H8).
//! An import that stops or fails records nothing; what it stored stays, and the next run
//! appends only the rest (JR-11). Sources are only read: a ledger from Core's copy on the
//! temporary disk, a journal opened read-only (ST-16, B-01). With nothing to import, no store is
//! opened or created.
//!
//! The first command (all but `ci-check`) or `SessionStart` to find a source runs it.
//! `SessionStart` imports only what fits its budget ([`Mode::Hook`]); a command imports the rest.
//! A source no run completes (over the journal cap, unreadable, failing) is told by each
//! `SessionStart`, not by every command (wi-fd5345, [`command_notes`]).

use std::collections::HashSet;
use std::io::ErrorKind;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context as _, Result};
use redkiln_core::StoreId;
use redkiln_core::envelope::{Author, Payload as _, Timestamp, decode_all};
use redkiln_core::journal::{ImportCompleted, ImportReport, ImportSource, MappedImport, Recording};
use redkiln_core::store::Filter;

use crate::env::Env;
use crate::journal;
use crate::store::Store;
use crate::store::legacy::identity::JOURNAL_STORE;
use crate::store::legacy::{Held, HeldRead, Import, OldImport};

mod journal_file;
mod notes;

use journal_file::{Checked, Read, check, read_journal};

pub(crate) use notes::{command_notes, notes, place, problem_lines};

/// How long the import may run (HK-5).
#[derive(Debug, Clone, Copy)]
pub(crate) enum Mode {
    /// `SessionStart`: sources up to [`MAX_HOOK_SOURCE_BYTES`] together (a journal no run reads
    /// counting nothing), and no batch, nor page of the store's events, read after
    /// `started + HOOK_START_BY`.
    Hook { started: Instant },
    /// A command: no deadline; Core's and this module's caps only.
    Command,
}

/// How long after the hook started a batch may still start.
pub(crate) const HOOK_START_BY: Duration = Duration::from_secs(2);
/// The most source bytes `SessionStart` imports; more waits for a command.
pub(crate) const MAX_HOOK_SOURCE_BYTES: u64 = 8 * 1024 * 1024;
/// The largest 1.x journal file read (H-13); 1.x journals are one session each.
pub(crate) const MAX_JOURNAL_FILE_BYTES: u64 = 32 * 1024 * 1024;
/// The most journal files one run imports, in name order (H-13); the rest wait for the next.
pub(crate) const MAX_JOURNAL_FILES: usize = 4096;

/// How many records one read of the store returns at most.
const PAGE: NonZeroUsize = match NonZeroUsize::new(512) {
    Some(page) => page,
    None => NonZeroUsize::MIN,
};

/// What the import did.
#[derive(Debug)]
#[must_use]
pub(crate) enum Legacy {
    /// No old ledger and no 1.x journal, or every one already imported: no store created.
    NothingDue,
    /// It ran: each source's outcome, and what waits.
    Ran(LegacyReport),
}

/// What one run of the import did.
#[derive(Debug, Default)]
pub(crate) struct LegacyReport {
    /// Each source it looked at, in order.
    pub(crate) sources: Vec<SourceRun>,
    /// Why the rest waits for another run, if it does.
    pub(crate) waiting: Option<Waiting>,
    /// Due journal files past [`MAX_JOURNAL_FILES`], left for the next run.
    pub(crate) left: usize,
    /// Whether the due sources fit what a `SessionStart` imports ([`MAX_HOOK_SOURCE_BYTES`]), so
    /// that it reaches them and tells a source that stays due; when they do not, only a command
    /// can tell it ([`command_notes`], review F1).
    pub(crate) fits_hook: bool,
}

impl LegacyReport {
    /// Whether the run read a source into the store, whole or until its deadline: what spends a
    /// hook's budget. A run that only found sources too large, unreadable or failing did not.
    pub(crate) fn imported_a_source(&self) -> bool {
        self.sources.iter().any(|source| {
            matches!(
                source.outcome,
                SourceOutcome::Completed { .. } | SourceOutcome::Stopped { .. }
            )
        })
    }
}

/// A 1.x ledger or a 1.x journal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceKind {
    Ledger,
    Journal,
}

/// One source and what became of it.
#[derive(Debug)]
pub(crate) struct SourceRun {
    pub(crate) kind: SourceKind,
    pub(crate) path: PathBuf,
    /// The ledger's store id, or [`JOURNAL_STORE`]; `None` for a ledger that could not be read.
    pub(crate) store_id: Option<StoreId>,
    pub(crate) outcome: SourceOutcome,
    /// When its outcome was decided.
    pub(crate) decided: Decided,
}

/// When a source's outcome was decided: before its import, by what the file is (over the cap,
/// not openable, a ledger Core cannot read), which stays so until the file changes; or by the
/// import itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Decided {
    BeforeImport,
    ByImport,
}

/// What became of one source.
#[derive(Debug)]
pub(crate) enum SourceOutcome {
    /// Imported and recorded; each record skipped as malformed, and why.
    Completed {
        report: ImportReport,
        problems: Vec<(u64, String)>,
    },
    /// The hook's deadline passed: what was stored stays, nothing is recorded.
    Stopped {
        report: ImportReport,
        problems: Vec<(u64, String)>,
    },
    /// Read nothing: the file is over [`MAX_JOURNAL_FILE_BYTES`].
    TooLarge { bytes: u64 },
    /// It could not be read or imported; tried again by the next run.
    Failed(anyhow::Error),
}

/// Why journals, or everything, wait for another run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Waiting {
    /// The due sources are more than a hook imports.
    TooLargeForHook { bytes: u64 },
    /// A found ledger is not imported yet, so no journal is.
    Ledger,
    /// The hook's deadline passed.
    Deadline,
}

/// Imports every old source not yet imported, ledgers first.
///
/// # Errors
/// The store cannot be located, opened or read, or the clock cannot be read as a `Timestamp`.
/// A source that cannot be read is a [`SourceOutcome`], not an error.
pub(crate) fn import_due(env: &Env, mode: Mode, actor: &str) -> Result<Legacy> {
    let journals = journal_files(&env.root())?;
    if env.old_ledgers.is_empty() && journals.is_empty() {
        return Ok(Legacy::NothingDue);
    }
    let path = env.store_path()?;
    let existing = Store::open_existing(path, env.wait).context(StoreUnopened)?;
    let done = match &existing {
        Some(store) => imported_sources(store)?,
        None => HashSet::new(),
    };
    let is_due = |p: &&PathBuf| !done.contains(p.to_string_lossy().as_ref());
    let ledgers: Vec<&PathBuf> = env.old_ledgers.iter().filter(is_due).collect();
    let mut journals: Vec<&PathBuf> = journals.iter().filter(is_due).collect();
    if ledgers.is_empty() && journals.is_empty() {
        return Ok(Legacy::NothingDue);
    }
    let mut report = LegacyReport {
        left: journals.len().saturating_sub(MAX_JOURNAL_FILES),
        ..LegacyReport::default()
    };
    journals.truncate(MAX_JOURNAL_FILES);
    // Measured in both modes: a command tells a source that stays due only when no session
    // start could reach it (review F1).
    let bytes = due_bytes(&ledgers, &journals);
    report.fits_hook = bytes <= MAX_HOOK_SOURCE_BYTES;
    if matches!(mode, Mode::Hook { .. }) && !report.fits_hook {
        report.waiting = Some(Waiting::TooLargeForHook { bytes });
        return Ok(Legacy::Ran(report));
    }
    let store = match existing {
        Some(store) => store,
        None => Store::open(path, env.wait).context(StoreUnopened)?,
    };
    let run = Run {
        store: &store,
        ts: Timestamp::from_system_time(SystemTime::now()).context("the import's time")?,
        author: store.author(actor)?,
        deadline: match mode {
            Mode::Hook { started } => started.checked_add(HOOK_START_BY),
            Mode::Command => None,
        },
    };
    run.sources(&ledgers, &journals, &mut report)?;
    Ok(Legacy::Ran(report))
}

/// How many 1.x sources wait for the import.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Due {
    pub(crate) ledgers: usize,
    pub(crate) journals: usize,
}

/// The 1.x sources `store` has not imported (every one when there is no store yet), counted
/// and read only: `status` reports them and imports nothing (decision D3).
///
/// # Errors
/// The journal directory cannot be listed, or the store's import records cannot be read.
pub(crate) fn due(env: &Env, store: Option<&Store>) -> Result<Due> {
    let journals = journal_files(&env.root())?;
    let done = match store {
        Some(store) => imported_sources(store)?,
        None => HashSet::new(),
    };
    let not_done = |p: &&PathBuf| !done.contains(p.to_string_lossy().as_ref());
    Ok(Due {
        ledgers: env.old_ledgers.iter().filter(not_done).count(),
        journals: journals.iter().filter(not_done).count(),
    })
}

/// The store could not be opened for the import: a hook leaves telling it to the prime, which
/// opens it again.
#[derive(Debug)]
pub(crate) struct StoreUnopened;

impl std::fmt::Display for StoreUnopened {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("opening the store for the 1.x import")
    }
}

/// Why a ledger that did not complete holds the journals back.
fn waiting_after(outcome: &SourceOutcome) -> Option<Waiting> {
    match outcome {
        SourceOutcome::Completed { .. } => None,
        SourceOutcome::Stopped { .. } => Some(Waiting::Deadline),
        SourceOutcome::TooLarge { .. } | SourceOutcome::Failed(_) => Some(Waiting::Ledger),
    }
}

/// One run's store and how it records each import.
struct Run<'s> {
    store: &'s Store,
    ts: Timestamp,
    author: Author,
    deadline: Option<Instant>,
}

impl Run<'_> {
    /// Imports `ledgers`, then, when every one completed, `journals`, into `report`; when one did
    /// not, only the journals no run imports are reported ([`check`]).
    ///
    /// # Errors
    /// The store cannot be read for the events it holds.
    fn sources(
        &self,
        ledgers: &[&PathBuf],
        journals: &[&PathBuf],
        report: &mut LegacyReport,
    ) -> Result<()> {
        for path in ledgers {
            let source = self.ledger(path);
            let waiting = waiting_after(&source.outcome);
            report.sources.push(source);
            if waiting.is_some() {
                report.waiting = waiting;
                // The journals wait, but one that no run imports is told now (round 5 F3).
                report.sources.extend(
                    journals
                        .iter()
                        .map(|path| check(path))
                        .filter_map(Checked::settled),
                );
                return Ok(());
            }
        }
        self.journals(journals, report)
    }

    /// Imports `journals` into `report`, in order. One over the cap or that cannot be opened is
    /// settled first, needing no held event, so even a hook past its deadline tells it (review
    /// round 2 F1); the store's events are read only when a journal may import.
    ///
    /// # Errors
    /// The store cannot be read for the events it holds.
    fn journals(&self, journals: &[&PathBuf], report: &mut LegacyReport) -> Result<()> {
        let checked: Vec<Checked<'_>> = journals.iter().map(|path| check(path)).collect();
        if !checked.iter().any(|c| matches!(c, Checked::Due(_))) {
            report
                .sources
                .extend(checked.into_iter().filter_map(Checked::settled));
            return Ok(());
        }
        // After every ledger, so this run's ledger events are among those a line can equal. A
        // hook past its deadline reads no more, neither a page of the store's events nor a
        // journal (review F3, wi-233b23).
        let held = match self.store.held(self.deadline)? {
            HeldRead::All(held) => Some(held),
            HeldRead::PastDeadline => None,
        };
        let mut importing = held.as_ref();
        for checked in checked {
            match (checked, importing) {
                (Checked::Settled(source), _) => report.sources.push(source),
                (Checked::Due(path), Some(held)) => {
                    let source = self.journal(path, held);
                    if matches!(source.outcome, SourceOutcome::Stopped { .. }) {
                        importing = None;
                    }
                    report.sources.push(source);
                }
                // Past the deadline: it waits, and only settled journals are still told.
                (Checked::Due(_), None) => {}
            }
        }
        if importing.is_none() {
            report.waiting = Some(Waiting::Deadline);
        }
        Ok(())
    }

    /// How the source at `path`, whose events take `store_id`, is imported and recorded.
    fn import(&self, path: &Path, store_id: StoreId) -> Import {
        Import {
            recording: Recording {
                source: ImportSource::Legacy {
                    path: path.to_string_lossy().into_owned(),
                    store_id,
                },
                // Each record owns its time and author: a few short words per source.
                ts: self.ts.clone(),
                author: self.author.clone(),
            },
            deadline: self.deadline,
        }
    }

    /// The 1.x ledger at `path`, read from Core's copy (ST-15).
    fn ledger(&self, path: &Path) -> SourceRun {
        let source = |store_id, outcome, decided| SourceRun {
            kind: SourceKind::Ledger,
            path: path.to_path_buf(),
            store_id,
            outcome,
            decided,
        };
        let ledger = match redkiln_core::legacy::read_ledger(path) {
            Ok(ledger) => ledger,
            Err(e) => {
                let e = anyhow::Error::new(e).context(format!("reading {}", path.display()));
                return source(None, SourceOutcome::Failed(e), Decided::BeforeImport);
            }
        };
        let store_id = ledger.store_id;
        let imported = self
            .store
            .import_ledger(&ledger.events, self.import(path, store_id));
        source(Some(store_id), outcome(imported, path), Decided::ByImport)
    }

    /// The 1.x journal at `path`, opened read-only.
    fn journal(&self, path: &Path, held: &Held) -> SourceRun {
        let outcome = match read_journal(path) {
            Ok(Read::Bytes(bytes)) => outcome(
                self.store
                    .import_journal(&bytes, held, self.import(path, JOURNAL_STORE)),
                path,
            ),
            Ok(Read::TooLarge(bytes)) => SourceOutcome::TooLarge { bytes },
            Err(e) => SourceOutcome::Failed(e),
        };
        SourceRun {
            kind: SourceKind::Journal,
            path: path.to_path_buf(),
            store_id: Some(JOURNAL_STORE),
            outcome,
            decided: Decided::ByImport,
        }
    }
}

/// An import's result as a source's outcome.
fn outcome(imported: Result<OldImport>, path: &Path) -> SourceOutcome {
    match imported {
        Ok(OldImport {
            outcome: MappedImport::Completed(report),
            problems,
        }) => SourceOutcome::Completed { report, problems },
        Ok(OldImport {
            outcome: MappedImport::Stopped(report),
            problems,
        }) => SourceOutcome::Stopped { report, problems },
        Err(e) => SourceOutcome::Failed(e.context(format!("importing {}", path.display()))),
    }
}

/// Every `*.jsonl` file directly under `<root>/.kb/_ledger/journal/`, in name order; no
/// directory is no journals.
///
/// # Errors
/// The directory exists and cannot be listed.
fn journal_files(root: &Path) -> Result<Vec<PathBuf>> {
    let dir = root.join(journal::DIR);
    let entries = match std::fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("listing {}", dir.display())),
    };
    let mut paths = entries
        .map(|entry| entry.map(|e| e.path()))
        .collect::<std::io::Result<Vec<PathBuf>>>()
        .with_context(|| format!("listing {}", dir.display()))?;
    paths.retain(|p| p.extension().is_some_and(|x| x == "jsonl") && p.is_file());
    paths.sort();
    Ok(paths)
}

/// The paths of the old sources this store recorded importing itself (decision H3). A record
/// ingested from another store's journal names what that store read, so it does not count.
///
/// # Errors
/// A store error.
fn imported_sources(store: &Store) -> Result<HashSet<String>> {
    let filter = Filter::all().of_type(ImportCompleted::TYPE)?;
    let core = store.core();
    let mine = core.store_id();
    let mut done = HashSet::new();
    let mut after = 0;
    loop {
        let page = core
            .read(&filter, after, PAGE)
            .context("reading the store's import records")?;
        let Some(end) = page.last().map(|stored| stored.position) else {
            break;
        };
        let own = page.iter().filter(|stored| stored.id.store() == mine);
        for envelope in decode_all::<ImportCompleted>(own).events {
            if let ImportSource::Legacy { path, .. } = envelope.data.source {
                done.insert(path);
            }
        }
        if page.len() < PAGE.get() {
            break;
        }
        after = end;
    }
    Ok(done)
}

/// The bytes the due sources hold: each ledger with its `-wal`, and each journal up to
/// [`MAX_JOURNAL_FILE_BYTES`]. A journal over it counts 0, since no run reads it: the hook reaches
/// it and says it is not imported, rather than leaving it to a command that never imports it
/// either (wi-fd5345). A file that cannot be measured counts 0: its import says why.
fn due_bytes(ledgers: &[&PathBuf], journals: &[&PathBuf]) -> u64 {
    let size = |path: &Path| std::fs::metadata(path).map_or(0, |m| m.len());
    let wal = |path: &Path| {
        let mut name = path.as_os_str().to_owned();
        name.push("-wal");
        size(Path::new(&name))
    };
    let journal = |path: &Path| match size(path) {
        len if len > MAX_JOURNAL_FILE_BYTES => 0,
        len => len,
    };
    ledgers
        .iter()
        .map(|p| size(p).saturating_add(wal(p)))
        .chain(journals.iter().map(|p| journal(p)))
        .fold(0, u64::saturating_add)
}

#[cfg(test)]
mod tests;
