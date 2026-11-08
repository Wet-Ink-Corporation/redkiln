//! The hooks, in harness-neutral terms. A harness's adapter (`harness::claude_code`) reads its
//! payload into a [`HookInput`], and writes the [`HookOutput`] that [`serve`] returns; nothing
//! here reads or writes a harness's protocol (SPEC CLI-13). Per [`HookEvent`]:
//!
//! * `SessionStart` gives the marker convention and the open counts as context.
//! * `Stop` / `SubagentStop` parse `weigh-in` blocks from the turn's last message and raise them.
//!   `Stop` then exports the store to the journal refs (`journal::refs`, JR-08) inside
//!   `Budget::STOP`, which ends before the hook's timeout; it logs only what a person must act
//!   on (D12). `SubagentStop` exports nothing: its events leave at the session's Stop (D11).
//! * `PostToolUse` (`Write`, `Edit`, `MultiEdit`) scans only the text the tool wrote, then gives
//!   the decisions anchored at that file, at most three, as context (`recall`, #116).
//!
//! These are command hooks: they cost no model tokens. They never fail a session: any error,
//! including output that cannot be written, is appended to `<state dir>/hook-errors.log` and the
//! hook exits 0. A store path the user gave that Core refuses (a 1.x ledger, another program's
//! database) gets nothing beside it: its refusal goes to the default store's state dir,
//! `<git-common-dir>/redkiln/weighin/` (outside git nowhere), and `SessionStart` reports it.
//! Input that cannot be read or parsed is logged only inside a git repository:
//! outside one, the state dir would be a `.weighin/` the hook creates in whatever folder it was
//! started in, so there it writes nothing. The next `SessionStart` counts the lines added since
//! the last one and tells the user, so capture is never off without a word. (The launcher says so
//! itself when it finds no binary.)
//!
//! In Cowork (as the adapter detects it) hooks run in the cloud workspace, whose
//! project directory is a scratch folder deleted with the session, while the user's files and
//! ledger are on their computer. There the hooks write nothing, not even the error log:
//! `SessionStart` prints the convention, `Stop` nudges on one-way markers, and `/weigh-in` sweeps
//! the conversation and records through the computer's shell.

mod errors;
mod session_start;

use anyhow::Result;
use redkiln_core::store::Wait;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;
use weighin_core::Ledger;
use weighin_core::event::Channel;
use weighin_core::marker::{Door, parse_markers};
use weighin_core::scan::{scan_markdown, scan_todos};

use crate::env::{self, Env, Vars};
use crate::journal::refs::{self, Budget, Checkpointed, Imported, Marked, Scope};
use crate::legacy_import::{self, Legacy};
use crate::ops::{self, RaiseInput, RaiseOutcome, Session, is_markdown, scannable};
use crate::{recall, state_file, store};
pub(crate) use session_start::prime;
use session_start::{CONVENTION, COWORK_PRIME, session_start};

/// What a harness hands a hook, read from its payload by the harness's adapter.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct HookInput {
    pub(crate) event: HookEvent,
    /// The folder the hook works in; `None` falls back to the process's.
    pub(crate) cwd: Option<PathBuf>,
    /// The session the harness names, as given.
    pub(crate) session: Option<String>,
}

/// The event a hook runs for, with what it carries.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum HookEvent {
    /// A session starts; `source` is its kind (`startup` when `None`).
    SessionStart { source: Option<String> },
    /// The model ended its turn.
    Stop(Turn),
    /// A subagent ended its turn.
    SubagentStop(Turn),
    /// A tool ran; `None` when it named no tool input or no file.
    PostToolUse(Option<ToolWrite>),
    /// A named event weighin does not handle: dispatches nothing; its name labels log lines.
    Other(String),
    /// No event named (no payload, or not JSON): labelled `hook` in the log.
    Unnamed,
}

impl HookEvent {
    /// The event's label in log lines: its name, or `hook`.
    fn label(&self) -> &str {
        match self {
            Self::SessionStart { .. } => "SessionStart",
            Self::Stop(_) => "Stop",
            Self::SubagentStop(_) => "SubagentStop",
            Self::PostToolUse(_) => "PostToolUse",
            Self::Other(name) => name,
            Self::Unnamed => "hook",
        }
    }
}

/// The turn a `Stop` or `SubagentStop` ends.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Turn {
    /// The model's last message; empty when the harness gave none.
    pub(crate) message: String,
    /// The subagent that spoke, when the harness names one.
    pub(crate) agent: Option<String>,
}

