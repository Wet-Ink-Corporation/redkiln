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
//! hook exits 0. A failing hook also says so in its own output (CLI-09, [`Hook::note`]), and the
//! adapter tells output it could not write in one line on stderr. A hook of a registered event
//! keeps a heartbeat while it runs (`heartbeat`, HK-5), so the next hook logs one its timeout
//! killed or that was interrupted. A store path the user gave
//! that Core refuses (a 1.x ledger, another program's database) gets nothing beside it: its
//! refusal goes to the default store's state dir,
//! `<git-common-dir>/redkiln/weighin/` (outside git nowhere), and `SessionStart` reports it.
//! Input that cannot be read or parsed is logged only inside a git repository:
//! outside one, the state dir would be a `.weighin/` the hook creates in whatever folder it was
//! started in, so there it writes nothing. The next `SessionStart` counts the lines added since
//! the last one and tells the user, so capture is never off without a word. (The launcher says so
//! itself when it finds no binary.)
//!
//! In Cowork (as the adapter detects it) hooks run in the cloud workspace, whose
//! project directory is a scratch folder deleted with the session, while the user's files and
//! ledger are on their computer. There the hooks write nothing to disk, not even the error log
//! (a failure is noted in the output only):
//! `SessionStart` prints the convention, `Stop` nudges on one-way markers, and `/weigh-in` sweeps
//! the conversation and records through the computer's shell.

mod errors;
mod failures;
mod heartbeat;
mod session_start;
mod turn;

use anyhow::Result;
use redkiln_core::store::Wait;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use weighin_core::scan::{scan_markdown, scan_todos};

use crate::env::{Env, Vars};
use crate::journal::refs::{self, Budget, Imported};
use crate::legacy_import::{self, Legacy};
use crate::ops::{RaiseInput, RaiseOutcome, Session, is_markdown, scannable};
use errors::{Appended, log_error};
pub(crate) use failures::Failures;
use failures::{Logged, Reach, Rule, one_line};
pub(crate) use session_start::prime;
use session_start::{CONVENTION, COWORK_PRIME, session_start};
use turn::{capture, cowork_nudge};

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

/// The kinds of event weighin is registered for, by which the stdin cap, the output's form and
/// the failure note go; any other event is `Other`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EventKind {
    SessionStart,
    /// `Stop` and `SubagentStop`.
    Turn,
    PostToolUse,
    Other,
}

impl EventKind {
    /// The kind of the event called `name` (as [`HookEvent`]'s labels spell it).
    pub(crate) fn named(name: &str) -> Self {
        match name {
            "SessionStart" => Self::SessionStart,
            "Stop" | "SubagentStop" => Self::Turn,
            "PostToolUse" => Self::PostToolUse,
            _ => Self::Other,
        }
    }
}

