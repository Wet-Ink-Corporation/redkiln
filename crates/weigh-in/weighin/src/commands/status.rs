//! `weighin status [--json]` (SPEC CLI-12): where the store is and how Weigh-In is doing, as the
//! envelope `weighin.status/1` (`weighin_contract::status`) or a few lines.
//!
//! | Exit | When |
//! |---|---|
//! | 0 | the report was written, whatever its health (CLI-06) |
//! | 2 | usage |
//! | 70 | it could not run: no working directory |
//! | 74 | output failed |
//!
//! A closed stdout exits as earned, saying nothing (CLI-07). `status` locates the store as
//! `Env::discover` does, without stopping where a command would (an unlocated or refused store,
//! a refused session and a broken config are what it reports), opens an existing store under
//! `Wait::Command` and never creates one, reads every weighin event once, and never runs the
//! one-time 1.x import (decision D3), marks the hook error log seen or rewrites a heartbeat
//! slot. Opening an existing store may still make Core's own repairs: a stale `-wal` set aside,
//! a store left in WAL switched to TRUNCATE (ST-05), an identity re-minted at a new path
//! (ID-01). `health` comes from the findings alone, so the two cannot disagree.

use std::collections::BTreeMap;
use std::io::{self, StdoutLock, Write};
use std::process::ExitCode;

use redkiln_core::envelope::SPEC_VERSION;
use redkiln_core::location::Source;
use redkiln_core::store::Wait;
use weighin_contract::status::{self as contract, Health, HookRun, HookState, Status, StoreSource};

use crate::Globals;
use crate::cli_envelope::{Envelope, Exit, Finding, Severity, output_failed, say};
use crate::env::{ConfigSource, Env, RepoClass, Vars};
use crate::hooks::{self, State};
use crate::legacy_import;
use crate::store::{self, Store};
use crate::text::Report;

/// The command as the envelope names it.
const COMMAND: &str = "status";

/// Runs `status`; its exit code.
pub(crate) fn run(g: Globals, json: bool) -> ExitCode {
    let exit = match crate::working_dir(g.dir) {
        Ok(cwd) => {
            let session = g.session.map(|s| (s, "--session"));
            let env = Env::discover(&cwd, session, g.store, Vars::from_process());
            write(&gather(&env, hooks::unix_now()), json)
        }
        Err(e) => could_not_run(json, &e),
    };
    ExitCode::from(exit.code())
}

/// What `status` found.
struct Found {
    data: Status,
    findings: Vec<Finding>,
}

/// What reading the store gave.
#[derive(Default)]
struct StoreRead {
    schema: Option<String>,
    unreadable: BTreeMap<String, u64>,
}

/// The report for `env`, with hook slots judged at `now` (seconds since 1970; `None` when the
/// clock reads before it, which leaves each slot as recorded).
fn gather(env: &Env, now: Option<u64>) -> Found {
    let mut findings = Vec::new();
    let store = read_store(env, &mut findings);
    environment_findings(env, &mut findings);
    let hook_errors = hook_errors(env, &mut findings);
    let hooks = hook_runs(env, now, &mut findings);
    notices(env, &mut findings);
    let (path, source) = match &env.store {
        Ok(at) => (
            Some(at.path.to_string_lossy().into_owned()),
            Some(store_source(at.source)),
        ),
        Err(_) => (None, None),
    };
    let data = Status {
        plugin: contract::PLUGIN.to_owned(),
        version: env!("CARGO_PKG_VERSION").to_owned(),
        spec: SPEC_VERSION.to_owned(),
        store: path,
        store_source: source,
        store_schema: store.schema,
        repo_class: env.repo_class().into(),
        unreadable: store.unreadable,
        // weighin records no telemetry (D9 of #17 PR 5), so it drops none.
        dropped: 0,
        health: health(&findings),
        hook_errors,
        hooks,
    };
    Found { data, findings }
}

/// The store's schema and unreadable events, each failure a finding; then whether 1.x sources
/// wait for the import.
fn read_store(env: &Env, findings: &mut Vec<Finding>) -> StoreRead {
    let path = match &env.store {
        Ok(at) => &at.path,
        Err(e) => {
            let message = format!("locating the store: {}", Report(e));
            findings.push(Finding::new(
                "store/unlocated".to_owned(),
                &message,
                Severity::Error,
            ));
            return StoreRead::default();
        }
    };
    let store = match Store::open_existing(path, Wait::Command) {
        Ok(store) => store,
        Err(e) => {
            // Core's refusals (ST-07): an old ledger, another program's file, a newer major.
            let key = if store::refusal(&e).is_some() {
                "store/refused"
            } else {
                "store/unreadable"
            };
            findings.push(Finding::new(
                key.to_owned(),
                &format!("{e:#}"),
                Severity::Error,
            ));
            return StoreRead::default();
        }
    };
    let mut read = StoreRead::default();
    if let Some(store) = &store {
        read.schema = Some(store.core().schema().to_string());
        match store.read_all() {
            Ok(events) => {
                let skipped = events.unusable.total();
                if skipped > 0 {
                    let message = format!(
                        "{skipped} event(s) in the store are of a form this weighin cannot read, \
                         so they are skipped; a newer weighin wrote them"
                    );
                    let key = "events/unreadable".to_owned();
                    findings.push(Finding::new(key, &message, Severity::Warning));
                }
                read.unreadable = events.unusable.into_by_type();
            }
            Err(e) => {
                let key = "store/unreadable".to_owned();
                findings.push(Finding::new(key, &format!("{e:#}"), Severity::Error));
            }
        }
    }
    legacy_due(env, store.as_ref(), findings);
    read
}