/// What a `PostToolUse` tool wrote: the tool, the file, and the text.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct ToolWrite {
    pub(crate) tool: Tool,
    /// The file, as the tool named it.
    pub(crate) file: String,
    /// The text written: a `Write`'s content, an `Edit`'s or each `MultiEdit` edit's new text.
    /// Empty for any other tool.
    pub(crate) texts: Vec<String>,
}

/// The tools whose writes a `PostToolUse` hook reads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tool {
    Write,
    Edit,
    MultiEdit,
    /// Any other tool: nothing is scanned or recalled.
    Other,
}

/// What a hook says, in harness-neutral terms; the adapter renders it.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct HookOutput {
    /// Added to the model's context: `SessionStart`'s prime, `PostToolUse`'s recall lines.
    pub(crate) context: Option<String>,
    /// Shown to the person, one paragraph.
    pub(crate) message: Option<String>,
}

/// The environment a hook works in: `cwd` (else the process's) and `session`, writing under a
/// hook's wait (ST-09).
pub(crate) fn discover(cwd: Option<&Path>, session: Option<String>, vars: Vars) -> Env {
    let cwd = cwd.map_or_else(
        || std::env::current_dir().unwrap_or_default(),
        Path::to_path_buf,
    );
    let session = session.map(|s| (s, "the hook's session_id"));
    Env::discover(&cwd, session, None, vars).with_wait(Wait::Hook)
}

/// Log why the input was missing (inside a git repository only), dispatch the hook, and return
/// what it says; the adapter writes it.
pub(crate) fn serve(hook: &Hook, unread: Option<anyhow::Error>) -> HookOutput {
    if let Some(e) = unread
        && hook.in_git()
    {
        hook.log(&e);
    }
    hook.run()
}

/// One hook invocation: its event, the environment it works in, and whether it runs in Cowork.
pub(crate) struct Hook {
    event: HookEvent,
    /// The event the harness named outside the payload (the command line), which labels the
    /// hook when the payload names none.
    named: Option<String>,
    env: Env,
    /// Cowork's cloud workspace: the hook writes nothing there (module doc).
    cowork: bool,
    /// When the hook started: the Stop hook's journal export is budgeted from here.
    started: Instant,
}

impl Hook {
    /// The hook for `event` in `env`, in Cowork when `cowork`, started at `started`.
    pub(crate) fn new(event: HookEvent, env: Env, cowork: bool, started: Instant) -> Self {
        Self {
            event,
            named: None,
            env,
            cowork,
            started,
        }
    }

    /// This hook, with `named` the event the harness named outside the payload.
    #[must_use]
    pub(crate) fn named(self, named: Option<String>) -> Self {
        Self { named, ..self }
    }

    /// The event this hook runs for.
    pub(crate) fn event(&self) -> &HookEvent {
        &self.event
    }

    /// The hook's label in log lines and notes: the payload's event, else the event named
    /// outside it, else `hook`.
    pub(crate) fn label(&self) -> &str {
        match (&self.event, self.named.as_deref()) {
            (HookEvent::Unnamed, Some(named)) if !named.is_empty() => named,
            (event, _) => event.label(),
        }
    }

    /// Whether the hook's environment names a git repository, where the state dir is the
    /// ledger's own and logging input failures creates nothing in the user's folder.
    fn in_git(&self) -> bool {
        self.env.repo_root.is_some() || self.env.common_dir.is_some()
    }

    /// Append `e` to the error log, named by the hook's event, or `hook` when the payload names
    /// none. In Cowork there is no log to append to: its folder is deleted with the session.
    pub(crate) fn log(&self, e: &anyhow::Error) {
        if self.cowork {
            return;
        }
        log_error(&self.env, self.label(), e);
    }

