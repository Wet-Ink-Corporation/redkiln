//! The one-time import of anneal ledgers into the suite store (Move 2 step 3.5, SPEC ST-15,
//! JR-07, JR-13): every old ledger location found (`StoreLocation::old_ledgers`), each read
//! only from Core's copy (`redkiln_core::legacy::read_ledger`, ST-16) and mapped by [`map`].
//!
//! Each source is imported once: its import is recorded as one `core.import_completed` naming
//! it (`{"legacy": {path, store_id}}`), and a source this store recorded is never read again. An
//! import that stops or fails records nothing; what it stored stays, and the next run appends
//! only the rest (JR-11), since every record keeps its old identity. With nothing due, no store
//! is opened or created.
//!
//! Who runs it: `SessionStart` ([`at_session_start`]), within a hook's budget ([`Budget::hook`]:
//! sources up to [`MAX_HOOK_SOURCE_BYTES`], no batch started [`HOOK_START_BY`] after the hook
//! began), and every command that opens the store to write ([`open_for_command`]), without one.
//! Never `bench status`, which reports what is due ([`due`]), nor any other hook.

mod map;

use std::collections::HashSet;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use redkiln_core::StoreId;
use redkiln_core::envelope::{Actor, Author, Timestamp};
use redkiln_core::journal::{ImportReport, ImportSource, LineError, MappedImport, Recording};
use redkiln_core::suite::Owner;
use redkiln_core::trust::Trust;

use crate::error::CliError;
use crate::error_chain::reason;
use crate::paths::{GIT_TIMEOUT, PathError, StoreLocation};
use crate::plan::decider;
use crate::store::{Store, StoreError, Wait};
use map::{MAX_PROBLEMS, Records};

pub(crate) use map::Tally;

/// How long after a hook started an import batch may still start.
pub(crate) const HOOK_START_BY: Duration = Duration::from_secs(2);

/// The most source bytes (each ledger with its `-wal`) `SessionStart` imports; more waits for a
/// command.
pub(crate) const MAX_HOOK_SOURCE_BYTES: u64 = 8 * 1024 * 1024;

/// How much an import may read and for how long.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Budget {
    /// No batch is stored after it (`None`: no deadline).
    pub(crate) deadline: Option<Instant>,
    /// The most source bytes read; more waits for another run (`None`: no cap beyond Core's).
    pub(crate) max_bytes: Option<u64>,
}

impl Budget {
    /// A command's: no deadline, Core's caps only.
    pub(crate) const COMMAND: Self = Self {
        deadline: None,
        max_bytes: None,
    };

    /// A hook's, which started at `started`: [`MAX_HOOK_SOURCE_BYTES`] and [`HOOK_START_BY`].
    pub(crate) fn hook(started: Instant) -> Self {
        Self {
            deadline: started.checked_add(HOOK_START_BY),
            max_bytes: Some(MAX_HOOK_SOURCE_BYTES),
        }
    }
}

/// What one run of the import did.
#[derive(Debug, Default)]
pub(crate) struct Run {
    /// Each source it looked at, in order.
    pub(crate) sources: Vec<SourceRun>,
    /// Why the rest waits for another run, if it does.
    pub(crate) waiting: Option<Waiting>,
}

/// One source and what became of it.
#[derive(Debug)]
pub(crate) struct SourceRun {
    /// The old ledger.
    pub(crate) path: PathBuf,
    /// What became of it.
    pub(crate) outcome: Outcome,
}

/// What became of one source.
#[derive(Debug)]
pub(crate) enum Outcome {
    /// Imported and recorded.
    Completed(Imported),
    /// The deadline passed: what was stored stays, nothing is recorded.
    Stopped(Imported),
    /// It could not be read or imported, and is tried again by the next run; why.
    Failed(String),
}

/// What importing one source did: Core's counts, what mapping found, and why each record
/// skipped was (the first [`MAX_PROBLEMS`], in record order).
#[derive(Debug)]
pub(crate) struct Imported {
    /// Core's counts (an approval imported with its plan approval counts as a record).
    pub(crate) report: ImportReport,
    /// What mapping found.
    pub(crate) tally: Tally,
}