/// A note when 1.x sources wait for the one-time import, which `status` never runs (D3).
fn legacy_due(env: &Env, store: Option<&Store>, findings: &mut Vec<Finding>) {
    let key = "legacy/due".to_owned();
    match legacy_import::due(env, store) {
        Ok(due) if due.ledgers == 0 && due.journals == 0 => {}
        Ok(due) => {
            let message = format!(
                "{} weighin 1.x ledger(s) and {} 1.x journal(s) wait for the one-time import, \
                 which the next weighin command or session start runs",
                due.ledgers, due.journals
            );
            findings.push(Finding::new(key, &message, Severity::Note));
        }
        Err(e) => {
            let message = format!("whether 1.x sources wait for the import is unknown: {e:#}");
            findings.push(Finding::new(key, &message, Severity::Warning));
        }
    }
}

/// A broken config and a refused session: a command would stop on either.
fn environment_findings(env: &Env, findings: &mut Vec<Finding>) {
    if let ConfigSource::Broken(e) = &env.config_source {
        let message = format!(
            "this folder's config cannot be used, so commands stop and hooks run on the \
             defaults: {}",
            Report(e)
        );
        findings.push(Finding::new(
            "config/broken".to_owned(),
            &message,
            Severity::Warning,
        ));
    }
    if let Some(e) = &env.session_error {
        findings.push(Finding::new(
            "session/refused".to_owned(),
            e,
            Severity::Warning,
        ));
    }
}

/// What locating the store took or found that the user should hear about.
fn notices(env: &Env, findings: &mut Vec<Finding>) {
    for (n, notice) in (1_u32..).zip(&env.notices) {
        let key = format!("store/notice:{n}");
        findings.push(Finding::new(key, &notice.to_string(), Severity::Note));
    }
}

/// The lines of the hook error log the next `SessionStart` reports, read only.
fn hook_errors(env: &Env, findings: &mut Vec<Finding>) -> u64 {
    let key = "hooks/errors".to_owned();
    match hooks::unreported(env) {
        Ok(None) => 0,
        Ok(Some(found)) => {
            let message = format!(
                "{} hook error(s) since the last session start, so capture may be off. Last: {}",
                found.count(),
                found.last
            );
            findings.push(Finding::new(key, &message, Severity::Warning));
            u64::try_from(found.lines).unwrap_or(u64::MAX)
        }
        Err(e) => {
            let message = format!("the hook error log cannot be read: {e:#}");
            findings.push(Finding::new(key, &message, Severity::Warning));
            0
        }
    }
}

/// Each hook event's last run from its heartbeat slot, judged at `now` as the next hook would;
/// a death no hook has logged yet is a finding.
fn hook_runs(
    env: &Env,
    now: Option<u64>,
    findings: &mut Vec<Finding>,
) -> BTreeMap<String, HookRun> {
    let mut runs = BTreeMap::new();
    for (event, slot) in hooks::slots(env) {
        let slot = match slot {
            Ok(slot) => slot,
            Err(e) => {
                let key = format!("hooks/heartbeat:{event}");
                findings.push(Finding::new(key, &format!("{e:#}"), Severity::Warning));
                continue;
            }
        };
        if let Some(line) = now.and_then(|now| slot.lost_report(now)) {
            let key = format!("hooks/lost:{event}");
            let message = format!("{event}: {line}");
            findings.push(Finding::new(key, &message, Severity::Warning));
        }
        let state = now.map_or(slot.state, |now| slot.state_at(now));
        let run = HookRun {
            state: state.into(),
            at: slot.at(),
        };
        runs.insert(event.to_owned(), run);
    }
    runs
}

/// `off` with an error finding, `degraded` with a warning, else `ok`: from the findings alone.
fn health(findings: &[Finding]) -> Health {
    let has = |severity| findings.iter().any(|f| f.severity == Some(severity));
    if has(Severity::Error) {
        Health::Off
    } else if has(Severity::Warning) {
        Health::Degraded
    } else {
        Health::Ok
    }
}