    /// What the hook says (possibly nothing).
    fn run(&self) -> HookOutput {
        let env = &self.env;
        if self.cowork {
            return match &self.event {
                HookEvent::SessionStart { .. } => HookOutput {
                    context: Some(format!("{COWORK_PRIME}{CONVENTION}")),
                    message: None,
                },
                HookEvent::Stop(turn) | HookEvent::SubagentStop(turn) => HookOutput {
                    context: None,
                    message: cowork_nudge(&turn.message),
                },
                HookEvent::PostToolUse(_) | HookEvent::Other(_) | HookEvent::Unnamed => {
                    HookOutput::default()
                }
            };
        }
        let result = match &self.event {
            HookEvent::SessionStart { source } => {
                let source = source.as_deref().unwrap_or("startup");
                // The imports come first, so the items they bring count in the prime. When the
                // 1.x import read a source, the journal refs wait for the next session or
                // command, so the hook keeps to its budget (decision H12); a run that read none
                // holds them back no session (review F1).
                let (mut notes, spent) = self.legacy();
                if !spent {
                    notes.extend(self.imported());
                }
                return session_start(env, source, notes);
            }
            HookEvent::Stop(turn) => self.checkpointed(capture(env, turn)),
            // A subagent's events leave at its session's Stop (D11).
            HookEvent::SubagentStop(turn) => capture(env, turn),
            HookEvent::PostToolUse(write) => return self.post_tool_use(write.as_ref()),
            HookEvent::Other(_) | HookEvent::Unnamed => Ok(None),
        };
        let message = result.unwrap_or_else(|e| {
            self.log(&e);
            None
        });
        HookOutput {
            context: None,
            message,
        }
    }

    /// `PostToolUse`: scan what the tool wrote, then, when the tool edited a file, read the
    /// ledger once, as it stands after the scan, for the decisions anchored at that file (#116),
    /// and give them as context. The scan's result stands whatever follows: the recall raises
    /// nothing, and each failure is logged once and says nothing. Nothing is exported here
    /// (D11): the session's Stop exports it.
    fn post_tool_use(&self, write: Option<&ToolWrite>) -> HookOutput {
        let env = &self.env;
        if let Err(e) = scan(env, write) {
            self.log(&e);
        }
        let Some(edit) = write.and_then(|w| recall_edit(env, w)) else {
            return HookOutput::default();
        };
        let events = match ops::stored_events(env) {
            Ok(events) => events,
            Err(e) => {
                self.log(&e.context("reading the ledger after the edit"));
                return HookOutput::default();
            }
        };
        let ledger = Ledger::fold(events.iter().map(|(p, e)| (*p, e)));
        HookOutput {
            // `None` is no decision anchored at the file, not a failure: nothing to add.
            context: recall::edit_context(env, &ledger, &edit),
            message: None,
        }
    }

    /// `result`, after exporting the store to the journal refs inside the Stop budget, whatever
    /// `result` is: a capture that failed part way may have raised items first, and capture
    /// runs first so this turn's items leave with it (JR-08). An export failure, and each
    /// outcome a person must act on (`needs_a_person`, D12), is logged; the result is unchanged,
    /// and the export says nothing. The session's own journal goes first, then only the
    /// journals with an event after the watermark, so the Stop's work grows with what is new,
    /// not with the store's history.
    fn checkpointed(&self, result: Result<Option<String>>) -> Result<Option<String>> {
        let budget = Budget::STOP;
        match refs::checkpoint(&self.env, budget, self.started, Scope::SinceWatermark) {
            Ok(Checkpointed::Ran(report, marked)) => {
                for line in refs::needs_a_person(&report) {
                    self.log(&anyhow::anyhow!("{line}"));
                }
                if let Marked::Failed(e) = marked {
                    self.log(&e);
                }
            }
            // Not git, a client repository, opted out or no store: nothing to say.
            Ok(Checkpointed::Skipped(_)) => {}
            Err(e) => self.log(&e),
        }
        let took = self.started.elapsed();
        if let Some(worst) = budget.worst_case()
            && took > worst
        {
            // The budget's bound assumes Core's call pattern: running past it means that grew.
            self.log(&anyhow::anyhow!(
                "the Stop hook ran {took:?} to the end of its journal export, past its budget's \
                 worst case of {worst:?}"
            ));
        }
        result
    }

