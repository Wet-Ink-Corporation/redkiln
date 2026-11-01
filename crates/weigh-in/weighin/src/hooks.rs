//! Claude Code hook entry point. One command (`weighin hook`) reads the hook's JSON on stdin and
//! dispatches on `hook_event_name`:
//!
//! * `SessionStart` prints the marker convention and the open counts (added to context).
//! * `Stop` / `SubagentStop` parse `weigh-in` blocks from `last_assistant_message` and raise them.
//! * `PostToolUse` (`Write`, `Edit`, `MultiEdit`) scans only the text the tool wrote, then adds
//!   the decisions anchored at that file, at most three, as `additionalContext` (`recall`, #116).
//!
//! These are command hooks: they cost no model tokens. They never fail a session: any error,
//! including output that cannot be written, is appended to `<state dir>/hook-errors.log` and the
//! hook exits 0. Input that cannot be read or parsed is logged only inside a git repository:
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

use anyhow::Result;
use serde_json::Value;
use std::fmt::Write as _;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use weighin_core::Ledger;
use weighin_core::event::Channel;
use weighin_core::marker::{Door, parse_markers};
use weighin_core::scan::{scan_markdown, scan_todos};

use crate::env::{self, Config, ConfigSource, Env};
use crate::ops::{self, RaiseInput, RaiseOutcome, Session, is_markdown, read_ledger, scannable};
use crate::text::{Report, clip};
use crate::{journal, recall, shell_bin};

/// Run a hook: read its JSON from `input`, dispatch it, and write what it prints to `output`
/// (possibly nothing). Never fails: output that cannot be written is logged like any other hook
/// error, and input that cannot be read or parsed is logged inside a git repository.
pub(crate) fn entry(input: &mut impl Read, output: &mut impl Write) {
    let (payload, unread) = read_payload(input);
    let env = discover(&payload);
    let hook = Hook {
        payload,
        env,
        cowork: env::cowork(),
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

/// The environment the payload names: its `cwd` (else the process's) and its session.
fn discover(payload: &Value) -> Env {
    let cwd = payload.get("cwd").and_then(Value::as_str).map_or_else(
        || std::env::current_dir().unwrap_or_default(),
        PathBuf::from,
    );
    let session = payload
        .get("session_id")
        .and_then(Value::as_str)
        .map(str::to_owned);
    Env::discover(&cwd, session)
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
                return session_start(env, source);
            }
            "Stop" | "SubagentStop" => self.journalled(capture(env, payload)),
            "PostToolUse" => return self.post_tool_use(),
            _ => Ok(String::new()),
        };
        result.unwrap_or_else(|e| {
            self.log(&e);
            String::new()
        })
    }

    /// `PostToolUse`: scan what the tool wrote, then read the ledger once, as it stands after the
    /// scan, for the session's journal and for the decisions anchored at the edited file
    /// (#116), and print those decisions as context. The scan's result stands whatever follows:
    /// the recall raises nothing, and each failure is logged once and prints nothing.
    fn post_tool_use(&self) -> String {
        let env = &self.env;
        if let Err(e) = scan(env, &self.payload) {
            self.log(&e);
        }
        let edit = recall_edit(env, &self.payload);
        let journal = journal::kept(env);
        if edit.is_none() && !journal {
            return String::new();
        }
        let events = match ops::stored_events(env) {
            Ok(events) => events,
            Err(e) => {
                self.log(&e.context("reading the ledger after the edit"));
                return String::new();
            }
        };
        if journal && let Err(e) = journal::sync_events(env, journal::WAIT, &events) {
            self.log(&e.context("updating the session journal"));
        }
        let Some(edit) = edit else {
            return String::new();
        };
        let ledger = Ledger::fold(events.iter().map(|(p, e)| (*p, e)));
        // `None` is no decision anchored at the file, not a failure: nothing to add.
        recall::edit_context(env, &ledger, &edit).unwrap_or_default()
    }

    /// `result`, after bringing the session's journal up to date, whatever `result` is: a
    /// capture that failed part way may have raised items first (B-28). A journal failure is
    /// logged like any other hook failure, and the result is unchanged.
    fn journalled(&self, result: Result<String>) -> Result<String> {
        if let Err(e) = journal::sync(&self.env, journal::WAIT) {
            self.log(&e.context("updating the session journal"));
        }
        result
    }
}

