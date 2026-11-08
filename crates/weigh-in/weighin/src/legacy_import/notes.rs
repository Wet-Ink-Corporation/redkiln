//! What the user hears of a 1.x import: one line per thing to know, and the skipped records for
//! the hook error log.

use super::{
    LegacyReport, MAX_JOURNAL_FILE_BYTES, MAX_JOURNAL_FILES, SourceKind, SourceOutcome, SourceRun,
    Waiting,
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
/// and what the next run imports, is still said.
pub(crate) fn command_notes(report: &LegacyReport) -> Vec<String> {
    told(report, Standing::Left)
}

/// Whether the notes tell of a source that stays due: one over the journal cap, one that could
/// not be read or imported, and the journals a ledger that did not import holds back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Standing {
    Told,
    Left,
}

/// The notes of `report`, those of a source that stays due as `standing` says.
fn told(report: &LegacyReport, standing: Standing) -> Vec<String> {
    let mut notes = Vec::new();
    for source in &report.sources {
        let stays_due = matches!(
            source.outcome,
            SourceOutcome::TooLarge { .. } | SourceOutcome::Failed(_)
        );
        if stays_due && standing == Standing::Left {
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
    if report.left > 0 {
        notes.push(format!(
            "{} more weighin 1.x journal(s) wait: one run imports at most {MAX_JOURNAL_FILES}, and the next imports the rest",
            report.left
        ));
    }
    let held_back = report.waiting == Some(Waiting::Ledger);
    if !(held_back && standing == Standing::Left) {
        notes.extend(report.waiting.map(waiting));
    }
    notes
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
            "the weighin 1.x ledger and journals here hold {bytes} bytes, more than a session start imports; the next weighin command imports them"
        ),
        Waiting::Ledger => "the weighin 1.x journals wait until the 1.x ledger is imported".to_owned(),
        Waiting::Deadline => "the weighin 1.x import ran out of time; the next weighin command or session start finishes it".to_owned(),
    }
}
