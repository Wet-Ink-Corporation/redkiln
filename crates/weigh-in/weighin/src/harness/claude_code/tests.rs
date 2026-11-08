use super::{Cap, Form, harness_of, input_of, read_payload, render, respond};
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
fn render_post_tool_use_with_a_message_is_a_system_message_beside_any_context() {
    let noted = render(Form::PostToolUse, output(None, Some("Weigh-In: failed")));
    let both = render(
        Form::PostToolUse,
        output(Some("one"), Some("Weigh-In: failed")),
    );

    assert_eq!(noted, r#"{"systemMessage":"Weigh-In: failed"}"#);
    assert_eq!(
        both,
        r#"{"hookSpecificOutput":{"additionalContext":"one","hookEventName":"PostToolUse"},"systemMessage":"Weigh-In: failed"}"#
    );
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
        assert_eq!(Form::of(event.kind()), form, "{event:?}");
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
    let called = Called {
        dir,
        cowork,
        named: None,
    };
    serve_called(&called, input, output, &mut io::sink());
}

/// How a test hook is called: in `dir`, in Cowork or not, and the event `named` on the command
/// line, as `hooks.json` passes it.
struct Called<'a> {
    dir: &'a Path,
    cowork: bool,
    named: Option<&'a str>,
}

/// [`serve_in`] as `called` says, writing its error line to `errors`.
fn serve_called(
    called: &Called<'_>,
    input: &mut impl Read,
    output: &mut impl Write,
    errors: &mut impl Write,
) {
    let (payload, unread) = read_payload(input, Cap::of(called.named));
    let hook = Hook::new(
        input_of(payload).event,
        env_in(called.dir),
        called.cowork,
        std::time::Instant::now(),
    )
    .named(called.named.map(str::to_owned));
    respond(&hook, unread, output, errors);
}

/// [`serve_in`] outside Cowork, for the event `named` on the command line.
fn serve_named(dir: &Path, named: &str, input: &mut impl Read, output: &mut impl Write) {
    let called = Called {
        dir,
        cowork: false,
        named: Some(named),
    };
    serve_called(&called, input, output, &mut io::sink());
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns git and writes the error log through the file system"
)]
fn a_stdout_failure_writes_one_stderr_line() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let input = payload(tmp.path(), "SessionStart");
    let called = Called {
        dir: tmp.path(),
        cowork: false,
        named: None,
    };
    let mut errors = Vec::new();

    serve_called(&called, &mut input.as_bytes(), &mut Broken, &mut errors);

    assert_eq!(
        String::from_utf8(errors).expect("utf-8 error line"),
        "weighin: error: hook SessionStart could not write its output: output gone\n"
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns git and lists a directory through the file system"
)]
fn a_stdout_failure_in_cowork_too_writes_one_stderr_line_and_no_file() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let input = payload(tmp.path(), "Stop");
    let called = Called {
        dir: tmp.path(),
        cowork: true,
        named: Some("Stop"),
    };
    let mut errors = Vec::new();

    serve_called(&called, &mut input.as_bytes(), &mut Broken, &mut errors);

    assert_eq!(
        String::from_utf8(errors).expect("utf-8 error line"),
        "weighin: error: hook Stop could not write its output: output gone\n"
    );
    let written = listing(tmp.path());
    assert!(written.is_empty(), "{written:?}");
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns git and lists a directory through the file system"
)]
fn a_cowork_failure_is_noted_without_a_log() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let called = Called {
        dir: tmp.path(),
        cowork: true,
        named: Some("Stop"),
    };
    let mut noted = Vec::new();

    serve_called(
        &called,
        &mut "not json".as_bytes(),
        &mut noted,
        &mut io::sink(),
    );

    let noted = String::from_utf8(noted).expect("utf-8 output");
    assert!(
        noted.starts_with(
            r#"{"systemMessage":"Weigh-In: the Stop hook failed, so capture may be off: parsing the hook input: "#
        ) && noted.ends_with(r#" Hooks keep no log in Cowork."}"#),
        "{noted}"
    );
    let written = listing(tmp.path());
    assert!(written.is_empty(), "{written:?}");
}

/// A `SessionStart` payload for `dir` of exactly `len` bytes, padded in a field no hook reads.
fn session_start_of_len(dir: &Path, len: usize) -> String {
    let bare =
        serde_json::json!({ "cwd": dir, "hook_event_name": "SessionStart", "pad": "" }).to_string();
    let pad = len.checked_sub(bare.len()).expect("room for the padding");
    let sized = serde_json::json!({
        "cwd": dir,
        "hook_event_name": "SessionStart",
        "pad": " ".repeat(pad),
    })
    .to_string();
    assert_eq!(sized.len(), len, "the payload is exactly {len} bytes");
    sized
}

