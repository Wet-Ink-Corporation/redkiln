//! A failing hook says so in its own output (CLI-09), as decision D2 B reads it: `Stop`,
//! `SubagentStop` and `PostToolUse` note a failure when the error log held nothing unreported
//! before it, or when it could not be logged; `SessionStart` notes only what it could not log,
//! since its report reads the rest back; any other event stays silent. Hook state files are
//! used only when they are regular files (HK-6).

use crate::support::{MESSAGE, hook, setup, state_of, store_of, weighin};
use serde_json::{Value, json};
use std::path::Path;

type TestResult = Result<(), Box<dyn std::error::Error>>;

/// The lines of the repository's error log, none when there is no log.
fn logged(repo: &Path) -> std::io::Result<Vec<String>> {
    match std::fs::read_to_string(state_of(repo).join("hook-errors.log")) {
        Ok(log) => Ok(log.lines().map(str::to_owned).collect()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(e) => Err(e),
    }
}

/// A directory where the store should be: it exists, so it is opened, and every open fails.
fn break_the_store(repo: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(store_of(repo))
}

fn stop(repo: &Path) -> Value {
    json!({"hook_event_name": "Stop", "session_id": "s1", "cwd": repo, "last_assistant_message": MESSAGE})
}

fn system_message(printed: &str) -> Result<String, Box<dyn std::error::Error>> {
    let out: Value = serde_json::from_str(printed)?;
    let message = out
        .get("systemMessage")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("no systemMessage in {printed}"))?;
    Ok(message.to_owned())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_stop_whose_capture_fails_notes_it_in_its_system_message() -> TestResult {
    let (_tmp, repo) = setup();
    break_the_store(&repo)?;

    let printed = hook(&repo, &stop(&repo));

    let message = system_message(&printed)?;
    let log = logged(&repo)?;
    let first = log.first().ok_or("nothing logged")?;
    let (_, line) = first.split_once(" Stop: ").ok_or("not a Stop line")?;
    assert!(
        message.starts_with(&format!(
            "Weigh-In: the Stop hook failed, so capture may be off: {}",
            line.trim_end_matches('.')
        )),
        "{message}"
    );
    assert!(
        message.ends_with(&format!(
            "Logged in {}.",
            state_of(&repo).join("hook-errors.log").display()
        )),
        "{message}"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_second_failure_in_the_session_is_only_logged() -> TestResult {
    let (_tmp, repo) = setup();
    break_the_store(&repo)?;
    let first = hook(&repo, &stop(&repo));
    let logged_first = logged(&repo)?.len();

    let second = hook(&repo, &stop(&repo));

    assert!(first.contains("the Stop hook failed"), "{first}");
    assert_eq!(second, "", "the log already holds an unreported failure");
    assert!(logged(&repo)?.len() > logged_first, "still logged");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_post_tool_use_failure_is_noted_once() -> TestResult {
    let (_tmp, repo) = setup();
    break_the_store(&repo)?;
    let edit = json!({
        "hook_event_name": "PostToolUse", "session_id": "s1", "cwd": repo, "tool_name": "Edit",
        "tool_input": {"file_path": repo.join("src/retention.rs"), "old_string": "0", "new_string": "0\n"}
    });

    let first = hook(&repo, &edit);
    let second = hook(&repo, &edit);

    let message = system_message(&first)?;
    assert!(
        message.starts_with("Weigh-In: the PostToolUse hook failed, so capture may be off: "),
        "{message}"
    );
    let out: Value = serde_json::from_str(&first)?;
    assert_eq!(out.get("hookSpecificOutput"), None, "{first}");
    assert_eq!(second, "");
    assert_eq!(logged(&repo)?.len(), 2, "each failure is logged");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_session_start_that_cannot_log_notes_its_failure_directly() -> TestResult {
    let (_tmp, repo) = setup();
    break_the_store(&repo)?;
    // A directory where the log goes: nothing can be appended.
    std::fs::create_dir_all(state_of(&repo).join("hook-errors.log"))?;
    let start = json!({"hook_event_name": "SessionStart", "session_id": "s1", "cwd": repo, "source": "startup"});

    let printed = hook(&repo, &start);

    let message = system_message(&printed)?;
    assert!(
        message.contains("Weigh-In: the SessionStart hook failed, so capture may be off: ")
            && message.contains("It could not be logged: ")
            && message.contains("is not a regular file, so weighin leaves it alone"),
        "{message}"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_session_start_that_logged_its_failure_reports_it_once() -> TestResult {
    let (_tmp, repo) = setup();
    break_the_store(&repo)?;
    let start = json!({"hook_event_name": "SessionStart", "session_id": "s1", "cwd": repo, "source": "startup"});

    let printed = hook(&repo, &start);

    let message = system_message(&printed)?;
    assert!(
        message.contains("hook error(s) since the last session"),
        "{message}"
    );
    assert!(!message.contains("hook failed"), "{message}");
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn other_events_and_no_event_stay_silent() -> TestResult {
    let (_tmp, repo) = setup();

    // Input that is not JSON fails each of them, and is logged inside git.
    let printed = [
        weighin(&repo, "s1", &["hook", "Notification"], Some("not json")),
        weighin(&repo, "s1", &["hook"], Some("not json")),
    ];

    assert_eq!(printed, [(true, String::new()), (true, String::new())]);
    let log = logged(&repo)?;
    assert!(
        log.len() == 2
            && log
                .first()
                .is_some_and(|l| l.contains(" Notification: parsing the hook input: "))
            && log
                .get(1)
                .is_some_and(|l| l.contains(" hook: parsing the hook input: ")),
        "{log:?}"
    );
    Ok(())
}

#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_named_stop_whose_input_is_not_json_notes_it() -> TestResult {
    let (_tmp, repo) = setup();

    let (ok, printed) = weighin(&repo, "s1", &["hook", "Stop"], Some("not json"));

    assert!(ok, "{printed}");
    let message = system_message(&printed)?;
    assert!(
        message.starts_with(
            "Weigh-In: the Stop hook failed, so capture may be off: parsing the hook input: "
        ),
        "{message}"
    );
    Ok(())
}

#[cfg(unix)]
#[test]
#[cfg_attr(miri, ignore = "spawns the weighin binary and git")]
fn a_symlinked_error_log_is_left_alone_and_the_failure_is_noted() -> TestResult {
    let (tmp, repo) = setup();
    break_the_store(&repo)?;
    std::fs::create_dir_all(state_of(&repo))?;
    let elsewhere = tmp.path().join("elsewhere.log");
    std::fs::write(&elsewhere, "kept\n")?;
    std::os::unix::fs::symlink(&elsewhere, state_of(&repo).join("hook-errors.log"))?;

    let printed = hook(&repo, &stop(&repo));

    let message = system_message(&printed)?;
    assert!(
        message.starts_with("Weigh-In: the Stop hook failed, so capture may be off: ")
            && message.contains("It could not be logged: ")
            && message.contains("is not a regular file, so weighin leaves it alone"),
        "{message}"
    );
    assert_eq!(std::fs::read_to_string(&elsewhere)?, "kept\n");
    Ok(())
}
