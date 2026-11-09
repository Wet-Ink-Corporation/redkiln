//! Claude Code's hook protocol, read and written here only (SPEC CLI-13).
//!
//! `weighin hook <event>` reads the hook's JSON payload on stdin. Every field is optional, and a
//! field of the wrong type reads as absent: `hook_event_name` names the event, `cwd` and
//! `session_id` the folder and session, `source` the session start's kind (`startup` when
//! absent), `last_assistant_message` (and a subagent's `agent_id`) the turn a `Stop` or
//! `SubagentStop` captures, and `tool_name` with `tool_input`'s `file_path`, `content`,
//! `new_string` or `edits` what a `PostToolUse` wrote. Input that cannot be read is
//! dispatched as far as it was read; input that is not JSON, or is over its event's [`Cap`], is
//! the null payload, which names no event, so the event the command line names stands in for
//! its labels and form.
//!
//! What the hook says comes back as [`HookOutput`] and is written per event ([`render`]): plain
//! stdout for `SessionStart`'s context alone (it reaches only the model), otherwise one JSON
//! object whose `systemMessage` is shown to the person and whose `hookSpecificOutput`
//! carries `additionalContext` for the model.
//!
//! A hook always exits 0. Nothing here means "block": weighin has no guard, and exit 2 is only
//! clap's usage error, never a hook's. `CLAUDE_CODE_ENTRYPOINT=remote_cowork` is Cowork's cloud
//! workspace, where the hooks write nothing ([`detect`]).

use serde_json::{Map, Value};
use std::io::{self, ErrorKind, Read, Write};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::env::{Harness, Vars};
use crate::hooks::{
    self, EventKind, Failures, Hook, HookEvent, HookInput, HookOutput, Tool, ToolWrite, Turn,
};

/// The variables Claude Code gives the session in, read after `WEIGHIN_SESSION`.
pub(crate) const SESSION_VARS: [&str; 2] = ["CLAUDE_SESSION_ID", "CLAUDE_CODE_SESSION_ID"];

/// `--session`'s help, which names them.
pub(crate) const SESSION_HELP: &str =
    "Session key (defaults to $WEIGHIN_SESSION, $CLAUDE_SESSION_ID, $CLAUDE_CODE_SESSION_ID)";

/// The variable that says where Claude Code runs.
const ENTRYPOINT: &str = "CLAUDE_CODE_ENTRYPOINT";

/// [`ENTRYPOINT`]'s value in Cowork's cloud workspace.
const COWORK: &str = "remote_cowork";

/// Where this process runs, as [`ENTRYPOINT`] says; the one place weighin reads it.
#[must_use]
pub(crate) fn detect() -> Harness {
    harness_of(std::env::var(ENTRYPOINT).ok().as_deref())
}

/// The [`Harness`] an entry point value names: Cowork's, any other non-empty one (Claude Code on
/// this computer), or none.
fn harness_of(entrypoint: Option<&str>) -> Harness {
    match entrypoint {
        Some(COWORK) => Harness::Cowork,
        Some(v) if !v.is_empty() => Harness::ClaudeCode,
        Some(_) | None => Harness::Other,
    }
}

/// `weighin hook [<event>]`: read the payload from `input`, at most its event's [`Cap`], run the
/// hook, and write what it prints to `output` (possibly nothing; a failure note among it,
/// CLI-09). `named` is the event `hooks.json` passes on the command line: it sets the cap, and
/// stands for the event when the payload names none. Never fails: output that cannot be written
/// is logged like any other hook error and told in one line on `errors`, and input that cannot
/// be read or parsed is logged inside a git repository.
pub(crate) fn entry(
    named: Option<&str>,
    input: &mut impl Read,
    output: &mut impl Write,
    errors: &mut impl Write,
) {
    // First, so the journal export's budget counts everything the hook did before it (HK-5).
    let started = Instant::now();
    let (payload, unread) = read_payload(input, Cap::of(named));
    let HookInput {
        event,
        cwd,
        session,
    } = input_of(payload);
    let env = hooks::discover(cwd.as_deref(), session, Vars::from_process());
    let timeout = timeout(&event);
    let hook = Hook::new(event, env, detect() == Harness::Cowork, started)
        .named(named.map(str::to_owned))
        .timed(timeout);
    respond(&hook, unread, output, errors);
}