/// Why the rest waits for another run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Waiting {
    /// The due sources are more than the budget reads: a command imports them.
    TooLarge {
        /// Their bytes.
        bytes: u64,
    },
    /// The deadline passed.
    Deadline,
}

/// The old ledgers in `old` that `store` has not recorded importing: all of them with no store.
///
/// # Errors
/// [`StoreError::Core`] when the store's import records cannot be read.
pub(crate) fn due<'p>(
    store: Option<&Store>,
    old: &'p [PathBuf],
) -> Result<Vec<&'p Path>, StoreError> {
    if old.is_empty() {
        return Ok(Vec::new());
    }
    let done = match store {
        Some(store) => store.imported_sources()?,
        None => HashSet::new(),
    };
    Ok(old
        .iter()
        .filter(|path| !done.contains(path.to_string_lossy().as_ref()))
        .map(PathBuf::as_path)
        .collect())
}

/// Opens the store `location` names for a command, creating it when absent, and first imports
/// every old ledger it has not, without a deadline. What the import did is written to standard
/// error: one `bench: warning:` line per source, its problems after its outcome (SPEC CLI-08).
///
/// # Errors
/// [`CliError::Ledger`] when the store cannot be opened or its import records read;
/// [`CliError::Path`] when standard error refuses the report. A source that cannot be imported
/// is reported, not an error: the next command tries it again.
pub(crate) fn open_for_command(location: &StoreLocation) -> Result<Store, CliError> {
    let store = Store::open(&location.path)?;
    let due = due(Some(&store), &location.old_ledgers)?;
    if due.is_empty() {
        return Ok(store);
    }
    let owner = || decider::owner(&location.from, GIT_TIMEOUT).map_err(|error| reason(&error));
    let run = Importer::new(&store, Budget::COMMAND, "cli", owner)?.run(&due);
    let mut err = io::stderr().lock();
    for warning in warnings(lines(&run)) {
        crate::diagnostic::warning(&mut err, &warning).map_err(PathError::Warning)?;
    }
    Ok(store)
}

/// `lines` as warnings, one per headline (CLI-08: one line each): each detail line joins the
/// headline before it after `"; "`. A detail before any headline is a warning of its own.
fn warnings(lines: Vec<Line>) -> Vec<String> {
    let mut warnings: Vec<String> = Vec::new();
    for line in lines {
        match warnings.last_mut() {
            Some(warning) if !line.headline => {
                warning.push_str("; ");
                warning.push_str(&line.text);
            }
            _ => warnings.push(line.text),
        }
    }
    warnings
}

/// [`open_for_command`] for a command that opens the store itself after: with no old ledger
/// found, nothing is opened.
///
/// # Errors
/// As [`open_for_command`].
pub(crate) fn import_for_command(location: &StoreLocation) -> Result<(), CliError> {
    if location.old_ledgers.is_empty() {
        return Ok(());
    }
    open_for_command(location).map(drop)
}

/// `SessionStart`'s import, within `budget`: `None` when nothing is due, and then no store is
/// opened or created; otherwise what the run did. The owner is asked of git within
/// `git_timeout`, only when a human approval needs one.
///
/// # Errors
/// [`StoreError`] when the store cannot be opened, created or read; a source that cannot be
/// imported is an [`Outcome::Failed`].
pub(crate) fn at_session_start(
    location: &StoreLocation,
    budget: Budget,
    git_timeout: Duration,
) -> Result<Option<Run>, StoreError> {
    if location.old_ledgers.is_empty() {
        return Ok(None);
    }
    let existing = Store::open_existing(&location.path, Wait::Hook)?;
    let due = due(existing.as_ref(), &location.old_ledgers)?;
    if due.is_empty() {
        return Ok(None);
    }
    if let Some(max) = budget.max_bytes {
        let bytes = due_bytes(&due);
        if bytes > max {
            return Ok(Some(Run {
                sources: Vec::new(),
                waiting: Some(Waiting::TooLarge { bytes }),
            }));
        }
    }
    let store = match existing {
        Some(store) => store,
        None => Store::open_with(&location.path, Wait::Hook)?,
    };
    let owner = || decider::owner(&location.from, git_timeout).map_err(|error| reason(&error));
    Ok(Some(
        Importer::new(&store, budget, "hook", owner)?.run(&due),
    ))
}

