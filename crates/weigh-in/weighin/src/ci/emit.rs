//! ci-check's output: the `weighin.ci-check/2` envelope with `--json` (SPEC CLI-01 to CLI-07),
//! else a summary line and a line per violation; its findings; and the could-not-run path.
//!
//! Open items are not findings: they are data for the caller (CLI-06), so a report exits 0.

use std::io::{self, StdoutLock, Write};

use weighin_contract::ci_check::v2::{Blocks, COMMAND, Data, Reason, SCHEMA, Violation};

use crate::cli_envelope::{Envelope, Exit, Finding, Severity, output_failed, say};

/// What a report shows.
const PROVES: &str = "every journal ref in this clone is a single-parent chain whose file only grew, and every committed journal keeps at head each complete line it held at base";

/// What a report does not show.
const DOES_NOT_PROVE: &str = "that this clone fetched the journal refs (data.read.refs), that history before a shallow fetch's boundary was kept, or that the open items are all a branch leaves open";

/// `data` as the envelope with `--json`, else as text; 0, or what a failed stdout earns.
pub(super) fn write(data: &Data, json: bool) -> Exit {
    emit(Exit::Success, |out| {
        if json {
            Envelope {
                schema: SCHEMA,
                command: COMMAND,
                data,
                findings: &findings(data),
                exit: Exit::Success,
                proves: Some(PROVES),
                does_not_prove: Some(DOES_NOT_PROVE),
            }
            .write(out)
        } else {
            render_text(data, out)
        }
    })
}

/// ci-check could not run (70): `e` on stderr, and with `--json` the envelope with `data: {}`
/// and one `internal` finding, as `status` writes it.
pub(super) fn could_not_run(json: bool, e: &anyhow::Error) -> Exit {
    let chain = format!("{e:#}");
    let said = say("error", &chain, Exit::Internal);
    if !json {
        return said;
    }
    let findings = [Finding::new("internal".to_owned(), &chain, Severity::Error)];
    emit(Exit::Internal, |out| {
        Envelope {
            schema: SCHEMA,
            command: COMMAND,
            data: &serde_json::Map::new(),
            findings: &findings,
            exit: Exit::Internal,
            proves: None,
            does_not_prove: None,
        }
        .write(out)
    })
}

/// Writes through `write` on stdout and flushes it: `earned`, or what a failed stdout earns
/// (CLI-07).
fn emit(earned: Exit, write: impl FnOnce(&mut StdoutLock<'static>) -> io::Result<()>) -> Exit {
    let mut stdout = io::stdout().lock();
    match write(&mut stdout).and_then(|()| stdout.flush()) {
        Ok(()) => earned,
        Err(e) => output_failed(&e, earned),
    }
}

/// The report's findings: an error per violation, a warning when no journal ref was read, and
/// a warning per weighin type this build could not read.
fn findings(data: &Data) -> Vec<Finding> {
    let mut found: Vec<Finding> = data
        .journal
        .violations
        .iter()
        .map(|v| Finding::new(key(v), &violation_line(v), Severity::Error))
        .collect();
    if data.read.refs.is_empty() {
        found.push(Finding::new(
            "ci-check/no-journal-refs".to_owned(),
            "no journal refs in this clone; fetch +refs/redkiln/journal/*:refs/redkiln/journal/* to read them",
            Severity::Warning,
        ));
    }
    for (event_type, count) in &data.unreadable {
        found.push(Finding::new(
            format!("ci-check/unreadable:{event_type}"),
            &format!("{count} {event_type} event(s) this build cannot read were skipped (EV-10)"),
            Severity::Warning,
        ));
    }
    found
}

/// A violation's stable key: `ci-check/<reason>:<journal>[@<commit>][:<line>]`.
fn key(v: &Violation) -> String {
    let mut key = format!("ci-check/{}:{}", reason_name(v.reason), v.journal);
    if let Some(commit) = &v.commit {
        key.push('@');
        key.push_str(commit);
    }
    if let Some(line) = v.line {
        key.push(':');
        key.push_str(&line.to_string());
    }
    key
}

/// `<journal>[@<short commit>]:<line|-> <reason>`.
fn violation_line(v: &Violation) -> String {
    let at = v
        .commit
        .as_deref()
        .map_or_else(String::new, |commit| format!("@{}", short(commit)));
    let line = v.line.map_or_else(|| "-".to_owned(), |n| n.to_string());
    format!("{}{at}:{line} {}", v.journal, reason_name(v.reason))
}

/// The reason's wire name.
const fn reason_name(reason: Reason) -> &'static str {
    match reason {
        Reason::Removed => "removed",
        Reason::Missing => "missing",
        Reason::Altered => "altered",
        Reason::Undecodable => "undecodable",
        Reason::Merge => "merge",
        Reason::Tree => "tree",
        Reason::Unended => "unended",
        Reason::TooLarge => "too-large",
        Reason::Rewritten => "rewritten",
        Reason::Unreadable => "unreadable",
    }
}

/// The text form: one summary line, then one line per violation.
///
/// # Errors
/// Fails when `out` cannot be written.
fn render_text(data: &Data, out: &mut impl Write) -> io::Result<()> {
    let count = |b: Blocks| data.open_items.iter().filter(|i| i.blocks == b).count();
    let journal = if data.journal.ok {
        "journal ok".to_owned()
    } else {
        format!("journal: {} violation(s)", data.journal.violations.len())
    };
    writeln!(
        out,
        "ci-check {}..{}: {} open ({} block merge, {} block release, {} unknown) · {} decision(s) · {journal} · {} journal ref(s)",
        short(&data.base),
        short(&data.head),
        data.open_items.len(),
        count(Blocks::Merge),
        count(Blocks::Release),
        count(Blocks::Unknown),
        data.decisions.len(),
        data.read.refs.len(),
    )?;
    for v in &data.journal.violations {
        writeln!(out, "  {}", violation_line(v))?;
    }
    Ok(())
}

/// A sha's first seven characters, as git abbreviates it.
fn short(sha: &str) -> &str {
    sha.get(..7).unwrap_or(sha)
}

#[cfg(test)]
mod tests;
