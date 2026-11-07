//! `weighin journal checkpoint|import|relay|pre-push` (SPEC JR-07 to JR-13): arguments to the
//! call in `journal::refs`, its outcome to the CLI envelope (`--json`) or one summary line, and
//! SPEC CLI-05's exit codes.
//!
//! | Command | 0 | 1 | 2 | 70 | 74 |
//! |---|---|---|---|---|---|
//! | `checkpoint` | report written, skips, broken refs and deferrals included | none | usage | store or local git failed | output failed |
//! | `import` | report written, rewritten refs and failed fetches included | none | usage | store, local git or an import stopped | output failed |
//! | `relay <branch>` | gone, or merged with the branch deleted or a newer commit on it | the inbox was left in place | usage, or a branch that is no inbox | local git failed | output failed |
//! | `pre-push` | always, every failure included (said on stderr) | none | usage | none | none |
//!
//! Errors and warnings go to stderr as `weighin: error: <chain>` and `weighin: warning: <chain>`,
//! each on one line (CLI-08). When stdout's reader has gone, a success exits 0 without a word
//! and a failure keeps its CLI-05 code (CLI-07 read with CLI-05); stderr is a separate stream,
//! so a failure's error, `--json` or not, and the store's notices are still said there.

use std::fmt::Display;
use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::time::Instant;

use redkiln_core::journal::JournalName;
use redkiln_core::journal::refs::{Deleted, RefsError, RefsExported, RefsImport, Relayed};
use serde::Serialize;

use crate::cli_envelope::{Envelope, Exit, Finding, Severity, one_line};
use crate::env::Env;
use crate::journal::pre_push::{self, Pushing};
use crate::journal::refs::{self, Budget, Checkpointed, Imported, Marked, Scope, Skip};
use crate::{Globals, JournalCmd, RelayArgs};

/// A command's envelope ids (CLI-02).
#[derive(Debug, Clone, Copy)]
struct Named {
    schema: &'static str,
    command: &'static str,
}

const CHECKPOINT: Named = Named {
    schema: "weighin.journal-checkpoint/1",
    command: "journal checkpoint",
};
const IMPORT: Named = Named {
    schema: "weighin.journal-import/1",
    command: "journal import",
};
const RELAY: Named = Named {
    schema: "weighin.journal-relay/1",
    command: "journal relay",
};
const PRE_PUSH: Named = Named {
    schema: "weighin.journal-pre-push/1",
    command: "journal pre-push",
};

/// Run one journal subcommand; its exit code.
pub(crate) fn run(g: Globals, cmd: JournalCmd) -> ExitCode {
    // Every budget is measured from the command's start.
    let started = Instant::now();
    let exit = match cmd {
        JournalCmd::Checkpoint(a) => checkpoint(g, a.json, started),
        JournalCmd::Import(a) => import(g, a.json, started),
        JournalCmd::Relay(a) => relay(g.dir, &a),
        JournalCmd::PrePush(a) => match pre_push(g, a.json, started) {
            // git aborts a push on any non-zero exit, so whatever the hook could not do or
            // write, already said on stderr where it could, the push goes on.
            Exit::Success | Exit::LeftInPlace | Exit::Usage | Exit::Internal | Exit::Output => {
                Exit::Success
            }
        },
    };
    ExitCode::from(exit.code())
}

fn checkpoint(g: Globals, json: bool, started: Instant) -> Exit {
    let env = match crate::environment(g) {
        Ok(env) => env,
        Err(e) => return internal(CHECKPOINT, json, &e),
    };
    let noticed = warn_notices(&env);
    let exit = match refs::checkpoint(&env, Budget::COMMAND, started, Scope::All) {
        Ok(done) => emit(CHECKPOINT, json, &checkpointed(&done)),
        Err(e) => internal(CHECKPOINT, json, &e),
    };
    settle(json, noticed, exit)
}

fn import(g: Globals, json: bool, started: Instant) -> Exit {
    let env = match crate::environment(g) {
        Ok(env) => env,
        Err(e) => return internal(IMPORT, json, &e),
    };
    let noticed = warn_notices(&env);
    let exit = match refs::import(&env, Budget::COMMAND, started, "cli") {
        Ok(done) => emit(IMPORT, json, &imported(&done)),
        Err(e) => internal(IMPORT, json, &e),
    };
    settle(json, noticed, exit)
}