    /// `SessionStart`'s one-time import of the weighin 1.x sources inside its budget
    /// (`legacy_import`, ST-15), what the user should hear of it, and whether it spent the
    /// session start's budget importing a source
    /// ([`legacy_import::LegacyReport::imported_a_source`]). Each record it skipped is logged,
    /// and a failure is logged except a store that cannot be located or opened, which
    /// `session_start` tells or logs itself. A run past the session start's worst case is
    /// logged.
    fn legacy(&self) -> (Vec<String>, bool) {
        let mode = legacy_import::Mode::Hook {
            started: self.started,
        };
        let (notes, spent) = match legacy_import::import_due(&self.env, mode, "hook") {
            Ok(Legacy::NothingDue) => return (Vec::new(), false),
            Ok(Legacy::Ran(report)) => {
                for line in legacy_import::problem_lines(&report) {
                    self.log(&anyhow::anyhow!("{line}"));
                }
                let notes = legacy_import::notes(&report)
                    .into_iter()
                    .map(|note| format!("Weigh-In: {note}."))
                    .collect();
                (notes, report.imported_a_source())
            }
            Err(e) => {
                let unopened = e.downcast_ref::<legacy_import::StoreUnopened>().is_some();
                if self.env.store.is_ok() && !unopened {
                    self.log(&e);
                }
                return (Vec::new(), false);
            }
        };
        let took = self.started.elapsed();
        if let Some(worst) = Budget::SESSION_START.worst_case()
            && took > worst
        {
            self.log(&anyhow::anyhow!(
                "the SessionStart hook ran {took:?} to the end of its 1.x import, past its \
                 budget's worst case of {worst:?}"
            ));
        }
        (notes, spent)
    }

    /// `SessionStart`'s import of the journal sources inside its budget (JR-10, D10), and what
    /// the user should hear of it: a rewritten ref, a source that could not be read. A failed
    /// fetch or a passed deadline says nothing, since an offline laptop is normal. An error is
    /// logged, except a store that cannot be located or opened, which `session_start` opens again
    /// for its prime and tells or logs itself. An import that ran past its budget's worst case is
    /// logged, as the Stop's export is.
    fn imported(&self) -> Vec<String> {
        let notes = match refs::import(&self.env, Budget::SESSION_START, self.started, "hook") {
            Ok(Imported::Ran(report)) => refs::import_notes(&report),
            // Not git or a client repository: nothing to import.
            Ok(Imported::Skipped(_)) => Vec::new(),
            Err(e) => {
                let unopened = e.downcast_ref::<refs::StoreUnopened>().is_some();
                if self.env.store.is_ok() && !unopened {
                    self.log(&e);
                }
                Vec::new()
            }
        };
        let took = self.started.elapsed();
        if let Some(worst) = Budget::SESSION_START.worst_case()
            && took > worst
        {
            self.log(&anyhow::anyhow!(
                "the SessionStart hook ran {took:?} to the end of its journal import, past its \
                 budget's worst case of {worst:?}"
            ));
        }
        notes
    }
}

#[expect(
    clippy::let_underscore_must_use,
    let_underscore_drop,
    reason = "last-resort sink: a hook has no other channel to report a failed append (hooks.rs module doc)"
)]
fn log_error(env: &Env, event: &str, e: &anyhow::Error) {
    let Some(dir) = log_dir(env, e) else {
        return;
    };
    // Appended only when it is a regular file or absent (HK-6).
    if state_file::regular_or_absent(&dir.join("hook-errors.log")).is_err() {
        return;
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("hook-errors.log"))
    {
        let _ = writeln!(f, "{} {event}: {e:#}", env::now());
    }
}

/// Whether `e` is Core refusing a store path the user gave (an old ledger, another program's
/// database), beside which nothing is written, not even the state dir (D4, review F2). A refused
/// file at the default place is in redkiln's own directory, and is logged as any failure.
fn refused_given_store(env: &Env, e: &anyhow::Error) -> bool {
    env.store_was_given() && store::is_refused(e)
}

/// Where `e` is logged, the directory made: the state dir, except for
/// [`refused_given_store`], logged in the default store's state dir, and outside git nowhere
/// (`SessionStart` reports it).
fn log_dir(env: &Env, e: &anyhow::Error) -> Option<PathBuf> {
    let dir = if refused_given_store(env, e) {
        env.default_state_dir()?
    } else {
        env.state_dir().ok()?
    };
    state_file::real_dir(&dir).ok()?;
    Some(dir)
}

/// Cowork: no ledger here to write, so nudge straight from the markers in the last message.
fn cowork_nudge(text: &str) -> Option<String> {
    let one_way = parse_markers(text)
        .iter()
        .filter(|m| m.door == Door::OneWay)
        .count();
    (one_way > 0).then(|| {
        format!(
            "Weigh-In: {one_way} one-way question(s) in that message. Run /weigh-in while the context is fresh."
        )
    })
}

