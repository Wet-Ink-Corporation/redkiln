//! Journal refs from weighin (SPEC JR-07 to JR-13): the store's durable events exported to
//! `refs/redkiln/journal/<name>` (or the inbox branch where refs are refused), journal sources
//! imported back, and one inbox branch relayed, all through `redkiln_core::journal::refs`.
//!
//! The export reads every plugin's durable events, so it is a suite capability. weighin is the
//! only binary over the store today; a second one makes it a kit (an H-16 call then).
//!
//! A network failure is an outcome, not an error (JR-07): it comes back in Core's report, and
//! the findings here describe it. Errors are kept for the store and local git.

use std::path::Path;
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context as _, Result};
use redkiln_core::envelope::Timestamp;
use redkiln_core::journal::JournalName;
use redkiln_core::journal::refs::{
    Breach, Deferral, Deleted, JOURNAL_REFS, MAX_CALLS_PAST_DEADLINE, RefsCheckpoint, RefsError,
    RefsExported, RefsImport, RefsTarget, Refusal, Relayed, checkpoint_refs_since, import_refs,
};
use redkiln_core::location::GIT_TIMEOUT;
use serde::{Serialize, Serializer};

use super::watermark::Watermark;
use crate::cli_envelope::{Exit, Finding, Severity};
use crate::env::{Env, RepoClass};
use crate::store::Store;
use crate::text::clip;

/// The remote journal refs go to and come from (JR-07). Another remote is a later `suite.toml`
/// setting.
const REMOTE: &str = "origin";

/// The most characters of what git said that a finding keeps.
const MAX_SAID_CHARS: usize = 200;

/// The git calls, local or remote, Core can still make after the deadline passes.
const CALLS_AFTER_DEADLINE: u32 = MAX_CALLS_PAST_DEADLINE;

/// How long a caller lets journal-ref work run (HK-5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Budget {
    /// After `started + start_by`, no journal or source is started; `None` sets no deadline.
    pub(crate) start_by: Option<Duration>,
    /// The most one git call may take before Core kills it.
    pub(crate) per_call: Duration,
}

impl Budget {
    /// The Stop hook, inside its 20 s timeout in `hooks.json` (D3): worst case 4 s + 7 × 2 s =
    /// 18 s, leaving time for the output after it.
    pub(crate) const STOP: Self = Self {
        start_by: Some(Duration::from_secs(4)),
        per_call: Duration::from_secs(2),
    };
    /// `journal pre-push`: git waits for it, so it is bounded, if less tightly than a hook:
    /// worst case 6 s + 7 × 2 s = 20 s.
    pub(crate) const PRE_PUSH: Self = Self {
        start_by: Some(Duration::from_secs(6)),
        per_call: Duration::from_secs(2),
    };
    /// `journal checkpoint` and `journal import` run by hand: no deadline, every call bounded.
    pub(crate) const COMMAND: Self = Self {
        start_by: None,
        per_call: GIT_TIMEOUT,
    };
    /// `journal relay`, run by CI under its job timeout: slow pushes are given time.
    pub(crate) const RELAY: Self = Self {
        start_by: None,
        per_call: Duration::from_secs(60),
    };

    /// The Core target for `repo`. A deadline `Instant` cannot represent is `started` itself,
    /// which defers everything rather than waiting without a bound (H-03, H-13).
    fn target(self, repo: &Path, started: Instant) -> RefsTarget<'_> {
        RefsTarget {
            repo,
            remote: REMOTE,
            timeout: self.per_call,
            deadline: self
                .start_by
                .map(|by| started.checked_add(by).unwrap_or(started)),
        }
    }

    /// The longest a checkpoint can run after it started: `start_by` plus every call Core can
    /// still make after the deadline; `None` with no deadline, or on overflow. The Stop hook
    /// logs a checkpoint that ran longer, which means Core's call pattern grew (plan risk 4).
    pub(crate) fn worst_case(self) -> Option<Duration> {
        self.start_by?
            .checked_add(self.per_call.checked_mul(CALLS_AFTER_DEADLINE)?)
    }
}

/// Why journal-ref work did nothing here. None of these is a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Skip {
    /// Not in a git worktree.
    NotGit,
    /// A `client` repository: its events never leave the machine (JR-09, ST-13).
    Client,
    /// The config sets `"journal": false`.
    OptedOut,
    /// No store exists, so there is nothing to export, and none is created.
    NoStore,
    /// Every ref the push sends is a journal ref or an inbox branch (JR-08).
    JournalOnlyPush,
}