fn relay(dir: Option<PathBuf>, args: &RelayArgs) -> Exit {
    let dir = match crate::working_dir(dir) {
        Ok(dir) => dir,
        Err(e) => return internal(RELAY, args.json, &e),
    };
    match refs::relay(&dir, &args.branch, Budget::RELAY) {
        Ok(outcome) => emit(RELAY, args.json, &relayed(&args.branch, &outcome)),
        Err(e @ RefsError::NotInbox(_)) => say("error", &e, Exit::Usage),
        Err(e) => {
            let e = anyhow::Error::new(e).context(format!("relaying {}", args.branch));
            internal(RELAY, args.json, &e)
        }
    }
}

/// The checkpoint unless only journal refs are pushed. Every failure is a warning: the exit
/// it would earn elsewhere is returned for `run` to set aside.
fn pre_push(g: Globals, json: bool, started: Instant) -> Exit {
    let read = match pre_push::pushing(io::stdin().lock()) {
        // A checkpoint's own push: nothing to export, so no environment is even looked for.
        Ok(Pushing::OnlyJournals) => {
            let skipped = Checkpointed::Skipped(Skip::JournalOnlyPush);
            return emit(PRE_PUSH, json, &checkpointed(&skipped));
        }
        Ok(Pushing::Other) => Exit::Success,
        Err(e) => {
            let msg = format_args!("reading git's pre-push input: {e}; exporting anyway");
            say("warning", &msg, Exit::Success)
        }
    };
    let env = match crate::environment(g) {
        Ok(env) => env,
        Err(e) => return pre_push_failed(json, &e),
    };
    let noticed = worst(read, warn_notices(&env));
    let done = match refs::checkpoint(&env, Budget::PRE_PUSH, started, Scope::All) {
        Ok(done) => done,
        Err(e) => return pre_push_failed(json, &e),
    };
    settle(json, noticed, emit(PRE_PUSH, json, &checkpointed(&done)))
}

/// `pre-push` could not export: `e` as a warning, and with `--json` the envelope with the
/// checkpoint's data empty (its schema's shape) and one `internal` finding.
fn pre_push_failed(json: bool, e: &anyhow::Error) -> Exit {
    let failure = Failure {
        exit: Exit::Success,
        data: CheckpointData::empty(None),
    };
    failed(PRE_PUSH, json, e, failure)
}

/// One command's result, as the envelope and the summary line say it.
struct Outcome<D> {
    data: D,
    findings: Vec<Finding>,
    exit: Exit,
    /// The one line written without `--json`.
    line: String,
}

/// `outcome` as the envelope with `--json`, else its line on stdout and its findings on stderr;
/// the exit code it earned.
fn emit<D: Serialize>(named: Named, json: bool, outcome: &Outcome<D>) -> Exit {
    let mut stdout = io::stdout().lock();
    let written = if json {
        Envelope {
            schema: named.schema,
            command: named.command,
            data: &outcome.data,
            findings: &outcome.findings,
            exit: outcome.exit,
        }
        .write(&mut stdout)
    } else {
        writeln!(stdout, "{}", outcome.line)
    };
    if let Err(e) = written.and_then(|()| stdout.flush()) {
        return output_failed(&e, outcome.exit);
    }
    if json {
        return outcome.exit;
    }
    outcome.findings.iter().fold(outcome.exit, |exit, finding| {
        say(level(finding.severity), &finding.message, exit)
    })
}

/// How a command that could not run ends: its exit, and the `data` its envelope carries.
struct Failure<D> {
    exit: Exit,
    data: D,
}

/// The command could not run (70): `e` on stderr, and with `--json` the envelope with
/// `data: {}` and one `internal` finding.
fn internal(named: Named, json: bool, e: &anyhow::Error) -> Exit {
    let failure = Failure {
        exit: Exit::Internal,
        data: serde_json::Map::new(),
    };
    failed(named, json, e, failure)
}

/// `e` on stderr at `error` (or `warning` when the failure's exit is success, as for
/// `pre-push`), and with `--json` the envelope with its `data` and one `internal` finding.
fn failed<D: Serialize>(named: Named, json: bool, e: &anyhow::Error, failure: Failure<D>) -> Exit {
    let Failure { exit, data } = failure;
    let chain = format!("{e:#}");
    let said = say(if exit.ok() { "warning" } else { "error" }, &chain, exit);
    if !json {
        return said;
    }
    let outcome = Outcome {
        data,
        findings: vec![Finding::new("internal".to_owned(), &chain, Severity::Error)],
        exit,
        line: String::new(),
    };
    settle(json, said, emit(named, json, &outcome))
}