/// Raise the markers in `turn`'s message; the message for the person when this session gains a
/// one-way question.
fn capture(env: &Env, turn: &Turn) -> Result<Option<String>> {
    let markers = parse_markers(&turn.message);
    if markers.is_empty() {
        return Ok(None);
    }
    let mut s = Session::open(env, "hook")?;
    let one_way_before = one_way_here(&s, env);
    let mut raised = 0;
    for m in &markers {
        let mut input = RaiseInput::from_marker(m, Channel::Marker);
        if let Some(agent) = &turn.agent {
            input.quote = Some(format!("[subagent {agent}]\n{}", m.raw));
        }
        if matches!(s.raise(&input)?, RaiseOutcome::New(_)) {
            raised += 1;
        }
    }
    let one_way_after = one_way_here(&s, env);
    // One user-visible nudge when this session gains a one-way question; never an automatic sitting.
    if raised > 0 && one_way_after > one_way_before {
        return Ok(Some(format!(
            "Weigh-In: {one_way_after} one-way question(s) from this session are waiting. Run /weigh-in while the context is fresh."
        )));
    }
    Ok(None)
}

fn one_way_here(s: &Session<'_>, env: &Env) -> usize {
    env.session.as_deref().map_or(0, |id| {
        s.ledger
            .open_in_session(id)
            .iter()
            .filter(|i| i.door() == Door::OneWay)
            .count()
    })
}

/// Raise what a `PostToolUse` tool wrote. It says nothing.
fn scan(env: &Env, write: Option<&ToolWrite>) -> Result<()> {
    let Some(write) = write else {
        return Ok(());
    };
    if write.texts.is_empty() {
        return Ok(());
    }
    // Borrowed from the input: a large Write is not copied to be scanned.
    let written: Vec<&str> = write.texts.iter().map(String::as_str).collect();
    scan_text(env, &write.file, &written, "hook")?;
    Ok(())
}

/// A `Write`, `Edit` or `MultiEdit` to a file in this repository, for `recall`; `None` for any
/// other tool or file. The line a `Write` touched is not known, so it carries no text.
fn recall_edit<'w>(env: &Env, write: &'w ToolWrite) -> Option<recall::Edit<'w>> {
    let edited = match write.tool {
        Tool::Edit | Tool::MultiEdit => write.texts.iter().map(String::as_str).collect(),
        Tool::Write => Vec::new(),
        Tool::Other => return None,
    };
    recall::Edit::of(env, &write.file, edited)
}

/// Scan text written to `file` (or the whole file, for `weighin scan`) and raise what it finds.
/// Returns the outcomes, one per candidate.
///
/// # Errors
/// Fails when the ledger cannot be written.
pub(crate) fn scan_text(
    env: &Env,
    file: &str,
    written: &[&str],
    actor: &str,
) -> Result<Vec<RaiseOutcome>> {
    let rel = env.relative(file);
    // Files outside this repository are not ours to raise from or to annotate later.
    if Path::new(&rel).is_absolute() || rel.starts_with("../") || !scannable(env, &rel) {
        return Ok(Vec::new());
    }
    let is_md = is_markdown(&rel);
    let mut candidates = Vec::new();
    for w in written {
        if is_md && env.config.scan.markdown {
            candidates.extend(scan_markdown(w));
        } else if !is_md && env.config.scan.todos {
            candidates.extend(scan_todos(w));
        }
    }
    if candidates.is_empty() {
        return Ok(Vec::new());
    }
    // Find each candidate's real line in the file as it now stands.
    let on_disk = std::fs::read_to_string(env.root().join(&rel)).unwrap_or_default();
    let mut s = Session::open(env, actor)?;
    let mut out = Vec::new();
    for c in candidates {
        let line = on_disk
            .lines()
            .position(|l| l.trim() == c.quote.trim())
            .map(|i| i + 1);
        let input = RaiseInput {
            q: c.question.clone(),
            kind: Some(if c.todo {
                "deferral".into()
            } else if c
                .heading
                .as_deref()
                .is_some_and(|h| h.to_lowercase().contains("assum"))
            {
                "assumption".into()
            } else {
                "question".into()
            }),
            door: Some("unsure".into()),
            authority: None,
            did: None,
            flip: None,
            at: line.map(|l| format!("{rel}:{l}")),
            sym: None,
            blocks: None,
            channel: Some("scan".into()),
            quote: Some(c.quote.clone()),
            source_path: Some(rel.clone()),
            source_line: line,
        };
        out.push(s.raise(&input)?);
    }
    Ok(out)
}