impl Skip {
    /// The reason as the envelope and the text output spell it.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::NotGit => "not-git",
            Self::Client => "client",
            Self::OptedOut => "opted-out",
            Self::NoStore => "no-store",
            Self::JournalOnlyPush => "journal-only-push",
        }
    }
}

impl Serialize for Skip {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

/// Which journals [`checkpoint`] examines.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Scope {
    /// Every journal: `journal checkpoint` and `journal pre-push`, the sweep.
    All,
    /// Only the journals with an event after the watermark: the Stop hook.
    SinceWatermark,
}

/// Whether a checkpoint moved the watermark.
#[derive(Debug)]
pub(crate) enum Marked {
    /// To the position up to which the store is settled (Core's `through`).
    Moved,
    /// Already there: nothing more is settled, so the next checkpoint looks again.
    Kept,
    /// It could not be written; the next Stop examines every journal again.
    Failed(anyhow::Error),
}

/// What [`checkpoint`] did.
#[derive(Debug)]
#[must_use]
pub(crate) enum Checkpointed {
    Skipped(Skip),
    Ran(RefsExported, Marked),
}

/// What [`import`] did.
#[derive(Debug)]
#[must_use]
pub(crate) enum Imported {
    Skipped(Skip),
    Ran(RefsImport),
}

/// Export the store's durable events that their journals lack (JR-08), from the journals
/// `scope` names. Skips, in this order: not in git, a `client` repository, opted out, then no
/// store (none is created). Every run, whatever its scope, moves the watermark to the position
/// up to which it settled the store: every journal whose last event is at or before it was
/// examined and neither deferred nor broken.
///
/// # Errors
/// The store cannot be located, opened or read, or git fails locally.
pub(crate) fn checkpoint(
    env: &Env,
    budget: Budget,
    started: Instant,
    scope: Scope,
) -> Result<Checkpointed> {
    let Some(repo) = env.repo_root.as_deref() else {
        return Ok(Checkpointed::Skipped(Skip::NotGit));
    };
    let class = env.repo_class();
    if class == RepoClass::Client {
        return Ok(Checkpointed::Skipped(Skip::Client));
    }
    if !env.config.journal {
        return Ok(Checkpointed::Skipped(Skip::OptedOut));
    }
    // Every error here says what was being done: the Stop hook logs it as it is.
    let opened = env
        .store_path()
        .and_then(|path| Store::open_existing(path, env.wait))
        .context("exporting the journals")?;
    let Some(store) = opened else {
        return Ok(Checkpointed::Skipped(Skip::NoStore));
    };
    let target = budget.target(repo, started);
    let watermark = Watermark::of(env, repo, store.core().store_id())?;
    let recorded = watermark.read();
    let since = match scope {
        Scope::All => 0,
        Scope::SinceWatermark => recorded,
    };
    let export = |since| {
        checkpoint_refs_since(store.core(), class.into(), &target, since)
            .context("exporting the journals")
    };
    let mut scoped = export(since)?;
    if scoped.through < since {
        // A store behind its watermark is not the one it was written for (a store recreated
        // since): nothing it holds is known to be exported.
        scoped = export(0)?;
    }
    Ok(match scoped.outcome {
        RefsCheckpoint::Client => Checkpointed::Skipped(Skip::Client),
        RefsCheckpoint::Written(report) => {
            // Core's `through` already stops before the first journal deferred or broken, so a
            // run cut short still records the journals it settled, and a sweep that found a
            // broken ref may move it back.
            let marked = if scoped.through == recorded {
                Marked::Kept
            } else {
                watermark
                    .write(scoped.through)
                    .map_or_else(Marked::Failed, |()| Marked::Moved)
            };
            Checkpointed::Ran(report, marked)
        }
    })
}