#[expect(
    clippy::let_underscore_must_use,
    let_underscore_drop,
    reason = "last-resort sink: a hook has no other channel to report a failed append (hooks.rs module doc)"
)]
fn log_error(env: &Env, event: &str, e: &anyhow::Error) {
    let Ok(dir) = env.ensure_state_dir() else {
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

/// `SessionStart`: the prime for Claude's context and, when hooks have failed since the last
/// session or the config is broken, a one-line message for the user. A prime that fails is
/// logged and reported with the rest, and Claude still gets the convention. A broken config is
/// reported here, once a session, and never logged.
fn session_start(env: &Env, source: &str) -> String {
    let broken = broken_config(env);
    let mut context = prime(env, source).unwrap_or_else(|e| {
        log_error(env, "SessionStart", &e);
        CONVENTION.to_owned()
    });
    if let Some(copy) = shell_copy(env) {
        context.push_str("Weigh-In binary for the shell: ");
        context.push_str(&copy.display().to_string());
        context.push('\n');
    }
    let message = match (broken, new_errors(env)) {
        (Some(broken), Some(errors)) => Some(format!("{broken} {errors}")),
        (broken, errors) => broken.or(errors),
    };
    match message {
        // Plain stdout reaches only Claude; `systemMessage` is shown to the user.
        Some(message) => serde_json::json!({
            "systemMessage": message,
            "hookSpecificOutput": {
                "hookEventName": "SessionStart",
                "additionalContext": context,
            },
        })
        .to_string(),
        None => context,
    }
}

/// On Windows inside a git repository, a copy of this binary in the repository's git dir, where
/// the skill's shell can reach it when it cannot see the plugin folder (#127, `shell_bin`). Nowhere
/// else: other platforms reach the plugin folder, and outside git the state dir would be a
/// `.weighin/` created in the user's folder. A failed copy is logged, and `SessionStart` goes on.
fn shell_copy(env: &Env) -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }
    // The repository's git dir, not the state dir, which follows a configured `store` to a folder
    // the shell may not see.
    let dir = env.common_dir.as_deref()?.join("weighin");
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(e) => {
            log_error(env, "SessionStart", &anyhow::Error::from(e));
            return None;
        }
    };
    match shell_bin::place(&exe, &dir, env!("CARGO_PKG_VERSION")) {
        Ok(copy) => Some(copy),
        Err(unplaced) => {
            log_error(env, "SessionStart", &unplaced.error);
            unplaced.existing
        }
    }
}

/// When the folder's config cannot be used, the sentence that leads the user's message. It is
/// not logged: `new_errors` would read it straight back as a hook failure, and every other
/// command reports it itself. Hooks never fail a session, so they carry on with the defaults,
/// which drop the config's settings (a `store` elsewhere among them) until the file is fixed.
fn broken_config(env: &Env) -> Option<String> {
    let ConfigSource::Broken(e) = &env.config_source else {
        return None;
    };
    // The fault only: the sentence names the file already, and a deep folder's absolute path
    // would push the fault past the clip.
    let fault = std::error::Error::source(e)
        .map_or_else(|| e.to_string(), |cause| Report(cause).to_string());
    Some(format!(
        "Weigh-In: {} is broken ({}). Until it is fixed, hooks run on the defaults, so items captured now go to the default ledger, not a `store` the config names, and other weighin commands stop.",
        Config::PATH,
        clip(&fault, 200)
    ))
}

/// How far `hook-errors.log` had been reported, as a byte offset and the log's first line then,
/// beside the log.
const ERRORS_SEEN: &str = "hook-errors.seen";