impl HookEvent {
    /// The event's kind.
    pub(crate) fn kind(&self) -> EventKind {
        match self {
            Self::SessionStart { .. } => EventKind::SessionStart,
            Self::Stop(_) | Self::SubagentStop(_) => EventKind::Turn,
            Self::PostToolUse(_) => EventKind::PostToolUse,
            Self::Other(_) | Self::Unnamed => EventKind::Other,
        }
    }

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
/// what it says, its failure note included ([`Hook::note`]); the adapter writes it.
pub(crate) fn serve(hook: &Hook, unread: Option<anyhow::Error>) -> HookOutput {
    let mut failures = Failures::default();
    if let Some(e) = unread {
        if hook.in_git() || hook.cowork {
            hook.log(&mut failures, &e);
        } else {
            // Outside git the state dir would be a folder made in the user's folder.
            let why = "hooks log input failures only inside a git repository";
            failures.push(&e, Logged::Not(why.to_owned()));
        }
    }
    let mut output = heartbeat::around(hook, &mut failures, |failures| hook.run(failures));
    if let Some(note) = hook.note(&failures) {
        output.message = Some(match output.message {
            Some(message) => format!("{message} {note}"),
            None => note,
        });
    }
    output
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
    /// The harness's timeout for the hook's event, which its heartbeat records (HK-5); `None`
    /// keeps no heartbeat.
    timeout: Option<Duration>,
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
            timeout: None,
        }
    }

    /// This hook, with `timeout` the harness's timeout for its event (HK-5).
    #[must_use]
    pub(crate) fn timed(self, timeout: Option<Duration>) -> Self {
        Self { timeout, ..self }
    }

    /// This hook, with `named` the event the harness named outside the payload.
    #[must_use]
    pub(crate) fn named(self, named: Option<String>) -> Self {
        Self { named, ..self }
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

    /// Append `e` to the error log, named by [`Hook::label`], and record in `failures` where it
    /// went. In Cowork there is no log to append to: its folder is deleted with the session.
    pub(crate) fn log(&self, failures: &mut Failures, e: &anyhow::Error) {
        if self.cowork {
            failures.push(e, Logged::Cowork);
            return;
        }
        let logged = match log_error(&self.env, self.label(), e) {
            Ok(Appended { path, fresh }) => Logged::At { path, fresh },
            Err(why) => Logged::Not(one_line(&why, 300)),
        };
        failures.push(e, logged);
    }

    /// The kind of the hook's event: the payload's, else the one named outside it.
    pub(crate) fn kind(&self) -> EventKind {
        match &self.event {
            HookEvent::Unnamed => EventKind::named(self.label()),
            event => event.kind(),
        }
    }

    /// The note of the hook's `failures` for the person (CLI-09), by [`Rule::of`].
    fn note(&self, failures: &Failures) -> Option<String> {
        let reach = match (&self.event, &self.env.store) {
            (HookEvent::Unnamed, _) => Reach::Undispatched,
            (_, Ok(_)) => Reach::Located,
            (_, Err(_)) => Reach::Unlocated,
        };
        failures.note(self.label(), Rule::of(self.kind(), reach))
    }

    /// What the hook says (possibly nothing), recording each failure in `failures`.
    fn run(&self, failures: &mut Failures) -> HookOutput {
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
                let (mut notes, spent) = self.legacy(failures);
                if !spent {
                    notes.extend(self.imported(failures));
                }
                return session_start(self, failures, source, notes);
            }
            HookEvent::Stop(turn) => self.checkpointed(failures, capture(env, turn)),
            // A subagent's events leave at its session's Stop (D11).
            HookEvent::SubagentStop(turn) => capture(env, turn),
            HookEvent::PostToolUse(write) => return self.post_tool_use(failures, write.as_ref()),
            HookEvent::Other(_) | HookEvent::Unnamed => Ok(None),
        };
        let message = result.unwrap_or_else(|e| {
            self.log(failures, &e);
            None
        });
        HookOutput {
            context: None,
            message,
        }
    }

    /// `SessionStart`'s one-time import of the weighin 1.x sources inside its budget
    /// (`legacy_import`, ST-15), what the user should hear of it, and whether it spent the
    /// session start's budget importing a source
    /// ([`legacy_import::LegacyReport::imported_a_source`]). Each record it skipped is logged,
    /// and a failure is logged except a store that cannot be located or opened, which
    /// `session_start` tells or logs itself. A run past the session start's worst case is
    /// logged.
    fn legacy(&self, failures: &mut Failures) -> (Vec<String>, bool) {
        let mode = legacy_import::Mode::Hook {
            started: self.started,
        };
        let (notes, spent) = match legacy_import::import_due(&self.env, mode, "hook") {
            Ok(Legacy::NothingDue) => return (Vec::new(), false),
            Ok(Legacy::Ran(report)) => {
                for line in legacy_import::problem_lines(&report) {
                    self.log(failures, &anyhow::anyhow!("{line}"));
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
                    self.log(failures, &e);
                }
                return (Vec::new(), false);
            }
        };
        let took = self.started.elapsed();
        if let Some(worst) = Budget::SESSION_START.worst_case()
            && took > worst
        {
            self.log(
                failures,
                &anyhow::anyhow!(
                    "the SessionStart hook ran {took:?} to the end of its 1.x import, past its \
                 budget's worst case of {worst:?}"
                ),
            );
        }
        (notes, spent)
    }

    /// `SessionStart`'s import of the journal sources inside its budget (JR-10, D10), and what
    /// the user should hear of it: a rewritten ref, a source that could not be read. A failed
    /// fetch or a passed deadline says nothing, since an offline laptop is normal. An error is
    /// logged, except a store that cannot be located or opened, which `session_start` opens again
    /// for its prime and tells or logs itself. An import that ran past its budget's worst case is
    /// logged, as the Stop's export is.
    fn imported(&self, failures: &mut Failures) -> Vec<String> {
        let notes = match refs::import(&self.env, Budget::SESSION_START, self.started, "hook") {
            Ok(Imported::Ran(report)) => refs::import_notes(&report),
            // Not git or a client repository: nothing to import.
            Ok(Imported::Skipped(_)) => Vec::new(),
            Err(e) => {
                let unopened = e.downcast_ref::<refs::StoreUnopened>().is_some();
                if self.env.store.is_ok() && !unopened {
                    self.log(failures, &e);
                }
                Vec::new()
            }
        };
        let took = self.started.elapsed();
        if let Some(worst) = Budget::SESSION_START.worst_case()
            && took > worst
        {
            self.log(
                failures,
                &anyhow::anyhow!(
                    "the SessionStart hook ran {took:?} to the end of its journal import, past its \
                 budget's worst case of {worst:?}"
                ),
            );
        }
        notes
    }
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