/// Import every journal source not yet imported at its commit (JR-10 to JR-13), recorded as
/// written now by `actor` (`hook` or `cli`). Skips outside git and in a `client` repository;
/// otherwise opens the store, creating it in an `own` repository (D10).
///
/// # Errors
/// As [`checkpoint`], plus an import that stopped.
pub(crate) fn import(env: &Env, budget: Budget, started: Instant, actor: &str) -> Result<Imported> {
    let Some(repo) = env.repo_root.as_deref() else {
        return Ok(Imported::Skipped(Skip::NotGit));
    };
    if env.repo_class() == RepoClass::Client {
        return Ok(Imported::Skipped(Skip::Client));
    }
    let store = Store::open(env.store_path()?, env.wait)?;
    let ts = Timestamp::from_system_time(SystemTime::now()).context("the import's time")?;
    let author = store.author(actor)?;
    let report = import_refs(store.core(), &budget.target(repo, started), &ts, &author)
        .context("importing the journal refs")?;
    Ok(Imported::Ran(report))
}

/// Relay `branch` of [`REMOTE`] in the repository at `dir` (JR-07). Opens no store.
///
/// # Errors
/// [`RefsError::NotInbox`] when `branch` names no inbox, and [`RefsError`] when git fails
/// locally.
pub(crate) fn relay(dir: &Path, branch: &str, budget: Budget) -> Result<Relayed, RefsError> {
    redkiln_core::journal::refs::relay(&budget.target(dir, Instant::now()), branch)
}

/// A breach as findings and text name it.
const fn breach_name(breach: Breach) -> &'static str {
    match breach {
        Breach::Merge => "merge",
        Breach::Tree => "tree",
        Breach::Unended => "unended",
        Breach::TooLarge => "too-large",
    }
}

/// Why a journal was deferred, as findings and text say it; git's words cut to
/// [`MAX_SAID_CHARS`].
fn deferral_reason(deferral: &Deferral) -> String {
    match deferral {
        Deferral::Late => "late".to_owned(),
        Deferral::RetriesSpent => "retries-spent".to_owned(),
        Deferral::InboxOccupied => "inbox-occupied".to_owned(),
        Deferral::InboxTooLarge => "inbox-too-large".to_owned(),
        Deferral::TooLarge => "too-large".to_owned(),
        Deferral::Failed(said) => format!("failed: {}", clip(said, MAX_SAID_CHARS)),
    }
}

/// The findings of a checkpoint (CLI-03): each broken ref, each deferred journal, and the events
/// that have no line.
pub(crate) fn checkpoint_findings(report: &RefsExported) -> Vec<Finding> {
    let broken = report.broken.iter().map(|(name, breach)| {
        Finding::new(
            format!("journal/broken:{name}"),
            &broken_message(name, *breach),
            Severity::Error,
        )
    });
    let deferred = report.deferred.iter().map(|(name, deferral)| {
        Finding::new(
            format!("journal/deferred:{name}"),
            &deferred_message(name, deferral),
            Severity::Warning,
        )
    });
    let unexportable = (report.unexportable > 0).then(|| {
        Finding::new(
            "journal/unexportable".to_owned(),
            &format!(
                "{} event(s) are not envelopes and have no journal line",
                report.unexportable
            ),
            Severity::Note,
        )
    });
    broken.chain(deferred).chain(unexportable).collect()
}

/// The checkpoint outcomes a person must act on, one line each, for the hook error log (HK-2):
/// each broken ref, and each deferral that persists until someone acts (an occupied or too
/// large inbox, a journal too large to merge). A late, retried-out or failed journal is retried
/// by the next checkpoint and is not listed: an offline laptop is normal (D12).
pub(crate) fn needs_a_person(report: &RefsExported) -> Vec<String> {
    let broken = report
        .broken
        .iter()
        .map(|(name, breach)| broken_message(name, *breach));
    let lasting = report
        .deferred
        .iter()
        .filter(|(_, deferral)| match deferral {
            Deferral::InboxOccupied | Deferral::InboxTooLarge | Deferral::TooLarge => true,
            Deferral::Late | Deferral::RetriesSpent | Deferral::Failed(_) => false,
        })
        .map(|(name, deferral)| deferred_message(name, deferral));
    broken.chain(lasting).collect()
}

/// A broken journal ref, and what a person does about it.
fn broken_message(name: &JournalName, breach: Breach) -> String {
    format!(
        "{JOURNAL_REFS}{name} breaks JR-05 ({}); move it aside to \
         refs/redkiln/broken/<name>/<sha>",
        breach_name(breach)
    )
}

/// A journal deferred, and why.
fn deferred_message(name: &JournalName, deferral: &Deferral) -> String {
    format!("{name} deferred: {}", deferral_reason(deferral))
}