/// One run's store, budget, record and owner.
#[derive(Debug)]
pub(crate) struct Importer<'s, F> {
    /// The store imported into.
    pub(crate) store: &'s Store,
    /// How much the run may read and for how long.
    pub(crate) budget: Budget,
    /// When the run started and who runs it, which each source's `core.import_completed` names.
    pub(crate) ts: Timestamp,
    /// Who runs it.
    pub(crate) author: Author,
    /// The suite owner who decides a human's approval (AP-05), asked only when one needs it.
    pub(crate) owner: F,
}

impl<'s, F: Fn() -> Result<Option<Owner>, String>> Importer<'s, F> {
    /// An importer into `store` within `budget`, recorded as run now by `actor`.
    ///
    /// # Errors
    /// [`StoreError::Field`] when the clock or `actor` cannot be written in an envelope.
    pub(crate) fn new(
        store: &'s Store,
        budget: Budget,
        actor: &str,
        owner: F,
    ) -> Result<Self, StoreError> {
        Ok(Self {
            store,
            budget,
            // The import's adapter reads the clock, as `store::Timestamp::now` does for writes.
            ts: Timestamp::from_system_time(SystemTime::now()).map_err(StoreError::Field)?,
            author: Author {
                actor: Actor::parse(actor).map_err(StoreError::Field)?,
                trust: Trust::asserted(),
                // The import's record owns its writer: two short words.
                writer: store.writer().clone(),
            },
            owner,
        })
    }

    /// Imports each of `due` in order. After a source stops at the deadline the rest wait; a
    /// source that cannot be read or imported is an [`Outcome::Failed`] and the next goes on.
    pub(crate) fn run(&self, due: &[&Path]) -> Run {
        let mut run = Run::default();
        for path in due {
            if self
                .budget
                .deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
            {
                run.waiting = Some(Waiting::Deadline);
                break;
            }
            let outcome = self.source(path);
            let stopped = matches!(outcome, Outcome::Stopped(_));
            run.sources.push(SourceRun {
                path: path.to_path_buf(),
                outcome,
            });
            if stopped {
                run.waiting = Some(Waiting::Deadline);
                break;
            }
        }
        run
    }

    /// How the source at `path`, whose events take `store_id`, is recorded (JR-13).
    fn recording(&self, path: &Path, store_id: StoreId) -> Recording {
        Recording {
            source: ImportSource::Legacy {
                path: path.to_string_lossy().into_owned(),
                store_id,
            },
            // Each source's record owns its time and author: a few short words.
            ts: self.ts.clone(),
            author: self.author.clone(),
        }
    }

    /// Reads the old ledger at `path` from Core's copy and imports it.
    fn source(&self, path: &Path) -> Outcome {
        let ledger = match redkiln_core::legacy::read_ledger(path) {
            Ok(ledger) => ledger,
            Err(error) => return Outcome::Failed(reason(&error)),
        };
        let mut records = Records::new(
            &ledger.events,
            ledger.store_id,
            self.store.writer(),
            &self.owner,
        );
        let recording = self.recording(path, ledger.store_id);
        let imported = self
            .store
            .import(&mut records, recording, self.budget.deadline);
        finish(records.into_tally(), imported)
    }
}

/// One source's outcome from Core's import and what mapping found.
fn finish(mut tally: Tally, imported: Result<MappedImport, StoreError>) -> Outcome {
    let (report, completed) = match imported {
        Ok(MappedImport::Completed(report)) => (report, true),
        Ok(MappedImport::Stopped(report)) => (report, false),
        Err(error) => return Outcome::Failed(reason(&error)),
    };
    // Core names why it refused a mapped record; an unmapped one is already in the tally.
    tally.problems.extend(
        report
            .problems
            .iter()
            .filter(|problem| !matches!(problem.reason, LineError::Unmapped))
            .map(|problem| (problem.line, reason(&problem.reason))),
    );
    tally.problems.sort_by_key(|(number, _)| *number);
    tally.problems.truncate(MAX_PROBLEMS);
    let imported = Imported { report, tally };
    if completed {
        Outcome::Completed(imported)
    } else {
        Outcome::Stopped(imported)
    }
}