/// The timeout `hooks.json` gives `event`, which its heartbeat records (HK-5); `None` for an
/// event weighin is not registered for.
fn timeout(event: &HookEvent) -> Option<Duration> {
    let secs = match event {
        HookEvent::Stop(_) => 20,
        HookEvent::SessionStart { .. } | HookEvent::SubagentStop(_) | HookEvent::PostToolUse(_) => {
            10
        }
        HookEvent::Other(_) | HookEvent::Unnamed => return None,
    };
    Some(Duration::from_secs(secs))
}

/// How much of stdin a hook reads, by the event named on the command line, which is known before
/// the payload is read (HK-6, decision D9): a session start's payload is small, a turn's last
/// message may be long, and a `Write` carries the whole file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Cap {
    kib: u64,
}

impl Cap {
    /// The cap for the event `named`: `SessionStart` 64 KiB, `Stop` and `SubagentStop` 4 MiB,
    /// anything else or nothing 8 MiB.
    fn of(named: Option<&str>) -> Self {
        let kib = match named.map(EventKind::named) {
            Some(EventKind::SessionStart) => 64,
            Some(EventKind::Turn) => 4 * 1024,
            Some(EventKind::PostToolUse | EventKind::Other) | None => 8 * 1024,
        };
        Self { kib }
    }

    /// The cap in bytes.
    fn bytes(self) -> u64 {
        self.kib.saturating_mul(1024)
    }
}

/// The hook's payload, and why it is missing or partial when it is: at most `cap` bytes are
/// read, and one more tells an input over the cap, which is the null payload (a cut document
/// cannot be read) and dispatches nothing. Whatever was read before a failure is still
/// dispatched; input that is not UTF-8 or not JSON is the null payload. stdin is not drained:
/// the hook ends soon after, closing it.
fn read_payload(input: &mut impl Read, cap: Cap) -> (Value, Option<anyhow::Error>) {
    let mut bytes = Vec::new();
    let read = input
        .by_ref()
        .take(cap.bytes().saturating_add(1))
        .read_to_end(&mut bytes);
    if u64::try_from(bytes.len()).map_or(true, |len| len > cap.bytes()) {
        let over = anyhow::anyhow!(
            "the hook input is over its {} KiB cap, so nothing was dispatched",
            cap.kib
        );
        return (Value::Null, Some(over));
    }
    let text = String::from_utf8(bytes);
    match (read, text) {
        (Ok(_), Ok(text)) => match serde_json::from_str::<Value>(&text) {
            Ok(payload) => (payload, None),
            Err(e) => (
                Value::Null,
                Some(anyhow::Error::new(e).context("parsing the hook input")),
            ),
        },
        // The error `read_to_string` gave, so the logged line is as it was.
        (Ok(_), Err(_)) => {
            let e = io::Error::new(ErrorKind::InvalidData, "stream did not contain valid UTF-8");
            (
                Value::Null,
                Some(anyhow::Error::new(e).context("reading the hook input")),
            )
        }
        // What was read is dispatched when it parses; the read error is what is logged.
        (Err(e), text) => (
            text.map_or(Value::Null, |t| {
                serde_json::from_str(&t).unwrap_or(Value::Null)
            }),
            Some(anyhow::Error::new(e).context("reading the hook input")),
        ),
    }
}

/// Run `hook`, logging `unread` inside git, and write what it prints for its event to `output`.
/// When `output` cannot be written, the failure is logged and told in one line on `errors`
/// (CLI-08's form, CLI-09); a failed write there is logged too, with nowhere left to tell it.
fn respond(
    hook: &Hook,
    unread: Option<anyhow::Error>,
    output: &mut impl Write,
    errors: &mut impl Write,
) {
    // A payload that names no event (none, or not read) takes the one the command line named.
    let form = Form::of(hook.kind());
    let out = render(form, hooks::serve(hook, unread));
    let Err(e) = output
        .write_all(out.as_bytes())
        .and_then(|()| output.flush())
    else {
        return;
    };
    // After the output: these failures can be told only here and in the log.
    let mut late = Failures::default();
    let line = format!(
        "weighin: error: hook {} could not write its output: {e}",
        hook.label()
    );
    hook.log(
        &mut late,
        &anyhow::Error::new(e).context("writing the hook output"),
    );
    if let Err(e) = writeln!(errors, "{line}").and_then(|()| errors.flush()) {
        hook.log(
            &mut late,
            &anyhow::Error::new(e).context("writing the hook's error line"),
        );
    }
}

