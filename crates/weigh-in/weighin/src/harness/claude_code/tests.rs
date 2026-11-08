use super::{Form, harness_of, input_of, read_payload, render, respond};
use crate::env::{Env, Harness, Vars};
use crate::git;
use crate::hooks::{Hook, HookEvent, HookInput, HookOutput, Tool, ToolWrite, Turn, discover};
use redkiln_core::store::Wait;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

fn output(context: Option<&str>, message: Option<&str>) -> HookOutput {
    HookOutput {
        context: context.map(str::to_owned),
        message: message.map(str::to_owned),
    }
}

#[test]
fn render_session_start_context_alone_is_plain_text() {
    let printed = render(Form::SessionStart, output(Some("prime\n"), None));

    assert_eq!(printed, "prime\n");
}

#[test]
fn render_session_start_with_a_message_is_json_with_both() {
    let printed = render(
        Form::SessionStart,
        output(Some("prime \"q\"\n"), Some("Weigh-In: told")),
    );

    assert_eq!(
        printed,
        r#"{"hookSpecificOutput":{"additionalContext":"prime \"q\"\n","hookEventName":"SessionStart"},"systemMessage":"Weigh-In: told"}"#
    );
}

#[test]
fn render_a_turn_message_is_a_system_message() {
    let printed = render(Form::Turn, output(None, Some("Weigh-In: 1 one-way")));

    assert_eq!(printed, r#"{"systemMessage":"Weigh-In: 1 one-way"}"#);
}

#[test]
fn render_a_turn_without_a_message_is_nothing() {
    assert_eq!(render(Form::Turn, HookOutput::default()), "");
}

#[test]
fn render_post_tool_use_context_is_additional_context() {
    let printed = render(Form::PostToolUse, output(Some("one\ntwo"), None));

    assert_eq!(
        printed,
        r#"{"hookSpecificOutput":{"additionalContext":"one\ntwo","hookEventName":"PostToolUse"}}"#
    );
}

#[test]
fn render_post_tool_use_without_context_is_nothing() {
    assert_eq!(render(Form::PostToolUse, HookOutput::default()), "");
}

#[test]
fn render_silent_prints_nothing_whatever_the_output() {
    let printed = render(Form::Silent, output(Some("context"), Some("message")));

    assert_eq!(printed, "");
}

#[test]
fn the_form_follows_the_event() {
    let turn = || Turn {
        message: String::new(),
        agent: None,
    };
    let forms = [
        (HookEvent::SessionStart { source: None }, Form::SessionStart),
        (HookEvent::Stop(turn()), Form::Turn),
        (HookEvent::SubagentStop(turn()), Form::Turn),
        (HookEvent::PostToolUse(None), Form::PostToolUse),
        (HookEvent::Other("Notification".to_owned()), Form::Silent),
        (HookEvent::Unnamed, Form::Silent),
    ];
    for (event, form) in forms {
        assert_eq!(Form::of(&event), form, "{event:?}");
    }
}

#[test]
fn input_of_session_start_reads_cwd_session_and_source() {
    let payload = serde_json::json!({
        "hook_event_name": "SessionStart",
        "cwd": "/work/repo",
        "session_id": "s-1",
        "source": "resume",
    });

    let input = input_of(payload);

    assert_eq!(
        input,
        HookInput {
            event: HookEvent::SessionStart {
                source: Some("resume".to_owned())
            },
            cwd: Some(PathBuf::from("/work/repo")),
            session: Some("s-1".to_owned()),
        }
    );
}

#[test]
fn input_of_reads_a_field_of_the_wrong_type_as_absent() {
    let payload = serde_json::json!({
        "hook_event_name": "SessionStart",
        "cwd": 7,
        "session_id": ["s"],
        "source": false,
    });

    let input = input_of(payload);

    assert_eq!(
        input,
        HookInput {
            event: HookEvent::SessionStart { source: None },
            cwd: None,
            session: None,
        }
    );
}

#[test]
fn input_of_stop_reads_the_last_message_or_empty() {
    let said = input_of(serde_json::json!({
        "hook_event_name": "Stop",
        "last_assistant_message": "done",
    }));
    let silent = input_of(serde_json::json!({ "hook_event_name": "Stop" }));

    assert_eq!(
        (said.event, silent.event),
        (
            HookEvent::Stop(Turn {
                message: "done".to_owned(),
                agent: None
            }),
            HookEvent::Stop(Turn {
                message: String::new(),
                agent: None
            })
        )
    );
}

#[test]
fn input_of_subagent_stop_reads_the_agent() {
    let input = input_of(serde_json::json!({
        "hook_event_name": "SubagentStop",
        "last_assistant_message": "found it",
        "agent_id": "a-7",
    }));

    assert_eq!(
        input.event,
        HookEvent::SubagentStop(Turn {
            message: "found it".to_owned(),
            agent: Some("a-7".to_owned())
        })
    );
}

#[test]
fn input_of_post_tool_use_reads_a_write() {
    let input = input_of(serde_json::json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Write",
        "tool_input": { "file_path": "/r/a.md", "content": "text" },
    }));

    assert_eq!(
        input.event,
        HookEvent::PostToolUse(Some(ToolWrite {
            tool: Tool::Write,
            file: "/r/a.md".to_owned(),
            texts: vec!["text".to_owned()],
        }))
    );
}