/// The lines added to `hook-errors.log` since the last `SessionStart` reported it, as a message;
/// records the log's length so each line is reported once. Writes nothing when there is no log,
/// and overwrites rather than deletes (B-01).
fn new_errors(env: &Env) -> Option<String> {
    let dir = env.state_dir();
    let log_path = dir.join("hook-errors.log");
    let log = std::fs::read(&log_path).ok()?;
    // The length reported so far, then the start of the log's first line at that time. A
    // different start means the log was truncated since, so all of it is new. Lines begin with a
    // timestamp, so this assumes a truncation removes the first line; a log trimmed only at its
    // end is not detected. A record with no first line, from an older build, is taken as it
    // stands; an older build reading this record reports the whole log once, then records a
    // bare length again.
    let head = String::from_utf8_lossy(first_line(&log));
    let recorded = std::fs::read_to_string(dir.join(ERRORS_SEEN)).unwrap_or_default();
    let (offset, recorded_head) = match recorded.split_once('\n') {
        Some((offset, recorded_head)) => (offset, Some(recorded_head)),
        None => (recorded.as_str(), None),
    };
    let seen = offset
        .trim()
        .parse::<usize>()
        .ok()
        .filter(|&n| n <= log.len() && recorded_head.is_none_or(|h| h == head))
        .unwrap_or(0);
    if seen == log.len() {
        return None;
    }
    let record = format!("{}\n{head}", log.len());
    let marked = match std::fs::write(dir.join(ERRORS_SEEN), record) {
        Ok(()) => String::new(),
        Err(e) => format!(" Could not mark this report as seen, so it will repeat: {e}."),
    };
    let added = String::from_utf8_lossy(log.get(seen..)?);
    let lines: Vec<&str> = added.lines().filter(|l| !l.trim().is_empty()).collect();
    let last = clip(lines.last()?, 200);
    Some(format!(
        "Weigh-In: {} hook error(s) since the last session, so capture may be off. Last: {last} (full log: {}){marked}",
        lines.len(),
        log_path.display()
    ))
}

/// How much of the log's first line `hook-errors.seen` keeps: the timestamp and event, and more.
const SEEN_HEAD_BYTES: usize = 80;

/// The bytes of `log` before its first newline, at most [`SEEN_HEAD_BYTES`] of them.
fn first_line(log: &[u8]) -> &[u8] {
    let line = log.split(|&b| b == b'\n').next().unwrap_or_default();
    line.get(..SEEN_HEAD_BYTES).unwrap_or(line)
}

/// The convention, the session key and the counts, in at most ~150 tokens.
///
/// # Errors
/// Fails when the ledger cannot be read.
pub(crate) fn prime(env: &Env, source: &str) -> Result<String> {
    // Read-only: a session that raises nothing must not leave a store behind.
    let ledger = read_ledger(env)?;
    let open = ledger.open();
    let here = env
        .session
        .as_deref()
        .map_or(0, |id| ledger.open_in_session(id).len());
    let one_way = open.iter().filter(|i| i.door() == Door::OneWay).count();
    let defaults = ledger.unacknowledged_defaults().len();
    let due = match ledger
        .due_for_review(&env::now(), env.config.caps.review_age_days)
        .len()
    {
        0 => String::new(),
        n => format!(" · {n} decision(s) due a look"),
    };
    let mut out = String::new();
    writeln!(
        out,
        "Weigh-In ({source}): session {} · {here} open here · {} open in repo ({one_way} one-way) · {defaults} default(s) to review{due}.",
        env.session.as_deref().unwrap_or("?"),
        open.len()
    )?;
    out.push_str(CONVENTION);
    Ok(out)
}

const COWORK_PRIME: &str = "Weigh-In (cowork): the ledger lives in the user's connected folder, not in this workspace; /weigh-in records there through the computer's shell.\n";

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

