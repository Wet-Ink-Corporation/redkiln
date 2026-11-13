//! `bench status [--json] [--fields <sections>] [--tail <n>]`: the one read model every UI
//! renders.
//!
//! It folds the ledger and the anneal home's state files into one document, the envelope
//! `bench.status/1` (SPEC CLI-12, `bench_contract::status`), so a UI (the cockpit mod) holds no
//! logic of its own. CLI-12's common keys come first (`status::common`): the store, its source
//! and schema, the repository class, the unusable events by type, `dropped` (always 0) and
//! `health` from the findings. Each section is read by the module that owns its data, never
//! re-implemented here:
//!
//! - `phase`, `plan`, `tasks`, `gates` and `ledger_tail` from the ledger (`status::ledger`):
//!   `mode::active`, `plan::approval_record`, `hook::handoff::facts`' task count, the newest
//!   `gate_blocked`/`stop_blocked` events and `facts::event_word`;
//! - `verdict` from `verdict::pull_request` against `HEAD` (`status::head_verdict`);
//! - `clef` and `context` from the anneal home (`status::home`): `decide`'s configuration and
//!   `counts.json`, and the fresher of the snapshots `bench statusline` and the mod save
//!   (`context::Sources`).
//!
//! A section not asked for, or whose source does not exist, is `null`; one whose source exists
//! but cannot be read is `null` too, with one `status/unreadable:<section>` warning saying
//! why, and the exit code is unchanged. `context` has two sources: when one cannot be read, the other's reading is
//! still shown beside the finding. `--fields` limits which sections are read, not the
//! common keys: `unreadable` reads every Bench event in the ledger once on each run (its cost
//! grows with the ledger; `docs/status.md`, Cost), and `repo_class` runs git once more to find
//! the worktree's suite config when `--store` or a variable chose the store. An anneal
//! ledger not yet imported is one `ledger/old-ledger` note; `status` never imports it
//! (`legacy_import`: the cockpit polls it). A report
//! (SPEC CLI-06): exit 0 whenever the document was written; without a ledger it carries one
//! `status/no-store` note finding, and no ledger is created. See `docs/status.md`.

mod common;
mod head_verdict;
mod home;
mod ledger;
mod text;
mod unreadable;

use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::legacy_import;
use crate::store::{Store, Wait};
use bench_contract::status::{
    Clef, Context, Gate, HeadVerdict, Phase, Plan, Status, TailEvent, Tasks,
};
use clap::{Args, ValueEnum};
use redkiln_core::envelope::SPEC_VERSION;

use crate::command::CommandName;
use crate::envelope::{Envelope, Finding, Severity};
use crate::error::CliError;
use crate::error_chain::reason;
use crate::existing_ledger::{self, LedgerError};
use crate::exit_codes::{self, Code};
use crate::paths::{self, GIT_TIMEOUT, PathError, StoreLocation};
use common::StoreFacts;
use unreadable::Unreadable;

/// The most events `--tail` may ask for (H-13).
const MAX_TAIL: u16 = 1000;

/// `bench status`.
#[derive(Debug, Args)]
pub struct StatusArgs {
    /// Print the JSON envelope, whose `data` is the status document, instead of the text form
    #[arg(long)]
    json: bool,
    /// Only these sections, comma-separated [default: all]
    #[arg(long, value_enum, value_delimiter = ',', value_name = "SECTIONS")]
    fields: Vec<Section>,
    /// How many of the newest ledger events `ledger_tail` lists (0 to 1000)
    #[arg(
        long,
        value_name = "N",
        default_value_t = 10,
        value_parser = clap::value_parser!(u16).range(..=i64::from(MAX_TAIL))
    )]
    tail: u16,
}

/// A section of the status document, as `--fields` names it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Section {
    /// The worktree's bench mode and its playbook.
    Phase,
    /// The build plan's approval.
    Plan,
    /// The build plan's tasks done and in all.
    Tasks,
    /// The newest gates that blocked.
    Gates,
    /// The verdict for `HEAD`.
    Verdict,
    /// Clef's decision points: decisions answered and mode.
    Clef,
    /// The latest context-usage snapshot.
    Context,
    /// The newest ledger events.
    #[value(name = "ledger_tail", alias = "ledger-tail")]
    LedgerTail,
}

impl Section {
    /// The section's key in the document and in its findings.
    const fn key(self) -> &'static str {
        match self {
            Self::Phase => "phase",
            Self::Plan => "plan",
            Self::Tasks => "tasks",
            Self::Gates => "gates",
            Self::Verdict => "verdict",
            Self::Clef => "clef",
            Self::Context => "context",
            Self::LedgerTail => "ledger_tail",
        }
    }
}

impl StatusArgs {
    /// The sections asked for.
    fn wanted(&self) -> Wanted<'_> {
        Wanted {
            fields: &self.fields,
        }
    }
}

/// The sections asked for: all of them when `--fields` names none.
#[derive(Debug, Clone, Copy)]
struct Wanted<'a> {
    fields: &'a [Section],
}