/// The findings of an import (CLI-03): rewritten refs, passed-over sources, failed fetches and
/// the malformed lines each source reported.
pub(crate) fn import_findings(report: &RefsImport) -> Vec<Finding> {
    let rewritten = report.rewritten.iter().map(|source| {
        Finding::new(
            format!("journal/rewritten:{source}"),
            &format!(
                "{source} was rewritten since the last import (JR-05); its new lines were imported"
            ),
            Severity::Error,
        )
    });
    let passed_over = report.passed_over.iter().map(|source| {
        Finding::new(
            format!("journal/passed-over:{source}"),
            &format!("{source} was passed over: no journal this reader can read"),
            Severity::Warning,
        )
    });
    let fetch_failed = report.fetch_failed.iter().map(|said| {
        Finding::new(
            "journal/fetch-failed".to_owned(),
            &format!(
                "fetching journal sources failed: {}",
                clip(said, MAX_SAID_CHARS)
            ),
            Severity::Warning,
        )
    });
    let malformed = report.sources.iter().flat_map(|read| {
        read.report.problems.iter().map(|problem| {
            Finding::new(
                format!("journal/malformed:{}:{}", read.source, problem.line),
                &format!(
                    "{} line {}: {}",
                    read.source,
                    problem.line,
                    crate::text::Report(&problem.reason)
                ),
                Severity::Warning,
            )
        })
    });
    rewritten
        .chain(passed_over)
        .chain(fetch_failed)
        .chain(malformed)
        .collect()
}

/// The exit and findings of a relay of `branch`: 0 when the inbox is gone or merged and no
/// longer needs a person, else 1 with one finding saying why it was left in place.
pub(crate) fn relay_findings(branch: &str, outcome: &Relayed) -> (Exit, Vec<Finding>) {
    let left = |kind: &str, message: &str| {
        (
            Exit::LeftInPlace,
            vec![Finding::new(
                format!("relay/{kind}:{branch}"),
                message,
                Severity::Error,
            )],
        )
    };
    match outcome {
        Relayed::Gone
        | Relayed::Merged {
            deleted: Deleted::Yes | Deleted::NewerArrived,
            ..
        } => (Exit::Success, Vec::new()),
        Relayed::Merged {
            deleted: Deleted::Failed(said),
            ..
        } => left(
            "delete-failed",
            &format!(
                "the inbox was merged, but deleting it failed: {}",
                clip(said, MAX_SAID_CHARS)
            ),
        ),
        Relayed::Refused(refusal) => left("refused", &format!("refused: {}", refused(refusal))),
        Relayed::Broken(breach) => left(
            "broken",
            &format!("the journal ref breaks JR-05 ({})", breach_name(*breach)),
        ),
        Relayed::TooLarge => left("too-large", "merging would make the journal too large"),
        Relayed::RetriesSpent => left(
            "retries-spent",
            "every retry of a rejected push was rejected",
        ),
        Relayed::Failed(said) => left("failed", &clip(said, MAX_SAID_CHARS)),
    }
}