#[test]
fn the_cap_follows_the_named_event() {
    let caps = [
        (Some("SessionStart"), 65_536),
        (Some("Stop"), 4_194_304),
        (Some("SubagentStop"), 4_194_304),
        (Some("PostToolUse"), 8_388_608),
        (Some("Notification"), 8_388_608),
        (None, 8_388_608),
    ];
    for (named, bytes) in caps {
        assert_eq!(Cap::of(named).bytes(), bytes, "{named:?}");
    }
}

/// An endless input that counts the bytes it served.
struct Endless {
    served: usize,
}

impl Read for Endless {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        buf.fill(b' ');
        self.served = self.served.saturating_add(buf.len());
        Ok(buf.len())
    }
}

#[test]
fn reads_at_most_the_cap_and_one_byte() {
    let mut input = Endless { served: 0 };

    let (payload, unread) = read_payload(&mut input, Cap::of(Some("SessionStart")));

    assert_eq!(input.served, 65_537);
    assert_eq!(payload, serde_json::Value::Null);
    assert_eq!(
        unread.map(|e| format!("{e:#}")),
        Some("the hook input is over its 64 KiB cap, so nothing was dispatched".to_owned())
    );
}

#[test]
#[cfg_attr(
    miri,
    ignore = "spawns git and reads the ledger through the file system"
)]
fn an_input_over_its_cap_dispatches_nothing_and_is_logged_and_noted_in_git() {
    let tmp = git_repo();
    let at_cap = session_start_of_len(tmp.path(), 65_536);
    let over = session_start_of_len(tmp.path(), 65_537);
    let (mut dispatched, mut refused) = (Vec::new(), Vec::new());

    serve_named(
        tmp.path(),
        "SessionStart",
        &mut at_cap.as_bytes(),
        &mut dispatched,
    );
    let log_at_cap = error_log(tmp.path());
    serve_named(
        tmp.path(),
        "SessionStart",
        &mut over.as_bytes(),
        &mut refused,
    );

    let dispatched = String::from_utf8(dispatched).expect("utf-8 output");
    assert!(dispatched.contains("```weigh-in"), "{dispatched}");
    assert_eq!(log_at_cap, "", "the payload at the cap is no failure");
    let log_path = tmp.path().join(".redkiln/weighin/hook-errors.log");
    assert_eq!(
        String::from_utf8(refused).expect("utf-8 output"),
        serde_json::json!({
            "systemMessage": format!(
                "Weigh-In: the SessionStart hook failed, so capture may be off: the hook input is \
                 over its 64 KiB cap, so nothing was dispatched. Logged in {}.",
                log_path.display()
            )
        })
        .to_string()
    );
    let log = error_log(tmp.path());
    assert_eq!(log.lines().count(), 1, "{log}");
    assert!(
        log.contains(
            " SessionStart: the hook input is over its 64 KiB cap, so nothing was dispatched"
        ),
        "{log}"
    );
}

#[test]
#[cfg_attr(miri, ignore = "spawns git")]
fn a_session_start_whose_input_is_not_json_notes_it() {
    let tmp = git_repo();
    let mut output = Vec::new();

    serve_named(
        tmp.path(),
        "SessionStart",
        &mut "not json".as_bytes(),
        &mut output,
    );

    let noted = String::from_utf8(output).expect("utf-8 output");
    assert!(
        noted.starts_with(
            r#"{"systemMessage":"Weigh-In: the SessionStart hook failed, so capture may be off: parsing the hook input: "#
        ) && noted.contains(" Logged in "),
        "{noted}"
    );
    assert_eq!(error_log(tmp.path()).lines().count(), 1);
}

#[test]
#[cfg_attr(miri, ignore = "spawns git")]
fn invalid_utf8_within_the_cap_logs_the_same_line() {
    let tmp = git_repo();
    let mut output = Vec::new();

    serve_named(
        tmp.path(),
        "Stop",
        &mut &b"{\"hook_event_name\": \"Stop\", \"x\": \"\xff\"}"[..],
        &mut output,
    );

    let log = error_log(tmp.path());
    assert_eq!(log.lines().count(), 1, "{log}");
    assert!(
        log.contains(" Stop: reading the hook input: stream did not contain valid UTF-8"),
        "{log}"
    );
}

#[test]
#[cfg_attr(miri, ignore = "spawns git")]
fn an_input_failure_outside_git_is_noted_as_not_logged() {
    let tmp = tempfile::tempdir().expect("temp dir");
    let mut output = Vec::new();

    serve_named(tmp.path(), "Stop", &mut "not json".as_bytes(), &mut output);

    let noted = String::from_utf8(output).expect("utf-8 output");
    assert!(
        noted.ends_with(
            r#" It could not be logged: hooks log input failures only inside a git repository."}"#
        ),
        "{noted}"
    );
    let written = listing(tmp.path());
    assert!(written.is_empty(), "{written:?}");
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