impl Wanted<'_> {
    fn has(self, section: Section) -> bool {
        self.fields.is_empty() || self.fields.contains(&section)
    }

    fn any(self, sections: &[Section]) -> bool {
        sections.iter().any(|section| self.has(*section))
    }
}

/// Bench's sections as they are gathered: each `None` until read, and left `None` when not
/// asked for or without data.
#[derive(Debug, Default)]
struct Sections {
    phase: Option<Phase>,
    plan: Option<Plan>,
    tasks: Option<Tasks>,
    gates: Option<Vec<Gate>>,
    verdict: Option<HeadVerdict>,
    clef: Option<Clef>,
    context: Option<Context>,
    ledger_tail: Option<Vec<TailEvent>>,
}

/// The sections as they are gathered, and the findings for what could not be read.
#[derive(Debug, Default)]
struct Gathered {
    document: Sections,
    findings: Vec<Finding>,
}

impl Gathered {
    /// Keeps `read`'s value, or notes why `section` could not be read and leaves it `null`.
    fn keep<T>(&mut self, section: Section, read: Result<Option<T>, Unreadable>) -> Option<T> {
        match read {
            Ok(value) => value,
            Err(why) => {
                self.unreadable(section.key(), &why.reason());
                None
            }
        }
    }

    fn unreadable(&mut self, what: &str, why: &str) {
        self.findings.push(
            Finding::new(
                format!("status/unreadable:{what}"),
                format!("{what} could not be read: {why}"),
            )
            .with_severity(Severity::Warning),
        );
    }

