//! Claude Code hook entry point. One command (`weighin hook`) reads the hook's JSON on stdin and
//! dispatches on `hook_event_name`:
//!
//! * `SessionStart` prints the marker convention and the open counts (added to context).
//! * `Stop` / `SubagentStop` parse `weigh-in` blocks from `last_assistant_message` and raise them.
//!   `Stop` then exports the store to the journal refs (`journal::refs`, JR-08) inside
//!   `Budget::STOP`, which ends before the hook's timeout; it logs only what a person must act
//!   on (D12). `SubagentStop` exports nothing: its events leave at the session's Stop (D11).
//! * `PostToolUse` (`Write`, `Edit`, `MultiEdit`) scans only the text the tool wrote, then adds
//!   the decisions anchored at that file, at most three, as `additionalContext` (`recall`, #116).
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
//! In Cowork (`CLAUDE_CODE_ENTRYPOINT=remote_cowork`) hooks run in the cloud workspace, whose
//! project directory is a scratch folder deleted with the session, while the user's files and
//! ledger are on their computer. There the hooks write nothing, not even the error log:
//! `SessionStart` prints the convention, `Stop` nudges on one-way markers, and `/weigh-in` sweeps
//! the conversation and records through the computer's shell.

mod errors;
mod session_start;

use anyhow::Result;
use redkiln_core::store::Wait;
use serde_json::Value;
use std::io::{Read, Write};
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
use crate::{recall, store};
pub(crate) use session_start::prime;
use session_start::{CONVENTION, COWORK_PRIME, session_start};

/// Run a hook: read its JSON from `input`, dispatch it, and write what it prints to `output`
/// (possibly nothing). Never fails: output that cannot be written is logged like any other hook
/// error, and input that cannot be read or parsed is logged inside a git repository.
pub(crate) fn entry(input: &mut impl Read, output: &mut impl Write) {
    // First, so the journal export's budget counts everything the hook did before it (HK-5).
    let started = Instant::now();
    let (payload, unread) = read_payload(input);
    let env = discover(&payload, Vars::from_process());
    let hook = Hook {
        payload,
        env,
        cowork: env::cowork(),
        started,
    };
    serve(&hook, unread, output);
}

/// The hook's payload, and why it is missing or partial when it is. Whatever was read before a
/// failure is still dispatched; input that is not JSON is the null payload, which dispatches to
/// nothing.
fn read_payload(input: &mut impl Read) -> (Value, Option<anyhow::Error>) {
    let mut stdin = String::new();
    let read = input.read_to_string(&mut stdin);
    let parsed = serde_json::from_str::<Value>(&stdin);
    match (read, parsed) {
        (Ok(_), Ok(payload)) => (payload, None),
        (Ok(_), Err(e)) => (
            Value::Null,
            Some(anyhow::Error::new(e).context("parsing the hook input")),
        ),
        (Err(e), parsed) => (
            parsed.unwrap_or(Value::Null),
            Some(anyhow::Error::new(e).context("reading the hook input")),
        ),
    }
}

/// The environment the payload names: its `cwd` (else the process's) and its session, writing
/// under a hook's wait (ST-09).
fn discover(payload: &Value, vars: Vars) -> Env {
    let cwd = payload.get("cwd").and_then(Value::as_str).map_or_else(
        || std::env::current_dir().unwrap_or_default(),
        PathBuf::from,
    );
    let session = payload
        .get("session_id")
        .and_then(Value::as_str)
        .map(|s| (s.to_owned(), "the hook's session_id"));
    Env::discover(&cwd, session, None, vars).with_wait(Wait::Hook)
}

/// Log why the input was missing (inside a git repository only), dispatch the hook, and write
/// what it prints to `output`.
fn serve(hook: &Hook, unread: Option<anyhow::Error>, output: &mut impl Write) {
    if let Some(e) = unread
        && hook.in_git()
    {
        hook.log(&e);
    }
    let out = hook.run();
    if let Err(e) = output
        .write_all(out.as_bytes())
        .and_then(|()| output.flush())
    {
        hook.log(&anyhow::Error::new(e).context("writing the hook output"));
    }
}

/// One hook invocation: its payload, the environment it names, and whether it runs in Cowork.
struct Hook {
    payload: Value,
    env: Env,
    /// Cowork's cloud workspace: the hook writes nothing there (module doc).
    cowork: bool,
    /// When the hook started: the Stop hook's journal export is budgeted from here.
    started: Instant,
}

impl Hook {
    fn event(&self) -> &str {
        self.payload
            .get("hook_event_name")
            .and_then(Value::as_str)
            .unwrap_or("")
    }