/// Why the relay refused an inbox commit, in words.
fn refused(refusal: &Refusal) -> String {
    match refusal {
        Refusal::Parents(n) => format!("the inbox commit has {n} parent(s), not one"),
        Refusal::Changes(listed) => format!(
            "the inbox commit changes more than its file: {}",
            clip(listed, MAX_SAID_CHARS)
        ),
        Refusal::TooLarge => "the inbox file is too large to merge".to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use redkiln_core::journal::refs::{
        Breach, Deferral, Deleted, ImportedRef, RefsExported, RefsImport, Refusal, Relayed,
    };
    use redkiln_core::journal::{ImportReport, JournalName, LineError, Problem};

    use super::{
        Budget, CALLS_AFTER_DEADLINE, checkpoint_findings, import_findings, needs_a_person,
        relay_findings,
    };
    use crate::cli_envelope::{Exit, Finding, Severity};

    fn name(text: &str) -> JournalName {
        JournalName::parse(text).expect("a name")
    }

    fn finding(key: &str, message: &str, severity: Severity) -> Finding {
        Finding {
            key: key.to_owned(),
            message: message.to_owned(),
            severity: Some(severity),
        }
    }

    #[test]
    fn the_pre_push_budget_ends_within_twenty_seconds_and_commands_set_no_deadline() {
        let worst = Budget::PRE_PUSH
            .worst_case()
            .expect("pre-push has a deadline");
        let calls = Budget::PRE_PUSH
            .per_call
            .checked_mul(CALLS_AFTER_DEADLINE)
            .expect("no overflow");
        assert_eq!(
            Some(worst),
            Budget::PRE_PUSH
                .start_by
                .and_then(|by| by.checked_add(calls))
        );
        assert!(worst <= Duration::from_secs(20), "{worst:?}");
        assert_eq!(Budget::COMMAND.worst_case(), None);
        assert_eq!(Budget::RELAY.worst_case(), None);
    }

    /// `plugins/weigh-in/hooks/hooks.json`'s Stop timeout, which
    /// `hooks_json_timeouts_match_the_budgets` pins in the file.
    const STOP_HOOK_TIMEOUT: Duration = Duration::from_secs(20);

    /// Time kept back for the hook's own work after the git calls: capture, logging, output.
    const MARGIN: Duration = Duration::from_secs(2);

    #[test]
    fn every_hook_budget_ends_inside_its_hook_timeout() {
        let worst = Budget::STOP.worst_case().expect("Stop has a deadline");
        // Derived from the budget's constants, so a change of per-call time only has to fit.
        let calls = Budget::STOP
            .per_call
            .checked_mul(CALLS_AFTER_DEADLINE)
            .expect("no overflow");
        assert_eq!(
            Some(worst),
            Budget::STOP.start_by.and_then(|by| by.checked_add(calls))
        );
        assert!(
            worst.saturating_add(MARGIN) <= STOP_HOOK_TIMEOUT,
            "{worst:?}"
        );
    }

    #[test]
    fn needs_a_person_lists_broken_refs_and_lasting_deferrals_only() {
        let report = RefsExported {
            broken: vec![(name("s1"), Breach::Tree)],
            deferred: vec![
                (name("s2"), Deferral::Late),
                (name("s3"), Deferral::RetriesSpent),
                (name("s4"), Deferral::InboxOccupied),
                (name("s5"), Deferral::InboxTooLarge),
                (name("s6"), Deferral::TooLarge),
                (name("s7"), Deferral::Failed("fatal: offline".to_owned())),
            ],
            ..RefsExported::default()
        };
        assert_eq!(
            needs_a_person(&report),
            [
                "refs/redkiln/journal/s1 breaks JR-05 (tree); move it aside to \
                 refs/redkiln/broken/<name>/<sha>",
                "s4 deferred: inbox-occupied",
                "s5 deferred: inbox-too-large",
                "s6 deferred: too-large",
            ]
        );
    }

    #[test]
    fn a_deadline_instant_cannot_represent_is_the_start() {
        let started = Instant::now();
        let budget = Budget {
            start_by: Some(Duration::MAX),
            per_call: Duration::from_secs(1),
        };
        let target = budget.target(std::path::Path::new("."), started);
        assert_eq!(target.deadline, Some(started));
        assert_eq!(target.remote, "origin");
        assert_eq!(target.timeout, Duration::from_secs(1));
        assert_eq!(
            Budget::COMMAND
                .target(std::path::Path::new("."), started)
                .deadline,
            None
        );
    }

    #[test]
    fn checkpoint_findings_have_stable_keys() {
        let report = RefsExported {
            appended: 3,
            direct: vec![name("s1")],
            broken: vec![(name("s2"), Breach::Merge), (name("s3"), Breach::TooLarge)],
            deferred: vec![
                (name("s4"), Deferral::Late),
                (name("s5"), Deferral::RetriesSpent),
                (name("s6"), Deferral::InboxOccupied),
                (name("s7"), Deferral::InboxTooLarge),
                (name("s8"), Deferral::TooLarge),
                (name("s9"), Deferral::Failed("fatal: no\nroute".to_owned())),
            ],
            unexportable: 2,
            ..RefsExported::default()
        };
        let aside = "move it aside to refs/redkiln/broken/<name>/<sha>";
        let broken = [("s2", "merge"), ("s3", "too-large")].map(|(n, breach)| {
            finding(
                &format!("journal/broken:{n}"),
                &format!("refs/redkiln/journal/{n} breaks JR-05 ({breach}); {aside}"),
                Severity::Error,
            )
        });
        let deferred = [
            ("s4", "late"),
            ("s5", "retries-spent"),
            ("s6", "inbox-occupied"),
            ("s7", "inbox-too-large"),
            ("s8", "too-large"),
            ("s9", "failed: fatal: no route"),
        ]
        .map(|(n, why)| {
            finding(
                &format!("journal/deferred:{n}"),
                &format!("{n} deferred: {why}"),
                Severity::Warning,
            )
        });
        let unexportable = finding(
            "journal/unexportable",
            "2 event(s) are not envelopes and have no journal line",
            Severity::Note,
        );
        let mut expected = broken.to_vec();
        expected.extend(deferred);
        expected.push(unexportable);
        assert_eq!(checkpoint_findings(&report), expected);
    }

    #[test]
    fn a_clean_checkpoint_has_no_findings() {
        let report = RefsExported {
            appended: 1,
            direct: vec![name("s1")],
            ..RefsExported::default()
        };
        assert_eq!(checkpoint_findings(&report), []);
    }

    #[test]
    fn import_findings_have_stable_keys() {
        let report = RefsImport {
            sources: vec![ImportedRef {
                source: "refs/redkiln/journal/s1".to_owned(),
                commit: "abc".to_owned(),
                report: ImportReport {
                    read: 2,
                    malformed: 1,
                    problems: vec![Problem {
                        line: 2,
                        reason: LineError::NoOrigin,
                    }],
                    ..ImportReport::default()
                },
            }],
            rewritten: vec!["refs/redkiln/journal/s2".to_owned()],
            passed_over: vec!["refs/redkiln/journal/.x".to_owned()],
            fetch_failed: vec!["fatal: offline".to_owned()],
            late: false,
        };
        assert_eq!(
            import_findings(&report),
            [
                finding(
                    "journal/rewritten:refs/redkiln/journal/s2",
                    "refs/redkiln/journal/s2 was rewritten since the last import (JR-05); its \
                     new lines were imported",
                    Severity::Error
                ),
                finding(
                    "journal/passed-over:refs/redkiln/journal/.x",
                    "refs/redkiln/journal/.x was passed over: no journal this reader can read",
                    Severity::Warning
                ),
                finding(
                    "journal/fetch-failed",
                    "fetching journal sources failed: fatal: offline",
                    Severity::Warning
                ),
                finding(
                    "journal/malformed:refs/redkiln/journal/s1:2",
                    "refs/redkiln/journal/s1 line 2: the line has no origin",
                    Severity::Warning
                ),
            ]
        );
    }

    const BRANCH: &str = "redkiln/inbox/s1";

    fn merged(deleted: Deleted) -> Relayed {
        Relayed::Merged { added: 2, deleted }
    }

    #[test]
    fn relay_outcomes_that_need_no_person_exit_0_with_no_finding() {
        for outcome in [
            Relayed::Gone,
            merged(Deleted::Yes),
            merged(Deleted::NewerArrived),
        ] {
            assert_eq!(
                relay_findings(BRANCH, &outcome),
                (Exit::Success, Vec::new()),
                "{outcome:?}"
            );
        }
    }

    #[test]
    fn relay_outcomes_that_leave_the_inbox_exit_1_with_one_error() {
        let cases = [
            (
                merged(Deleted::Failed("denied".to_owned())),
                "delete-failed",
                "the inbox was merged, but deleting it failed: denied",
            ),
            (
                Relayed::Refused(Refusal::Parents(2)),
                "refused",
                "refused: the inbox commit has 2 parent(s), not one",
            ),
            (
                Relayed::Refused(Refusal::Changes("M\tREADME.md".to_owned())),
                "refused",
                "refused: the inbox commit changes more than its file: M\tREADME.md",
            ),
            (
                Relayed::Refused(Refusal::TooLarge),
                "refused",
                "refused: the inbox file is too large to merge",
            ),
            (
                Relayed::Broken(Breach::Unended),
                "broken",
                "the journal ref breaks JR-05 (unended)",
            ),
            (
                Relayed::TooLarge,
                "too-large",
                "merging would make the journal too large",
            ),
            (
                Relayed::RetriesSpent,
                "retries-spent",
                "every retry of a rejected push was rejected",
            ),
            (
                Relayed::Failed("fatal: offline".to_owned()),
                "failed",
                "fatal: offline",
            ),
        ];
        for (outcome, kind, message) in cases {
            let key = format!("relay/{kind}:{BRANCH}");
            assert_eq!(
                relay_findings(BRANCH, &outcome),
                (
                    Exit::LeftInPlace,
                    vec![finding(&key, message, Severity::Error)]
                ),
                "{outcome:?}"
            );
        }
    }
}