#[test]
fn input_of_post_tool_use_reads_an_edit() {
    let input = input_of(serde_json::json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Edit",
        "tool_input": { "file_path": "a.rs", "old_string": "x", "new_string": "y" },
    }));

    assert_eq!(
        input.event,
        HookEvent::PostToolUse(Some(ToolWrite {
            tool: Tool::Edit,
            file: "a.rs".to_owned(),
            texts: vec!["y".to_owned()],
        }))
    );
}

#[test]
fn input_of_post_tool_use_reads_multi_edit_texts_in_order() {
    let input = input_of(serde_json::json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "MultiEdit",
        "tool_input": {
            "file_path": "a.rs",
            "edits": [
                { "new_string": "first" },
                { "old_string": "no new text" },
                { "new_string": "second" },
            ],
        },
    }));

    assert_eq!(
        input.event,
        HookEvent::PostToolUse(Some(ToolWrite {
            tool: Tool::MultiEdit,
            file: "a.rs".to_owned(),
            texts: vec!["first".to_owned(), "second".to_owned()],
        }))
    );
}

#[test]
fn input_of_post_tool_use_of_another_tool_carries_no_text() {
    let input = input_of(serde_json::json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "NotebookEdit",
        "tool_input": { "file_path": "a.ipynb", "new_string": "cell" },
    }));

    assert_eq!(
        input.event,
        HookEvent::PostToolUse(Some(ToolWrite {
            tool: Tool::Other,
            file: "a.ipynb".to_owned(),
            texts: Vec::new(),
        }))
    );
}

#[test]
fn input_of_post_tool_use_without_a_file_is_no_write() {
    let no_input = input_of(serde_json::json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Write",
    }));
    let no_file = input_of(serde_json::json!({
        "hook_event_name": "PostToolUse",
        "tool_name": "Write",
        "tool_input": { "content": "text" },
    }));

    assert_eq!(
        (no_input.event, no_file.event),
        (HookEvent::PostToolUse(None), HookEvent::PostToolUse(None))
    );
}

#[test]
fn input_of_an_unknown_event_is_other() {
    let input = input_of(serde_json::json!({ "hook_event_name": "Notification" }));

    assert_eq!(input.event, HookEvent::Other("Notification".to_owned()));
}

#[test]
fn input_of_the_null_payload_or_an_empty_event_is_unnamed() {
    let null = input_of(serde_json::Value::Null);
    let empty = input_of(serde_json::json!({ "hook_event_name": "" }));

    assert_eq!(
        (null, empty.event),
        (
            HookInput {
                event: HookEvent::Unnamed,
                cwd: None,
                session: None,
            },
            HookEvent::Unnamed
        )
    );
}

#[test]
fn detect_reads_cowork_from_its_entry_point() {
    assert_eq!(harness_of(Some("remote_cowork")), Harness::Cowork);
}

#[test]
fn detect_reads_any_other_entry_point_as_claude_code() {
    assert_eq!(harness_of(Some("cli")), Harness::ClaudeCode);
}

#[test]
fn detect_reads_an_empty_or_unset_entry_point_as_no_harness() {
    assert_eq!(
        (harness_of(Some("")), harness_of(None)),
        (Harness::Other, Harness::Other)
    );
}

// The hook tests below moved from `hooks.rs` with their assertions unchanged (decision D1a).

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
    let hook = Hook::new(
        input_of(payload).event,
        env_in(dir),
        cowork,
        std::time::Instant::now(),
    );
    respond(&hook, unread, output);
}

#[test]
#[cfg_attr(miri, ignore = "spawns git")]
fn a_hook_writes_under_a_hooks_wait_and_a_command_under_a_commands() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let payload = serde_json::json!({ "cwd": tmp.path(), "hook_event_name": "Stop" });

    let input = input_of(payload);
    let hook = discover(input.cwd.as_deref(), input.session, Vars::default());
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
