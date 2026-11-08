//! What `weighin journal import` says of the one-time 1.x import it runs first
//! (`legacy_import`): `data.legacy` and `data.legacy_waiting` of `weighin.journal-import/1`
//! (decision H6), its findings, and its part of the summary line.

use redkiln_core::StoreId;
use serde::Serialize;

use crate::cli_envelope::{Finding, Severity};
use crate::legacy_import::{
    Legacy, LegacyReport, SourceKind, SourceOutcome, SourceRun, Waiting, place,
};

/// One 1.x source in `data.legacy`.
#[derive(Serialize)]
pub(super) struct LegacyData {
    kind: &'static str,
    path: String,
    store_id: Option<StoreId>,
    outcome: &'static str,
    read: u64,
    imported: u64,
    present: u64,
    malformed: u64,
}

/// `data.legacy`: each 1.x source the import looked at; empty when nothing was due.
pub(super) fn data(legacy: &Legacy) -> Vec<LegacyData> {
    report(legacy).map_or_else(Vec::new, |report| {
        report.sources.iter().map(source_data).collect()
    })
}

/// `data.legacy_waiting`: why the rest waits for another run, or `null`.
pub(super) fn waiting(legacy: &Legacy) -> Option<&'static str> {
    report(legacy)?.waiting.map(|waiting| match waiting {
        Waiting::TooLargeForHook { .. } => "too-large-for-hook",
        Waiting::Ledger => "ledger",
        Waiting::Deadline => "deadline",
    })
}

/// Its findings: each record skipped as malformed (`legacy-malformed`, at most `MAX_PROBLEMS`
/// per source), and each source not read (`legacy-unread`), all warnings.
pub(super) fn findings(legacy: &Legacy) -> Vec<Finding> {
    let Some(report) = report(legacy) else {
        return Vec::new();
    };
    report.sources.iter().flat_map(source_findings).collect()
}

/// `; 1.x: imported 3 event(s) from 2 source(s)`, or nothing when nothing was due.
pub(super) fn line(legacy: &Legacy) -> String {
    let Some(report) = report(legacy) else {
        return String::new();
    };
    let imported = data(legacy)
        .iter()
        .fold(0_u64, |sum, s| sum.saturating_add(s.imported));
    let line = format!(
        "; 1.x: imported {imported} event(s) from {} source(s)",
        report.sources.len()
    );
    match waiting(legacy) {
        Some(waiting) => format!("{line}, the rest waits ({waiting})"),
        None => line,
    }
}

fn report(legacy: &Legacy) -> Option<&LegacyReport> {
    match legacy {
        Legacy::NothingDue => None,
        Legacy::Ran(report) => Some(report),
    }
}

fn source_data(source: &SourceRun) -> LegacyData {
    let (outcome, counts) = match &source.outcome {
        SourceOutcome::Completed { report, .. } => ("completed", Some(report)),
        SourceOutcome::Stopped { report, .. } => ("stopped", Some(report)),
        SourceOutcome::TooLarge { .. } => ("too-large", None),
        SourceOutcome::Failed(_) => ("failed", None),
    };
    let count = |of: fn(&redkiln_core::journal::ImportReport) -> u64| counts.map_or(0, of);
    LegacyData {
        kind: match source.kind {
            SourceKind::Ledger => "ledger",
            SourceKind::Journal => "journal",
        },
        path: source.path.to_string_lossy().into_owned(),
        store_id: source.store_id,
        outcome,
        read: count(|r| r.read),
        imported: count(|r| r.imported),
        present: count(|r| r.present),
        malformed: count(|r| r.malformed),
    }
}

fn source_findings(source: &SourceRun) -> Vec<Finding> {
    let path = source.path.display();
    match &source.outcome {
        SourceOutcome::Completed { problems, .. } | SourceOutcome::Stopped { problems, .. } => {
            problems
                .iter()
                .map(|(number, why)| {
                    Finding::new(
                        format!("legacy-malformed:{path}:{number}"),
                        &format!(
                            "{path} {} {number} was skipped as malformed: {why}",
                            place(source.kind)
                        ),
                        Severity::Warning,
                    )
                })
                .collect()
        }
        SourceOutcome::TooLarge { bytes } => vec![Finding::new(
            format!("legacy-unread:{path}"),
            &format!(
                "{path} is {bytes} bytes, more than a 1.x journal read, so it is not imported"
            ),
            Severity::Warning,
        )],
        SourceOutcome::Failed(e) => vec![Finding::new(
            format!("legacy-unread:{path}"),
            &format!("{e:#}"),
            Severity::Warning,
        )],
    }
}