/// The bytes the due sources hold: each ledger with its `-wal`. A file that cannot be measured
/// counts 0: its import says why.
fn due_bytes(due: &[&Path]) -> u64 {
    let size = |path: &Path| std::fs::metadata(path).map_or(0, |m| m.len());
    due.iter()
        .map(|path| {
            let mut wal = path.as_os_str().to_owned();
            wal.push("-wal");
            size(path).saturating_add(size(Path::new(&wal)))
        })
        .fold(0, u64::saturating_add)
}

/// One line of a run's report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Line {
    /// A source's outcome, or why the run waits; otherwise a detail under it.
    pub(crate) headline: bool,
    /// The text, without a prefix.
    pub(crate) text: String,
}

/// What `run` did, as lines a person reads: per source its outcome, its approvals and dropped
/// fields, then each problem; then why the rest waits.
pub(crate) fn lines(run: &Run) -> Vec<Line> {
    let mut lines = Vec::new();
    for source in &run.sources {
        let path = source.path.display();
        match &source.outcome {
            Outcome::Completed(imported) => {
                let head = format!(
                    "the anneal ledger {path} is left as it is: its events are imported into the \
                     suite store, once ({})",
                    counts(imported)
                );
                push_imported(&mut lines, head, imported);
            }
            Outcome::Stopped(imported) => {
                let head = format!(
                    "the anneal ledger {path} is left as it is: its import stopped at the hook's \
                     deadline ({}); the next session or bench command goes on with it",
                    counts(imported)
                );
                push_imported(&mut lines, head, imported);
            }
            Outcome::Failed(why) => lines.push(Line {
                headline: true,
                text: format!(
                    "the anneal ledger {path} is left as it is, not imported: {why}; the next \
                     session or bench command tries again"
                ),
            }),
        }
    }
    if let Some(Waiting::TooLarge { bytes }) = run.waiting {
        lines.push(Line {
            headline: true,
            text: format!(
                "the anneal ledgers to import ({bytes} bytes) are more than a session start \
                 imports; the next bench command that records imports them, leaving them as they \
                 are"
            ),
        });
    }
    lines
}

/// Core's counts of one import, as words.
fn counts(imported: &Imported) -> String {
    let report = &imported.report;
    let approvals = match imported.tally.approvals {
        0 => String::new(),
        n => format!("; {n} plan approvals also recorded for other plugins"),
    };
    format!(
        "{} imported, {} already there, {} skipped{approvals}",
        report.imported, report.present, report.malformed
    )
}

/// `head`, then the detail lines of `imported`.
fn push_imported(lines: &mut Vec<Line>, head: String, imported: &Imported) {
    lines.push(Line {
        headline: true,
        text: head,
    });
    let tally = &imported.tally;
    let detail = |text: String| Line {
        headline: false,
        text,
    };
    let unrecorded = tally
        .no_owner
        .saturating_add(tally.model_or_unknown)
        .saturating_add(tally.bad_hash);
    if unrecorded > 0 {
        lines.push(detail(format!(
            "{unrecorded} plan approvals are recorded for bench only (no owner: {}, by a model or \
             no one known: {}, a hash that is no sha256 subject: {})",
            tally.no_owner, tally.model_or_unknown, tally.bad_hash
        )));
    }
    if let Some(why) = &tally.owner_error {
        lines.push(detail(format!(
            "the owner could not be asked of git: {why}"
        )));
    }
    if tally.context_dropped > 0 {
        lines.push(detail(format!(
            "{} worktree, branch or head values Core refuses were dropped from events kept",
            tally.context_dropped
        )));
    }
    lines.extend(
        tally
            .problems
            .iter()
            .map(|(number, why)| detail(format!("record {number}: {why}"))),
    );
}

#[cfg(test)]
mod tests;
