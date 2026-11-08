//! What the user hears of a 1.x import: one line per thing to know, and the skipped records for
//! the hook error log.

use super::{
    Decided, LegacyReport, MAX_JOURNAL_FILE_BYTES, MAX_JOURNAL_FILES, SourceKind, SourceOutcome,
    SourceRun, Waiting,
};

/// One line for the user per thing they should hear: a ledger imported once, records skipped, a
/// source not imported, and what waits. `SessionStart` says these, prefixed `Weigh-In: `.
///
/// A journal imported cleanly says nothing: its events already counted in `stats`, and a 1.x
/// journal pulled later would otherwise put a line on whichever command ran next. `weighin
/// journal import --json` lists every source.
pub(crate) fn notes(report: &LegacyReport) -> Vec<String> {
    told(report, Standing::Told)
}

/// [`notes`] less those of a source that stays due: one no run completes, which `SessionStart`
/// tells once per session start rather than every command (wi-fd5345). What this run imported,
/// and what the next run imports, is still said; and when the due sources are more than a
/// session start imports, so it never reaches them, every note is (review F1).
pub(crate) fn command_notes(report: &LegacyReport) -> Vec<String> {
    told(report, Standing::Left)
}

/// Whether the notes tell of a source that stays due ([`stays_due`]), and of the journals such
/// a ledger holds back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Standing {
    Told,
    Left,
}

/// The notes of `report`, those of a source that stays due as `standing` says.
fn told(report: &LegacyReport, standing: Standing) -> Vec<String> {
    // A session start that cannot reach the sources tells none of them: then a command does.
    let leave = standing == Standing::Left && report.fits_hook;
    // Held back by a ledger whose failure stays until the file changes.
    let held_back = report.waiting == Some(Waiting::Ledger)
        && report
            .sources
            .iter()
            .any(|s| s.kind == SourceKind::Ledger && stays_due(s));
    let mut notes = Vec::new();
    for source in &report.sources {
        if stays_due(source) && leave {
            continue;
        }
        match &source.outcome {
            SourceOutcome::Completed {
                report: counts,
                problems,
            } => {
                if source.kind == SourceKind::Ledger {
                    notes.push(format!(
                        "imported the weighin 1.x ledger at {} once: {} event(s), {} already held, {} skipped as malformed; the file is left as it was",
                        source.path.display(),
                        counts.imported,
                        counts.present,
                        counts.malformed
                    ));
                }
                if let Some(note) = skipped(source, counts.malformed, problems) {
                    notes.push(note);
                }
            }
            // The deadline's note says what waits.
            SourceOutcome::Stopped { .. } => {}
            SourceOutcome::TooLarge { bytes } => notes.push(format!(
                "the weighin 1.x journal {} is {bytes} bytes, more than the {MAX_JOURNAL_FILE_BYTES} read, so it is not imported",
                source.path.display()
            )),
            SourceOutcome::Failed(e) => notes.push(format!(
                "could not import the weighin 1.x {} {}: {e:#}; the next weighin command tries again",
                kind(source.kind),
                source.path.display()
            )),
        }
    }
    // Journals held back by a ledger that stays due wait with it (review F2).
    if report.left > 0 && !(held_back && leave) {
        notes.push(format!(
            "{} more weighin 1.x journal(s) wait: one run imports at most {MAX_JOURNAL_FILES}, and the next imports the rest",
            report.left
        ));
    }
    if !(held_back && leave) {
        notes.extend(report.waiting.map(waiting));
    }
    notes
}

/// Whether `source` stays due until its file changes: over the cap, or failed before its import
/// (not openable, a ledger Core cannot read). A failure of the import itself may pass, so a
/// command tells it (review round 3 F2).
fn stays_due(source: &SourceRun) -> bool {
    match source.outcome {
        SourceOutcome::TooLarge { .. } => true,
        SourceOutcome::Failed(_) => source.decided == Decided::BeforeImport,
        SourceOutcome::Completed { .. } | SourceOutcome::Stopped { .. } => false,
    }
}

/// Each record skipped as malformed, one line each, naming its source and where in it: for the
/// hook error log.
pub(crate) fn problem_lines(report: &LegacyReport) -> Vec<String> {
    report
        .sources
        .iter()
        .flat_map(|source| {
            let problems = match &source.outcome {
                SourceOutcome::Completed { problems, .. }
                | SourceOutcome::Stopped { problems, .. } => problems.as_slice(),
                SourceOutcome::TooLarge { .. } | SourceOutcome::Failed(_) => &[],
            };
            problems.iter().map(move |(number, why)| {
                format!(
                    "weighin 1.x import: {} {} {number} skipped: {why}",
                    source.path.display(),
                    place(source.kind)
                )
            })
        })
        .collect()
}

/// A source's kind in a sentence.
const fn kind(kind: SourceKind) -> &'static str {
    match kind {
        SourceKind::Ledger => "ledger",
        SourceKind::Journal => "journal",
    }
}

/// What a record's number is in its source.
pub(crate) const fn place(kind: SourceKind) -> &'static str {
    match kind {
        SourceKind::Ledger => "position",
        SourceKind::Journal => "line",
    }
}

/// The note for `malformed` records skipped in `source`, naming the first.
fn skipped(source: &SourceRun, malformed: u64, problems: &[(u64, String)]) -> Option<String> {
    let (number, why) = problems.first()?;
    Some(format!(
        "{malformed} record(s) of {} were skipped as malformed (the first, {} {number}: {why})",
        source.path.display(),
        place(source.kind)
    ))
}

/// What waits, in a sentence.
fn waiting(waiting: Waiting) -> String {
    match waiting {
        Waiting::TooLargeForHook { bytes } => format!(
            "the weighin 1.x ledger and journals here hold {bytes} bytes, more than a session start imports; the next weighin command imports them, or says why it cannot"
        ),
        Waiting::Ledger => "the weighin 1.x journals wait until the 1.x ledger is imported".to_owned(),
        Waiting::Deadline => "the weighin 1.x import ran out of time; the next weighin command or session start finishes it".to_owned(),
    }
}