    /// Whether the hook's environment names a git repository, where the state dir is the
    /// ledger's own and logging input failures creates nothing in the user's folder.
    fn in_git(&self) -> bool {
        self.env.repo_root.is_some() || self.env.common_dir.is_some()
    }

    /// Append `e` to the error log, named by the hook's event, or `hook` when the payload names
    /// none. In Cowork there is no log to append to: its folder is deleted with the session.
    fn log(&self, e: &anyhow::Error) {
        if self.cowork {
            return;
        }
        let event = match self.event() {
            "" => "hook",
            event => event,
        };
        log_error(&self.env, event, e);
    }

    /// What to print on stdout (possibly nothing).
    fn run(&self) -> String {
        let (env, payload, event) = (&self.env, &self.payload, self.event());
        if self.cowork {
            return match event {
                "SessionStart" => format!("{COWORK_PRIME}{CONVENTION}"),
                "Stop" | "SubagentStop" => cowork_nudge(payload),
                _ => String::new(),
            };
        }
        let result = match event {
            "SessionStart" => {
                let source = payload
                    .get("source")
                    .and_then(Value::as_str)
                    .unwrap_or("startup");
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
            "Stop" => self.checkpointed(capture(env, payload)),
            // A subagent's events leave at its session's Stop (D11).
            "SubagentStop" => capture(env, payload),
            "PostToolUse" => return self.post_tool_use(),
            _ => Ok(String::new()),
        };
        result.unwrap_or_else(|e| {
            self.log(&e);
            String::new()
        })
    }

    /// `PostToolUse`: scan what the tool wrote, then, when the tool edited a file, read the
    /// ledger once, as it stands after the scan, for the decisions anchored at that file (#116),
    /// and print them as context. The scan's result stands whatever follows: the recall raises
    /// nothing, and each failure is logged once and prints nothing. Nothing is exported here
    /// (D11): the session's Stop exports it.
    fn post_tool_use(&self) -> String {
        let env = &self.env;
        if let Err(e) = scan(env, &self.payload) {
            self.log(&e);
        }
        let Some(edit) = recall_edit(env, &self.payload) else {
            return String::new();
        };
        let events = match ops::stored_events(env) {
            Ok(events) => events,
            Err(e) => {
                self.log(&e.context("reading the ledger after the edit"));
                return String::new();
            }
        };
        let ledger = Ledger::fold(events.iter().map(|(p, e)| (*p, e)));
        // `None` is no decision anchored at the file, not a failure: nothing to add.
        recall::edit_context(env, &ledger, &edit).unwrap_or_default()
    }

    /// `result`, after exporting the store to the journal refs inside the Stop budget, whatever
    /// `result` is: a capture that failed part way may have raised items first, and capture
    /// runs first so this turn's items leave with it (JR-08). An export failure, and each
    /// outcome a person must act on (`needs_a_person`, D12), is logged; the result is unchanged,
    /// and the export prints nothing. The session's own journal goes first, then only the
    /// journals with an event after the watermark, so the Stop's work grows with what is new,
    /// not with the store's history.
    fn checkpointed(&self, result: Result<String>) -> Result<String> {
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
    std::fs::create_dir_all(&dir).ok()?;
    Some(dir)
}

/// Cowork: no ledger here to write, so nudge straight from the markers in the last message.
fn cowork_nudge(payload: &Value) -> String {
    let text = payload
        .get("last_assistant_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    let one_way = parse_markers(text)
        .iter()
        .filter(|m| m.door == Door::OneWay)
        .count();
    if one_way == 0 {
        return String::new();
    }
    let msg = format!(
        "Weigh-In: {one_way} one-way question(s) in that message. Run /weigh-in while the context is fresh."
    );
    serde_json::json!({ "systemMessage": msg }).to_string()
}

fn capture(env: &Env, payload: &Value) -> Result<String> {
    let text = payload
        .get("last_assistant_message")
        .and_then(Value::as_str)
        .unwrap_or("");
    let markers = parse_markers(text);
    if markers.is_empty() {
        return Ok(String::new());
    }
    let mut s = Session::open(env, "hook")?;
    let one_way_before = one_way_here(&s, env);
    let mut raised = 0;
    for m in &markers {
        let mut input = RaiseInput::from_marker(m, Channel::Marker);
        if let Some(agent) = payload.get("agent_id").and_then(Value::as_str) {
            input.quote = Some(format!("[subagent {agent}]\n{}", m.raw));
        }
        if matches!(s.raise(&input)?, RaiseOutcome::New(_)) {
            raised += 1;
        }
    }
    let one_way_after = one_way_here(&s, env);
    // One user-visible nudge when this session gains a one-way question; never an automatic sitting.
    if raised > 0 && one_way_after > one_way_before {
        let msg = format!(
            "Weigh-In: {one_way_after} one-way question(s) from this session are waiting. Run /weigh-in while the context is fresh."
        );
        return Ok(serde_json::json!({ "systemMessage": msg }).to_string());
    }
    Ok(String::new())
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

/// What a `PostToolUse` payload says a tool wrote: the tool, the file, and the text.
struct Written<'p> {
    tool: &'p str,
    file: &'p str,
    /// The text written: a `Write`'s content, an `Edit`'s or each `MultiEdit` edit's `new_string`.
    /// Empty for any other tool.
    texts: Vec<&'p str>,
}

impl<'p> Written<'p> {
    /// `None` when the payload names no tool input or no file.
    fn of(payload: &'p Value) -> Option<Self> {
        let tool = payload
            .get("tool_name")
            .and_then(Value::as_str)
            .unwrap_or("");
        // Borrowed from the payload: a large Write is not copied to be scanned.
        let input = payload.get("tool_input")?;
        let file = input.get("file_path").and_then(Value::as_str)?;
        Some(Self {
            tool,
            file,
            texts: written_texts(tool, input),
        })
    }
}

/// Raise what a `PostToolUse` tool wrote. It prints nothing.
fn scan(env: &Env, payload: &Value) -> Result<()> {
    let Some(Written {
        file,
        texts: written,
        ..
    }) = Written::of(payload)
    else {
        return Ok(());
    };
    if written.is_empty() {
        return Ok(());
    }
    scan_text(env, file, &written, "hook")?;
    Ok(())
}

/// A `Write`, `Edit` or `MultiEdit` to a file in this repository, for `recall`; `None` for any
/// other tool or file. The line a `Write` touched is not known, so it carries no text.
fn recall_edit<'p>(env: &Env, payload: &'p Value) -> Option<recall::Edit<'p>> {
    let w = Written::of(payload)?;
    let edited = match w.tool {
        "Edit" | "MultiEdit" => w.texts,
        "Write" => Vec::new(),
        _ => return None,
    };
    recall::Edit::of(env, w.file, edited)
}

/// The text `tool` wrote, per its input.
fn written_texts<'p>(tool: &str, input: &'p Value) -> Vec<&'p str> {
    match tool {
        "Write" => input
            .get("content")
            .and_then(Value::as_str)
            .into_iter()
            .collect(),
        "Edit" => input
            .get("new_string")
            .and_then(Value::as_str)
            .into_iter()
            .collect(),
        "MultiEdit" => input
            .get("edits")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|e| e.get("new_string").and_then(Value::as_str))
                    .collect()
            })
            .unwrap_or_default(),
        _ => Vec::new(),
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