    /// The document: CLI-12's keys for `location`, `store` and `repo_class`, with `health`
    /// from every finding gathered, then the sections.
    fn into_status(
        self,
        location: &StoreLocation,
        store: StoreFacts,
        repo_class: bench_contract::status::RepoClass,
    ) -> (Status, Vec<Finding>) {
        let Self { document, findings } = self;
        let Sections {
            phase,
            plan,
            tasks,
            gates,
            verdict,
            clef,
            context,
            ledger_tail,
        } = document;
        let status = Status {
            plugin: bench_contract::plugin::NAME.to_owned(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            spec: SPEC_VERSION.to_owned(),
            store: Some(common::store_path(location)),
            store_source: Some(common::store_source(location.source)),
            store_schema: store.schema,
            repo_class,
            unreadable: store.unreadable,
            // Bench records no telemetry, so it drops none (ST-09).
            dropped: 0,
            health: common::health(&findings),
            phase,
            plan,
            tasks,
            gates,
            verdict,
            clef,
            context,
            ledger_tail,
        };
        (status, findings)
    }
}

/// Runs `bench status` in `cwd`, reading environment variables through `lookup`: exit 0
/// whenever the document was written, a missing ledger included. It never creates a ledger.
///
/// # Errors
/// [`CliError::Path`] when git cannot say where the ledger is, [`CliError::Ledger`] when a
/// ledger file exists but cannot be opened, [`CliError::Output`] when `out` cannot be written.
/// Anything a section cannot read is a finding instead.
pub(crate) fn run<O: Write>(
    args: &StatusArgs,
    cwd: &Path,
    lookup: impl Fn(&str) -> Option<OsString>,
    out: &mut O,
) -> Result<Code, CliError> {
    let (location, store) = existing_ledger::located(cwd, &lookup, GIT_TIMEOUT, Wait::Hook)
        // A UI refreshes often and should not hang on a busy writer: Core's hook wait, 2 s (H-13).
        .map_err(|error| match error {
            LedgerError::CurrentDir(source) => CliError::CurrentDir(source),
            LedgerError::Locate(source) => CliError::Path(source),
            LedgerError::Open(source) => CliError::Ledger(source),
        })?;
    let mut gathered = Gathered::default();
    old_ledgers(store.as_ref(), &location.old_ledgers, &mut gathered);
    let mut facts = StoreFacts::default();
    // A report (SPEC CLI-06): it exits 0 whenever it wrote its document, a missing ledger
    // included, which is a note for the reader to judge.
    if let Some(store) = &store {
        facts = common::store_facts(store, &mut gathered);
        from_ledger(store, cwd, args, &mut gathered);
    } else {
        gathered.findings.push(
            Finding::new(
                "status/no-store",
                "no ledger exists yet: nothing has been recorded here. fix: run an anneal \
                 command that records, such as `bench mode build`",
            )
            .with_severity(Severity::Note),
        );
    }
    from_home(&lookup, args.wanted(), &mut gathered);
    let repo_class = common::repo_class(&location, cwd, &mut gathered);
    let (document, findings) = gathered.into_status(&location, facts, repo_class);
    if args.json {
        Envelope::new(CommandName::Status, None, exit_codes::OK, &document)
            .with_findings(&findings)
            .emit(out)
            .map_err(CliError::Output)
    } else {
        text::write(&document, &findings, out).map_err(CliError::Output)?;
        Ok(exit_codes::OK)
    }
}

/// One `ledger/old-ledger` finding naming the anneal ledgers in `old` the store has not
/// imported yet; `status` reports them and imports nothing (the cockpit polls it), so the next
/// session start or recording command does.
fn old_ledgers(store: Option<&Store>, old: &[PathBuf], gathered: &mut Gathered) {
    let due = match legacy_import::due(store, old) {
        Ok(due) => due,
        Err(error) => {
            gathered.unreadable("old-ledger", &reason(&error));
            return;
        }
    };
    if due.is_empty() {
        return;
    }
    let paths: Vec<String> = due.iter().map(|path| path.display().to_string()).collect();
    gathered.findings.push(
        Finding::new(
            "ledger/old-ledger",
            format!(
                "the anneal ledger {} is not imported yet, and is left as it is. fix: start a \
                 session, or run a bench command that records, such as `bench mode build`, \
                 which imports it once",
                paths.join(", ")
            ),
        )
        .with_severity(Severity::Note),
    );
}

/// The sections read from the ledger, and the worktree-bound ones once its root is known.
fn from_ledger(store: &Store, cwd: &Path, args: &StatusArgs, gathered: &mut Gathered) {
    let wanted = args.wanted();
    if wanted.has(Section::Gates) {
        gathered.document.gates = gathered.keep(Section::Gates, ledger::gates(store).map(Some));
    }
    if wanted.has(Section::LedgerTail) {
        let read = ledger::tail(store, usize::from(args.tail)).map(Some);
        gathered.document.ledger_tail = gathered.keep(Section::LedgerTail, read);
    }
    let bound = [
        Section::Phase,
        Section::Plan,
        Section::Tasks,
        Section::Verdict,
    ];
    if !wanted.any(&bound) {
        return;
    }
    let root = match paths::worktree_root_within(cwd, GIT_TIMEOUT) {
        Ok(root) => root,
        // Outside a worktree there is no mode, no plan and no HEAD: those sections are absent.
        Err(PathError::NotARepository { .. }) => return,
        Err(error) => {
            gathered.unreadable("worktree", &reason(&error));
            return;
        }
    };
    let worktree = ledger::Worktree { store, root: &root };
    from_worktree(&worktree, wanted, gathered);
}

/// `phase`, `plan`, `tasks` and `verdict` of the worktree at `worktree.root`.
fn from_worktree(worktree: &ledger::Worktree<'_>, wanted: Wanted<'_>, gathered: &mut Gathered) {
    if wanted.any(&[Section::Phase, Section::Plan, Section::Tasks]) {
        let entered = gathered.keep(Section::Phase, worktree.mode());
        if let Some(build) = entered.as_ref().and_then(ledger::BuildPlan::of) {
            from_plan(worktree, &build, wanted, gathered);
        }
        if wanted.has(Section::Phase) {
            gathered.document.phase = entered.map(ledger::phase);
        }
    }
    if wanted.has(Section::Verdict) {
        let read = head_verdict::read(worktree.store, Path::new(worktree.root));
        gathered.document.verdict = gathered.keep(Section::Verdict, read);
    }
}

/// `plan` and `tasks` of the build plan, whose bytes are read once for both.
fn from_plan(
    worktree: &ledger::Worktree<'_>,
    build: &ledger::BuildPlan<'_>,
    wanted: Wanted<'_>,
    gathered: &mut Gathered,
) {
    let ledger::PlanRead { approval, bytes } = match worktree.plan(build) {
        Ok(read) => read,
        Err(why) => {
            let why = why.reason();
            for section in [Section::Plan, Section::Tasks] {
                if wanted.has(section) {
                    gathered.unreadable(section.key(), &why);
                }
            }
            return;
        }
    };
    if wanted.has(Section::Plan) {
        gathered.document.plan = Some(approval);
    }
    if wanted.has(Section::Tasks) {
        let read = worktree.tasks(build, &bytes).map(Some);
        gathered.document.tasks = gathered.keep(Section::Tasks, read);
    }
}

/// `clef` and `context`, from the anneal home.
fn from_home(
    lookup: impl Fn(&str) -> Option<OsString>,
    wanted: Wanted<'_>,
    gathered: &mut Gathered,
) {
    if !wanted.any(&[Section::Clef, Section::Context]) {
        return;
    }
    let home = match paths::anneal_home_from(&lookup) {
        Ok(home) => home,
        Err(error) => {
            gathered.unreadable("home", &reason(&error));
            return;
        }
    };
    if wanted.has(Section::Clef) {
        gathered.document.clef = gathered.keep(Section::Clef, home::clef(&home, &lookup));
    }
    if wanted.has(Section::Context) {
        let home::ContextRead {
            context,
            unreadable,
        } = home::context(&home);
        if !unreadable.is_empty() {
            let why: Vec<String> = unreadable.iter().map(Unreadable::reason).collect();
            gathered.unreadable(Section::Context.key(), &why.join("; "));
        }
        gathered.document.context = context;
    }
}

#[cfg(test)]
mod tests;