const CONVENTION: &str = "When you assume, defer, make a call the user did not examine, or cannot decide, end your message with:\n```weigh-in\nkind: question|assumption|deferral|call\ndoor: one-way|two-way|unsure\nauthority: authority|judgment\nq: <the full question?>\ndid: <what you did meanwhile, or blocked>\nflip: <the fact that would change the answer>\nat: <path:line>\n```\nA list of calls for the user (\"Decisions for you\", \"Your calls\") gets one block per call. No TODO/FIXME/\"revisit later\" without a block. If you cannot ask the user, take no one-way path: `did: blocked`, then continue with independent work.\n";

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
    use super::{Hook, new_errors, read_payload, serve};
    use crate::env::Env;
    use crate::git;
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
        std::fs::read_to_string(dir.join(".weighin").join("hook-errors.log")).unwrap_or_default()
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

    /// Run a hook on `input` in `dir` (inside git when `dir` is a repository) with its ledger
    /// under `dir/.weighin`, whatever the process environment says (`WEIGHIN_STORE`,
    /// `CLAUDE_CODE_ENTRYPOINT`).
    fn serve_in(dir: &Path, cowork: bool, input: &mut impl Read, output: &mut impl Write) {
        let (payload, unread) = read_payload(input);
        let env =
            Env::discover(dir, None).with_store(Some(dir.join(".weighin").join("ledger.sqlite3")));
        serve(
            &Hook {
                payload,
                env,
                cowork,
            },
            unread,
            output,
        );
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "reads and writes the error log through the file system"
    )]
    fn a_seen_record_from_an_older_build_still_counts_from_its_length() {
        let tmp = tempfile::tempdir().expect("temp dir");
        let state = tmp.path().join(".weighin");
        std::fs::create_dir_all(&state).expect("state dir");
        let env = Env::discover(tmp.path(), None).with_store(Some(state.join("ledger.sqlite3")));
        let old = "2026-10-01T00:00:00Z Stop: reported before\n";
        let added = "2026-10-02T00:00:00Z Stop: logged since\n";
        std::fs::write(state.join("hook-errors.log"), format!("{old}{added}")).expect("log");
        // Older builds recorded the length alone.
        std::fs::write(state.join("hook-errors.seen"), old.len().to_string()).expect("seen");

        let report = new_errors(&env).expect("the added error is reported");

        assert!(
            report.starts_with("Weigh-In: 1 hook error(s) since the last session")
                && report.contains("Last: 2026-10-02T00:00:00Z Stop: logged since"),
            "{report}"
        );
    }

    #[test]
    #[cfg_attr(
        miri,
        ignore = "reads and writes the error log through the file system"
    )]
    fn errors_logged_after_the_log_was_truncated_are_all_reported() {
        use std::fmt::Write as _;
        let tmp = tempfile::tempdir().expect("temp dir");
        let state = tmp.path().join(".weighin");
        std::fs::create_dir_all(&state).expect("state dir");
        let log = state.join("hook-errors.log");
        let env = Env::discover(tmp.path(), None).with_store(Some(state.join("ledger.sqlite3")));
        let old = "2026-10-01T00:00:00Z Stop: an old failure that was reported\n".repeat(3);
        std::fs::write(&log, &old).expect("old log");
        assert!(
            new_errors(&env).is_some(),
            "the old errors are reported once"
        );
        // The owner truncates the log once read; new errors grow it past the old length.
        let new = (1..=4).fold(String::new(), |mut acc, n| {
            writeln!(
                acc,
                "2026-10-02T00:00:0{n}Z Stop: new failure number {n} after the log was truncated"
            )
            .expect("writing to a String");
            acc
        });
        assert!(new.len() > old.len(), "the new log outgrows the old one");
        std::fs::write(&log, &new).expect("new log");

        let report = new_errors(&env).expect("the new errors are reported");

        assert!(
            report.starts_with("Weigh-In: 4 hook error(s) since the last session")
                && report.contains("Last: 2026-10-02T00:00:04Z Stop: new failure number 4"),
            "{report}"
        );
        assert_eq!(new_errors(&env), None, "reported once");
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