#[cfg(test)]
mod tests {
    use super::{Hook, discover, read_payload, serve};
    use crate::env::{Env, Vars};
    use crate::git;
    use redkiln_core::store::Wait;
    use std::io::{self, Read, Write};
    use std::path::Path;

    /// A reader or writer whose every call fails.
    struct Broken;

    impl Read for Broken {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("input gone"))
        }
    }

    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "output gone"))
        }

        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::new(io::ErrorKind::BrokenPipe, "output gone"))
        }
    }

    fn payload(dir: &Path, event: &str) -> String {
        serde_json::json!({ "cwd": dir, "hook_event_name": event }).to_string()
    }

    fn error_log(dir: &Path) -> String {
        std::fs::read_to_string(dir.join(".redkiln/weighin/hook-errors.log")).unwrap_or_default()
    }

    /// The environment of a hook in `dir` with its store at `dir/.redkiln/events.sqlite3`,
    /// whatever the process environment says (`REDKILN_STORE`, `WEIGHIN_STORE`).
    fn env_in(dir: &Path) -> Env {
        let store = dir.join(".redkiln/events.sqlite3");
        Env::discover(dir, None, Some(store), Vars::default()).with_wait(Wait::Hook)
    }

    /// A temporary directory that is a git repository.
    fn git_repo() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().expect("temp dir");
        git::run(tmp.path(), &["init", "-q"]).expect("git init");
        tmp
    }

    /// The entries in `dir`, which a hook that writes nothing leaves empty.
    fn listing(dir: &Path) -> Vec<std::fs::DirEntry> {
        std::fs::read_dir(dir)
            .expect("temp dir listing")
            .collect::<io::Result<_>>()
            .expect("temp dir entries")
    }

    /// Run a hook on `input` in `dir` (inside git when `dir` is a repository) with its store
    /// under `dir/.redkiln`, whatever the process environment says (`CLAUDE_CODE_ENTRYPOINT`
    /// among it).
    fn serve_in(dir: &Path, cowork: bool, input: &mut impl Read, output: &mut impl Write) {
        let (payload, unread) = read_payload(input);
        let env = env_in(dir);
        serve(
            &Hook {
                payload,
                env,
                cowork,
                started: std::time::Instant::now(),
            },
            unread,
            output,
        );
    }

    #[test]
    #[cfg_attr(miri, ignore = "spawns git")]
    fn a_hook_writes_under_a_hooks_wait_and_a_command_under_a_commands() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let payload = serde_json::json!({ "cwd": tmp.path(), "hook_event_name": "Stop" });

        let hook = discover(&payload, Vars::default());
        let command = Env::discover(tmp.path(), None, None, Vars::default());

        assert_eq!((hook.wait, command.wait), (Wait::Hook, Wait::Command));
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "spawns git and writes the error log through the file system"
    )]
    fn a_hook_that_cannot_write_its_output_logs_it() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let input = payload(tmp.path(), "SessionStart");

        serve_in(tmp.path(), false, &mut input.as_bytes(), &mut Broken);

        let log = error_log(tmp.path());
        assert!(
            log.contains("SessionStart: writing the hook output: output gone"),
            "{log}"
        );
    }

    #[test]
    #[cfg_attr(miri, ignore = "spawns git")]
    fn a_hook_whose_input_fails_dispatches_what_it_read_and_logs_the_failure() {
        let tmp = git_repo();
        let input = payload(tmp.path(), "Notification");
        let mut output = Vec::new();

        serve_in(
            tmp.path(),
            false,
            &mut input.as_bytes().chain(Broken),
            &mut output,
        );

        assert!(output.is_empty(), "{}", String::from_utf8_lossy(&output));
        let log = error_log(tmp.path());
        assert!(
            log.contains("Notification: reading the hook input: input gone"),
            "{log}"
        );
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "spawns git and reads the ledger through the file system"
    )]
    fn a_hook_prints_what_it_dispatched() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let input = payload(tmp.path(), "SessionStart");
        let mut output = Vec::new();

        serve_in(tmp.path(), false, &mut input.as_bytes(), &mut output);

        let printed = String::from_utf8(output).expect("utf-8 output");
        assert!(printed.contains("```weigh-in"), "{printed}");
        assert_eq!(error_log(tmp.path()), "");
    }

    #[test]
    #[cfg_attr(miri, ignore = "spawns git")]
    fn input_that_is_not_utf8_is_logged_once_under_a_placeholder_event_in_git() {
        let tmp = git_repo();
        let mut output = Vec::new();

        serve_in(tmp.path(), false, &mut &b"\xff"[..], &mut output);

        assert!(output.is_empty(), "{}", String::from_utf8_lossy(&output));
        let log = error_log(tmp.path());
        assert_eq!(log.lines().count(), 1, "{log}");
        assert!(
            log.contains(" hook: reading the hook input: stream did not contain valid UTF-8"),
            "{log}"
        );
    }

    #[test]
    #[cfg_attr(miri, ignore = "spawns git")]
    fn input_that_is_not_json_is_logged_once_in_git() {
        let tmp = git_repo();
        let mut output = Vec::new();

        serve_in(tmp.path(), false, &mut "not json".as_bytes(), &mut output);

        assert!(output.is_empty(), "{}", String::from_utf8_lossy(&output));
        let log = error_log(tmp.path());
        assert_eq!(log.lines().count(), 1, "{log}");
        assert!(log.contains(" hook: parsing the hook input: "), "{log}");
    }

    #[test]
    #[cfg_attr(miri, ignore = "spawns git")]
    fn input_that_is_not_utf8_writes_nothing_outside_git() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let mut output = Vec::new();

        serve_in(tmp.path(), false, &mut &b"\xff"[..], &mut output);

        assert!(output.is_empty(), "{}", String::from_utf8_lossy(&output));
        let written = listing(tmp.path());
        assert!(written.is_empty(), "{written:?}");
    }

    #[test]
    #[cfg_attr(miri, ignore = "spawns git")]
    fn input_that_is_not_json_writes_nothing_outside_git() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let mut output = Vec::new();

        serve_in(tmp.path(), false, &mut "not json".as_bytes(), &mut output);

        assert!(output.is_empty(), "{}", String::from_utf8_lossy(&output));
        let written = listing(tmp.path());
        assert!(written.is_empty(), "{written:?}");
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "spawns git and lists a directory through the file system"
    )]
    fn cowork_hooks_write_nothing_even_when_their_input_and_output_fail() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let input = payload(tmp.path(), "SessionStart");

        serve_in(tmp.path(), true, &mut input.as_bytes(), &mut Broken);
        serve_in(tmp.path(), true, &mut &b"\xff"[..], &mut Vec::new());
        serve_in(tmp.path(), true, &mut Broken, &mut Broken);

        let written = listing(tmp.path());
        assert!(written.is_empty(), "{written:?}");
    }
}