/// The exit after stdout failed with `e`: when its reader stopped reading, the exit the command
/// earned, said nowhere (CLI-07: a success stays 0, a failure keeps its code, CLI-05); else 74,
/// said on stderr.
fn output_failed(e: &io::Error, earned: Exit) -> Exit {
    if e.kind() == io::ErrorKind::BrokenPipe {
        return earned;
    }
    say(
        "error",
        &format_args!("writing standard output: {e}"),
        Exit::Output,
    )
}

/// `weighin: <level>: <message>` on stderr, the message on one line (CLI-08); `exit`, or 74 for
/// a success that cannot be said.
fn say(level: &str, message: &dyn Display, exit: Exit) -> Exit {
    let message = one_line(&message.to_string());
    match writeln!(io::stderr().lock(), "weighin: {level}: {message}") {
        Ok(()) => exit,
        // A failure keeps its own code: with stderr gone, the code is the only report left.
        Err(_) if !exit.ok() => exit,
        Err(_) => Exit::Output,
    }
}

/// `second`, unless it succeeded and `first` did not.
const fn worst(first: Exit, second: Exit) -> Exit {
    if second.ok() { first } else { second }
}

/// The exit once the result is out: `emitted`, what writing it earned, after warnings that
/// earned `said` on stderr before it. With `--json` the envelope's `ok` was written from
/// `emitted`, so a warning stderr could not take leaves the exit as the envelope says (CLI-02);
/// without it, that failure is an output error (74, CLI-05).
const fn settle(json: bool, said: Exit, emitted: Exit) -> Exit {
    if json { emitted } else { worst(said, emitted) }
}

/// The stderr level of a finding: CLI-08 knows only errors and warnings.
const fn level(severity: Option<Severity>) -> &'static str {
    match severity {
        Some(Severity::Error) => "error",
        Some(Severity::Warning | Severity::Note) | None => "warning",
    }
}

/// What locating the store found (an old ledger, a fallback), as warnings.
fn warn_notices(env: &Env) -> Exit {
    env.notices
        .iter()
        .fold(Exit::Success, |exit, notice| say("warning", notice, exit))
}

/// `data` of `weighin.journal-checkpoint/1` and `weighin.journal-pre-push/1`.
#[derive(Serialize)]
struct CheckpointData<'a> {
    skipped: Option<Skip>,
    appended: u64,
    direct: Vec<&'a str>,
    relayed: Vec<&'a str>,
    local_only: bool,
    unexportable: u64,
    relay_reason: Option<&'a str>,
}

impl CheckpointData<'_> {
    /// Nothing exported: skipped for `skipped`, or (`None`) a checkpoint that could not run.
    const fn empty(skipped: Option<Skip>) -> Self {
        Self {
            skipped,
            appended: 0,
            direct: Vec::new(),
            relayed: Vec::new(),
            local_only: false,
            unexportable: 0,
            relay_reason: None,
        }
    }
}

fn checkpointed(done: &Checkpointed) -> Outcome<CheckpointData<'_>> {
    let (report, marked) = match done {
        Checkpointed::Skipped(skip) => {
            return Outcome {
                data: CheckpointData::empty(Some(*skip)),
                findings: Vec::new(),
                exit: Exit::Success,
                line: format!("nothing to export ({})", skip.as_str()),
            };
        }
        Checkpointed::Ran(report, marked) => (report, marked),
    };
    let direct: Vec<&str> = report.direct.iter().map(JournalName::as_str).collect();
    let relayed: Vec<&str> = report.relayed.iter().map(JournalName::as_str).collect();
    let line = export_line(report, &direct, &relayed);
    let mut findings = refs::checkpoint_findings(report);
    if let Marked::Failed(e) = marked {
        // The export stands; only the Stop hook's next scope widens to every journal.
        findings.push(Finding::new(
            "journal/watermark".to_owned(),
            &format!("{e:#}"),
            Severity::Warning,
        ));
    }
    Outcome {
        data: CheckpointData {
            skipped: None,
            appended: report.appended,
            direct,
            relayed,
            local_only: report.local_only,
            unexportable: report.unexportable,
            relay_reason: report.relay_reason.as_deref(),
        },
        findings,
        exit: Exit::Success,
        line,
    }
}