/// ST-03's step as the contract spells it.
const fn store_source(source: Source) -> StoreSource {
    match source {
        Source::Flag => StoreSource::Flag,
        Source::Env => StoreSource::Env,
        Source::Config => StoreSource::Config,
        Source::GitCommonDir => StoreSource::GitCommonDir,
        Source::Folder => StoreSource::Folder,
    }
}

impl From<RepoClass> for contract::RepoClass {
    fn from(class: RepoClass) -> Self {
        match class {
            RepoClass::Own => Self::Own,
            RepoClass::Client => Self::Client,
        }
    }
}

impl From<State> for HookState {
    fn from(state: State) -> Self {
        match state {
            State::Running => Self::Running,
            State::Finished => Self::Finished,
            State::TimedOut => Self::TimedOut,
            State::Interrupted => Self::Interrupted,
        }
    }
}

/// `found` as the envelope with `--json`, else as lines; its exit.
fn write(found: &Found, json: bool) -> Exit {
    emit(Exit::Success, |out| {
        if json {
            Envelope {
                schema: contract::SCHEMA,
                command: COMMAND,
                data: &found.data,
                findings: &found.findings,
                exit: Exit::Success,
                proves: None,
                does_not_prove: None,
            }
            .write(out)
        } else {
            write_lines(out, found)
        }
    })
}

/// `Weigh-In <version> (rk-weigh-in, SPEC <spec>): <health>`, the store's line, then one line
/// per finding.
fn write_lines(out: &mut impl Write, found: &Found) -> io::Result<()> {
    let data = &found.data;
    let health = match data.health {
        Health::Ok => "ok",
        Health::Degraded => "degraded",
        Health::Off => "off",
    };
    writeln!(
        out,
        "Weigh-In {} ({}, SPEC {}): {health}",
        data.version, data.plugin, data.spec
    )?;
    let class = match data.repo_class {
        contract::RepoClass::Own => "own",
        contract::RepoClass::Client => "client",
    };
    match (&data.store, data.store_source) {
        (Some(path), Some(source)) => {
            let schema = data
                .store_schema
                .as_deref()
                .map_or_else(|| "no store yet".to_owned(), |s| format!("schema {s}"));
            let source = source_name(source);
            writeln!(out, "store: {path} ({source}, {schema}), {class}")?;
        }
        _ => writeln!(out, "store: not located, {class}")?,
    }
    for finding in &found.findings {
        let level = match finding.severity {
            Some(Severity::Error) => "error",
            Some(Severity::Warning) => "warning",
            Some(Severity::Note) | None => "note",
        };
        writeln!(out, "{level}: {}", finding.message)?;
    }
    Ok(())
}

/// ST-03's name for the step.
const fn source_name(source: StoreSource) -> &'static str {
    match source {
        StoreSource::Flag => "flag",
        StoreSource::Env => "env",
        StoreSource::Config => "config",
        StoreSource::GitCommonDir => "git_common_dir",
        StoreSource::Folder => "folder",
    }
}

/// The command could not run (70): `e` on stderr, and with `--json` the envelope with
/// `data: {}` and one `internal` finding, as the journal commands write it.
fn could_not_run(json: bool, e: &anyhow::Error) -> Exit {
    let chain = format!("{e:#}");
    let said = say("error", &chain, Exit::Internal);
    if !json {
        return said;
    }
    let findings = [Finding::new("internal".to_owned(), &chain, Severity::Error)];
    emit(Exit::Internal, |out| {
        Envelope {
            schema: contract::SCHEMA,
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

/// Writes through `write` on stdout and flushes it: `earned`, or what a failed stdout earns.
fn emit(earned: Exit, write: impl FnOnce(&mut StdoutLock<'static>) -> io::Result<()>) -> Exit {
    let mut stdout = io::stdout().lock();
    match write(&mut stdout).and_then(|()| stdout.flush()) {
        Ok(()) => earned,
        Err(e) => output_failed(&e, earned),
    }
}

#[cfg(test)]
mod tests {
    use super::health;
    use crate::cli_envelope::{Finding, Severity};
    use weighin_contract::status::Health;

    fn finding(severity: Severity) -> Finding {
        Finding::new("k".to_owned(), "m", severity)
    }

    #[test]
    fn health_is_off_with_an_error_degraded_with_a_warning_else_ok() {
        let cases = [
            (vec![], Health::Ok),
            (vec![finding(Severity::Note)], Health::Ok),
            (vec![finding(Severity::Warning)], Health::Degraded),
            (
                vec![finding(Severity::Note), finding(Severity::Warning)],
                Health::Degraded,
            ),
            (vec![finding(Severity::Error)], Health::Off),
            (
                vec![finding(Severity::Warning), finding(Severity::Error)],
                Health::Off,
            ),
        ];
        for (findings, expected) in cases {
            assert_eq!(health(&findings), expected, "{findings:?}");
        }
    }
}