/// What the payload hands the hook. Strings are moved out of the payload, never copied.
fn input_of(mut payload: Value) -> HookInput {
    let name = take_str(&mut payload, "hook_event_name");
    HookInput {
        event: event_of(name, &mut payload),
        cwd: take_str(&mut payload, "cwd").map(PathBuf::from),
        session: take_str(&mut payload, "session_id"),
    }
}

/// The event `name`d, with the fields of `payload` it carries; an empty name is none.
fn event_of(name: Option<String>, payload: &mut Value) -> HookEvent {
    let Some(name) = name else {
        return HookEvent::Unnamed;
    };
    match name.as_str() {
        "SessionStart" => HookEvent::SessionStart {
            source: take_str(payload, "source"),
        },
        "Stop" => HookEvent::Stop(turn(payload)),
        "SubagentStop" => HookEvent::SubagentStop(turn(payload)),
        "PostToolUse" => HookEvent::PostToolUse(tool_write(payload)),
        "" => HookEvent::Unnamed,
        _ => HookEvent::Other(name),
    }
}

/// The string at `key` of `value`, moved out; `None` when it is absent or not a string.
fn take_str(value: &mut Value, key: &str) -> Option<String> {
    match value.get_mut(key).map(Value::take) {
        Some(Value::String(s)) => Some(s),
        _ => None,
    }
}

/// The turn a `Stop` or `SubagentStop` ends: its last message (empty when absent) and agent.
fn turn(payload: &mut Value) -> Turn {
    Turn {
        message: take_str(payload, "last_assistant_message").unwrap_or_default(),
        agent: take_str(payload, "agent_id"),
    }
}

/// What a `PostToolUse` tool wrote; `None` when the payload names no tool input or no file.
fn tool_write(payload: &mut Value) -> Option<ToolWrite> {
    let tool = match take_str(payload, "tool_name").as_deref() {
        Some("Write") => Tool::Write,
        Some("Edit") => Tool::Edit,
        Some("MultiEdit") => Tool::MultiEdit,
        _ => Tool::Other,
    };
    let input = payload.get_mut("tool_input")?;
    let file = take_str(input, "file_path")?;
    let texts = match tool {
        Tool::Write => take_str(input, "content").into_iter().collect(),
        Tool::Edit => take_str(input, "new_string").into_iter().collect(),
        Tool::MultiEdit => match input.get_mut("edits") {
            Some(Value::Array(edits)) => edits
                .iter_mut()
                .filter_map(|edit| take_str(edit, "new_string"))
                .collect(),
            _ => Vec::new(),
        },
        Tool::Other => Vec::new(),
    };
    Some(ToolWrite { tool, file, texts })
}

/// How an event's output is written.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Form {
    SessionStart,
    /// `Stop` and `SubagentStop`: a message for the person, never context.
    Turn,
    PostToolUse,
    /// An event weighin does not handle, or none: nothing is written.
    Silent,
}

impl Form {
    /// The form of an event of `kind`.
    fn of(kind: EventKind) -> Self {
        match kind {
            EventKind::SessionStart => Self::SessionStart,
            EventKind::Turn => Self::Turn,
            EventKind::PostToolUse => Self::PostToolUse,
            EventKind::Other => Self::Silent,
        }
    }
}

/// The bytes to print for `output` in `form` (possibly none).
fn render(form: Form, output: HookOutput) -> String {
    let HookOutput { context, message } = output;
    match (form, message) {
        (Form::Silent, _) => String::new(),
        // Plain stdout reaches only the model; no context is nothing to print.
        (Form::SessionStart, None) => context.unwrap_or_default(),
        (Form::SessionStart, message) => json_of(context.map(|c| ("SessionStart", c)), message),
        (Form::PostToolUse, message) => json_of(context.map(|c| ("PostToolUse", c)), message),
        (Form::Turn, message) => json_of(None, message),
    }
}

/// One JSON object: `message` as the `systemMessage` shown to the person, and `context`, an
/// event's name and its text, as that event's `additionalContext` for the model; nothing when
/// there is neither. Keys are in sorted order, as `serde_json` keeps them without
/// `preserve_order`.
fn json_of(context: Option<(&str, String)>, message: Option<String>) -> String {
    let mut out = Map::new();
    if let Some((event, context)) = context {
        out.insert(
            "hookSpecificOutput".to_owned(),
            serde_json::json!({ "hookEventName": event, "additionalContext": context }),
        );
    }
    if let Some(message) = message {
        out.insert("systemMessage".to_owned(), Value::String(message));
    }
    if out.is_empty() {
        return String::new();
    }
    Value::Object(out).to_string()
}

#[cfg(test)]
mod tests;