/// `exported 3 line(s): pushed s1, s2; inbox s3`.
fn export_line(report: &RefsExported, direct: &[&str], relayed: &[&str]) -> String {
    let mut parts = Vec::new();
    if !direct.is_empty() {
        let how = if report.local_only {
            "moved locally"
        } else {
            "pushed"
        };
        parts.push(format!("{how} {}", direct.join(", ")));
    }
    if !relayed.is_empty() {
        parts.push(format!("inbox {}", relayed.join(", ")));
    }
    let mut line = format!("exported {} line(s)", report.appended);
    if !parts.is_empty() {
        line.push_str(": ");
        line.push_str(&parts.join("; "));
    }
    line
}

/// `data` of `weighin.journal-import/1`.
#[derive(Serialize)]
struct ImportData<'a> {
    skipped: Option<Skip>,
    sources: Vec<SourceData<'a>>,
    late: bool,
}

/// One source an import read.
#[derive(Serialize)]
struct SourceData<'a> {
    source: &'a str,
    commit: &'a str,
    read: u64,
    imported: u64,
    present: u64,
    malformed: u64,
    incomplete: bool,
}

fn imported(done: &Imported) -> Outcome<ImportData<'_>> {
    let report: &RefsImport = match done {
        Imported::Skipped(skip) => {
            return Outcome {
                data: ImportData {
                    skipped: Some(*skip),
                    sources: Vec::new(),
                    late: false,
                },
                findings: Vec::new(),
                exit: Exit::Success,
                line: format!("nothing to import ({})", skip.as_str()),
            };
        }
        Imported::Ran(report) => report,
    };
    let sources: Vec<SourceData<'_>> = report
        .sources
        .iter()
        .map(|read| SourceData {
            source: &read.source,
            commit: &read.commit,
            read: read.report.read,
            imported: read.report.imported,
            present: read.report.present,
            malformed: read.report.malformed,
            incomplete: read.report.incomplete,
        })
        .collect();
    let events = sources
        .iter()
        .fold(0_u64, |sum, s| sum.saturating_add(s.imported));
    let mut line = format!(
        "imported {events} event(s) from {} source(s)",
        sources.len()
    );
    if report.late {
        line.push_str("; the deadline left the rest for later");
    }
    Outcome {
        data: ImportData {
            skipped: None,
            sources,
            late: report.late,
        },
        findings: refs::import_findings(report),
        exit: Exit::Success,
        line,
    }
}

/// `data` of `weighin.journal-relay/1`.
#[derive(Serialize)]
struct RelayData<'a> {
    branch: &'a str,
    outcome: &'static str,
    added: Option<u64>,
    deleted: Option<&'static str>,
}

fn relayed<'a>(branch: &'a str, outcome: &Relayed) -> Outcome<RelayData<'a>> {
    let (exit, findings) = refs::relay_findings(branch, outcome);
    let (name, merged) = match outcome {
        Relayed::Gone => ("gone", None),
        Relayed::Refused(_) => ("refused", None),
        Relayed::Broken(_) => ("broken", None),
        Relayed::TooLarge => ("too-large", None),
        Relayed::RetriesSpent => ("retries-spent", None),
        Relayed::Failed(_) => ("failed", None),
        Relayed::Merged { added, deleted } => ("merged", Some((*added, deleted))),
    };
    let line = match merged {
        Some((added, Deleted::Yes)) => format!("relayed {branch}: {added} line(s), branch deleted"),
        Some((added, Deleted::NewerArrived)) => {
            format!("relayed {branch}: {added} line(s), branch kept for a newer commit")
        }
        None if exit.ok() => format!("{branch} is gone: nothing to relay"),
        Some((_, Deleted::Failed(_))) | None => format!("left {branch} in place"),
    };
    Outcome {
        data: RelayData {
            branch,
            outcome: name,
            added: merged.map(|(added, _)| added),
            deleted: merged.map(|(_, deleted)| match deleted {
                Deleted::Yes => "yes",
                Deleted::NewerArrived => "newer-arrived",
                Deleted::Failed(_) => "failed",
            }),
        },
        findings,
        exit,
        line,
    }
}
